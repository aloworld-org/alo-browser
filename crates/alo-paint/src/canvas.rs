/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Pixels, and putting coverage onto them.
//!
//! A canvas holds colour as **floats**, for the reason
//! `alo_value::Rgba` gives: compositing multiplies and adds channels, and
//! doing that in eight bits loses a little every time. A page draws a
//! background, then a border over it, then text over that; three roundings on
//! every pixel is how a flat colour turns into a slightly wrong one. The eight
//! bits arrive once, when the picture is written out.
//!
//! That is also what Chrome 154's default raster path draws: measured on 36
//! translucent fills (queue item 436, corpus case `translucent-fills`), it
//! agrees with this canvas wherever the blend is not exactly a half. Its
//! software path blends in eight bits and is up to a unit darker, which is
//! an approximation for speed rather than a different answer.

use crate::region::Region;
use alo_value::Rgba;

/// A rectangle of pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<Rgba>,
    /// The only pixels anything drawn onto it may change, while a list is
    /// painted again over the drawing it holds ([`crate::repaint`], queue
    /// item 870); [`None`], every pixel.
    confined: Option<Region>,
}

impl Canvas {
    /// A canvas of a size, filled with a colour.
    ///
    /// Filling rather than starting transparent is deliberate: a page has a
    /// background, and a picture that started transparent would have to
    /// remember to draw one.
    pub fn new(width: u32, height: u32, background: Rgba) -> Self {
        let count = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            pixels: vec![background; count],
            confined: None,
        }
    }

    /// Change only the pixels of `region` from now on, whatever is drawn
    /// onto it, or every pixel again for [`None`] (queue item 870).
    ///
    /// Every pixel is changed only through [`Self::blend`],
    /// [`Self::replace`], [`Self::fill_rect`] and [`Self::draw_over`], and
    /// each asks this first. So a list drawn onto a confined canvas leaves
    /// every pixel outside the region exactly as it was, and puts every
    /// pixel inside it exactly as drawing the list onto the whole canvas
    /// would have: nothing drawn here reads one pixel to write another.
    pub fn confine(&mut self, region: Option<Region>) {
        self.confined = region;
    }

    /// The pixels it is confined to, if it is.
    pub fn confinement(&self) -> Option<Region> {
        self.confined
    }

    /// Every pixel of `region` put back to `color`, as a canvas made in it
    /// starts: where a list is painted again from nothing.
    pub fn reset(&mut self, region: Region, color: Rgba) {
        for y in region.top..region.bottom.min(self.height) {
            for x in region.left..region.right.min(self.width) {
                if let Some(index) = self.index(x, y)
                    && let Some(pixel) = self.pixels.get_mut(index)
                {
                    *pixel = color;
                }
            }
        }
    }

    /// How wide it is.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// How tall it is.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Whether it has no pixels at all.
    pub fn is_empty(&self) -> bool {
        self.pixels.is_empty()
    }

    /// The colour of one pixel, or [`None`] for a pixel that is not on the
    /// canvas.
    pub fn at(&self, x: u32, y: u32) -> Option<Rgba> {
        self.pixels.get(self.index(x, y)?).copied()
    }

    /// Every pixel, row by row from the top.
    pub fn pixels(&self) -> &[Rgba] {
        &self.pixels
    }

    /// Draw a colour over one pixel, at a coverage.
    ///
    /// Coverage multiplies the colour's own alpha rather than replacing it, so
    /// a half-transparent colour at half coverage is a quarter — which is what
    /// anti-aliased text on a translucent background has to be.
    pub fn blend(&mut self, x: u32, y: u32, color: Rgba, coverage: u8) {
        if coverage == 0 {
            return;
        }
        let Some(index) = self.writable(x, y) else {
            return;
        };
        let Some(pixel) = self.pixels.get_mut(index) else {
            return;
        };
        *pixel = blended(color, coverage, *pixel);
    }

    /// Draw a colour over a run of pixels of row `y` from column `left`, one
    /// coverage each, each also multiplied by what `inside` lets through at
    /// its column: [`Self::blend`] at each pixel of the run, in one pass
    /// along the row rather than a lookup per pixel (queue item 892). A
    /// pixel off the canvas, or outside its confinement, is passed over.
    pub(crate) fn blend_run(
        &mut self,
        (left, y): (u32, u32),
        coverages: &[u8],
        color: Rgba,
        inside: impl Fn(u32) -> u8,
    ) {
        if y >= self.height
            || self
                .confined
                .is_some_and(|region| !(region.top..region.bottom).contains(&y))
        {
            return;
        }
        let (from, to) = self
            .confined
            .map_or((0, self.width), |region| (region.left, region.right));
        let length = u32::try_from(coverages.len()).unwrap_or(u32::MAX);
        let start = left.max(from);
        let end = left.saturating_add(length).min(to).min(self.width);
        if start >= end {
            return;
        }
        let row = (y as usize) * (self.width as usize);
        let (Some(pixels), Some(values)) = (
            self.pixels
                .get_mut(row + start as usize..row + end as usize),
            coverages.get((start - left) as usize..(end - left) as usize),
        ) else {
            return;
        };
        for ((pixel, &value), x) in pixels.iter_mut().zip(values).zip(start..end) {
            if value == 0 {
                continue;
            }
            let through = inside(x);
            if through == 0 {
                continue;
            }
            let combined = crate::clip::multiply(value, through);
            if combined == 0 {
                continue;
            }
            *pixel = blended(color, combined, *pixel);
        }
    }

    /// [`Self::replace`] at each of `length` pixels of row `y` from column
    /// `left`, along the row (queue item 892): `made` is told each pixel's
    /// column and what it holds. A pixel off the canvas, or outside its
    /// confinement, is passed over.
    pub(crate) fn replace_run(
        &mut self,
        (left, y): (u32, u32),
        length: u32,
        mut made: impl FnMut(u32, Rgba) -> Rgba,
    ) {
        if y >= self.height
            || self
                .confined
                .is_some_and(|region| !(region.top..region.bottom).contains(&y))
        {
            return;
        }
        let (from, to) = self
            .confined
            .map_or((0, self.width), |region| (region.left, region.right));
        let start = left.max(from);
        let end = left.saturating_add(length).min(to).min(self.width);
        let row = (y as usize) * (self.width as usize);
        let Some(pixels) = self
            .pixels
            .get_mut(row + start as usize..row + end.max(start) as usize)
        else {
            return;
        };
        for (pixel, x) in pixels.iter_mut().zip(start..end) {
            *pixel = made(x, *pixel);
        }
    }

    /// Put a pixel in place of one, made from the pixel it replaces: for a
    /// drawing whose arithmetic onto what is beneath is its own rather than
    /// [`Rgba::over`]'s, as a smoothed picture's is
    /// ([`crate::mix_onto`]). A pixel off the canvas is passed over.
    pub fn replace(&mut self, x: u32, y: u32, made: impl FnOnce(Rgba) -> Rgba) {
        let Some(index) = self.writable(x, y) else {
            return;
        };
        if let Some(pixel) = self.pixels.get_mut(index) {
            *pixel = made(*pixel);
        }
    }

    /// Fill a whole rectangle of pixels with a colour.
    ///
    /// The rectangle is in whole pixels and is clipped to the canvas; a
    /// rectangle entirely off the canvas draws nothing rather than failing.
    pub fn fill_rect(&mut self, left: i32, top: i32, width: u32, height: u32, color: Rgba) {
        if color.is_invisible() {
            return;
        }
        // Only the rows and columns a confined canvas would keep (queue item
        // 872): the others would be refused pixel by pixel all the same.
        let (rows, columns) = match self.confined {
            Some(region) => (
                steps(top, height, (region.top, region.bottom)),
                steps(left, width, (region.left, region.right)),
            ),
            None => (0..height, 0..width),
        };
        for row in rows {
            for column in columns.clone() {
                let (Some(x), Some(y)) = (offset(left, column), offset(top, row)) else {
                    continue;
                };
                self.blend(x, y, color, 255);
            }
        }
    }

    /// Draw another canvas over this one, at an opacity.
    ///
    /// This is what `opacity` means and why it needs a canvas of its own: a
    /// half-transparent group is drawn **once**, whole, at half strength.
    /// Drawing each of its boxes at half strength separately would show every
    /// box through every other one, which is the classic wrong picture.
    pub fn draw_over(&mut self, other: &Canvas, opacity: f32) {
        let opacity = opacity.clamp(0.0, 1.0);
        if opacity <= 0.0 {
            return;
        }
        let (width, height) = (self.width.min(other.width), self.height.min(other.height));
        let Some(within) = Region::whole(width, height) else {
            return;
        };
        let within = match self.confined {
            Some(confined) => match confined.meeting(within) {
                Some(both) => both,
                None => return,
            },
            None => within,
        };
        for y in within.top..within.bottom {
            for x in within.left..within.right {
                let Some(over) = other.at(x, y) else {
                    continue;
                };
                if over.alpha <= 0.0 {
                    continue;
                }
                let Some(index) = self.writable(x, y) else {
                    continue;
                };
                let Some(under) = self.pixels.get(index).copied() else {
                    continue;
                };
                let faded = Rgba {
                    alpha: over.alpha * opacity,
                    ..over
                };
                if let Some(pixel) = self.pixels.get_mut(index) {
                    *pixel = faded.over(under);
                }
            }
        }
    }

    /// Where a pixel is held, if it is on the canvas and drawing may change
    /// it.
    fn writable(&self, x: u32, y: u32) -> Option<usize> {
        if self.confined.is_some_and(|region| !region.holds(x, y)) {
            return None;
        }
        self.index(x, y)
    }

    fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some((y as usize) * (self.width as usize) + (x as usize))
    }
}

/// `color` drawn over `under` at `coverage`, as [`Canvas::blend`] draws it.
///
/// An opaque colour wholly covering a pixel is that colour (queue item
/// 892): [`Rgba::over`] keeps `under` at `1 − 1`, which is nothing, so its
/// alpha is `1 + a·0` and each channel `(c·1 + b·a·0) / 1`, the colour's own
/// — for every `under` whose alpha and each `b·a` are numbers, which every
/// pixel a drawing makes is. Written without the arithmetic: a page's background,
/// its cards and its panel are mostly such pixels.
/// `an_opaque_colour_wholly_covering_is_itself_every_bit` holds the two
/// alike.
fn blended(color: Rgba, coverage: u8, under: Rgba) -> Rgba {
    if coverage == u8::MAX
        && color.alpha.to_bits() == 1.0_f32.to_bits()
        && under.alpha.is_finite()
        && [under.red, under.green, under.blue]
            .iter()
            .all(|channel| (channel * under.alpha).is_finite())
    {
        return color;
    }
    over_at(color, coverage, under)
}

/// `color` over `under` at `coverage`, by [`Rgba::over`].
fn over_at(color: Rgba, coverage: u8, under: Rgba) -> Rgba {
    let over = Rgba {
        alpha: color.alpha * (f32::from(coverage) / 255.0),
        ..color
    };
    over.over(under)
}

/// A position on the canvas, or [`None`] when it is off the left or top edge.
fn offset(base: i32, step: u32) -> Option<u32> {
    let step = i32::try_from(step).ok()?;
    u32::try_from(base.checked_add(step)?).ok()
}

/// The steps of a run `length` long from `base` that land between `from`
/// and `to`.
fn steps(base: i32, length: u32, (from, to): (u32, u32)) -> core::ops::Range<u32> {
    let clamp = |at: u32| {
        let step = (i64::from(at) - i64::from(base)).clamp(0, i64::from(length));
        u32::try_from(step).unwrap_or(0)
    };
    clamp(from)..clamp(to).max(clamp(from))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_is_blended_pixel_by_pixel_and_only_where_the_canvas_is_confined() {
        // Queue item 892: a run along a row is `blend` at each of its pixels,
        // and a confined canvas refuses the run's pixels outside, as it
        // refuses `blend`'s.
        let values: Vec<u8> = (0..30).map(|k| [0, 40, 128, 255, 255][k % 5]).collect();
        let colour = Rgba::from_rgba8(200, 30, 30, 255);
        let through = |x: u32| [255, 255, 90, 0][(x % 4) as usize];
        for confined in [None, Region::new(6, 2, 19, 5)] {
            for (left, y) in [(0, 3), (4, 3), (10, 1), (25, 3), (3, 9)] {
                let mut run = Canvas::new(32, 8, Rgba::from_rgba8(10, 120, 40, 200));
                run.confine(confined);
                let mut each = run.clone();
                run.blend_run((left, y), &values, colour, through);
                for (offset, value) in (0_u32..).zip(&values) {
                    let x = left + offset;
                    each.blend(x, y, colour, crate::clip::multiply(*value, through(x)));
                }
                assert_eq!(run, each, "{confined:?} from ({left}, {y})");
            }
        }
    }

    #[test]
    fn a_run_replaced_is_each_pixel_replaced_and_only_where_confined() {
        let made = |x: u32, under: Rgba| Rgba {
            red: under.red * 0.5,
            green: f32::from(u8::try_from(x % 7).unwrap_or(0)) / 7.0,
            ..under
        };
        for confined in [None, Region::new(6, 2, 19, 5)] {
            for (left, y) in [(0, 3), (4, 3), (10, 1), (25, 3), (3, 9)] {
                let mut run = Canvas::new(32, 8, Rgba::from_rgba8(10, 120, 40, 200));
                run.confine(confined);
                let mut each = run.clone();
                run.replace_run((left, y), 12, made);
                for x in left..left + 12 {
                    each.replace(x, y, |under| made(x, under));
                }
                assert_eq!(run, each, "{confined:?} from ({left}, {y})");
            }
        }
    }

    #[test]
    fn an_opaque_colour_wholly_covering_is_itself_every_bit() {
        // Queue item 892: over every level of every channel beneath, at
        // every alpha beneath, and over floats no level is — what `over`
        // makes of an opaque colour at full coverage is the colour.
        let mut state = 892_u32;
        let mut next = move || {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            f32::from(u16::try_from((state >> 8) & 0xffff).unwrap_or(0)) / 65_535.0
        };
        for level in 0..=u8::MAX {
            for alpha in 0..=u8::MAX {
                let colour = Rgba::from_rgba8(level, u8::MAX - level, alpha, u8::MAX);
                let under = Rgba::from_rgba8(alpha, level, level / 2, alpha);
                assert_eq!(
                    blended(colour, u8::MAX, under),
                    over_at(colour, u8::MAX, under)
                );
                let odd = Rgba {
                    red: next(),
                    green: next(),
                    blue: next(),
                    alpha: next(),
                };
                let made = blended(colour, u8::MAX, odd);
                let worked = over_at(colour, u8::MAX, odd);
                assert_eq!(made, worked, "{colour} over {odd:?}");
                assert_eq!(
                    [made.red, made.green, made.blue, made.alpha].map(f32::to_bits),
                    [worked.red, worked.green, worked.blue, worked.alpha].map(f32::to_bits),
                );
            }
        }
        // Not a number beneath, or not wholly covered, or not opaque: worked
        // out as before.
        let opaque = Rgba::from_rgba8(10, 20, 30, 255);
        let broken = Rgba {
            alpha: f32::NAN,
            ..Rgba::WHITE
        };
        assert!(blended(opaque, u8::MAX, broken).alpha.is_nan());
        let under = Rgba::from_rgba8(200, 100, 50, 255);
        assert_eq!(blended(opaque, 254, under), over_at(opaque, 254, under));
        let clear = Rgba::from_rgba8(10, 20, 30, 254);
        assert_eq!(
            blended(clear, u8::MAX, under),
            over_at(clear, u8::MAX, under)
        );
    }

    fn close(left: Rgba, right: Rgba) -> bool {
        (left.red - right.red).abs() < 0.01
            && (left.green - right.green).abs() < 0.01
            && (left.blue - right.blue).abs() < 0.01
            && (left.alpha - right.alpha).abs() < 0.01
    }

    #[test]
    fn a_new_canvas_is_the_colour_it_was_made_with() {
        let canvas = Canvas::new(3, 2, Rgba::WHITE);
        assert_eq!((canvas.width(), canvas.height()), (3, 2));
        assert_eq!(canvas.pixels().len(), 6);
        assert!(!canvas.is_empty());
        for y in 0..2 {
            for x in 0..3 {
                assert_eq!(canvas.at(x, y), Some(Rgba::WHITE));
            }
        }
    }

    #[test]
    fn a_canvas_of_no_size_is_empty_rather_than_a_failure() {
        let canvas = Canvas::new(0, 10, Rgba::WHITE);
        assert!(canvas.is_empty());
        assert_eq!(canvas.at(0, 0), None);
    }

    #[test]
    fn asking_for_a_pixel_that_is_not_there_is_answered_with_nothing() {
        let canvas = Canvas::new(2, 2, Rgba::WHITE);
        assert_eq!(canvas.at(2, 0), None);
        assert_eq!(canvas.at(0, 2), None);
        assert_eq!(canvas.at(u32::MAX, u32::MAX), None);
    }

    #[test]
    fn full_coverage_replaces_a_pixel_and_no_coverage_leaves_it() {
        let mut canvas = Canvas::new(1, 1, Rgba::WHITE);
        canvas.blend(0, 0, Rgba::BLACK, 0);
        assert_eq!(canvas.at(0, 0), Some(Rgba::WHITE));

        canvas.blend(0, 0, Rgba::BLACK, 255);
        assert_eq!(canvas.at(0, 0), Some(Rgba::BLACK));
    }

    #[test]
    fn half_coverage_is_half_way_between_the_two_colours() {
        let mut canvas = Canvas::new(1, 1, Rgba::WHITE);
        canvas.blend(0, 0, Rgba::BLACK, 128);
        let blended = canvas.at(0, 0).expect("a pixel");
        assert!(
            close(blended, Rgba::new(0.498, 0.498, 0.498, 1.0)),
            "expected a mid grey, got {blended}",
        );
    }

    #[test]
    fn coverage_multiplies_the_colours_own_alpha_rather_than_replacing_it() {
        let mut canvas = Canvas::new(1, 1, Rgba::WHITE);
        let half_black = Rgba::new(0.0, 0.0, 0.0, 0.5);
        canvas.blend(0, 0, half_black, 128);
        let blended = canvas.at(0, 0).expect("a pixel");
        assert!(
            close(blended, Rgba::new(0.749, 0.749, 0.749, 1.0)),
            "half a colour at half coverage is a quarter, got {blended}",
        );
    }

    #[test]
    fn drawing_off_the_canvas_draws_nothing_and_does_not_mind() {
        let mut canvas = Canvas::new(2, 2, Rgba::WHITE);
        canvas.blend(5, 5, Rgba::BLACK, 255);
        canvas.fill_rect(-10, -10, 3, 3, Rgba::BLACK);
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(canvas.at(x, y), Some(Rgba::WHITE), "{x},{y}");
            }
        }
    }

    #[test]
    fn a_rectangle_fills_the_pixels_it_covers_and_no_others() {
        let mut canvas = Canvas::new(4, 4, Rgba::WHITE);
        canvas.fill_rect(1, 1, 2, 2, Rgba::BLACK);
        for y in 0..4 {
            for x in 0..4 {
                let inside = (1..3).contains(&x) && (1..3).contains(&y);
                let expected = if inside { Rgba::BLACK } else { Rgba::WHITE };
                assert_eq!(canvas.at(x, y), Some(expected), "{x},{y}");
            }
        }
    }

    #[test]
    fn a_rectangle_that_hangs_off_the_edge_draws_the_part_that_is_on() {
        let mut canvas = Canvas::new(2, 2, Rgba::WHITE);
        canvas.fill_rect(1, 1, 5, 5, Rgba::BLACK);
        assert_eq!(canvas.at(0, 0), Some(Rgba::WHITE));
        assert_eq!(canvas.at(1, 1), Some(Rgba::BLACK));
    }

    #[test]
    fn an_invisible_colour_draws_nothing() {
        let mut canvas = Canvas::new(2, 2, Rgba::WHITE);
        canvas.fill_rect(0, 0, 2, 2, Rgba::TRANSPARENT);
        assert_eq!(canvas.at(0, 0), Some(Rgba::WHITE));
    }
    #[test]
    fn a_group_drawn_over_at_half_opacity_is_half_way() {
        let mut page = Canvas::new(1, 1, Rgba::WHITE);
        let mut group = Canvas::new(1, 1, Rgba::TRANSPARENT);
        group.blend(0, 0, Rgba::BLACK, 255);
        page.draw_over(&group, 0.5);
        let blended = page.at(0, 0).expect("a pixel");
        assert!(close(blended, Rgba::new(0.5, 0.5, 0.5, 1.0)), "{blended}");
    }

    #[test]
    fn two_boxes_in_one_group_do_not_show_through_each_other() {
        // The whole point of a group: black over black at half opacity is one
        // half-black, not two.
        let mut group = Canvas::new(1, 1, Rgba::TRANSPARENT);
        group.blend(0, 0, Rgba::BLACK, 255);
        group.blend(0, 0, Rgba::BLACK, 255);
        let mut page = Canvas::new(1, 1, Rgba::WHITE);
        page.draw_over(&group, 0.5);
        let blended = page.at(0, 0).expect("a pixel");
        assert!(close(blended, Rgba::new(0.5, 0.5, 0.5, 1.0)), "{blended}");
    }

    #[test]
    fn a_group_at_no_opacity_draws_nothing_and_at_full_draws_all_of_it() {
        let mut group = Canvas::new(1, 1, Rgba::TRANSPARENT);
        group.blend(0, 0, Rgba::BLACK, 255);

        let mut page = Canvas::new(1, 1, Rgba::WHITE);
        page.draw_over(&group, 0.0);
        assert_eq!(page.at(0, 0), Some(Rgba::WHITE));

        page.draw_over(&group, 1.0);
        assert_eq!(page.at(0, 0), Some(Rgba::BLACK));
    }
}
