/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's mark and a word, beside the colour that says *alo is acting*
//! (queue item 974, ADR 0112 *Answered* 1, ADR 0074 § 2 as amended).
//!
//! Deep teal means alo is acting on every alo product, and it is never
//! drawn alone: wherever the interface shows the agent at work — a chat it
//! works in, on the panel and on the rail, and the working line over a
//! card's page — it shows **alo's mark** and **a word** with the colour.
//! The mark is the owner's icon in its small drawing,
//! `docs/brand/alo-browser-icon-small.svg`, as it is (every picture under 64
//! device pixels is that drawing, ADR 0067 § 6): compiled in, handed to the
//! drawing as a picture beside the document, and drawn by every `<img>`
//! naming [`SRC`] — so a panel with two chats at work draws it twice, which
//! a drawing moved out of a `<template>` could not.
//!
//! The mark is decoration, `aria-hidden` with no text of its own: the word
//! beside it is what a person and an agent read.

use crate::resource::{Resource, SVG_TYPE};

/// The name an `<img>` gives alo's mark: no page can name it, since only
/// the interface's documents are handed it.
pub const SRC: &str = "alo:mark";

/// The word a chat the agent works in says, on the panel and on the rail.
pub const WORKING: &str = "Working";

/// The owner's icon in its small drawing, as it is.
const SMALL: &[u8] = include_bytes!("../../../docs/brand/alo-browser-icon-small.svg");

/// alo's mark, as the drawing is handed it beside an interface document.
pub fn picture() -> Resource {
    Resource::new(SRC, Some(SVG_TYPE), SMALL.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mark_is_the_owners_small_drawing_as_svg() {
        let mark = picture();
        assert!(mark.is_svg());
        assert_eq!(mark.src, SRC);
        let text = std::str::from_utf8(SMALL).unwrap_or_default();
        assert!(text.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(text.contains("id=\"teal-small\""), "the small drawing");
    }
}
