/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The keys a release carries, taken from outside the repository and put
//! in the macOS application as it is made (ADR 0091, queue item 748).
//!
//! `alo-bundle --keys-from keychain` takes every slot's key
//! ([`crate::carried_keys::Slot::ALL`]) from this Mac's login keychain —
//! the item the slot names, for the person running it — before anything is
//! made; `alo-bundle --keys-from DIRECTORY` takes each from the file in
//! `DIRECTORY` named as the slot is (`search`, `answer`), which is how a test gives a
//! fake key and how a release machine without the keychain item would.
//! A slot with no key is refused, saying which, and nothing is made: a key
//! asked for is never quietly left out. Without `--keys-from` the
//! application carries none, and a browser in it reads the keychain at run
//! time as a development build does.
//!
//! [`Taken::put_in`] writes them at `Contents/Resources/keys/SLOT`
//! ([`crate::carried_keys::write`]) before the application is sealed, so
//! the seal covers them. Nothing here writes a key anywhere else, or says
//! one: [`Taken`]'s `Debug` names its slots alone.

use crate::carried_keys::{self, Slot};
use crate::keychain::{self, Unread};
use crate::search_key::Key;
use core::fmt;
use std::path::{Path, PathBuf};

/// The word `--keys-from` takes for this Mac's keychain.
pub const KEYCHAIN: &str = "keychain";

/// Where the keys a release carries are taken from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// This Mac's login keychain, for this account.
    Keychain(String),
    /// A directory holding one file per slot.
    Directory(PathBuf),
}

impl Source {
    /// What `--keys-from` names by `written`: [`KEYCHAIN`] for this Mac's
    /// keychain, for the person running this; anything else a directory.
    pub fn named(written: &Path) -> Self {
        if written.as_os_str() == KEYCHAIN {
            Self::Keychain(keychain::this_person())
        } else {
            Self::Directory(written.to_path_buf())
        }
    }

    /// `slot`'s key, from here.
    fn take(&self, slot: Slot) -> Result<Key, String> {
        match self {
            Self::Keychain(account) => {
                let missing = || {
                    format!(
                        "no {} in this Mac's keychain ({}) for the application to carry",
                        slot.what(),
                        slot.service()
                    )
                };
                match keychain::generic_password(slot.service(), account) {
                    Ok(written) => Key::carried(&written).ok_or_else(missing),
                    Err(Unread::NoTool) => Err(format!(
                        "this Mac's keychain could not be read ({})",
                        keychain::SECURITY
                    )),
                    Err(Unread::Missing) => Err(missing()),
                }
            }
            Self::Directory(directory) => {
                let file = directory.join(slot.name());
                let written = std::fs::read_to_string(&file).map_err(|_| {
                    format!(
                        "no {} at {} for the application to carry",
                        slot.what(),
                        file.display()
                    )
                })?;
                Key::carried(&written).ok_or_else(|| {
                    format!(
                        "{} holds no {} a request header can carry",
                        file.display(),
                        slot.what()
                    )
                })
            }
        }
    }
}

/// Every slot's key, taken and not yet put in.
pub struct Taken(Vec<(Slot, Key)>);

impl fmt::Debug for Taken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(|(slot, _)| slot.name()))
            .finish()
    }
}

impl Taken {
    /// The slots taken, in words a person reads: *the Brave Search key*.
    pub fn said(&self) -> String {
        self.0
            .iter()
            .map(|(slot, _)| format!("the {}", slot.what()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Put every key in the application being made at `application`.
    ///
    /// # Errors
    ///
    /// What the disk said, with which file.
    pub fn put_in(&self, application: &Path) -> Result<(), String> {
        let resources = application.join("Contents").join("Resources");
        for (slot, key) in &self.0 {
            carried_keys::write(&resources, *slot, key)?;
        }
        Ok(())
    }
}

/// Every slot's key from `source`.
///
/// # Errors
///
/// The first slot with none, in a sentence naming it — never a key.
pub fn taken(source: &Source) -> Result<Taken, String> {
    Slot::ALL
        .iter()
        .map(|slot| source.take(*slot).map(|key| (*slot, key)))
        .collect::<Result<_, _>>()
        .map(Taken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search_key::Held;

    const NOT_A_KEY: &str = "not-a-real-key-to-bundle-4567";

    fn directory(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("alo-bundle-keys-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn keychain_names_the_keychain_and_anything_else_a_directory() {
        assert!(matches!(
            Source::named(Path::new("keychain")),
            Source::Keychain(_)
        ));
        assert_eq!(
            Source::named(Path::new("./keychain")),
            Source::Directory(PathBuf::from("./keychain"))
        );
    }

    #[test]
    fn a_directorys_keys_are_taken_put_in_and_read_back_as_carried() {
        let given = directory("given");
        std::fs::write(given.join("search"), format!("{NOT_A_KEY}\n")).unwrap();
        // The answer's slot is taken too (queue item 773): missing, refused.
        let said = taken(&Source::Directory(given.clone())).unwrap_err();
        assert!(said.starts_with("no OpenAI key at "), "{said}");
        std::fs::write(given.join("answer"), "not-a-real-answer-key-0987").unwrap();
        let taken = taken(&Source::Directory(given.clone())).unwrap();
        assert_eq!(format!("{taken:?}"), r#"["search", "answer"]"#);
        assert_eq!(taken.said(), "the Brave Search key, the OpenAI key");

        let application = directory("made").join("alo browser.app");
        let resources = application.join("Contents").join("Resources");
        std::fs::create_dir_all(&resources).unwrap();
        taken.put_in(&application).unwrap();
        let read = carried_keys::read(&resources, Slot::Search)
            .unwrap()
            .unwrap();
        assert_eq!(read.header_value(), NOT_A_KEY);
        assert_eq!(read.held(), Held::Carried);
        let answer = carried_keys::read(&resources, Slot::Answer)
            .unwrap()
            .unwrap();
        assert_eq!(answer.header_value(), "not-a-real-answer-key-0987");
        assert_eq!(answer.held(), Held::Carried);
        let file = std::fs::read(carried_keys::path(&resources, Slot::Search)).unwrap();
        assert!(
            !file
                .windows(NOT_A_KEY.len())
                .any(|window| window == NOT_A_KEY.as_bytes()),
            "the key is plain in its file"
        );
        let _ = std::fs::remove_dir_all(&given);
        if let Some(made) = application.parent() {
            let _ = std::fs::remove_dir_all(made);
        }
    }

    #[test]
    fn a_slot_with_no_key_is_refused_saying_which() {
        let empty = directory("empty");
        let said = taken(&Source::Directory(empty.clone())).unwrap_err();
        assert_eq!(
            said,
            format!(
                "no Brave Search key at {} for the application to carry",
                empty.join("search").display()
            )
        );
        std::fs::write(empty.join("search"), "two words").unwrap();
        let said = taken(&Source::Directory(empty.clone())).unwrap_err();
        assert!(
            said.ends_with("holds no Brave Search key a request header can carry"),
            "{said}"
        );
        assert!(!said.contains("two words"), "{said}");
        let _ = std::fs::remove_dir_all(&empty);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_keychain_with_no_item_is_refused_saying_which() {
        let said = taken(&Source::Keychain(
            "alo-no-such-account-for-a-test".to_owned(),
        ))
        .unwrap_err();
        assert_eq!(
            said,
            "no Brave Search key in this Mac's keychain (alo-brave-search) for the application to \
             carry"
        );
    }
}
