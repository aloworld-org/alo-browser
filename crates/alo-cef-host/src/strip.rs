/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The strip an expanded card shows a link's address in (queue item 811,
//! `docs/design/shell.md`, approved by the owner 2026-11-06): where it
//! stands and what it says, in numbers, so a test holds it to them.
//!
//! A card's browser is made with no status bubble, for its life (queue
//! item 808): Chromium's own bubble was a window beside a parked card,
//! seen at a display's edge. So once the card is expanded — the page in
//! its own window, seen — the address of the link under the pointer, which
//! Chromium tells the host as its status (`OnStatusMessage`), is shown by
//! the host (`strip_mac.rs` draws it): a strip at the page's bottom left
//! corner, on the paper
//! colour, in the quiet ink, 12 px, [`HEIGHT`] tall, [`PADDING`] either
//! side of the words, and at most a third of the window wide, as Chrome's
//! own bubble is at most a third — so an address longer than that is cut,
//! and never reaches across the page. Nothing is said while the pointer is
//! over no link, and the strip is not drawn. Unseen, a card shows no strip:
//! the parked window is transparent and its strip would not be.

/// How tall the strip is, in the window's points.
pub const HEIGHT: i32 = 24;

/// The room either side of its words, in points.
pub const PADDING: i32 = 8;

/// The strip's background: the interface's paper, `#f4f1ec`.
pub const PAPER: u32 = 0xF4_F1EC;

/// Its words' colour: the interface's quiet ink, `#5f5a52`.
pub const QUIET: u32 = 0x5F_5A52;

/// Its words' face: the interface's own where the system has it, and the
/// system's own face otherwise — the host carries no font of alo's, which
/// are handed to alo's own renderers alone (ADR 0084).
pub const FACE_NAME: &str = "Instrument Sans";

/// Its words' size, in points.
pub const SIZE: f64 = 12.0;

/// A rectangle in the window's points, from its content's top left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    /// From the left edge.
    pub x: i32,
    /// From the top edge.
    pub y: i32,
    /// Across.
    pub width: i32,
    /// Down.
    pub height: i32,
}

/// What the strip says for Chromium's status `status`: the address, or
/// nothing — the strip hidden — when the pointer is over no link.
pub fn says(status: Option<&str>) -> Option<&str> {
    status.map(str::trim).filter(|said| !said.is_empty())
}

/// Where the strip stands in a window whose content is `window` points
/// (across, down), its words `words` points wide as CEF lays them out: at
/// the bottom left corner, its words and padding wide, at most a third of
/// the window — never less than nothing.
pub fn placed(window: (i32, i32), words: i32) -> Placed {
    let wide = words
        .saturating_add(PADDING.saturating_mul(2))
        .min(window.0 / 3)
        .max(0);
    Placed {
        x: 0,
        y: window.1.saturating_sub(HEIGHT).max(0),
        width: wide,
        height: HEIGHT.min(window.1.max(0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strip_stands_at_the_bottom_left_its_words_wide_at_most_a_third() {
        // An expanded card over the design's first screen, 1200 × 820.
        assert_eq!(
            placed((1200, 820), 180),
            Placed {
                x: 0,
                y: 796,
                width: 196,
                height: 24
            }
        );
        // A long address is cut at a third of the window.
        assert_eq!(placed((1200, 820), 900).width, 400);
        assert_eq!(placed((878, 552), 900).width, 292);
        // A window shorter than the strip holds what it can.
        assert_eq!(
            placed((300, 10), 20),
            Placed {
                x: 0,
                y: 0,
                width: 36,
                height: 10
            }
        );
    }

    #[test]
    fn it_says_a_links_address_and_nothing_over_no_link() {
        assert_eq!(
            says(Some("https://example.com/a")),
            Some("https://example.com/a")
        );
        assert_eq!(says(Some("")), None);
        assert_eq!(says(Some("  ")), None);
        assert_eq!(says(None), None);
    }
}
