//! Pictures, written out and read back.
//!
//! **This is the only file that names `png`.** A PNG is a specification with a
//! compressor in it, which ADR 0001 says to rent.
//!
//! A picture is written for one reason: `CLAUDE.md` asks for a **reference
//! render** for anything visual — a small deterministic raster, committed, so
//! that a change which moves a pixel says so. Reading one back is the other
//! half of that, and it is why decoding is here as well as encoding.

use crate::canvas::Canvas;
use alo_value::Rgba;
use core::fmt;

/// Something that went wrong reading or writing a picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PictureError {
    /// The bytes are not a picture this engine can read.
    Unreadable(String),
    /// The picture could not be written.
    Unwritable(String),
}

impl fmt::Display for PictureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PictureError::Unreadable(why) => write!(f, "not a picture we can read: {why}"),
            PictureError::Unwritable(why) => write!(f, "could not write the picture: {why}"),
        }
    }
}

impl std::error::Error for PictureError {}

/// A canvas as the bytes of a PNG.
///
/// Eight bits a channel, which is where the float channels the canvas holds
/// finally become bytes — once, at the end, rather than on every blend.
///
/// # Errors
///
/// [`PictureError::Unwritable`] if the encoder refuses the canvas, which a
/// canvas of no size does.
pub fn to_png(canvas: &Canvas) -> Result<Vec<u8>, PictureError> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, canvas.width(), canvas.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| PictureError::Unwritable(error.to_string()))?;

        let mut data = Vec::with_capacity(canvas.pixels().len() * 4);
        for pixel in canvas.pixels() {
            let (red, green, blue, alpha) = pixel.to_rgba8();
            data.extend_from_slice(&[red, green, blue, alpha]);
        }
        writer
            .write_image_data(&data)
            .map_err(|error| PictureError::Unwritable(error.to_string()))?;
    }
    Ok(bytes)
}

/// A canvas from the bytes of a PNG.
///
/// # Errors
///
/// [`PictureError::Unreadable`] for bytes that are not a PNG, or are one this
/// engine cannot read.
pub fn from_png(bytes: &[u8]) -> Result<Canvas, PictureError> {
    // The decoder wants something it can seek in; a slice of bytes becomes
    // that by wrapping it in a cursor.
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|error| PictureError::Unreadable(error.to_string()))?;
    let mut buffer = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| PictureError::Unreadable(error.to_string()))?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(PictureError::Unreadable(
            "only eight-bit RGBA is read here, which is all this engine writes".to_owned(),
        ));
    }

    let mut canvas = Canvas::new(info.width, info.height, Rgba::TRANSPARENT);
    for y in 0..info.height {
        for x in 0..info.width {
            let at = ((y as usize) * (info.width as usize) + (x as usize)) * 4;
            let Some(pixel) = buffer.get(at..at + 4) else {
                continue;
            };
            let (Some(red), Some(green), Some(blue), Some(alpha)) = (
                pixel.first().copied(),
                pixel.get(1).copied(),
                pixel.get(2).copied(),
                pixel.get(3).copied(),
            ) else {
                continue;
            };
            canvas.blend(x, y, Rgba::from_rgba8(red, green, blue, alpha), 255);
        }
    }
    Ok(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_canvas_written_out_and_read_back_is_the_same_canvas() {
        let mut canvas = Canvas::new(4, 3, Rgba::WHITE);
        canvas.fill_rect(1, 1, 2, 1, Rgba::BLACK);
        canvas.blend(0, 0, Rgba::new(1.0, 0.0, 0.0, 1.0), 255);

        let bytes = to_png(&canvas).expect("a canvas with pixels in it");
        let read = from_png(&bytes).expect("what we just wrote");

        assert_eq!((read.width(), read.height()), (4, 3));
        for y in 0..3 {
            for x in 0..4 {
                assert_eq!(
                    read.at(x, y).map(Rgba::to_rgba8),
                    canvas.at(x, y).map(Rgba::to_rgba8),
                    "pixel {x},{y}",
                );
            }
        }
    }

    #[test]
    fn the_same_canvas_writes_the_same_bytes_every_time() {
        // The whole point of a reference render: a picture that has not
        // changed must not produce a file that has.
        let canvas = Canvas::new(8, 8, Rgba::WHITE);
        assert_eq!(
            to_png(&canvas).expect("a canvas"),
            to_png(&canvas).expect("a canvas"),
        );
    }

    #[test]
    fn a_canvas_of_no_size_cannot_be_written_and_says_so() {
        let error = to_png(&Canvas::new(0, 0, Rgba::WHITE)).expect_err("no pixels");
        assert!(matches!(error, PictureError::Unwritable(_)));
        assert!(error.to_string().starts_with("could not write"));
    }

    #[test]
    fn bytes_that_are_not_a_picture_are_refused_with_a_reason() {
        let error = from_png(b"not a png at all").expect_err("not a picture");
        assert!(matches!(error, PictureError::Unreadable(_)));
        assert!(error.to_string().starts_with("not a picture we can read"));
    }

    #[test]
    fn transparency_survives_the_round_trip() {
        let mut canvas = Canvas::new(2, 1, Rgba::TRANSPARENT);
        canvas.blend(0, 0, Rgba::new(0.0, 0.0, 1.0, 0.5), 255);
        let read = from_png(&to_png(&canvas).expect("a canvas")).expect("what we wrote");
        assert_eq!(read.at(0, 0).map(Rgba::to_rgba8), Some((0, 0, 255, 128)),);
        assert_eq!(read.at(1, 0).map(Rgba::to_rgba8), Some((0, 0, 0, 0)));
    }
}
