/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A pattern, parsed: what [`parse`](super::parse) makes and
//! [`emit`](super::emit) compiles.
//!
//! The tree is the whole grammar, including what is not built yet — a
//! property escape, a `v` class with its operations — because the parser has
//! to read all of it to say whether the pattern is one at all (ADR 0029 § 6).
//! What cannot be compiled yet is refused by name when it is compiled.
//!
//! A character is a `u32`: a code unit without `u` or `v`, a code point with
//! either. That is the specification's own reading — the same pattern text is
//! a different list of characters in the two modes — and keeping one type for
//! both means the mode is decided once, when the text is read.

/// A whole pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    /// The disjunction it is.
    pub body: Node,
    /// How many capturing groups it has.
    pub groups: u32,
    /// The name of each capturing group, by its number less one.
    pub names: Vec<Option<Vec<u16>>>,
}

/// One piece of a pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// `a|b|c`: alternatives, tried in order.
    Alternatives(Vec<Node>),
    /// `abc`: terms, one after another. Empty is the empty alternative.
    Sequence(Vec<Node>),
    /// One character.
    Char(u32),
    /// `.`.
    Any,
    /// `[…]`, or `\d` and its kin outside a class.
    Class(Class),
    /// `^`, `$`, `\b` or `\B`.
    Assertion(Assertion),
    /// `(?=…)`, `(?!…)`, `(?<=…)` or `(?<!…)`.
    Look {
        /// Whether it looks before the position rather than after it.
        behind: bool,
        /// Whether it succeeds when its body does not.
        negated: bool,
        /// What it looks for.
        body: Box<Node>,
    },
    /// `(…)`, `(?<name>…)` or `(?:…)`.
    Group {
        /// The group's number, counting from one, if it captures.
        capture: Option<u32>,
        /// What is inside it.
        body: Box<Node>,
    },
    /// `(?ims-ims:…)`: a group whose flags differ from the pattern's.
    Modified {
        /// The flags it turns on.
        add: Modifiers,
        /// The flags it turns off.
        remove: Modifiers,
        /// What is inside it.
        body: Box<Node>,
    },
    /// A term and how many times it repeats.
    Repeat {
        /// What repeats.
        body: Box<Node>,
        /// The fewest times.
        min: u32,
        /// The most times, where [`u32::MAX`] is without limit — no string is
        /// long enough to tell the two apart.
        max: u32,
        /// Whether it takes as many as it can first.
        greedy: bool,
        /// How many capturing groups come before it.
        before: u32,
        /// How many capturing groups are inside it.
        inside: u32,
    },
    /// `\1` or `\k<name>`: what a group captured, again.
    Backreference(Reference),
    /// `\p{…}` or `\P{…}`, which needs the rented Unicode tables (item 322).
    Property(Property),
}

/// Which group a backreference names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reference {
    /// `\1`: by number, counting from one.
    Number(u32),
    /// `\k<name>`: every group of that name, of which at most one can have
    /// taken part in a match.
    Name(Vec<u16>),
}

/// An assertion, which matches a position rather than a character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assertion {
    /// `^`.
    Start,
    /// `$`.
    End,
    /// `\b`.
    Boundary,
    /// `\B`.
    NotBoundary,
}

/// The flags a modified group may change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// `i`.
    pub ignore_case: bool,
    /// `m`.
    pub multiline: bool,
    /// `s`.
    pub dot_all: bool,
}

impl Modifiers {
    /// Whether it changes nothing.
    pub const fn is_empty(self) -> bool {
        !self.ignore_case && !self.multiline && !self.dot_all
    }
}

/// `\p{…}`, read but not looked up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// `\P` rather than `\p`.
    pub negated: bool,
    /// The name, or the lone name-or-value.
    pub name: String,
    /// The value after `=`, if there was one.
    pub value: Option<String>,
}

/// A class: `[…]`, `[^…]`, or an escape that stands for one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Class {
    /// `[^…]`.
    pub negated: bool,
    /// What is in it.
    pub body: ClassBody,
}

/// What is in a class, in the two grammars the language has for one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassBody {
    /// Without `v`: characters, ranges and escapes, whose union it is.
    Ranges(Vec<ClassItem>),
    /// With `v`: a set expression.
    Set(SetExpression),
}

/// One thing in a class without `v`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassItem {
    /// A character.
    Char(u32),
    /// `a-z`, both ends included.
    Range(u32, u32),
    /// `\d`, `\s`, `\w` or their complements.
    Escape(ClassEscape),
    /// `\p{…}` under `u`.
    Property(Property),
}

/// The class escapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassEscape {
    /// `\d`.
    Digit,
    /// `\D`.
    NotDigit,
    /// `\s`.
    Space,
    /// `\S`.
    NotSpace,
    /// `\w`.
    Word,
    /// `\W`.
    NotWord,
}

/// A `v` class's contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetExpression {
    /// Operands and ranges side by side.
    Union(Vec<SetOperand>),
    /// `a&&b&&c`.
    Intersection(Vec<SetOperand>),
    /// `a--b--c`.
    Subtraction(Vec<SetOperand>),
}

/// One operand of a `v` class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetOperand {
    /// A character.
    Char(u32),
    /// A range, which only a union may hold.
    Range(u32, u32),
    /// A class inside the class.
    Nested(Class),
    /// `\q{abc|d}`: strings.
    Strings(Vec<Vec<u32>>),
    /// `\d` and its kin.
    Escape(ClassEscape),
    /// `\p{…}`.
    Property(Property),
}
