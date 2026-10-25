/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The ring around the box an agent's verb acted on (queue item 624).
//!
//! The clearest thing the window can say about how an agent works is that it
//! names a box rather than guessing at pixels — and the box knows where it
//! was drawn. So the ring is drawn on **the node's own rectangles**, read
//! from the same layout the frame under it was painted from, and nowhere
//! else: each CSS edge times the scale and rounded to the nearest device
//! pixel. The ring is drawn as alo draws focus (`ds/global.css`: `outline:
//! 2px solid var(--accent); outline-offset: 2px`) — [`RING_WIDTH`] CSS pixels
//! thick, [`RING_GAP`] outside the box's edge — so the page's own pixels show
//! between the two. Drawn on the edge itself it vanished on alo's own
//! buttons, which are filled with that same accent: the first reference
//! render showed a ringed "Try again" that nobody could tell was ringed. A
//! box broken across lines has a rectangle per
//! line (`SnapshotNode::rects`), and each is ringed, because their union
//! would ring text that belongs to somebody else.
//!
//! # Read to draw, never acted on
//!
//! ADR 0002: no verb takes a coordinate. These rectangles go one way — from
//! the page's tree to the window's pixels, after the verb has been decided,
//! carried out and answered — so a verb comes to the same thing, bit for
//! bit, whether anybody is watching or not. A press on the ring is the
//! page's: it is a mark over the page, not a control of the browser's.
//!
//! # As long as the frame it was measured against
//!
//! A verb runs inside one exchange with its renderer, and no frame can show
//! it until it has run; the first frame a person can see it on is the one
//! painted after it. The ring is drawn on that frame and goes with it: the
//! next frame of the tab, or another tab chosen, takes it away, because a
//! rectangle measured against one layout is a guess about the next.
//! [`crate::showing`] keeps that rule.
//!
//! It is clipped to the frame's place in the window: a box partly scrolled
//! away, or against the page's edge, is ringed where the ring falls on the
//! page, and the browser's own lines along the bottom are drawn over it
//! ([`crate::compose`]).

use crate::colours::mark;
use crate::place::PixelRect;
use crate::scale::Scale;
use alo_layout::Rect;
use alo_paint::Canvas;

/// How thick the ring is, in CSS pixels: alo's focus outline, two pixels of
/// `--accent`.
pub const RING_WIDTH: u32 = 2;

/// How far outside the box the ring is, in CSS pixels: alo's
/// `outline-offset`.
pub const RING_GAP: u32 = 2;

/// A rectangle of whole device pixels over a frame, from the frame's
/// top-left corner — which may reach past it on any side, as a box scrolled
/// partly away does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Marked {
    /// Its left edge.
    pub x: i32,
    /// Its top edge.
    pub y: i32,
    /// How wide.
    pub width: u32,
    /// How tall.
    pub height: u32,
}

/// The farthest a device pixel is put from a frame's corner: a box laid out
/// farther away than this is ringed as though it ended here, which no frame
/// reaches (`scale.rs`'s largest window is 2²³).
const FARTHEST: f32 = 16_777_216.0;

/// Each of a node's `rects`, in CSS pixels over the page's viewport, as the
/// device pixels of a frame painted at `scale`.
///
/// A rectangle with no area, or with an edge that is not a number, marks
/// nothing.
pub fn marked(rects: &[Rect], scale: f32) -> Vec<Marked> {
    rects
        .iter()
        .filter_map(|rect| {
            let left = device(rect.left(), scale)?;
            let top = device(rect.top(), scale)?;
            let right = device(rect.right(), scale)?;
            let bottom = device(rect.bottom(), scale)?;
            let width = u32::try_from(i64::from(right) - i64::from(left)).ok()?;
            let height = u32::try_from(i64::from(bottom) - i64::from(top)).ok()?;
            (width > 0 && height > 0).then_some(Marked {
                x: left,
                y: top,
                width,
                height,
            })
        })
        .collect()
}

/// A CSS edge at `scale`, rounded to the nearest device pixel.
fn device(css: f32, scale: f32) -> Option<i32> {
    let product = css * scale;
    if !product.is_finite() {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "rounded and clamped to ±2^24, which an i32 holds exactly"
    )]
    let rounded = product.round().clamp(-FARTHEST, FARTHEST) as i32;
    Some(rounded)
}

/// The pieces of the ring around `marked`, in the window's device pixels,
/// for a frame placed at `frame`: the top, the bottom, the left and the
/// right, in that order, each clipped to `frame` and left out where nothing
/// of it shows.
///
/// Its inner edge is [`RING_GAP`] CSS pixels outside the box's at `scale`
/// ([`Scale::device`]), and it is [`RING_WIDTH`] thick from there.
pub fn ring(marked: Marked, frame: PixelRect, scale: Scale) -> Vec<PixelRect> {
    let gap = i64::from(scale.device(RING_GAP));
    let thick = i64::from(scale.device(RING_WIDTH));
    let inner_left = i64::from(frame.x) + i64::from(marked.x) - gap;
    let inner_top = i64::from(frame.y) + i64::from(marked.y) - gap;
    let inner_right = inner_left + i64::from(marked.width) + 2 * gap;
    let inner_bottom = inner_top + i64::from(marked.height) + 2 * gap;
    let (left, top) = (inner_left - thick, inner_top - thick);
    let (right, bottom) = (inner_right + thick, inner_bottom + thick);
    [
        (left, top, right, inner_top),
        (left, inner_bottom, right, bottom),
        (left, inner_top, inner_left, inner_bottom),
        (inner_right, inner_top, right, inner_bottom),
    ]
    .into_iter()
    .filter_map(|piece| clipped(piece, frame))
    .collect()
}

/// `(left, top, right, bottom)` cut to `frame`, if anything is left.
fn clipped(
    (left, top, right, bottom): (i64, i64, i64, i64),
    frame: PixelRect,
) -> Option<PixelRect> {
    let left = left.max(i64::from(frame.x));
    let top = top.max(i64::from(frame.y));
    let right = right.min(i64::from(frame.x) + i64::from(frame.width));
    let bottom = bottom.min(i64::from(frame.y) + i64::from(frame.height));
    if right <= left || bottom <= top {
        return None;
    }
    Some(PixelRect {
        x: u32::try_from(left).ok()?,
        y: u32::try_from(top).ok()?,
        width: u32::try_from(right - left).ok()?,
        height: u32::try_from(bottom - top).ok()?,
    })
}

/// Draw the ring around each of `marked` over a frame placed at `frame` of
/// `canvas`.
pub fn draw(canvas: &mut Canvas, marked: &[Marked], frame: PixelRect, scale: Scale) {
    for piece in marked.iter().flat_map(|each| ring(*each, frame, scale)) {
        let (Ok(x), Ok(y)) = (i32::try_from(piece.x), i32::try_from(piece.y)) else {
            continue;
        };
        canvas.fill_rect(x, y, piece.width, piece.height, mark());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_value::Rgba;

    fn rect(x: u32, y: u32, width: u32, height: u32) -> PixelRect {
        PixelRect {
            x,
            y,
            width,
            height,
        }
    }

    fn at(x: i32, y: i32, width: u32, height: u32) -> Marked {
        Marked {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn a_rectangle_is_its_css_edges_times_the_scale_rounded() {
        let field = Rect::new(8.0, 30.0, 200.0, 21.0);
        assert_eq!(marked(&[field], 1.0), vec![at(8, 30, 200, 21)]);
        assert_eq!(marked(&[field], 2.0), vec![at(16, 60, 400, 42)]);
        // At 1.5 each edge is rounded on its own, a half away from zero: the
        // bottom, 76.5, to 77, so the box is 32 tall and not 31.5.
        assert_eq!(marked(&[field], 1.5), vec![at(12, 45, 300, 32)]);
        let quarter = Rect::new(0.25, -10.5, 10.0, 4.0);
        assert_eq!(marked(&[quarter], 2.0), vec![at(1, -21, 20, 8)]);
    }

    #[test]
    fn each_line_of_a_broken_box_is_its_own_and_nothing_empty_is_marked() {
        let lines = [
            Rect::new(300.0, 10.0, 80.0, 16.0),
            Rect::new(0.0, 0.0, 0.0, 16.0),
            Rect::new(0.0, 26.0, 40.0, 16.0),
            Rect::new(f32::NAN, 0.0, 4.0, 4.0),
            Rect::new(0.0, f32::INFINITY, 4.0, 4.0),
        ];
        assert_eq!(
            marked(&lines, 1.0),
            vec![at(300, 10, 80, 16), at(0, 26, 40, 16)]
        );
    }

    #[test]
    fn a_box_laid_out_beyond_any_frame_is_held_at_the_farthest_pixel() {
        let far = Rect::new(1.0e30, 0.0, 1.0e30, 4.0);
        assert_eq!(marked(&[far], 1.0), vec![]);
        let wide = Rect::new(-1.0e30, 0.0, 2.0e30, 4.0);
        assert_eq!(
            marked(&[wide], 1.0),
            vec![at(-16_777_216, 0, 33_554_432, 4)]
        );
    }

    #[test]
    fn the_ring_is_two_css_pixels_thick_two_outside_the_boxs_edge() {
        // A field at 8,30 200×21 in a frame placed below a band of 36: the
        // box is 8,66 to 208,87 in the window, the ring's inside 6,64 to
        // 210,89 and its outside 4,62 to 212,91.
        let frame = rect(0, 36, 480, 400);
        let pieces = ring(at(8, 30, 200, 21), frame, Scale::ONE);
        assert_eq!(
            pieces,
            vec![
                rect(4, 62, 208, 2),
                rect(4, 89, 208, 2),
                rect(4, 64, 2, 25),
                rect(210, 64, 2, 25),
            ]
        );
        // At two, four device pixels out and four thick; at 1.5, three.
        let doubled = ring(at(16, 60, 400, 42), rect(0, 72, 960, 800), Scale::TWO);
        assert_eq!(doubled[0], rect(8, 124, 416, 4));
        assert_eq!(doubled[3], rect(420, 128, 4, 50));
        let half = ring(
            at(12, 45, 300, 32),
            rect(0, 54, 720, 600),
            Scale::ONE_AND_A_HALF,
        );
        assert_eq!(half[0], rect(6, 93, 312, 3));
        assert_eq!(half[2], rect(6, 96, 3, 38));
    }

    #[test]
    fn a_box_partly_scrolled_away_is_ringed_only_where_the_frame_shows_it() {
        let frame = rect(0, 36, 100, 50);
        // Above the frame's top: no top piece, and the sides start at it.
        let pieces = ring(at(10, -10, 20, 20), frame, Scale::ONE);
        assert_eq!(
            pieces,
            vec![rect(6, 48, 28, 2), rect(6, 36, 2, 12), rect(32, 36, 2, 12)]
        );
        // Just past the frame's right edge, its ring still reaches onto the
        // page; farther, nothing of it does.
        assert_eq!(
            ring(at(100, 0, 10, 10), frame, Scale::ONE),
            vec![rect(96, 48, 4, 2), rect(96, 36, 2, 12)]
        );
        assert_eq!(ring(at(104, 0, 10, 10), frame, Scale::ONE), vec![]);
        assert_eq!(ring(at(0, 54, 10, 10), frame, Scale::ONE), vec![]);
    }

    #[test]
    fn drawn_it_is_the_accent_on_the_ring_and_the_page_in_the_gap() {
        let mut canvas = Canvas::new(40, 40, Rgba::WHITE);
        let frame = rect(0, 10, 40, 30);
        // The box is 6,16 to 16,24 in the window; the ring 2 out, 2 thick.
        draw(&mut canvas, &[at(6, 6, 10, 8)], frame, Scale::ONE);
        let accent = Some(mark().to_rgba8());
        let white = Some(Rgba::WHITE.to_rgba8());
        let pixel = |x, y| canvas.at(x, y).map(Rgba::to_rgba8);
        for (x, y) in [(2, 12), (19, 12), (2, 27), (19, 27), (3, 13), (18, 26)] {
            assert_eq!(pixel(x, y), accent, "ring at {x},{y}");
        }
        // The gap, the box's own edge and inside, and past the ring.
        for (x, y) in [
            (4, 14),
            (5, 15),
            (6, 16),
            (15, 23),
            (10, 20),
            (1, 12),
            (20, 12),
            (2, 11),
            (2, 28),
        ] {
            assert_eq!(pixel(x, y), white, "not the ring at {x},{y}");
        }
    }
}
