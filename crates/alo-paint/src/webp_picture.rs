/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A WebP a page sent.
//!
//! **This is the only file that names `image_webp`** (ADR 0001). The crate is
//! the one `image` uses, pure Rust, and it forbids `unsafe` throughout.
//!
//! # A size the decoder believes before it checks
//!
//! A WebP is a RIFF container. The simple kinds hold one bitstream whose header
//! *is* the picture's size, so bounding that size is bounding the decode. The
//! **extended** kind declares a canvas, and the lossy bitstreams inside it —
//! the picture itself, or each frame of an animation — declare sizes of their
//! own, up to 16 383 square.
//!
//! The rented decoder reserves memory for a lossy bitstream by **its own**
//! header and only afterwards compares that with the canvas. So a WebP whose
//! canvas is one pixel can make it reserve four hundred megabytes before it
//! notices the lie: more than [`MOST_PIXELS`] allows, from a file of a few
//! dozen bytes. Checking the canvas alone would not catch it.
//!
//! So before the decoder is handed anything, [`lossy_sizes_agreed`] walks the
//! container and puts every lossy bitstream's declared size through
//! [`agreed_size`]. It reads chunk headers and four bytes of each bitstream,
//! and no more: it is a bound, not a second decoder. A lossless bitstream needs
//! no such walk, because the decoder compares its size with the canvas before
//! it reserves anything.
//!
//! # A frame that is not its own size
//!
//! The same walk checks one more thing, because the decoder does not. Each
//! frame of an animation declares its size, and so does the lossy bitstream
//! inside it. Where the frame has no alpha the decoder compares the two; where
//! it has an alpha chunk it does not, and walks the bitstream's size over an
//! alpha plane of the frame's — **and panics** the moment the bitstream is the
//! larger. Found by mutating `a-picture-in-each-format`'s animated WebP: one in
//! three hundred thousand mutations, from a file of three hundred bytes. A
//! renderer that panics on a picture is a page that takes its tab down, so a
//! frame whose bitstream is not the frame's size is refused here, before the
//! decoder can reach that loop. The rented crate is at its newest release, and
//! this is the one place its answer is not taken on trust.
//!
//! # The first frame, and only the first
//!
//! An animated WebP is drawn as its first frame, as an animated GIF is (queue
//! item 109).

use crate::canvas::Canvas;
use crate::encode::{MOST_PIXELS, PictureError};
use crate::picture::agreed_size;
use alo_value::Rgba;
use std::io::Cursor;

/// A canvas from a WebP: the picture, or an animation's first frame.
///
/// # Errors
///
/// [`PictureError::Unreadable`] for bytes the decoder refuses, and for a
/// canvas, or any lossy bitstream inside it, that fails [`agreed_size`].
pub(crate) fn from_webp(bytes: &[u8]) -> Result<Canvas, PictureError> {
    // Before the decoder sees a byte: see this module's note.
    lossy_sizes_agreed(bytes)?;

    let mut decoder = image_webp::WebPDecoder::new(Cursor::new(bytes))
        .map_err(|error| PictureError::Unreadable(error.to_string()))?;
    let (width, height) = decoder.dimensions();
    agreed_size(width.into(), height.into())?;
    decoder.set_memory_limit(usize::try_from(MOST_PIXELS * 4).unwrap_or(usize::MAX));

    // Three bytes a pixel without alpha and four with: the decoder hands back
    // what the file holds rather than widening it.
    let channels = if decoder.has_alpha() { 4 } else { 3 };
    let size = decoder
        .output_buffer_size()
        .ok_or_else(|| PictureError::Unreadable("a WebP too large to hold in memory".to_owned()))?;
    let mut buffer = vec![0; size];
    decoder
        .read_image(&mut buffer)
        .map_err(|error| PictureError::Unreadable(error.to_string()))?;

    let mut canvas = Canvas::new(width, height, Rgba::TRANSPARENT);
    for y in 0..height {
        for x in 0..width {
            let at = ((y as usize) * (width as usize) + (x as usize)) * channels;
            let Some(sample) = buffer.get(at..at + channels) else {
                continue;
            };
            let colour = match *sample {
                [red, green, blue, alpha] => Rgba::from_rgba8(red, green, blue, alpha),
                [red, green, blue] => Rgba::from_rgba8(red, green, blue, 255),
                _ => continue,
            };
            canvas.blend(x, y, colour, 255);
        }
    }
    Ok(canvas)
}

/// Whether every lossy bitstream in a WebP declares a size this engine holds,
/// and each one inside an animation frame declares that frame's size.
///
/// Top-level `VP8 ` chunks, and the lossy bitstream inside each animation
/// frame (`ANMF`), which is the only place one may nest — found the way the
/// decoder finds it, which [`frame_agreed`] says more about. A file cut short, or one
/// whose chunk lengths lie, is not refused here: the walk stops where the bytes
/// do, and the decoder refuses it in its own words. This only refuses sizes.
///
/// # Errors
///
/// [`PictureError::Unreadable`] from [`agreed_size`], for the first lossy
/// bitstream that declares too many pixels or none, and for one that is not
/// the size of the frame it is in.
fn lossy_sizes_agreed(bytes: &[u8]) -> Result<(), PictureError> {
    // `RIFF`, the container's length, and `WEBP`: twelve bytes, then chunks.
    let Some(body) = bytes.get(12..) else {
        return Ok(());
    };
    for (name, payload) in chunks(body) {
        match &name {
            b"VP8 " => {
                if let Some((width, height)) = lossy_size(payload) {
                    agreed_size(width.into(), height.into())?;
                }
            }
            b"ANMF" => frame_agreed(payload)?,
            _ => {}
        }
    }
    Ok(())
}

/// One animation frame: its lossy bitstream bounded, and the frame's size.
///
/// The frame's header is sixteen bytes — three each of position across and
/// down, three each of width and height less one, three of duration and one of
/// flags — and the frame's own chunks follow.
///
/// **Which chunk is the lossy bitstream is the decoder's rule, not the
/// format's.** The format says a frame is `VP8 `, `VP8L`, or `ALPH` followed by
/// `VP8 `. The decoder reads the first chunk by its name, but after an `ALPH`
/// it decodes the next chunk as a lossy bitstream **whatever it is called** —
/// so a frame of `ALPH` and a chunk named anything else got past a walk that
/// looked for the name, and panicked the decoder the same way. The second
/// mutation search found it. So this asks of a frame what the decoder will do
/// with it: the first chunk if it is `VP8 `, or the second if the first is
/// `ALPH`.
fn frame_agreed(frame: &[u8]) -> Result<(), PictureError> {
    let declared = match frame.get(6..12) {
        Some(&[w0, w1, w2, h0, h1, h2]) => Some((
            u32::from_le_bytes([w0, w1, w2, 0]) + 1,
            u32::from_le_bytes([h0, h1, h2, 0]) + 1,
        )),
        _ => None,
    };
    let inner = chunks(frame.get(16..).unwrap_or_default());
    let lossy = match inner.as_slice() {
        [([b'V', b'P', b'8', b' '], payload), ..]
        | [([b'A', b'L', b'P', b'H'], _), (_, payload), ..] => Some(*payload),
        _ => None,
    };
    if let Some(payload) = lossy {
        let Some((width, height)) = lossy_size(payload) else {
            return Ok(());
        };
        agreed_size(width.into(), height.into())?;
        if let Some((frame_width, frame_height)) = declared {
            if (u32::from(width), u32::from(height)) != (frame_width, frame_height) {
                return Err(PictureError::Unreadable(format!(
                    "a WebP frame of {frame_width}×{frame_height} holding a picture of \
                     {width}×{height}"
                )));
            }
        }
    }
    Ok(())
}

/// The size a lossy bitstream declares.
///
/// A VP8 key frame begins with three bytes of frame tag and three of start
/// code, then the width and the height as two little-endian sixteen-bit
/// numbers whose top two bits are a scale and not part of the size. [`None`]
/// for a bitstream too short to say, which is left to the decoder to refuse.
fn lossy_size(bitstream: &[u8]) -> Option<(u16, u16)> {
    let &[width_low, width_high, height_low, height_high] = bitstream.get(6..10)? else {
        return None;
    };
    Some((
        u16::from_le_bytes([width_low, width_high]) & 0x3fff,
        u16::from_le_bytes([height_low, height_high]) & 0x3fff,
    ))
}

/// The chunks of a RIFF body, as their four-byte names and their payloads.
///
/// Each is a name, a little-endian thirty-two-bit length, the payload, and one
/// byte of padding when the length is odd. A payload whose length runs past
/// the end is cut at the end; a header cut short ends the walk. Every step is
/// checked arithmetic, because the lengths came from the file.
fn chunks(mut body: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut found = Vec::new();
    while let Some(&[a, b, c, d, l0, l1, l2, l3]) = body.get(..8) {
        let length = usize::try_from(u32::from_le_bytes([l0, l1, l2, l3])).unwrap_or(usize::MAX);
        let rest = body.get(8..).unwrap_or_default();
        found.push(([a, b, c, d], rest.get(..length).unwrap_or(rest)));
        let Some(next) = length
            .checked_add(length & 1)
            .and_then(|padded| padded.checked_add(8))
        else {
            break;
        };
        let Some(after) = body.get(next..) else {
            break;
        };
        body = after;
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chunk: its name, its length, its payload and any padding.
    fn chunk(name: [u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = name.to_vec();
        let length = u32::try_from(payload.len()).unwrap_or(0);
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            bytes.push(0);
        }
        bytes
    }

    /// A RIFF file around some chunks.
    fn riff(chunks: &[Vec<u8>]) -> Vec<u8> {
        let body: Vec<u8> = chunks.concat();
        let mut bytes = b"RIFF".to_vec();
        let length = u32::try_from(body.len() + 4).unwrap_or(0);
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(b"WEBP");
        bytes.extend_from_slice(&body);
        bytes
    }

    /// The first ten bytes of a VP8 key frame declaring a size, and nothing a
    /// decoder could make a picture of after them.
    fn vp8_declaring(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0x50, 0x02, 0x00, 0x9d, 0x01, 0x2a];
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes.extend_from_slice(&[0; 16]);
        bytes
    }

    /// An extended header: flags, three reserved bytes, and the canvas's width
    /// and height less one, three bytes each.
    fn vp8x(flags: u8, width: u32, height: u32) -> Vec<u8> {
        let mut payload = vec![flags, 0, 0, 0];
        payload.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
        payload.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
        chunk(*b"VP8X", &payload)
    }

    fn refusal(bytes: &[u8]) -> String {
        from_webp(bytes)
            .err()
            .map(|why| why.to_string())
            .unwrap_or_default()
    }

    /// The case this module's note is about: a one-pixel canvas carrying a
    /// bitstream that says sixteen thousand square. Refused **for its size**,
    /// which is only possible if the walk ran first — the decoder's own
    /// refusal, after it had reserved the memory, says the sizes disagree.
    #[test]
    fn a_one_pixel_canvas_cannot_carry_a_bitstream_claiming_more_pixels_than_this_engine_holds() {
        let bytes = riff(&[vp8x(0, 1, 1), chunk(*b"VP8 ", &vp8_declaring(16383, 16383))]);
        let why = refusal(&bytes);
        assert!(why.contains("more than"), "{why:?}");
    }

    #[test]
    fn nor_can_an_animation_frame() {
        let mut frame = vec![0; 16];
        frame.extend_from_slice(&chunk(*b"VP8 ", &vp8_declaring(16383, 16383)));
        let bytes = riff(&[
            vp8x(0b10, 1, 1),
            chunk(*b"ANIM", &[0; 6]),
            chunk(*b"ANMF", &frame),
        ]);
        let why = refusal(&bytes);
        assert!(why.contains("more than"), "{why:?}");
    }

    #[test]
    fn a_canvas_claiming_more_pixels_than_this_engine_holds_is_refused() {
        // Sixteen million by five: eighty million pixels, from a canvas
        // header of ten bytes.
        let bytes = riff(&[
            vp8x(0, 16_000_000, 5),
            chunk(*b"VP8 ", &vp8_declaring(1, 1)),
        ]);
        let why = refusal(&bytes);
        assert!(why.contains("more than"), "{why:?}");
    }

    /// A frame of one pixel, with an alpha chunk, holding a bitstream of two.
    /// The decoder walks the bitstream's size over the frame's alpha plane and
    /// panics; this is refused before it is handed over.
    #[test]
    fn a_frame_holding_a_picture_of_another_size_is_refused() {
        let mut frame = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        frame.extend_from_slice(&chunk(*b"ALPH", &[0, 0]));
        frame.extend_from_slice(&chunk(*b"VP8 ", &vp8_declaring(2, 1)));
        let bytes = riff(&[
            vp8x(0b1_0010, 1, 1),
            chunk(*b"ANIM", &[0; 6]),
            chunk(*b"ANMF", &frame),
        ]);
        let why = refusal(&bytes);
        assert!(why.contains("holding a picture of 2×1"), "{why:?}");
    }

    /// After an alpha chunk the decoder takes the next chunk as the picture
    /// whatever its name, so the walk does too.
    #[test]
    fn after_an_alpha_chunk_the_next_chunk_is_the_picture_whatever_its_name() {
        let mut frame = vec![0; 16];
        frame.extend_from_slice(&chunk(*b"ALPH", &[0, 0]));
        frame.extend_from_slice(&chunk(*b"JUNK", &vp8_declaring(2, 1)));
        let why = frame_agreed(&frame)
            .err()
            .map(|why| why.to_string())
            .unwrap_or_default();
        assert!(why.contains("holding a picture of 2×1"), "{why:?}");
    }

    #[test]
    fn a_frame_holding_a_picture_of_its_own_size_is_left_to_the_decoder() {
        let mut frame = vec![0; 16];
        frame.extend_from_slice(&chunk(*b"VP8 ", &vp8_declaring(1, 1)));
        assert!(frame_agreed(&frame).is_ok());
    }

    #[test]
    fn the_two_scale_bits_are_not_part_of_the_size() {
        // 0xc001 is a width of one with both scale bits set: a size of one,
        // rather than 49 153.
        assert_eq!(
            lossy_size(&vp8_declaring(0xc001, 0x3fff)),
            Some((1, 0x3fff))
        );
    }

    #[test]
    fn a_bitstream_too_short_to_say_is_left_to_the_decoder() {
        assert_eq!(lossy_size(&[0x50, 0x02, 0x00, 0x9d]), None);
        assert!(refusal(&riff(&[chunk(*b"VP8 ", &[0x50, 0x02])])).contains("not a picture"));
    }

    #[test]
    fn the_walk_follows_padding_and_stops_where_the_bytes_do() {
        let body = [chunk(*b"ABCD", &[1, 2, 3]), chunk(*b"EFGH", &[4])].concat();
        let found = chunks(&body);
        assert_eq!(found.len(), 2);
        assert_eq!(found.first(), Some(&(*b"ABCD", [1, 2, 3].as_slice())));
        assert_eq!(found.get(1), Some(&(*b"EFGH", [4].as_slice())));

        // A length that runs past the end is cut at the end rather than
        // trusted, and a header cut short ends the walk.
        let mut lying = b"ABCD".to_vec();
        lying.extend_from_slice(&u32::MAX.to_le_bytes());
        lying.extend_from_slice(&[9, 9]);
        assert_eq!(chunks(&lying), vec![(*b"ABCD", [9, 9].as_slice())]);
        assert!(chunks(b"ABCD\x01").is_empty());
    }
}
