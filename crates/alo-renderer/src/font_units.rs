/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The cascade's `ex` and `ch`, measured in the fonts this renderer was
//! handed.
//!
//! `alo-style` asks (`alo_style::MeasureFace`) and cannot answer: it has no
//! fonts and must not open any. `alo-text` has the fonts and does not know
//! what a computed style is. This renderer knows both, so the answer is
//! assembled here, and it is the same face layout sets the text in because
//! both read the style through `alo_layout::text_style_of`.

use alo_style::{ComputedStyle, FaceUnits, MeasureFace};
use alo_text::{FontDatabase, TextMeasurer};

/// The faces of a font database, measured for the cascade.
#[derive(Debug, Clone, Copy)]
pub struct Faces<'a> {
    measurer: TextMeasurer<'a>,
}

impl<'a> Faces<'a> {
    /// The faces in these fonts.
    pub fn new(fonts: &'a FontDatabase) -> Self {
        Self {
            measurer: TextMeasurer::new(fonts),
        }
    }
}

impl MeasureFace for Faces<'_> {
    fn face_units(&self, style: &ComputedStyle) -> Option<FaceUnits> {
        // The first face of the chain, which is the first available font CSS
        // measures `ex` and `ch` in. A face with no `0` or no `x` has already
        // answered half its size for that unit, in `alo_text::Font::metrics`.
        let metrics = self.measurer.face(&alo_layout::text_style_of(style))?;
        Some(FaceUnits {
            x_height: metrics.x_height,
            zero_width: metrics.zero_width,
        })
    }
}
