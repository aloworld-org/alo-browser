/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A compiled pattern: the instructions the [`matcher`](super::matcher) runs.
//!
//! ADR 0029 § 2: *parsed to a tree, compiled to instructions, run by a loop*.
//! The instructions are a backtracking machine's. A choice is a [`Inst::Split`]
//! whose second way is written down to come back to; a repeat keeps its count
//! in a counter rather than in a native frame; a lookaround marks where it
//! began so that leaving it can forget every choice made inside. The program
//! holds no heap reference at all, which is why the collector never traces
//! one and why one program is shared by every object a literal makes
//! (ADR 0029 § 5).
//!
//! # Direction is in the instruction
//!
//! A lookbehind matches its body **right to left**: its terms in reverse
//! order, each character read from before the position. That is the
//! specification's meaning — `(?<=(\d+)(\d+))$` against `1053` captures `1`
//! and `053`, because the second group, read first, is greedy — and it is
//! compiled in rather than decided at run time: every instruction that reads
//! characters says which way it reads.

use super::flags::Flags;
use super::set::Set;

/// A character an instruction compares against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Single {
    /// Exactly this character.
    Char(u32),
    /// `.`: anything but a line ending, or anything at all under `s`.
    Any {
        /// Whether a line ending matches too.
        line_endings: bool,
    },
    /// One of the program's sets.
    Set(u32),
}

/// One instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inst {
    /// Read one character and require it to match.
    Read {
        /// What it must be.
        single: Single,
        /// Whether it is read from before the position.
        back: bool,
    },
    /// Read as many characters that match as the repeat allows, holding one
    /// place to come back to however many it read: `.*`, `\s+`, `[a-z]{2,5}?`.
    ///
    /// Only a single character is repeated this way, and that is what makes
    /// it the same as the general repeat: one character cannot match the empty
    /// string and holds no capture, so there is no empty check to make and
    /// nothing to reset.
    Many {
        /// What each must be.
        single: Single,
        /// Whether they are read from before the position.
        back: bool,
        /// The fewest.
        min: u32,
        /// The most, where [`u32::MAX`] is without limit.
        max: u32,
        /// Whether it takes as many as it can before trying fewer.
        greedy: bool,
    },
    /// `^`.
    Start {
        /// Whether the position after a line ending counts.
        multiline: bool,
    },
    /// `$`.
    End {
        /// Whether the position before a line ending counts.
        multiline: bool,
    },
    /// `\b`, or `\B` when negated.
    Boundary {
        /// `\B`.
        negated: bool,
    },
    /// Go to `first`, and come back to `second` if what follows fails.
    Split {
        /// The way tried first.
        first: u32,
        /// The way tried if it fails.
        second: u32,
    },
    /// Go to an instruction.
    Jump(u32),
    /// Write the position into a capture slot: slot `2n` is where group `n`
    /// began and `2n + 1` where it ended.
    Save(u32),
    /// Forget the captures in slots `from` to `to`, not including `to`: a
    /// repeat's groups, at the start of each iteration.
    Clear {
        /// The first slot.
        from: u32,
        /// The slot past the last.
        to: u32,
    },
    /// Match what one of the groups in a list captured, again.
    Backreference {
        /// Which list, in [`Program::references`].
        list: u32,
        /// Whether it is matched before the position.
        back: bool,
    },
    /// Set a repeat's count to zero.
    RepeatEnter {
        /// Its counter.
        counter: u32,
    },
    /// Decide whether a repeat goes round again: always while it has fewer
    /// than its least, never once it has its most, and otherwise both ways
    /// in the order its greediness says. The body follows this instruction.
    RepeatTest {
        /// Its counter.
        counter: u32,
        /// The fewest.
        min: u32,
        /// The most, where [`u32::MAX`] is without limit.
        max: u32,
        /// Whether going round again is tried first.
        greedy: bool,
        /// The instruction after the repeat.
        exit: u32,
    },
    /// Remember where an iteration began, for the empty check.
    RepeatMark {
        /// Its counter.
        counter: u32,
    },
    /// End an iteration: fail one that matched nothing and was not needed to
    /// reach the least — the specification's empty check, which is what stops
    /// `(a*)*` going round for ever — then count it and test again.
    RepeatLoop {
        /// Its counter.
        counter: u32,
        /// The fewest.
        min: u32,
        /// The [`Inst::RepeatTest`] to go back to.
        test: u32,
    },
    /// Begin a lookaround.
    LookEnter {
        /// Whether it looks behind.
        behind: bool,
        /// Whether it succeeds when its body fails.
        negated: bool,
        /// The instruction after its [`Inst::LookLeave`].
        exit: u32,
    },
    /// The body of the innermost lookaround matched.
    LookLeave,
    /// The whole pattern matched.
    Match,
}

/// A compiled pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// The instructions.
    pub(super) code: Vec<Inst>,
    /// The sets [`Single::Set`] names.
    pub(super) sets: Vec<Set>,
    /// The groups each [`Inst::Backreference`] may name.
    pub(super) references: Vec<Vec<u32>>,
    /// How many counters the repeats use.
    pub(super) counters: u32,
    /// How many capturing groups, not counting the whole match.
    groups: u32,
    /// Each group's name, by its number less one.
    names: Vec<Option<Vec<u16>>>,
    /// The flags.
    flags: Flags,
    /// The pattern's text, as code units: `[[OriginalSource]]`.
    source: Vec<u16>,
}

impl Program {
    /// A program of this code, with what it was compiled from.
    pub(super) fn new(
        code: Vec<Inst>,
        sets: Vec<Set>,
        references: Vec<Vec<u32>>,
        counters: u32,
        pattern: (u32, Vec<Option<Vec<u16>>>),
        flags: Flags,
        source: &str,
    ) -> Self {
        let (groups, names) = pattern;
        Self {
            code,
            sets,
            references,
            counters,
            groups,
            names,
            flags,
            source: source.encode_utf16().collect(),
        }
    }

    /// How many capturing groups it has.
    pub const fn groups(&self) -> u32 {
        self.groups
    }

    /// The name of group `n`, counting from one, if it has one.
    pub fn name(&self, n: u32) -> Option<&[u16]> {
        let at = usize::try_from(n.checked_sub(1)?).ok()?;
        self.names.get(at)?.as_deref()
    }

    /// Whether any group has a name, which decides whether a match has a
    /// `groups` object at all.
    pub fn has_names(&self) -> bool {
        self.names.iter().any(Option::is_some)
    }

    /// Its flags.
    pub const fn flags(&self) -> &Flags {
        &self.flags
    }

    /// The pattern's text.
    pub fn source(&self) -> &[u16] {
        &self.source
    }

    /// How many instructions it is.
    pub fn len(&self) -> usize {
        self.code.len()
    }

    /// Whether it has no instructions, which no compiled program has: every
    /// one ends in [`Inst::Match`].
    pub fn is_empty(&self) -> bool {
        self.code.is_empty()
    }
}
