/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A JPEG a page sent.
//!
//! **This is the only file that names `jpeg_decoder`** (ADR 0001). Split from
//! [`crate::picture`] when GIF and WebP arrived, because a decoder is a reason
//! to change and that file would otherwise have had four.

use crate::canvas::Canvas;
use crate::encode::PictureError;
use crate::picture::agreed_size;
use alo_value::Rgba;

/// A canvas from a JPEG.
///
/// # Errors
///
/// [`PictureError::Unreadable`], with the same bound every format has — see
/// [`crate::picture`] for why that is the point rather than a coincidence.
pub(crate) fn from_jpeg(bytes: &[u8]) -> Result<Canvas, PictureError> {
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

    agreed_size(info.width.into(), info.height.into())?;

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
