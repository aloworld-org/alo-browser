/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The numbers ADR 0025 § 3 chose, and the one it computes.
//!
//! Every one of them is **ours to choose and ours to change**, and the ADR says
//! how: with a measurement beside it, as a change to a constant here. None of
//! them comes from a measurement today.
//!
//! # Fixed, so that they say nothing about the machine
//!
//! [`BUCKET_QUOTA`] is the same on every machine and in private browsing,
//! because a quota computed from the disk tells every page the size of that
//! disk, and one that differs in private browsing tells every page somebody is
//! browsing privately. The profile's own bound *is* computed from the disk —
//! and is never reported to a page, which is the difference.
//!
//! # Measured once, at start
//!
//! [`Limits::for_the_volume_at`] asks the operating system how much of the
//! store's volume is free ([`crate::volume`]) and builds the bounds from that;
//! [`Limits::for_a_volume_with`] builds them from a number, for a test that
//! wants a bound it can reach. A volume that cannot be asked gives a profile
//! bound of zero, never an unbounded one.

use crate::volume;
use std::path::Path;

/// Every bucket's quota: 1 GiB, whatever the machine.
pub const BUCKET_QUOTA: u64 = 1 << 30;

/// The cap on one `localStorage` area, within its bucket's quota: 5 MiB,
/// counted as two bytes per UTF-16 code unit.
pub const LOCAL_AREA: u64 = 5 << 20;

/// The most the profile's bound can be: 8 GiB, however much is free.
pub const PROFILE_MOST: u64 = 8 << 30;

/// The share of the volume's free space at start the profile's bound may be,
/// as a divisor: a fifth.
pub const PROFILE_SHARE: u64 = 5;

/// The bounds one store enforces.
///
/// Values rather than constants alone for the reason `alo-net`'s disk cache
/// gives: a bound *is* a value, and a test wants one it can reach without
/// writing a gigabyte to find out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The most counted bytes one bucket may hold, every part of it together.
    pub bucket: u64,
    /// The most counted bytes one `localStorage` area may hold.
    pub local_area: u64,
    /// The most counted bytes every bucket in the profile may hold together.
    /// Never reported to a page.
    pub profile: u64,
}

impl Limits {
    /// ADR 0025's limits, for a profile on a volume with this many bytes free
    /// when the browser started.
    ///
    /// The profile's bound is the smaller of [`PROFILE_MOST`] and a fifth of
    /// that. A volume with almost nothing free gets a bound of almost nothing,
    /// and writes are refused with `QuotaExceededError` as on a full disk —
    /// which is what the specification allows `estimate()` to have been wrong
    /// about.
    pub fn for_a_volume_with(free_at_start: u64) -> Self {
        Self {
            bucket: BUCKET_QUOTA,
            local_area: LOCAL_AREA,
            profile: PROFILE_MOST.min(free_at_start / PROFILE_SHARE),
        }
    }

    /// ADR 0025's limits, for a profile on the volume this path is on, as that
    /// volume reports itself now.
    ///
    /// Called once, when the browser starts: the bound is a fifth of what was
    /// free *then*, and does not move as the disk fills, because a bound that
    /// shrank as the profile grew would evict a site for having been written.
    /// A volume that cannot be asked counts as nothing free, so the profile's
    /// bound is zero and every write is refused.
    pub fn for_the_volume_at(path: &Path) -> Self {
        Self::for_a_volume_with(volume::free_space(path).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_numbers_are_the_ones_the_adr_chose() {
        assert_eq!(BUCKET_QUOTA, 1_073_741_824);
        assert_eq!(LOCAL_AREA, 5_242_880);
        assert_eq!(PROFILE_MOST, 8_589_934_592);
    }

    #[test]
    fn the_profile_is_a_fifth_of_what_is_free_and_never_more_than_eight_gibibytes() {
        assert_eq!(Limits::for_a_volume_with(10 << 30).profile, 2 << 30);
        assert_eq!(Limits::for_a_volume_with(40 << 30).profile, PROFILE_MOST);
        assert_eq!(Limits::for_a_volume_with(4 << 40).profile, PROFILE_MOST);
        assert_eq!(Limits::for_a_volume_with(0).profile, 0);
        assert_eq!(Limits::for_a_volume_with(u64::MAX).profile, PROFILE_MOST);
    }

    /// Two machines with different disks give a page the same quota.
    #[test]
    fn the_quota_does_not_depend_on_the_disk() {
        let small = Limits::for_a_volume_with(2 << 30);
        let large = Limits::for_a_volume_with(2 << 40);
        assert_eq!(small.bucket, large.bucket);
        assert_eq!(small.local_area, large.local_area);
        assert_ne!(small.profile, large.profile);
    }

    #[test]
    fn a_volume_that_cannot_be_asked_bounds_the_profile_at_nothing() {
        let nowhere = std::env::temp_dir().join("alo-storage-limits-nowhere/not/here");
        let limits = Limits::for_the_volume_at(&nowhere);
        assert_eq!(limits.profile, 0);
        assert_eq!(limits.bucket, BUCKET_QUOTA);
        assert_eq!(limits.local_area, LOCAL_AREA);
    }
}
