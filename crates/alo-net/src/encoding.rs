/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which encoding a page is in, and turning its bytes into text.
//!
//! **This is the only file that names `encoding_rs`.** Which byte means which
//! character in `windows-1252`, `shift_jis` or `euc-kr` is a set of tables the
//! industry took twenty years to agree on, and ADR 0001 says to rent that kind
//! of thing.
//!
//! What is **ours** is the algorithm above the tables: deciding *which*
//! encoding a page is in when the page, the server and the bytes disagree —
//! which they frequently do. That is a sequence of rules rather than a table,
//! and getting it wrong shows up as mojibake on somebody's news site.
//!
//! # The order, and why it is an order
//!
//! HTML's own, and every step exists because the one before it can be absent
//! or wrong:
//!
//! 1. **A byte order mark.** It is in the bytes themselves, so it beats
//!    anything anybody *said* about them — including a `Content-Type` that
//!    contradicts it.
//! 2. **The `Content-Type` header's `charset`.** The server's claim.
//! 3. **A `<meta charset>` in the first part of the document.** The author's
//!    claim, and the one most pages actually carry.
//! 4. **UTF-8**, which is what the modern web is and what a page with nothing
//!    to say about itself is overwhelmingly likely to be.
//!
//! Detecting an encoding from the *shape* of the bytes when nobody has said —
//! guessing that this looks like Shift JIS — is stage 3's, and this engine
//! does not guess.

use crate::media_type::MediaType;
use core::fmt;

/// Where the answer came from, which is worth keeping.
///
/// A page decoded as UTF-8 because nobody said otherwise is a different fact
/// from one decoded as UTF-8 because it said so, and only the first is worth
/// looking at twice when somebody reports mojibake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A byte order mark, which is in the bytes rather than in a claim.
    ByteOrderMark,
    /// The `Content-Type` header said so.
    Header,
    /// A `<meta>` in the document said so.
    Meta,
    /// Nobody said, so it is what the modern web is.
    Default,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Source::ByteOrderMark => "a byte order mark",
            Source::Header => "the Content-Type header",
            Source::Meta => "a <meta> in the document",
            Source::Default => "nothing, so UTF-8",
        })
    }
}

/// What an encoding was decided to be, and how.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sniffed {
    /// The encoding's own name, as the standard spells it.
    pub label: String,
    /// Which step of the algorithm answered.
    pub source: Source,
}

/// Text, and what it took to get it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    /// The text.
    pub text: String,
    /// Which encoding it was read as, and how that was decided.
    pub encoding: Sniffed,
    /// Whether any byte could not be read and became a replacement character.
    ///
    /// **Kept rather than hidden.** A page that decoded with errors is a page
    /// that is mislabelled, and a browser that silently produced
    /// question marks would leave nobody able to find out why.
    pub had_errors: bool,
}

/// Which encoding these bytes are in.
///
/// `declared` is the `Content-Type`'s charset, if the load had one.
pub fn sniff(bytes: &[u8], declared: Option<&MediaType>) -> Sniffed {
    if let Some(label) = from_byte_order_mark(bytes) {
        return Sniffed {
            label: label.to_owned(),
            source: Source::ByteOrderMark,
        };
    }
    if let Some(charset) = declared.and_then(MediaType::charset)
        && let Some(known) = known(charset)
    {
        return Sniffed {
            label: known.to_owned(),
            source: Source::Header,
        };
    }
    if let Some(label) = from_meta(bytes) {
        return Sniffed {
            label,
            source: Source::Meta,
        };
    }
    Sniffed {
        label: "UTF-8".to_owned(),
        source: Source::Default,
    }
}

/// Bytes into text, deciding the encoding first.
pub fn decode(bytes: &[u8], declared: Option<&MediaType>) -> Decoded {
    let encoding = sniff(bytes, declared);
    let chosen =
        encoding_rs::Encoding::for_label(encoding.label.as_bytes()).unwrap_or(encoding_rs::UTF_8);
    let (text, _, had_errors) = chosen.decode(bytes);
    Decoded {
        text: text.into_owned(),
        encoding,
        had_errors,
    }
}

/// The label a byte order mark spells out, if there is one.
///
/// A mark is **removed** by the decoder rather than left in the text, which is
/// what stops a page beginning with an invisible character that then fails to
/// match `<!DOCTYPE`.
fn from_byte_order_mark(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0xEF, 0xBB, 0xBF, ..] => Some("UTF-8"),
        [0xFE, 0xFF, ..] => Some("UTF-16BE"),
        [0xFF, 0xFE, ..] => Some("UTF-16LE"),
        _ => None,
    }
}

/// The encoding a `<meta>` declares, from the first part of the document.
///
/// Both spellings pages actually use: `<meta charset=…>` and the older
/// `<meta http-equiv="Content-Type" content="…; charset=…">`. Only the first
/// kilobyte is read, which is what HTML says and what stops a `charset` word
/// deep inside a page's text being mistaken for a declaration.
fn from_meta(bytes: &[u8]) -> Option<String> {
    const LOOK_AT: usize = 1024;
    let head = bytes.get(..LOOK_AT.min(bytes.len()))?;
    // ASCII-lowercased for searching. A label is ASCII in every encoding this
    // could be, so reading the head as Latin-1 cannot lose one.
    let text: String = head.iter().map(|byte| char::from(*byte)).collect();
    let lowered = text.to_ascii_lowercase();

    let mut at = 0usize;
    while let Some(found) = lowered.get(at..).and_then(|rest| rest.find("charset")) {
        let after = at + found + "charset".len();
        at = after;
        let Some(rest) = lowered.get(after..) else {
            break;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let value = rest
            .trim_start()
            .trim_start_matches(['"', '\''])
            .split(|c: char| c == '"' || c == '\'' || c == ';' || c == '>' || c.is_whitespace())
            .next()
            .unwrap_or_default();
        if let Some(label) = known(value) {
            return Some(label.to_owned());
        }
    }
    None
}

/// The standard's own name for a label, or [`None`] for one nobody has heard
/// of.
///
/// A label this engine cannot place is **not** a reason to guess: the next
/// step of the algorithm answers instead, and the last step is UTF-8.
fn known(label: &str) -> Option<&'static str> {
    encoding_rs::Encoding::for_label(label.trim().as_bytes()).map(encoding_rs::Encoding::name)
}
