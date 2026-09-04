/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a server compressed, and what it hopes we will not notice.
//!
//! Every fixture here was made by an implementation that is not ours — the
//! `gzip`, `brotli` and `zstd` command-line tools, and Python's `zlib`. A suite
//! that compressed with `flate2` and then decompressed with `flate2` would
//! prove that one crate agrees with itself, which is not the question anybody
//! is asking. See `tests/compressed/README.md` for how to re-derive each one.

use alo_net::cause::{Cause, Identities};
use alo_net::decompress::{Encoding, undo, undo_within, what_was_applied};
use alo_net::{Headers, Request, http};

/// What caused every request in this file: a person, in a tab of their own.
///
/// ADR 0012 § 1 makes the cause an argument rather than something a caller may
/// forget, so a test has to say what it means too — and what these mean is
/// somebody opening a page. The same tab each time, because it is one person.
fn a_person() -> Cause {
    Cause::Person {
        tab: Identities::default().a_tab(),
    }
}

/// One of the frozen bodies.
fn frozen(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/compressed")
        .join(name);
    std::fs::read(path).unwrap_or_default()
}

/// What every fixture in this file decodes back to.
fn the_page() -> Vec<u8> {
    frozen("page.html")
}

/// A URL, or a placeholder that is not one — because `expect` outside a test
/// function is denied, and this helper is not a test function.
fn url(text: &str) -> alo_url::Url {
    alo_url::parse(text).unwrap_or_else(|_| alo_url::Url {
        scheme: "about".to_owned(),
        host: None,
        port: None,
        path: "not-a-url".to_owned(),
        query: None,
        fragment: None,
        serialised: "about:not-a-url".to_owned(),
    })
}

fn saying(value: &str) -> Headers {
    let mut headers = Headers::new();
    headers.add("Content-Encoding", value);
    headers
}

#[test]
fn gzip_from_the_gzip_tool_decodes_to_the_page() {
    let applied = what_was_applied(&saying("gzip")).expect("gzip is an encoding we undo");
    assert_eq!(applied, vec![Encoding::Gzip]);
    assert_eq!(
        undo(frozen("page.html.gz"), &applied).expect("a real gzip stream decodes"),
        the_page()
    );
}

#[test]
fn brotli_from_the_brotli_tool_decodes_to_the_page() {
    let applied = what_was_applied(&saying("br")).expect("br is an encoding we undo");
    assert_eq!(
        undo(frozen("page.html.br"), &applied).expect("a real brotli stream decodes"),
        the_page()
    );
}

#[test]
fn zstd_from_the_zstd_tool_decodes_to_the_page() {
    let applied = what_was_applied(&saying("zstd")).expect("zstd is an encoding we undo");
    assert_eq!(
        undo(frozen("page.html.zst"), &applied).expect("a real zstd stream decodes"),
        the_page()
    );
}

/// `deflate` is two formats sharing one name: the specification says zlib, and
/// a meaningful number of servers send raw DEFLATE because an early and popular
/// one did. Both have to work, so both are frozen and both are asserted.
#[test]
fn deflate_means_zlib_and_also_means_raw_and_both_decode() {
    let applied = what_was_applied(&saying("deflate")).expect("deflate is an encoding we undo");
    assert_eq!(
        undo(frozen("page.html.zz"), &applied).expect("zlib-wrapped deflate decodes"),
        the_page(),
        "the spelling the specification asks for"
    );
    assert_eq!(
        undo(frozen("page.html.deflate"), &applied).expect("raw deflate decodes"),
        the_page(),
        "the spelling a great many servers actually send"
    );
}

/// The bound in `decompress.rs` is on what comes **out**, and this is the test
/// that says why that is a different bound from every other one in the crate.
/// Eight kibibytes arrive. A limit that watched what arrived would let all of
/// them through and then hold eight mebibytes.
#[test]
fn a_body_that_is_small_on_the_wire_and_enormous_out_is_refused() {
    let bomb = frozen("bomb.gz");
    assert!(
        bomb.len() < 16 * 1024,
        "the fixture has to be small on the wire or it is not the attack"
    );
    let applied = vec![Encoding::Gzip];
    let refused = undo_within(bomb.clone(), &applied, 64 * 1024);
    let why = match refused {
        Ok(out) => panic!("a thousand-to-one bomb decoded into {} bytes", out.len()),
        Err(why) => why.why,
    };
    assert!(
        why.contains("decodes to more than"),
        "the refusal should say what happened, and said {why:?}"
    );
    // And the same bytes are fine when there is room for them: the limit is a
    // limit rather than a dislike of gzip.
    assert_eq!(
        undo_within(bomb, &applied, 16 * 1024 * 1024)
            .expect("with room, it is just a long run of zeroes")
            .len(),
        8 * 1024 * 1024
    );
}

/// Only the formats that carry an integrity check can promise this: gzip a
/// CRC32, zlib an Adler-32, zstd an XXH64. Raw DEFLATE and brotli carry none,
/// so a corruption that leaves a structurally valid stream is undetectable in
/// any implementation — which is why brotli is tested for a mislabelled body
/// but not for a flipped byte.
#[test]
fn a_stream_that_is_not_what_it_said_is_refused_rather_than_decoded_into_rubbish() {
    for (name, fixture) in [
        ("gzip", "page.html.gz"),
        ("deflate", "page.html.zz"),
        ("zstd", "page.html.zst"),
    ] {
        let applied = what_was_applied(&saying(name)).unwrap_or_default();
        // The page itself, uncompressed, labelled as though it were not.
        assert!(
            undo(the_page(), &applied).is_err(),
            "{name} accepted a body that was never {name}"
        );
        // And a real stream with a byte flipped in the middle of it. This is
        // what the checksum is for.
        let mut corrupt = frozen(fixture);
        let middle = corrupt.len() / 2;
        if let Some(byte) = corrupt.get_mut(middle) {
            *byte ^= 0xff;
        }
        assert!(
            undo(corrupt, &applied).is_err(),
            "{name} decoded a corrupt stream instead of refusing it"
        );
    }
}

#[test]
fn a_stream_that_stops_in_the_middle_is_refused() {
    for (name, fixture) in [
        ("gzip", "page.html.gz"),
        ("br", "page.html.br"),
        ("zstd", "page.html.zst"),
        ("deflate", "page.html.zz"),
    ] {
        let applied = what_was_applied(&saying(name)).unwrap_or_default();
        let whole = frozen(fixture);
        let mut truncated = whole.clone();
        truncated.truncate(whole.len() / 2);
        assert!(
            undo(truncated, &applied).is_err(),
            "{name} treated half a stream as a whole one"
        );
    }
}

/// An encoding we cannot undo has to be an error. Handing the compressed bytes
/// up as though they were the page renders rubbish, and rubbish chosen by
/// whoever sent it.
#[test]
fn an_encoding_this_engine_does_not_know_is_refused_rather_than_passed_through() {
    let refused = what_was_applied(&saying("exi"));
    let why = match refused {
        Ok(applied) => panic!("an unknown encoding parsed as {applied:?}"),
        Err(why) => why.why,
    };
    assert!(
        why.contains("exi"),
        "the refusal should name it, and said {why:?}"
    );
}

#[test]
fn identity_is_a_thing_a_server_may_say_and_means_no_op() {
    let applied = what_was_applied(&saying("identity")).expect("identity is legal");
    assert_eq!(applied, vec![Encoding::Identity]);
    assert_eq!(
        undo(the_page(), &applied).expect("identity does nothing"),
        the_page()
    );
}

#[test]
fn no_content_encoding_at_all_is_no_encodings() {
    assert_eq!(
        what_was_applied(&Headers::new()).expect("saying nothing is legal"),
        Vec::new()
    );
}

/// `Content-Encoding` is a list, and the order is the order it was applied —
/// so it is undone backwards. A test that only ever used one encoding would
/// pass with the loop written either way round.
#[test]
fn two_encodings_are_undone_in_the_opposite_order_from_the_one_they_were_applied() {
    // The gzip fixture, brotli'd on top of it, is `Content-Encoding: gzip, br`.
    let applied = what_was_applied(&saying("gzip, br")).expect("both halves are encodings we undo");
    assert_eq!(applied, vec![Encoding::Gzip, Encoding::Brotli]);
    assert_eq!(
        undo(frozen("page.html.gz.br"), &applied).expect("both layers come off"),
        the_page()
    );
    // Backwards, it is a brotli stream fed to gzip, which is refused rather
    // than decoded into anything.
    assert!(
        undo(
            frozen("page.html.gz.br"),
            &[Encoding::Brotli, Encoding::Gzip]
        )
        .is_err(),
        "undoing in the wrong order produced something"
    );
}

/// A repeated header and a comma-separated one say the same thing, and a
/// server may use either.
#[test]
fn a_repeated_header_is_the_same_list_as_a_comma_separated_one() {
    let mut repeated = Headers::new();
    repeated.add("Content-Encoding", "gzip");
    repeated.add("Content-Encoding", "br");
    assert_eq!(
        what_was_applied(&repeated).expect("both are encodings we undo"),
        what_was_applied(&saying("gzip, br")).expect("both are encodings we undo")
    );
}

/// A list is its own bomb: each layer is bounded on its own, but the work is
/// that bound times the length of the list.
#[test]
fn a_body_compressed_more_times_than_this_engine_will_undo_is_refused() {
    let refused = what_was_applied(&saying("gzip, gzip, gzip, gzip, gzip"));
    assert!(
        refused.is_err(),
        "five layers of gzip was accepted as a reasonable thing for a server to send"
    );
}

/// Asking is the other half: a server sends `br` because we said we read it.
#[test]
fn a_request_says_which_encodings_this_engine_reads() {
    let url = url("https://example.com/");
    let sent = String::from_utf8(http::write_request(&Request::get(url.clone(), a_person())))
        .unwrap_or_default();
    assert!(
        sent.contains("Accept-Encoding: br, zstd, gzip, deflate\r\n"),
        "the request did not ask for anything: {sent:?}"
    );

    // But a caller who asked for something else keeps it. A download that
    // resumes wants `identity`, because a byte range of a compressed stream is
    // a range of bytes nobody can decompress.
    let mut request = Request::get(url, a_person());
    request.headers.add("Accept-Encoding", "identity");
    let sent = String::from_utf8(http::write_request(&request)).unwrap_or_default();
    assert!(
        sent.contains("Accept-Encoding: identity\r\n"),
        "the caller's choice was overwritten: {sent:?}"
    );
    assert_eq!(
        sent.matches("Accept-Encoding").count(),
        1,
        "the request asked twice: {sent:?}"
    );
}

/// Named on its own because it is a defect this engine has and the rented crate
/// does not fix. `ruzstd` computes a frame's checksum and reads the one the
/// frame carries and compares them for nobody, so a zstd body with a flipped
/// byte decoded into rubbish and reported success. Deleting the comparison in
/// `decompress.rs` makes this test fail and nothing else.
#[test]
fn a_zstd_body_that_does_not_match_its_own_checksum_is_refused() {
    let mut corrupt = frozen("page.html.zst");
    let middle = corrupt.len() / 2;
    if let Some(byte) = corrupt.get_mut(middle) {
        *byte ^= 0xff;
    }
    let refused = undo(corrupt, &[Encoding::Zstd]);
    let why = match refused {
        Ok(out) => panic!("a corrupt zstd frame decoded into {} bytes", out.len()),
        Err(why) => why.why,
    };
    assert!(
        why.contains("checksum"),
        "the refusal should say the checksum did not match, and said {why:?}"
    );
}
