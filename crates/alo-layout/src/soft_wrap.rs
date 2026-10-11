/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whether a line may break before an atomic box.
//!
//! An `inline-block` or a picture has no `white-space` that speaks for the
//! line it stands in: its own governs what is inside it. Which box does is
//! CSS Text 3 § 5.1's rule, and Chrome 154 follows it (measured, queue item
//! 447, in boxes 80 wide holding a box 60 wide):
//!
//! - **After a space**, the box holding the space decides, because the
//!   space is where the line would break. `ab <span normal>x </span><ib>`
//!   in a `nowrap` block breaks before the box; `ab <span normal><ib>` in
//!   the same block does not, because its space is the block's.
//! - **Otherwise**, the nearest box holding both what came before and the
//!   atomic box decides. `abc<span normal><ib>` in a `nowrap` block does not
//!   break, though the box is in a `normal` span, and `abc<span
//!   nowrap><ib>` in a `normal` block does.
//!
//! So a box in a `nowrap` line stays on it and overflows, as text does,
//! where this engine used to start a new line for it whatever the line said.
//!
//! The line builder says what it lays down and how deep, and asks this file
//! before it places an atomic box on a line already holding something.

/// What the line builder has laid down most recently, as far as a break
/// before the next atomic box is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SoftWrap {
    /// Whether the last thing laid down was a space, and if so whether its
    /// box lets a line break after it.
    space: Option<bool>,
    /// How many inline boxes deep the nearest box holding both the last
    /// thing laid down and whatever comes next is: the depth it was laid at,
    /// or less once a box it was in has closed. Nought is the block.
    shared: usize,
}

impl SoftWrap {
    /// Nothing laid down yet.
    pub(crate) const START: SoftWrap = SoftWrap {
        space: None,
        shared: 0,
    };

    /// Something that is not a space — a letter, an atomic box — was laid
    /// down `depth` inline boxes deep.
    pub(crate) fn content(&mut self, depth: usize) {
        *self = SoftWrap {
            space: None,
            shared: depth,
        };
    }

    /// A space was laid down `depth` inline boxes deep, in a box whose
    /// `white-space` lets a line break after it or not.
    pub(crate) fn space(&mut self, depth: usize, wraps: bool) {
        *self = SoftWrap {
            space: Some(wraps),
            shared: depth,
        };
    }

    /// An inline box closed, leaving `depth` open: whatever comes next is
    /// no longer inside it, so it is no longer the box both are in.
    pub(crate) fn closed(&mut self, depth: usize) {
        self.shared = self.shared.min(depth);
    }

    /// Whether the line may break before an atomic box laid down now.
    ///
    /// `wraps_at` says whether the box `depth` deep lets lines wrap: nought
    /// is the block holding the lines, one the outermost open inline box.
    pub(crate) fn allows(&self, wraps_at: impl Fn(usize) -> bool) -> bool {
        self.space.unwrap_or_else(|| wraps_at(self.shared))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `nowrap` block holding a `normal` span holding a `nowrap` one.
    fn nested(depth: usize) -> bool {
        depth == 1
    }

    #[test]
    fn after_a_space_the_spaces_box_decides() {
        let mut wrap = SoftWrap::START;
        wrap.space(1, true);
        wrap.closed(0);
        assert!(wrap.allows(nested), "x </span><ib>: the span's space");
        wrap.space(0, false);
        assert!(!wrap.allows(|_| true), "ab <span normal><ib>: the block's");
    }

    #[test]
    fn otherwise_the_box_holding_both_decides() {
        let mut wrap = SoftWrap::START;
        wrap.content(0);
        assert!(!wrap.allows(nested), "abc<span normal><ib>: the block");
        wrap.content(1);
        assert!(wrap.allows(nested), "<span normal>x<ib>: the span");
        wrap.content(2);
        wrap.closed(1);
        assert!(wrap.allows(nested), "the inner span closed: the outer one");
        wrap.closed(0);
        assert!(!wrap.allows(nested), "both closed: the block");
    }

    #[test]
    fn a_box_opening_after_does_not_change_who_decides() {
        // `closed` only ever lowers the depth, and an opening is not
        // reported at all: the box both are in is no deeper than the last
        // thing laid down.
        let mut wrap = SoftWrap::START;
        wrap.content(1);
        wrap.closed(3);
        assert_eq!(wrap.shared, 1);
    }

    #[test]
    fn laying_something_down_forgets_the_space_before_it() {
        let mut wrap = SoftWrap::START;
        wrap.space(0, true);
        wrap.content(0);
        assert!(!wrap.allows(|_| false));
    }
}
