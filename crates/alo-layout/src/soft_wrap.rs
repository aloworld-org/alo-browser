/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether a line may break where one thing laid down meets the next.
//!
//! Inside one piece of text the text's own `white-space` decides. Where two
//! boxes meet — a word and a `<span>` after it, an `inline-block` and the
//! text beside it — something has to say which box's `white-space` speaks
//! for the break. The next thing's own does not: `ab <span nowrap>cdefghij`
//! in a `normal` block breaks at the space, before the span. Chrome 154 was
//! measured in boxes 80 wide (queue items 447 and 449), and its rule is two
//! rules:
//!
//! - **After a space**, the box holding the space decides (CSS Text 3 § 5.1),
//!   or the box holding what came before the space, because Chrome breaks
//!   before a run of spaces as well as after it. `ab<span normal> cdefghij`
//!   in a `nowrap` block breaks, by the span's space; `<span normal>abcd</span>
//!   efghij` in `nowrap` breaks, by the span's "abcd"; `ab <span normal>cd…`
//!   in `nowrap` does not, the space and the "ab" both the block's. A box
//!   closed between the two does not count: `<span nowrap>x </span>cd…` in a
//!   `normal` block keeps "x " with "cd".
//! - **Otherwise** — a hyphen, an atomic box, anything that is not a space —
//!   the box holding what came before decides, or any box it was in that has
//!   closed since, out to the box it closed into. `<span nowrap>abc-</span>
//!   defghij` in a `normal` block breaks after the hyphen, by the block it
//!   closed into; `<span normal>abc-</span>defghij` in `nowrap` breaks, by
//!   the span; `abc-<span normal>defghij` in `nowrap` does not, because a box
//!   opening after does not count. After an atomic box this is the box it is
//!   in: `<span normal>ab<ib></span>de` in `nowrap` breaks before "de".
//!
//! CSS Text 3 asks the nearest box holding both sides instead, and Chrome
//! agrees with it everywhere but two places: an atomic box's own box and a
//! box closed between, each of which lets Chrome break where CSS would not.
//! This file follows Chrome, which is what alo is compared against.
//!
//! Whether there is a place to break at all is the line builder's question:
//! between two letters there is none, beside an atomic box there always is
//! (CSS Text 3 § 5.1, a no-break space included), and after a space there
//! always is. This file answers only whether a break there is allowed.

/// What was laid down last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Last {
    /// Nothing yet: the block's own `white-space` answers.
    Nothing,
    /// A letter, or anything else in text that is not a space.
    Letter,
    /// An `inline-block`, a picture, a field.
    Atomic,
    /// One or more spaces.
    Space,
}

/// What the line builder has laid down most recently, as far as a break
/// before the next thing is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SoftWrap {
    last: Last,
    /// After a letter or an atomic box: whether its box, or any box it was in
    /// that has closed since, lets a line wrap.
    content: bool,
    /// Whether the box holding the last letter or atomic box lets a line
    /// wrap, closed boxes not counted: what decides a break before a space
    /// laid down after it.
    own: bool,
    /// After a space: whether a line may break after it — its box wraps, or
    /// what came before it allowed a break before it.
    space: bool,
    /// How many inline boxes deep the last thing was laid, or less once a box
    /// it was in has closed. Nought is the block.
    shared: usize,
}

impl SoftWrap {
    /// Nothing laid down yet, in a block whose `white-space` wraps or not.
    pub(crate) fn start(wraps: bool) -> SoftWrap {
        SoftWrap {
            last: Last::Nothing,
            content: wraps,
            own: wraps,
            space: false,
            shared: 0,
        }
    }

    /// A letter — anything in text that is not a space — was laid down
    /// `depth` inline boxes deep, in a box that wraps or not.
    pub(crate) fn letter(&mut self, depth: usize, wraps: bool) {
        self.content(Last::Letter, depth, wraps);
    }

    /// An atomic box was laid down `depth` inline boxes deep, in a box that
    /// wraps or not.
    pub(crate) fn atomic(&mut self, depth: usize, wraps: bool) {
        self.content(Last::Atomic, depth, wraps);
    }

    fn content(&mut self, last: Last, depth: usize, wraps: bool) {
        *self = SoftWrap {
            last,
            content: wraps,
            own: wraps,
            space: false,
            shared: depth,
        };
    }

    /// A space was laid down `depth` inline boxes deep, in a box whose
    /// `white-space` lets a line wrap or not.
    pub(crate) fn space(&mut self, depth: usize, wraps: bool) {
        let before = match self.last {
            Last::Nothing => false,
            Last::Letter | Last::Atomic => self.own,
            Last::Space => self.space,
        };
        self.last = Last::Space;
        self.space = before || wraps;
        self.shared = depth;
    }

    /// An inline box closed, leaving `depth` open, the innermost of which
    /// (or the block, at nought) wraps or not. A break after a letter or an
    /// atomic box is allowed by the box it closed into as well; a break
    /// after a space is not.
    pub(crate) fn closed(&mut self, depth: usize, wraps: bool) {
        if depth < self.shared {
            self.shared = depth;
            if self.last != Last::Space {
                self.content = self.content || wraps;
            }
        }
    }

    /// Whether what was laid last was a letter, after which there is a place
    /// to break only where the text on both sides says there is.
    pub(crate) fn after_letter(&self) -> bool {
        self.last == Last::Letter
    }

    /// Whether a line may break before whatever is laid down next, given
    /// that there is a place to break there.
    pub(crate) fn allows(&self) -> bool {
        match self.last {
            Last::Space => self.space,
            Last::Nothing | Last::Letter | Last::Atomic => self.content,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn after_a_space_the_spaces_box_decides() {
        // `ab<span normal> cd` in `nowrap`: the span's space.
        let mut wrap = SoftWrap::start(false);
        wrap.letter(0, false);
        wrap.space(1, true);
        assert!(wrap.allows());
        // `ab <span normal>cd` in `nowrap`: the block's.
        let mut wrap = SoftWrap::start(false);
        wrap.letter(0, false);
        wrap.space(0, false);
        assert!(!wrap.allows());
    }

    #[test]
    fn after_a_space_what_came_before_it_decides_as_well() {
        // `<span normal>abcd</span> efgh` in `nowrap`.
        let mut wrap = SoftWrap::start(false);
        wrap.letter(1, true);
        wrap.closed(0, false);
        wrap.space(0, false);
        assert!(wrap.allows());
        // `<span normal>ab<ib></span> cd` in `nowrap`.
        let mut wrap = SoftWrap::start(false);
        wrap.atomic(1, true);
        wrap.closed(0, false);
        wrap.space(0, false);
        assert!(wrap.allows());
    }

    #[test]
    fn a_box_closed_before_a_space_does_not_count() {
        // `<span nowrap>abcd</span><span nowrap> </span>efgh` in `normal`.
        let mut wrap = SoftWrap::start(true);
        wrap.letter(1, false);
        wrap.closed(0, true);
        wrap.space(1, false);
        wrap.closed(0, true);
        assert!(!wrap.allows());
    }

    #[test]
    fn a_box_closed_after_a_space_does_not_count() {
        // `<span nowrap>x </span>cd` in `normal`.
        let mut wrap = SoftWrap::start(true);
        wrap.letter(1, false);
        wrap.space(1, false);
        wrap.closed(0, true);
        assert!(!wrap.allows());
    }

    #[test]
    fn otherwise_the_box_before_or_one_it_closed_into_decides() {
        // `<span nowrap>abc-</span>def` in `normal`: the block it closed
        // into.
        let mut wrap = SoftWrap::start(true);
        wrap.letter(1, false);
        assert!(!wrap.allows());
        wrap.closed(0, true);
        assert!(wrap.allows());
        // `<span normal>abc-</span>def` in `nowrap`: the span.
        let mut wrap = SoftWrap::start(false);
        wrap.letter(1, true);
        wrap.closed(0, false);
        assert!(wrap.allows());
        // `abc-<span normal>def` in `nowrap`: opening does not count.
        let mut wrap = SoftWrap::start(false);
        wrap.letter(0, false);
        assert!(!wrap.allows());
    }

    #[test]
    fn a_box_opened_and_closed_after_does_not_count() {
        // `abc<span normal></span>def` in `nowrap`: the span closes back to
        // the depth the "abc" was laid at, so it is not a box the "abc" was
        // in.
        let mut wrap = SoftWrap::start(false);
        wrap.letter(0, false);
        wrap.closed(1, true);
        assert!(!wrap.allows());
        assert_eq!(wrap.shared, 0);
    }

    #[test]
    fn an_atomic_box_answers_by_the_box_it_is_in() {
        let mut wrap = SoftWrap::start(false);
        wrap.atomic(1, true);
        assert!(wrap.allows());
        assert!(!wrap.after_letter());
        wrap.letter(1, true);
        assert!(wrap.after_letter());
    }

    #[test]
    fn nothing_yet_is_the_block() {
        assert!(SoftWrap::start(true).allows());
        assert!(!SoftWrap::start(false).allows());
    }

    #[test]
    fn a_run_of_spaces_keeps_what_any_of_them_allowed() {
        // `<span normal>a </span> b` in `nowrap`: the span's space allows a
        // break, and the block's after it does not take that back.
        let mut wrap = SoftWrap::start(false);
        wrap.letter(1, true);
        wrap.space(1, true);
        wrap.closed(0, false);
        wrap.space(0, false);
        assert!(wrap.allows());
    }
}
