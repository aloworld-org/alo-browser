/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where an agent outside this browser may have a page saved as a PDF, and
//! saving it there (queue item 956, ADR 0112 § 4).
//!
//! # Only where a person allowed
//!
//! `print_pdf` names a path, and the browser process writes the file — so
//! an agent that could name any path could have this browser write over
//! anything its person can. A person allows directories when they start
//! it (`alo --agent-saves DIR`, as many as they like), and **a path is
//! refused unless the directory it is in, read with every link followed,
//! is one of them or inside one**. With none allowed, every path is
//! refused. As with [`crate::agent_sites`], this is ours and only narrows:
//! a person's deliberate act that no agent can make, name, widen or read
//! (ADR 0026).
//!
//! On alo OS a file leaves a Flatpak through the document portal instead
//! (ADR 0112, queue item 966); this is what stands until then, and what a
//! person running alo from a terminal names.
//!
//! # A path, plainly
//!
//! Absolute, with no `.` or `..` in it, naming a file whose name ends in
//! `.pdf` — anything else is [`Unsaved::NotAPath`], said before the
//! directory is read. A directory that is not there is not one a person
//! allowed.
//!
//! # Never over anything
//!
//! The file is made new (`O_CREAT | O_EXCL`), so a file already there — or
//! a link left there, which is never followed — is refused as
//! [`Unsaved::Exists`], and so is one that appears between the judging and
//! the writing. It is made readable by its person alone, written whole and
//! synced; a write that fails removes what it made.

use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};

/// The longest path an agent may name, in characters: the longest address
/// a tab may be named by ([`crate::asking::MOST_ADDRESS`]).
pub const MOST_PATH: usize = crate::asking::MOST_ADDRESS;

/// What a file the browser writes for an agent must end in.
pub const SUFFIX: &str = ".pdf";

/// What a PDF begins with: the file's header, `%PDF-` and its version.
pub const HEADER: &[u8] = b"%PDF-";

/// The directories a person allowed a page to be saved in, each as it is
/// with every link followed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentSaves {
    directories: Vec<PathBuf>,
}

/// Why a directory a person named cannot be allowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotAllowable {
    /// It is not an absolute path.
    NotAbsolute(PathBuf),
    /// It could not be read, in the system's words.
    Unread(PathBuf, String),
    /// It is something other than a directory.
    NotADirectory(PathBuf),
}

impl core::fmt::Display for NotAllowable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotAbsolute(path) => write!(f, "{} is not an absolute path", path.display()),
            Self::Unread(path, why) => write!(f, "{} could not be read ({why})", path.display()),
            Self::NotADirectory(path) => write!(f, "{} is not a directory", path.display()),
        }
    }
}

/// Why a page was not saved where an agent asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsaved {
    /// Not an absolute path of a `.pdf` file with no `.` or `..` in it.
    NotAPath,
    /// Not in a directory the person allowed — or in none that is there.
    NotAllowedThere,
    /// Something is at the path already, and nothing is written over it.
    Exists,
    /// The file could not be written, in the system's words; nothing of
    /// it is left.
    NotWritten(String),
}

impl AgentSaves {
    /// None allowed: every path refused.
    pub fn none() -> Self {
        Self::default()
    }

    /// The directories `named`, which a person named; none if they named
    /// none.
    ///
    /// # Errors
    ///
    /// The first that is not an absolute path, cannot be read, or is not a
    /// directory.
    pub fn named(named: impl IntoIterator<Item = PathBuf>) -> Result<Self, NotAllowable> {
        let mut directories: Vec<PathBuf> = Vec::new();
        for directory in named {
            if !directory.is_absolute() {
                return Err(NotAllowable::NotAbsolute(directory));
            }
            let followed = fs::canonicalize(&directory)
                .map_err(|why| NotAllowable::Unread(directory.clone(), why.to_string()))?;
            if !followed.is_dir() {
                return Err(NotAllowable::NotADirectory(directory));
            }
            if !directories.contains(&followed) {
                directories.push(followed);
            }
        }
        Ok(Self { directories })
    }

    /// Whether a person allowed none.
    pub fn is_empty(&self) -> bool {
        self.directories.is_empty()
    }

    /// The directories allowed, each with every link followed.
    pub fn directories(&self) -> &[PathBuf] {
        &self.directories
    }

    /// Where the file `path` names would be written — its directory with
    /// every link followed, and its name — if an agent may have a page saved
    /// there, and nothing is there yet.
    ///
    /// # Errors
    ///
    /// [`Unsaved::NotAPath`], [`Unsaved::NotAllowedThere`] or
    /// [`Unsaved::Exists`].
    pub fn judge(&self, path: &str) -> Result<PathBuf, Unsaved> {
        // Read as written: a path's own components drop a `.` inside it.
        let plain = !path.split('/').any(|part| part == "." || part == "..");
        let path = Path::new(path);
        let plain = plain
            && path.is_absolute()
            && path
                .components()
                .all(|part| matches!(part, Component::RootDir | Component::Normal(_)));
        let name = path.file_name().and_then(|name| name.to_str());
        let named_a_pdf = name.is_some_and(|name| {
            name.len() > SUFFIX.len()
                && name
                    .get(name.len() - SUFFIX.len()..)
                    .is_some_and(|end| end.eq_ignore_ascii_case(SUFFIX))
        });
        let (Some(name), Some(directory), true, true) = (name, path.parent(), plain, named_a_pdf)
        else {
            return Err(Unsaved::NotAPath);
        };
        let followed = fs::canonicalize(directory).map_err(|_| Unsaved::NotAllowedThere)?;
        if !self
            .directories
            .iter()
            .any(|allowed| followed.starts_with(allowed))
        {
            return Err(Unsaved::NotAllowedThere);
        }
        let file = followed.join(name);
        // Without following a link: one left at the path is something there.
        if fs::symlink_metadata(&file).is_ok() {
            return Err(Unsaved::Exists);
        }
        Ok(file)
    }
}

/// Write `bytes` as a new file at `file`, which [`AgentSaves::judge`]
/// answered: made new, readable by its person alone, written whole and
/// synced.
///
/// # Errors
///
/// [`Unsaved::Exists`] when something is there by now, and
/// [`Unsaved::NotWritten`] for anything else the system refused, after
/// removing what was made.
pub fn save(file: &Path, bytes: &[u8]) -> Result<(), Unsaved> {
    let made = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(file);
    let mut made = match made {
        Ok(made) => made,
        Err(why) if why.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(Unsaved::Exists);
        }
        Err(why) => return Err(Unsaved::NotWritten(why.to_string())),
    };
    let written = made.write_all(bytes).and_then(|()| made.sync_all());
    drop(made);
    written.map_err(|why| {
        let _ = fs::remove_file(file);
        Unsaved::NotWritten(why.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of this test's own, gone with the value.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("alo-agent-saves-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(path.join("allowed").join("inner")).unwrap();
            fs::create_dir_all(path.join("elsewhere")).unwrap();
            Self(fs::canonicalize(&path).unwrap())
        }

        fn at(&self, rest: &str) -> String {
            self.0.join(rest).to_string_lossy().into_owned()
        }

        fn saves(&self) -> AgentSaves {
            AgentSaves::named([self.0.join("allowed")]).unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_pdf_in_an_allowed_directory_or_one_inside_it_is_let_written() {
        let scratch = Scratch::new("allowed");
        let saves = scratch.saves();
        assert_eq!(
            saves.judge(&scratch.at("allowed/page.pdf")),
            Ok(scratch.0.join("allowed").join("page.pdf"))
        );
        assert_eq!(
            saves.judge(&scratch.at("allowed/inner/Page.PDF")),
            Ok(scratch.0.join("allowed").join("inner").join("Page.PDF"))
        );
    }

    #[test]
    fn with_none_allowed_every_path_is_refused() {
        let scratch = Scratch::new("none");
        assert_eq!(
            AgentSaves::none().judge(&scratch.at("allowed/page.pdf")),
            Err(Unsaved::NotAllowedThere)
        );
    }

    #[test]
    fn a_path_outside_every_allowed_directory_is_refused() {
        let scratch = Scratch::new("outside");
        let saves = scratch.saves();
        for outside in ["elsewhere/page.pdf", "page.pdf", "allowed-not/page.pdf"] {
            assert_eq!(
                saves.judge(&scratch.at(outside)),
                Err(Unsaved::NotAllowedThere),
                "{outside}"
            );
        }
        // A directory that is not there is not one a person allowed.
        assert_eq!(
            saves.judge(&scratch.at("allowed/missing/page.pdf")),
            Err(Unsaved::NotAllowedThere)
        );
    }

    #[test]
    fn a_link_out_of_an_allowed_directory_is_followed_and_refused() {
        let scratch = Scratch::new("link");
        let link = scratch.0.join("allowed").join("out");
        std::os::unix::fs::symlink(scratch.0.join("elsewhere"), &link).unwrap();
        assert_eq!(
            scratch.saves().judge(&scratch.at("allowed/out/page.pdf")),
            Err(Unsaved::NotAllowedThere)
        );
    }

    #[test]
    fn anything_but_an_absolute_plain_path_of_a_pdf_is_not_a_path() {
        let scratch = Scratch::new("plain");
        let saves = scratch.saves();
        for not in [
            "page.pdf".to_owned(),
            "allowed/page.pdf".to_owned(),
            scratch.at("allowed/../allowed/page.pdf"),
            scratch.at("allowed/./page.pdf"),
            scratch.at("allowed/page.txt"),
            scratch.at("allowed/.pdf"),
            scratch.at("allowed/"),
            "/".to_owned(),
            String::new(),
        ] {
            assert_eq!(saves.judge(&not), Err(Unsaved::NotAPath), "{not:?}");
        }
    }

    #[test]
    fn nothing_is_written_over_a_file_or_a_link_already_there() {
        let scratch = Scratch::new("there");
        let saves = scratch.saves();
        let file = scratch.0.join("allowed").join("kept.pdf");
        fs::write(&file, b"the person's own").unwrap();
        assert_eq!(
            saves.judge(&scratch.at("allowed/kept.pdf")),
            Err(Unsaved::Exists)
        );
        assert_eq!(save(&file, b"%PDF-1.4 over"), Err(Unsaved::Exists));
        assert_eq!(fs::read(&file).unwrap(), b"the person's own");
        let link = scratch.0.join("allowed").join("link.pdf");
        std::os::unix::fs::symlink(scratch.0.join("elsewhere").join("x.pdf"), &link).unwrap();
        assert_eq!(
            saves.judge(&scratch.at("allowed/link.pdf")),
            Err(Unsaved::Exists)
        );
        assert_eq!(save(&link, b"%PDF-1.4"), Err(Unsaved::Exists));
        assert!(!scratch.0.join("elsewhere").join("x.pdf").exists());
    }

    #[test]
    fn a_file_is_written_whole_and_readable_by_its_person_alone() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = Scratch::new("written");
        let file = scratch
            .saves()
            .judge(&scratch.at("allowed/new.pdf"))
            .unwrap();
        save(&file, b"%PDF-1.7 the page").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"%PDF-1.7 the page");
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn a_directory_named_that_is_not_one_is_refused_as_the_browser_starts() {
        let scratch = Scratch::new("named");
        assert!(matches!(
            AgentSaves::named([PathBuf::from("relative")]),
            Err(NotAllowable::NotAbsolute(_))
        ));
        assert!(matches!(
            AgentSaves::named([scratch.0.join("missing")]),
            Err(NotAllowable::Unread(..))
        ));
        let file = scratch.0.join("a-file");
        fs::write(&file, b"").unwrap();
        assert!(matches!(
            AgentSaves::named([file]),
            Err(NotAllowable::NotADirectory(_))
        ));
        assert!(AgentSaves::named(Vec::new()).unwrap().is_empty());
    }
}
