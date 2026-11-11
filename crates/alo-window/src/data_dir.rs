/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One directory that holds everything an alo keeps, so that a second alo
//! started with another is a separate instance (`alo --data DIR`, queue
//! item 957, ADR 0112 § 5) — as Firefox's `--profile DIR --no-remote` is,
//! which alo OS asks of a browser.
//!
//! # What moves
//!
//! Under `DIR`, made readable by its person alone:
//!
//! - `Chats/` — what was chosen in *Settings* ([`crate::settings_file`])
//!   and the sites blocking is off for ([`crate::blocking_off`]), where the
//!   chats' directory is;
//! - `alo-browser/agent.sock` — the agent's socket
//!   ([`crate::socket_place::socket_under`]), so the second alo's agents
//!   reach it and never the first.
//!
//! Nothing is shared with another alo but the program: each card's
//! Chromium is its own and is thrown away with its card, as always (ADR
//! 0069, ADR 0111 § 1), and no helper serves two.
//!
//! # Nothing secret, until there is a secret store
//!
//! A sealed file is only as safe as the key it is sealed under, and a data
//! directory has no secret store to keep a key in until queue item 969
//! gives it one: a keychain file unlocked by a password this repository
//! writes would be unlocked by a public password, and protect nothing. So
//! under `--data` **no key is kept anywhere** ([`keeping`]):
//!
//! - the cards' sign-ins are held in memory only, for the run, and never
//!   written ([`crate::sign_ins::SignIns::in_memory`]); *Settings* says
//!   they are kept only until alo quits;
//! - the chats are not kept, for their key would have nowhere to be, and
//!   the window says why ([`NO_SECRET_STORE`]);
//! - a person's own service keys are not kept, and *Settings* says so; the
//!   keys the application carries still work
//!   ([`crate::service_keys::Keys::carried_only`]).
//!
//! The person's login keychain is never asked either: a second instance
//! touches nothing of the first's.

use crate::keychain::Keychain;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The chats' directory in it.
pub const CHATS: &str = "Chats";

/// Why an alo kept in a data directory keeps no chats, as the window says
/// it.
pub const NO_SECRET_STORE: &str = "this data folder has no secret store to keep the chats' \
     key in yet (queue item 969), so chats are kept only until alo quits";

/// A directory everything an alo keeps is kept in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    /// The directory at `root`, an absolute path, made readable by its
    /// person alone if it is not there. One that is there must be already:
    /// a directory of the person's is never changed to suit alo.
    ///
    /// # Errors
    ///
    /// Why not, in words: not an absolute path, one the system would not
    /// make, something there that is not a directory or is a link, or a
    /// directory others may enter.
    pub fn at(root: &Path) -> Result<Self, String> {
        if !root.is_absolute() {
            return Err(format!("{} is not an absolute path", root.display()));
        }
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(root)
            .map_err(|why| format!("{} could not be made ({why})", root.display()))?;
        let found = std::fs::symlink_metadata(root)
            .map_err(|why| format!("{} could not be read ({why})", root.display()))?;
        if !found.is_dir() {
            return Err(format!("{} is not a directory", root.display()));
        }
        if found.permissions().mode() & 0o077 != 0 {
            return Err(format!(
                "{} may be entered by others, so alo keeps nothing in it",
                root.display()
            ));
        }
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    /// The directory itself.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where its chats are kept.
    pub fn chats(&self) -> PathBuf {
        self.root.join(CHATS)
    }

    /// Where its agents' socket is.
    pub fn socket(&self) -> PathBuf {
        crate::socket_place::socket_under(&self.root)
    }
}

/// Where a conversation keeps what it keeps, decided once as alo starts.
#[derive(Debug, Clone)]
pub struct Keeping {
    /// Where its chats are kept, or why nowhere.
    pub chats: Result<PathBuf, String>,
    /// The keychain its keys are in — the person's — or [`None`] under a
    /// data directory, which keeps no key.
    pub keychain: Option<Keychain>,
    /// Its cards' sign-ins.
    pub sign_ins: std::sync::Arc<crate::sign_ins::SignIns>,
    /// Where its socket goes, when not in the person's runtime directory.
    pub socket: Option<PathBuf>,
}

/// Where a conversation keeps what it keeps: under `data` when a person
/// named one — its settings and socket, and no key, so no sign-in or chat
/// past the run; otherwise the chats in `chats` or alo's own directory,
/// the sign-ins in `sign_ins` (a hand run's, ADR 0111 § 6) or alo's own,
/// and the person's keychain.
pub fn keeping(data: Option<&DataDir>, chats: Option<PathBuf>, sign_ins: Option<&Path>) -> Keeping {
    let Some(data) = data else {
        return Keeping {
            chats: chats.map_or_else(crate::chat_store::for_this_person, Ok),
            keychain: Some(Keychain::Theirs),
            sign_ins: std::sync::Arc::new(crate::sign_ins::SignIns::for_this_alo(sign_ins)),
            socket: None,
        };
    };
    Keeping {
        chats: Ok(data.chats()),
        keychain: None,
        sign_ins: std::sync::Arc::new(crate::sign_ins::SignIns::in_memory()),
        socket: Some(data.socket()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("alo-data-dir-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn everything_kept_is_under_the_one_directory_and_it_is_private() {
        let root = scratch("under");
        let data = DataDir::at(&root.join("profile")).unwrap();
        assert_eq!(data.chats(), root.join("profile").join("Chats"));
        assert_eq!(
            data.socket(),
            root.join("profile").join("alo-browser").join("agent.sock")
        );
        let mode = std::fs::metadata(data.root()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn two_directories_share_nothing_they_keep() {
        let root = scratch("two");
        let one = DataDir::at(&root.join("one")).unwrap();
        let two = DataDir::at(&root.join("two")).unwrap();
        assert_ne!(one.chats(), two.chats());
        assert_ne!(one.socket(), two.socket());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_path_that_is_not_an_absolute_directory_is_refused() {
        assert!(DataDir::at(Path::new("relative/profile")).is_err());
        let root = scratch("file");
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("a-file");
        std::fs::write(&file, b"").unwrap();
        assert!(DataDir::at(&file).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_directory_there_already_is_taken_only_if_private_and_never_changed() {
        let root = scratch("open");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(DataDir::at(&root).is_err());
        let mode = std::fs::metadata(&root).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755, "left as it was");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(DataDir::at(&root).is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Every file under `root`, however deep.
    fn files_under(root: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let mut waiting = vec![root.to_path_buf()];
        while let Some(at) = waiting.pop() {
            for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    waiting.push(path);
                } else {
                    found.push(path);
                }
            }
        }
        found
    }

    #[test]
    fn under_a_data_directory_no_key_is_kept_and_sign_ins_live_in_memory() {
        let root = scratch("keeping");
        let data = DataDir::at(&root).unwrap();
        let keeping = keeping(Some(&data), None, None);
        assert_eq!(
            keeping.keychain, None,
            "no keychain, the person's or a file"
        );
        assert_eq!(keeping.chats, Ok(data.chats()));
        assert_eq!(keeping.socket, Some(data.socket()));
        assert_eq!(keeping.sign_ins.kept_as(), crate::sign_ins::Kept::InMemory);
        // And *Settings* says so, in the row's own words.
        let row = crate::conversation_sign_ins::shown(&keeping.sign_ins.kept_as());
        assert!(
            row.said()
                .starts_with("Sign-ins are kept only until alo quits")
        );
        // A sign-in held, and nothing written for it — nor anything else.
        let cookie = crate::kept_cookie::KeptCookie {
            name: "session".to_owned(),
            value: "secret-0957".to_owned(),
            domain: "example.com".to_owned(),
            path: "/".to_owned(),
            expires: Some(4_102_444_800.0),
            secure: true,
            http_only: true,
            same_site: None,
            priority: crate::kept_cookie::Priority::Medium,
            source_scheme: crate::kept_cookie::Scheme::Secure,
            source_port: 443,
        };
        let changes = crate::kept_cookie::Changes {
            gone: Vec::new(),
            made: vec![cookie],
        };
        keeping.sign_ins.change(&changes).unwrap();
        assert_eq!(keeping.sign_ins.cookies().len(), 1);
        assert_eq!(files_under(&root), Vec::<PathBuf>::new());
        let _ = std::fs::remove_dir_all(&root);
    }
}
