/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a press that pays pays, as the agent's question shows it (queue
//! item 826, `docs/design/shell.md`, *Paying*, approved by the owner
//! 2026-11-06; ADR 0099): the amount, the merchant and the page's own
//! summary lines, read by the browser process off the page's tree
//! (`alo-window`'s `agent_paying`), never asked of the model and never
//! sent to it.
//!
//! Data, and nothing else. What could not be found is said plainly, and
//! the person is still asked: [`NO_AMOUNT`], [`NO_SUMMARY`]. Every string
//! is the page's — a stranger's — and enters the document as a text node.

/// The most of the page's summary lines shown.
pub const MOST_SUMMARY: usize = 6;

/// The longest a summary line, the amount or the merchant is, in
/// characters.
pub const PAYING_LINE: usize = 120;

/// What the question says when the page shows no amount alo can read.
pub const NO_AMOUNT: &str =
    "alo could not find the amount on this page: read it on the page before you pay.";

/// What it says when the page shows no summary alo can read.
pub const NO_SUMMARY: &str = "alo could not find a summary of what you pay for on this page.";

/// What the button that lets a paying press go says: *Pay*.
pub const PAY: &str = "Pay";

/// What a press that pays pays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paying {
    /// The amount, as the page writes it — *UGX 45,000*, *$42.00* — or
    /// [`None`] when it shows none alo can read.
    pub amount: Option<String>,
    /// Who is paid: the page's name for itself, and the site's host, which
    /// the browser knows.
    pub merchant: String,
    /// The page's own summary of what is paid for, a line each, at most
    /// [`MOST_SUMMARY`]; none when it shows none alo can read.
    pub lines: Vec<String>,
}

impl Paying {
    /// Whether it is within the bounds the wire holds: at most
    /// [`MOST_SUMMARY`] lines, and no text past [`PAYING_LINE`] characters.
    pub fn is_bounded(&self) -> bool {
        let short = |text: &str| text.chars().count() <= PAYING_LINE;
        self.lines.len() <= MOST_SUMMARY
            && self.amount.as_deref().is_none_or(short)
            && short(&self.merchant)
            && self.lines.iter().all(|line| short(line))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn its_bounds_are_six_lines_and_a_line_of_words() {
        let paying = Paying {
            amount: Some("UGX 45,000".to_owned()),
            merchant: "Kampala Coffee (127.0.0.1)".to_owned(),
            lines: vec!["Flat white \u{d7} 2 UGX 16,000".to_owned(); MOST_SUMMARY],
        };
        assert!(paying.is_bounded());
        let mut more = paying.clone();
        more.lines.push("one more".to_owned());
        assert!(!more.is_bounded());
        let mut long = paying;
        long.amount = Some("9".repeat(PAYING_LINE + 1));
        assert!(!long.is_bounded());
    }
}
