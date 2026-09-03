//! A painted frame: what comes back when a renderer is asked for a picture.
//!
//! Eight bits a channel, which is where the float channels a canvas holds
//! become bytes. `alo_paint::Canvas` says why they are floats until then:
//! compositing multiplies and adds, and rounding on every blend is how a flat
//! colour turns into a slightly wrong one.
//!
//! A frame is the **one thing ADR 0005 lets processes share** — read-only
//! pixels, which is why it is bytes rather than a canvas: a canvas is ours,
//! and what crosses a boundary should be nothing but a rectangle of numbers.

use alo_paint::{Canvas, PictureError, to_png};

/// A rectangle of finished pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// How wide, in device pixels.
    pub width: u32,
    /// How tall.
    pub height: u32,
    /// Red, green, blue and alpha for every pixel, row by row from the top.
    pub pixels: Vec<u8>,
}

impl Frame {
    /// A canvas, finished.
    pub fn from_canvas(canvas: &Canvas) -> Self {
        let mut pixels = Vec::with_capacity(canvas.pixels().len().saturating_mul(4));
        for pixel in canvas.pixels() {
            let (red, green, blue, alpha) = pixel.to_rgba8();
            pixels.extend_from_slice(&[red, green, blue, alpha]);
        }
        Self {
            width: canvas.width(),
            height: canvas.height(),
            pixels,
        }
    }

    /// Whether it has no pixels at all.
    pub fn is_empty(&self) -> bool {
        self.pixels.is_empty()
    }

    /// One pixel, as red, green, blue and alpha.
    ///
    /// [`None`] for a pixel that is not on the frame, which is a real answer
    /// rather than a failure.
    pub fn at(&self, x: u32, y: u32) -> Option<(u8, u8, u8, u8)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        Some((
            *self.pixels.get(index)?,
            *self.pixels.get(index + 1)?,
            *self.pixels.get(index + 2)?,
            *self.pixels.get(index + 3)?,
        ))
    }
}

/// A frame as the bytes of a PNG, for a test that wants to look at it.
///
/// # Errors
///
/// [`PictureError::Unwritable`] if the encoder refuses it, which a frame of no
/// size does.
pub fn frame_to_png(frame: &Frame) -> Result<Vec<u8>, PictureError> {
    // Through a canvas, because `alo_paint::encode` is the only file allowed to
    // name `png` and this crate is not going to be the second one.
    let mut canvas = Canvas::new(frame.width, frame.height, alo_value::Rgba::TRANSPARENT);
    for y in 0..frame.height {
        for x in 0..frame.width {
            let Some((red, green, blue, alpha)) = frame.at(x, y) else {
                continue;
            };
            canvas.blend(
                x,
                y,
                alo_value::Rgba::from_rgba8(red, green, blue, 255),
                alpha,
            );
        }
    }
    to_png(&canvas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_value::Rgba;

    #[test]
    fn a_canvas_becomes_bytes_once() {
        let mut canvas = Canvas::new(2, 1, Rgba::WHITE);
        canvas.blend(0, 0, Rgba::BLACK, 255);
        let frame = Frame::from_canvas(&canvas);
        assert_eq!((frame.width, frame.height), (2, 1));
        assert_eq!(frame.pixels.len(), 8);
        assert_eq!(frame.at(0, 0), Some((0, 0, 0, 255)));
        assert_eq!(frame.at(1, 0), Some((255, 255, 255, 255)));
        assert!(!frame.is_empty());
    }

    #[test]
    fn asking_for_a_pixel_that_is_not_there_is_answered_with_nothing() {
        let frame = Frame::from_canvas(&Canvas::new(1, 1, Rgba::WHITE));
        assert_eq!(frame.at(1, 0), None);
        assert_eq!(frame.at(0, 1), None);
        assert_eq!(frame.at(u32::MAX, u32::MAX), None);
    }

    #[test]
    fn a_frame_of_no_size_is_empty_rather_than_a_failure() {
        let frame = Frame::from_canvas(&Canvas::new(0, 4, Rgba::WHITE));
        assert!(frame.is_empty());
        assert!(frame_to_png(&frame).is_err());
    }

    #[test]
    fn a_frame_can_be_looked_at() {
        let frame = Frame::from_canvas(&Canvas::new(3, 2, Rgba::WHITE));
        let png = frame_to_png(&frame).expect("a picture");
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']), "it is a PNG");
    }
}
