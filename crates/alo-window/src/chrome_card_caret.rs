/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a person is typing on a card's page, for an input method's
//! candidate window (queue item 802, `docs/design/shell.md`, approved by the
//! owner 2026-11-06: *macOS input methods work inside cards, with the
//! candidate window shown where you are typing*).
//!
//! macOS shows an input method's candidates under the rectangle the window
//! says the caret is in (winit's `set_ime_cursor_area`). The caret is
//! Chromium's, in the card's page, and nothing on the DevTools pipe says
//! where it is. So as a composition begins the thread holding the card asks
//! Chromium, through its DOM domain and no script, where the field that has
//! the keyboard ends its words:
//!
//! 1. the element with the keyboard — `DOM.querySelector(":focus")` on the
//!    document;
//! 2. everything under it, its shadow tree too — a field's editing box is
//!    in one (`DOM.describeNode`, depth −1, pierced) — and in it the last
//!    words, a field's placeholder never counted ([`chrome_protocol::last_inside`]);
//! 3. where those words are drawn (`DOM.getContentQuads`): the caret is at
//!    the right edge of their last line, as tall as it ([`caret_of`]); with
//!    no words, at the left edge of the innermost last element — an empty
//!    field's editing box.
//!
//! **Where you are typing is taken to be the end of the field's words** —
//! where a person typing into a search box is. A caret moved back into the
//! words (an arrow, a press in them) is not read: the candidates then stand
//! at the words' end, not at the caret. Cut to queue item 847; the exact
//! caret is Chromium's text-input client's to say in the Chromium alo
//! carries (`firstRectForCharacterRange`), which is `unsafe` to ask in Rust
//! and would need its own ADR.

use crate::chrome_protocol::{self, Command};
use crate::chrome_tab::Tab;

/// Where the caret is, in the page's whole CSS pixels, rounded: a
/// rectangle one pixel wide, as tall as the line it is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caret {
    /// From the page's left edge.
    pub x: i32,
    /// From the page's top edge.
    pub y: i32,
    /// Down.
    pub height: i32,
}

/// The caret for the last line of a quad Chrome drew `quad` as: at its
/// right edge after `words`, at its left edge before an empty box — as tall
/// as the line, its corners in Chrome's order (top left, top right, bottom
/// right, bottom left).
pub fn caret_of(quad: &[f64; 8], words: bool) -> Caret {
    let [left, top, right, _, _, bottom, _, _] = *quad;
    // Whole CSS pixels, far inside an i32 on any page a person reads; a
    // page laid out past them is held at the edge.
    let whole = |value: f64| {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "rounded and clamped to an i32's range first"
        )]
        let whole = value
            .round()
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32;
        whole
    };
    Caret {
        x: whole(if words { right } else { left }),
        y: whole(top),
        height: whole((bottom - top).max(0.0)),
    }
}

/// Where the caret is in `tab`'s page, as the module says — or [`None`]
/// when no element has the keyboard, or Chrome could not say.
pub fn caret(tab: &mut Tab<'_>) -> Option<Caret> {
    let document = chrome_protocol::document_root(&tab.ask(&Command::ReadDocument).ok()?).ok()?;
    let focused =
        chrome_protocol::selected(&tab.ask(&Command::Focused { document }).ok()?).ok()??;
    let inside =
        chrome_protocol::last_inside(&tab.ask(&Command::DescribeAll { node: focused }).ok()?)
            .ok()??;
    let quads =
        chrome_protocol::quads(&tab.ask(&Command::WhereDrawn { node: inside.node }).ok()?).ok()?;
    quads.last().map(|quad| caret_of(quad, inside.words))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_caret_stands_after_the_last_lines_words_or_at_an_empty_boxs_start() {
        let line = [10.0, 20.0, 74.5, 20.0, 74.5, 43.0, 10.0, 43.0];
        assert_eq!(
            caret_of(&line, true),
            Caret {
                x: 75,
                y: 20,
                height: 23
            }
        );
        assert_eq!(
            caret_of(&line, false),
            Caret {
                x: 10,
                y: 20,
                height: 23
            }
        );
    }
}
