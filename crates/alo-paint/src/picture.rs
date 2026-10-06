/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Reading a picture a page sent, in whatever format it turns out to be.
//!
//! This file decides **which** format a run of bytes is and holds the one
//! bound every format answers to. It names no decoder: each rented one lives in
//! a file of its own (ADR 0001) — PNG in [`crate::encode`], because that file
//! already rented `png` for reference renders, and JPEG, GIF and WebP in
//! `jpeg_picture.rs`, `gif_picture.rs` and `webp_picture.rs`. A decoder is a
//! reason to change, and four of them in one file would be four.
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
//! The reason the formats are one item and not four: a decoder with its own
//! limits, or none, would be a second way in. Every one goes through
//! [`agreed_size`], so every one refuses a picture of no size and one larger
//! than [`MOST_PIXELS`], and every one asks **before** the allocation rather
//! than after.
//!
//! # What is not read
//!
//! AVIF. Its pixels are an AV1 frame. ADR 0021 chose `rav1d` to decode it,
//! through `rav1d`'s own safe Rust API, and that API is not in any release yet:
//! the newest release offers only a C interface, which this engine could call
//! only with `unsafe` of its own (queue item 269). Until the release, an AVIF
//! is refused like any other format this engine does not read.

use crate::canvas::Canvas;
use crate::encode::{MOST_PIXELS, PictureError, picture_from_png};
use crate::gif_picture::from_gif;
use crate::jpeg_picture::from_jpeg;
use crate::webp_picture::from_webp;

/// What a run of bytes turns out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// A PNG.
    Png,
    /// A JPEG.
    Jpeg,
    /// A GIF, of either version.
    Gif,
    /// A WebP: lossy, lossless or extended.
    WebP,
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
        // Both versions there are, and only those: a signature with any other
        // version is a file this engine has never seen described.
        if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            return Some(Format::Gif);
        }
        // A RIFF container says what it holds in bytes eight to twelve; the
        // four between are its length. RIFF also carries WAV and AVI, which is
        // why `RIFF` alone is not enough.
        if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP".as_slice()) {
            return Some(Format::WebP);
        }
        None
    }

    /// What to call it.
    pub fn name(self) -> &'static str {
        match self {
            Format::Png => "PNG",
            Format::Jpeg => "JPEG",
            Format::Gif => "GIF",
            Format::WebP => "WebP",
        }
    }
}

/// A canvas from the bytes of a picture on a page, whatever format it is in.
///
/// An animated picture comes back as its **first frame**: a still picture is a
/// better answer than a gap while playback is outstanding (queue item 109).
///
/// # Errors
///
/// [`PictureError::Unreadable`] for bytes in no format this engine reads, and
/// for a picture larger than [`MOST_PIXELS`] or of no size at all.
pub fn read(bytes: &[u8]) -> Result<Canvas, PictureError> {
    match Format::of(bytes) {
        Some(Format::Png) => picture_from_png(bytes),
        Some(Format::Jpeg) => from_jpeg(bytes),
        Some(Format::Gif) => from_gif(bytes),
        Some(Format::WebP) => from_webp(bytes),
        None => Err(PictureError::Unreadable(
            "bytes in no picture format this engine reads".to_owned(),
        )),
    }
}

/// Whether a size a file declared is one this engine will hold.
///
/// The one bound, for every format. It is asked of a size the file **said**,
/// before a decoder reserves anything for it, because a hundred-byte file that
/// parses perfectly can declare seventeen gigabytes.
///
/// # Errors
///
/// [`PictureError::Unreadable`] for a size with no pixels in it, and for one
/// with more than [`MOST_PIXELS`].
pub(crate) fn agreed_size(width: u64, height: u64) -> Result<(), PictureError> {
    let pixels = width.saturating_mul(height);
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gif_is_known_by_either_version_and_no_other() {
        assert_eq!(Format::of(b"GIF87a..."), Some(Format::Gif));
        assert_eq!(Format::of(b"GIF89a..."), Some(Format::Gif));
        assert_eq!(Format::of(b"GIF90a..."), None);
        assert_eq!(Format::of(b"GIF8"), None);
    }

    #[test]
    fn a_riff_file_is_a_webp_only_when_it_says_so() {
        assert_eq!(Format::of(b"RIFF\0\0\0\0WEBPVP8 "), Some(Format::WebP));
        // The same container holding a sound, and one cut before it says.
        assert_eq!(Format::of(b"RIFF\0\0\0\0WAVEfmt "), None);
        assert_eq!(Format::of(b"RIFF\0\0\0\0WEB"), None);
    }

    #[test]
    fn an_avif_is_refused_rather_than_attempted() {
        let avif = b"\0\0\0\x1cftypavif\0\0\0\0avifmif1";
        assert_eq!(Format::of(avif), None);
        assert!(read(avif).is_err());
    }

    #[test]
    fn the_bound_refuses_nothing_and_too_much_and_nothing_between() {
        assert!(agreed_size(0, 10).is_err());
        assert!(agreed_size(10, 0).is_err());
        assert!(agreed_size(1, 1).is_ok());
        assert!(agreed_size(8192, 8192).is_ok());
        assert!(agreed_size(8192, 8193).is_err());
        // A product that would overflow is still a refusal, not a wrap to
        // something small.
        assert!(agreed_size(u64::MAX, u64::MAX).is_err());
    }
}
