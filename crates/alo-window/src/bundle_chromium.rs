/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which Chromium `alo` starts, inside the macOS application and outside it,
//! and what the application says first when its own is missing (ADR 0077,
//! queue items 719 and 993).
//!
//! Inside `alo browser.app` the only Chromium ever started is the one the
//! application carries ([`Bundle::chromium`]) — for an address chosen in
//! the conversation, which is what the application opens (queue item 993),
//! and for a page named with `--chrome` — **never the machine's Chrome,
//! even when one is installed**: the owner's words, *no fall back, as if
//! the user only has alo-browser on his PC*. Outside it, `alo --chrome` run
//! from a terminal is a developer's tool and drives the machine's Chrome as
//! it always has.
//!
//! An application whose own Chromium is not there still opens the
//! conversation, which this engine draws: [`decide`] says on its terminal
//! that no site chosen in it can open, and each site chosen is answered in
//! the conversation itself with why — *… could not be opened: alo's own
//! Chromium is not in the application: it was looked for at …*
//! ([`crate::conversation_chrome::Went::Unopened`]). One that is there but
//! cannot be started or does not answer — damaged — is the same, said as a
//! site chosen fails to open. A page a person named with `--chrome` is
//! never quietly drawn by another renderer: its failure is said.
//!
//! Until queue item 993 an application opened with nothing named showed
//! its own page in that Chromium with our frame above it, and with none
//! there drew a second page with this engine saying so; neither is what a
//! Mac application opens, and both are gone.

use crate::beside::{Chromium, NotCarried, is_program};
use crate::bundle::Bundle;
use crate::opening::Opening;

/// What `alo` opens, which Chromium it would start, and what it says first.
#[derive(Debug, Clone, PartialEq)]
pub struct Decided {
    /// What it opens.
    pub opening: Opening,
    /// Which Chromium it starts, for a page drawn by one or an address
    /// chosen in the conversation.
    pub chromium: Chromium,
    /// What it says on its terminal before anything opens, if anything:
    /// that the conversation's sites cannot open, its Chromium missing.
    pub said: Option<String>,
}

/// What `alo` asked for `asked` opens, inside `bundle` or outside any.
///
/// Outside an application: what was asked, with the machine's Chrome for
/// `--chrome` — today's behaviour, unchanged. Inside one: what
/// [`Bundle::opening`] opens, with its own Chromium and no other; and when
/// that is the conversation and the Chromium is not a program at its place,
/// the sentence saying no site chosen in it can open.
pub fn decide(bundle: Option<&Bundle>, asked: Opening) -> Decided {
    let Some(bundle) = bundle else {
        return Decided {
            opening: asked,
            chromium: Chromium::Machines,
            said: None,
        };
    };
    let program = bundle.chromium();
    let opening = bundle.opening(asked);
    let said = (opening.conversation && !is_program(&program)).then(|| {
        format!(
            "{}, so no site chosen in the conversation can open, and the conversation says so",
            NotCarried {
                looked: program.clone()
            }
        )
    });
    Decided {
        opening,
        chromium: Chromium::Carried(program),
        said,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{CHROMIUM, PAGE};
    use crate::opening::Renderer;
    use std::path::{Path, PathBuf};

    /// An application of this test's own, carrying its page and — when
    /// `chromium` — a program at its Chromium's place: its `alo`.
    fn shaped(name: &str, chromium: bool) -> Option<PathBuf> {
        let root =
            std::env::temp_dir().join(format!("alo-bundle-chromium-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let contents = root.join("alo browser.app").join("Contents");
        std::fs::create_dir_all(contents.join("MacOS")).ok()?;
        std::fs::create_dir_all(contents.join("Resources")).ok()?;
        std::fs::write(contents.join("Resources").join(PAGE), "<p>here").ok()?;
        if chromium {
            let program = contents.join(CHROMIUM);
            std::fs::create_dir_all(program.parent()?).ok()?;
            std::fs::write(&program, "#!/bin/sh\n").ok()?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).ok()?;
            }
        }
        Some(contents.join("MacOS").join("alo"))
    }

    /// Remove what [`shaped`] made around `alo`.
    fn unshape(alo: &Path) {
        if let Some(root) = alo.ancestors().nth(4) {
            let _ = std::fs::remove_dir_all(root);
        }
    }

    fn opening(words: &[&str]) -> Option<Opening> {
        Opening::from_arguments(words.iter().map(|word| (*word).to_owned())).ok()
    }

    #[test]
    fn in_an_application_carrying_a_chromium_only_that_one_is_started() {
        let Some(alo) = shaped("carrying", true) else {
            panic!("no directory");
        };
        let bundle = Bundle::around(&alo);
        let contents = alo.parent().and_then(Path::parent).map(Path::to_path_buf);
        let Some(contents) = contents else {
            panic!("no contents");
        };
        let carried = contents.join("Frameworks/alo Chromium.app/Contents/MacOS/alo Chromium");
        for words in [
            &[][..],
            &["--chrome", "https://example.com/"][..],
            &["--conversation"][..],
        ] {
            let Some(asked) = opening(words) else {
                panic!("these are arguments");
            };
            let decided = decide(bundle.as_ref(), asked);
            assert_eq!(
                decided.chromium,
                Chromium::Carried(carried.clone()),
                "{words:?}: its own, never the machine's"
            );
            assert_eq!(decided.said, None, "{words:?}");
        }
        // Queue item 993: opened from the Finder, with nothing named, it is
        // the conversation window, never the application's page.
        let Some(nothing) = opening(&[]) else {
            panic!("no arguments");
        };
        let decided = decide(bundle.as_ref(), nothing);
        assert!(decided.opening.conversation);
        assert_eq!(decided.opening.renderer, Renderer::ThisEngine);
        assert_eq!(decided.opening.page, None);
        unshape(&alo);
    }

    #[test]
    fn in_an_application_carrying_none_the_conversation_opens_and_says_why_no_site_can() {
        let Some(alo) = shaped("bare", false) else {
            panic!("no directory");
        };
        let bundle = Bundle::around(&alo);
        let Some(contents) = alo.parent().and_then(Path::parent) else {
            panic!("no contents");
        };
        let carried = contents.join(CHROMIUM);
        let said = format!(
            "alo's own Chromium is not in the application: it was looked for at {}, so no site \
             chosen in the conversation can open, and the conversation says so",
            carried.display()
        );
        for words in [&[][..], &["--conversation"][..], &["--scale", "2"][..]] {
            let Some(asked) = opening(words) else {
                panic!("these are arguments");
            };
            let decided = decide(bundle.as_ref(), asked);
            assert!(decided.opening.conversation, "{words:?}");
            assert_eq!(decided.opening.page, None, "{words:?}");
            assert_eq!(decided.chromium, Chromium::Carried(carried.clone()));
            assert_eq!(decided.said.as_deref(), Some(said.as_str()), "{words:?}");
        }
        // A page named with --chrome is refused by the carried Chromium's
        // absence when it is started, never handed to the machine's Chrome.
        let Some(named) = opening(&["--chrome", "https://example.com/"]) else {
            panic!("these are arguments");
        };
        let decided = decide(bundle.as_ref(), named.clone());
        assert_eq!(decided.opening, named);
        assert_eq!(decided.chromium, Chromium::Carried(carried));
        assert_eq!(decided.said, None);
        unshape(&alo);
    }

    #[test]
    fn outside_an_application_nothing_changes() {
        for words in [
            &[][..],
            &["--chrome", "https://example.com/"][..],
            &["--conversation"][..],
            &["page.html"][..],
        ] {
            let Some(asked) = opening(words) else {
                panic!("these are arguments");
            };
            assert_eq!(
                decide(None, asked.clone()),
                Decided {
                    opening: asked.clone(),
                    chromium: Chromium::Machines,
                    said: None,
                },
                "{words:?}"
            );
        }
    }
}
