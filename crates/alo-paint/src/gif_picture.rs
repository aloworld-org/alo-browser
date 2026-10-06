/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A GIF a page sent.
//!
//! **This is the only file that names `gif`** (ADR 0001). The crate is the one
//! `image` uses, pure Rust, and it forbids `unsafe` throughout.
//!
//! # Two sizes, and both are bounded
//!
//! A GIF declares a **screen** — the picture's size — and then each frame
//! declares a rectangle of its own somewhere on it. Those are two numbers from
//! the file, and the decoder reserves memory for the second, so both are put
//! through [`agreed_size`] before anything is decoded. Bounding only the screen
//! would let a one-pixel GIF carry a frame of four billion.
//!
//! # The first frame, and only the first
//!
//! An animated GIF is drawn as its first frame, laid on a transparent screen
//! where its rectangle says. That is what a browser shows before playback
//! starts, and a still picture is a better answer than a gap while playback is
//! outstanding (queue item 109). A frame reaching past the screen is cut at its
//! edge: the screen is the picture, and `GIF89a` says so.

use crate::canvas::Canvas;
use crate::encode::{MOST_PIXELS, PictureError};
use crate::picture::agreed_size;
use alo_value::Rgba;
use core::num::NonZeroU64;

/// The most the decoder may reserve for one frame: four bytes for each pixel
/// [`MOST_PIXELS`] allows.
///
/// A second line of defence rather than the first — [`agreed_size`] has already
/// refused anything larger by the time a frame is read — but the decoder's own
/// default is fifty megabytes, a number nobody here chose, and the bound should
/// be ours in both places.
const MOST_BYTES_A_FRAME: NonZeroU64 = match NonZeroU64::new(MOST_PIXELS * 4) {
    Some(bytes) => bytes,
    None => NonZeroU64::MIN,
};

/// A canvas from a GIF: its first frame, on a screen of its declared size.
///
/// # Errors
///
/// [`PictureError::Unreadable`] for bytes the decoder refuses, for a GIF with
/// no frame in it, and for a screen or a first frame that fails
/// [`agreed_size`].
pub(crate) fn from_gif(bytes: &[u8]) -> Result<Canvas, PictureError> {
    let mut options = gif::DecodeOptions::new();
    // Eight-bit RGBA whatever the frame holds: the palette looked up and the
    // transparent index made transparent, so one shape of pixel leaves here.
    options.set_color_output(gif::ColorOutput::RGBA);
    options.set_memory_limit(gif::MemoryLimit::Bytes(MOST_BYTES_A_FRAME));
    let mut decoder = options
        .read_info(bytes)
        .map_err(|error| PictureError::Unreadable(error.to_string()))?;

    // The screen first: it is the picture's size, and the canvas's.
    let (width, height) = (decoder.width(), decoder.height());
    agreed_size(width.into(), height.into())?;

    // Then the frame's own rectangle, read from its descriptor before a single
    // pixel of it is decoded.
    let frame = decoder
        .next_frame_info()
        .map_err(|error| PictureError::Unreadable(error.to_string()))?
        .ok_or_else(|| PictureError::Unreadable("a GIF with no frame in it".to_owned()))?;
    let (left, top) = (u32::from(frame.left), u32::from(frame.top));
    let (frame_width, frame_height) = (frame.width, frame.height);

    let mut canvas = Canvas::new(width.into(), height.into(), Rgba::TRANSPARENT);
    // A frame of no size draws nothing on its screen. That is a transparent
    // picture of the declared size, not a broken one.
    if frame_width == 0 || frame_height == 0 {
        return Ok(canvas);
    }
    agreed_size(frame_width.into(), frame_height.into())?;

    let mut buffer = vec![0; decoder.buffer_size()];
    decoder
        .read_into_buffer(&mut buffer)
        .map_err(|error| PictureError::Unreadable(error.to_string()))?;

    for y in 0..u32::from(frame_height) {
        for x in 0..u32::from(frame_width) {
            let at = ((y as usize) * usize::from(frame_width) + (x as usize)) * 4;
            let Some(&[red, green, blue, alpha]) = buffer.get(at..at + 4) else {
                continue;
            };
            // `blend` ignores a pixel off the canvas, which is how a frame
            // reaching past the screen is cut at its edge.
            canvas.blend(
                left + x,
                top + y,
                Rgba::from_rgba8(red, green, blue, alpha),
                255,
            );
        }
    }
    Ok(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GIF written out by hand, so a test can say exactly what the file
    /// declared: a screen, a palette of two, and one frame of the given
    /// rectangle whose pixels are all index one.
    fn hand_written(screen: (u16, u16), frame: (u16, u16, u16, u16)) -> Vec<u8> {
        let (left, top, width, height) = frame;
        let mut bytes = b"GIF89a".to_vec();
        bytes.extend_from_slice(&screen.0.to_le_bytes());
        bytes.extend_from_slice(&screen.1.to_le_bytes());
        // A global palette of two colours, black and red.
        bytes.extend_from_slice(&[0x80, 0, 0, 0, 0, 0, 255, 0, 0]);
        bytes.push(b',');
        for value in [left, top, width, height] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.push(0);
        // LZW, minimum code size two: clear, then index one for every pixel,
        // then end. Codes are three bits wide for the first few, which is all a
        // test this small needs, so a clear code (4), ones, and an end code (5).
        let pixels = usize::from(width) * usize::from(height);
        let mut codes = vec![4u16];
        codes.extend(std::iter::repeat_n(1u16, pixels.min(2)));
        codes.push(5);
        let mut packed = Vec::new();
        let (mut accumulator, mut filled) = (0u32, 0u32);
        for code in codes {
            accumulator |= u32::from(code) << filled;
            filled += 3;
            while filled >= 8 {
                packed.push(u8::try_from(accumulator & 0xff).unwrap_or(0));
                accumulator >>= 8;
                filled -= 8;
            }
        }
        if filled > 0 {
            packed.push(u8::try_from(accumulator & 0xff).unwrap_or(0));
        }
        bytes.push(2);
        bytes.push(u8::try_from(packed.len()).unwrap_or(0));
        bytes.extend_from_slice(&packed);
        bytes.push(0);
        bytes.push(b';');
        bytes
    }

    #[test]
    fn a_hand_written_gif_reads_back_as_it_was_written() {
        let canvas = from_gif(&hand_written((2, 1), (0, 0, 2, 1)));
        let canvas = canvas.unwrap_or_else(|why| panic!("{why}"));
        assert_eq!((canvas.width(), canvas.height()), (2, 1));
        assert_eq!(canvas.at(0, 0).map(Rgba::to_rgba8), Some((255, 0, 0, 255)));
        assert_eq!(canvas.at(1, 0).map(Rgba::to_rgba8), Some((255, 0, 0, 255)));
    }

    #[test]
    fn a_frame_lies_where_its_rectangle_says_and_the_rest_is_clear() {
        let canvas = from_gif(&hand_written((3, 2), (2, 1, 1, 1)));
        let canvas = canvas.unwrap_or_else(|why| panic!("{why}"));
        assert_eq!((canvas.width(), canvas.height()), (3, 2));
        assert_eq!(canvas.at(2, 1).map(Rgba::to_rgba8), Some((255, 0, 0, 255)));
        assert_eq!(canvas.at(0, 0).map(Rgba::to_rgba8), Some((0, 0, 0, 0)));
    }

    #[test]
    fn a_frame_reaching_past_the_screen_is_cut_at_its_edge() {
        let canvas = from_gif(&hand_written((1, 1), (0, 0, 2, 1)));
        let canvas = canvas.unwrap_or_else(|why| panic!("{why}"));
        assert_eq!((canvas.width(), canvas.height()), (1, 1));
        assert_eq!(canvas.at(0, 0).map(Rgba::to_rgba8), Some((255, 0, 0, 255)));
    }

    #[test]
    fn a_frame_of_no_size_is_a_clear_picture_of_the_screens_size() {
        let canvas = from_gif(&hand_written((2, 2), (0, 0, 0, 0)));
        let canvas = canvas.unwrap_or_else(|why| panic!("{why}"));
        assert_eq!((canvas.width(), canvas.height()), (2, 2));
        assert_eq!(canvas.at(1, 1).map(Rgba::to_rgba8), Some((0, 0, 0, 0)));
    }

    #[test]
    fn a_screen_of_no_size_is_refused() {
        let why = from_gif(&hand_written((0, 0), (0, 0, 1, 1)))
            .err()
            .map(|why| why.to_string())
            .unwrap_or_default();
        assert!(why.contains("no pixels"), "{why:?}");
    }

    /// Bounding the screen alone would let a one-pixel GIF carry a frame the
    /// decoder reserves gigabytes for. This is the case that says the frame is
    /// asked as well, and asked first: the refusal names the size, rather than
    /// the decoder's own limit or a failure partway through the pixels.
    #[test]
    fn a_frame_claiming_more_pixels_than_this_engine_holds_is_refused_for_its_size() {
        let why = from_gif(&hand_written((1, 1), (0, 0, u16::MAX, u16::MAX)))
            .err()
            .map(|why| why.to_string())
            .unwrap_or_default();
        assert!(why.contains("more than"), "{why:?}");
    }

    /// A screen, a palette, and the trailer straight after. The decoder
    /// refuses this itself, before a frame is asked for; the `no frame` refusal
    /// above is for a decoder that one day does not.
    #[test]
    fn a_gif_with_no_frame_is_refused() {
        let mut bytes = b"GIF89a".to_vec();
        bytes.extend_from_slice(&[1, 0, 1, 0, 0x80, 0, 0, 0, 0, 0, 0, 0, 0]);
        bytes.push(b';');
        assert!(from_gif(&bytes).is_err());
    }
}
