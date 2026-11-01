/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the conversation's chats are kept between launches, and how each
//! is written there and read back (queue item 757, ADR 0092 § 1, § 3, § 4).
//!
//! # Where
//!
//! A directory of alo's own, only this user's: on a Mac,
//! `~/Library/Application Support/org.aloworld.Browser/Chats`, under the
//! application's identifier (`crate::info_plist`), as macOS asks an
//! application to keep what it makes ([`at_home`]); elsewhere
//! `$XDG_DATA_HOME/alo/chats`, or `~/.local/share/alo/chats`. `alo
//! --conversation --chats DIR` names another, which is how a hand run and
//! every test keep theirs apart from a person's. Made, if it is not there,
//! readable by this user alone (0700), and each file in it likewise (0600).
//! **Nothing in it leaves the machine**: nothing reads it but this browser
//! process, and nothing it holds is sent anywhere (ROADMAP, *no
//! telemetry*).
//!
//! # A file per chat
//!
//! `chat-N.json`, N the chat's number, never reused, given as the chat is
//! made, the newest highest — so the panel's order, newest first, is the
//! files' numbers, and no file lists the others ([`crate::chat_record`]
//! is what is in one). A chat is written when what is kept of it changes,
//! and only then; a chat with no turns is never written.
//!
//! # Written whole or not at all
//!
//! Each write goes to `chat-N.json.writing` beside it, is flushed to the
//! disk, then renamed over `chat-N.json`, and the directory flushed: the
//! rename is atomic, so a file is always the last whole write or the one
//! before it, never half of one — whatever stops alo, a crash or the power,
//! between the two. A `.writing` left by such a stop is never read, and is
//! overwritten by the next write of its chat.
//!
//! # A damaged file is said, never lost
//!
//! A `chat-N.json` that does not read as a chat ([`crate::chat_record::read`])
//! is **left where it is, untouched**, and said, with why, in this
//! browser's words ([`Opened::unread`]); its number is never given to a
//! chat again, so nothing overwrites it. Nothing is ever deleted here:
//! deleting a chat from the panel is queue item 789.

use crate::chat_record::{self, LARGEST, Record, Unread};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The directory's name under the application's own, on a Mac.
pub const DIRECTORY: &str = "Chats";

/// The identifier the application's own directory is named by.
pub const APPLICATION: &str = crate::info_plist::IDENTIFIER;

/// What every chat's file is named beginning with.
pub const PREFIX: &str = "chat-";

/// What every chat's file is named ending with.
pub const SUFFIX: &str = ".json";

/// What a file is named while it is written, after its chat's name.
pub const WRITING: &str = ".writing";

/// Where a person's chats are kept, for the person whose home is `home`
/// and whose `XDG_DATA_HOME` is `data` — read from the environment by
/// [`for_this_person`].
pub fn at_home(home: &Path, data: Option<&Path>) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join(APPLICATION)
            .join(DIRECTORY)
    } else {
        data.filter(|data| data.is_absolute())
            .map_or_else(|| home.join(".local").join("share"), Path::to_path_buf)
            .join("alo")
            .join("chats")
    }
}

/// Where this person's chats are kept, from `HOME` and `XDG_DATA_HOME`.
///
/// # Errors
///
/// Why there is nowhere: no `HOME`, or one that is not an absolute path.
pub fn for_this_person() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .ok_or_else(|| "this person has no home directory (HOME) to keep them in".to_owned())?;
    let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
    Ok(at_home(&home, data.as_deref()))
}

/// The file the chat numbered `number` is kept in, in `directory`.
pub fn file_of(directory: &Path, number: u64) -> PathBuf {
    directory.join(format!("{PREFIX}{number}{SUFFIX}"))
}

/// The number of the chat a file named `name` holds, if it is named as a
/// chat's file is.
pub fn number_of(name: &str) -> Option<u64> {
    let digits = name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// A chat read back: its number, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// Its number, which its file is named by.
    pub number: u64,
    /// Its name and turns.
    pub record: Record,
}

/// A chat's file that could not be read, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Damaged {
    /// The file, left where it is.
    pub file: PathBuf,
    /// Why it was not read, in words.
    pub why: String,
}

impl Damaged {
    /// What a person is told of it.
    pub fn said(&self) -> String {
        format!(
            "a chat kept in {} could not be read, so it is not shown, and the file is left as it \
             is: {}",
            self.file.display(),
            self.why
        )
    }
}

/// What opening the store found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Opened {
    /// Every chat read, the newest — highest numbered — first.
    pub chats: Vec<Kept>,
    /// Every chat's file that could not be read.
    pub unread: Vec<Damaged>,
    /// The highest number any chat's file is named by, read or not: every
    /// chat made after is numbered past it.
    pub highest: Option<u64>,
}

/// The chats' directory, opened: where each chat is written.
#[derive(Debug)]
pub struct ChatStore {
    directory: PathBuf,
}

impl ChatStore {
    /// Open the store at `directory`, made — only this user's — if it is
    /// not there, and read every chat's file in it.
    ///
    /// # Errors
    ///
    /// Why it cannot be kept there, in words: it could not be made, is not
    /// a directory, or could not be listed.
    pub fn open(directory: &Path) -> Result<(Self, Opened), String> {
        if !directory.is_absolute() {
            return Err(format!(
                "{} is not an absolute path to keep chats in",
                directory.display()
            ));
        }
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(|why| format!("{} could not be made: {why}", directory.display()))?;
        let listed = fs::read_dir(directory)
            .map_err(|why| format!("{} could not be listed: {why}", directory.display()))?;
        let mut opened = Opened::default();
        for entry in listed.flatten() {
            let name = entry.file_name();
            let Some(number) = name.to_str().and_then(number_of) else {
                continue;
            };
            opened.highest = opened.highest.max(Some(number));
            let file = entry.path();
            match read_file(&file) {
                Ok(record) => opened.chats.push(Kept { number, record }),
                Err(why) => opened.unread.push(Damaged { file, why }),
            }
        }
        opened
            .chats
            .sort_by_key(|kept| std::cmp::Reverse(kept.number));
        opened.unread.sort_by(|a, b| a.file.cmp(&b.file));
        let store = Self {
            directory: directory.to_path_buf(),
        };
        Ok((store, opened))
    }

    /// Where it is.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Write `written`, the chat numbered `number` as
    /// [`crate::chat_record::write`] wrote it, whole or not at all.
    ///
    /// # Errors
    ///
    /// Why it was not written, in words; the file is then as it was.
    pub fn write(&self, number: u64, written: &str) -> Result<(), String> {
        let file = file_of(&self.directory, number);
        let mut writing = file.clone().into_os_string();
        writing.push(WRITING);
        let writing = PathBuf::from(writing);
        let failed =
            |why: std::io::Error| format!("{} could not be written: {why}", file.display());
        let mut out = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&writing)
            .map_err(failed)?;
        // A `.writing` left from a build that made it wider is narrowed.
        out.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(failed)?;
        out.write_all(written.as_bytes()).map_err(failed)?;
        out.sync_all().map_err(failed)?;
        drop(out);
        fs::rename(&writing, &file).map_err(failed)?;
        // The rename itself, flushed: the directory's entry.
        File::open(&self.directory)
            .and_then(|directory| directory.sync_all())
            .map_err(failed)
    }
}

/// The chat in `file`, read whole.
fn read_file(file: &Path) -> Result<Record, String> {
    let mut bytes = Vec::new();
    File::open(file)
        .and_then(|opened| opened.take(LARGEST + 1).read_to_end(&mut bytes))
        .map_err(|why| format!("it could not be read ({why})"))?;
    chat_record::read(&bytes).map_err(|Unread(why)| why)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_renderer::turn::Turn;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A directory of its own for one test, under the temporary directory:
    /// never the person's.
    fn scratch(name: &str) -> PathBuf {
        static MADE: AtomicU32 = AtomicU32::new(0);
        let made = MADE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "alo-chat-store-{}-{name}-{made}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        directory
    }

    fn visited(said: &str) -> Vec<Turn> {
        vec![Turn::Visited {
            said: said.to_owned(),
            url: alo_url::parse("https://a.example/").unwrap(),
            ran: Vec::new(),
        }]
    }

    #[test]
    fn chats_are_kept_at_home_under_the_applications_identifier() {
        let home = Path::new("/Users/someone");
        if cfg!(target_os = "macos") {
            assert_eq!(
                at_home(home, Some(Path::new("/elsewhere"))),
                PathBuf::from(
                    "/Users/someone/Library/Application Support/org.aloworld.Browser/Chats"
                )
            );
        } else {
            assert_eq!(
                at_home(home, None),
                PathBuf::from("/Users/someone/.local/share/alo/chats")
            );
            assert_eq!(
                at_home(home, Some(Path::new("/d"))),
                PathBuf::from("/d/alo/chats")
            );
        }
    }

    #[test]
    fn a_files_number_is_read_only_from_a_chats_name() {
        assert_eq!(number_of("chat-0.json"), Some(0));
        assert_eq!(number_of("chat-17.json"), Some(17));
        for not in [
            "chat-.json",
            "chat-1.json.writing",
            "chat--1.json",
            "chat-+1.json",
            "chat-1a.json",
            "Chat-1.json",
            "chat-99999999999999999999999.json",
            ".DS_Store",
        ] {
            assert_eq!(number_of(not), None, "{not}");
        }
        assert_eq!(file_of(Path::new("/k"), 3), PathBuf::from("/k/chat-3.json"));
    }

    #[test]
    fn a_chat_written_is_read_back_newest_first_and_only_this_users() {
        let directory = scratch("kept");
        let (store, opened) = ChatStore::open(&directory).expect("opened");
        assert_eq!(opened, Opened::default(), "nothing there yet");
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&directory), 0o700);
        store
            .write(1, &chat_record::write("first", &visited("a")))
            .expect("written");
        store
            .write(4, &chat_record::write("second", &visited("b")))
            .expect("written");
        // Written again: replaced whole.
        store
            .write(1, &chat_record::write("first again", &visited("c")))
            .expect("written");
        assert_eq!(mode(&file_of(&directory, 1)), 0o600);
        assert!(
            !directory.join("chat-1.json.writing").exists(),
            "renamed into place"
        );
        let (_, opened) = ChatStore::open(&directory).expect("opened again");
        let names: Vec<(u64, &str)> = opened
            .chats
            .iter()
            .map(|kept| (kept.number, kept.record.name.as_str()))
            .collect();
        assert_eq!(names, vec![(4, "second"), (1, "first again")]);
        assert_eq!(opened.highest, Some(4));
        assert!(opened.unread.is_empty());
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_damaged_file_is_said_and_left_as_it_is_and_its_number_is_never_given_again() {
        let directory = scratch("damaged");
        let (store, _) = ChatStore::open(&directory).expect("opened");
        store
            .write(2, &chat_record::write("good", &visited("a")))
            .expect("written");
        let damaged = file_of(&directory, 7);
        fs::write(&damaged, b"{\"format\":\"alo chat\",\"version\":1,\"na").unwrap();
        // Half a write, and files that are not chats: never read.
        fs::write(directory.join("chat-9.json.writing"), b"{").unwrap();
        fs::write(directory.join("notes.txt"), b"mine").unwrap();
        let (_, opened) = ChatStore::open(&directory).expect("opened");
        assert_eq!(opened.chats.len(), 1);
        assert_eq!(opened.highest, Some(7), "the damaged file's number is held");
        let [unread] = opened.unread.as_slice() else {
            panic!("{opened:?}");
        };
        assert_eq!(unread.file, damaged);
        assert!(unread.why.contains("not JSON"), "{}", unread.why);
        assert!(
            unread.said().contains("is left as it is"),
            "{}",
            unread.said()
        );
        assert_eq!(
            fs::read(&damaged).unwrap(),
            b"{\"format\":\"alo chat\",\"version\":1,\"na",
            "untouched"
        );
        assert_eq!(fs::read(directory.join("notes.txt")).unwrap(), b"mine");
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_store_that_cannot_be_made_says_why() {
        let directory = scratch("blocked");
        fs::write(&directory, b"a file, not a directory").unwrap();
        let refused = ChatStore::open(&directory.join("Chats"))
            .err()
            .unwrap_or_default();
        assert!(refused.contains("could not be made"), "{refused}");
        let refused = ChatStore::open(Path::new("relative/chats"))
            .err()
            .unwrap_or_default();
        assert!(refused.contains("not an absolute path"), "{refused}");
        let _ = fs::remove_file(&directory);
    }
}
