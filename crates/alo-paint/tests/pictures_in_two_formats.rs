//! A picture in whatever format it turns out to be, and the same bounds either
//! way.
//!
//! The reason JPEG and PNG are one item rather than two: a second decoder with
//! its own limits, or none, would be a second way in. Every test that matters
//! here is run against **both** formats, from one list, so that adding a third
//! means adding it to the list rather than remembering to.

use alo_paint::picture::{Format, read};

/// The frozen pictures beside the corpus case, which is where the real files
/// live rather than being duplicated here.
fn frozen(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../alo-corpus/cases/a-picture")
        .join(name);
    std::fs::read(path).unwrap_or_default()
}

/// Both formats, as the same picture, so a test can say "either of these".
fn both() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("PNG", frozen("stripes.png")),
        ("JPEG", frozen("stripes.jpg")),
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
    assert_eq!(Format::of(b"neither of those"), None);
    assert_eq!(Format::of(&[]), None);

    // The corpus case has this same file under a `.png` name, and it decodes.
    let named_wrong = frozen("stripes.jpg");
    assert_eq!(Format::of(&named_wrong), Some(Format::Jpeg));
    assert!(read(&named_wrong).is_ok());
}

#[test]
fn a_format_this_engine_does_not_read_is_refused_rather_than_attempted() {
    let why = read(b"GIF89a and then some bytes")
        .err()
        .map(|why| why.to_string())
        .unwrap_or_default();
    assert!(why.contains("no picture format"), "{why:?}");
}

// --- Both formats, the same picture ------------------------------------------

#[test]
fn both_formats_come_back_the_same_size() {
    for (name, bytes) in both() {
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
fn both_formats_come_back_the_right_way_up() {
    for (name, bytes) in both() {
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

// --- The same refusals, either way -------------------------------------------

#[test]
fn neither_format_reads_bytes_that_stop_in_the_middle_into_a_different_size() {
    for (name, whole) in both() {
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

#[test]
fn neither_format_decodes_a_corrupt_file_into_a_different_size() {
    for (name, whole) in both() {
        let mut refused = 0;
        for at in (4..whole.len()).step_by(7) {
            let mut broken = whole.clone();
            if let Some(byte) = broken.get_mut(at) {
                *byte ^= 0xff;
            }
            match read(&broken) {
                Ok(canvas) => assert_eq!(
                    (canvas.width(), canvas.height()),
                    (24, 24),
                    "{name}: a corrupt file decoded to a different size"
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
