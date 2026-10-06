/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Numbers as SVG writes them, in lists.
//!
//! SVG's number grammar is narrower than Rust's float parser — no `inf`, no
//! `NaN`, no `_` — and wider than a split on whitespace: `10-20` is two
//! numbers, and so is `.5.5`. `points`, `transform` and (with item 272) path
//! data all share it, so it is read once, here, by a scanner that walks the
//! text and allocates nothing.
//!
//! **Every number is finite or it is an error.** An exponent that overflows a
//! float (`1e99999`) is not a very large coordinate; it is a stranger's input
//! that would become an infinity in a path, and it stops the list where it is.

/// Numbers separated by SVG's comma-or-whitespace, read one at a time.
///
/// Reading stops at the first thing that is not a number where one should be:
/// [`Numbers::next`] then answers [`None`] and [`Numbers::failed`] says so,
/// which is what lets a list be used *up to its first error* (SVG 2's rule for
/// `points`, and item 272's for path data).
#[derive(Debug, Clone)]
pub struct Numbers<'a> {
    bytes: &'a [u8],
    at: usize,
    failed: bool,
}

impl<'a> Numbers<'a> {
    /// A scanner at the start of some text.
    pub fn new(text: &'a str) -> Self {
        Self {
            bytes: text.as_bytes(),
            at: 0,
            failed: false,
        }
    }

    /// Whether reading stopped at something that was not a number, rather than
    /// at the end of the text.
    pub fn failed(&self) -> bool {
        self.failed
    }

    /// Whether everything has been read, with nothing but separators left.
    pub fn at_end(&mut self) -> bool {
        self.skip_separator();
        self.at >= self.bytes.len()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_some_and(is_whitespace) {
            self.at += 1;
        }
    }

    /// Whitespace, then at most one comma, then whitespace.
    fn skip_separator(&mut self) {
        self.skip_whitespace();
        if self.peek() == Some(b',') {
            self.at += 1;
            self.skip_whitespace();
        }
    }

    fn skip_digits(&mut self) -> usize {
        let start = self.at;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.at += 1;
        }
        self.at - start
    }

    /// One number, if the text here is one.
    fn number(&mut self) -> Option<f32> {
        let start = self.at;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.at += 1;
        }
        let whole = self.skip_digits();
        let mut fraction = 0;
        if self.peek() == Some(b'.') {
            self.at += 1;
            fraction = self.skip_digits();
        }
        if whole == 0 && fraction == 0 {
            self.at = start;
            return None;
        }
        // An exponent only counts if it has digits: `2em` is the number 2
        // followed by something else, not a malformed exponent.
        if matches!(self.peek(), Some(b'e' | b'E')) {
            let mark = self.at;
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if self.skip_digits() == 0 {
                self.at = mark;
            }
        }
        let text = core::str::from_utf8(self.bytes.get(start..self.at)?).ok()?;
        let value = text.parse::<f32>().ok().filter(|value| value.is_finite());
        if value.is_none() {
            self.at = start;
        }
        value
    }
}

impl Iterator for Numbers<'_> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.failed {
            return None;
        }
        if self.at > 0 {
            self.skip_separator();
        } else {
            self.skip_whitespace();
        }
        if self.at >= self.bytes.len() {
            return None;
        }
        let value = self.number();
        if value.is_none() {
            self.failed = true;
        }
        value
    }
}

/// The whole text as exactly one number, or [`None`].
pub fn number(text: &str) -> Option<f32> {
    let mut numbers = Numbers::new(text);
    let value = numbers.next()?;
    numbers.at_end().then_some(value)
}

/// SVG's whitespace, which is XML's: space, tab, line feed, carriage return.
fn is_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(text: &str) -> (Vec<f32>, bool) {
        let mut numbers = Numbers::new(text);
        let read: Vec<f32> = numbers.by_ref().collect();
        (read, numbers.failed())
    }

    #[test]
    fn numbers_are_separated_by_commas_whitespace_or_their_own_signs() {
        assert_eq!(
            all("1,2 3 , 4\t5\n6"),
            (vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], false)
        );
        assert_eq!(all("10-20+30"), (vec![10.0, -20.0, 30.0], false));
        assert_eq!(all(".5.5"), (vec![0.5, 0.5], false));
        assert_eq!(all("  "), (vec![], false));
    }

    #[test]
    fn exponents_are_numbers_and_a_bare_e_is_not_one() {
        assert_eq!(all("1e2 1.5E-1 -2e+1"), (vec![100.0, 0.15, -20.0], false));
        assert_eq!(number("2e"), None, "an exponent with no digits");
        assert_eq!(number("3"), Some(3.0));
        assert_eq!(number(" -0.25 "), Some(-0.25));
    }

    #[test]
    fn a_list_is_read_up_to_its_first_error() {
        assert_eq!(all("1 2 x 3"), (vec![1.0, 2.0], true));
        assert_eq!(all("1,,2"), (vec![1.0], true), "two commas is an error");
        assert_eq!(all("."), (vec![], true));
        assert_eq!(all("-"), (vec![], true));
    }

    #[test]
    fn an_overflowing_or_foreign_number_is_refused_not_infinite() {
        assert_eq!(all("1 1e99999 2"), (vec![1.0], true));
        assert_eq!(number("inf"), None);
        assert_eq!(number("NaN"), None);
        assert_eq!(number("1_000"), None);
        assert_eq!(number("1 2"), None, "two numbers are not one");
        assert_eq!(number(""), None);
    }

    #[test]
    fn a_very_long_number_is_read_without_trouble() {
        let long = format!("1{}", "0".repeat(10_000));
        assert_eq!(number(&long), None, "10^10000 does not fit a float");
        let digits = format!("0.{}1", "0".repeat(10_000));
        assert_eq!(number(&digits), Some(0.0));
    }
}
