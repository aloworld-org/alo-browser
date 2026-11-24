/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether `alo` runs inside the macOS application, what it opens there
//! when a person named nothing, and where the application carries its own
//! Chromium (queue items 477, 719 and 993; ADR 0067 § 3, ADR 0068, ADR 0077).
//!
//! An application opened from the Finder is given no arguments, its input
//! is `/dev/null` and its directory `/` (ADR 0067 § 1, measured). Since the
//! owner's decision of 15 November 2026 (queue item 993, ADR 0068's and
//! ADR 0077's addenda) it then opens **the conversation window**, as `alo
//! --conversation` does — alo behaves as any other Mac application, and
//! nothing named is never the application's own page with our frame and
//! band above it. That page, `page.html` in `Contents/Resources` (the
//! corpus case `the-applications-page`, with its committed render), is
//! still carried: it is what marks a program as inside the application
//! ([`Bundle::around`]), and `alo --chrome` naming it, from a terminal, is
//! the developer's tool that still shows it in the carried Chromium framed.
//!
//! The application's licences are reached from the conversation through
//! macOS's own *About alo* panel, whose credits are the application's
//! `Credits.html` ([`crate::bundle_credits`]).
//!
//! # Its own Chromium, and never the machine's Chrome
//!
//! The application starts only the Chromium at [`CHROMIUM`] inside its own
//! `Contents` ([`Bundle::chromium`]): `Contents/Frameworks/alo Chromium.app`,
//! an application inside the application as macOS keeps helper bundles —
//! alo's own CEF host (ADR 0079), with its program at `Contents/MacOS/alo
//! Chromium`, its helpers and CEF's framework, put there by `alo-bundle`
//! (queue item 720, [`crate::bundle_carried`]). It never uses the machine's
//! Chrome, even when one is installed (ADR 0077): the owner's words, *as if
//! the user only has alo-browser on his PC*.
//!
//! Where the program is decides it, the way [`crate::beside`] finds the
//! renderer: a program at `NAME.app/Contents/MacOS/` whose
//! `Contents/Resources` holds the page. Never the environment, and never
//! the working directory, which macOS sets to `/`.

use crate::opening::{Opening, Renderer};
use std::path::{Path, PathBuf};

/// The file name of the application's own page (the corpus case
/// `the-applications-page`).
pub const PAGE: &str = "page.html";

/// Where the application carries its own Chromium's program, under its
/// `Contents` (ADR 0077): alo's CEF host, named as
/// [`crate::cef_host::EXECUTABLE`] names it (ADR 0079, queue item 720).
pub const CHROMIUM: &str = "Frameworks/alo Chromium.app/Contents/MacOS/alo Chromium";

/// The macOS application a program runs inside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    /// Its `Contents`, holding `Resources` and [`CHROMIUM`].
    contents: PathBuf,
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
        (named && resources.join(PAGE).is_file()).then(|| Self {
            contents: contents.to_path_buf(),
            resources,
        })
    }

    /// The program of the Chromium it carries, the only one it starts
    /// ([`CHROMIUM`], ADR 0077) — whether or not one is there.
    pub fn chromium(&self) -> PathBuf {
        self.contents.join(CHROMIUM)
    }

    /// Its `Contents/Resources`, holding its pages and the keys it carries
    /// ([`crate::carried_keys`], ADR 0091).
    pub fn resources(&self) -> &Path {
        &self.resources
    }

    /// Its own page, which marks a program as inside it and which `alo
    /// --chrome` naming it shows in its own Chromium, framed — from a
    /// terminal only since queue item 993.
    pub fn page(&self) -> PathBuf {
        self.resources.join(PAGE)
    }

    /// What `alo` opens inside this application when it was asked for
    /// `opening` (queue item 993).
    ///
    /// A page named is opened as it always is — `--chrome PAGE` too, in the
    /// application's own Chromium with our frame above it — and so is the
    /// conversation asked for. Nothing named — no page, no `--chrome` and no
    /// `--conversation`, which is how macOS opens an application from the
    /// Finder or the Dock — opens the conversation, as `--conversation`
    /// would, keeping every other option given (`--scale`,
    /// `--frozen-fonts`, `--agent-site`, `--data`), each of which the
    /// conversation takes. Whether the application's Chromium is there does
    /// not change what opens: the conversation is drawn by this engine, and
    /// says why a site chosen in it cannot open when it is not
    /// ([`crate::bundle_chromium`]).
    pub fn opening(&self, opening: Opening) -> Opening {
        let named =
            opening.page.is_some() || opening.renderer == Renderer::Chrome || opening.conversation;
        if named {
            return opening;
        }
        Opening {
            conversation: true,
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

    /// An application at `/A.app`, as [`Bundle::around`] would find it.
    fn a_bundle() -> Bundle {
        Bundle {
            contents: PathBuf::from("/A.app/Contents"),
            resources: PathBuf::from("/A.app/Contents/Resources"),
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
            bundle.as_ref().map(Bundle::chromium),
            Some(
                programs
                    .with_file_name("Frameworks")
                    .join("alo Chromium.app/Contents/MacOS/alo Chromium")
            ),
            "its own Chromium, inside it, whether or not one is there"
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
    fn the_conversation_asked_for_is_opened_as_asked() {
        let bundle = a_bundle();
        let Some(asked) = opening(&["--conversation"]) else {
            panic!("an argument");
        };
        assert_eq!(bundle.opening(asked.clone()), asked);
    }

    #[test]
    fn nothing_named_opens_the_conversation_keeping_every_option_given() {
        // Queue item 993: what macOS gives an application opened from the
        // Finder — nothing — and every option the conversation takes beside
        // nothing named.
        let bundle = a_bundle();
        for words in [
            &[][..],
            &["--agent-site", "https://example.com"][..],
            &["--scale", "2"][..],
            &["--frozen-fonts"][..],
            &["--data", "/tmp/alo-profile"][..],
        ] {
            let Some(asked) = opening(words) else {
                panic!("these are arguments");
            };
            let opened = bundle.opening(asked.clone());
            assert!(opened.conversation, "{words:?}");
            assert_eq!(opened.renderer, Renderer::ThisEngine, "{words:?}");
            assert_eq!(opened.page, None, "{words:?}: no page of its own");
            assert_eq!(
                Opening {
                    conversation: false,
                    ..opened
                },
                asked,
                "{words:?}: everything else as given"
            );
        }
        let Some(asked) = opening(&["--scale", "1.5"]) else {
            panic!("an argument");
        };
        assert_eq!(asked.scale, Some(Scale::ONE_AND_A_HALF));
        assert_eq!(bundle.opening(asked).scale, Some(Scale::ONE_AND_A_HALF));
    }

    #[test]
    fn a_page_named_is_kept_and_chrome_still_shows_the_applications_page() {
        let bundle = a_bundle();
        for words in [
            &["page.html"][..],
            &["--chrome", "https://example.com/"][..],
            &["--chrome", "/A.app/Contents/Resources/page.html"][..],
        ] {
            let Some(asked) = opening(words) else {
                panic!("these are arguments");
            };
            assert_eq!(bundle.opening(asked.clone()), asked, "{words:?}");
            assert!(!asked.conversation, "{words:?}");
        }
    }
}
