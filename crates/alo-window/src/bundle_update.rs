/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a release's updates come from, given to the application as it is
//! made (ADR 0116 § 3, queue item 721): `alo-bundle --updates DIRECTORY`.
//!
//! `DIRECTORY` holds two files, as the application will:
//!
//! - `address`, one line: where the feed is (HTTPS);
//! - `keys`, the two public keys, as minisign writes them: the key in use,
//!   then the spare.
//!
//! They are checked as the running alo will check them
//! ([`alo_update::given::checked`]) — and **either of the worthless test
//! keys is refused**, so no application this program makes trusts a key
//! anybody can sign with — and put in `Contents/Resources/update` before
//! the application is sealed, so the seal covers them. Without
//! `--updates` the application carries neither, and does not update
//! itself: *This copy of alo does not update itself.* — every build made
//! from this repository today. The address is never in this repository
//! (the owner's rule of 9 November 2026); the private shipping repository
//! will give it.

use std::path::Path;

/// The address and the keys in `directory`, checked: what `--updates`
/// names.
///
/// # Errors
///
/// Why not, in words: a file not there or not read, an address updates
/// may not come from, keys that are not two, two the same, or a test key.
pub fn taken(directory: &Path) -> Result<(String, String), String> {
    let read = |name: &str| {
        let file = directory.join(name);
        std::fs::read_to_string(&file)
            .map_err(|why| format!("{} could not be read: {why}", file.display()))
    };
    let address = read(alo_update::given::ADDRESS)?;
    let keys = read(alo_update::given::KEYS)?;
    alo_update::given::checked(&address, &keys)
        .map_err(|why| format!("{} is refused: {why}", directory.display()))?;
    Ok((address, keys))
}

/// Put `address` and `keys` in `application`, checked again.
///
/// # Errors
///
/// Why not, in words.
pub fn put_in(application: &Path, (address, keys): (&str, &str)) -> Result<(), String> {
    let resources = application.join("Contents").join("Resources");
    alo_update::given::put_in(&resources, address, keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_update::given::{self, Updating};
    use alo_update::{Version, test_key};

    /// Two strangers' keys, built into nothing but this test.
    const KEYS: &str = "RWSnhCT54ZPK6PIMYN2tCd7c4c3o0XBhFzBi8pI/aNszPoU7TSXh7QIr\n\
                        RWQIu18dDhmlGBp3BBqO/pi/FVDRaAuNrqv121/Ga9EiKO1JOzyyXcuh\n";

    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "alo-window-bundle-update-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::create_dir_all(&root);
        root
    }

    #[test]
    fn what_updates_names_is_put_in_the_application_and_read_back_as_updating_itself() {
        let root = scratch("given");
        let named = root.join("updates");
        std::fs::create_dir_all(&named).expect("made");
        std::fs::write(named.join("address"), "https://updates.example.org/alo\n")
            .expect("written");
        std::fs::write(named.join("keys"), KEYS).expect("written");
        let (address, keys) = taken(&named).expect("taken");
        let application = root.join("alo browser.app");
        put_in(&application, (&address, &keys)).expect("put in");
        let resources = application.join("Contents").join("Resources");
        let read = given::read(
            Some(&resources),
            Version::of(0, 4, 2),
            Some("macos-arm64"),
            &root,
        );
        assert!(
            matches!(&read, Updating::Itself { address, .. } if address.as_str() == "https://updates.example.org/alo"),
            "{read:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// ADR 0116 § 6: `alo-bundle` refuses to build in a public key equal
    /// to the worthless test key, as the key in use or the spare.
    #[test]
    fn the_worthless_test_keys_are_refused_and_nothing_is_put_in() {
        let root = scratch("test-key");
        let named = root.join("updates");
        std::fs::create_dir_all(&named).expect("made");
        std::fs::write(named.join("address"), "https://updates.example.org/alo\n")
            .expect("written");
        for keys in [
            format!("{}\n{}\n", test_key::PUBLIC, test_key::SPARE),
            format!(
                "{}\n{}\n",
                KEYS.lines().next().unwrap_or_default(),
                test_key::SPARE
            ),
        ] {
            std::fs::write(named.join("keys"), &keys).expect("written");
            let refused = taken(&named);
            assert!(
                refused
                    .as_ref()
                    .is_err_and(|why| why.contains("worthless test key")),
                "{refused:?}"
            );
            let application = root.join("alo browser.app");
            assert!(put_in(&application, ("https://updates.example.org/", &keys)).is_err());
            assert!(!application.join("Contents/Resources/update").exists());
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
