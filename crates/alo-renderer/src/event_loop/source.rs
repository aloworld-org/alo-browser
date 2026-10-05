/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The scripts a loop has run, and where in one of them an offset is (queue
//! item 241, cut from 78).
//!
//! The engine places a throw by **program and byte offset**
//! ([`Unwound`]), and keeps no source text — so a line and a column are this
//! file's to work out, from the text the loop was handed. Every script the
//! loop compiles is kept here for the loop's life with the name its embedder
//! gave it, because a function a page's first script declared can throw while
//! its fifth is running, and the place to point at is in the first.
//!
//! # A column, in a file that is one line long
//!
//! Minified script is one line, and the column is the only thing that finds
//! anything in it. A column is counted in UTF-16 code units from the start of
//! its line ([`Position`]'s definition, which is every other engine's and every
//! developer tool's) — and counting from the start of the file for every place
//! of every report would let a page that throws in a loop make this renderer
//! read its megabyte bundle thirty-two times per throw. So the text is marked
//! once as it is kept, every [`MARK_EVERY`] bytes, with the line and column at
//! each mark; a place is counted from the nearest mark before it, which is at
//! most that many bytes of reading however long the script is.

use std::rc::Rc;

use alo_js::interpret::Unwound;
use alo_js::{Position, Unit};

/// How far apart the marks are, in bytes: the most a place reads to be found.
pub(super) const MARK_EVERY: usize = 4096;

/// Where a call a throw left was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// In a script this loop ran: its name, and a line and column counted
    /// from one, the column in UTF-16 code units.
    Script {
        /// The name the script was queued under.
        name: String,
        /// The line.
        line: usize,
        /// The column.
        column: usize,
    },
    /// In a program this loop did not compile — one an embedder ran on the
    /// engine itself — at a byte offset, which is all there is to say.
    Elsewhere {
        /// The byte offset.
        at: usize,
    },
}

/// A throw's places, innermost first, and how many calls further out were
/// left and not kept.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trace {
    /// The calls, innermost first.
    pub places: Vec<Place>,
    /// How many more there were.
    pub left_out: usize,
}

/// One script the loop compiled.
#[derive(Debug)]
struct Source {
    unit: Rc<Unit>,
    name: String,
    text: String,
    /// `(offset, line, column)` at the start of the text and then every
    /// [`MARK_EVERY`] bytes or so, at a character boundary that does not
    /// split a `\r\n`.
    marks: Vec<(usize, usize, usize)>,
}

/// Every script the loop has compiled.
#[derive(Debug, Default)]
pub(super) struct Sources {
    kept: Vec<Source>,
}

impl Sources {
    /// Keep a script the loop compiled, under its name.
    pub(super) fn keep(&mut self, unit: &Rc<Unit>, name: &str, text: &str) {
        self.kept.push(Source {
            unit: Rc::clone(unit),
            name: name.to_owned(),
            text: text.to_owned(),
            marks: marks(text),
        });
    }

    /// The places a throw left, in words a person can find.
    pub(super) fn trace(&self, unwound: &Unwound) -> Trace {
        Trace {
            places: unwound
                .places()
                .iter()
                .map(|place| self.place(place.unit(), place.at()))
                .collect(),
            left_out: unwound.left_out(),
        }
    }

    /// Where a byte offset in a program is.
    fn place(&self, unit: &Rc<Unit>, at: usize) -> Place {
        let Some(source) = self
            .kept
            .iter()
            .find(|source| Rc::ptr_eq(&source.unit, unit))
        else {
            return Place::Elsewhere { at };
        };
        let (line, column) = source.line_and_column(at);
        Place::Script {
            name: source.name.clone(),
            line,
            column,
        }
    }
}

impl Source {
    /// The line and column of a byte offset, counted from the mark before it.
    fn line_and_column(&self, at: usize) -> (usize, usize) {
        let at = at.min(self.text.len());
        let before = self.marks.partition_point(|&(offset, _, _)| offset <= at);
        let (offset, line, column) = before
            .checked_sub(1)
            .and_then(|which| self.marks.get(which))
            .copied()
            .unwrap_or((0, 1, 1));
        let Some(rest) = self.text.get(offset..) else {
            return (line, column);
        };
        let further = Position::of(rest, at.saturating_sub(offset));
        if further.line == 1 {
            (
                line,
                column.saturating_add(further.column).saturating_sub(1),
            )
        } else {
            (
                line.saturating_add(further.line).saturating_sub(1),
                further.column,
            )
        }
    }
}

/// The marks of a text: where each is, and the line and column there.
fn marks(text: &str) -> Vec<(usize, usize, usize)> {
    let mut marks = vec![(0, 1, 1)];
    let mut last: (usize, usize, usize) = (0, 1, 1);
    let mut next = MARK_EVERY;
    while next < text.len() {
        // A mark must be somewhere `text.get(mark..)` can start, and must not
        // sit between the two halves of a `\r\n`, which are one line ending.
        while next < text.len()
            && (!text.is_char_boundary(next)
                || (text.as_bytes().get(next) == Some(&b'\n')
                    && text.as_bytes().get(next.saturating_sub(1)) == Some(&b'\r')))
        {
            next = next.saturating_add(1);
        }
        if next >= text.len() {
            break;
        }
        let (offset, line, column) = last;
        let further = text
            .get(offset..)
            .map(|rest| Position::of(rest, next.saturating_sub(offset)));
        let Some(further) = further else {
            break;
        };
        last = if further.line == 1 {
            (
                next,
                line,
                column.saturating_add(further.column).saturating_sub(1),
            )
        } else {
            (
                next,
                line.saturating_add(further.line).saturating_sub(1),
                further.column,
            )
        };
        marks.push(last);
        next = next.saturating_add(MARK_EVERY);
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Line and column from the marks, and from the start: at every offset of
    /// a short text, and in a long one at every offset near a mark — where a
    /// mistake in carrying a line or a column across would show — and at
    /// every 61st offset besides. (Counting from the start at every offset of
    /// a long text is the quadratic reading the marks exist to avoid.)
    fn agree(text: &str) {
        let source = Source {
            unit: Rc::new(Unit::new()),
            name: "s".to_owned(),
            text: text.to_owned(),
            marks: marks(text),
        };
        let near_a_mark = |at: usize| {
            source
                .marks
                .iter()
                .any(|&(offset, _, _)| at.abs_diff(offset) <= 16)
        };
        for at in 0..=text.len() {
            let wanted = text.len() <= 2 * MARK_EVERY || at % 61 == 0 || near_a_mark(at);
            if !wanted || !text.is_char_boundary(at) {
                continue;
            }
            let whole = Position::of(text, at);
            assert_eq!(
                source.line_and_column(at),
                (whole.line, whole.column),
                "at byte {at}"
            );
        }
    }

    #[test]
    fn a_short_script_has_one_mark() {
        assert_eq!(marks("a\nb"), vec![(0, 1, 1)]);
        agree("a\nb\r\nc\u{2028}d");
    }

    #[test]
    fn marks_agree_with_counting_from_the_start() {
        // Long lines, short lines, each kind of line ending, and characters
        // of one to four bytes and one or two code units, so that marks land
        // inside a character, inside a `\r\n` and mid-line.
        let mut text = String::new();
        for round in 0..3000 {
            text.push_str(match round % 6 {
                0 => "let a = 1;\n",
                1 => "é€😀 ",
                2 => "x\r\n",
                3 => "\r",
                4 => "\u{2029}",
                _ => "abcdefghij",
            });
        }
        assert!(text.len() > 3 * MARK_EVERY);
        assert!(marks(&text).len() > 3);
        agree(&text);
    }

    #[test]
    fn a_mark_never_splits_a_line_ending() {
        let mut text = "a".repeat(MARK_EVERY - 1);
        text.push_str("\r\nb");
        let made = marks(&text);
        assert!(made.iter().all(|&(offset, _, _)| offset != MARK_EVERY));
        agree(&text);
    }

    #[test]
    fn a_one_line_bundle_is_found_by_its_column() {
        let text = "x;".repeat(3 * MARK_EVERY);
        agree(&text);
        let source = Source {
            unit: Rc::new(Unit::new()),
            name: "s".to_owned(),
            text: text.clone(),
            marks: marks(&text),
        };
        assert_eq!(source.line_and_column(5000), (1, 5001));
        // Past the end is the end, rather than a place that does not exist.
        assert_eq!(source.line_and_column(usize::MAX), (1, text.len() + 1));
    }

    #[test]
    fn a_program_nobody_kept_is_said_by_its_offset() {
        let sources = Sources::default();
        assert_eq!(
            sources.place(&Rc::new(Unit::new()), 9),
            Place::Elsewhere { at: 9 }
        );
    }
}
