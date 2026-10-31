/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The interface's own typefaces, carried by the application (queue item
//! 706, ADR 0084).
//!
//! The owner's design is set in two families, both under the SIL Open Font
//! License 1.1: **Instrument Sans** for everything a person reads in the
//! interface, and **Newsreader** for the mark *alo* and the greeting. Fonts
//! are rented physics (`CLAUDE.md`), so these are the files as Google Fonts
//! publishes them (`github.com/google/fonts`, `ofl/instrumentsans` and
//! `ofl/newsreader`), unmodified, each beside its own `OFL.txt` in
//! `interface/fonts/` — ADR 0084 records which commit and which hashes.
//!
//! # Variable, and set by weight alone
//!
//! Both are variable fonts, one file each for every weight the design uses
//! (Instrument Sans 400, 500 and 600; Newsreader 400 and 500). This engine
//! sets a variable font's `wght` axis to the weight asked for — measured,
//! shaped and outlined at that instance (`alo_text::Font::at_weight`) — and
//! leaves every other axis at the file's default: Instrument Sans' `wdth`
//! at 100, which is the design's, and Newsreader's `opsz` at 18, where a
//! browser sets it to the size of the text (`font-optical-sizing: auto`).
//! That difference is queue item 752's.
//!
//! # Compiled in, and handed over as every font is
//!
//! The bytes are part of the program, as the corpus's `DejaVu` faces are, so
//! nothing is read from a disk or a network to find them. A renderer is
//! confined and opens no file (ADR 0010); the browser process hands these to
//! the conversation's renderers as bytes on the pipe, as it hands every face
//! ([`crate::face`]). Only the interface's renderers are handed them — never
//! a page's — so no page is drawn in them, and none can tell from its
//! fonts that it is in this browser.
//!
//! # And a face for emoji
//!
//! Neither family has emoji, and a search's titles and descriptions are
//! full of them. So the interface carries a third face, **Noto Color
//! Emoji**, also under the OFL 1.1 — the file `googlefonts/noto-emoji`
//! publishes, unmodified — whose glyphs are colour pictures (`CBDT`), drawn
//! by `alo-paint` as pictures (ADR 0088). It is handed to the interface's
//! renderers with the other two, never to a page's, and the shaper looks in
//! it first for a character meant to be shown as an emoji
//! (`alo_text::emoji`).
//!
//! # The licence goes with them
//!
//! The OFL lets the fonts be bundled with software so long as each file's
//! copyright notice and the licence go with it. [`LICENCES`] are those two
//! files, as published, and `alo-bundle` puts them in the application's
//! `Contents/Resources/licences`.

use crate::face::Face;
use alo_text::Font;

/// The family the interface's text is set in.
pub const INSTRUMENT_SANS: &str = "Instrument Sans";

/// The family the mark and the greeting are set in.
pub const NEWSREADER: &str = "Newsreader";

/// `InstrumentSans[wdth,wght].ttf`, as published.
const INSTRUMENT_SANS_FILE: &[u8] = include_bytes!("../interface/fonts/InstrumentSans.ttf");

/// `Newsreader[opsz,wght].ttf`, as published.
const NEWSREADER_FILE: &[u8] = include_bytes!("../interface/fonts/Newsreader.ttf");

/// The family emoji are drawn in, in the interface (ADR 0088).
pub const NOTO_COLOR_EMOJI: &str = "Noto Color Emoji";

/// `fonts/NotoColorEmoji.ttf` of `googlefonts/noto-emoji` at `v2.051`, as
/// published.
const NOTO_COLOR_EMOJI_FILE: &[u8] = include_bytes!("../interface/fonts/NotoColorEmoji.ttf");

/// Each family's licence: its file name as carried, and its text — the
/// copyright notice and the SIL Open Font License 1.1, as published beside
/// the font.
pub const LICENCES: [(&str, &str); 3] = [
    (
        "OFL-InstrumentSans.txt",
        include_str!("../interface/fonts/OFL-InstrumentSans.txt"),
    ),
    (
        "OFL-Newsreader.txt",
        include_str!("../interface/fonts/OFL-Newsreader.txt"),
    ),
    (
        "OFL-NotoColorEmoji.txt",
        include_str!("../interface/fonts/OFL-NotoColorEmoji.txt"),
    ),
];

/// The interface's faces, as a renderer is handed them: Instrument Sans,
/// then Newsreader, then Noto Color Emoji, each filed under what the font
/// says it is.
///
/// A face the font would not describe is left out rather than filed under
/// a name written here, which is a broken build rather than anything a page
/// could cause, and which the tests below would say.
pub fn faces() -> Vec<Face> {
    [INSTRUMENT_SANS_FILE, NEWSREADER_FILE, NOTO_COLOR_EMOJI_FILE]
        .into_iter()
        .filter_map(|bytes| Face::described(bytes.to_vec()))
        .collect()
}

/// The same faces as fonts, for a renderer built in this process — a test's
/// — rather than one handed them over a pipe.
pub fn fonts() -> Vec<Font> {
    faces()
        .into_iter()
        .filter_map(|face| Font::load(&face.family, face.weight(), face.slant, face.bytes))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_text::{Slant, Weight};

    #[test]
    fn the_faces_are_filed_under_the_designs_two_families_and_the_emoji_face() {
        let named: Vec<(String, u16, Slant)> = faces()
            .into_iter()
            .map(|face| (face.family, face.weight, face.slant))
            .collect();
        assert_eq!(
            named,
            vec![
                (INSTRUMENT_SANS.to_owned(), 400, Slant::Normal),
                (NEWSREADER.to_owned(), 400, Slant::Normal),
                (NOTO_COLOR_EMOJI.to_owned(), 400, Slant::Normal),
            ]
        );
    }

    #[test]
    fn each_is_variable_over_every_weight_the_design_uses() {
        let fonts = fonts();
        let ranges: Vec<(String, Option<(u16, u16)>)> = fonts
            .iter()
            .map(|font| {
                (
                    font.family().to_owned(),
                    font.weight_axis()
                        .map(|axis| (axis.lightest().value(), axis.heaviest().value())),
                )
            })
            .collect();
        assert_eq!(
            ranges,
            vec![
                (INSTRUMENT_SANS.to_owned(), Some((400, 700))),
                (NEWSREADER.to_owned(), Some((200, 800))),
                (NOTO_COLOR_EMOJI.to_owned(), None),
            ]
        );
        // Set to 600, a font is drawn at 600, not at the 400 its `OS/2` names.
        let semibold = fonts.first().map(|font| font.at_weight(Weight::new(600)));
        assert_eq!(
            semibold
                .as_ref()
                .map(|font| (font.weight().value(), font.variable_weight().is_some())),
            Some((600, true))
        );
    }

    #[test]
    fn each_licence_is_the_ofl_with_its_familys_copyright() {
        let (designs, emoji) = LICENCES.split_at(2);
        for ((name, text), family) in designs.iter().zip([INSTRUMENT_SANS, NEWSREADER]) {
            assert!(
                text.starts_with("Copyright 20"),
                "{name} starts with its copyright notice"
            );
            assert!(
                text.contains(&format!("The {family} Project Authors")),
                "{name}"
            );
            assert!(
                text.contains("SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007"),
                "{name} is the OFL 1.1"
            );
        }
        // Noto's is the licence alone, as published beside the font: its
        // copyright notice is in the font's own `name` table.
        for (name, text) in emoji {
            assert!(
                text.starts_with("This Font Software is licensed under the SIL Open Font License"),
                "{name}"
            );
            assert!(
                text.contains("SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007"),
                "{name} is the OFL 1.1"
            );
        }
        // In its `name` table, as UTF-16 big-endian (Windows' platform).
        let notice: Vec<u8> = "Copyright 2022 Google Inc."
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        assert!(
            NOTO_COLOR_EMOJI_FILE
                .windows(notice.len())
                .any(|window| window == notice.as_slice()),
            "the font states its copyright"
        );
    }

    #[test]
    fn the_emoji_face_draws_pictures_and_the_designs_two_do_not() {
        let drawn: Vec<(String, bool)> = fonts()
            .iter()
            .map(|font| (font.family().to_owned(), font.draws_pictures()))
            .collect();
        assert_eq!(
            drawn,
            vec![
                (INSTRUMENT_SANS.to_owned(), false),
                (NEWSREADER.to_owned(), false),
                (NOTO_COLOR_EMOJI.to_owned(), true),
            ]
        );
    }
}
