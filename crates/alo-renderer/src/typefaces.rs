/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The interface's own typefaces, carried by the application (queue items
//! 706 and 853, ADR 0084).
//!
//! The owner's design sets every word of the interface in **Manrope** (since
//! 12 November 2026, queue item 975, ADR 0112 *Answered* 3; Inter from 7
//! November, queue item 853; before, Instrument Sans), what it sets in a
//! monospace in **Geist Mono**, and the mark *alo* and the greeting in
//! **Newsreader**, whatever else is chosen. A person may set the
//! interface's words in one of five other faces instead — **Inter**,
//! **Geist**, **DM Sans**, **Plus Jakarta Sans** and **Instrument Sans** —
//! in *Settings* (queue item 854, [`crate::interface_face`]). All eight are
//! under the SIL Open Font License 1.1, and fonts are rented physics
//! (`CLAUDE.md`), so each is the file its foundry or Google Fonts publishes,
//! unmodified, beside its own licence in `interface/fonts/` — ADR 0084
//! records where each came from and its hashes.
//!
//! # Variable, and set by weight and size alone
//!
//! Every one is a variable font, one file for every weight the design uses
//! (400, 500 and 600; Newsreader 400 and 500). Manrope's file names its
//! lightest instance, 200, as its own, and is drawn at the weight asked
//! for as every other is. This engine sets a variable
//! font's `wght` axis to the weight asked for — measured, shaped and
//! outlined at that instance (`alo_text::Font::at_weight`) — and its `opsz`
//! axis, where it has one, to the CSS pixels the text is set at, as Chrome
//! does (`font-optical-sizing: auto`, queue item 752): Newsreader's from 6
//! to 72, Inter's from 14 to 32 and DM Sans' from 9 to 40. Every other axis
//! is left at the file's default: Instrument Sans' `wdth` at 100, the
//! design's.
//!
//! # Compiled in, and handed over as every font is
//!
//! The bytes are part of the program, as the corpus's `DejaVu` faces are, so
//! nothing is read from a disk or a network to find them. A renderer is
//! confined and opens no file (ADR 0010); the browser process hands these to
//! the conversation's renderers as bytes on the pipe, as it hands every face
//! ([`crate::face`]) — all of them, so a face chosen in *Settings* is drawn
//! at once, with nothing handed again. Only the interface's renderers are
//! handed them — never a page's — so no page is drawn in them, and none can
//! tell from its fonts that it is in this browser.
//!
//! # And a face for emoji
//!
//! No family here has emoji, and a search's titles and descriptions are
//! full of them. So the interface carries one more face, **Noto Color
//! Emoji**, also under the OFL 1.1 — the file `googlefonts/noto-emoji`
//! publishes, unmodified — whose glyphs are colour pictures (`CBDT`), drawn
//! by `alo-paint` as pictures (ADR 0088). It is handed to the interface's
//! renderers with the others, never to a page's, and the shaper looks in
//! it first for a character meant to be shown as an emoji
//! (`alo_text::emoji`).
//!
//! # The licence goes with them
//!
//! The OFL lets the fonts be bundled with software so long as each file's
//! copyright notice and the licence go with it. [`LICENCES`] are those
//! files, as published, and `alo-bundle` puts them in the application's
//! `Contents/Resources/licences`.

use crate::face::Face;
use alo_text::Font;

/// The family the interface's words are set in unless a person chose
/// another (queue item 975, ADR 0112 *Answered* 3): Manrope, its file's
/// typographic family.
pub const MANROPE: &str = "Manrope";

/// The family the interface sets a monospace in (queue item 975): Geist
/// Mono, whatever face is chosen for its words.
pub const GEIST_MONO: &str = "Geist Mono";

/// Inter, the interface's default face from queue item 853 to 975 and a
/// face a person may choose in *Settings* since: its file calls it *Inter
/// Variable* — its `name` table has no typographic family, so that is the
/// family a stylesheet asks for (ADR 0084 § 2).
pub const INTER: &str = "Inter Variable";

/// Geist, a face a person may choose in *Settings* (queue item 854).
pub const GEIST: &str = "Geist";

/// DM Sans, a face a person may choose in *Settings*: its typographic
/// family (its legacy one is *DM Sans 9pt*).
pub const DM_SANS: &str = "DM Sans";

/// Plus Jakarta Sans, a face a person may choose in *Settings*.
pub const PLUS_JAKARTA_SANS: &str = "Plus Jakarta Sans";

/// Instrument Sans: the interface's face until queue item 853, kept as a
/// fifth choice in *Settings*.
pub const INSTRUMENT_SANS: &str = "Instrument Sans";

/// The family the mark and the greeting are set in, whatever is chosen.
pub const NEWSREADER: &str = "Newsreader";

/// `ofl/manrope/Manrope[wght].ttf` of `google/fonts`, as published.
const MANROPE_FILE: &[u8] = include_bytes!("../interface/fonts/Manrope.ttf");

/// `geist-font/GeistMono/variable/GeistMono[wght].ttf` of Geist 1.9.2's
/// release (`vercel/geist-font`), as published; its licence is Geist's.
const GEIST_MONO_FILE: &[u8] = include_bytes!("../interface/fonts/GeistMono.ttf");

/// `InterVariable.ttf` of Inter 4.1's release (`rsms/inter`), as published.
const INTER_FILE: &[u8] = include_bytes!("../interface/fonts/Inter.ttf");

/// `geist-font/Geist/variable/Geist[wght].ttf` of Geist 1.9.2's release
/// (`vercel/geist-font`), as published.
const GEIST_FILE: &[u8] = include_bytes!("../interface/fonts/Geist.ttf");

/// `ofl/dmsans/DMSans[opsz,wght].ttf` of `google/fonts`, as published.
const DM_SANS_FILE: &[u8] = include_bytes!("../interface/fonts/DMSans.ttf");

/// `ofl/plusjakartasans/PlusJakartaSans[wght].ttf` of `google/fonts`, as
/// published.
const PLUS_JAKARTA_SANS_FILE: &[u8] = include_bytes!("../interface/fonts/PlusJakartaSans.ttf");

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
/// the font (Noto's notice is in the font itself). Geist Mono is Geist's
/// project's, under Geist's licence, the one file the release carries for
/// both.
pub const LICENCES: [(&str, &str); 8] = [
    (
        "OFL-Manrope.txt",
        include_str!("../interface/fonts/OFL-Manrope.txt"),
    ),
    (
        "OFL-Inter.txt",
        include_str!("../interface/fonts/OFL-Inter.txt"),
    ),
    (
        "OFL-Geist.txt",
        include_str!("../interface/fonts/OFL-Geist.txt"),
    ),
    (
        "OFL-DMSans.txt",
        include_str!("../interface/fonts/OFL-DMSans.txt"),
    ),
    (
        "OFL-PlusJakartaSans.txt",
        include_str!("../interface/fonts/OFL-PlusJakartaSans.txt"),
    ),
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

/// The interface's faces, as a renderer is handed them: the six a
/// person may set its words in, Manrope first, then Geist Mono, then
/// Newsreader, then Noto Color Emoji, each filed under what the font says
/// it is.
///
/// A face the font would not describe is left out rather than filed under
/// a name written here, which is a broken build rather than anything a page
/// could cause, and which the tests below would say.
pub fn faces() -> Vec<Face> {
    [
        MANROPE_FILE,
        INTER_FILE,
        GEIST_FILE,
        DM_SANS_FILE,
        PLUS_JAKARTA_SANS_FILE,
        INSTRUMENT_SANS_FILE,
        GEIST_MONO_FILE,
        NEWSREADER_FILE,
        NOTO_COLOR_EMOJI_FILE,
    ]
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

    /// The six faces a person may set the interface's words in, in the
    /// order they are handed, then the monospace, the serif and the emoji
    /// face.
    const FAMILIES: [&str; 9] = [
        MANROPE,
        INTER,
        GEIST,
        DM_SANS,
        PLUS_JAKARTA_SANS,
        INSTRUMENT_SANS,
        GEIST_MONO,
        NEWSREADER,
        NOTO_COLOR_EMOJI,
    ];

    #[test]
    fn the_faces_are_filed_under_what_each_font_says_it_is() {
        let named: Vec<(String, u16, Slant)> = faces()
            .into_iter()
            .map(|face| (face.family, face.weight, face.slant))
            .collect();
        assert_eq!(
            named,
            FAMILIES
                .iter()
                // Manrope's file names its lightest instance as its own.
                .map(|family| {
                    let weight = if *family == MANROPE { 200 } else { 400 };
                    ((*family).to_owned(), weight, Slant::Normal)
                })
                .collect::<Vec<_>>()
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
                (MANROPE.to_owned(), Some((200, 800))),
                (INTER.to_owned(), Some((100, 900))),
                (GEIST.to_owned(), Some((100, 900))),
                (DM_SANS.to_owned(), Some((100, 1000))),
                (PLUS_JAKARTA_SANS.to_owned(), Some((200, 800))),
                (INSTRUMENT_SANS.to_owned(), Some((400, 700))),
                (GEIST_MONO.to_owned(), Some((100, 900))),
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
        let (designs, emoji) = LICENCES.split_at(7);
        let projects = [
            "The Manrope Project Authors",
            "The Inter Project Authors",
            "The Geist Project Authors",
            "The DM Sans Project Authors",
            "The Plus Jakarta Sans Project Authors",
            "The Instrument Sans Project Authors",
            "The Newsreader Project Authors",
        ];
        for ((name, text), project) in designs.iter().zip(projects) {
            assert!(
                text.starts_with("Copyright "),
                "{name} starts with its copyright notice"
            );
            assert!(text.contains(project), "{name}");
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
        // Geist Mono is carried under Geist's licence: its own notice names
        // the same project.
        let geists: Vec<u8> = "Copyright 2024 The Geist Project Authors"
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        assert!(
            GEIST_MONO_FILE
                .windows(geists.len())
                .any(|window| window == geists.as_slice()),
            "Geist Mono's notice is Geist's project's"
        );
    }

    #[test]
    fn inter_and_dm_sans_vary_by_optical_size_and_the_rest_do_not() {
        let sized: Vec<(String, Option<(f32, f32)>)> = fonts()
            .iter()
            .map(|font| (font.family().to_owned(), font.optical_axis()))
            .collect();
        assert_eq!(
            sized,
            vec![
                (MANROPE.to_owned(), None),
                (INTER.to_owned(), Some((14.0, 32.0))),
                (GEIST.to_owned(), None),
                (DM_SANS.to_owned(), Some((9.0, 40.0))),
                (PLUS_JAKARTA_SANS.to_owned(), None),
                (INSTRUMENT_SANS.to_owned(), None),
                (GEIST_MONO.to_owned(), None),
                (NEWSREADER.to_owned(), Some((6.0, 72.0))),
                (NOTO_COLOR_EMOJI.to_owned(), None),
            ]
        );
    }

    #[test]
    fn the_emoji_face_draws_pictures_and_the_designs_do_not() {
        let drawn: Vec<(String, bool)> = fonts()
            .iter()
            .map(|font| (font.family().to_owned(), font.draws_pictures()))
            .collect();
        assert_eq!(
            drawn,
            FAMILIES
                .iter()
                .map(|family| ((*family).to_owned(), *family == NOTO_COLOR_EMOJI))
                .collect::<Vec<_>>()
        );
    }
}
