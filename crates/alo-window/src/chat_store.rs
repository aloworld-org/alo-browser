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
//! # A file per chat, sealed
//!
//! `chat-N.sealed`, N the chat's number, never reused, given as the chat is
//! made, the newest highest — so the panel's order, newest first, is the
//! files' numbers, and no file lists the others ([`crate::chat_record`]
//! is what is in one). Each is the chat's words sealed under the key in the
//! person's keychain ([`crate::chat_seal`], [`crate::chat_key`], queue item
//! 790, ADR 0098), so a copy of the file off the disk, or read by another
//! account or a program without the keychain's leave, is not the chat. A
//! chat is written when what is kept of it changes, and only then; a chat
//! with no turns is never written.
//!
//! # A chat kept in words before, sealed once
//!
//! A `chat-N.json` — the chat's words as ADR 0092 first kept them — is read
//! as it always was, then **sealed once**: the sealed file written whole
//! and flushed, and only then the words' file removed and the directory
//! flushed. Stopped between the two, the next launch finds both: the
//! sealed one is read, and the words' file removed only if it says the
//! same chat; otherwise both are left, and said. A chat that could not be
//! sealed is read from its words as before, said, and sealed at its next
//! write.
//!
//! # Written whole or not at all
//!
//! Each write goes to `chat-N.sealed.writing` beside it, is flushed to the
//! disk, then renamed over `chat-N.sealed`, and the directory flushed: the
//! rename is atomic, so a file is always the last whole write or the one
//! before it, never half of one — whatever stops alo, a crash or the power,
//! between the two. A `.writing` left by such a stop is never read, and is
//! overwritten by the next write of its chat.
//!
//! # A damaged file is said, never lost
//!
//! A chat's file that does not read as a chat — does not open under the
//! keychain's key, or does not read as a chat's words once opened
//! ([`crate::chat_record::read`]) — is **left where it is, untouched**,
//! and said, with why, in this browser's words ([`Opened::unread`]); its
//! number is never given to a chat again, so nothing overwrites it.
//!
//! # A chat deleted
//!
//! [`ChatStore::remove`] removes the files of the one chat it is given the
//! number of, once a person's undo has passed (queue item 789): its sealed
//! file, and its words' file if it was read from one and not yet sealed.
//! Never a damaged file, and never a file of another number.

use crate::chat_record::{self, LARGEST, Record, Unread};
use crate::chat_seal::{self, Key};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
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

/// What a sealed chat's file is named ending with.
pub const SEALED: &str = ".sealed";

/// What a chat kept in words, before chats were sealed, is named ending
/// with (ADR 0092 § 3).
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

/// The sealed file the chat numbered `number` is kept in, in `directory`.
pub fn file_of(directory: &Path, number: u64) -> PathBuf {
    directory.join(format!("{PREFIX}{number}{SEALED}"))
}

/// The file the chat numbered `number` was kept in as words, before chats
/// were sealed.
pub fn plain_of(directory: &Path, number: u64) -> PathBuf {
    directory.join(format!("{PREFIX}{number}{SUFFIX}"))
}

/// How a chat's file holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// Sealed under the keychain's key.
    Sealed,
    /// In words, as before chats were sealed.
    Plain,
}

/// The number of the chat a file named `name` holds, and how, if it is
/// named as a chat's file is.
pub fn number_of(name: &str) -> Option<(u64, Form)> {
    let rest = name.strip_prefix(PREFIX)?;
    let (digits, form) = if let Some(digits) = rest.strip_suffix(SEALED) {
        (digits, Form::Sealed)
    } else {
        (rest.strip_suffix(SUFFIX)?, Form::Plain)
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().map(|number| (number, form))
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
    /// How many chats kept in words were sealed as the store opened.
    pub sealed: usize,
    /// What else a person is told: a chat that could not be sealed, a
    /// words' file left beside a sealed one.
    pub said: Vec<String>,
}

/// The chats' directory, opened: where each chat is written, sealed under
/// its key.
#[derive(Debug)]
pub struct ChatStore {
    directory: PathBuf,
    key: Key,
    /// The chats read from their words and not yet sealed, by number: their
    /// words' file is removed once a sealed one is written, or with them.
    plain: RefCell<HashSet<u64>>,
}

/// The files found for one number.
#[derive(Default)]
struct Found {
    sealed: Option<PathBuf>,
    plain: Option<PathBuf>,
}

impl ChatStore {
    /// Open the store at `directory`, made — only this user's — if it is
    /// not there, every chat's file in it read under `key`, and every chat
    /// kept in words sealed.
    ///
    /// # Errors
    ///
    /// Why it cannot be kept there, in words: it could not be made, is not
    /// a directory, or could not be listed.
    pub fn open(directory: &Path, key: Key) -> Result<(Self, Opened), String> {
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
        let mut found: BTreeMap<u64, Found> = BTreeMap::new();
        for entry in listed.flatten() {
            let name = entry.file_name();
            let Some((number, form)) = name.to_str().and_then(number_of) else {
                continue;
            };
            let files = found.entry(number).or_default();
            match form {
                Form::Sealed => files.sealed = Some(entry.path()),
                Form::Plain => files.plain = Some(entry.path()),
            }
        }
        let store = Self {
            directory: directory.to_path_buf(),
            key,
            plain: RefCell::new(HashSet::new()),
        };
        let mut opened = Opened::default();
        for (number, files) in found {
            opened.highest = opened.highest.max(Some(number));
            store.read_one(number, files, &mut opened);
        }
        opened
            .chats
            .sort_by_key(|kept| std::cmp::Reverse(kept.number));
        opened.unread.sort_by(|a, b| a.file.cmp(&b.file));
        Ok((store, opened))
    }

    /// Read the chat numbered `number` from the files found for it into
    /// `opened`, sealing it if it was kept in words.
    fn read_one(&self, number: u64, files: Found, opened: &mut Opened) {
        match (files.sealed, files.plain) {
            (Some(sealed), plain) => match self.read_sealed(&sealed, number) {
                Ok(record) => {
                    if let Some(plain) = plain {
                        // Stopped between sealing and removing: removed if
                        // it says the same chat, and otherwise left.
                        if read_plain(&plain).as_ref() == Ok(&record) {
                            let _ = fs::remove_file(&plain).and_then(|()| self.flush());
                        } else {
                            opened.said.push(format!(
                                "{} is left as it is beside the sealed {}, which is the chat \
                                 shown",
                                plain.display(),
                                sealed.display()
                            ));
                        }
                    }
                    opened.chats.push(Kept { number, record });
                }
                Err(why) => {
                    opened.unread.push(Damaged { file: sealed, why });
                    if let Some(plain) = plain {
                        opened.unread.push(Damaged {
                            file: plain,
                            why: "it is a chat's words beside a sealed file of the same number \
                                  that could not be read"
                                .to_owned(),
                        });
                    }
                }
            },
            (None, Some(plain)) => match read_plain(&plain) {
                Ok(record) => {
                    let written = chat_record::write(&record.name, &record.turns);
                    match self.seal_file(number, &written) {
                        Ok(()) => match fs::remove_file(&plain).and_then(|()| self.flush()) {
                            Ok(()) => opened.sealed += 1,
                            Err(why) => opened.said.push(format!(
                                "{} was sealed, but its words' file could not be removed \
                                     ({why})",
                                plain.display()
                            )),
                        },
                        Err(why) => {
                            self.plain.borrow_mut().insert(number);
                            opened.said.push(format!(
                                "a chat kept in {} could not be sealed, so it is kept in \
                                 words until it is next written: {why}",
                                plain.display()
                            ));
                        }
                    }
                    opened.chats.push(Kept { number, record });
                }
                Err(why) => opened.unread.push(Damaged { file: plain, why }),
            },
            (None, None) => {}
        }
    }

    /// Where it is.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Write `written`, the chat numbered `number` as
    /// [`crate::chat_record::write`] wrote it, sealed, whole or not at all;
    /// a words' file it was read from is then removed.
    ///
    /// # Errors
    ///
    /// Why it was not written, in words; the file is then as it was.
    pub fn write(&self, number: u64, written: &str) -> Result<(), String> {
        self.seal_file(number, written)?;
        if self.plain.borrow_mut().remove(&number) {
            let plain = plain_of(&self.directory, number);
            fs::remove_file(&plain)
                .and_then(|()| self.flush())
                .map_err(|why| {
                    format!(
                        "{} was sealed, but its words' file could not be removed ({why})",
                        plain.display()
                    )
                })?;
        }
        Ok(())
    }

    /// Remove the chat numbered `number`: its sealed file, and its words'
    /// file if it was read from one and not yet sealed — never any other
    /// number's, and never a file that could not be read. A chat never
    /// written has nothing to remove.
    ///
    /// # Errors
    ///
    /// Why a file of it could not be removed, in words.
    pub fn remove(&self, number: u64) -> Result<(), String> {
        let mut files = vec![file_of(&self.directory, number)];
        if self.plain.borrow_mut().remove(&number) {
            files.push(plain_of(&self.directory, number));
        }
        for file in files {
            match fs::remove_file(&file) {
                Ok(()) => {}
                Err(why) if why.kind() == std::io::ErrorKind::NotFound => {}
                Err(why) => return Err(format!("{} could not be removed: {why}", file.display())),
            }
        }
        self.flush()
            .map_err(|why| format!("{} could not be flushed: {why}", self.directory.display()))
    }

    /// The words of the chat numbered `number` as its sealed file holds
    /// them, opened: what a test, or a person's own tool, reads of it.
    ///
    /// # Errors
    ///
    /// Why it could not be read or opened, in words.
    pub fn words(&self, number: u64) -> Result<String, String> {
        let bytes = read_bounded(&file_of(&self.directory, number))?;
        let opened = chat_seal::open(&self.key, number, &bytes).map_err(|why| why.to_string())?;
        String::from_utf8(opened).map_err(|_| "its words are not text".to_owned())
    }

    /// Seal `written` into the chat numbered `number`'s file, whole or not
    /// at all.
    fn seal_file(&self, number: u64, written: &str) -> Result<(), String> {
        let file = file_of(&self.directory, number);
        let mut writing = file.clone().into_os_string();
        writing.push(WRITING);
        let writing = PathBuf::from(writing);
        let failed =
            |why: std::io::Error| format!("{} could not be written: {why}", file.display());
        let sealed = chat_seal::seal(&self.key, number, written.as_bytes())?;
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
        out.write_all(&sealed).map_err(failed)?;
        out.sync_all().map_err(failed)?;
        drop(out);
        fs::rename(&writing, &file).map_err(failed)?;
        // The rename itself, flushed: the directory's entry.
        self.flush().map_err(failed)
    }

    /// The directory's entries, flushed.
    fn flush(&self) -> std::io::Result<()> {
        File::open(&self.directory).and_then(|directory| directory.sync_all())
    }

    /// The chat sealed in `file` as the chat numbered `number`, opened and
    /// read whole.
    fn read_sealed(&self, file: &Path, number: u64) -> Result<Record, String> {
        let bytes = read_bounded(file)?;
        let opened = chat_seal::open(&self.key, number, &bytes).map_err(|why| why.to_string())?;
        chat_record::read(&opened).map_err(|Unread(why)| why)
    }
}

/// At least as many bytes as sealing adds to a chat's words: its magic,
/// nonce and tag (a test holds it to them).
const SEALING: u64 = 64;

/// How long a sealed file of the largest chat may be.
const LARGEST_SEALED: u64 = LARGEST + SEALING;

/// `file`'s bytes, read whole up to one past the largest a chat's file is.
fn read_bounded(file: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    File::open(file)
        .and_then(|opened| opened.take(LARGEST_SEALED + 1).read_to_end(&mut bytes))
        .map_err(|why| format!("it could not be read ({why})"))?;
    Ok(bytes)
}

/// The chat kept in words in `file`, read whole.
fn read_plain(file: &Path) -> Result<Record, String> {
    let bytes = read_bounded(file)?;
    chat_record::read(&bytes).map_err(|Unread(why)| why)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_seal::{KEY_BYTES, MAGIC, NONCE_BYTES, TAG_BYTES};
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

    /// A test's own key: never the keychain's.
    fn key() -> Key {
        Key::of([5; KEY_BYTES])
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
        assert_eq!(number_of("chat-0.sealed"), Some((0, Form::Sealed)));
        assert_eq!(number_of("chat-17.sealed"), Some((17, Form::Sealed)));
        assert_eq!(number_of("chat-3.json"), Some((3, Form::Plain)));
        for not in [
            "chat-.sealed",
            "chat-.json",
            "chat-1.sealed.writing",
            "chat-1.json.writing",
            "chat--1.sealed",
            "chat-+1.sealed",
            "chat-1a.sealed",
            "Chat-1.sealed",
            "chat-99999999999999999999999.sealed",
            ".DS_Store",
        ] {
            assert_eq!(number_of(not), None, "{not}");
        }
        assert_eq!(
            file_of(Path::new("/k"), 3),
            PathBuf::from("/k/chat-3.sealed")
        );
        assert_eq!(
            plain_of(Path::new("/k"), 3),
            PathBuf::from("/k/chat-3.json")
        );
        let added = u64::try_from(MAGIC.len() + NONCE_BYTES + TAG_BYTES).unwrap();
        assert!(added <= SEALING, "{added}");
    }

    #[test]
    fn a_chat_written_is_sealed_and_read_back_newest_first_and_only_this_users() {
        let directory = scratch("kept");
        let (store, opened) = ChatStore::open(&directory, key()).expect("opened");
        assert_eq!(opened, Opened::default(), "nothing there yet");
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&directory), 0o700);
        store
            .write(1, &chat_record::write("first", &visited("Kampala")))
            .expect("written");
        store
            .write(4, &chat_record::write("second", &visited("b")))
            .expect("written");
        // Written again: replaced whole.
        store
            .write(1, &chat_record::write("first again", &visited("Kampala")))
            .expect("written");
        assert_eq!(mode(&file_of(&directory, 1)), 0o600);
        assert!(
            !directory.join("chat-1.sealed.writing").exists(),
            "renamed into place"
        );
        let bytes = fs::read(file_of(&directory, 1)).unwrap();
        assert!(bytes.starts_with(MAGIC));
        for word in [&b"Kampala"[..], b"first again", b"alo chat"] {
            assert!(
                !bytes.windows(word.len()).any(|window| window == word),
                "no word of it in the clear"
            );
        }
        assert!(store.words(1).unwrap().contains("Kampala"));
        let (_, opened) = ChatStore::open(&directory, key()).expect("opened again");
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
    fn a_chat_kept_in_words_is_sealed_once_and_its_words_file_removed_after() {
        let directory = scratch("converted");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let words = chat_record::write("before", &visited("Nakasero"));
        fs::write(plain_of(&directory, 2), &words).unwrap();
        let (store, opened) = ChatStore::open(&directory, key()).expect("opened");
        assert_eq!(opened.sealed, 1);
        assert_eq!(opened.chats.len(), 1);
        assert_eq!(opened.chats[0].record.name, "before");
        assert!(!plain_of(&directory, 2).exists(), "the words' file removed");
        assert_eq!(store.words(2).unwrap(), words, "sealed as it was");
        // Read again: nothing more to seal.
        let (_, opened) = ChatStore::open(&directory, key()).expect("opened");
        assert_eq!((opened.sealed, opened.chats.len()), (0, 1));
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn stopped_between_sealing_and_removing_the_same_words_are_removed_and_others_left() {
        let directory = scratch("between");
        let (store, _) = ChatStore::open(&directory, key()).expect("opened");
        let words = chat_record::write("same", &visited("a"));
        store.write(3, &words).expect("written");
        fs::write(plain_of(&directory, 3), &words).unwrap();
        let other = chat_record::write("other", &visited("b"));
        store.write(5, &words).expect("written");
        fs::write(plain_of(&directory, 5), &other).unwrap();
        let (_, opened) = ChatStore::open(&directory, key()).expect("opened");
        assert!(!plain_of(&directory, 3).exists(), "the same chat: removed");
        assert_eq!(fs::read_to_string(plain_of(&directory, 5)).unwrap(), other);
        assert_eq!(opened.said.len(), 1, "{:?}", opened.said);
        assert!(opened.said[0].contains("is left as it is"));
        assert_eq!(opened.chats.len(), 2);
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_file_sealed_under_another_key_is_said_and_left_as_it_is() {
        let directory = scratch("another-key");
        let (store, _) = ChatStore::open(&directory, Key::of([6; KEY_BYTES])).expect("opened");
        store
            .write(1, &chat_record::write("theirs", &visited("a")))
            .expect("written");
        let before = fs::read(file_of(&directory, 1)).unwrap();
        let (store, opened) = ChatStore::open(&directory, key()).expect("opened");
        assert!(opened.chats.is_empty());
        assert_eq!(opened.highest, Some(1), "its number is held");
        let [unread] = opened.unread.as_slice() else {
            panic!("{opened:?}");
        };
        assert!(
            unread.why.contains("does not open with the key"),
            "{}",
            unread.why
        );
        assert_eq!(
            fs::read(file_of(&directory, 1)).unwrap(),
            before,
            "untouched"
        );
        // A chat removed is its own file alone: the other key's is kept.
        store.remove(1).expect("removed");
        assert!(!file_of(&directory, 1).exists());
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_chat_removed_takes_its_own_files_and_no_other() {
        let directory = scratch("removed");
        let (store, _) = ChatStore::open(&directory, key()).expect("opened");
        for number in [1, 2] {
            store
                .write(number, &chat_record::write("x", &visited("a")))
                .expect("written");
        }
        fs::write(plain_of(&directory, 7), b"{").unwrap();
        fs::write(directory.join("notes.txt"), b"mine").unwrap();
        store.remove(1).expect("removed");
        assert!(!file_of(&directory, 1).exists());
        assert!(file_of(&directory, 2).exists());
        assert!(plain_of(&directory, 7).exists(), "a damaged file: never");
        assert!(directory.join("notes.txt").exists());
        // Nothing written: nothing to remove, and no refusal.
        store.remove(9).expect("nothing there");
        // A chat read from words that could not be sealed: its words go too.
        store.plain.borrow_mut().insert(2);
        fs::write(plain_of(&directory, 2), b"words").unwrap();
        store.remove(2).expect("removed");
        assert!(!file_of(&directory, 2).exists());
        assert!(!plain_of(&directory, 2).exists());
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_damaged_file_is_said_and_left_as_it_is_and_its_number_is_never_given_again() {
        let directory = scratch("damaged");
        let (store, _) = ChatStore::open(&directory, key()).expect("opened");
        store
            .write(2, &chat_record::write("good", &visited("a")))
            .expect("written");
        let damaged = plain_of(&directory, 7);
        fs::write(&damaged, b"{\"format\":\"alo chat\",\"version\":1,\"na").unwrap();
        let changed = file_of(&directory, 8);
        let mut sealed = chat_seal::seal(&key(), 8, b"{}").unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        fs::write(&changed, &sealed).unwrap();
        // Half a write, and files that are not chats: never read.
        fs::write(directory.join("chat-9.sealed.writing"), b"{").unwrap();
        fs::write(directory.join("notes.txt"), b"mine").unwrap();
        let (_, opened) = ChatStore::open(&directory, key()).expect("opened");
        assert_eq!(opened.chats.len(), 1);
        assert_eq!(
            opened.highest,
            Some(8),
            "the damaged files' numbers are held"
        );
        let [first, second] = opened.unread.as_slice() else {
            panic!("{opened:?}");
        };
        assert_eq!(first.file, damaged);
        assert!(first.why.contains("not JSON"), "{}", first.why);
        assert!(
            first.said().contains("is left as it is"),
            "{}",
            first.said()
        );
        assert_eq!(second.file, changed);
        assert!(second.why.contains("changed since"), "{}", second.why);
        assert_eq!(
            fs::read(&damaged).unwrap(),
            b"{\"format\":\"alo chat\",\"version\":1,\"na",
            "untouched"
        );
        assert_eq!(fs::read(&changed).unwrap(), sealed, "untouched");
        assert_eq!(fs::read(directory.join("notes.txt")).unwrap(), b"mine");
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_store_that_cannot_be_made_says_why() {
        let directory = scratch("blocked");
        fs::write(&directory, b"a file, not a directory").unwrap();
        let refused = ChatStore::open(&directory.join("Chats"), key())
            .err()
            .unwrap_or_default();
        assert!(refused.contains("could not be made"), "{refused}");
        let refused = ChatStore::open(Path::new("relative/chats"), key())
            .err()
            .unwrap_or_default();
        assert!(refused.contains("not an absolute path"), "{refused}");
        let _ = fs::remove_file(&directory);
    }
}
