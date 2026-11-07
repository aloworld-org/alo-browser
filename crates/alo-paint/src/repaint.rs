/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A list painted over the drawing of the list before it, only where the
//! two differ (queue item 870, ADR 0101).
//!
//! A letter typed into the one box changes its text and its caret; the rest
//! of the first screen — a card's shadows blurred at two, its fills and
//! clips, the place left clear for its page — is the same list it was. So
//! the canvas that holds the last drawing is painted again only inside the
//! region where the two lists can differ ([`crate::damage`]): that region is
//! put back to the colour a new canvas starts as, the canvas is confined to
//! it ([`Canvas::confine`]), and the whole new list is drawn. Every pixel
//! inside comes out as drawing the new list onto a new canvas would have
//! made it, since it is the same drawing; every pixel outside is one the two
//! lists draw alike. What a repaint saves is what [`crate::render()`] passes
//! over on a confined canvas: an item that cannot reach the region, and a
//! clip that cannot.

use crate::canvas::Canvas;
use crate::damage::{Damage, damage};
use crate::display::DisplayList;
use crate::region::Region;
use crate::render::render;
use alo_value::Rgba;

/// What painting a list again changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repainted {
    /// No pixel: the list draws exactly what the canvas holds.
    Nothing,
    /// The pixels of this region, and no others.
    Within(Region),
    /// Every pixel: drawn from nothing.
    Whole,
}

impl Repainted {
    /// How many pixels were painted again on a canvas of `size`.
    pub fn area(self, size: (u32, u32)) -> u64 {
        match self {
            Repainted::Nothing => 0,
            Repainted::Within(region) => region.area(),
            Repainted::Whole => u64::from(size.0) * u64::from(size.1),
        }
    }
}

/// Make `canvas`, which holds `before` drawn onto a canvas its size made in
/// `ground`, hold `after` drawn the same way, painting only what has to be.
///
/// The caller vouches for what the canvas holds. A canvas holding anything
/// else comes out wrong where the lists draw alike, which is why
/// `alo-renderer` hands one in only with the list it was drawn from, at the
/// same size, scale and ground.
pub fn repaint(
    before: &DisplayList,
    after: &DisplayList,
    canvas: &mut Canvas,
    ground: Rgba,
) -> Repainted {
    let size = (canvas.width(), canvas.height());
    match damage(before, after, size) {
        Damage::None => Repainted::Nothing,
        Damage::Within(region) => {
            canvas.reset(region, ground);
            canvas.confine(Some(region));
            render(after, canvas);
            canvas.confine(None);
            Repainted::Within(region)
        }
        Damage::Whole => {
            *canvas = Canvas::new(size.0, size.1, ground);
            render(after, canvas);
            Repainted::Whole
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::DisplayItem;
    use crate::fill_rule::FillRule;
    use crate::paint::Paint;
    use crate::path::Path;
    use alo_box::BoxId;

    fn list(items: Vec<DisplayItem>) -> DisplayList {
        let mut list = DisplayList::default();
        for item in items {
            list.push(item);
        }
        list
    }

    fn fill(x: f32, y: f32, width: f32, height: f32, color: Rgba) -> DisplayItem {
        DisplayItem::Fill {
            box_id: BoxId::from_index_for_tests(0),
            path: crate::corner::rounded_rectangle(
                alo_layout::Rect::new(x, y, width, height),
                crate::corner::Corners::all(3.0),
            ),
            paint: Paint::Solid(color),
            rule: FillRule::NonZero,
        }
    }

    fn shadow(x: f32, y: f32) -> DisplayItem {
        DisplayItem::Shadow {
            box_id: BoxId::from_index_for_tests(0),
            path: Path::rectangle(x, y, 30.0, 20.0),
            blur: 6.0,
            color: Rgba::from_rgba8(0, 0, 0, 90),
        }
    }

    fn drawn(list: &DisplayList, size: (u32, u32)) -> Canvas {
        let mut canvas = Canvas::new(size.0, size.1, Rgba::WHITE);
        render(list, &mut canvas);
        canvas
    }

    #[test]
    fn a_moved_shape_is_painted_again_only_around_it_and_comes_out_as_drawn_whole() {
        let backdrop = || {
            vec![
                fill(0.0, 0.0, 120.0, 80.0, Rgba::from_rgba8(240, 238, 230, 255)),
                shadow(70.0, 40.0),
                fill(70.0, 40.0, 30.0, 20.0, Rgba::WHITE),
            ]
        };
        let mut before = backdrop();
        before.push(fill(
            10.25,
            10.5,
            8.0,
            6.0,
            Rgba::from_rgba8(20, 40, 200, 160),
        ));
        let mut after = backdrop();
        after.push(fill(
            14.75,
            10.5,
            8.0,
            6.0,
            Rgba::from_rgba8(20, 40, 200, 160),
        ));
        let (before, after) = (list(before), list(after));
        let mut canvas = drawn(&before, (120, 80));
        let repainted = repaint(&before, &after, &mut canvas, Rgba::WHITE);
        assert_eq!(canvas, drawn(&after, (120, 80)));
        assert_eq!(
            repainted,
            Repainted::Within(Region {
                left: 9,
                top: 9,
                right: 24,
                bottom: 18
            })
        );
        assert!(
            canvas.confinement().is_none(),
            "the canvas is let go of after"
        );
    }

    #[test]
    fn a_translucent_picture_across_the_region_is_drawn_again_only_inside_it() {
        // Drawn over itself outside the region, a translucent picture would
        // be blended twice: only the canvas itself keeps it inside, since a
        // picture is not drawn as coverage.
        let picture = std::sync::Arc::new(Canvas::new(20, 20, Rgba::from_rgba8(200, 0, 0, 128)));
        let with = |x: f32| {
            list(vec![
                DisplayItem::Picture {
                    box_id: BoxId::from_index_for_tests(0),
                    rect: alo_layout::Rect::new(0.0, 0.0, 40.0, 40.0),
                    picture: std::sync::Arc::clone(&picture),
                    fade: u8::MAX,
                },
                fill(x, 30.0, 4.0, 4.0, Rgba::BLACK),
            ])
        };
        let (before, after) = (with(30.0), with(34.0));
        let mut canvas = drawn(&before, (60, 50));
        let repainted = repaint(&before, &after, &mut canvas, Rgba::WHITE);
        assert!(matches!(repainted, Repainted::Within(_)), "{repainted:?}");
        assert_eq!(canvas, drawn(&after, (60, 50)));
    }

    #[test]
    fn the_same_list_paints_nothing_and_a_new_scale_paints_everything() {
        let one = list(vec![fill(2.0, 2.0, 4.0, 4.0, Rgba::BLACK)]);
        let mut canvas = drawn(&one, (8, 8));
        assert_eq!(
            repaint(&one, &one, &mut canvas, Rgba::WHITE),
            Repainted::Nothing
        );
        let two = DisplayList::from_parts(one.items().to_vec(), Vec::new(), 2.0);
        let mut grown = Canvas::new(16, 16, Rgba::WHITE);
        assert_eq!(
            repaint(&one, &two, &mut grown, Rgba::WHITE),
            Repainted::Whole
        );
        assert_eq!(grown, drawn(&two, (16, 16)));
        assert_eq!(Repainted::Whole.area((16, 16)), 256);
    }
}
