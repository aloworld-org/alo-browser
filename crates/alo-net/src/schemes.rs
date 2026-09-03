/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The two schemes that need nothing.
//!
//! `data:` carries its own bytes and `file:` reads them off this machine.
//! Neither needs a socket, so both can be built and tested before there is a
//! network — which is the point of doing them first. HTTP arrives in queue
//! item 53 as a third function with the same shape.
//!
//! # `file:` is the one to be careful with
//!
//! It reads whatever the caller names, so **it must only ever be called by the
//! browser process**. ADR 0005 gives a renderer no filesystem at all, and this
//! is the thing it is being kept away from. There is no path traversal to
//! defend against here — the URL parser has already normalised the path — but
//! there is no sandbox either, and that is deliberate: the sandbox is the
//! process boundary rather than a check in this file.

use crate::media_type::MediaType;
use crate::response::{Response, Status};
use alo_url::Url;

/// Read a `data:` URL's own bytes.
///
/// `data:[<media type>][;base64],<data>`. The bytes are in the URL, so this
/// cannot fail on anything but a malformed URL — which it reports rather than
/// guessing at.
///
/// # Errors
///
/// A sentence, when the URL is not a `data:` URL this engine can read.
pub fn data(url: &Url) -> Result<Response, String> {
    let rest = url
        .serialised
        .strip_prefix("data:")
        .ok_or_else(|| "not a data: URL".to_owned())?;
    let (head, body) = rest
        .split_once(',')
        .ok_or_else(|| "a data: URL needs a comma".to_owned())?;

    let (media, base64) = match head.strip_suffix(";base64") {
        Some(media) => (media, true),
        None => (head, false),
    };
    let bytes = if base64 {
        decode_base64(body).ok_or_else(|| "the base64 does not decode".to_owned())?
    } else {
        percent_decode(body)
    };

    let mut response = Response::ok(url.clone(), bytes);
    // A `data:` URL with no media type is `text/plain;charset=US-ASCII`, which
    // is what the standard says and what a caller would otherwise have to know.
    let declared = if media.trim().is_empty() {
        "text/plain;charset=US-ASCII".to_owned()
    } else {
        media.to_owned()
    };
    if MediaType::parse(&declared).is_some() {
        response.headers.add("Content-Type", declared);
    }
    Ok(response)
}

/// Read a `file:` URL off this machine.
///
/// # Errors
///
/// A sentence, when the URL is not a readable path — which includes a file
/// that is not there, and one this process may not read.
pub fn file(url: &Url) -> Result<Response, String> {
    if url.scheme != "file" {
        return Err("not a file: URL".to_owned());
    }
    let path = file_path(url).ok_or_else(|| "not a path this machine has".to_owned())?;
    let bytes = std::fs::read(&path).map_err(|why| format!("{}: {why}", path.display()))?;

    let mut response = Response::ok(url.clone(), bytes);
    if let Some(media) = from_extension(&path) {
        response.headers.add("Content-Type", media);
    }
    Ok(response)
}

/// The path a `file:` URL names on this machine.
fn file_path(url: &Url) -> Option<std::path::PathBuf> {
    // The URL parser has already resolved `..` and decoded the percent
    // escapes' bytes, so what is left is to turn the path into one this
    // platform understands.
    let decoded = percent_decode(&url.path);
    let text = String::from_utf8(decoded).ok()?;
    if text.is_empty() {
        return None;
    }
    Some(std::path::PathBuf::from(text))
}

/// What a file's name suggests it is.
///
/// Deliberately short. Guessing a media type from a name is a **fallback**,
/// and a wrong guess on a page from the network is a security bug — which is
/// why on `file:`, where nobody sent a header, it is all there is, and why
/// queue item 53 brings the header that overrides it.
fn from_extension(path: &std::path::Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match extension.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "txt" => "text/plain; charset=utf-8",
        _ => return None,
    })
}

/// `%41` into `A`, leaving anything that is not an escape alone.
fn percent_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0usize;
    while at < bytes.len() {
        let byte = bytes.get(at).copied().unwrap_or(b'0');
        if byte == b'%'
            && let Some(high) = bytes.get(at + 1).and_then(|byte| hex(*byte))
            && let Some(low) = bytes.get(at + 2).and_then(|byte| hex(*byte))
        {
            out.push(high * 16 + low);
            at += 3;
            continue;
        }
        out.push(byte);
        at += 1;
    }
    out
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Base64, as `data:` URLs use it.
///
/// Ours rather than rented for once, because it is twenty lines and adding a
/// dependency for twenty lines is its own kind of cost. Whitespace is skipped,
/// which real `data:` URLs contain when they have been wrapped in markup.
fn decode_base64(text: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut held: u32 = 0;
    let mut bits = 0u32;
    for byte in text.bytes() {
        if byte.is_ascii_whitespace() || byte == b'=' {
            continue;
        }
        let value = ALPHABET.iter().position(|held| *held == byte)?;
        let value = u32::try_from(value).ok()?;
        held = (held << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            let shifted = (held >> bits) & 0xFF;
            out.push(u8::try_from(shifted).ok()?);
        }
    }
    Some(out)
}

/// A response that says a load did not happen, for a caller that wants one
/// rather than an error.
pub fn failed(url: &Url, status: u16) -> Response {
    Response {
        url: url.clone(),
        status: Status(status),
        headers: crate::headers::Headers::new(),
        body: Vec::new(),
    }
}
