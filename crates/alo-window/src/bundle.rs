/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether `alo` runs inside the macOS application, and what it opens there
//! when a person named nothing (queue item 477; ADR 0067 § 3, ADR 0068).
//!
//! An application opened from the Finder is given no arguments, its input
//! is `/dev/null` and its directory `/` (ADR 0067 § 1, measured). So it
//! carries a page of its own, in `Contents/Resources`, and opens it when
//! nothing is named: in the machine's Chrome, driven by this browser, or —
//! on a Mac with no Chrome where its installer puts it — drawn by this
//! engine on a second page that says so (ADR 0068 § 3). Both pages are
//! markup we wrote, kept as the corpus cases `the-applications-page` and
//! `the-applications-page-without-chrome` with their committed renders.
//!
//! Where the program is decides it, the way [`crate::beside`] finds the
//! renderer: a program at `NAME.app/Contents/MacOS/` whose
//! `Contents/Resources` holds the page. Never the environment, and never
//! the working directory, which macOS sets to `/`.

use crate::conductor::Fonts;
use crate::opening::{Opening, Renderer};
use std::path::{Path, PathBuf};

/// The file name of the page the application opens in Chrome.
pub const PAGE: &str = "page.html";

/// The file name of the page this engine draws where there is no Chrome.
pub const WITHOUT_CHROME: &str = "without-chrome.html";

/// The macOS application a program runs inside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    /// Its `Contents/Resources`, holding [`PAGE`].
    resources: PathBuf,
}

impl Bundle {
    /// The application the program at `program` runs inside, if it is at
    /// `NAME.app/Contents/MacOS/` and the application carries its page.
    pub fn around(program: &Path) -> Option<Self> {
        let programs = program.parent()?;
        let contents = programs.parent()?;
        let application = contents.parent()?;
        let named = programs.file_name()? == "MacOS"
            && contents.file_name()? == "Contents"
            && application.extension()? == "app";
        let resources = contents.join("Resources");
        (named && resources.join(PAGE).is_file()).then_some(Self { resources })
    }

    /// The page it opens in Chrome.
    pub fn page(&self) -> PathBuf {
        self.resources.join(PAGE)
    }

    /// The page this engine draws on a Mac with no Chrome.
    pub fn page_without_chrome(&self) -> PathBuf {
        self.resources.join(WITHOUT_CHROME)
    }

    /// What `alo` opens inside this application when it was asked for
    /// `opening`, and whether the machine has a Chrome.
    ///
    /// A page named is opened as it always is. Nothing named — no page and
    /// no `--chrome` — opens [`Bundle::page`] in Chrome; with no Chrome,
    /// [`Bundle::page_without_chrome`] in this engine. An option only this
    /// engine uses (`--scale`, `--frozen-fonts`) beside nothing named is a
    /// person asking for this engine from a terminal, and it opens the empty
    /// tab it always has: the application's page would say Chrome draws it.
    pub fn opening(&self, opening: Opening, chrome: bool) -> Opening {
        let named = opening.page.is_some() || opening.renderer == Renderer::Chrome;
        if named {
            return opening;
        }
        if !chrome {
            return Opening {
                page: Some(self.page_without_chrome()),
                ..opening
            };
        }
        if opening.scale.is_some() || opening.fonts == Fonts::AsStarted {
            return opening;
        }
        Opening {
            page: Some(self.page()),
            renderer: Renderer::Chrome,
            ..opening
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::Scale;

    /// An application of this test's own called `application`, carrying its
    /// page or not: its programs' directory.
    fn shaped(name: &str, application: &str, page: bool) -> Option<PathBuf> {
        let root = std::env::temp_dir().join(format!("alo-bundle-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let contents = root.join(application).join("Contents");
        std::fs::create_dir_all(contents.join("MacOS")).ok()?;
        std::fs::create_dir_all(contents.join("Resources")).ok()?;
        if page {
            std::fs::write(contents.join("Resources").join(PAGE), "<p>here").ok()?;
        }
        Some(contents.join("MacOS"))
    }

    /// Remove what [`shaped`] made around `programs`.
    fn unshape(programs: &Path) {
        if let Some(root) = programs.ancestors().nth(3) {
            let _ = std::fs::remove_dir_all(root);
        }
    }

    fn opening(words: &[&str]) -> Option<Opening> {
        Opening::from_arguments(words.iter().map(|word| (*word).to_owned())).ok()
    }

    #[test]
    fn a_program_inside_an_application_carrying_its_page_is_in_a_bundle() {
        let Some(programs) = shaped("inside", "alo browser.app", true) else {
            panic!("no directory");
        };
        let bundle = Bundle::around(&programs.join("alo"));
        let resources = programs.with_file_name("Resources");
        assert_eq!(
            bundle.as_ref().map(Bundle::page),
            Some(resources.join("page.html"))
        );
        assert_eq!(
            bundle.as_ref().map(Bundle::page_without_chrome),
            Some(resources.join("without-chrome.html"))
        );
        unshape(&programs);
    }

    #[test]
    fn a_program_anywhere_else_is_not() {
        let Some(bare) = shaped("bare", "alo browser.app", false) else {
            panic!("no directory");
        };
        assert_eq!(Bundle::around(&bare.join("alo")), None, "no page");
        let Some(folder) = shaped("folder", "alo browser", true) else {
            panic!("no directory");
        };
        assert_eq!(Bundle::around(&folder.join("alo")), None, "not an .app");
        unshape(&bare);
        unshape(&folder);
        for program in ["/", "alo", "/usr/local/bin/alo", "target/debug/alo"] {
            assert_eq!(Bundle::around(Path::new(program)), None, "{program}");
        }
    }

    #[test]
    fn nothing_named_opens_the_applications_page_in_chrome() {
        let bundle = Bundle {
            resources: PathBuf::from("/A.app/Contents/Resources"),
        };
        let Some(asked) = opening(&["--agent-site", "https://example.com"]) else {
            panic!("these are arguments");
        };
        let opened = bundle.opening(asked.clone(), true);
        assert_eq!(opened.renderer, Renderer::Chrome);
        assert_eq!(
            opened.page,
            Some(PathBuf::from("/A.app/Contents/Resources/page.html"))
        );
        assert_eq!(opened.agents, asked.agents, "kept to the sites named");
    }

    #[test]
    fn with_no_chrome_this_engine_draws_the_page_that_says_so() {
        let bundle = Bundle {
            resources: PathBuf::from("/A.app/Contents/Resources"),
        };
        for words in [&[][..], &["--scale", "2"][..], &["--frozen-fonts"][..]] {
            let Some(asked) = opening(words) else {
                panic!("these are arguments");
            };
            let opened = bundle.opening(asked.clone(), false);
            assert_eq!(opened.renderer, Renderer::ThisEngine, "{words:?}");
            assert_eq!(
                opened.page,
                Some(PathBuf::from(
                    "/A.app/Contents/Resources/without-chrome.html"
                )),
                "{words:?}"
            );
            assert_eq!((opened.scale, opened.fonts), (asked.scale, asked.fonts));
        }
    }

    #[test]
    fn a_page_named_or_this_engines_options_are_kept() {
        let bundle = Bundle {
            resources: PathBuf::from("/A.app/Contents/Resources"),
        };
        for words in [
            &["page.html"][..],
            &["--chrome", "https://example.com/"][..],
            &["--scale", "2"][..],
            &["--frozen-fonts"][..],
        ] {
            let Some(asked) = opening(words) else {
                panic!("these are arguments");
            };
            assert_eq!(bundle.opening(asked.clone(), true), asked, "{words:?}");
        }
        let Some(asked) = opening(&["--scale", "1.5"]) else {
            panic!("an argument");
        };
        assert_eq!(asked.scale, Some(Scale::ONE_AND_A_HALF));
        assert_eq!(bundle.opening(asked.clone(), true).page, None);
    }
}
