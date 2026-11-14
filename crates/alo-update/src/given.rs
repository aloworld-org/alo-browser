/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a build was given when it was made: where updates come from and
//! the two public keys they are signed with — or nothing, and then it
//! does not update itself (ADR 0116 § 3).
//!
//! # Given at build time, as the service keys are
//!
//! `alo-bundle` puts them in the application before it is sealed, so the
//! seal covers them ([`put_in`]), from files given on its command line —
//! the private shipping repository will give them; anybody else building
//! alo gives their own, or none. They are two files in the application's
//! `Contents/Resources/update/`:
//!
//! - `address`, one line: where the feed is ([`crate::address`]);
//! - `keys`, two lines: the key in use, then the spare
//!   ([`crate::signature::Keys`]), each as minisign writes it; a
//!   `minisign.pub`'s untrusted comment line before one is passed over.
//!
//! # Whether this copy updates itself ([`Updating`])
//!
//! In order:
//!
//! 1. Inside a Flatpak (the file `/.flatpak-info` is there) it never
//!    checks: *Updates come from your software source.* Flatpak updates
//!    alo there, with its remote's own signatures, and a program inside one
//!    cannot replace itself (§ 3, alo OS).
//! 2. A platform that does not update itself ([`crate::platform`]), the
//!    workspace's own version `0.0.0` (no release), a program run outside
//!    an application, or an application given no address or keys: *This
//!    copy of alo does not update itself.* Every build from this repository
//!    today is one.
//! 3. An application given an address and keys that do not read, two keys
//!    that are the same, or **either of the worthless test keys**
//!    ([`crate::test_key`]): it does not update itself either, and says
//!    why — a build that trusted a key anybody can sign with would install
//!    anybody's browser.
//! 4. Otherwise it updates itself from that address, trusting those keys.

use crate::address::Address;
use crate::signature::{Key, Keys};
use crate::test_key;
use crate::version::Version;
use std::fs;
use std::io::Read as _;
use std::path::Path;

/// The folder under `Contents/Resources` the two files are in.
pub const FOLDER: &str = "update";

/// The file holding the address.
pub const ADDRESS: &str = "address";

/// The file holding the two keys.
pub const KEYS: &str = "keys";

/// The file whose presence says alo runs inside a Flatpak.
pub const FLATPAK_INFO: &str = ".flatpak-info";

/// What *Settings* says of a copy that does not update itself.
pub const NOT_ITSELF: &str = "This copy of alo does not update itself.";

/// What *Settings* says inside a Flatpak.
pub const SOFTWARE_SOURCE: &str = "Updates come from your software source.";

/// The most bytes either file is read to.
const LARGEST: u64 = 4096;

/// Whether, and from where, this copy updates itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Updating {
    /// It updates itself from `address`, trusting `keys`.
    Itself {
        /// Where the feed is.
        address: Address,
        /// The two keys either of which may sign.
        keys: Keys,
    },
    /// It does not; `why` says why, for the record, when there is more to
    /// say than [`NOT_ITSELF`].
    NotItself {
        /// Why, in words, if the build was given something it refused.
        why: Option<String>,
    },
    /// Inside a Flatpak: its software source updates it.
    FromSoftwareSource,
}

impl Updating {
    /// What *Settings* says when this copy does not update itself, if it
    /// does not.
    pub fn said(&self) -> Option<&'static str> {
        match self {
            Updating::Itself { .. } => None,
            Updating::NotItself { .. } => Some(NOT_ITSELF),
            Updating::FromSoftwareSource => Some(SOFTWARE_SOURCE),
        }
    }
}

/// Whether the system whose root is `root` runs alo inside a Flatpak.
pub fn inside_a_flatpak(root: &Path) -> bool {
    root.join(FLATPAK_INFO).is_file()
}

/// Whether this copy updates itself: it is `version`, run on `platform`
/// from an application whose `Contents/Resources` is `resources` (none if
/// it runs outside one), on a system whose root is `root`.
pub fn read(
    resources: Option<&Path>,
    version: Version,
    platform: Option<&str>,
    root: &Path,
) -> Updating {
    if inside_a_flatpak(root) {
        return Updating::FromSoftwareSource;
    }
    let not_itself = Updating::NotItself { why: None };
    let (Some(resources), Some(_)) = (resources, platform) else {
        return not_itself;
    };
    if !version.is_released() {
        return not_itself;
    }
    let folder = resources.join(FOLDER);
    if !folder.exists() {
        return not_itself;
    }
    match read_folder(&folder) {
        Ok((address, keys)) => Updating::Itself { address, keys },
        Err(why) => Updating::NotItself { why: Some(why) },
    }
}

/// The address and keys in `folder`, or why they do not read.
fn read_folder(folder: &Path) -> Result<(Address, Keys), String> {
    let address = bounded(&folder.join(ADDRESS))?;
    let keys = bounded(&folder.join(KEYS))?;
    checked(&address, &keys)
}

/// The address and the keys `address` and `keys` are, as a build may be
/// given them, or why not.
///
/// # Errors
///
/// Why, in words.
pub fn checked(address: &str, keys: &str) -> Result<(Address, Keys), String> {
    let address = Address::read(address).map_err(|why| format!("its update address: {why}"))?;
    let lines: Vec<&str> = keys
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("untrusted comment:"))
        .collect();
    let [in_use, spare] = lines.as_slice() else {
        return Err(format!(
            "its update keys are {} lines, and are two: the key in use and the spare",
            lines.len()
        ));
    };
    let in_use = Key::read(in_use)?;
    let spare = Key::read(spare)?;
    if in_use == spare {
        return Err("its two update keys are the same key".to_owned());
    }
    if test_key::is_a_test_key(&in_use) || test_key::is_a_test_key(&spare) {
        return Err("it was given the worthless test key, which anybody can sign with".to_owned());
    }
    Ok((address, Keys { in_use, spare }))
}

/// Put `address` and `keys` in the application whose `Contents/Resources`
/// is `resources`, after checking them as [`read`] will: what `alo-bundle`
/// does before it seals the application.
///
/// # Errors
///
/// Why not, in words; nothing is written then.
pub fn put_in(resources: &Path, address: &str, keys: &str) -> Result<(), String> {
    let (address, keys) = checked(address, keys)?;
    let folder = resources.join(FOLDER);
    let failed = |why: std::io::Error| format!("{} could not be written: {why}", folder.display());
    fs::create_dir_all(&folder).map_err(failed)?;
    fs::write(folder.join(ADDRESS), format!("{}\n", address.as_str())).map_err(failed)?;
    fs::write(
        folder.join(KEYS),
        format!("{}\n{}\n", keys.in_use.as_str(), keys.spare.as_str()),
    )
    .map_err(failed)?;
    Ok(())
}

/// `file`'s text, read to at most [`LARGEST`] bytes.
fn bounded(file: &Path) -> Result<String, String> {
    let said = |why: String| format!("{}: {why}", file.display());
    let opened = fs::File::open(file).map_err(|why| said(why.to_string()))?;
    let mut bytes = Vec::new();
    opened
        .take(LARGEST + 1)
        .read_to_end(&mut bytes)
        .map_err(|why| said(why.to_string()))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > LARGEST {
        return Err(said(format!("longer than {LARGEST} bytes")));
    }
    String::from_utf8(bytes).map_err(|_| said("not text".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A key nobody holds a test secret for: a stranger's, made with the
    /// test keys and built into nothing but this test.
    const OTHER: &str = "RWSnhCT54ZPK6PIMYN2tCd7c4c3o0XBhFzBi8pI/aNszPoU7TSXh7QIr";
    /// A fourth key, a stranger's too, for a spare that is no test key.
    const FOURTH: &str = "RWQIu18dDhmlGBp3BBqO/pi/FVDRaAuNrqv121/Ga9EiKO1JOzyyXcuh";

    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("alo-update-given-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        let _ = fs::create_dir_all(&directory);
        directory
    }

    const RELEASED: Version = Version::of(0, 4, 2);

    #[test]
    fn a_build_given_nothing_does_not_update_itself_and_says_so() {
        let resources = scratch("nothing");
        let root = scratch("nothing-root");
        let not_itself = Updating::NotItself { why: None };
        assert_eq!(
            read(Some(&resources), RELEASED, Some("macos-arm64"), &root),
            not_itself
        );
        assert_eq!(read(None, RELEASED, Some("macos-arm64"), &root), not_itself);
        assert_eq!(
            not_itself.said(),
            Some("This copy of alo does not update itself.")
        );
        let _ = fs::remove_dir_all(&resources);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_build_given_an_address_and_two_keys_updates_itself_unless_it_is_no_release() {
        let resources = scratch("given");
        let root = scratch("given-root");
        put_in(
            &resources,
            "https://updates.example.org/alo",
            &format!("{OTHER}\n{FOURTH}\n"),
        )
        .expect("put in");
        let read_back = read(Some(&resources), RELEASED, Some("macos-arm64"), &root);
        let Updating::Itself { address, keys } = &read_back else {
            panic!("{read_back:?}");
        };
        assert_eq!(address.as_str(), "https://updates.example.org/alo");
        assert_eq!((keys.in_use.as_str(), keys.spare.as_str()), (OTHER, FOURTH));
        assert_eq!(read_back.said(), None);
        // The workspace's own version, or a platform with no word, never.
        assert_eq!(
            read(
                Some(&resources),
                Version::UNRELEASED,
                Some("macos-arm64"),
                &root
            ),
            Updating::NotItself { why: None }
        );
        assert_eq!(
            read(Some(&resources), RELEASED, None, &root),
            Updating::NotItself { why: None }
        );
        let _ = fs::remove_dir_all(&resources);
        let _ = fs::remove_dir_all(&root);
    }

    /// ADR 0116 § 6: nothing a person runs trusts the worthless test key —
    /// not as the key in use, not as the spare, however it is written.
    #[test]
    fn a_build_given_the_worthless_test_key_refuses_it_and_does_not_update_itself() {
        let resources = scratch("test-key");
        let root = scratch("test-key-root");
        for keys in [
            format!("{}\n{OTHER}\n", test_key::PUBLIC),
            format!("{OTHER}\n{}\n", test_key::SPARE),
            format!(
                "untrusted comment: minisign public key\n{}\n{OTHER}",
                test_key::PUBLIC
            ),
        ] {
            let refused = put_in(&resources, "https://updates.example.org/", &keys);
            assert!(
                refused
                    .as_ref()
                    .is_err_and(|why| why.contains("worthless test key")),
                "{keys}: {refused:?}"
            );
        }
        assert!(!resources.join(FOLDER).exists(), "nothing written");
        // Written by hand past `alo-bundle`, it is refused as it is read.
        fs::create_dir_all(resources.join(FOLDER)).expect("made");
        fs::write(
            resources.join(FOLDER).join(ADDRESS),
            "https://updates.example.org/\n",
        )
        .expect("written");
        fs::write(
            resources.join(FOLDER).join(KEYS),
            format!("{}\n{}\n", test_key::PUBLIC, test_key::SPARE),
        )
        .expect("written");
        let read_back = read(Some(&resources), RELEASED, Some("macos-arm64"), &root);
        assert!(
            matches!(&read_back, Updating::NotItself { why: Some(why) } if why.contains("worthless test key")),
            "{read_back:?}"
        );
        let _ = fs::remove_dir_all(&resources);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn odd_keys_or_an_odd_address_are_refused() {
        let resources = scratch("odd");
        for (address, keys, why) in [
            ("https://e.org/", OTHER.to_owned(), "1 lines"),
            (
                "https://e.org/",
                format!("{OTHER}\n{OTHER}"),
                "the same key",
            ),
            (
                "https://e.org/",
                format!("{OTHER}\n{FOURTH}\n{OTHER}"),
                "3 lines",
            ),
            (
                "https://e.org/",
                format!("{OTHER}\nRWQ"),
                "not a minisign public key",
            ),
            ("http://e.org/", format!("{OTHER}\n{FOURTH}"), "HTTPS"),
        ] {
            let refused = put_in(&resources, address, &keys);
            assert!(
                refused.as_ref().is_err_and(|said| said.contains(why)),
                "{why}: {refused:?}"
            );
        }
        let _ = fs::remove_dir_all(&resources);
    }

    #[test]
    fn inside_a_flatpak_its_software_source_updates_it() {
        let root = scratch("flatpak-root");
        let resources = scratch("flatpak");
        put_in(
            &resources,
            "https://updates.example.org/",
            &format!("{OTHER}\n{FOURTH}"),
        )
        .expect("put in");
        assert!(!inside_a_flatpak(&root));
        fs::write(
            root.join(FLATPAK_INFO),
            "[Application]\nname=org.aloworld.Browser\n",
        )
        .expect("written");
        assert!(inside_a_flatpak(&root));
        let updating = read(Some(&resources), RELEASED, Some("macos-arm64"), &root);
        assert_eq!(updating, Updating::FromSoftwareSource);
        assert_eq!(
            updating.said(),
            Some("Updates come from your software source.")
        );
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&resources);
    }
}
