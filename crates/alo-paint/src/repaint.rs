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
use crate::damage::{Damage, damage_standing};
use crate::display::DisplayList;
use crate::kept::Kept;
use crate::region::Region;
use crate::render::draw;
use alo_value::Rgba;

/// What painting a list again changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repainted {
    /// No pixel: the list draws exactly what the canvas holds.
    Nothing,
    /// The pixels of this region, and no others: the smallest holding every
    /// piece painted again ([`crate::pieces`]), which is what changed.
    Within(Region),
    /// Every pixel: drawn from nothing.
    Whole,
}

impl Repainted {
    /// What two paints one after the other changed between them (queue
    /// item 872): the smallest region holding both, or every pixel.
    #[must_use]
    pub fn joined(self, other: Self) -> Self {
        match (self, other) {
            (Repainted::Whole, _) | (_, Repainted::Whole) => Repainted::Whole,
            (Repainted::Nothing, other) | (other, Repainted::Nothing) => other,
            (Repainted::Within(one), Repainted::Within(two)) => Repainted::Within(one.joined(two)),
        }
    }

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
///
/// What coverage `kept` holds is taken where it can be and what is made is
/// kept for the next drawing ([`crate::kept`], queue item 871): the masks of
/// clips compared, and every shape, shadow and clip drawn or passed over.
pub fn repaint(
    before: &DisplayList,
    after: &DisplayList,
    canvas: &mut Canvas,
    ground: Rgba,
    kept: &mut Kept,
) -> Repainted {
    let size = (canvas.width(), canvas.height());
    // Every pixel drawn is drawn onto `ground`, a new canvas or a piece put
    // back to it: what a backdrop's blur is kept as blurred from (queue
    // item 894).
    kept.on(Some(ground));
    kept.begin();
    let (damage, standing) = damage_standing(before, after, size, kept);
    let repainted = match damage {
        // Nothing drawn, and what is kept is kept as it was: the two lists
        // draw alike, so it is all still this list's.
        Damage::None => return Repainted::Nothing,
        Damage::Within(pieces) => {
            // A backdrop whose crop nothing under it changed is drawn from
            // the blur the last drawing kept (queue item 885): the region
            // need not hold all of the crop it reads.
            kept.stand(standing);
            // Each piece on its own (queue item 891), as one region was:
            // every pixel in it as drawn whole, whichever piece is first.
            let mut bounds: Option<Region> = None;
            for &piece in &pieces {
                canvas.reset(piece, ground);
                canvas.confine(Some(piece));
                draw(after, canvas, Some(kept));
                kept.painted(piece);
                bounds = Some(bounds.map_or(piece, |was| was.joined(piece)));
            }
            canvas.confine(None);
            match bounds {
                Some(bounds) => Repainted::Within(bounds),
                None => return Repainted::Nothing,
            }
        }
        Damage::Whole => {
            *canvas = Canvas::new(size.0, size.1, ground);
            draw(after, canvas, Some(kept));
            Repainted::Whole
        }
    };
    kept.end();
    repainted
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::DisplayItem;
    use crate::fill_rule::FillRule;
    use crate::paint::Paint;
    use crate::path::Path;
    use crate::render::render;
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
        let repainted = repaint(&before, &after, &mut canvas, Rgba::WHITE, &mut Kept::new());
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
        let repainted = repaint(&before, &after, &mut canvas, Rgba::WHITE, &mut Kept::new());
        assert!(matches!(repainted, Repainted::Within(_)), "{repainted:?}");
        assert_eq!(canvas, drawn(&after, (60, 50)));
    }

    #[test]
    fn the_same_list_paints_nothing_and_a_new_scale_paints_everything() {
        let one = list(vec![fill(2.0, 2.0, 4.0, 4.0, Rgba::BLACK)]);
        let mut canvas = drawn(&one, (8, 8));
        assert_eq!(
            repaint(&one, &one, &mut canvas, Rgba::WHITE, &mut Kept::new()),
            Repainted::Nothing
        );
        let two = DisplayList::from_parts(one.items().to_vec(), Vec::new(), 2.0);
        let mut grown = Canvas::new(16, 16, Rgba::WHITE);
        assert_eq!(
            repaint(&one, &two, &mut grown, Rgba::WHITE, &mut Kept::new()),
            Repainted::Whole
        );
        assert_eq!(grown, drawn(&two, (16, 16)));
        assert_eq!(Repainted::Whole.area((16, 16)), 256);
    }

    #[test]
    fn a_clip_whose_foot_moves_paints_again_only_the_strip_between_from_what_was_kept() {
        // A card's shadow and fill inside the conversation's clip, which ends
        // higher once the offers open (queue item 871): only the rows between
        // the two feet differ, and the shadow is not blurred again for them.
        let inside = |foot: f32| {
            list(vec![
                fill(0.0, 0.0, 120.0, 90.0, Rgba::from_rgba8(240, 238, 230, 255)),
                DisplayItem::PushClip {
                    box_id: BoxId::from_index_for_tests(0),
                    path: Path::rectangle(4.0, 4.0, 112.0, foot - 4.0),
                },
                shadow(20.0, 20.0),
                fill(20.0, 20.0, 30.0, 50.0, Rgba::WHITE),
                DisplayItem::PopClip,
            ])
        };
        let (open, closed) = (inside(52.5), inside(80.0));
        let mut kept = Kept::new();
        let mut canvas = Canvas::new(120, 90, Rgba::WHITE);
        crate::render::render_keeping(&closed, &mut canvas, &mut kept);
        assert_eq!(kept.made().blurred, 1);
        for (before, after) in [(&closed, &open), (&open, &closed)] {
            let repainted = repaint(before, after, &mut canvas, Rgba::WHITE, &mut kept);
            assert_eq!(
                canvas,
                drawn(after, (120, 80 + 10)),
                "every pixel as drawn whole"
            );
            let Repainted::Within(region) = repainted else {
                panic!("a clip's foot moving painted {repainted:?}");
            };
            assert!(
                region.top >= 52 && region.bottom <= 81,
                "only the rows between the feet: {region:?}"
            );
            assert_eq!(
                kept.made().blurred,
                0,
                "the shadow is taken as it was blurred"
            );
        }
    }

    /// A page of stripes, a box blurring what is behind it under a
    /// translucent white, and a mark drawn over the box: queue item 885.
    fn frosted(stripe: f32, mark: f32, beside: f32) -> DisplayList {
        let mut items = vec![fill(
            0.0,
            0.0,
            160.0,
            100.0,
            Rgba::from_rgba8(240, 238, 230, 255),
        )];
        for at in [10.0, 30.0, 50.0, 70.0] {
            items.push(fill(
                at,
                0.0,
                8.0,
                100.0,
                Rgba::from_rgba8(30, 60, 160, 255),
            ));
        }
        items.push(fill(
            stripe,
            40.0,
            6.0,
            30.0,
            Rgba::from_rgba8(200, 30, 30, 255),
        ));
        // Outside the box, as near it as its blur reaches: never read.
        items.push(fill(beside, 50.0, 4.0, 10.0, Rgba::BLACK));
        let rect = alo_layout::Rect::new(20.0, 20.0, 100.0, 60.0);
        let shape = crate::corner::rounded_rectangle(rect, crate::corner::Corners::all(16.0));
        items.push(DisplayItem::Backdrop {
            box_id: BoxId::from_index_for_tests(1),
            rect,
            path: shape.clone(),
            blur: 6.0,
        });
        items.push(DisplayItem::Fill {
            box_id: BoxId::from_index_for_tests(1),
            path: shape,
            paint: Paint::Solid(Rgba::from_rgba8(255, 255, 255, 178)),
            rule: FillRule::NonZero,
        });
        items.push(fill(mark, 60.0, 10.0, 6.0, Rgba::BLACK));
        list(items)
    }

    #[test]
    fn a_change_under_a_blurred_box_paints_its_whole_crop_again() {
        // Queue item 885: the red stripe moved under the box changes what the
        // blur reads, and so every pixel the box's backdrop draws — and so
        // does one only the list before had, taken away.
        let moved = frosted(48.0, 60.0, 124.0);
        let gone = list(
            frosted(40.0, 60.0, 124.0)
                .items()
                .iter()
                .filter(|item| {
                    !matches!(item, DisplayItem::Fill { paint: Paint::Solid(color), .. }
                        if *color == Rgba::from_rgba8(200, 30, 30, 255))
                })
                .cloned()
                .collect(),
        );
        for after in [moved, gone] {
            let before = frosted(40.0, 60.0, 124.0);
            let mut kept = Kept::new();
            let mut canvas = Canvas::new(160, 100, Rgba::WHITE);
            crate::render::render_keeping(&before, &mut canvas, &mut kept);
            let repainted = repaint(&before, &after, &mut canvas, Rgba::WHITE, &mut kept);
            assert_eq!(
                canvas,
                drawn(&after, (160, 100)),
                "every pixel as drawn whole"
            );
            let Repainted::Within(region) = repainted else {
                panic!("painted {repainted:?}");
            };
            assert!(
                region.left <= 20 && region.top <= 20 && region.right >= 120 && region.bottom >= 80,
                "the whole crop: {region:?}"
            );
            assert_eq!(kept.made().backdrops, 1, "blurred again");
        }
    }

    #[test]
    fn a_change_over_a_blurred_box_or_past_its_crop_takes_the_blur_kept() {
        // Queue item 885: what is drawn over the box, or beside it however
        // near, changes nothing the blur reads — it is taken as the last
        // drawing kept it, and only the change is painted again.
        let mut kept = Kept::new();
        let mut canvas = Canvas::new(160, 100, Rgba::WHITE);
        let first = frosted(40.0, 60.0, 124.0);
        crate::render::render_keeping(&first, &mut canvas, &mut kept);
        assert_eq!(kept.made().backdrops, 1);
        let over = frosted(40.0, 64.0, 124.0);
        let beside = frosted(40.0, 64.0, 128.0);
        for (before, after) in [(&first, &over), (&over, &beside), (&beside, &first)] {
            let repainted = repaint(before, after, &mut canvas, Rgba::WHITE, &mut kept);
            assert_eq!(
                canvas,
                drawn(after, (160, 100)),
                "every pixel as drawn whole"
            );
            let Repainted::Within(region) = repainted else {
                panic!("painted {repainted:?}");
            };
            assert!(region.area() < 2000, "only the changes: {region:?}");
            assert_eq!(kept.made().backdrops, 0, "the blur taken as kept");
        }
    }

    /// A page 1200 by 900 of stripes, a blurred box in its middle, and
    /// marks: `under` drawn before the box, `over` after it at its foot,
    /// `far` in the top corner — each at its x — for queue item 891.
    fn marked(under: f32, over: f32, far: f32) -> DisplayList {
        let mut items = vec![fill(
            0.0,
            0.0,
            1200.0,
            900.0,
            Rgba::from_rgba8(240, 238, 230, 255),
        )];
        for at in (0..12).map(|k| 40.0 + 100.0 * f32::from(u8::try_from(k).unwrap_or(0))) {
            items.push(fill(
                at,
                0.0,
                30.0,
                900.0,
                Rgba::from_rgba8(30, 60, 160, 255),
            ));
        }
        items.push(fill(
            under,
            400.0,
            20.0,
            20.0,
            Rgba::from_rgba8(200, 30, 30, 255),
        ));
        let rect = alo_layout::Rect::new(400.0, 300.0, 400.0, 300.0);
        let shape = crate::corner::rounded_rectangle(rect, crate::corner::Corners::all(16.0));
        items.push(DisplayItem::Backdrop {
            box_id: BoxId::from_index_for_tests(1),
            rect,
            path: shape.clone(),
            blur: 6.0,
        });
        items.push(DisplayItem::Fill {
            box_id: BoxId::from_index_for_tests(1),
            path: shape,
            paint: Paint::Solid(Rgba::from_rgba8(255, 255, 255, 178)),
            rule: FillRule::NonZero,
        });
        items.push(fill(over, 590.0, 30.0, 20.0, Rgba::BLACK));
        items.push(fill(
            far,
            40.0,
            12.0,
            12.0,
            Rgba::from_rgba8(20, 140, 40, 255),
        ));
        list(items)
    }

    #[test]
    fn far_changes_are_painted_as_pieces_each_as_drawn_whole() {
        // Queue item 891: a mark moved over the box's foot and one moved in
        // the far corner are two pieces, not the rectangle holding both;
        // nothing under the box changed, so its blur is taken as kept.
        let size = (1200, 900);
        let first = marked(100.0, 500.0, 1100.0);
        let mut kept = Kept::new();
        let mut canvas = Canvas::new(size.0, size.1, Rgba::WHITE);
        crate::render::render_keeping(&first, &mut canvas, &mut kept);
        let moved = marked(100.0, 520.0, 1130.0);
        let repainted = repaint(&first, &moved, &mut canvas, Rgba::WHITE, &mut kept);
        assert_eq!(canvas, drawn(&moved, size), "every pixel as drawn whole");
        assert!(matches!(repainted, Repainted::Within(_)), "{repainted:?}");
        let made = kept.made();
        assert_eq!((made.pieces, made.backdrops), (2, 0), "{made:?}");
        assert!(made.painted < 20_000, "only around the marks: {made:?}");
    }

    #[test]
    fn a_piece_that_meets_a_crop_read_afresh_holds_all_of_it() {
        // A mark moved under the box damages its whole crop; a mark over
        // its foot reaching past its edge is joined with it, and the far
        // one is a piece of its own — every pixel as drawn whole.
        let size = (1200, 900);
        let first = marked(100.0, 500.0, 1100.0);
        let mut kept = Kept::new();
        let mut canvas = Canvas::new(size.0, size.1, Rgba::WHITE);
        crate::render::render_keeping(&first, &mut canvas, &mut kept);
        let moved = marked(450.0, 785.0, 1130.0);
        for (was, after) in [(&first, &moved), (&moved, &first)] {
            repaint(was, after, &mut canvas, Rgba::WHITE, &mut kept);
            assert_eq!(canvas, drawn(after, size), "every pixel as drawn whole");
            let made = kept.made();
            assert_eq!((made.pieces, made.backdrops), (2, 1), "{made:?}");
            assert!(
                made.painted >= 400 * 2 * 300 * 2 / 4,
                "the whole crop and the far mark: {made:?}"
            );
        }
    }

    /// `list` without its blurred box, the backdrop and the fill over it.
    fn unboxed(list: &DisplayList) -> Vec<DisplayItem> {
        list.items()
            .iter()
            .filter(|item| item.box_id() != Some(BoxId::from_index_for_tests(1)))
            .cloned()
            .collect()
    }

    /// [`marked`] with or without its blurred box, and a bar down the page
    /// at `bar` drawn over everything.
    fn barred(boxed: bool, bar: f32) -> DisplayList {
        let page = marked(100.0, 500.0, 1100.0);
        let mut items = if boxed {
            page.items().to_vec()
        } else {
            unboxed(&page)
        };
        items.push(fill(
            bar,
            0.0,
            10.0,
            900.0,
            Rgba::from_rgba8(160, 20, 120, 255),
        ));
        list(items)
    }

    #[test]
    fn a_backdrop_drawn_anew_holds_its_whole_crop_in_one_piece() {
        // Queue item 894: a blurred box that only the list after has, and a
        // tall bar moved over its left edge — far too costly to join with
        // its crop by waste alone. The bar's piece meets the crop the blur
        // reads afresh, so it is joined with it: painted on its own after
        // the crop, it would blur pixels the crop's piece had already
        // finished.
        let size = (1200, 900);
        for (was, after) in [
            (barred(false, 1150.0), barred(true, 395.0)),
            (barred(false, 395.0), barred(true, 1150.0)),
        ] {
            let mut kept = Kept::new();
            let mut canvas = drawn(&was, size);
            let repainted = repaint(&was, &after, &mut canvas, Rgba::WHITE, &mut kept);
            assert!(matches!(repainted, Repainted::Within(_)), "{repainted:?}");
            assert_eq!(canvas, drawn(&after, size), "every pixel as drawn whole");
            assert_eq!(kept.made().backdrops, 1, "{:?}", kept.made());
        }
    }

    #[test]
    fn a_blur_closed_over_and_opened_again_is_taken_while_nothing_under_it_changed() {
        // Queue item 894: *Settings*' veil opened, closed and opened again.
        // Opened again over the very items it was blurred from, its blur is
        // taken as it was made; over any change under it, blurred anew —
        // every pixel, each time, as drawn whole.
        let size = (1200, 900);
        let open = |under: f32| marked(under, 500.0, 1100.0);
        let shut = |under: f32| list(unboxed(&open(under)));
        let mut kept = Kept::new();
        let mut canvas = Canvas::new(size.0, size.1, Rgba::WHITE);
        let mut last = DisplayList::default();
        for (after, blurred, alike) in [
            (open(100.0), 1, 0),
            (shut(100.0), 0, 0),
            (shut(100.0), 0, 0),
            (open(100.0), 0, 1),
            (shut(100.0), 0, 0),
            (shut(450.0), 0, 0),
            (open(450.0), 1, 0),
            (shut(450.0), 0, 0),
            (open(100.0), 1, 0),
        ] {
            repaint(&last, &after, &mut canvas, Rgba::WHITE, &mut kept);
            assert_eq!(canvas, drawn(&after, size), "every pixel as drawn whole");
            let made = kept.made();
            assert_eq!(
                (made.backdrops, made.blurs_alike),
                (blurred, alike),
                "{made:?}"
            );
            last = after;
        }
    }

    #[test]
    fn a_blur_not_kept_is_blurred_from_a_crop_painted_whole() {
        // With nothing kept, a change over the box paints the whole crop
        // again, for its blur reads all of it.
        let (before, after) = (frosted(40.0, 60.0, 124.0), frosted(40.0, 64.0, 124.0));
        let mut canvas = drawn(&before, (160, 100));
        let mut kept = Kept::new();
        let repainted = repaint(&before, &after, &mut canvas, Rgba::WHITE, &mut kept);
        assert_eq!(canvas, drawn(&after, (160, 100)));
        let Repainted::Within(region) = repainted else {
            panic!("painted {repainted:?}");
        };
        assert!(region.left <= 20 && region.right >= 120, "{region:?}");
    }
}
