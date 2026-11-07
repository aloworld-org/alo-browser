/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where one display item can change pixels (queue item 870).
//!
//! Asked of an item changed between two lists, to say what has to be painted
//! again ([`crate::damage`]), and of an item while a canvas is confined, to
//! pass over one that cannot reach the region ([`crate::render()`]). Both
//! uses lean on the answer being **never too small**: an item said to reach
//! less than it draws would leave a stale pixel, which is a wrong picture.
//! So each kind is answered from the very bounds [`crate::render()`] draws it
//! in — a shape's points under the transform in force, whole pixels
//! outwards and one more ([`Region::around`]), a blur's whole
//! [`crate::blur::reach`] beyond — and a kind whose reach is not that plain
//! is [`Reach::Unknown`], which every caller takes as the whole canvas.

use crate::blur::reach;
use crate::display::DisplayItem;
use crate::path::{Path, Point};
use crate::region::Region;
use crate::render::outlined_glyphs_at;
use crate::synthesis::leaned;
use alo_value::Matrix;

/// What an item can change on a canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// No pixel: it draws nothing, or nothing on the canvas.
    Nothing,
    /// No pixel outside this region.
    Within(Region),
    /// Any pixel, as far as can be said.
    Unknown,
}

impl Reach {
    /// The region, the whole canvas for [`Reach::Unknown`], or [`None`] for
    /// [`Reach::Nothing`].
    pub fn region(self, size: (u32, u32)) -> Option<Region> {
        match self {
            Reach::Nothing => None,
            Reach::Within(region) => Some(region),
            Reach::Unknown => Region::whole(size.0, size.1),
        }
    }
}

/// Where `item` can change pixels on a canvas of `size`, drawn under
/// `current` — the transform in force, the screen's growth in it — with
/// its letters set as on a screen of `scale`.
///
/// Pushes and pops change no pixel themselves; what they change is how
/// every item between them is drawn, which a caller has to settle by
/// comparing them ([`crate::damage`]). A pop is [`Reach::Unknown`] all the
/// same where a group is drawn back, so that nobody takes it for nothing.
pub fn reach_of(item: &DisplayItem, current: Matrix, scale: f32, size: (u32, u32)) -> Reach {
    let shape = |path: &Path, grown: f32| within(&moved(path, current), grown, size);
    match item {
        DisplayItem::Area { .. }
        | DisplayItem::PushClip { .. }
        | DisplayItem::PopClip
        | DisplayItem::PushTransform { .. }
        | DisplayItem::PopTransform
        | DisplayItem::PushGroup { .. } => Reach::Nothing,
        DisplayItem::Fill { path, .. } => shape(path, 0.0),
        DisplayItem::Beneath { rect, .. } => shape(
            &Path::rectangle(rect.left(), rect.top(), rect.size.width, rect.size.height),
            0.0,
        ),
        DisplayItem::Shadow { path, blur, .. } => shape(path, reach(blur * current.scale_factor())),
        DisplayItem::LettersShadow { glyphs, blur, .. } => {
            let grown = reach(blur * current.scale_factor());
            joined(glyphs.iter().map(|glyph| shape(glyph, grown)))
        }
        DisplayItem::LayeredGlyph { layers, .. } => {
            joined(layers.iter().map(|(path, _)| shape(path, 0.0)))
        }
        DisplayItem::Text {
            text,
            origin,
            font,
            oblique,
            size: font_size,
            letter_spacing,
            shadows,
            ..
        } => {
            // The letters exactly as `render` outlines them, and each shadow
            // as it casts them: moved by its offset, softened by its blur.
            let glyphs: Vec<Path> =
                outlined_glyphs_at(text, font, *font_size, scale, *letter_spacing, *origin)
                    .iter()
                    .filter(|glyph| !glyph.is_empty())
                    .map(|glyph| leaned(glyph, *oblique, origin.1))
                    .collect();
            let letters = glyphs.iter().map(|glyph| shape(glyph, 0.0));
            let cast = shadows.iter().flat_map(|shadow| {
                let by = Point::new(shadow.offset.0, shadow.offset.1);
                let grown = reach(shadow.blur * current.scale_factor());
                glyphs
                    .iter()
                    .map(move |glyph| (glyph.translated(by), grown))
            });
            joined(letters.chain(cast.map(|(glyph, grown)| shape(&glyph, grown))))
        }
        // A picture, a colour glyph drawn by its program or its shadow: each
        // drawn by a path of its own, and none of them what an edit changes.
        DisplayItem::Picture { .. }
        | DisplayItem::PaintedGlyph { .. }
        | DisplayItem::GlyphShadow { .. }
        | DisplayItem::PopGroup => Reach::Unknown,
    }
}

/// The pixels a shape already under its transform can change, grown by
/// `grown` device pixels each way.
fn within(path: &Path, grown: f32, size: (u32, u32)) -> Reach {
    let Some((left, top, right, bottom)) = path.bounds() else {
        return Reach::Nothing;
    };
    match Region::around(
        (left - grown, top - grown, right + grown, bottom + grown),
        size,
    ) {
        Some(region) => Reach::Within(region),
        None => Reach::Nothing,
    }
}

/// What several reaches reach together.
fn joined(reaches: impl Iterator<Item = Reach>) -> Reach {
    let mut found = Reach::Nothing;
    for reach in reaches {
        found = match (found, reach) {
            (Reach::Unknown, _) | (_, Reach::Unknown) => return Reach::Unknown,
            (Reach::Nothing, other) | (other, Reach::Nothing) => other,
            (Reach::Within(one), Reach::Within(two)) => Reach::Within(one.joined(two)),
        };
    }
    found
}

/// The same shape under `transform`, left alone under none.
fn moved(path: &Path, transform: Matrix) -> Path {
    if transform.is_identity() {
        return path.clone();
    }
    path.transformed(transform)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fill_rule::FillRule;
    use crate::paint::Paint;
    use alo_box::BoxId;
    use alo_value::Rgba;

    fn fill(x: f32, y: f32, width: f32, height: f32) -> DisplayItem {
        DisplayItem::Fill {
            box_id: BoxId::from_index_for_tests(0),
            path: Path::rectangle(x, y, width, height),
            paint: Paint::Solid(Rgba::BLACK),
            rule: FillRule::NonZero,
        }
    }

    #[test]
    fn a_fill_reaches_its_whole_pixels_at_its_scale_and_one_more() {
        let item = fill(10.25, 4.0, 2.0, 2.0);
        assert_eq!(
            reach_of(&item, Matrix::scaling(2.0), 2.0, (100, 100)),
            Reach::Within(Region {
                left: 19,
                top: 7,
                right: 26,
                bottom: 13
            })
        );
    }

    #[test]
    fn a_shadow_reaches_as_far_as_its_blur_and_off_the_canvas_reaches_nothing() {
        let item = DisplayItem::Shadow {
            box_id: BoxId::from_index_for_tests(0),
            path: Path::rectangle(40.0, 40.0, 10.0, 10.0),
            blur: 8.0,
            color: Rgba::BLACK,
        };
        let far = reach(8.0);
        let Reach::Within(region) = reach_of(&item, Matrix::IDENTITY, 1.0, (200, 200)) else {
            panic!("a shadow on the canvas reaches some of it");
        };
        assert!(far > 0.0);
        assert!(f64::from(region.left) <= f64::from(40.0 - far));
        assert!(f64::from(region.right) >= f64::from(50.0 + far));
        assert_eq!(
            reach_of(
                &fill(500.0, 0.0, 4.0, 4.0),
                Matrix::IDENTITY,
                1.0,
                (200, 200)
            ),
            Reach::Nothing
        );
    }

    #[test]
    fn a_push_changes_no_pixel_and_a_picture_is_unknown() {
        let push = DisplayItem::PushGroup {
            box_id: BoxId::from_index_for_tests(0),
            opacity: 0.5,
        };
        assert_eq!(
            reach_of(&push, Matrix::IDENTITY, 1.0, (8, 8)),
            Reach::Nothing
        );
        assert_eq!(
            reach_of(&DisplayItem::PopGroup, Matrix::IDENTITY, 1.0, (8, 8)),
            Reach::Unknown
        );
        assert_eq!(Reach::Unknown.region((8, 8)), Region::whole(8, 8));
    }
}
