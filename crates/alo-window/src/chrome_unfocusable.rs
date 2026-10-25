/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A control that stopped a form Chrome would not send, and that Chrome
//! could not give the keyboard to, read from what Chrome logs (queue item
//! 646, ADR 0051's amendment of 25 October 2026).
//!
//! A press Chrome refuses gives the first failing control it can focus the
//! keyboard and a bubble ([`crate::chrome_bubble`]). A failing control it
//! cannot focus — `hidden`, `display: none`, inside an `inert` part of the
//! page — gets neither: measured against Chrome 155.0.8059.86, headless,
//! Chrome instead **logs it**, as `Log.entryAdded` with source `rendering`
//! and level `error`, in the words
//! `An invalid form control with name='a' is not focusable.`, and with the
//! control itself as the entry's one argument, a value of `subtype`
//! `node`. It does so for each such control in the form, in the order of
//! the document, whether or not a bubble is shown for another; the name is
//! the control's `name` attribute, empty when it has none, and these words
//! are not translated.
//!
//! Those words are not the control's message: Chrome says nothing of which
//! constraint failed, nor what a person would read, since a person reads
//! nothing. They are what Chrome says about the control, and so what the
//! answer carries.
//!
//! # What it does not say
//!
//! **A control whose `invalid` the page cancels** is neither logged nor
//! given a bubble, and the press is told of exactly as one whose `submit`
//! the page cancels: nothing in Chrome's tree, its log or its events tells
//! the two apart. That press answers `Activated` — the page took it, as
//! ADR 0051 § 6 says of a cancelled `submit`.

use crate::chrome_protocol::Logged;

/// The words Chrome's entry begins with, before the control's name.
const BEFORE_THE_NAME: &str = "An invalid form control with name='";

/// The words Chrome's entry ends with, after the control's name.
const AFTER_THE_NAME: &str = "' is not focusable.";

/// A control that stopped a form, which Chrome could not give the
/// keyboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unfocusable {
    /// The control, by the `objectId` Chrome handed it over as.
    pub object: String,
    /// What Chrome said about it, word for word.
    pub words: String,
}

/// The control `logged` says stopped a form and could not be given the
/// keyboard — or [`None`] when `logged` is something else Chrome logged.
pub fn control(logged: Logged) -> Option<Unfocusable> {
    let named = logged
        .text
        .strip_prefix(BEFORE_THE_NAME)
        .is_some_and(|rest| rest.ends_with(AFTER_THE_NAME));
    if logged.source != "rendering" || logged.level != "error" || !named {
        return None;
    }
    let [object] = <[String; 1]>::try_from(logged.nodes).ok()?;
    Some(Unfocusable {
        object,
        words: logged.text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logged(source: &str, level: &str, text: &str, nodes: &[&str]) -> Logged {
        Logged {
            source: source.to_owned(),
            level: level.to_owned(),
            text: text.to_owned(),
            nodes: nodes.iter().map(|node| (*node).to_owned()).collect(),
        }
    }

    const HIDDEN: &str = "An invalid form control with name='a' is not focusable.";

    #[test]
    fn a_control_chrome_could_not_focus_is_the_node_it_logged() {
        assert_eq!(
            control(logged("rendering", "error", HIDDEN, &["7.2.1"])),
            Some(Unfocusable {
                object: "7.2.1".to_owned(),
                words: HIDDEN.to_owned(),
            })
        );
        // A control with no name, and one whose name says the words again.
        for text in [
            "An invalid form control with name='' is not focusable.",
            "An invalid form control with name='' is not focusable.' is not focusable.",
        ] {
            assert_eq!(
                control(logged("rendering", "error", text, &["7.2.1"])).map(|one| one.words),
                Some(text.to_owned()),
                "{text}"
            );
        }
    }

    #[test]
    fn anything_else_chrome_logged_names_no_control() {
        for other in [
            logged("network", "error", HIDDEN, &["7.2.1"]),
            logged("rendering", "warning", HIDDEN, &["7.2.1"]),
            logged("rendering", "error", HIDDEN, &[]),
            logged("rendering", "error", HIDDEN, &["7.2.1", "7.2.2"]),
            logged(
                "rendering",
                "error",
                "An invalid form control with name='a' is focusable.",
                &["7.2.1"],
            ),
            logged(
                "rendering",
                "error",
                "A form control with name='a' is not focusable.",
                &["7.2.1"],
            ),
        ] {
            assert_eq!(control(other.clone()), None, "{other:?}");
        }
    }
}
