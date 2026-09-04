/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! HTTP/1.1 responses, read from byte streams that never came over a socket.
//!
//! Queue item 53's closing condition — *"a frozen page's own byte stream
//! replays through it identically, and a truncated response is an error rather
//! than a short page"* — plus the readings that are **almost** right, which is
//! where nearly every famous HTTP bug lives.

use alo_net::body::{self, Framing};
use alo_net::cause::{Cause, Identities};
use alo_net::http::{read_head, write_request};
use alo_net::{Purpose, Request, Status};
use core::fmt::Write as _;
use std::io::BufReader;

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

/// Read a whole response — head and body — out of bytes.
fn read(bytes: &[u8]) -> Result<(Status, Vec<u8>), String> {
    let mut source = BufReader::new(bytes);
    let head = read_head(&mut source).map_err(|why| why.to_string())?;
    let framing = Framing::of(head.status, &head.headers).map_err(|why| why.to_string())?;
    let body = body::read(&mut source, framing).map_err(|why| why.to_string())?;
    Ok((head.status, body))
}

#[test]
fn a_response_with_a_length_replays_exactly() {
    let (status, body) = read(
        b"HTTP/1.1 200 OK\r\n\
          Content-Type: text/html; charset=utf-8\r\n\
          Content-Length: 12\r\n\
          \r\n\
          <p>hello</p>",
    )
    .expect("a response");
    assert_eq!(status, Status::OK);
    assert_eq!(body, b"<p>hello</p>");
}

#[test]
fn a_chunked_response_replays_as_the_bytes_it_adds_up_to() {
    let (_, body) = read(
        b"HTTP/1.1 200 OK\r\n\
          Transfer-Encoding: chunked\r\n\
          \r\n\
          5\r\nhello\r\n\
          7\r\n, world\r\n\
          0\r\n\r\n",
    )
    .expect("a response");
    assert_eq!(body, b"hello, world");
}

#[test]
fn a_chunk_may_carry_an_extension_nobody_reads() {
    let (_, body) = read(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n\
          5;something=else\r\nhello\r\n0\r\n\r\n",
    )
    .expect("a response");
    assert_eq!(body, b"hello");
}

#[test]
fn trailers_after_the_last_chunk_are_read_past_rather_than_left_behind() {
    let (_, body) = read(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n\
          2\r\nhi\r\n0\r\nExpires: never\r\n\r\n",
    )
    .expect("a response");
    assert_eq!(body, b"hi");
}

#[test]
fn a_response_nobody_framed_is_read_until_the_connection_closes() {
    let (_, body) =
        read(b"HTTP/1.1 200 OK\r\n\r\neverything after the blank line").expect("a response");
    assert_eq!(body, b"everything after the blank line");
}

#[test]
fn a_truncated_body_is_an_error_rather_than_a_short_page() {
    // The half of the closing condition that matters. A browser that showed
    // the first half of a bank statement and said nothing would be worse than
    // one that showed nothing.
    let answer = read(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nonly this much");
    match answer {
        Err(why) => assert!(why.contains("stopped after 14"), "{why}"),
        Ok((_, body)) => panic!("a short body was accepted: {body:?}"),
    }
}

#[test]
fn a_chunked_body_that_stops_in_the_middle_is_an_error() {
    let answer = read(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhel");
    assert!(answer.is_err(), "{answer:?}");
}

#[test]
fn the_statuses_that_have_no_body_do_not_get_one() {
    // A `204` with a `Content-Length` is a server being wrong, and a parser
    // that believed it would read the next response as this one's body.
    for status in [204u16, 304, 100] {
        let bytes = format!("HTTP/1.1 {status} Something\r\nContent-Length: 5\r\n\r\nhello");
        let (read_status, body) = read(bytes.as_bytes()).expect("a response");
        assert_eq!(read_status.0, status);
        assert!(body.is_empty(), "{status} should have no body");
    }
}

// ---- the readings that are almost right ----

#[test]
fn two_content_lengths_that_disagree_are_refused() {
    // Request smuggling, in its plainest form: this parser and the proxy in
    // front of it would disagree about where this response ends.
    let answer = read(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Length: 100\r\n\r\nhello");
    match answer {
        Err(why) => assert!(why.contains("disagree"), "{why}"),
        Ok(_) => panic!("two lengths were accepted"),
    }
}

#[test]
fn two_content_lengths_that_agree_are_a_server_being_odd_rather_than_an_attack() {
    let (_, body) = read(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Length: 5\r\n\r\nhello")
        .expect("a response");
    assert_eq!(body, b"hello");
}

#[test]
fn a_length_and_an_encoding_together_are_refused() {
    let answer =
        read(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nTransfer-Encoding: chunked\r\n\r\nhello");
    match answer {
        Err(why) => assert!(why.contains("different things"), "{why}"),
        Ok(_) => panic!("both framings were accepted"),
    }
}

#[test]
fn a_space_before_the_colon_is_refused() {
    // Some parsers accept it and some do not, and a chain containing both is
    // a smuggling chain.
    let answer = read(b"HTTP/1.1 200 OK\r\nContent-Length : 5\r\n\r\nhello");
    match answer {
        Err(why) => assert!(why.contains("space before the colon"), "{why}"),
        Ok(_) => panic!("a space before the colon was accepted"),
    }
}

#[test]
fn a_header_continued_onto_the_next_line_is_refused() {
    // Removed from the standard in 2014, for this reason.
    let answer = read(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\tand more\r\n\r\nhello");
    match answer {
        Err(why) => assert!(why.contains("continued"), "{why}"),
        Ok(_) => panic!("a folded header was accepted"),
    }
}

#[test]
fn a_transfer_encoding_this_engine_does_not_read_is_refused() {
    // `gzip, chunked` used to be this test's example and is now a body this
    // engine reads — queue item 153, and `a_body_encoded_for_one_hop.rs` is
    // where the whole of that header now lives. `compress` is LZW and is not
    // one we rent, so it is still the answer here.
    let answer = read(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: compress, chunked\r\n\r\nx");
    assert!(answer.is_err(), "{answer:?}");
}

#[test]
fn what_is_not_a_status_line_is_refused() {
    for bytes in [
        &b""[..],
        b"\r\n",
        b"not http at all\r\n\r\n",
        b"HTTP/1.1\r\n\r\n",
        b"HTTP/1.1 20 OK\r\n\r\n",
        b"HTTP/1.1 2000 OK\r\n\r\n",
        b"HTTP/1.1 abc OK\r\n\r\n",
        b"HTTP/2.0 200 OK\r\n\r\n",
    ] {
        assert!(
            read(bytes).is_err(),
            "{:?} should be refused",
            String::from_utf8_lossy(bytes)
        );
    }
}

#[test]
fn a_status_line_with_no_reason_is_fine_because_a_reason_means_nothing() {
    let (status, _) = read(b"HTTP/1.1 200\r\nContent-Length: 0\r\n\r\n").expect("a response");
    assert_eq!(status, Status::OK);
}

#[test]
fn nothing_a_server_can_send_makes_this_allocate_without_end() {
    // `docs/autonomy/LOOP.md`, stage 2. Every one of these is a server that
    // costs itself nothing and asks this process for everything.
    let enormous_header = format!(
        "HTTP/1.1 200 OK\r\nX-Long: {}\r\n\r\n",
        "a".repeat(64 * 1024)
    );
    let many_headers = {
        let mut out = "HTTP/1.1 200 OK\r\n".to_owned();
        for at in 0..10_000 {
            let _ = write!(out, "X-{at}: value\r\n");
        }
        out.push_str("\r\n");
        out
    };
    let absurd_length = "HTTP/1.1 200 OK\r\nContent-Length: 99999999999999\r\n\r\n".to_owned();
    let absurd_chunk =
        "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nffffffffffffffff\r\n".to_owned();
    let never_ending_line = format!("HTTP/1.1 200 OK\r\n{}", "x".repeat(100_000));

    for bytes in [
        enormous_header,
        many_headers,
        absurd_length,
        absurd_chunk,
        never_ending_line,
        "HTTP/1.1 200 OK\r\nContent-Length: -1\r\n\r\n".to_owned(),
        "HTTP/1.1 200 OK\r\nContent-Length: 0x10\r\n\r\n".to_owned(),
        "HTTP/1.1 200 OK\r\n: novalue\r\n\r\n".to_owned(),
        "HTTP/1.1 200 OK\r\nBad Name: x\r\n\r\n".to_owned(),
    ] {
        // Either answer is fine. Not returning one is not.
        let _ = read(bytes.as_bytes());
    }

    // And bytes that are not text at all.
    let _ = read(&[0xFFu8; 4096]);
    let _ = read(b"HTTP/1.1 200 OK\r\n\xff\xfe: x\r\n\r\n");
}

// ---- the request going out ----

#[test]
fn a_request_writes_the_host_from_the_url_and_not_from_a_header() {
    // Which site a shared server thinks it is talking to. Two sources
    // disagreeing about it is the same class of bug as two Content-Lengths,
    // so a caller does not get to set it.
    let mut request = Request::get(url("https://example.com/a/b?c=d"), a_person());
    request.headers.add("Host", "somewhere-else.example");
    request.headers.add("Accept", "text/html");

    let bytes = String::from_utf8(write_request(&request)).expect("ascii");
    assert!(bytes.starts_with("GET /a/b?c=d HTTP/1.1\r\n"), "{bytes:?}");
    assert!(bytes.contains("Host: example.com\r\n"), "{bytes:?}");
    assert!(!bytes.contains("somewhere-else"), "{bytes:?}");
    assert!(bytes.contains("Accept: text/html\r\n"), "{bytes:?}");
    assert!(bytes.ends_with("\r\n\r\n"), "{bytes:?}");
}

#[test]
fn a_non_default_port_is_part_of_the_host_and_a_default_one_is_not() {
    let plain = String::from_utf8(write_request(&Request::get(
        url("https://example.com/"),
        a_person(),
    )))
    .expect("ascii");
    assert!(plain.contains("Host: example.com\r\n"), "{plain:?}");

    let odd = String::from_utf8(write_request(&Request::get(
        url("https://example.com:8443/"),
        a_person(),
    )))
    .expect("ascii");
    assert!(odd.contains("Host: example.com:8443\r\n"), "{odd:?}");
}

#[test]
fn a_caller_cannot_set_the_headers_that_say_where_the_body_ends() {
    let mut request =
        Request::get(url("http://example.com/"), a_person()).for_purpose(Purpose::Fetch);
    request.headers.add("Content-Length", "999");
    request.headers.add("Transfer-Encoding", "chunked");
    let bytes = String::from_utf8(write_request(&request)).expect("ascii");
    assert!(!bytes.contains("999"), "{bytes:?}");
    assert!(!bytes.contains("chunked"), "{bytes:?}");
}
