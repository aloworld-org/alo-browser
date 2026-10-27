/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The macOS application put together from the programs built beside the
//! command that makes it (queue item 687; ADR 0067 § 6).
//!
//! `alo browser.app` holds `alo`, `alo-render` and `alo-ask` in
//! `Contents/MacOS`, the application's two pages in `Contents/Resources`,
//! and its [`crate::info_plist`]. The pages are the corpus cases
//! `the-applications-page` and `the-applications-page-without-chrome`, taken
//! into the command as it is compiled, so the pages an application carries
//! are the bytes whose renders are committed — never a copy that drifted.
//!
//! It is made beside where it is going, under a directory of its own, sealed
//! there, and moved into place whole: a command that stopped halfway leaves no application a
//! person could open half-made. An application already there is refused,
//! never replaced — removing a person's file is theirs to do.
//!
//! Sealing it is [`crate::bundle_seal`]'s.

use crate::bundle::{PAGE, WITHOUT_CHROME};
use crate::info_plist::{EXECUTABLE, NAME};
use core::fmt;
use std::path::{Path, PathBuf};

/// The application's page, the corpus case `the-applications-page`.
const THE_PAGE: &str = include_str!("../../alo-corpus/cases/the-applications-page/page.html");

/// The page this engine draws on a Mac with no Chrome, the corpus case
/// `the-applications-page-without-chrome`.
const THE_PAGE_WITHOUT_CHROME: &str =
    include_str!("../../alo-corpus/cases/the-applications-page-without-chrome/page.html");

/// The programs the application holds, the one it runs first.
pub const PROGRAMS: [&str; 3] = [EXECUTABLE, crate::beside::RENDERER, "alo-ask"];

/// The application's directory name.
pub fn file_name() -> String {
    format!("{NAME}.app")
}

/// Each page the application carries: its name in `Contents/Resources`, and
/// its markup.
pub fn pages() -> [(&'static str, &'static str); 2] {
    [(PAGE, THE_PAGE), (WITHOUT_CHROME, THE_PAGE_WITHOUT_CHROME)]
}

/// Why the application was not made.
#[derive(Debug, PartialEq, Eq)]
pub enum NotMade {
    /// A program it holds is not in the directory named: `cargo build
    /// --release` has not made it there.
    NoProgram(PathBuf),
    /// An application is already where this one would go.
    AlreadyThere(PathBuf),
    /// The disk refused: what was being done, and its words.
    Disk(String, String),
}

impl fmt::Display for NotMade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProgram(program) => write!(
                f,
                "{} is not there: build the programs first, with cargo build --release",
                program.display()
            ),
            Self::AlreadyThere(application) => write!(
                f,
                "{} is already there, and is not replaced: remove it first",
                application.display()
            ),
            Self::Disk(doing, why) => write!(f, "{doing}: {why}"),
        }
    }
}

impl std::error::Error for NotMade {}

/// The application made, not yet where it is going: under a directory of
/// its own beside it, removed with whatever is in it unless the
/// application is [`Made::put_in_place`].
#[derive(Debug)]
pub struct Made {
    /// The directory it is made under.
    making: PathBuf,
    /// Where it goes.
    going: PathBuf,
}

impl Made {
    /// The application as it is made, for it to be sealed there.
    pub fn application(&self) -> PathBuf {
        self.making.join(file_name())
    }

    /// Move the application where it goes: its path there.
    ///
    /// # Errors
    ///
    /// [`NotMade::Disk`] when the disk refuses, or [`NotMade::AlreadyThere`]
    /// when an application was put there while this one was made.
    pub fn put_in_place(self) -> Result<PathBuf, NotMade> {
        if self.going.symlink_metadata().is_ok() {
            return Err(NotMade::AlreadyThere(self.going.clone()));
        }
        std::fs::rename(self.application(), &self.going).map_err(|why| {
            NotMade::Disk(
                format!("{} could not be put in place", self.going.display()),
                why.to_string(),
            )
        })?;
        Ok(self.going.clone())
    }
}

impl Drop for Made {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.making);
    }
}

/// Make the application from the programs in `programs`, to go in `into`.
///
/// # Errors
///
/// [`NotMade`]: a program missing from `programs`, an application already in
/// `into`, or the disk refusing. Nothing is left in `into` by a refusal.
pub fn make(programs: &Path, into: &Path) -> Result<Made, NotMade> {
    let going = into.join(file_name());
    if going.symlink_metadata().is_ok() {
        return Err(NotMade::AlreadyThere(going));
    }
    for program in PROGRAMS {
        let path = programs.join(program);
        if !crate::beside::is_program(&path) {
            return Err(NotMade::NoProgram(path));
        }
    }
    let made = Made {
        making: into.join(format!(".making-{}.{}", file_name(), std::process::id())),
        going,
    };
    put_together(programs, &made.application())?;
    Ok(made)
}

/// Write the application's files under `application`.
fn put_together(programs: &Path, application: &Path) -> Result<(), NotMade> {
    let contents = application.join("Contents");
    let held = contents.join("MacOS");
    let resources = contents.join("Resources");
    for directory in [&held, &resources] {
        std::fs::create_dir_all(directory).map_err(|why| {
            NotMade::Disk(
                format!("{} could not be made", directory.display()),
                why.to_string(),
            )
        })?;
    }
    for program in PROGRAMS {
        let from = programs.join(program);
        std::fs::copy(&from, held.join(program)).map_err(|why| {
            NotMade::Disk(
                format!("{} could not be copied", from.display()),
                why.to_string(),
            )
        })?;
    }
    let mut files: Vec<(PathBuf, String)> = pages()
        .iter()
        .map(|(name, markup)| (resources.join(name), (*markup).to_owned()))
        .collect();
    files.push((contents.join("Info.plist"), crate::info_plist::text()));
    for (path, text) in files {
        std::fs::write(&path, text).map_err(|why| {
            NotMade::Disk(
                format!("{} could not be written", path.display()),
                why.to_string(),
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::Bundle;

    /// A directory of this test's own, empty.
    fn directory(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("alo-made-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::create_dir_all(&root);
        root
    }

    /// Programs of this test's own, each a script that runs.
    fn programs(root: &Path) {
        for program in PROGRAMS {
            let path = root.join(program);
            let _ = std::fs::write(&path, format!("#!/bin/sh\necho {program}\n"));
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
            }
        }
    }

    #[test]
    fn the_application_holds_its_programs_pages_and_list() {
        let root = directory("whole");
        programs(&root);
        let application = root.join("alo browser.app");
        let placed = make(&root, &root).and_then(Made::put_in_place);
        assert_eq!(placed, Ok(application.clone()));
        let contents = application.join("Contents");
        for program in ["alo", "alo-render", "alo-ask"] {
            assert_eq!(
                std::fs::read_to_string(contents.join("MacOS").join(program)).ok(),
                Some(format!("#!/bin/sh\necho {program}\n")),
                "{program}"
            );
        }
        assert_eq!(
            std::fs::read_to_string(contents.join("Info.plist")).ok(),
            Some(crate::info_plist::text())
        );
        let bundle = Bundle::around(&contents.join("MacOS").join("alo"));
        assert!(bundle.is_some(), "alo knows it is in the application");
        for (name, markup) in pages() {
            assert_eq!(
                std::fs::read_to_string(contents.join("Resources").join(name)).ok(),
                Some(markup.to_owned()),
                "{name}"
            );
        }
        let left: Vec<_> = std::fs::read_dir(&root)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|e| e.file_name())
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(
            left.len(),
            PROGRAMS.len() + 1,
            "nothing half-made: {left:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_application_never_put_in_place_leaves_nothing() {
        let root = directory("dropped");
        programs(&root);
        let made = make(&root, &root);
        let application = made.as_ref().map(Made::application);
        assert!(
            application
                .as_ref()
                .is_ok_and(|path| path.join("Contents/Info.plist").is_file())
        );
        drop(made);
        let left = std::fs::read_dir(&root).map_or(0, Iterator::count);
        assert_eq!(left, PROGRAMS.len(), "only the programs");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_pages_are_the_corpus_cases() {
        assert!(THE_PAGE.contains("This page is drawn by this Mac's Chrome"));
        assert!(THE_PAGE_WITHOUT_CHROME.contains("Chrome"));
        assert_ne!(THE_PAGE, THE_PAGE_WITHOUT_CHROME);
    }

    #[test]
    fn a_program_missing_is_named_and_nothing_is_made() {
        let root = directory("missing");
        programs(&root);
        let _ = std::fs::remove_file(root.join("alo-ask"));
        assert_eq!(
            make(&root, &root).err(),
            Some(NotMade::NoProgram(root.join("alo-ask")))
        );
        let left = std::fs::read_dir(&root).map_or(0, Iterator::count);
        assert_eq!(left, 2, "the two programs, and nothing made");
        assert!(
            NotMade::NoProgram(root.join("alo-ask"))
                .to_string()
                .ends_with("is not there: build the programs first, with cargo build --release")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_application_already_there_is_kept() {
        let root = directory("there");
        programs(&root);
        let application = root.join("alo browser.app");
        let _ = std::fs::create_dir_all(&application);
        let _ = std::fs::write(application.join("a person's"), "kept");
        assert_eq!(
            make(&root, &root).err(),
            Some(NotMade::AlreadyThere(application.clone()))
        );
        assert_eq!(
            std::fs::read_to_string(application.join("a person's")).ok(),
            Some("kept".to_owned())
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
