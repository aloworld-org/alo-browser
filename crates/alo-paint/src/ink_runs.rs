/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which font draws each part of a piece of text.
//!
//! Layout measures a line run by run, each run in the first font of its
//! fallback chain that **has** its characters (`alo_text::split`), so a
//! `⌘` in a family without one is measured in the next font that has it.
//! Paint drew the whole piece in the chain's first font, which only agreed
//! with layout while that font had every character it was asked for. Queue
//! item 706 made it disagree where a person sees it: the interface's
//! Instrument Sans has no `⌘`, so *New chat*'s `⌘T` was drawn as a missing
//! glyph's box, and a site's name in Greek or Cyrillic would have been too.
//!
//! So a piece is drawn as the runs it was measured in: the same split,
//! each run in its font, each starting where the runs before it ended.
//!
//! # What nothing has
//!
//! Characters no font has are a run of their own with no font, which
//! layout gives no room. They are drawn as they were before this file, in
//! the chain's first font, with the run they are next to — so a page whose
//! characters some font has is drawn in that font, and nothing else about
//! what is drawn changes.

use crate::render::run_width_at;
use alo_text::{Font, FontDatabase, FontRequest};

/// One run of a piece of text, in the font that draws it.
#[derive(Debug, Clone)]
pub struct InkRun<'a> {
    /// What it says.
    pub text: &'a str,
    /// The font it is drawn in.
    pub font: Font,
    /// Where its pen starts, on the baseline.
    pub origin: (f32, f32),
}

/// How a piece of text is set, for [`runs`].
#[derive(Debug, Clone, Copy)]
pub struct Setting<'a> {
    /// The fonts the page has.
    pub fonts: &'a FontDatabase,
    /// What the text asked for.
    pub request: &'a FontRequest,
    /// The chain's first font: what a run no font has is drawn in.
    pub first: &'a Font,
    /// How big, in CSS pixels.
    pub size: f32,
    /// Device pixels to the CSS pixel.
    pub scale: f32,
    /// Extra room after every character.
    pub letter_spacing: f32,
}

/// `text` as the runs it is drawn in, the first at `origin`.
///
/// Neighbouring runs in the same font are one: a piece all in one font is
/// one run, exactly `text`, in that font, at `origin`.
pub fn runs<'a>(text: &'a str, origin: (f32, f32), setting: Setting<'_>) -> Vec<InkRun<'a>> {
    let mut spans: Vec<(std::ops::Range<usize>, Font)> = Vec::new();
    for run in alo_text::split(text, setting.fonts, setting.request) {
        let font = run.font.unwrap_or_else(|| setting.first.clone());
        match spans.last_mut() {
            Some((range, held)) if same_face(held, &font) && range.end == run.range.start => {
                range.end = run.range.end;
            }
            _ => spans.push((run.range, font)),
        }
    }
    let mut pen = origin.0;
    spans
        .into_iter()
        .filter_map(|(range, font)| {
            let text = text.get(range)?;
            let run = InkRun {
                text,
                origin: (pen, origin.1),
                font,
            };
            pen += run_width_at(
                text,
                &run.font,
                setting.size,
                setting.scale,
                setting.letter_spacing,
            );
            Some(run)
        })
        .collect()
}

/// Whether two fonts are the same face set to the same instance.
fn same_face(one: &Font, two: &Font) -> bool {
    one.family() == two.family() && one.weight() == two.weight() && one.slant() == two.slant()
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_text::{Slant, Weight};

    /// `DejaVu Serif`, which has no `※` (U+203B), before `DejaVu Sans`, which does.
    fn fonts() -> FontDatabase {
        let mut database = FontDatabase::new();
        for (family, bytes) in [
            ("DejaVu Serif", dejavu::serif::regular()),
            ("DejaVu Sans", dejavu::sans::regular()),
        ] {
            if let Some(font) = Font::load(family, Weight::NORMAL, Slant::Normal, bytes.to_vec()) {
                database.add(font);
            }
        }
        database
    }

    fn drawn(text: &str) -> Vec<(String, String, u32)> {
        let fonts = fonts();
        let request = FontRequest {
            families: vec!["DejaVu Serif".to_owned()],
            ..FontRequest::default()
        };
        let Some(first) = fonts.chain(&request).into_iter().next() else {
            return Vec::new();
        };
        let setting = Setting {
            fonts: &fonts,
            request: &request,
            first: &first,
            size: 16.0,
            scale: 1.0,
            letter_spacing: 0.0,
        };
        runs(text, (10.0, 20.0), setting)
            .into_iter()
            .map(|run| {
                // In 64ths, where layout puts every length (ADR 0054).
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "a pen a few hundred pixels along, times 64"
                )]
                let at = (run.origin.0 * 64.0).round() as u32;
                (run.text.to_owned(), run.font.family().to_owned(), at)
            })
            .collect()
    }

    #[test]
    fn text_one_font_has_is_one_run_at_the_origin() {
        assert_eq!(
            drawn("New chat"),
            vec![("New chat".to_owned(), "DejaVu Serif".to_owned(), 640)]
        );
    }

    #[test]
    fn a_character_the_first_font_lacks_is_drawn_in_the_font_that_has_it() {
        let runs = drawn("※T");
        let families: Vec<(&str, &str)> = runs
            .iter()
            .map(|(text, family, _)| (text.as_str(), family.as_str()))
            .collect();
        assert_eq!(families, vec![("※", "DejaVu Sans"), ("T", "DejaVu Serif")]);
        // `T` starts where `※` ends: 10 px, then the width of `※` in
        // `DejaVu Sans` at 16 px.
        let sans = fonts().chain(&FontRequest::default()).into_iter().nth(1);
        assert!(sans.is_some(), "DejaVu Sans is loaded");
        let Some(sans) = sans else {
            return;
        };
        let width = run_width_at("※", &sans, 16.0, 1.0, 0.0);
        let starts: Vec<u32> = runs.iter().map(|(_, _, at)| *at).collect();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a pen a few dozen pixels along, times 64"
        )]
        let after = ((10.0 + width) * 64.0).round() as u32;
        assert_eq!(starts, vec![640, after]);
        assert!(after > 640, "the second run starts after the first");
    }

    #[test]
    fn what_no_font_has_stays_with_its_neighbour_in_the_first_font() {
        // U+E000 is private use: neither face has it.
        assert_eq!(
            drawn("a\u{e000}b"),
            vec![("a\u{e000}b".to_owned(), "DejaVu Serif".to_owned(), 640)]
        );
    }
}
