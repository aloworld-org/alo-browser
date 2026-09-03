/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What happens to the spaces, tabs and newlines a document is written with.
//!
//! Markup is written for people, so it is full of whitespace nobody meant to
//! see: an indented `<p>` begins with a newline and four spaces, and
//! `one   two` in a source file is one space on the screen. CSS calls this
//! *white space processing*, and until this file the engine did none of it —
//! it shaped whatever bytes the parser handed over, so an indented paragraph
//! was drawn with its indentation in it.
//!
//! # Two questions, and they are separate
//!
//! **What survives**, which is here: runs of spaces become one space, and
//! newlines either become spaces or stay. **Where a line may break**, which is
//! the line builder's: a preserved newline is a break that *must* happen, and
//! `nowrap` forbids the ones that may.
//!
//! Collapsing happens when the box is built rather than when it is drawn, so
//! that everything downstream — layout, paint, the agent tree — reads the same
//! text. Two collapsings would eventually disagree, which is the argument
//! ADR 0002 makes about two trees.

use core::fmt;

/// How whitespace in a box's text is treated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WhiteSpace {
    /// Runs of whitespace become one space, newlines included. Lines wrap.
    #[default]
    Normal,
    /// Everything is kept, and lines never wrap on their own.
    Pre,
    /// Everything is kept, and lines wrap as well.
    PreWrap,
    /// Runs of spaces and tabs become one space; **newlines are kept**. Lines
    /// wrap.
    PreLine,
    /// Collapsed like `normal`, and lines never wrap on their own.
    NoWrap,
}

impl WhiteSpace {
    /// What a `white-space` value says, or [`None`] for one this engine does
    /// not implement.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        for (value, name) in [
            (WhiteSpace::Normal, "normal"),
            (WhiteSpace::Pre, "pre"),
            (WhiteSpace::PreWrap, "pre-wrap"),
            (WhiteSpace::PreLine, "pre-line"),
            (WhiteSpace::NoWrap, "nowrap"),
        ] {
            if text.eq_ignore_ascii_case(name) {
                return Some(value);
            }
        }
        None
    }

    /// Whether a newline in the source is a line break on the screen.
    pub fn keeps_newlines(self) -> bool {
        matches!(
            self,
            WhiteSpace::Pre | WhiteSpace::PreWrap | WhiteSpace::PreLine
        )
    }

    /// Whether runs of spaces and tabs are squeezed to one.
    pub fn collapses_spaces(self) -> bool {
        matches!(
            self,
            WhiteSpace::Normal | WhiteSpace::NoWrap | WhiteSpace::PreLine
        )
    }

    /// Whether a line may break where the text allows it.
    ///
    /// `pre` and `nowrap` say no: a line ends where the author put a newline,
    /// or it overflows.
    pub fn wraps(self) -> bool {
        !matches!(self, WhiteSpace::Pre | WhiteSpace::NoWrap)
    }

    /// The text as it should be measured and drawn.
    ///
    /// Leading and trailing whitespace is **kept as a single space** rather
    /// than removed: the space between `<a>All</a>` and `<a>Due</a>` is a whole
    /// text box of its own, and dropping it here would render `AllDue`. Where a
    /// line actually begins or ends is the line builder's to decide, because
    /// only it knows what is beside what.
    pub fn apply(self, text: &str) -> String {
        if !self.collapses_spaces() && self.keeps_newlines() {
            return text.to_owned();
        }
        let mut out = String::with_capacity(text.len());
        let mut in_whitespace = false;
        for character in text.chars() {
            let is_newline = character == '\n' || character == '\r';
            if is_newline && self.keeps_newlines() {
                // A kept newline ends whatever run of spaces led up to it, and
                // is not itself a run: `a  \n  b` is one break, not three.
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push('\n');
                in_whitespace = true;
                continue;
            }
            if character.is_whitespace() {
                if !in_whitespace {
                    out.push(' ');
                    in_whitespace = true;
                }
                continue;
            }
            in_whitespace = false;
            out.push(character);
        }
        out
    }
}

impl fmt::Display for WhiteSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            WhiteSpace::Normal => "normal",
            WhiteSpace::Pre => "pre",
            WhiteSpace::PreWrap => "pre-wrap",
            WhiteSpace::PreLine => "pre-line",
            WhiteSpace::NoWrap => "nowrap",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_squeezes_every_kind_of_whitespace_into_one_space() {
        assert_eq!(WhiteSpace::Normal.apply("one   two"), "one two");
        assert_eq!(WhiteSpace::Normal.apply("one\n\ttwo"), "one two");
        assert_eq!(
            WhiteSpace::Normal.apply("\n      Indented markup\n    "),
            " Indented markup ",
            "the indentation a person wrote is not the indentation they meant",
        );
    }

    #[test]
    fn pre_line_keeps_the_newlines_and_squeezes_the_rest() {
        assert_eq!(
            WhiteSpace::PreLine.apply("Your workspace.\nYour servers.\nYour rules."),
            "Your workspace.\nYour servers.\nYour rules.",
        );
        assert_eq!(WhiteSpace::PreLine.apply("one   two"), "one two");
        assert_eq!(
            WhiteSpace::PreLine.apply("one  \n  two"),
            "one\ntwo",
            "a break eats the spaces around it rather than adding to them",
        );
    }

    #[test]
    fn pre_keeps_everything_exactly_as_it_was_written() {
        let source = "  one   two\n\n\tthree  ";
        assert_eq!(WhiteSpace::Pre.apply(source), source);
        assert_eq!(WhiteSpace::PreWrap.apply(source), source);
    }

    #[test]
    fn nowrap_collapses_like_normal_and_refuses_to_break() {
        assert_eq!(WhiteSpace::NoWrap.apply("one \n two"), "one two");
        assert!(!WhiteSpace::NoWrap.wraps());
        assert!(!WhiteSpace::Pre.wraps());
        assert!(WhiteSpace::Normal.wraps());
        assert!(WhiteSpace::PreLine.wraps());
        assert!(WhiteSpace::PreWrap.wraps());
    }

    #[test]
    fn the_space_between_two_words_in_two_boxes_survives() {
        // `<a>All</a> <a>Due</a>` gives the middle text box one space, and it
        // is the whole reason "AllDue" is not what that renders as.
        assert_eq!(WhiteSpace::Normal.apply(" "), " ");
        assert_eq!(WhiteSpace::Normal.apply("\n   "), " ");
    }

    #[test]
    fn a_value_this_engine_does_not_implement_is_refused() {
        assert_eq!(WhiteSpace::parse("PRE-LINE"), Some(WhiteSpace::PreLine));
        assert_eq!(WhiteSpace::parse("break-spaces"), None);
        assert_eq!(WhiteSpace::parse("collapse"), None);
        assert_eq!(WhiteSpace::parse(""), None);
    }
}
