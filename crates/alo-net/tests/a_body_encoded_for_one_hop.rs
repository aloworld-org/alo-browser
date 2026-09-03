/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Transfer-Encoding` that is not only `chunked` — queue item 153.
//!
//! Its closing condition: *"it decodes, or is refused by name — either is an
//! answer; handing up compressed bytes is not."* So every test here asserts one
//! of the two, and the one thing no test may end in is a body that came out
//! still compressed.
//!
//! The compressed fixtures are `tests/compressed/`'s, made by the `gzip` and
//! `brotli` tools rather than by the crates that read them — see the README
//! there. What this file adds around them is the **chunking**, because
//! `gzip, chunked` is a gzip stream cut into pieces and putting it back
//! together is the half that had never been tested.

use alo_net::body::{self, Framing};
use alo_net::http::read_head;
use alo_net::transfer;
use std::io::BufReader;

/// One of the frozen bodies.
fn frozen(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/compressed")
        .join(name);
    std::fs::read(path).unwrap_or_default()
}

/// What every fixture here decodes back to.
fn the_page() -> Vec<u8> {
    frozen("page.html")
}

/// A response saying `value`, carrying `body` cut into chunks of `each` bytes.
///
/// Small chunks on purpose: a stream that arrives in one chunk would not
/// notice a reader that de-chunked and decompressed in the wrong order, and
/// the wrong order is the bug this item is about.
fn chunked(value: &str, body: &[u8], each: usize) -> Vec<u8> {
    let mut bytes = format!("HTTP/1.1 200 OK\r\nTransfer-Encoding: {value}\r\n\r\n").into_bytes();
    for piece in body.chunks(each.max(1)) {
        bytes.extend_from_slice(format!("{:x}\r\n", piece.len()).as_bytes());
        bytes.extend_from_slice(piece);
        bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(b"0\r\n\r\n");
    bytes
}

/// A whole response, the way [`alo_net::exchange`] reads one: head, framing,
/// body, and then the transfer codings that were under the chunking.
///
/// This is the composition the item is about, and it is written out rather
/// than reached through a socket so that it can be run against bytes no server
/// would send.
fn read(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut source = BufReader::new(bytes);
    let head = read_head(&mut source).map_err(|why| why.why)?;
    let framing = Framing::of(head.status, &head.headers).map_err(|why| why.why)?;
    let body = body::read(&mut source, framing).map_err(|why| why.why)?;
    let transfer = transfer::of(&head.headers).map_err(|why| why.why)?;
    transfer::undo(body, &transfer).map_err(|why| why.why)
}

/// The refusal, or the reason a test wanted one and did not get it.
fn refusal(bytes: &[u8]) -> String {
    match read(bytes) {
        Err(why) => why,
        Ok(body) => format!("accepted, and handed up {} bytes", body.len()),
    }
}

// ---- it decodes ----

#[test]
fn a_gzip_stream_cut_into_chunks_arrives_as_the_page() {
    let body = read(&chunked("gzip, chunked", &frozen("page.html.gz"), 16))
        .expect("gzip under chunks is a body this engine reads");
    assert_eq!(body, the_page());
}

#[test]
fn brotli_under_chunks_arrives_as_the_page_too() {
    // Not a copy of the test above: `br` is the encoding this engine asks for
    // first, and it is one of the two that carry no checksum of their own.
    let body = read(&chunked("br, chunked", &frozen("page.html.br"), 7))
        .expect("brotli under chunks is a body this engine reads");
    assert_eq!(body, the_page());
}

#[test]
fn the_names_fold_case_because_a_header_value_is_not_case_sensitive() {
    let body = read(&chunked("GZIP, Chunked", &frozen("page.html.gz"), 32))
        .expect("a server shouting is still a server");
    assert_eq!(body, the_page());
}

#[test]
fn chunked_on_its_own_is_still_a_body_nobody_decompresses() {
    let body = read(&chunked("chunked", b"<p>hello</p>", 5)).expect("the ordinary case");
    assert_eq!(body, b"<p>hello</p>");
}

#[test]
fn identity_says_nothing_was_applied_rather_than_naming_a_coding() {
    // `identity` beside `chunked` is legal and means the chunks hold the page.
    let body =
        read(&chunked("identity, chunked", b"<p>hello</p>", 4)).expect("identity applies nothing");
    assert_eq!(body, b"<p>hello</p>");
}

// ---- or it is refused by name ----

#[test]
fn a_coding_this_engine_cannot_undo_is_refused_by_the_name_it_was_given() {
    // `compress` is a real transfer coding, is LZW, and is not one we rent.
    // The refusal has to name it: "the body was wrong" and "the body was
    // `compress`" are different amounts of help.
    let why = refusal(&chunked("compress, chunked", b"anything", 4));
    assert!(why.contains("does not read"), "{why}");
    assert!(why.contains("compress"), "{why}");
}

#[test]
fn a_coding_after_chunked_is_refused_because_nothing_could_have_applied_it() {
    let why = refusal(&chunked("chunked, gzip", b"anything", 4));
    assert!(why.contains("not the last"), "{why}");
}

#[test]
fn chunked_twice_is_refused() {
    // The shape a smuggling attempt takes when it is aimed at a recipient that
    // de-chunks once and one that de-chunks twice.
    let why = refusal(&chunked("chunked, chunked", b"anything", 4));
    assert!(why.contains("not the last"), "{why}");
}

#[test]
fn a_transfer_coded_body_that_is_not_ended_by_chunked_is_refused() {
    // Legal, and the standard says the body then ends when the connection
    // does. A compressed body delimited by a network event cannot be told from
    // one an attacker cut short, and brotli carries no checksum that would.
    let why = refusal(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: br\r\n\r\nnot really brotli");
    assert!(why.contains("not ended by chunked"), "{why}");
}

#[test]
fn an_empty_coding_in_the_list_is_refused() {
    for value in ["chunked,", ", chunked", "gzip,, chunked"] {
        let why = refusal(&chunked(value, b"anything", 4));
        assert!(why.contains("empty transfer coding"), "{value:?}: {why}");
    }
}

#[test]
fn a_list_longer_than_this_engine_will_undo_is_refused() {
    let why = refusal(&chunked("gzip, gzip, gzip, gzip, chunked", b"anything", 4));
    assert!(why.contains("times over"), "{why}");
}

#[test]
fn two_transfer_encoding_headers_are_refused_rather_than_joined() {
    // The standard joins them with a comma. The order of separate field lines
    // is what an intermediary is most likely to have changed, and the order is
    // what says which coding comes off first — so this engine refuses instead.
    let why = refusal(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n",
    );
    assert!(why.contains("more than one"), "{why}");
}

#[test]
fn a_length_beside_a_transfer_encoding_is_still_refused() {
    let why = refusal(
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nTransfer-Encoding: gzip, chunked\r\n\r\nhello",
    );
    assert!(why.contains("different things"), "{why}");
}

// ---- and the bytes are hostile ----

#[test]
fn chunks_that_hold_something_that_is_not_gzip_are_an_error_rather_than_a_page() {
    let why = refusal(&chunked(
        "gzip, chunked",
        b"<p>not compressed at all</p>",
        6,
    ));
    assert!(!why.starts_with("accepted"), "{why}");
}

#[test]
fn a_gzip_stream_with_a_byte_flipped_in_it_is_an_error() {
    let mut corrupt = frozen("page.html.gz");
    // Past the header, so it is the compressed data rather than the format.
    if let Some(byte) = corrupt.get_mut(40) {
        *byte ^= 0xff;
    }
    let why = refusal(&chunked("gzip, chunked", &corrupt, 16));
    assert!(!why.starts_with("accepted"), "{why}");
}

#[test]
fn chunks_that_stop_halfway_through_a_gzip_stream_are_an_error() {
    let whole = frozen("page.html.gz");
    let half = whole.get(..whole.len() / 2).unwrap_or_default().to_vec();
    let why = refusal(&chunked("gzip, chunked", &half, 16));
    assert!(!why.starts_with("accepted"), "{why}");
}

#[test]
fn a_bomb_under_the_chunking_is_bounded_the_same_way_as_one_that_is_not() {
    // Eight kibibytes of chunks that decode to eight mebibytes. It is allowed
    // through — the bound is [`alo_net::body::LARGEST_BODY`] and this is well
    // under it — and the point of the test is that the *chunked* path reaches
    // the same bounded decoder rather than a second one somebody wrote here.
    let body = read(&chunked("gzip, chunked", &frozen("bomb.gz"), 512))
        .expect("a body this size is under the bound");
    assert_eq!(body.len(), 8 * 1024 * 1024);
    assert!(body.iter().all(|byte| *byte == 0));
}

#[test]
fn nothing_a_server_can_send_makes_this_panic() {
    // Every hostile shape at once, against the whole read: a head, a framing,
    // a de-chunking and a decoding. Each must come back as a refusal or a
    // body, and neither is allowed to be a crash.
    let values = [
        "",
        ",",
        " ",
        "chunked chunked",
        "chunked;q=1",
        "gzip, chunked, gzip",
        "\u{fffd}, chunked",
        "chunked, ",
        "gzip,chunked",
        &"gzip, ".repeat(64),
    ];
    for value in values {
        for body in [&b""[..], b"\0\0\0\0", b"garbage", &frozen("page.html.gz")] {
            let bytes = chunked(value, body, 3);
            let _ = read(&bytes);
        }
    }
}
