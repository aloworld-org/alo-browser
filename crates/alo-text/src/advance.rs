/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the rented shaper rounds off a varied glyph's advance (queue item
//! 795).
//!
//! A variable font's glyph is as wide as its `hmtx` advance moved by the
//! delta `HVAR` interpolates for the instance — or, with no `HVAR`, by its
//! right phantom point's in `gvar`. The delta is a fraction of a font unit
//! almost everywhere but at a master. `rustybuzz` 0.20.1 asks `ttf-parser`
//! 0.25.1 for the advance, which rounds it to a whole unit, and positions
//! in whole units; Chrome 156 keeps the fraction (measured: Newsreader's
//! `o` at `opsz` 40 is 21.2311 px on its canvas, 21.24 rounded here;
//! Instrument Sans at 600, every glyph, up to 0.008 px at 16 px).
//!
//! So [`lost`] gives back what was rounded off — the exact advance less the
//! rounded one, in font units — for [`crate::shape`] to add to each glyph's
//! advance. A glyph whose advance the shaper set to nothing — a mark it
//! zeroed — keeps nothing.

use crate::font::{GlyphKind, Varied};

/// How much wider `glyph` is at `face`'s instance than the whole font units
/// the shaper was given for it: a fraction of a unit either way, or nothing
/// where the glyph's advance does not vary.
pub(crate) fn lost(face: &Varied<'_>, glyph: u16) -> f32 {
    face.advance(glyph).map_or(0.0, |(base, delta, rounded)| {
        f32::from(base) + delta - f32::from(rounded)
    })
}

/// Whether the face says `glyph` is a mark, whose advance a shaper sets
/// to nothing and which keeps nothing of [`lost`].
pub(crate) fn is_mark(face: &Varied<'_>, glyph: u16) -> bool {
    face.kind(glyph) == Some(GlyphKind::Mark)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{Font, OpticalSizing, Slant, Weight};

    fn newsreader(sizing: OpticalSizing) -> Option<Font> {
        let data = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../alo-renderer/interface/fonts/Newsreader.ttf"
        ))
        .ok()?;
        Some(Font::load("Newsreader", Weight::NORMAL, Slant::Normal, data)?.sized_optically(sizing))
    }

    #[test]
    fn at_an_instance_between_masters_the_fraction_rounded_off_is_given_back() {
        let Some(font) = newsreader(OpticalSizing::Auto) else {
            panic!("Newsreader is not a font");
        };
        // At 18 px, its default `opsz` and weight: no instance to correct.
        assert!(font.varied(18.0).is_none());
        let Some(face) = font.varied(40.0) else {
            panic!("Newsreader at 40 is not varied");
        };
        let o = face.glyph('o').unwrap_or_default();
        // `o` at `opsz` 40: 1061.55 units, given as 1062 — Chrome's canvas
        // 21.231 px at 40 is 1061.56 of the 2,000-unit em. The last
        // hundredth of a unit is `ttf-parser`'s coordinate, cut to 6,674
        // of 16,384 where 40 is 6,674.96: 0.000 15 px at 40.
        let lost = lost(&face, o);
        assert!(lost < 0.0 && lost > -0.5, "{lost}");
        let exact = face
            .advance(o)
            .map_or(0.0, |(_, _, rounded)| f32::from(rounded))
            + lost;
        assert!((exact * 40.0 / 2000.0 - 21.231_11).abs() < 1e-3, "{exact}");
        // At 72, a master again: whole units.
        let Some(face) = font.varied(72.0) else {
            panic!("Newsreader at 72 is not varied");
        };
        assert!(super::lost(&face, o).abs() < 1e-3);
    }

    #[test]
    fn a_mark_is_known_by_its_class() {
        let Some(font) = newsreader(OpticalSizing::Auto) else {
            panic!("Newsreader is not a font");
        };
        let Some(face) = font.varied(40.0) else {
            panic!("Newsreader at 40 is not varied");
        };
        let acute = face.glyph('\u{301}').unwrap_or_default();
        let o = face.glyph('o').unwrap_or_default();
        assert!(is_mark(&face, acute));
        assert!(!is_mark(&face, o));
    }
}
