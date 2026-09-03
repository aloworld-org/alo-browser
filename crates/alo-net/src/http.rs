/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! HTTP/1.1 messages: a request out, a response's head in.
//!
//! **Ours, not rented.** The syntax is a few lines of ASCII and the difficulty
//! is not in reading it — it is in refusing the readings that are *almost*
//! right. A header block is the most hostile input a browser accepts, and
//! nearly every famous HTTP bug is a parser being generous:
//!
//! - Two `Content-Length` headers that disagree. A parser that picks one has
//!   just disagreed with the proxy in front of it about where this response
//!   ends and the next begins, which is **request smuggling**.
//! - `Transfer-Encoding` *and* `Content-Length` together. Same bug, spelled
//!   differently.
//! - A space before the colon — `Content-Length : 5`. Some parsers accept it
//!   and some do not, and a chain containing both is a smuggling chain.
//! - A header continued on the next line by leading whitespace. Removed from
//!   the standard in 2014 for exactly this reason.
//!
//! So this file refuses all four, and every limit it applies is a named
//! constant rather than a number in a condition.

use crate::headers::Headers;
use crate::request::Request;
use crate::response::Status;
use core::fmt;
use core::fmt::Write as _;

/// The longest status line this engine will read.
///
/// A server with more to say than this is not one that is going to make sense.
const LONGEST_STATUS_LINE: usize = 8 * 1024;

/// The longest single header line.
const LONGEST_HEADER: usize = 8 * 1024;

/// The most headers a response may carry.
///
/// A bound rather than a limit somebody hit: without one, a server can make
/// this process allocate for as long as it cares to send, which is a denial of
/// service that costs the attacker nothing.
const MOST_HEADERS: usize = 200;

/// Why a response could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed {
    /// What is wrong, in words.
    pub why: String,
}

impl Malformed {
    fn new(why: impl Into<String>) -> Self {
        Self { why: why.into() }
    }
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the response is not HTTP this engine will read: {}",
            self.why
        )
    }
}

impl std::error::Error for Malformed {}

/// Which HTTP/1 a response is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// `HTTP/1.0`: a connection closes unless the response says otherwise.
    Http10,
    /// `HTTP/1.1`: a connection stays open unless the response says otherwise.
    Http11,
}

/// A response's head: everything before the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// Which HTTP/1 this is.
    ///
    /// Kept because it decides whether a connection may be reused when nobody
    /// said: 1.1 keeps it open unless told otherwise, and 1.0 closes it unless
    /// told otherwise. Dropping it would mean guessing, and guessing wrong
    /// leaves a socket that the next request hangs on.
    pub version: Version,
    /// The status.
    pub status: Status,
    /// The reason phrase, which servers are free to make up and often do.
    pub reason: String,
    /// The headers, in order, repeats kept.
    pub headers: Headers,
}

/// A request, as the bytes to send.
///
/// `Host` is written from the URL rather than from whatever a caller put in the
/// headers: it decides which site a shared server thinks it is talking to, and
/// letting two sources disagree about it is the same class of bug as two
/// `Content-Length`s.
pub fn write_request(request: &Request) -> Vec<u8> {
    let url = &request.url;
    let mut target = if url.path.is_empty() {
        "/".to_owned()
    } else {
        url.path.clone()
    };
    if let Some(query) = &url.query {
        target.push('?');
        target.push_str(query);
    }

    // Writing to a `String` cannot fail.
    let mut out = String::new();
    let _ = write!(out, "{} {target} HTTP/1.1\r\n", request.method);
    if let Some(host) = &url.host {
        let _ = match url.port {
            Some(port) => write!(out, "Host: {host}:{port}\r\n"),
            None => write!(out, "Host: {host}\r\n"),
        };
    }
    // Ask for the encodings this engine can undo — but only if the caller has
    // not asked for something else. Unlike `Host`, this one *is* the caller's
    // to set: a download that resumes wants `identity`, because a byte range
    // of a compressed stream is a range of bytes nobody can decompress.
    if request.headers.get("Accept-Encoding").is_none() {
        let _ = write!(
            out,
            "Accept-Encoding: {}\r\n",
            crate::decompress::Encoding::ASKED_FOR
        );
    }
    for header in request.headers.iter() {
        // The ones this function decides are not the caller's to set, for the
        // reason in this function's own note.
        if header.name.eq_ignore_ascii_case("host")
            || header.name.eq_ignore_ascii_case("content-length")
            || header.name.eq_ignore_ascii_case("transfer-encoding")
        {
            continue;
        }
        let _ = write!(out, "{}: {}\r\n", header.name, header.value);
    }
    // **No `Connection` header.** HTTP/1.1 keeps a connection open unless
    // somebody says otherwise, and this engine wants it kept — see
    // `crate::pool`. Saying `close` here is what queue item 53 did while there
    // was nothing to reuse it with.
    out.push_str("\r\n");
    out.into_bytes()
}

/// Read a response's head from a byte source.
///
/// # Errors
///
/// [`Malformed`] for anything this engine will not read — including the four
/// almost-right readings in this file's own note.
pub fn read_head(source: &mut impl std::io::Read) -> Result<Head, Malformed> {
    let line = read_line(source, LONGEST_STATUS_LINE)?;
    let (version, status, reason) = parse_status_line(&line)?;

    let mut headers = Headers::new();
    loop {
        let line = read_line(source, LONGEST_HEADER)?;
        if line.is_empty() {
            break;
        }
        if headers.len() >= MOST_HEADERS {
            return Err(Malformed::new(format!("more than {MOST_HEADERS} headers")));
        }
        let (name, value) = parse_header(&line)?;
        headers.add(name, value);
    }
    check_framing_is_unambiguous(&headers)?;
    Ok(Head {
        version,
        status,
        reason,
        headers,
    })
}

/// `HTTP/1.1 200 OK`.
fn parse_status_line(line: &str) -> Result<(Version, Status, String), Malformed> {
    let (version, rest) = if let Some(rest) = line.strip_prefix("HTTP/1.1 ") {
        (Version::Http11, rest)
    } else if let Some(rest) = line.strip_prefix("HTTP/1.0 ") {
        (Version::Http10, rest)
    } else {
        return Err({
            Malformed::new(format!(
                "the first line is not an HTTP/1 status line: {:?}",
                shorten(line)
            ))
        });
    };
    let (code, reason) = match rest.split_once(' ') {
        Some((code, reason)) => (code, reason),
        None => (rest, ""),
    };
    if code.len() != 3 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Malformed::new(format!(
            "{code:?} is not a three-digit status"
        )));
    }
    let code: u16 = code
        .parse()
        .map_err(|_| Malformed::new("the status does not fit in a status"))?;
    Ok((version, Status(code), reason.trim().to_owned()))
}

/// `Name: value`, and none of the readings that are almost that.
fn parse_header(line: &str) -> Result<(String, String), Malformed> {
    if line.starts_with(' ') || line.starts_with('\t') {
        // A header continued onto the next line. Removed from the standard in
        // 2014 because a chain of parsers disagreeing about it is a smuggling
        // chain.
        return Err(Malformed::new("a header is continued onto the next line"));
    }
    let (name, value) = line
        .split_once(':')
        .ok_or_else(|| Malformed::new(format!("{:?} is not a header", shorten(line))))?;
    if name.is_empty() {
        return Err(Malformed::new("a header with no name"));
    }
    if name.ends_with(' ') || name.ends_with('\t') {
        // `Content-Length : 5`. Some parsers accept it and some do not.
        return Err(Malformed::new(format!(
            "a space before the colon in {:?}",
            shorten(name)
        )));
    }
    if !name.bytes().all(is_token_byte) {
        return Err(Malformed::new(format!(
            "{:?} is not a header name",
            shorten(name)
        )));
    }
    Ok((name.to_owned(), value.trim().to_owned()))
}

/// Whether the head says exactly one thing about where the body ends.
///
/// The whole of request smuggling is a message that says two things and two
/// parsers each believing a different one.
fn check_framing_is_unambiguous(headers: &Headers) -> Result<(), Malformed> {
    let lengths: Vec<&str> = headers.all("Content-Length").collect();

    // On the header being **present**, not on what it parses to. A
    // `Transfer-Encoding: identity` beside a `Content-Length` applies no coding
    // here and is still refused, because a recipient that treated `identity` as
    // a transfer coding would frame this message by the connection closing
    // while we framed it by the length — which is the disagreement.
    if !lengths.is_empty() && headers.all("Transfer-Encoding").next().is_some() {
        return Err(Malformed::new(
            "both Content-Length and Transfer-Encoding, which say different \
             things about where this response ends",
        ));
    }
    if lengths.len() > 1 {
        let first = lengths.first().copied().unwrap_or_default();
        if lengths.iter().any(|held| *held != first) {
            return Err(Malformed::new("two Content-Length headers that disagree"));
        }
    }
    // What a transfer coding may be, and in what order — [`crate::transfer`],
    // because getting the order wrong is a framing bug rather than a decoding
    // one and it wanted a file to say so in.
    crate::transfer::of(headers)?;
    Ok(())
}

/// One line, without its ending, refusing one that never ends.
fn read_line(source: &mut impl std::io::Read, longest: usize) -> Result<String, Malformed> {
    let mut bytes = Vec::new();
    // Read a byte at a time so the limit is a limit rather than a hope: a
    // `read_until` with no bound will allocate for as long as a server keeps
    // sending, which is a denial of service that costs the sender nothing.
    loop {
        let mut byte = [0u8; 1];
        let read = source
            .read(&mut byte)
            .map_err(|why| Malformed::new(format!("the connection ended: {why}")))?;
        if read == 0 {
            if bytes.is_empty() {
                return Err(Malformed::new("the connection ended before the head did"));
            }
            break;
        }
        let byte = byte.first().copied().unwrap_or(b'\n');
        if byte == b'\n' {
            break;
        }
        if bytes.len() >= longest {
            return Err(Malformed::new(format!(
                "a line longer than {longest} bytes"
            )));
        }
        bytes.push(byte);
    }
    while bytes
        .last()
        .is_some_and(|byte| *byte == b'\n' || *byte == b'\r')
    {
        bytes.pop();
    }
    // A header is ASCII. Bytes that are not are refused rather than replaced,
    // because a header nobody can read is not one to act on.
    String::from_utf8(bytes).map_err(|_| Malformed::new("a line that is not text"))
}

/// Whether a byte may appear in a header name, per the standard's `token`.
fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

/// Enough of a string to say what is wrong, without putting a server's whole
/// answer into an error message.
pub(crate) fn shorten(text: &str) -> String {
    const ENOUGH: usize = 60;
    match text.char_indices().nth(ENOUGH) {
        Some((at, _)) => format!("{}…", text.get(..at).unwrap_or_default()),
        None => text.to_owned(),
    }
}
