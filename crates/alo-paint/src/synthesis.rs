/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A slant the font does not have, drawn by leaning the face that it does.
//!
//! A page asking for `font-style: italic` from a family with no slanted face
//! is answered with the upright one, because a face of the right family
//! beats a slanted face of another ([`alo_text::FontDatabase::chain`]). CSS
//! Fonts 4 § 3.3 lets a user agent make up the difference by obliquing the
//! upright face, and `font-synthesis`, whose initial value includes `style`,
//! says that it should. Every browser does. Drawing the face upright instead
//! is a page's emphasis lost without a word, which is what alo Sites'
//! featured testimonial was until this file.
//!
//! # How far, and about what
//!
//! **Fourteen degrees**, which is what CSS Fonts 4 § 3.3 makes `oblique`
//! mean when it is given no angle. `italic` with no italic face and `oblique`
//! with none are both that one angle, since neither names another.
//!
//! **About the baseline**, so a letter keeps its foot where the line put it
//! and leans right as it rises: a point `h` pixels above the baseline moves
//! `h · tan 14°` to the right, and a descender moves as far left. Nothing is
//! measured differently — advances are the upright face's, as in every
//! engine that synthesises — so the layout of a line is unchanged and only
//! its ink leans.
//!
//! # What it is not
//!
//! It is not a substitute for a face. A family that has a slanted face is
//! given it, and nothing is leaned twice. A variable font's `slnt` or `ital`
//! axis would be a better answer than a lean and is not read yet (queue item
//! 197); when it is, a face that can slant itself must not reach here.
//! `font-synthesis: none` and a synthesised bold are queue item 383.

use crate::path::Path;
use alo_text::Slant;
use alo_value::Matrix;

/// The angle an upright face is leaned by when a page asks for a slant it
/// does not have, in degrees: CSS Fonts 4's `oblique` with no angle.
pub const SYNTHETIC_OBLIQUE_DEGREES: f32 = 14.0;

/// How far to lean a face drawn for a request, in degrees: the synthetic
/// angle when a slant was asked for and the face chosen is upright, and none
/// otherwise.
///
/// Upright asked of a slanted face is not straightened: that face was chosen
/// because the family has nothing else, and un-leaning a design is not
/// something CSS asks for.
pub fn oblique_for(asked: Slant, face: Slant) -> f32 {
    match (asked, face) {
        (Slant::Italic, Slant::Normal) => SYNTHETIC_OBLIQUE_DEGREES,
        _ => 0.0,
    }
}

/// A run's outline leaned by `degrees` about the baseline at `baseline`.
///
/// The path is in page coordinates with `y` going down, so a point above the
/// baseline has the smaller `y` and moves right. A lean of nothing, or one
/// that is not a finite angle short of a right angle, leaves the path as it
/// is: the angle is ours today, and a nonsense one must not throw a run's
/// letters to infinity.
pub fn leaned(path: &Path, degrees: f32, baseline: f32) -> Path {
    if degrees == 0.0 || !degrees.is_finite() || degrees.abs() >= 90.0 {
        return path.clone();
    }
    let shear = degrees.to_radians().tan();
    path.transformed(Matrix {
        a: 1.0,
        b: 0.0,
        c: -shear,
        d: 1.0,
        e: shear * baseline,
        f: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::Point;

    fn post(x: f32, from: f32, to: f32) -> Path {
        let mut path = Path::new();
        path.move_to(Point::new(x, from));
        path.line_to(Point::new(x, to));
        path
    }

    fn ends(path: &Path) -> Vec<(f32, f32)> {
        path.segments()
            .iter()
            .filter_map(|segment| match *segment {
                crate::path::Segment::MoveTo(to) | crate::path::Segment::LineTo(to) => {
                    Some((to.x, to.y))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_slant_asked_of_an_upright_face_is_made_up_at_fourteen_degrees() {
        assert_eq!(
            oblique_for(Slant::Italic, Slant::Normal).to_bits(),
            14.0_f32.to_bits()
        );
    }

    #[test]
    fn a_face_that_already_has_the_slant_asked_for_is_not_leaned() {
        assert_eq!(
            oblique_for(Slant::Italic, Slant::Italic).to_bits(),
            0.0_f32.to_bits()
        );
        assert_eq!(
            oblique_for(Slant::Normal, Slant::Normal).to_bits(),
            0.0_f32.to_bits()
        );
        // Nor is a slanted face straightened when upright was asked of it.
        assert_eq!(
            oblique_for(Slant::Normal, Slant::Italic).to_bits(),
            0.0_f32.to_bits()
        );
    }

    #[test]
    fn a_lean_keeps_the_foot_on_the_baseline_and_moves_the_top_right() {
        // A post from 100 px above a baseline at y 200 down to the baseline.
        let leaned = leaned(&post(10.0, 100.0, 200.0), 14.0, 200.0);
        let points = ends(&leaned);
        let top = points.first().copied().expect("the top");
        let foot = points.get(1).copied().expect("the foot");
        // tan 14° is 0.249328…, so the top is 24.93 px right of the foot.
        assert!((top.0 - (10.0 + 24.932_8)).abs() < 0.001, "{top:?}");
        assert_eq!(
            top.1.to_bits(),
            100.0_f32.to_bits(),
            "a lean never moves a point up or down"
        );
        assert_eq!(
            foot.0.to_bits(),
            10.0_f32.to_bits(),
            "the foot stays where the line put it"
        );
        assert_eq!(foot.1.to_bits(), 200.0_f32.to_bits());
    }

    #[test]
    fn a_descender_leans_the_other_way() {
        let leaned = leaned(&post(10.0, 200.0, 220.0), 14.0, 200.0);
        let below = ends(&leaned).get(1).copied().expect("the bottom");
        assert!(below.0 < 10.0, "below the baseline moves left: {below:?}");
    }

    #[test]
    fn a_lean_of_nothing_or_of_nonsense_leaves_the_letters_where_they_are() {
        let upright = post(10.0, 100.0, 200.0);
        for degrees in [
            0.0,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            90.0,
            -90.0,
            1.0e30,
        ] {
            assert_eq!(leaned(&upright, degrees, 200.0), upright, "{degrees}");
        }
    }
}
