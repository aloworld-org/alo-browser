/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's linked style sheet, as it crosses the boundary in each direction
//! (ADR 0035 §§ 1 and 4, queue item 348).
//!
//! **Out**, a [`SheetAsk`]: a claim in the answer to the message whose work
//! found the `<link>` — [`crate::FromRenderer::Loaded`],
//! [`crate::FromRenderer::Acted`] or [`crate::FromRenderer::Delivered`] —
//! once per URL per document ([`crate::linked`]). It carries what only the
//! renderer knows: the URL as it resolved it, the `crossorigin` attribute as
//! `alo-net`'s mode and credentials, the `referrerpolicy` attribute, and the
//! nonce the `<link>` presents. **Never an origin, a cause, a cookie or a
//! tab**: the browser process knows each of those without being told
//! ([`crate::sheet_decide`]).
//!
//! **Back**, a [`SheetAnswer`]: a message of its own, which is a task of its
//! own (ADR 0016 § 2), naming the ask by the number the renderer chose and
//! carrying the sheet's bytes **only if they are a style sheet** — a 2xx
//! answer whose `Content-Type` is `text/css` ([`crate::sheet_make`]). Anything
//! else is a failure with no reason in it, so the bytes of whatever else a
//! page named are never in a renderer's memory (ADR 0035 § 3).

use alo_net::cors::{Credentials, Mode};
use alo_net::referrer::Policy;
use core::fmt;

/// The most style sheets one document may ask for over its life: 64.
///
/// Each is a request the browser process makes, in the process that may not
/// crash, and a script can add `<link>`s for as long as it runs. Sixty-four is
/// what one document may have of a page's fetches in flight at once
/// ([`crate::fetch_owed::MOST_IN_FLIGHT`], itself the connections `alo-net`'s
/// pool keeps idle for every site together); the only frozen page that links
/// a sheet, alo Sites' `section_cta.html`, links one. A page that needs more
/// opens an item to raise this, with the page frozen beside it.
///
/// Kept by both sides: the renderer asks for no more, and the browser process
/// refuses any past it whatever a renderer says ([`crate::sheet_owed`]). An
/// answer can therefore carry at most this many, which is the bound per answer
/// as well.
pub const MOST_SHEETS: usize = 64;

/// A style sheet a page links, as a renderer asks for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetAsk {
    /// The renderer's number for it, unique for the life of its document.
    /// The browser process echoes it back and never interprets it.
    pub number: u64,
    /// Where it is, resolved against the document's base URL and serialised
    /// by the renderer. A claim: the browser process parses it again.
    pub url: String,
    /// `no-cors` for a `<link>` with no `crossorigin`, `cors` for one with.
    pub mode: Mode,
    /// `include` with no `crossorigin` or with `use-credentials`,
    /// `same-origin` with `anonymous`.
    pub credentials: Credentials,
    /// The `<link>`'s `referrerpolicy`, if it named one this engine knows.
    pub referrer: Option<Policy>,
    /// The nonce the `<link>` presents ([`alo_dom::nonce::presented`]), which
    /// a page's `style-src` reads.
    pub nonce: Option<String>,
}

/// What became of a sheet the page asked for, sent to the renderer that
/// asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetAnswer {
    /// The ask's number, as the renderer chose it.
    pub number: u64,
    /// The sheet's bytes, undecoded — the renderer decodes them (ADR 0035
    /// § 3) — or [`None`] when it did not arrive. **No reason**, on purpose:
    /// the page is told the same thing whatever happened, and the reason is
    /// said to the person instead.
    pub bytes: Option<Vec<u8>>,
}

impl SheetAnswer {
    /// Sheet `number` did not arrive.
    pub const fn failed(number: u64) -> Self {
        Self {
            number,
            bytes: None,
        }
    }
}

impl fmt::Display for SheetAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.bytes {
            Some(bytes) => write!(
                f,
                "the answer to style sheet {}: {} bytes",
                self.number,
                bytes.len()
            ),
            None => write!(
                f,
                "the answer to style sheet {}: it did not arrive",
                self.number
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answer_says_how_much_arrived_and_a_failure_says_no_more() {
        let arrived = SheetAnswer {
            number: 2,
            bytes: Some(b"p{}".to_vec()),
        };
        assert_eq!(arrived.to_string(), "the answer to style sheet 2: 3 bytes");
        assert_eq!(
            SheetAnswer::failed(5).to_string(),
            "the answer to style sheet 5: it did not arrive"
        );
    }
}
