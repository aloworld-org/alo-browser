/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a person chose in *Settings*, kept on this machine (queue item 854,
//! `docs/design/shell.md`, *Settings*; approved by the owner 2026-11-07).
//!
//! # Where, and how
//!
//! In a file of words, [`FILE`], beside the chats in alo's own directory
//! (ADR 0092 § 1) — so `--chats DIR` keeps a hand run's choice apart from a
//! person's, as it keeps the sites blocking is off for
//! ([`crate::blocking_off`]) — readable by this user alone (`0600`, in a
//! directory made `0700` if it is not there), written whole or not at all
//! as a chat is: to a `.writing` file, flushed, renamed over the file and
//! the directory flushed. It is not sealed under the chats' key (ADR 0098):
//! which face the interface is set in says nothing about a person, and a
//! choice that needs the keychain to be read back would be lost with it.
//!
//! It is [`HEADER`], then one line, `font` and the face's name as a person
//! reads it (`font Plus Jakarta Sans`) — and, since queue item 878, how far
//! alo's agent may go on one task: `sites` and a number, `actions` and a
//! number, and `asks` and `unnamed` or `every` (`sites 3`, `actions 20`,
//! `asks unnamed`), each within its bounds
//! (`alo_renderer::agent_limits`). A file written before 878 names only the
//! font, and its limits are the default. Lines that are empty or begin
//! with `#` are passed over. Nothing in it leaves the machine.
//!
//! # Read back, or Inter and said
//!
//! No file is Inter, said nothing: alo was never set otherwise. Anything
//! else that does not read as written here — a face alo does not carry, a
//! line it does not know, no `font` line or two, a limit past its bounds or
//! named twice, bytes that are not UTF-8, a file larger than any it writes,
//! or one that cannot be read at all — is **Inter and the default limits,
//! and said**, with the file's path and why, on the terminal and in the
//! first screen's line as the window opens. The file is left as it is
//! until a person next chooses a face or a limit, which writes it whole.

use alo_renderer::agent_limits::{Asks, Limits};
use alo_renderer::interface_face::InterfaceFace;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The file the choices are kept in, in alo's directory.
pub const FILE: &str = "settings.txt";

/// What the file says first.
pub const HEADER: &str = "# alo: what a person chose in Settings (docs/design/shell.md)";

/// The word a line naming the face begins with.
const FONT: &str = "font";

/// The word a line giving the sites a run may open begins with.
const SITES: &str = "sites";

/// The word a line giving the actions a run may take begins with.
const ACTIONS: &str = "actions";

/// The word a line saying before which sites alo asks begins with.
const ASKS: &str = "asks";

/// The most bytes a settings file is read to: far past any alo writes.
pub const LARGEST: u64 = 4096;

/// The most characters of a line that does not read said back.
const SAID_OF_A_LINE: usize = 40;

/// The file of settings in `directory`.
pub fn file_in(directory: &Path) -> PathBuf {
    directory.join(FILE)
}

/// What a settings file read back as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// The face the interface is set in: the one kept, or Inter.
    pub face: InterfaceFace,
    /// How far alo's agent may go: the limits kept, or the default (queue
    /// item 878).
    pub limits: Limits,
    /// Why the file did not read, when it did not, in words naming it: the
    /// face is then Inter.
    pub said: Option<String>,
}

/// The settings kept in `file`: Inter, said nothing, when there is none;
/// Inter and why, when it does not read.
pub fn read(file: &Path) -> Read {
    match read_kept(file) {
        Ok((face, limits)) => Read {
            face,
            limits,
            said: None,
        },
        Err(why) => Read {
            face: InterfaceFace::Inter,
            limits: Limits::default(),
            said: Some(format!(
                "the settings in {} do not read ({why}), so alo is set in Inter",
                file.display()
            )),
        },
    }
}

/// The face and the limits `file` names, or why it does not read.
fn read_kept(file: &Path) -> Result<(InterfaceFace, Limits), String> {
    let opened = match File::open(file) {
        Ok(opened) => opened,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {
            return Ok((InterfaceFace::Inter, Limits::default()));
        }
        Err(why) => return Err(format!("it could not be opened: {why}")),
    };
    let mut bytes = Vec::new();
    opened
        .take(LARGEST + 1)
        .read_to_end(&mut bytes)
        .map_err(|why| format!("it could not be read: {why}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > LARGEST {
        return Err(format!("it is larger than {LARGEST} bytes"));
    }
    let text = String::from_utf8(bytes).map_err(|_| "it is not text".to_owned())?;
    let mut face = None;
    let (mut sites, mut actions, mut asks) = (None, None, None);
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (word, rest) = line
            .split_once(' ')
            .map(|(word, rest)| (word, rest.trim()))
            .ok_or_else(|| format!("it holds a line alo does not read: {:?}", said(line)))?;
        let twice = |what: &str| format!("it names {what} twice");
        match word {
            FONT => {
                let found = InterfaceFace::named(rest).ok_or_else(|| {
                    format!("it names a font alo does not carry: {:?}", said(rest))
                })?;
                if face.replace(found).is_some() {
                    return Err("it names a font twice".to_owned());
                }
            }
            // How far alo's agent may go (queue item 878).
            SITES => {
                let most = rest.parse::<u8>().map_err(|_| unread_limit(line))?;
                if sites.replace(most).is_some() {
                    return Err(twice("the sites a task may open"));
                }
            }
            ACTIONS => {
                let most = rest.parse::<u16>().map_err(|_| unread_limit(line))?;
                if actions.replace(most).is_some() {
                    return Err(twice("the actions a task may take"));
                }
            }
            ASKS => {
                let chosen = Asks::of_word(rest).ok_or_else(|| unread_limit(line))?;
                if asks.replace(chosen).is_some() {
                    return Err(twice("when alo asks"));
                }
            }
            _ => {
                return Err(format!(
                    "it holds a line alo does not read: {:?}",
                    said(line)
                ));
            }
        }
    }
    let face = face.ok_or_else(|| "it names no font".to_owned())?;
    let default = Limits::default();
    let limits = Limits::checked(
        sites.unwrap_or(default.sites),
        actions.unwrap_or(default.actions),
        asks.unwrap_or(default.asks),
    )
    .ok_or_else(|| "it names a limit for alo at work past its bounds".to_owned())?;
    Ok((face, limits))
}

/// Why a line giving a limit does not read.
fn unread_limit(line: &str) -> String {
    format!("it holds a limit alo does not read: {:?}", said(line))
}

/// At most [`SAID_OF_A_LINE`] characters of `line`, to be said back.
fn said(line: &str) -> String {
    line.chars().take(SAID_OF_A_LINE).collect()
}

/// What [`write`] puts in the file for `face` and `limits`.
pub fn text_of(face: InterfaceFace, limits: Limits) -> String {
    format!(
        "{HEADER}\n{FONT} {}\n{SITES} {}\n{ACTIONS} {}\n{ASKS} {}\n",
        face.name(),
        limits.sites,
        limits.actions,
        limits.asks.word()
    )
}

/// Keep `face` and `limits` in `file`, whole or not at all, readable by
/// this user alone; its directory made, readable by this user alone, if it
/// is not there.
///
/// # Errors
///
/// Why it was not written, in words; the file is then as it was.
pub fn write(file: &Path, face: InterfaceFace, limits: Limits) -> Result<(), String> {
    let failed = |why: std::io::Error| format!("{} could not be written: {why}", file.display());
    if let Some(directory) = file.parent()
        && !directory.is_dir()
    {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(directory)
            .map_err(failed)?;
    }
    let mut writing = file.as_os_str().to_owned();
    writing.push(".writing");
    let writing = PathBuf::from(writing);
    let mut out = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&writing)
        .map_err(failed)?;
    out.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(failed)?;
    out.write_all(text_of(face, limits).as_bytes())
        .map_err(failed)?;
    out.sync_all().map_err(failed)?;
    drop(out);
    fs::rename(&writing, file).map_err(failed)?;
    if let Some(directory) = file.parent() {
        File::open(directory)
            .and_then(|directory| directory.sync_all())
            .map_err(failed)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of this test's own, never a person's.
    fn scratch(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("alo-settings-file-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        directory
    }

    #[test]
    fn no_file_is_inter_and_nothing_said() {
        let directory = scratch("none");
        assert_eq!(
            read(&file_in(&directory)),
            Read {
                face: InterfaceFace::Inter,
                limits: Limits::default(),
                said: None
            }
        );
    }

    #[test]
    fn a_face_kept_is_read_back_in_words_only_this_users() {
        let directory = scratch("kept");
        let file = file_in(&directory);
        let limits = Limits::checked(5, 45, Asks::Every).expect("within");
        for face in InterfaceFace::ALL {
            write(&file, face, limits).expect("written");
            assert_eq!(
                read(&file),
                Read {
                    face,
                    limits,
                    said: None
                }
            );
        }
        assert_eq!(
            fs::read_to_string(&file).ok().as_deref(),
            Some(
                "# alo: what a person chose in Settings (docs/design/shell.md)\n\
                 font Instrument Sans\nsites 5\nactions 45\nasks every\n"
            )
        );
        let mode = |path: &Path| fs::metadata(path).map(|m| m.permissions().mode() & 0o777);
        assert_eq!(mode(&file).ok(), Some(0o600));
        assert_eq!(
            mode(&directory).ok(),
            Some(0o700),
            "made for this user alone"
        );
        assert!(!directory.join("settings.txt.writing").exists());
        let _ = fs::remove_dir_all(&directory);
    }

    /// Queue item 884: each of the three choices kept and read back by its
    /// word; a file from before, which wrote its old default `asks
    /// unnamed` whether it was chosen or not, read as the new default,
    /// *Never*, and nothing said.
    #[test]
    fn each_choice_of_asking_is_kept_and_an_older_files_old_default_is_never() {
        let directory = scratch("asks");
        let file = file_in(&directory);
        for asks in Asks::ALL {
            let limits = Limits::checked(3, 20, asks).expect("within");
            write(&file, InterfaceFace::Inter, limits).expect("written");
            assert_eq!(read(&file).limits.asks, asks);
            let text = fs::read_to_string(&file).unwrap_or_default();
            assert!(text.ends_with(&format!("asks {}\n", asks.word())), "{text}");
        }
        fs::write(
            &file,
            "# alo: what a person chose in Settings (docs/design/shell.md)\n\
             font Inter\nsites 4\nactions 15\nasks unnamed\n",
        )
        .expect("written");
        let older = read(&file);
        assert_eq!(
            (
                older.limits.sites,
                older.limits.actions,
                older.limits.asks,
                older.said
            ),
            (4, 15, Asks::Never, None)
        );
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_damaged_or_unknown_setting_is_inter_and_said_with_why_and_left_as_it_is() {
        let directory = scratch("damaged");
        fs::create_dir_all(&directory).expect("made");
        let file = file_in(&directory);
        let cases: [(&[u8], &str); 14] = [
            // How far alo's agent may go (queue item 878).
            (
                b"font Geist\nsites 0\n",
                "it names a limit for alo at work past its bounds",
            ),
            (
                b"font Geist\nactions 101\n",
                "it names a limit for alo at work past its bounds",
            ),
            (
                b"font Geist\nactions 22\n",
                "it names a limit for alo at work past its bounds",
            ),
            (
                b"font Geist\nsites many\n",
                "it holds a limit alo does not read: \"sites many\"",
            ),
            (
                b"font Geist\nasks always\n",
                "it holds a limit alo does not read: \"asks always\"",
            ),
            (
                b"font Geist\nsites 2\nsites 3\n",
                "it names the sites a task may open twice",
            ),
            (
                b"font Comic Sans\n",
                "it names a font alo does not carry: \"Comic Sans\"",
            ),
            (
                b"font inter\n",
                "it names a font alo does not carry: \"inter\"",
            ),
            (
                b"face Geist\n",
                "it holds a line alo does not read: \"face Geist\"",
            ),
            (
                b"fontGeist\n",
                "it holds a line alo does not read: \"fontGeist\"",
            ),
            (b"font Geist\nfont Inter\n", "it names a font twice"),
            (b"# alo\n\n", "it names no font"),
            (b"font Ge\xffist\n", "it is not text"),
            (b"", "it names no font"),
        ];
        for (bytes, why) in cases {
            fs::write(&file, bytes).expect("written");
            let read = read(&file);
            assert_eq!(read.face, InterfaceFace::Inter, "{why}");
            assert_eq!(read.limits, Limits::default(), "{why}");
            assert_eq!(
                read.said,
                Some(format!(
                    "the settings in {} do not read ({why}), so alo is set in Inter",
                    file.display()
                ))
            );
            assert_eq!(
                fs::read(&file).ok().as_deref(),
                Some(bytes),
                "left as it is"
            );
        }
        // Larger than any file alo writes: not read past its bound.
        fs::write(&file, "# ".repeat(4000)).expect("written");
        assert_eq!(
            read(&file).said,
            Some(format!(
                "the settings in {} do not read (it is larger than 4096 bytes), so alo is set \
                 in Inter",
                file.display()
            ))
        );
        // A line said back is cut, so the terminal is never a stranger's to fill.
        fs::write(&file, format!("font {}\n", "x".repeat(500))).expect("written");
        let said = read(&file).said.unwrap_or_default();
        assert!(
            said.contains(&format!("\"{}\"", "x".repeat(40))) && !said.contains(&"x".repeat(41)),
            "{said}"
        );
        // Spaces around a name and a carriage return are passed over, as an
        // editor might leave them.
        fs::write(&file, "font  DM Sans \r\n").expect("written");
        assert_eq!(read(&file).face, InterfaceFace::DmSans);
        // A file kept before 878 names only the font: its limits the
        // default, said nothing.
        fs::write(&file, "font Geist\n").expect("written");
        assert_eq!(
            read(&file),
            Read {
                face: InterfaceFace::Geist,
                limits: Limits::default(),
                said: None
            }
        );
        fs::write(&file, "font Geist\nactions 60\n").expect("written");
        assert_eq!(read(&file).limits.actions, 60);
        let _ = fs::remove_dir_all(&directory);
    }
}
