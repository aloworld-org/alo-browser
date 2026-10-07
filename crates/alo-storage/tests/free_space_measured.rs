/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 306's closing conditions, in numbers: the store's directory
//! reports a free-space figure that agrees with the volume's own report on
//! this machine, and a volume that cannot be asked bounds the profile at
//! nothing.
//!
//! The volume's own report is `df`, run on the same directory. It asks the
//! same question through a different call (`statfs` on macOS), so the two
//! agreeing is the figure being right rather than being consistent with
//! itself. They are read moments apart on a machine that is writing other
//! files, so they are held to agree within [`SLACK`] rather than to the byte.

#![cfg(unix)]

use alo_net::Partition;
use alo_storage::limits::{BUCKET_QUOTA, LOCAL_AREA, PROFILE_MOST, PROFILE_SHARE};
use alo_storage::{Limits, Refused, StorageKey, Store, volume};
use alo_url::Origin;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// How far the two reports may differ: 512 MiB, which is the most anything
/// else on this machine is likely to write or free between the two readings,
/// and a hundredth of what a profile's bound could move by it.
const SLACK: u64 = 512 << 20;

/// A directory of this test's own, not yet made.
fn somewhere(called: &str) -> PathBuf {
    let place = std::env::temp_dir().join(format!(
        "alo-storage-volume-{}-{called}",
        std::process::id().wrapping_mul(2_654_435_761)
    ));
    let _ = fs::remove_dir_all(&place);
    place
}

/// The bytes `df` says are available on the volume this path is on, in its
/// POSIX format: the fourth column of its last line, in kibibytes. [`None`]
/// when `df` fails or says something else.
fn what_df_says(path: &Path) -> Option<u64> {
    let said = Command::new("df").arg("-Pk").arg(path).output().ok()?;
    if !said.status.success() {
        return None;
    }
    let text = String::from_utf8(said.stdout).ok()?;
    let kibibytes: u64 = text
        .lines()
        .last()?
        .split_whitespace()
        .nth(3)?
        .parse()
        .ok()?;
    kibibytes.checked_mul(1024)
}

#[test]
fn the_stores_directory_reports_the_free_space_its_volume_does() {
    let place = somewhere("measured");
    let store = Store::on_its_volume(&place).expect("a store");
    let directory = store.directory().expect("a store on a disk");
    assert!(directory.is_dir(), "the store did not make its directory");

    let measured = volume::free_space(directory).expect("the volume answers");
    let reported = what_df_says(directory).expect("df reports the volume");
    assert!(measured > 0 && reported > 0);
    assert!(
        measured.abs_diff(reported) <= SLACK,
        "measured {measured} bytes free, df reports {reported}"
    );

    // The bounds the store took are ADR 0025's, from that figure.
    let limits = store.limits();
    assert_eq!(limits.bucket, BUCKET_QUOTA);
    assert_eq!(limits.local_area, LOCAL_AREA);
    let expected = PROFILE_MOST.min(reported / PROFILE_SHARE);
    assert!(
        limits.profile.abs_diff(expected) <= SLACK / PROFILE_SHARE,
        "a profile bound of {} bytes, where df's figure gives {expected}",
        limits.profile
    );
    assert!(limits.profile <= PROFILE_MOST);
    let _ = fs::remove_dir_all(&place);
}

#[test]
fn a_volume_that_cannot_be_asked_refuses_every_write() {
    let missing = somewhere("missing").join("not").join("here");
    assert_eq!(volume::free_space(&missing), None);
    let limits = Limits::for_the_volume_at(&missing);
    assert_eq!(limits.profile, 0, "an unasked volume was given room");

    let place = somewhere("unasked");
    let mut store = Store::at(&place, limits).expect("a store");
    let at = alo_url::parse("https://mail.example.com/").expect("a URL");
    let key =
        StorageKey::of(&Origin::of(&at), &Partition::of(&at)).expect("a key for a tuple origin");
    let refused = store.set_item(&key, "alo.locale", "fr");
    assert!(
        matches!(refused, Err(Refused::ProfileFull)),
        "a two-character write was not refused: {refused:?}"
    );
    assert_eq!(store.total(), 0);
    assert_eq!(store.len(), 0);
    assert_eq!(
        fs::read_dir(&place).expect("the directory").count(),
        0,
        "a refused write left a file"
    );
    let _ = fs::remove_dir_all(&place);
}
