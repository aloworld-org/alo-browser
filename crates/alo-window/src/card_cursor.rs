/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The pointer's shape over a card (queue item 799, `docs/design/shell.md`,
//! approved by the owner 2026-11-06: *the pointer follows the page — hand
//! over links, text cursor over text*).
//!
//! Over a card's page the shape is the page's to say, as in Chrome's own
//! window: Chromium decides it from the page — the hand over a link, the
//! I-beam over text a person can select or a field, whatever the page's
//! `cursor` says — and tells its embedder, never the DevTools pipe. The
//! Chromium alo carries tells alo's window ([`crate::chrome_told`]); the
//! window keeps the newest shape the page asked for and shows it:
//!
//! - **over the card's page box**, where the page is drawn and seen;
//! - **wherever the pointer is while a press on the page is held**, as a
//!   window keeps a drag's shape — the I-beam of a selection dragged past
//!   the page's edge;
//! - **the arrow everywhere else** — the card's bar, what the person said,
//!   the box, the panel, the sheet over the conversation — as every part
//!   of alo's own is drawn with the arrow today.
//!
//! A new page in the card begins at the arrow until it asks for another.
//! Shapes are named as CSS names them and set by the event loop
//! (`crate::window`); `none` hides the pointer over the page. A page's own
//! picture for a pointer (`cursor: url(…)`) is told as the arrow: cut to
//! queue item 846. Through the machine's Chrome nothing is told, and the
//! pointer stays the arrow.

/// The arrow, CSS's `default`.
pub const ARROW: &str = "default";

/// The shape to show: `page`'s, the newest the card's page asked for, when
/// the pointer is `over` its page box or a press on it is `held`; the
/// arrow otherwise.
pub fn shown(page: &'static str, over: bool, held: bool) -> &'static str {
    if over || held { page } else { ARROW }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pages_shape_over_its_box_or_while_pressed_and_the_arrow_elsewhere() {
        assert_eq!(shown("pointer", true, false), "pointer");
        assert_eq!(
            shown("text", false, true),
            "text",
            "a selection dragged off"
        );
        assert_eq!(shown("pointer", false, false), ARROW);
        assert_eq!(shown(ARROW, true, false), ARROW);
    }
}
