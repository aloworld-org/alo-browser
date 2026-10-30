/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which Chromium `alo` starts, inside the macOS application and outside it,
//! and what the application opens and says when its own is missing or
//! damaged (ADR 0077, queue item 719).
//!
//! Inside `alo browser.app` the only Chromium ever started is the one the
//! application carries ([`Bundle::chromium`]) — for its own page, for a
//! page named with `--chrome`, and for an address chosen in the
//! conversation — **never the machine's Chrome, even when one is
//! installed**: the owner's words, *no fall back, as if the user only has
//! alo-browser on his PC*. Outside it, `alo --chrome` run from a terminal is
//! a developer's tool and drives the machine's Chrome as it always has.
//!
//! An application whose own Chromium is not there draws its page with this
//! engine instead, on the page that says so, and says why on its terminal
//! ([`decide`]) — ADR 0068 § 3's way, with the reason ADR 0077 gives it. One
//! that is there but cannot be started or does not answer — damaged — is
//! the same, decided once starting it has failed ([`undrawn`]). A page a
//! person named is never quietly drawn by another renderer: that failure is
//! said, as it is outside the application.

use crate::beside::{Chromium, NotCarried, is_program};
use crate::bundle::Bundle;
use crate::opening::Opening;
use core::fmt;

/// What `alo` opens, which Chromium it would start, and what it says first.
#[derive(Debug, Clone, PartialEq)]
pub struct Decided {
    /// What it opens.
    pub opening: Opening,
    /// Which Chromium it starts, for a page drawn by one or an address
    /// chosen in the conversation.
    pub chromium: Chromium,
    /// What it says on its terminal before anything opens, if anything:
    /// why this engine draws the application's page.
    pub said: Option<String>,
}

/// What `alo` asked for `asked` opens, inside `bundle` or outside any.
///
/// Outside an application: what was asked, with the machine's Chrome for
/// `--chrome` — today's behaviour, unchanged. Inside one: its own Chromium
/// and no other, and [`Bundle::opening`] told whether that Chromium is a
/// program at its place; when it is not and that changed what opens, the
/// sentence saying so.
pub fn decide(bundle: Option<&Bundle>, asked: Opening) -> Decided {
    let Some(bundle) = bundle else {
        return Decided {
            opening: asked,
            chromium: Chromium::Machines,
            said: None,
        };
    };
    let program = bundle.chromium();
    let carried = is_program(&program);
    let opening = bundle.opening(asked.clone(), carried);
    let said = (!carried && opening != asked).then(|| {
        format!(
            "{}, so this browser's own engine draws the application's page",
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

/// What the application opens instead once its own Chromium, asked to draw
/// what `asked` opened, could not — it would not start, or did not answer,
/// for the reason `why` — and the sentence it says: this engine on the
/// page that says so, when the application's page was what it was drawing.
///
/// [`None`] outside an application, and for a page a person named, whose
/// failure is said as it is.
pub fn undrawn(
    bundle: Option<&Bundle>,
    asked: &Opening,
    why: &dyn fmt::Display,
) -> Option<(Opening, String)> {
    let bundle = bundle?;
    let instead = bundle.opening(asked.clone(), false);
    (instead != *asked).then(|| {
        let said = format!(
            "alo's own Chromium at {} could not draw the application's page — {why} — \
             so this browser's own engine draws it",
            bundle.chromium().display()
        );
        (instead, said)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{CHROMIUM, PAGE, WITHOUT_CHROME};
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
        let carried = contents.join("Frameworks/Chromium.app/Contents/MacOS/Chromium");
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
        let Some(nothing) = opening(&[]) else {
            panic!("no arguments");
        };
        let decided = decide(bundle.as_ref(), nothing);
        assert_eq!(decided.opening.renderer, Renderer::Chrome);
        assert_eq!(
            decided.opening.page,
            Some(contents.join("Resources").join(PAGE))
        );
        unshape(&alo);
    }

    #[test]
    fn in_an_application_carrying_none_this_engine_draws_the_page_that_says_so() {
        let Some(alo) = shaped("bare", false) else {
            panic!("no directory");
        };
        let bundle = Bundle::around(&alo);
        let Some(contents) = alo.parent().and_then(Path::parent) else {
            panic!("no contents");
        };
        let carried = contents.join(CHROMIUM);
        let Some(nothing) = opening(&[]) else {
            panic!("no arguments");
        };
        let decided = decide(bundle.as_ref(), nothing);
        assert_eq!(decided.opening.renderer, Renderer::ThisEngine);
        assert_eq!(
            decided.opening.page,
            Some(contents.join("Resources").join(WITHOUT_CHROME))
        );
        assert_eq!(decided.chromium, Chromium::Carried(carried.clone()));
        assert_eq!(
            decided.said,
            Some(format!(
                "alo's own Chromium is not in the application: it was looked for at {}, \
                 so this browser's own engine draws the application's page",
                carried.display()
            ))
        );
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
            assert_eq!(undrawn(None, &asked, &"it broke"), None, "{words:?}");
        }
    }

    #[test]
    fn a_damaged_chromium_has_this_engine_draw_the_applications_page_saying_why() {
        let Some(alo) = shaped("damaged", true) else {
            panic!("no directory");
        };
        let bundle = Bundle::around(&alo);
        let Some(contents) = alo.parent().and_then(Path::parent) else {
            panic!("no contents");
        };
        let Some(nothing) = opening(&[]) else {
            panic!("no arguments");
        };
        let decided = decide(bundle.as_ref(), nothing.clone());
        let Some((instead, said)) = undrawn(bundle.as_ref(), &nothing, &"Chrome has gone") else {
            panic!("the application's page is drawn instead");
        };
        assert_eq!(instead.renderer, Renderer::ThisEngine);
        assert_eq!(
            instead.page,
            Some(contents.join("Resources").join(WITHOUT_CHROME))
        );
        assert_eq!(
            said,
            format!(
                "alo's own Chromium at {} could not draw the application's page — Chrome has \
                 gone — so this browser's own engine draws it",
                contents.join(CHROMIUM).display()
            )
        );
        // What was decided before it failed is not what is drawn after.
        assert_ne!(decided.opening, instead);
        // A page a person named fails as named.
        let Some(named) = opening(&["--chrome", "https://example.com/"]) else {
            panic!("these are arguments");
        };
        assert_eq!(undrawn(bundle.as_ref(), &named, &"Chrome has gone"), None);
        unshape(&alo);
    }
}
