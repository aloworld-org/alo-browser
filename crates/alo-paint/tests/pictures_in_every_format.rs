/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A picture in whatever format it turns out to be, and the same bounds every
//! way.
//!
//! The reason the formats are one item rather than four: a decoder with its own
//! limits, or none, would be a second way in. Every test that matters here is
//! run against **every** format, from one list, so that adding another means
//! adding it to the list rather than remembering to — which is how GIF and WebP
//! arrived, seven files in four kinds of container.

use alo_paint::picture::{Format, read};

/// A frozen picture beside a corpus case, which is where the real files live
/// rather than being duplicated here.
fn frozen_in(case: &str, name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../alo-corpus/cases")
        .join(case)
        .join(name);
    std::fs::read(path).unwrap_or_default()
}

/// PNG and JPEG, from the case that first held them.
fn frozen(name: &str) -> Vec<u8> {
    frozen_in("a-picture", name)
}

/// GIF and WebP, from the case item 180 added.
fn frozen_new(name: &str) -> Vec<u8> {
    frozen_in("a-picture-in-each-format", name)
}

/// Every format, and every kind of each, as the same picture — so a test can
/// say "any of these". The animated ones are here too: their first frame *is*
/// the stripes, and the second is the stripes upside down, so a decoder that
/// drew the wrong frame fails the same test as one that drew the wrong way up.
fn every() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("PNG", frozen("stripes.png")),
        ("JPEG", frozen("stripes.jpg")),
        ("GIF", frozen_new("stripes.gif")),
        ("animated GIF", frozen_new("stripes-moving.gif")),
        ("lossy WebP", frozen_new("stripes.webp")),
        ("lossless WebP", frozen_new("stripes-lossless.webp")),
        ("animated WebP", frozen_new("stripes-moving.webp")),
    ]
}

/// The two with a see-through blue stripe, which are the stripes everywhere
/// else.
fn clear() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("GIF with transparency", frozen_new("stripes-clear.gif")),
        ("WebP with alpha", frozen_new("stripes-clear.webp")),
    ]
}

// --- The format comes from the bytes -----------------------------------------

/// A `src` ending in `.png` proves nothing: it is a string on a page, and the
/// server that answered may have sent something else — by mistake or on
/// purpose. A decoder handed the wrong format will either fail confusingly or,
/// worse, find something in it.
#[test]
fn the_format_is_decided_by_the_bytes_rather_than_by_any_name() {
    assert_eq!(Format::of(&frozen("stripes.png")), Some(Format::Png));
    assert_eq!(Format::of(&frozen("stripes.jpg")), Some(Format::Jpeg));
    assert_eq!(Format::of(&frozen_new("stripes.gif")), Some(Format::Gif));
    assert_eq!(
        Format::of(&frozen_new("stripes-moving.gif")),
        Some(Format::Gif)
    );
    for webp in [
        "stripes.webp",
        "stripes-lossless.webp",
        "stripes-clear.webp",
        "stripes-moving.webp",
    ] {
        assert_eq!(Format::of(&frozen_new(webp)), Some(Format::WebP), "{webp}");
    }
    assert_eq!(Format::of(b"none of those"), None);
    assert_eq!(Format::of(&[]), None);

    // The corpus case has this same file under a `.png` name, and it decodes.
    let named_wrong = frozen("stripes.jpg");
    assert_eq!(Format::of(&named_wrong), Some(Format::Jpeg));
    assert!(read(&named_wrong).is_ok());
}

#[test]
fn a_format_this_engine_does_not_read_is_refused_rather_than_attempted() {
    // An AVIF's first box, which is the format this engine does not read yet
    // (queue item 269).
    let why = read(b"\0\0\0\x1cftypavif\0\0\0\0avifmif1miaf")
        .err()
        .map(|why| why.to_string())
        .unwrap_or_default();
    assert!(why.contains("no picture format"), "{why:?}");
}

// --- Every format, the same picture -----------------------------------------

#[test]
fn every_format_comes_back_the_same_size() {
    for (name, bytes) in every().into_iter().chain(clear()) {
        let canvas = read(&bytes).unwrap_or_else(|why| panic!("{name}: {why}"));
        assert_eq!(
            (canvas.width(), canvas.height()),
            (24, 24),
            "{name} came back the wrong size"
        );
    }
}

/// The stripes are red, green and blue on purpose: a flipped picture or a wrong
/// row order is obvious rather than plausible.
///
/// Two things about the picture are for JPEG's sake, and both were learned by
/// getting them wrong. It asks which channel is **largest** rather than for an
/// exact colour, because JPEG does not promise the bytes back. And the stripes
/// are eight rows tall in a twenty-four-pixel picture rather than one row in a
/// three-pixel one, because a picture that small is a single DCT block and
/// chroma subsampling returns it as mud — the first version asserted green and
/// got `(130, 123, 115)`.
#[test]
fn every_format_comes_back_the_right_way_up() {
    for (name, bytes) in every() {
        let canvas = read(&bytes).unwrap_or_else(|why| panic!("{name}: {why}"));
        // The middle of each stripe, away from the boundaries a lossy
        // format blurs across.
        for (row, expected) in [(4, "red"), (12, "green"), (20, "blue")] {
            let pixel = canvas.at(1, row).unwrap_or_default();
            let (red, green, blue, alpha) = pixel.to_rgba8();
            let largest = match expected {
                "red" => red > green && red > blue,
                "green" => green > red && green > blue,
                _ => blue > red && blue > green,
            };
            assert!(
                largest,
                "{name} row {row} should be mostly {expected} and is ({red}, {green}, {blue})"
            );
            assert_eq!(alpha, 255, "{name} row {row} should be opaque");
        }
    }
}

// --- The same refusals, every way --------------------------------------------

#[test]
fn no_format_reads_bytes_that_stop_in_the_middle_into_a_different_size() {
    for (name, whole) in every().into_iter().chain(clear()) {
        assert!(!whole.is_empty(), "{name} is missing from the corpus case");
        let mut refused = 0;
        for cut in 1..whole.len() {
            let Some(part) = whole.get(..cut) else {
                continue;
            };
            match read(part) {
                Ok(canvas) => assert_eq!(
                    (canvas.width(), canvas.height()),
                    (24, 24),
                    "{name}: {cut} bytes produced a size the file never declared"
                ),
                Err(_) => refused += 1,
            }
        }
        assert!(
            refused > whole.len() / 2,
            "{name}: only {refused} of {} prefixes were refused",
            whole.len()
        );
    }
}

/// Where each file declares the picture's size.
///
/// A corrupt byte there is a file declaring a different size, and decoding it
/// at that size is honouring the declaration rather than inventing one — which
/// is what a GIF's screen and an animated WebP's canvas get, because nothing
/// else in either file repeats the size for a decoder to check it against. PNG
/// has a checksum over its header, and the other kinds repeat the size where
/// the decoder compares the two, so for those a flipped byte here is refused;
/// the range is the same rule either way.
fn size_field(name: &str, bytes: &[u8]) -> std::ops::Range<usize> {
    match name {
        // The header chunk's width and height.
        "PNG" => 16..24,
        // The frame header, wherever its marker is: two bytes of length and
        // one of precision after it, then the height and the width.
        "JPEG" => {
            let marker = bytes
                .windows(2)
                .position(|pair| pair == [0xff, 0xc0])
                .unwrap_or(0);
            marker + 5..marker + 9
        }
        // The logical screen.
        "GIF" | "animated GIF" | "GIF with transparency" => 6..10,
        // The bitstream's own header.
        "lossy WebP" => 26..30,
        // Fourteen bits each, after the one-byte signature.
        "lossless WebP" => 21..25,
        // The canvas in the extended header.
        _ => 24..30,
    }
}

/// Every byte past the signature, flipped in turn. A corrupt file is refused,
/// or comes back the size it was, or comes back a different size **only**
/// because the byte that changed was the size — and never panics, which is
/// what running every one of them through a decoder is for.
#[test]
fn no_format_decodes_a_corrupt_file_into_a_size_it_does_not_declare() {
    for (name, whole) in every().into_iter().chain(clear()) {
        let mut refused = 0;
        let size_at = size_field(name, &whole);
        for at in 4..whole.len() {
            let mut broken = whole.clone();
            if let Some(byte) = broken.get_mut(at) {
                *byte ^= 0xff;
            }
            match read(&broken) {
                Ok(canvas) => assert!(
                    (canvas.width(), canvas.height()) == (24, 24) || size_at.contains(&at),
                    "{name}: flipping byte {at} decoded to {}×{}, and byte {at} is not the size",
                    canvas.width(),
                    canvas.height()
                ),
                Err(_) => refused += 1,
            }
        }
        assert!(
            refused > 0,
            "{name}: not one corrupted byte was noticed, so nothing is being checked"
        );
    }
}

/// A JPEG's dimensions are in its frame header and its pixels come after, so
/// the size is knowable without decoding — which is what makes the bound a
/// bound rather than a check after the fact.
#[test]
fn a_jpeg_claiming_more_pixels_than_this_engine_holds_is_refused() {
    let mut bytes = frozen("stripes.jpg");
    // Find the start-of-frame marker and rewrite the height and width in it.
    // `0xffc0` is a baseline frame; the two bytes after the marker are its
    // length, then one byte of precision, then the height and the width.
    let mut at = None;
    for index in 0..bytes.len().saturating_sub(1) {
        if bytes.get(index) == Some(&0xff) && bytes.get(index + 1) == Some(&0xc0) {
            at = Some(index + 5);
            break;
        }
    }
    let Some(at) = at else {
        panic!("the frozen JPEG has no baseline frame marker to rewrite");
    };
    if let Some(field) = bytes.get_mut(at..at + 4) {
        // Sixty-five thousand square: four billion pixels, seventeen gigabytes.
        field.copy_from_slice(&[0xff, 0xff, 0xff, 0xff]);
    }
    let why = read(&bytes)
        .err()
        .map(|why| why.to_string())
        .unwrap_or_default();
    assert!(
        why.contains("more than"),
        "a JPEG claiming four billion pixels was not refused for its size: {why:?}"
    );
}

/// A see-through stripe stays see-through and the rest stays opaque: a GIF's
/// transparent index and a WebP's alpha chunk both reach the canvas.
#[test]
fn transparency_survives_either_format() {
    for (name, bytes) in clear() {
        let canvas = read(&bytes).unwrap_or_else(|why| panic!("{name}: {why}"));
        for row in [4, 12] {
            let (.., alpha) = canvas.at(1, row).unwrap_or_default().to_rgba8();
            assert_eq!(alpha, 255, "{name} row {row} should be opaque");
        }
        let (.., alpha) = canvas.at(1, 20).unwrap_or_default().to_rgba8();
        assert_eq!(alpha, 0, "{name} row 20 should be see-through");
    }
}

/// The frozen file with the bytes at `at` replaced, for a test that rewrites a
/// header to claim a size. A file too short to rewrite comes back unchanged,
/// and the test that asked for it then fails on what it reads.
fn rewritten(mut bytes: Vec<u8>, at: usize, with: &[u8]) -> Vec<u8> {
    if let Some(field) = bytes.get_mut(at..at + with.len()) {
        field.copy_from_slice(with);
    }
    bytes
}

fn refusal(bytes: &[u8]) -> String {
    read(bytes)
        .err()
        .map(|why| why.to_string())
        .unwrap_or_default()
}

/// A GIF's screen is six bytes in: two little-endian sixteen-bit numbers.
/// Sixty-five thousand square is four billion pixels.
#[test]
fn a_gif_claiming_more_pixels_than_this_engine_holds_is_refused() {
    let bytes = rewritten(frozen_new("stripes.gif"), 6, &[0xff; 4]);
    let why = refusal(&bytes);
    assert!(why.contains("more than"), "{why:?}");
}

/// A simple lossy WebP's size is in its bitstream's header, twenty-six bytes
/// in: the RIFF header, the chunk header, and six bytes of frame tag and start
/// code. Sixteen thousand square is two hundred and sixty-eight million.
#[test]
fn a_lossy_webp_claiming_more_pixels_than_this_engine_holds_is_refused() {
    let bytes = rewritten(frozen_new("stripes.webp"), 26, &[0xff, 0x3f, 0xff, 0x3f]);
    let why = refusal(&bytes);
    assert!(why.contains("more than"), "{why:?}");
}

/// An extended WebP's canvas is in its `VP8X` chunk, twenty-four bytes in:
/// three bytes each of width and height, less one. Sixteen million by five:
/// eighty-three million pixels. Not sixteen million square, which the decoder
/// refuses itself because the product overflows its arithmetic — a refusal,
/// but not this engine's bound being asked.
#[test]
fn an_extended_webp_claiming_more_pixels_than_this_engine_holds_is_refused() {
    for name in ["stripes-clear.webp", "stripes-moving.webp"] {
        let bytes = rewritten(frozen_new(name), 24, &[0xff, 0xff, 0xff, 0x04, 0, 0]);
        let why = refusal(&bytes);
        assert!(why.contains("more than"), "{name}: {why:?}");
    }
}

/// What the mutation search found, rebuilt from the frozen file by hand: the
/// first frame's chunk renamed so the decoder skips it, and the second frame —
/// the one with an alpha chunk — made to say it is sixteen pixels wide while
/// its lossy picture, untouched and perfectly valid, is twenty-four. With an
/// alpha chunk the rented decoder does not compare the two sizes, and it
/// panicked here, indexing past a sixteen-wide alpha plane, until the size walk
/// in front of it learned to refuse this.
///
/// The second search found the same panic with the picture's chunk renamed:
/// after an alpha chunk the decoder takes the next chunk as the picture
/// whatever it is called, and a walk that looked for `VP8 ` by name let it
/// through. So the case runs with both names.
#[test]
fn an_animation_frame_narrower_than_its_picture_is_refused_rather_than_a_panic() {
    for name in [b"VP8 ", b"VP\x82 "] {
        let bytes = rewritten(frozen_new("stripes-moving.webp"), 0x2c, b"ANMX");
        // The second frame's header: its chunk header at 0xae, then three bytes
        // each of position across and down, then the width less one.
        let bytes = rewritten(bytes, 0xae + 8 + 6, &[15, 0, 0]);
        // Its picture's chunk, after the alpha chunk at 0xc6.
        let bytes = rewritten(bytes, 0xe6, name);
        let why = refusal(&bytes);
        assert!(
            why.contains("a WebP frame of 16×24 holding a picture of 24×24"),
            "{name:?}: {why:?}"
        );
    }
}
