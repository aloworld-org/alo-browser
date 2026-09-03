//! Which encoding a page is in, when the page, the server and the bytes
//! disagree — which they frequently do.
//!
//! The order is HTML's and each step exists because the one before it can be
//! absent or wrong. A browser that assumed UTF-8 would render a great many
//! real pages as mojibake, and one that believed the first thing it was told
//! would render a smaller but more baffling set.

use alo_net::encoding::{Source, decode, sniff};
use alo_net::media_type::MediaType;

/// A `Content-Type`, or one that says nothing when the text is not one —
/// which no case here is, and an assertion rather than a panic in a helper.
fn header(text: &str) -> MediaType {
    MediaType::parse(text).unwrap_or_else(|| MediaType {
        kind: "application".to_owned(),
        subtype: "octet-stream".to_owned(),
        parameters: Vec::new(),
    })
}

#[test]
fn a_byte_order_mark_beats_everything_anybody_said() {
    // It is in the bytes rather than in a claim, so it wins even against a
    // server that says something else.
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice("héllo".as_bytes());

    let lied_to = header("text/html; charset=windows-1252");
    let read = decode(&bytes, Some(&lied_to));
    assert_eq!(read.encoding.source, Source::ByteOrderMark);
    assert_eq!(read.text, "héllo", "and the mark itself is not in the text");
}

#[test]
fn the_header_is_believed_when_there_is_no_mark() {
    // `é` as one byte, which is windows-1252 and is not valid UTF-8.
    let bytes = b"h\xe9llo";
    let read = decode(bytes, Some(&header("text/html; charset=windows-1252")));
    assert_eq!(read.encoding.source, Source::Header);
    assert_eq!(read.text, "héllo");
    assert!(!read.had_errors, "read as what it actually is");
}

#[test]
fn a_meta_in_the_document_answers_when_the_server_did_not() {
    let page = b"<!DOCTYPE html><html><head><meta charset=\"windows-1252\">\
</head><body>h\xe9llo</body></html>";
    let read = decode(page, None);
    assert_eq!(read.encoding.source, Source::Meta);
    assert!(read.text.contains("héllo"), "{}", read.text);
}

#[test]
fn the_older_spelling_of_a_meta_is_read_too() {
    // Pages written before 2010 say it this way, and there are a great many
    // of them still up.
    let page = b"<html><head><meta http-equiv=\"Content-Type\" \
content=\"text/html; charset=iso-8859-1\"></head><body>h\xe9llo</body></html>";
    let read = decode(page, None);
    assert_eq!(read.encoding.source, Source::Meta);
    assert!(read.text.contains("héllo"), "{}", read.text);
}

#[test]
fn a_page_that_says_nothing_is_utf_8_because_that_is_what_the_web_is() {
    let read = decode("héllo".as_bytes(), None);
    assert_eq!(read.encoding.source, Source::Default);
    assert_eq!(read.encoding.label, "UTF-8");
    assert_eq!(read.text, "héllo");
}

#[test]
fn a_meta_past_the_first_kilobyte_is_not_a_declaration() {
    // HTML only looks at the beginning, and so does this — otherwise the word
    // `charset` inside a page's own prose would change how it is read.
    let mut page = b"<html><body>".to_vec();
    page.extend(std::iter::repeat_n(b'x', 1100));
    page.extend_from_slice(b"<meta charset=\"windows-1252\">");
    let read = decode(&page, None);
    assert_eq!(read.encoding.source, Source::Default, "too late to count");
}

#[test]
fn a_label_nobody_has_heard_of_falls_through_rather_than_guessing() {
    // The header says something meaningless; the next step of the algorithm
    // answers instead of the whole load failing.
    let page = b"<html><head><meta charset=\"utf-8\"></head><body>h\xc3\xa9llo</body></html>";
    let read = decode(page, Some(&header("text/html; charset=not-an-encoding")));
    assert_eq!(read.encoding.source, Source::Meta);
    assert!(read.text.contains("héllo"), "{}", read.text);
}

#[test]
fn a_page_mislabelled_still_reads_and_says_that_it_did_not_read_cleanly() {
    // The half a browser must never hide: bytes that are not what they claim
    // become replacement characters, and the fact is kept so somebody can
    // find out why a page looks wrong.
    let read = decode(b"h\xe9llo", Some(&header("text/html; charset=utf-8")));
    assert!(read.had_errors, "0xe9 is not valid UTF-8");
    assert!(read.text.contains('\u{fffd}'), "{}", read.text);
    assert!(read.text.starts_with('h') && read.text.ends_with("llo"));
}

#[test]
fn the_two_sixteen_bit_marks_are_told_apart() {
    for (bytes, label) in [
        (vec![0xFE, 0xFF, 0x00, 0x68], "UTF-16BE"),
        (vec![0xFF, 0xFE, 0x68, 0x00], "UTF-16LE"),
    ] {
        let read = sniff(&bytes, None);
        assert_eq!(read.label, label);
        assert_eq!(read.source, Source::ByteOrderMark);
    }
    assert_eq!(decode(&[0xFF, 0xFE, 0x68, 0x00], None).text, "h");
}

#[test]
fn no_arrangement_of_bytes_makes_the_sniffer_panic() {
    let cases: Vec<Vec<u8>> = vec![
        Vec::new(),
        vec![0xEF],
        vec![0xEF, 0xBB],
        vec![0xFF],
        b"<meta charset=".to_vec(),
        b"<meta charset=\"".to_vec(),
        b"charset".to_vec(),
        b"charset=".to_vec(),
        b"charset=====".to_vec(),
        vec![0xFF; 4096],
        b"charset=\xff\xfe\xff\xfe".to_vec(),
        std::iter::repeat_n(b'c', 100_000).collect(),
    ];
    for bytes in &cases {
        let read = decode(bytes, None);
        let _ = read.text.len();
        let _ = sniff(bytes, Some(&header("text/html; charset=utf-8")));
    }
}
