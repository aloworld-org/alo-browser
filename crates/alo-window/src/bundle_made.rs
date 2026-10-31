/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The macOS application put together from the programs built beside the
//! command that makes it (queue item 687; ADR 0067 § 6).
//!
//! `alo browser.app` holds `alo`, `alo-render` and `alo-ask` in
//! `Contents/MacOS`, the application's two pages and its icon
//! ([`crate::bundle_icon`], item 689) in `Contents/Resources`, and its
//! [`crate::info_plist`]. The pages are the corpus cases
//! `the-applications-page` and `the-applications-page-without-chrome`, taken
//! into the command as it is compiled, so the pages an application carries
//! are the bytes whose renders are committed — never a copy that drifted.
//!
//! The interface's typefaces are compiled into `alo` (queue item 706, ADR
//! 0084), and their licences are carried beside it, as the SIL Open Font
//! License asks: `Contents/Resources/licences/OFL-InstrumentSans.txt` and
//! `OFL-Newsreader.txt`, the files published with the fonts
//! (`alo-renderer`'s `typefaces::LICENCES`).
//!
//! The Chromium the application draws the web with, at
//! [`crate::bundle::CHROMIUM`] (ADR 0077), is put in after, when there is
//! one to carry: alo's CEF host with CEF's framework and its licences
//! ([`crate::bundle_carried`], queue item 720). Without it the application
//! draws its page with this engine, on the page that says its own Chromium
//! is missing.
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

/// The page this engine draws when the application's own Chromium is
/// missing or damaged (ADR 0077), the corpus case
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
    /// The icon's pictures could not be put in an `.icns`.
    Icon(crate::icns::TooLarge),
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
            Self::Icon(why) => write!(f, "{}: {why}", crate::bundle_icon::FILE),
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

    /// The directory it is made under, outside the application: for what
    /// sealing it needs written and not carried, removed with it.
    pub fn making(&self) -> &Path {
        &self.making
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
    let licences = resources.join(crate::bundle_carried::LICENCES);
    for directory in [&held, &resources, &licences] {
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
    let icon = crate::bundle_icon::icns().map_err(NotMade::Icon)?;
    let mut files: Vec<(PathBuf, Vec<u8>)> = pages()
        .iter()
        .map(|(name, markup)| (resources.join(name), markup.as_bytes().to_vec()))
        .collect();
    files.push((resources.join(crate::bundle_icon::FILE), icon));
    files.extend(
        alo_renderer::typefaces::LICENCES
            .iter()
            .map(|(name, text)| (licences.join(name), text.as_bytes().to_vec())),
    );
    files.push((
        contents.join("Info.plist"),
        crate::info_plist::text().into_bytes(),
    ));
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
        assert_eq!(
            std::fs::read(contents.join("Resources/icon.icns")).ok(),
            crate::bundle_icon::icns().ok()
        );
        for (name, text) in alo_renderer::typefaces::LICENCES {
            assert_eq!(
                std::fs::read_to_string(contents.join("Resources/licences").join(name)).ok(),
                Some(text.to_owned()),
                "{name}, the licence of a typeface `alo` carries"
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
        // ADR 0077: the application's page is drawn by the Chromium it
        // carries, and the other says that Chromium is missing or damaged —
        // neither speaks of the machine's Chrome.
        assert!(THE_PAGE.contains("This page is drawn by the Chromium alo browser carries"));
        assert!(THE_PAGE_WITHOUT_CHROME.contains("own Chromium is missing or damaged"));
        assert!(!THE_PAGE.contains("Mac's Chrome"));
        assert!(!THE_PAGE_WITHOUT_CHROME.contains("Chrome was not found"));
        assert_ne!(THE_PAGE, THE_PAGE_WITHOUT_CHROME);
        // Item 720: the page links the licences of the Chromium carried,
        // where the application carries them.
        for (name, _) in crate::cef_pin::LICENCES {
            let link = format!("href=\"{}/{name}\"", crate::bundle_carried::LICENCES);
            assert!(THE_PAGE.contains(&link), "{link}");
        }
        assert!(THE_PAGE.contains("<code>Contents/Frameworks/alo Chromium.app</code>"));
        assert!(
            THE_PAGE_WITHOUT_CHROME.contains("<code>Contents/Frameworks/alo Chromium.app</code>")
        );
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
