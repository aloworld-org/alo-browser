/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whose key a service is asked with, and where a person's own is kept
//! (ADR 0106 § 4, queue items 904 and 905).
//!
//! A connector that takes a key ([`Keyed`]) is asked with the first of:
//!
//! 1. **the person's own**, given in *Settings*: a generic password of
//!    the connector's own keychain service ([`Keyed::service`]) for the
//!    person's login name;
//! 2. **the application's**, carried inside a release in the connector's
//!    slot ([`crate::carried_keys`], ADR 0091) — one that cannot be read
//!    is said, and never swapped for the next;
//! 3. **an older alo's keychain item** ([`Keyed::older`]), read so nothing
//!    set up before breaks, and never written.
//!
//! With none, the service is not set up, and says so: *Searching is not set up
//! yet: add a key for the search service in Settings.*
//!
//! # Kept, removed, never said
//!
//! [`Keys::keep`] puts a person's key in their keychain through
//! [`crate::keychain::keep`] — on the tool's standard input, as
//! hexadecimal, in place of their earlier one — after checking it is a key
//! a request header can carry ([`Key::given`]); [`Keys::remove`] deletes
//! it. Nothing here writes a key anywhere else or says one: every sentence
//! names the service by what it is, and the item by its service. Tests
//! name a keychain file of their own ([`Keychain::File`]), never the
//! person's.

use crate::bundle::Bundle;
use crate::carried_keys;
use crate::keychain::{self, Keychain};
use crate::search_key::Key;
use crate::services::Keyed;
use std::path::PathBuf;

/// Where a service's key is had from: the keychain the person's own and
/// the older items are in, for whom, and the application's resources, if
/// alo runs inside one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keys {
    /// The keychain asked.
    keychain: Keychain,
    /// The account its items are for.
    account: String,
    /// The application's `Contents/Resources`, if alo runs inside one.
    resources: Option<PathBuf>,
}

/// Which key a service would be asked with now, as *Settings* says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    /// The person's own, given in *Settings*.
    Yours,
    /// The one the application carries.
    Carried,
    /// An older alo's keychain item.
    Older,
    /// None.
    None,
}

impl Keys {
    /// The keys of the person running this, in their own keychain, and of
    /// the application `bundle`, when alo runs inside one.
    pub fn for_this_application(bundle: Option<&Bundle>) -> Self {
        Self::in_keychain(
            Keychain::Theirs,
            &keychain::this_person(),
            bundle.map(|bundle| bundle.resources().to_path_buf()),
        )
    }

    /// The keys of `account` in `keychain`, and of the application whose
    /// `Contents/Resources` is `resources`: a test's own keychain file.
    pub fn in_keychain(keychain: Keychain, account: &str, resources: Option<PathBuf>) -> Self {
        Self {
            keychain,
            account: account.to_owned(),
            resources,
        }
    }

    /// The key `keyed`'s service is asked with: the person's own, else the
    /// application's, else an older alo's.
    ///
    /// # Errors
    ///
    /// Why there is none, in words a person reads — never a key: none
    /// given; the person's own not one a request can carry, or not read; a
    /// carried key that cannot be read.
    pub fn key(&self, keyed: Keyed) -> Result<Key, String> {
        if let Some(written) = keychain::find(&self.keychain, keyed.service(), &self.account)? {
            return Key::given(&written).ok_or_else(|| {
                format!(
                    "the key given in Settings for {} is not one alo can send",
                    keyed.of()
                )
            });
        }
        if let Some(resources) = &self.resources
            && let Some(carried) = carried_keys::read(resources, keyed.slot())?
        {
            return Ok(carried);
        }
        // An older alo's item: read as it was then, any failure as none.
        let older = keychain::generic_password(keyed.older(), &self.account);
        let older = match (&self.keychain, older) {
            (Keychain::Theirs, Ok(written)) => Some(written),
            (Keychain::Theirs, Err(_)) => None,
            (Keychain::File(_), _) => {
                keychain::find(&self.keychain, keyed.older(), &self.account).unwrap_or(None)
            }
        };
        older
            .and_then(|written| Key::of(&written))
            .ok_or_else(|| missing(keyed))
    }

    /// Which key `keyed`'s service would be asked with now.
    ///
    /// # Errors
    ///
    /// As [`Keys::key`], when it cannot be told.
    pub fn kept(&self, keyed: Keyed) -> Result<Kept, String> {
        match self.key(keyed) {
            Ok(key) => Ok(match key.held() {
                crate::search_key::Held::Settings => Kept::Yours,
                crate::search_key::Held::Carried => Kept::Carried,
                crate::search_key::Held::Keychain => Kept::Older,
            }),
            Err(why) if why == missing(keyed) => Ok(Kept::None),
            Err(why) => Err(why),
        }
    }

    /// Keep `written` as the person's own key for `keyed`'s service, in
    /// place of any they gave before.
    ///
    /// # Errors
    ///
    /// When it is not a key a request header can carry, or the keychain
    /// did not keep it: never the key.
    pub fn keep(&self, keyed: Keyed, written: &str) -> Result<(), String> {
        let key = Key::given(written).ok_or_else(|| {
            format!(
                "that is not a key alo can send to {}: a key is letters, digits and signs, \
                 with no spaces",
                keyed.of()
            )
        })?;
        keychain::keep(
            &self.keychain,
            (keyed.service(), &self.account, keyed.label()),
            key.header_value(),
        )
    }

    /// Remove the person's own key for `keyed`'s service: whether there
    /// was one.
    ///
    /// # Errors
    ///
    /// When the keychain did not remove it.
    pub fn remove(&self, keyed: Keyed) -> Result<bool, String> {
        keychain::delete(&self.keychain, keyed.service(), &self.account)
    }
}

/// What is said when `keyed`'s service has no key at all.
pub fn missing(keyed: Keyed) -> String {
    format!("add a key for {} in Settings", keyed.of())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search_key::Held;
    use std::process::{Command, Stdio};

    /// A keychain file of this test's own, made unlocked under the
    /// temporary directory, and deleted when dropped: never the person's.
    struct Scratch(PathBuf);

    impl Scratch {
        fn made(name: &str) -> Self {
            let file = std::env::temp_dir().join(format!(
                "alo-service-keys-{}-{name}.keychain-db",
                std::process::id()
            ));
            let _ = std::fs::remove_file(&file);
            let made = Command::new(keychain::SECURITY)
                .args(["create-keychain", "-p", "alo-test"])
                .arg(&file)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("the tool");
            assert!(made.success());
            Self(file)
        }

        fn keychain(&self) -> Keychain {
            Keychain::File(self.0.clone())
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = Command::new(keychain::SECURITY)
                .arg("delete-keychain")
                .arg(&self.0)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = std::fs::remove_file(&self.0);
        }
    }

    /// An application's resources of this test's own, carrying `key` in
    /// `keyed`'s slot.
    fn carrying(name: &str, keyed: Keyed, key: &str) -> PathBuf {
        let resources = std::env::temp_dir().join(format!(
            "alo-service-keys-{}-{name}-resources",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&resources);
        std::fs::create_dir_all(&resources).expect("made");
        carried_keys::write(&resources, keyed.slot(), &Key::carried(key).expect("a key"))
            .expect("carried");
        resources
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_persons_own_key_wins_then_the_carried_then_the_older_and_none_is_said() {
        let scratch = Scratch::made("precedence");
        let resources = carrying("precedence", Keyed::BraveSearch, "carried-not-real-0001");
        let keys = Keys::in_keychain(scratch.keychain(), "alo-test", Some(resources.clone()));
        let bare = Keys::in_keychain(scratch.keychain(), "alo-test", None);
        // None of any kind.
        assert_eq!(
            bare.key(Keyed::BraveSearch).err().as_deref(),
            Some("add a key for the search service in Settings")
        );
        assert_eq!(bare.kept(Keyed::OpenAi), Ok(Kept::None));
        // An older alo's item.
        keychain::keep(
            &scratch.keychain(),
            (Keyed::BraveSearch.older(), "alo-test", "older"),
            "older-not-real-0002",
        )
        .expect("kept");
        let older = bare.key(Keyed::BraveSearch).expect("the older");
        assert_eq!(
            (older.header_value(), older.held()),
            ("older-not-real-0002", Held::Keychain)
        );
        assert_eq!(bare.kept(Keyed::BraveSearch), Ok(Kept::Older));
        // The application's carried key goes over the older one.
        let carried = keys.key(Keyed::BraveSearch).expect("the carried");
        assert_eq!(
            (carried.header_value(), carried.held()),
            ("carried-not-real-0001", Held::Carried)
        );
        // The person's own goes over both, and another's is another's.
        keys.keep(Keyed::BraveSearch, "  yours-not-real-0003 ")
            .expect("kept");
        let yours = keys.key(Keyed::BraveSearch).expect("yours");
        assert_eq!(
            (yours.header_value(), yours.held()),
            ("yours-not-real-0003", Held::Settings)
        );
        assert_eq!(keys.kept(Keyed::BraveSearch), Ok(Kept::Yours));
        assert_eq!(keys.kept(Keyed::OpenAi), Ok(Kept::None), "its own item");
        // Given again, it replaces the first.
        keys.keep(Keyed::BraveSearch, "yours-not-real-0004")
            .expect("kept again");
        assert_eq!(
            keys.key(Keyed::BraveSearch)
                .map(|key| key.header_value().to_owned()),
            Ok("yours-not-real-0004".to_owned())
        );
        // Removed, the carried one again; removed twice, nothing there.
        assert_eq!(keys.remove(Keyed::BraveSearch), Ok(true));
        assert_eq!(keys.remove(Keyed::BraveSearch), Ok(false));
        assert_eq!(keys.kept(Keyed::BraveSearch), Ok(Kept::Carried));
        let _ = std::fs::remove_dir_all(&resources);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_key_no_header_can_carry_is_never_kept_and_a_damaged_carried_one_is_said() {
        let scratch = Scratch::made("refused");
        let keys = Keys::in_keychain(scratch.keychain(), "alo-test", None);
        for not in ["", "two words", "a\r\nX-Other: b", "é", &"k".repeat(257)] {
            let refused = keys.keep(Keyed::OpenAi, not).err().unwrap_or_default();
            assert!(
                refused.starts_with("that is not a key alo can send to the AI service"),
                "{refused}"
            );
        }
        assert_eq!(keys.kept(Keyed::OpenAi), Ok(Kept::None), "nothing was kept");
        // A key with every sign a header may carry is kept and read back
        // exactly: the tool is handed hexadecimal, so no sign is misread.
        let signs = "k-_.~!#$%&'*+^`|\"\\(),/:;<=>?@[]{}0";
        keys.keep(Keyed::OpenAi, signs).expect("kept");
        assert_eq!(
            keys.key(Keyed::OpenAi)
                .map(|key| key.header_value().to_owned()),
            Ok(signs.to_owned())
        );
        // A carried key that cannot be read is said, never swapped for the
        // older item — but the person's own still wins over it.
        let resources = std::env::temp_dir().join(format!(
            "alo-service-keys-{}-damaged-resources",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&resources);
        std::fs::create_dir_all(resources.join(carried_keys::DIRECTORY)).expect("made");
        std::fs::write(
            carried_keys::path(&resources, Keyed::BraveSearch.slot()),
            b"x",
        )
        .expect("written");
        let damaged = Keys::in_keychain(scratch.keychain(), "alo-test", Some(resources.clone()));
        keychain::keep(
            &scratch.keychain(),
            (Keyed::BraveSearch.older(), "alo-test", "older"),
            "older-not-real-0005",
        )
        .expect("kept");
        assert_eq!(
            damaged.key(Keyed::BraveSearch).err().as_deref(),
            Some("the search key this application carries could not be read (keys/search)")
        );
        damaged
            .keep(Keyed::BraveSearch, "yours-not-real-0006")
            .expect("kept");
        assert_eq!(damaged.kept(Keyed::BraveSearch), Ok(Kept::Yours));
        let _ = std::fs::remove_dir_all(&resources);
    }

    #[test]
    fn what_is_said_names_the_service_by_what_it_is_and_never_holds_a_key() {
        for keyed in Keyed::ALL {
            let said = missing(keyed);
            assert!(
                said.starts_with("add a key for the ") && said.ends_with(" in Settings"),
                "{said}"
            );
            assert!(!said.to_lowercase().contains("brave"), "{said}");
            assert!(!said.to_lowercase().contains("openai"), "{said}");
        }
        let keys = Keys::in_keychain(Keychain::Theirs, "", None);
        let said = keys
            .keep(Keyed::BraveSearch, "not-real-0007")
            .err()
            .unwrap_or_default();
        assert!(!said.contains("not-real-0007"), "{said}");
    }
}
