/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A site's mark, built into the first screen's document beside its name
//! (queue item 747, ADR 0088): the site's own icon where one came and reads
//! as a picture, and its letter on one of the design's four tints where
//! none did — on All and Sites, on Videos and on News.
//!
//! # The icon
//!
//! The browser process fetched it from the provider's image proxy alone and
//! bounded it ([`crate::results::LARGEST_MARK`]); its bytes are a
//! stranger's, and are read only here, in the confined renderer. The mark
//! is an `<img>` in the letter's rounded tile, whose `src` is a name of
//! ours, [`src`] — the turn's place,
//! what the row is and its place — never the icon's address, and the bytes
//! are handed to the drawing as that picture ([`pictures`]), decoded by the
//! picture readers every `<img>` goes through.
//!
//! **An icon that does not read is the letter.** Before an `<img>` is made
//! its bytes are read ([`reads`]): bytes that do not decode, or decode to a
//! picture larger than [`LARGEST_MARK_SIDE`] either way, leave the site its
//! letter — never an empty square where a mark should be.
//!
//! # What an agent reads
//!
//! Nothing: a mark is decoration, hidden from the tree, as the letter is.
//! What the site is called is its name's to say.

use crate::resource::Resource;
use crate::start::Start;
use crate::start_document::{Unbuilt, append, attribute, element};
use crate::turn::Turn;
use alo_dom::{Document, NodeId};

/// The widest or tallest an icon may decode to and be drawn as a mark:
/// Brave's are 32 square, and a mark is drawn 18.
pub const LARGEST_MARK_SIDE: u32 = 256;

/// What a mark stands beside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Beside {
    /// A site found, on All or Sites.
    Site,
    /// A video, on Videos.
    Video,
    /// A news story, on News.
    Story,
}

impl Beside {
    /// Its word in a mark's name.
    fn word(self) -> &'static str {
        match self {
            Beside::Site => "site",
            Beside::Video => "video",
            Beside::Story => "story",
        }
    }
}

/// The name the mark beside the `place`th row of `beside` in the `turn`th
/// turn is drawn by.
pub fn src(turn: usize, beside: Beside, place: usize) -> String {
    format!("alo:mark-{turn}-{}-{place}", beside.word())
}

/// Whether `bytes` read as a picture this engine draws as a mark: one that
/// decodes, at most [`LARGEST_MARK_SIDE`] either way.
pub fn reads(bytes: &[u8]) -> bool {
    alo_paint::picture::read(bytes).is_ok_and(|picture| {
        picture.width() <= LARGEST_MARK_SIDE && picture.height() <= LARGEST_MARK_SIDE
    })
}

/// Every mark `start`'s turns draw, by the name each `<img>` gives it: what
/// the drawing is handed beside the document. Only the marks that read
/// ([`reads`]), as only those are drawn as pictures.
pub fn pictures(start: &Start) -> Vec<Resource> {
    let mut found = Vec::new();
    for (turn, each) in start.turns.iter().enumerate() {
        let Turn::Searched(results) = each else {
            continue;
        };
        let marks = [
            (
                Beside::Site,
                results
                    .drawn_sites()
                    .iter()
                    .map(|site| site.mark.as_ref())
                    .collect::<Vec<_>>(),
            ),
            (
                Beside::Video,
                results
                    .drawn_videos()
                    .iter()
                    .map(|video| video.mark.as_ref())
                    .collect(),
            ),
            (
                Beside::Story,
                results
                    .drawn_stories()
                    .iter()
                    .map(|story| story.mark.as_ref())
                    .collect(),
            ),
        ];
        for (beside, held) in marks {
            for (place, bytes) in held.into_iter().enumerate() {
                if let Some(bytes) = bytes.filter(|bytes| reads(bytes)) {
                    found.push(Resource::new(src(turn, beside, place), None, bytes.clone()));
                }
            }
        }
    }
    found
}

/// A row's mark, put in `into`: the site's icon, named `named`, when its
/// `icon` reads; else its `letter` on tint `tint`; else nothing.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no mark can cause.
pub fn mark(
    document: &mut Document,
    into: NodeId,
    icon: Option<(&[u8], String)>,
    letter: Option<String>,
    tint: usize,
) -> Result<(), Unbuilt> {
    if let Some((_, named)) = icon.filter(|(bytes, _)| reads(bytes)) {
        // The tile clips the picture to its rounded square: this engine
        // does not round a picture by its own `border-radius`.
        let tile = element(document, "span")?;
        attribute(document, tile, "class", "fav icon")?;
        attribute(document, tile, "aria-hidden", "true")?;
        let image = element(document, "img")?;
        attribute(document, image, "src", &named)?;
        attribute(document, image, "alt", "")?;
        append(document, tile, image)?;
        append(document, into, tile)?;
        return Ok(());
    }
    let Some(letter) = letter else {
        return Ok(());
    };
    let made = element(document, "span")?;
    attribute(document, made, "class", &format!("fav tint{tint}"))?;
    attribute(document, made, "aria-hidden", "true")?;
    let text = document.create_text_node(&letter);
    append(document, made, text)?;
    append(document, into, made)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_paint::canvas::Canvas;
    use alo_value::Rgba;

    fn icon(side: u32) -> Vec<u8> {
        alo_paint::encode::to_png(&Canvas::new(side, side, Rgba::from_rgba8(200, 30, 30, 255)))
            .unwrap_or_default()
    }

    #[test]
    fn an_icon_reads_when_it_decodes_and_is_no_larger_than_a_mark_may_be() {
        assert!(reads(&icon(32)));
        assert!(reads(&icon(LARGEST_MARK_SIDE)));
        assert!(!reads(&icon(LARGEST_MARK_SIDE + 1)));
        let mut cut = icon(32);
        cut.truncate(40);
        assert!(!reads(&cut), "a PNG cut short does not decode");
        assert!(!reads(b"<svg></svg>"));
    }

    #[test]
    fn each_mark_is_named_by_its_turn_its_row_and_its_place() {
        assert_eq!(src(2, Beside::Site, 0), "alo:mark-2-site-0");
        assert_eq!(src(0, Beside::Video, 3), "alo:mark-0-video-3");
        assert_eq!(src(1, Beside::Story, 1), "alo:mark-1-story-1");
    }
}
