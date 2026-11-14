/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's version, as a release names it: three numbers, `0.4.2` (ADR 0116
//! § 3).
//!
//! # Read strictly
//!
//! Exactly three parts, each digits alone, no sign, no leading zero but a
//! lone `0`, and each at most [`u32::MAX`]: so one version has one
//! spelling, and the order of two is the order of their numbers — never of
//! their text, where `0.10.0` would sort before `0.9.0`. Nothing else is
//! read: no `v`, no `-beta`, no build after a `+`. A version that cannot be
//! compared cannot be refused as older, so it is not read at all.
//!
//! # `0.0.0` never updates
//!
//! The workspace's own version ([`Version::UNRELEASED`]) is what every build
//! from this repository is today. It is never updated and never offered
//! ([`Version::is_released`]).

use core::fmt;

/// A version of alo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// Its three numbers, most significant first.
    parts: [u32; 3],
}

impl Version {
    /// The workspace's own version: a build that is no release.
    pub const UNRELEASED: Version = Version { parts: [0, 0, 0] };

    /// The version of these three numbers.
    pub const fn of(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            parts: [major, minor, patch],
        }
    }

    /// The version `text` spells, if it spells one as this file reads them.
    pub fn read(text: &str) -> Option<Self> {
        let mut parts = [0u32; 3];
        let mut pieces = text.split('.');
        for part in &mut parts {
            *part = number(pieces.next()?)?;
        }
        if pieces.next().is_some() {
            return None;
        }
        Some(Self { parts })
    }

    /// Whether this is a release's version, and not the workspace's.
    pub fn is_released(self) -> bool {
        self != Self::UNRELEASED
    }
}

/// One part: digits alone, no leading zero but a lone `0`.
fn number(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if text.len() > 1 && text.starts_with('0') {
        return None;
    }
    text.parse().ok()
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [major, minor, patch] = self.parts;
        write!(f, "{major}.{minor}.{patch}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_numbers_read_and_ordered_as_numbers() {
        assert_eq!(Version::read("0.4.2"), Some(Version::of(0, 4, 2)));
        assert!(Version::read("0.10.0") > Version::read("0.9.0"));
        assert!(Version::read("1.0.0") > Version::read("0.99.99"));
        assert_eq!(
            Version::read("4294967295.0.0"),
            Some(Version::of(u32::MAX, 0, 0))
        );
        assert_eq!(Version::of(0, 4, 2).to_string(), "0.4.2");
    }

    #[test]
    fn anything_with_a_second_spelling_or_more_is_not_read() {
        for odd in [
            "",
            "0.4",
            "0.4.2.1",
            "v0.4.2",
            "0.4.2-beta",
            "0.4.2+1",
            "00.4.2",
            "0.04.2",
            "0..2",
            "-1.0.0",
            "+1.0.0",
            " 0.4.2",
            "0.4.2 ",
            "4294967296.0.0",
            "٠.4.2",
        ] {
            assert_eq!(Version::read(odd), None, "{odd:?}");
        }
    }

    #[test]
    fn the_workspaces_own_version_is_no_release() {
        assert_eq!(Version::read("0.0.0"), Some(Version::UNRELEASED));
        assert!(!Version::UNRELEASED.is_released());
        assert!(Version::of(0, 0, 1).is_released());
    }
}
