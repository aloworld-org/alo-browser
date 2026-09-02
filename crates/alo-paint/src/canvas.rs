//! Pixels, and putting coverage onto them.
//!
//! A canvas holds colour as **floats**, for the reason
//! `alo_value::Rgba` gives: compositing multiplies and adds channels, and
//! doing that in eight bits loses a little every time. A page draws a
//! background, then a border over it, then text over that; three roundings on
//! every pixel is how a flat colour turns into a slightly wrong one. The eight
//! bits arrive once, when the picture is written out.

use alo_value::Rgba;

/// A rectangle of pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<Rgba>,
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
        let Some(index) = self.index(x, y) else {
            return;
        };
        let Some(under) = self.pixels.get(index).copied() else {
            return;
        };
        let over = Rgba {
            alpha: color.alpha * (f32::from(coverage) / 255.0),
            ..color
        };
        if let Some(pixel) = self.pixels.get_mut(index) {
            *pixel = over.over(under);
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
        for row in 0..height {
            for column in 0..width {
                let (Some(x), Some(y)) = (offset(left, column), offset(top, row)) else {
                    continue;
                };
                self.blend(x, y, color, 255);
            }
        }
    }

    fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some((y as usize) * (self.width as usize) + (x as usize))
    }
}

/// A position on the canvas, or [`None`] when it is off the left or top edge.
fn offset(base: i32, step: u32) -> Option<u32> {
    let step = i32::try_from(step).ok()?;
    u32::try_from(base.checked_add(step)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
