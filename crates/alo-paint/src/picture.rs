//! Reading a picture a page sent, in whatever format it turns out to be.
//!
//! **This is the only file that names `jpeg_decoder`.** PNG lives in
//! [`crate::encode`], because that file already rented `png` for reference
//! renders and a rented crate belongs to one file (ADR 0001).
//!
//! # The format comes from the bytes, not from the name
//!
//! A `src` ending in `.png` proves nothing: it is a string on a page, and the
//! server that answered may have sent something else — by mistake, or on
//! purpose. So the format is decided by what the bytes begin with, which is the
//! only thing that cannot be lied about without also being true.
//!
//! # Every format, the same bounds
//!
//! The reason JPEG and PNG are one item and not two: a second decoder with its
//! own limits, or none, would be a second way in. Both go through
//! [`MOST_PIXELS`], both refuse a picture of no size, and both are checked
//! **before** the allocation rather than after.

use crate::canvas::Canvas;
use crate::encode::{MOST_PIXELS, PictureError, picture_from_png};
use alo_value::Rgba;

/// What a run of bytes turns out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// A PNG.
    Png,
    /// A JPEG.
    Jpeg,
}

impl Format {
    /// What these bytes are, by what they begin with.
    ///
    /// [`None`] for anything this engine does not read, which is a refusal
    /// rather than an attempt: a decoder handed the wrong format will either
    /// fail confusingly or, worse, find something in it.
    pub fn of(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
            return Some(Format::Png);
        }
        // Every JPEG begins with a start-of-image marker. The third byte is the
        // next marker's start, which is `0xff` for every variant — JFIF, Exif
        // and the bare ones a camera writes.
        if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            return Some(Format::Jpeg);
        }
        None
    }

    /// What to call it.
    pub fn name(self) -> &'static str {
        match self {
            Format::Png => "PNG",
            Format::Jpeg => "JPEG",
        }
    }
}

/// A canvas from the bytes of a picture on a page, whatever format it is in.
///
/// # Errors
///
/// [`PictureError::Unreadable`] for bytes in no format this engine reads, and
/// for a picture larger than [`MOST_PIXELS`] or of no size at all.
pub fn read(bytes: &[u8]) -> Result<Canvas, PictureError> {
    match Format::of(bytes) {
        Some(Format::Png) => picture_from_png(bytes),
        Some(Format::Jpeg) => from_jpeg(bytes),
        None => Err(PictureError::Unreadable(
            "bytes in no picture format this engine reads".to_owned(),
        )),
    }
}

/// A canvas from a JPEG.
///
/// # Errors
///
/// [`PictureError::Unreadable`], with the same bounds PNG has — see this
/// module's own note about why that is the point rather than a coincidence.
fn from_jpeg(bytes: &[u8]) -> Result<Canvas, PictureError> {
    let mut decoder = jpeg_decoder::Decoder::new(bytes);
    // The header alone, first, so the size is known before anything is
    // reserved. A JPEG's dimensions are in its frame header and the pixels
    // come after; reading the whole thing to find out how big it is would be
    // reading a file to decide whether to read it.
    decoder
        .read_info()
        .map_err(|error| PictureError::Unreadable(error.to_string()))?;
    let info = decoder
        .info()
        .ok_or_else(|| PictureError::Unreadable("a JPEG with no frame in it".to_owned()))?;

    let pixels = u64::from(info.width) * u64::from(info.height);
    if pixels == 0 {
        return Err(PictureError::Unreadable(
            "a picture with no pixels in it".to_owned(),
        ));
    }
    if pixels > MOST_PIXELS {
        return Err(PictureError::Unreadable(format!(
            "a picture of {pixels} pixels, which is more than the {MOST_PIXELS} this engine holds"
        )));
    }

    let data = decoder
        .decode()
        .map_err(|error| PictureError::Unreadable(error.to_string()))?;
    let channels = match info.pixel_format {
        jpeg_decoder::PixelFormat::L8 => 1,
        jpeg_decoder::PixelFormat::RGB24 => 3,
        // Sixteen-bit greyscale and CMYK exist and are rare on the web. Refused
        // by name rather than approximated, because a wrong conversion is a
        // picture in the wrong colours and nobody would know which of the two
        // it was.
        other => {
            return Err(PictureError::Unreadable(format!(
                "a JPEG in {other:?}, which this engine does not convert"
            )));
        }
    };

    let mut canvas = Canvas::new(info.width.into(), info.height.into(), Rgba::TRANSPARENT);
    for y in 0..u32::from(info.height) {
        for x in 0..u32::from(info.width) {
            let at = ((y as usize) * (info.width as usize) + (x as usize)) * channels;
            let Some(sample) = data.get(at..at + channels) else {
                continue;
            };
            // A JPEG has no alpha: every pixel is opaque, which is why a JPEG
            // with a transparent background is a thing people ask for and never
            // get.
            let colour = match channels {
                1 => {
                    let grey = sample.first().copied().unwrap_or(0);
                    Rgba::from_rgba8(grey, grey, grey, 255)
                }
                _ => Rgba::from_rgba8(
                    sample.first().copied().unwrap_or(0),
                    sample.get(1).copied().unwrap_or(0),
                    sample.get(2).copied().unwrap_or(0),
                    255,
                ),
            };
            canvas.blend(x, y, colour, 255);
        }
    }
    Ok(canvas)
}
