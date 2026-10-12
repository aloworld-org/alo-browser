/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The size a face is set at: the font size cut to hundredths of a pixel.
//!
//! A font size is whatever the cascade made it, and `13.333333px` is a size
//! every control has. Chrome 154 does not set a face at that: it sets it at
//! the size **cut to hundredths, in `f32`**, so 13.333333 is 13.33 and
//! 10.666667 is 10.66. Every figure read from the face follows: a run's
//! advances, the ascent, descent and line gap a line takes (ADR 0050 § 1),
//! and the `x`, the `0` and the x-height that `ch`, `ex` and a text field's
//! width are counted in. The size itself does not: `1em` at 13.333333 px is
//! still 13.333333.
//!
//! Measured in headless Chrome 154, the source not read, in the corpus's
//! `DejaVu Sans`, `DejaVu Sans Mono` and `DejaVu Serif` at thirty sizes
//! (queue item 514). Of 720 runs of text, 717 were this engine's width at
//! the cut size and 318 at the size as written; all 720 line heights were
//! the cut size's and 696 the written one's. `50ch` and `50ex` were the cut
//! size's in all 90 boxes, and `50em` the written one's. The cut is in
//! `f32`, as the size is held: 13.33 is 13.3299999 as a float, and its
//! hundredfold rounds to 1,333 there, where in double it would be
//! 1,332.99999 and cut to 13.32, which Chrome does not do. The float cuts
//! the other way too: `8.11px` is 8.1099997, its hundredfold 810.99994,
//! and Chrome sets it at 8.1. Of 1,152 runs at 48 sizes, 40 of them cut
//! this way, 1,151 were the `f32` cut's width; 860 would have been another
//! width at the size as written, and none of Chrome's was that one.
//!
//! Six of the 2,922 runs measured in all are a unit off Chrome's. In each
//! this engine's sum lies within 0.03 of a unit of a grid line, so they are
//! how the shaper's advances are summed rather than the size (queue item
//! 527).

/// The size a face is set at, for a font size in CSS pixels.
///
/// Truncated toward zero at the hundredth, in `f32`: `13.333333` is 13.33,
/// `13.339999` is 13.33, `13.33` stays 13.33 and `16` stays 16. A size
/// whose hundredfold is past what an `f32` holds is left as it is, rather
/// than made infinite, and a size that is not a number stays not a number.
pub fn face_size(size: f32) -> f32 {
    let hundredfold = size * 100.0;
    if hundredfold.is_finite() {
        hundredfold.trunc() / 100.0
    } else {
        size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same(a: f32, b: f32) -> bool {
        a.to_bits() == b.to_bits()
    }

    #[test]
    fn a_size_is_cut_to_hundredths() {
        assert!(same(face_size(13.333_333), 13.33));
        assert!(same(face_size(10.666_667), 10.66));
        assert!(same(face_size(13.339_999), 13.33));
        assert!(same(face_size(24.999_9), 24.99));
    }

    #[test]
    fn a_size_already_in_hundredths_stays_where_it_is() {
        // 13.33 is 13.329_999_9 as a float. Its hundredfold in double is
        // 1,332.999_99, which would cut it to 13.32; in `f32` it is 1,333.
        for size in [13.33_f32, 10.66, 12.34, 14.2, 17.1, 21.7, 16.0, 13.0] {
            assert!(same(face_size(size), size), "{size}");
        }
    }

    #[test]
    fn a_hundredth_the_float_holds_short_of_is_cut_below_it() {
        // 8.11 is 8.109_999_7 as a float, and its hundredfold 810.999_94:
        // Chrome sets `8.11px` at 8.1, and `font-size: 33.1px` at 33.09.
        assert!(same(face_size(8.11), 8.1));
        assert!(same(face_size(33.1), 33.09));
    }

    #[test]
    fn a_size_out_of_the_ordinary_is_cut_without_harm() {
        assert!(same(face_size(0.0), 0.0));
        assert!(same(face_size(0.004), 0.0));
        assert!(face_size(f32::NAN).is_nan());
        assert!(same(face_size(f32::INFINITY), f32::INFINITY));
        assert!(same(face_size(f32::MAX), f32::MAX), "not made infinite");
        let huge = face_size(1.0e30);
        assert!(huge.is_finite() && (huge / 1.0e30 - 1.0).abs() < 1.0e-6);
        assert!(same(face_size(-13.333_333), -13.33), "toward zero");
    }
}
