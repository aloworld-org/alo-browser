/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Transfer-Encoding`: what was done to a body for this one hop.
//!
//! # It is not `Content-Encoding`, and the difference is the whole file
//!
//! The two headers name the same algorithms and mean different things.
//! `Content-Encoding` is a property of the **representation**: a gzipped
//! resource is gzipped in the cache, gzipped on the next hop, and gzipped in a
//! file somebody saves. `Transfer-Encoding` is a property of **this
//! connection**: the peer that sent these bytes applied it, the peer that
//! received them takes it off, and it does not survive the hop it describes.
//!
//! So the codings are [`crate::decompress`]'s — that file is the boundary for
//! the crates that undo them, and there is no second one — and what this file
//! decides is *which* codings are there and *in what order they come off*.
//!
//! # Why that ordering is its own responsibility
//!
//! `Transfer-Encoding: gzip, chunked` says the content was gzipped and then
//! the gzip was cut into chunks. So the chunks come off first and the gzip
//! second, and a reader that did it the other way round would be looking for
//! chunk headers inside compressed bytes.
//!
//! A reader that did the first half and forgot the second is worse, because it
//! does not fail: it hands compressed bytes up **labelled as a page**. That was
//! this engine until queue item 153, and it is why the header gets a file
//! rather than a line inside the message parser.
//!
//! # What is refused, and why each one
//!
//! Framing is the half of HTTP where being wrong is a security bug
//! ([`crate::body`] says so), and every rule below is a place where two
//! parsers could read one message differently:
//!
//! - **`chunked` anywhere but last.** It is what tells a recipient where the
//!   message ends, so a coding after it is a coding nobody could have applied.
//!   This also refuses `chunked, chunked` — the first of two is not the last —
//!   which is the shape a smuggling attempt takes when it is aimed at a
//!   recipient that de-chunks once and one that de-chunks twice.
//! - **A coding this engine cannot undo**, by name. Handing the bytes up is
//!   the failure this item exists to remove.
//! - **A coding that is not ended by `chunked`.** The standard allows it and
//!   says the body then ends when the connection does — but a compressed body
//!   delimited by a network event cannot be told from one an attacker cut
//!   short. gzip and zstd carry a checksum that would catch it; brotli and raw
//!   deflate carry **nothing at all**, which [`crate::decompress`] says in its
//!   own note. Refusing by name is an answer; a page that is quietly the first
//!   half of a page is not.
//! - **An empty coding in the list.** `chunked,` reads as one coding to some
//!   parsers and as one-and-a-blank to others, and a disagreement about a list
//!   whose last element decides the framing is the disagreement that matters.

use crate::decompress::Encoding;
use crate::headers::Headers;
use crate::http::{Malformed, shorten};

/// The most transfer codings this engine will undo for one body.
///
/// The same bound and the same reason as `Content-Encoding`'s: a list is a
/// bomb that costs the sender a few bytes to write, because the work is each
/// layer's bound times the length of the list. Real servers send at most one.
const MOST_CODINGS: usize = 4;

/// What a server did to a body for this hop.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Transfer {
    /// The codings applied to the content **before** it was chunked, in the
    /// order they were applied — so they are undone last-first, exactly as a
    /// `Content-Encoding` list is.
    ///
    /// `identity` is not in here. It says nothing was applied, so it
    /// contributes nothing rather than a no-op somebody has to remember is
    /// harmless.
    pub applied: Vec<Encoding>,
    /// Whether the body arrives in chunks, which is what decides its framing.
    pub chunked: bool,
}

/// What the head says was done to this body for this hop.
///
/// # Errors
///
/// [`Malformed`] for every reading in this file's own note — each of which is
/// a message that two parsers could read differently.
pub fn of(headers: &Headers) -> Result<Transfer, Malformed> {
    let lines: Vec<&str> = headers.all("Transfer-Encoding").collect();
    if lines.len() > 1 {
        // The standard joins repeated field lines with commas, so this *is*
        // one list. It is refused rather than joined because the order of the
        // list decides which coding comes off first, and the order of separate
        // field lines is the thing an intermediary is most likely to have
        // changed. Stricter than the standard, in the direction where being
        // wrong is a wrong page rather than a smuggled request.
        return Err(Malformed {
            why: "more than one Transfer-Encoding header".to_owned(),
        });
    }
    let Some(line) = lines.first() else {
        return Ok(Transfer::default());
    };

    let names: Vec<&str> = line.split(',').map(str::trim).collect();
    if names.len() > MOST_CODINGS {
        return Err(Malformed {
            why: format!("a body transfer-coded {} times over", names.len()),
        });
    }

    // Before anything about order, because a trailing comma makes `chunked`
    // stop being last and "chunked is not last" would be a true sentence about
    // the wrong thing. A refusal is only as useful as the reason in it.
    if names.iter().any(|name| name.is_empty()) {
        return Err(Malformed {
            why: "an empty transfer coding, which reads two ways".to_owned(),
        });
    }

    let mut applied = Vec::new();
    let mut chunked = false;
    for (position, name) in names.iter().enumerate() {
        if name.eq_ignore_ascii_case("chunked") {
            if position + 1 != names.len() {
                return Err(Malformed {
                    why: "chunked is not the last transfer coding, so something \
                          claims to have been applied after the message ended"
                        .to_owned(),
                });
            }
            chunked = true;
            continue;
        }
        let coding = Encoding::named(name).ok_or_else(|| Malformed {
            why: format!(
                "a Transfer-Encoding this engine does not read: {:?}",
                shorten(name)
            ),
        })?;
        if coding == Encoding::Identity {
            continue;
        }
        applied.push(coding);
    }

    if !applied.is_empty() && !chunked {
        return Err(Malformed {
            why: "a transfer-coded body that is not ended by chunked, which \
                  cannot be told from one that was cut short"
                .to_owned(),
        });
    }
    Ok(Transfer { applied, chunked })
}

/// Take off what was applied for this hop.
///
/// The chunking has already come off by the time a body reaches here — that is
/// [`crate::body`]'s job, and it has to happen first because the chunk headers
/// were written around these bytes rather than inside them.
///
/// # Errors
///
/// [`Malformed`] when a stream is not what it claimed to be, stops in the
/// middle, or decodes to more than this engine holds.
pub fn undo(body: Vec<u8>, transfer: &Transfer) -> Result<Vec<u8>, Malformed> {
    crate::decompress::undo(body, &transfer.applied)
}

#[cfg(test)]
mod tests {
    use super::{Transfer, of};
    use crate::decompress::Encoding;
    use crate::headers::Headers;

    fn saying(value: &str) -> Headers {
        let mut headers = Headers::new();
        headers.add("Transfer-Encoding", value);
        headers
    }

    #[test]
    fn no_header_is_nothing_applied() {
        assert_eq!(of(&Headers::new()), Ok(Transfer::default()));
    }

    #[test]
    fn chunked_alone_is_framing_and_no_coding() {
        let transfer = of(&saying("chunked")).expect("chunked is the ordinary case");
        assert!(transfer.chunked);
        assert!(transfer.applied.is_empty());
    }

    #[test]
    fn a_coding_before_chunked_comes_off_after_it() {
        let transfer = of(&saying("gzip, chunked")).expect("gzip under chunks is legal");
        assert!(transfer.chunked);
        assert_eq!(transfer.applied, vec![Encoding::Gzip]);
    }

    #[test]
    fn the_names_fold_case_because_a_header_value_is_not_case_sensitive() {
        assert_eq!(
            of(&saying("GZIP, Chunked")).expect("case does not change a coding"),
            of(&saying("gzip, chunked")).expect("case does not change a coding")
        );
    }

    #[test]
    fn identity_says_nothing_was_applied_and_so_applies_nothing() {
        let transfer = of(&saying("identity")).expect("identity is legal and means none");
        assert!(!transfer.chunked);
        assert!(transfer.applied.is_empty());
    }

    /// The refusal, or the reason a test wanted one and did not get it.
    fn refusal(headers: &Headers) -> String {
        match of(headers) {
            Err(refused) => refused.why,
            Ok(transfer) => format!("accepted, as {transfer:?}"),
        }
    }

    #[test]
    fn every_reading_that_two_parsers_could_differ_on_is_refused() {
        for (value, expected) in [
            ("chunked, gzip", "not the last"),
            ("chunked, chunked", "not the last"),
            ("compress, chunked", "does not read"),
            ("gzip", "not ended by chunked"),
            ("br", "not ended by chunked"),
            ("chunked,", "empty transfer coding"),
            (", chunked", "empty transfer coding"),
            ("gzip, gzip, gzip, gzip, chunked", "times over"),
        ] {
            let why = refusal(&saying(value));
            assert!(why.contains(expected), "{value:?} gave {why:?}");
        }
    }

    #[test]
    fn two_transfer_encoding_headers_are_refused_rather_than_joined() {
        let mut headers = Headers::new();
        headers.add("Transfer-Encoding", "gzip");
        headers.add("Transfer-Encoding", "chunked");
        let why = refusal(&headers);
        assert!(why.contains("more than one"), "{why:?}");
    }

    #[test]
    fn a_coding_nobody_can_read_does_not_put_a_whole_header_in_the_error() {
        let long = format!("{}, chunked", "x".repeat(4096));
        let why = refusal(&saying(&long));
        assert!(why.len() < 200, "an error {} bytes long", why.len());
    }
}
