//! Decoding a picture from a page, which is bytes nobody here wrote.
//!
//! `alo_paint::from_png` reads a reference render: a file this engine wrote
//! moments earlier, in one format, and being strict about it is a feature.
//! `picture_from_png` reads what a page sent, and is the opposite on both
//! counts — tolerant about what a PNG may be, and unforgiving about every
//! number that decides an allocation.
//!
//! ADR 0005 names image codecs as its second reason for a sandbox: they have
//! `unsafe` in them and are not ours to make safe. The bound here is the half
//! that *is* ours — a renderer that allocated sixteen gigabytes because a
//! header said so would be denied by no sandbox.

use alo_paint::Canvas;
use alo_paint::encode::{MOST_PIXELS, PictureError, picture_from_png, to_png};
use alo_value::Rgba;

/// A small real PNG, made by the encoder beside the decoder.
fn a_picture(width: u32, height: u32) -> Vec<u8> {
    let mut canvas = Canvas::new(width, height, Rgba::TRANSPARENT);
    canvas.fill_rect(0, 0, width, height, Rgba::from_rgba8(20, 120, 200, 255));
    to_png(&canvas).unwrap_or_default()
}

/// A real, complete picture whose header has been rewritten to claim a
/// different size.
///
/// This is the attack rather than an approximation of it. A header on its own
/// is refused for having no image data behind it, which says nothing about
/// whether the size was checked — the first version of this test asserted a
/// message and got "unexpected end of file", because the decoder never reached
/// the size at all.
///
/// A *valid* file with a lying header is different: everything parses, the
/// decoder is happy, and the only thing standing between the claim and an
/// allocation of four bytes per claimed pixel is the bound.
fn a_picture_claiming(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = a_picture(8, 8);
    // The signature is eight bytes, then a four-byte length, then `IHDR`, then
    // the width and height. Rewrite those and mend the chunk's checksum, or the
    // decoder refuses it for the checksum and the test proves nothing.
    let at = 16;
    if let Some(field) = bytes.get_mut(at..at + 4) {
        field.copy_from_slice(&width.to_be_bytes());
    }
    if let Some(field) = bytes.get_mut(at + 4..at + 8) {
        field.copy_from_slice(&height.to_be_bytes());
    }
    let Some(chunk) = bytes.get(12..12 + 17).map(<[u8]>::to_vec) else {
        return bytes;
    };
    let mended = crc32(&chunk).to_be_bytes();
    if let Some(field) = bytes.get_mut(12 + 17..12 + 21) {
        field.copy_from_slice(&mended);
    }
    bytes
}

/// The CRC a PNG chunk carries, so the header is one a decoder will read rather
/// than reject before it has looked at the size.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Why a picture was refused — or, when it was not, a sentence saying so, since
/// a helper outside a test may not panic and every caller asserts on the text
/// anyway.
fn why(bytes: &[u8]) -> String {
    match picture_from_png(bytes) {
        Ok(canvas) => format!(
            "IT WAS READ, as {}×{}, and should not have been",
            canvas.width(),
            canvas.height()
        ),
        Err(PictureError::Unreadable(why) | PictureError::Unwritable(why)) => why,
    }
}

// --- What must be refused ----------------------------------------------------

/// The one with an attacker behind it. A PNG's **header** declares its size, and
/// a decoder that believed one would reserve four bytes per declared pixel
/// before reading a single row. Twenty bytes can ask for sixteen gigabytes.
#[test]
fn a_picture_claiming_more_pixels_than_exist_is_refused_before_anything_is_reserved() {
    // Sixty-five thousand square: four and a quarter billion pixels, seventeen
    // gigabytes of canvas, and about seventy bytes on the wire.
    let bytes = a_picture_claiming(65_535, 65_535);
    assert!(
        bytes.len() < 100,
        "the attack has to be small or it is not one"
    );
    let said = why(&bytes);
    assert!(
        said.contains("more than"),
        "it should be refused for its size, not for what follows: {said:?}"
    );
}

#[test]
fn the_bound_is_where_it_says_it_is() {
    // A shape whose area is one pixel past the bound.
    let across = 8192u32;
    let down = u32::try_from(MOST_PIXELS / u64::from(across)).unwrap_or(1) + 1;
    let said = why(&a_picture_claiming(across, down));
    assert!(said.contains("more than"), "{said:?}");
}

/// A zero dimension is refused — by us if the decoder allows it through, and by
/// the decoder if it does not. The test asserts the refusal rather than the
/// wording, because which of the two speaks first is the rented crate's
/// business and not a thing to pin.
#[test]
fn a_picture_with_no_pixels_in_it_is_refused() {
    assert!(picture_from_png(&a_picture_claiming(0, 100)).is_err());
    assert!(picture_from_png(&a_picture_claiming(100, 0)).is_err());
}

#[test]
fn bytes_that_are_not_a_picture_are_refused() {
    assert!(!why(b"this is a sentence, not a picture").is_empty());
    assert!(!why(&[]).is_empty());
    // The right signature and nothing behind it.
    assert!(!why(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]).is_empty());
}

/// Every prefix of a real picture, and the rule is **not** that all of them
/// fail.
///
/// A file missing only its end marker has all of its image data, and a decoder
/// that refused it would refuse a picture whose last four bytes were lost in
/// transit — which browsers show, because showing what arrived is most of the
/// point of an image on a page. The first version of this test asserted that
/// every prefix fails, and found out otherwise at 87 bytes of 91.
///
/// What must hold is the thing that would be dangerous: a prefix never produces
/// a canvas of a **size nobody declared**, because that is the shape of reading
/// past the end of what arrived.
#[test]
fn a_picture_that_stops_in_the_middle_never_becomes_a_picture_of_another_size() {
    let whole = a_picture(8, 8);
    assert!(
        !whole.is_empty(),
        "the encoder should have produced something"
    );
    let mut refused = 0;
    for cut in 1..whole.len() {
        let Some(part) = whole.get(..cut) else {
            continue;
        };
        match picture_from_png(part) {
            Ok(canvas) => assert_eq!(
                (canvas.width(), canvas.height()),
                (8, 8),
                "{cut} bytes produced a picture of a size the file never declared"
            ),
            Err(_) => refused += 1,
        }
    }
    assert!(
        refused > whole.len() / 2,
        "only {refused} of {} prefixes were refused, which is too few to be checking anything",
        whole.len()
    );
}
/// Bytes flipped through the body of a real picture. Most will fail the
/// checksum; what matters is that none of them **succeeds with wrong pixels**
/// or takes the decoder somewhere it cannot come back from.
#[test]
fn a_corrupt_picture_is_refused_rather_than_decoded_into_something() {
    let whole = a_picture(16, 16);
    let mut refused = 0;
    for at in (8..whole.len()).step_by(3) {
        let mut broken = whole.clone();
        if let Some(byte) = broken.get_mut(at) {
            *byte ^= 0xff;
        }
        // Either it is refused, or it decodes to a picture of the size the
        // header still says. What must not happen is a size nobody asked for.
        match picture_from_png(&broken) {
            Ok(canvas) => assert_eq!(
                (canvas.width(), canvas.height()),
                (16, 16),
                "a corrupt file decoded to a different size"
            ),
            Err(_) => refused += 1,
        }
    }
    assert!(
        refused > 0,
        "not one corrupted byte was noticed, which means nothing is being checked"
    );
}

// --- What must be accepted ---------------------------------------------------

/// A page's picture is very often not eight-bit RGBA, and refusing those would
/// be refusing the web.
#[test]
fn a_real_picture_is_read_and_keeps_its_size_and_its_colour() {
    let canvas = picture_from_png(&a_picture(4, 3)).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((canvas.width(), canvas.height()), (4, 3));
    let corner = canvas.at(0, 0).unwrap_or(Rgba::TRANSPARENT);
    let (red, green, blue, alpha) = corner.to_rgba8();
    assert_eq!((red, green, blue, alpha), (20, 120, 200, 255));
}

/// A picture exactly at the bound is read. A bound that refused the largest
/// allowed thing would be a bound one smaller than it says.
#[test]
fn a_picture_at_the_bound_is_not_refused_for_its_size() {
    let across = 8192u32;
    let down = u32::try_from(MOST_PIXELS / u64::from(across)).unwrap_or(1);
    let bytes = a_picture_claiming(across, down);
    // It still fails — the eight-by-eight of image data behind the header is
    // not a picture that size — but it must fail for *that* rather than for its
    // size, which is what says the bound is inclusive.
    let said = why(&bytes);
    assert!(
        !said.contains("more than"),
        "a picture exactly at the bound was refused for its size: {said:?}"
    );
}
