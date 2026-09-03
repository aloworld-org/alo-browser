/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Undoing what a server compressed.
//!
//! `Content-Encoding: gzip` means the bytes on the wire are not the bytes the
//! page is made of. Undoing that is the last thing that happens to a body and
//! the first thing that happens to it that an attacker chose every byte of.
//!
//! # Why this is its own file
//!
//! Two reasons, and the second is the important one.
//!
//! It is the boundary for three rented crates (ADR 0001) — `flate2`,
//! `brotli-decompressor` and `ruzstd` — and a boundary is one file.
//!
//! And it is where a **decompression bomb** is stopped. Every other limit in
//! this crate bounds what *arrives*: [`crate::body::LARGEST_BODY`] against a
//! `Content-Length`, `LARGEST_CHUNK` against a chunk header. None of them help
//! here, because the whole point of compression is that what arrives is small.
//! A gigabyte of zeroes is about a megabyte of gzip and about six hundred bytes
//! of brotli. So the bound in this file is on **what comes out**, and it is the
//! only place in the crate where that distinction exists.
//!
//! # What "a corrupt stream is refused" can and cannot mean
//!
//! Three of these formats carry an integrity check of their own: gzip a CRC32,
//! zlib an Adler-32, zstd an optional XXH64. For those, a body that has been
//! altered is caught, and this file makes sure of it — `ruzstd` computes the
//! checksum and reads the one the frame carries and compares them for nobody,
//! so the comparison is written out below.
//!
//! **Raw DEFLATE and brotli carry no checksum at all.** A corruption that
//! happens to leave a structurally valid stream decodes to different bytes, and
//! nothing in any implementation could tell. That is a property of the formats
//! rather than a gap here, and it is written down because the alternative is a
//! reader assuming a guarantee that does not exist. What protects those two on
//! the wire is TLS, which is a different layer doing a different job.

use crate::body::LARGEST_BODY;
use crate::headers::Headers;
use crate::http::Malformed;
use std::io::Read;

/// The most encodings this engine will undo for one body.
///
/// `Content-Encoding` is a list, so `gzip, gzip, gzip, …` is a bomb built out
/// of a header rather than out of a stream: each layer is bounded on its own,
/// but the work is the bound times the length of the list. Real servers send
/// one. Four is room to be wrong in.
const MOST_ENCODINGS: usize = 4;

/// A way a body can have been compressed.
///
/// [`Encoding::Identity`] is in here because `identity` is a thing a server may
/// legally say, and "said nothing" and "said none" should not take different
/// paths through this file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Not compressed. A no-op, named.
    Identity,
    /// `gzip`, and its ancient spelling `x-gzip`. Carries a checksum.
    Gzip,
    /// `deflate`, which is two formats sharing one name — see [`undo_one`].
    Deflate,
    /// `br`.
    Brotli,
    /// `zstd`.
    Zstd,
}

impl Encoding {
    /// What a server called it, or `None` if this engine does not know it.
    ///
    /// Not knowing is deliberately distinguishable from `identity`. A body in
    /// an encoding we cannot undo must be an **error**: handing the compressed
    /// bytes up as though they were the page renders rubbish, and rubbish that
    /// came from a server is rubbish an attacker chose.
    pub fn named(name: &str) -> Option<Self> {
        let name = name.trim();
        if name.eq_ignore_ascii_case("identity") {
            Some(Encoding::Identity)
        } else if name.eq_ignore_ascii_case("gzip") || name.eq_ignore_ascii_case("x-gzip") {
            Some(Encoding::Gzip)
        } else if name.eq_ignore_ascii_case("deflate") {
            Some(Encoding::Deflate)
        } else if name.eq_ignore_ascii_case("br") {
            Some(Encoding::Brotli)
        } else if name.eq_ignore_ascii_case("zstd") {
            Some(Encoding::Zstd)
        } else {
            None
        }
    }

    /// What this engine asks for, in the order it prefers them.
    ///
    /// Sent as `Accept-Encoding`. Ordered by how well each does on markup
    /// rather than by novelty: brotli beats gzip on text by a fifth or so, and
    /// zstd sits between them while costing far less to decode.
    pub const ASKED_FOR: &'static str = "br, zstd, gzip, deflate";

    /// The name a server would use.
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Identity => "identity",
            Encoding::Gzip => "gzip",
            Encoding::Deflate => "deflate",
            Encoding::Brotli => "br",
            Encoding::Zstd => "zstd",
        }
    }
}

/// What was applied to a body, in the order it was applied.
///
/// # Errors
///
/// [`Malformed`] for an encoding this engine cannot undo, or for more of them
/// than it will undo at once.
pub fn what_was_applied(headers: &Headers) -> Result<Vec<Encoding>, Malformed> {
    let mut applied = Vec::new();
    // A repeated header and a comma-separated one mean the same thing, and a
    // server may use either. Both are one list.
    for held in headers.all("Content-Encoding") {
        for name in held.split(',') {
            if name.trim().is_empty() {
                continue;
            }
            let encoding = Encoding::named(name).ok_or_else(|| Malformed {
                why: format!(
                    "a body compressed with {:?}, which this engine cannot undo",
                    name.trim()
                ),
            })?;
            if applied.len() >= MOST_ENCODINGS {
                return Err(Malformed {
                    why: format!("a body compressed {} times over", applied.len() + 1),
                });
            }
            applied.push(encoding);
        }
    }
    Ok(applied)
}

/// Undo what was applied, innermost last.
///
/// `Content-Encoding: gzip, br` means gzip happened and then brotli happened,
/// so brotli is undone first. Getting this backwards produces an error rather
/// than a wrong page, which is luck rather than design — hence the test that
/// says so by name.
///
/// # Errors
///
/// [`Malformed`] when a stream is not what it claimed to be, stops in the
/// middle, or decodes to more than this engine holds.
pub fn undo(bytes: Vec<u8>, applied: &[Encoding]) -> Result<Vec<u8>, Malformed> {
    undo_within(bytes, applied, LARGEST_BODY)
}

/// Undo what was applied, refusing anything that decodes to more than `limit`.
///
/// The limit is a parameter rather than only a constant for two reasons. A
/// caller that knows a subresource should be small can say so, which is a
/// tighter bound than the whole-body one. And a test can prove the bound holds
/// without spending a quarter of a gigabyte to do it — the mechanism is the
/// same at eight kilobytes as at [`LARGEST_BODY`].
///
/// # Errors
///
/// [`Malformed`] when a stream is not what it claimed to be, stops in the
/// middle, or decodes to more than `limit`.
pub fn undo_within(bytes: Vec<u8>, applied: &[Encoding], limit: u64) -> Result<Vec<u8>, Malformed> {
    let mut bytes = bytes;
    for encoding in applied.iter().rev() {
        bytes = undo_one(&bytes, *encoding, limit)?;
    }
    Ok(bytes)
}

/// One layer.
///
/// # `deflate`, which is two formats sharing one name
///
/// The specification says zlib. A meaningful number of servers send raw
/// DEFLATE with no zlib wrapper, because an early and popular server did. So
/// zlib is tried and raw is the fallback — which is a compatibility wart, and
/// this engine takes it because the alternative is a blank page on sites that
/// work everywhere else.
///
/// # Errors
///
/// [`Malformed`], with the encoding named, because "the body was corrupt" and
/// "the body was corrupt **brotli**" are different amounts of help.
fn undo_one(bytes: &[u8], encoding: Encoding, limit: u64) -> Result<Vec<u8>, Malformed> {
    match encoding {
        Encoding::Identity => Ok(bytes.to_vec()),
        Encoding::Gzip => bounded(flate2::read::GzDecoder::new(bytes), encoding, limit),
        Encoding::Deflate => {
            match bounded(flate2::read::ZlibDecoder::new(bytes), encoding, limit) {
                Ok(out) => Ok(out),
                Err(zlib) => bounded(flate2::read::DeflateDecoder::new(bytes), encoding, limit)
                    .map_err(|_| zlib),
            }
        }
        Encoding::Brotli => bounded(
            brotli_decompressor::Decompressor::new(bytes, BLOCK),
            encoding,
            limit,
        ),
        Encoding::Zstd => {
            let mut decoder =
                ruzstd::decoding::StreamingDecoder::new(bytes).map_err(|why| Malformed {
                    why: format!("a body that said zstd and was not: {why}"),
                })?;
            let out = bounded(&mut decoder, encoding, limit)?;
            // `ruzstd` computes the frame's checksum, and reads the one the
            // frame carries, and compares them for nobody. So a zstd body with
            // a byte flipped in it decodes without complaint — which a test
            // caught, and which is the exact thing this item exists to
            // prevent. The comparison is ours.
            let frame = decoder.into_frame_decoder();
            if let Some(said) = frame.get_checksum_from_data()
                && frame.get_calculated_checksum() != Some(said)
            {
                return Err(Malformed {
                    why: "a zstd body whose contents do not match its own checksum".to_owned(),
                });
            }
            Ok(out)
        }
    }
}

/// How much to ask a decoder for at a time.
const BLOCK: usize = 16 * 1024;

/// Read a decoder to its end, and refuse a stream that decodes to more than
/// this engine holds.
///
/// The `+ 1` is the whole trick: taking exactly the limit cannot tell a body
/// that is the limit from one that is larger, so it takes one byte more and
/// treats having got it as the answer.
fn bounded(source: impl Read, encoding: Encoding, limit: u64) -> Result<Vec<u8>, Malformed> {
    let mut out = Vec::new();
    source
        .take(limit + 1)
        .read_to_end(&mut out)
        .map_err(|why| Malformed {
            why: format!("a body that said {} and was not: {why}", encoding.name()),
        })?;
    if out.len() as u64 > limit {
        return Err(Malformed {
            why: format!(
                "a {} body that decodes to more than the {limit} bytes this engine holds",
                encoding.name()
            ),
        });
    }
    Ok(out)
}
