/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A pattern's text into a [`Pattern`], or the early `SyntaxError` it is.
//!
//! ADR 0029 § 4: the grammar is the current specification's — `u` and `v`,
//! named groups and the same name in two alternatives, lookbehind, and the
//! `(?ims-ims:…)` modifiers — **all of it**, including the parts nothing
//! compiles yet, because whether a pattern is a pattern is a question about
//! the whole grammar. Annex B's extensions are refused by name
//! ([`Legacy`]).
//!
//! # The text is read as the mode reads it
//!
//! Without `u` or `v` a pattern is a list of UTF-16 code units, so `😀+`
//! repeats only the second half of the emoji; with either it is a list of code
//! points. [`Reader::new`] makes that list once, beside the byte offset each
//! character came from, so an error can point into the source.
//!
//! # It recurses, and the depth is bounded
//!
//! A group, a lookaround and a `v` class inside a class each cost a frame, and
//! [`bounds::DEEPEST_PATTERN`] is how many. Everything else — alternatives,
//! terms, a class's members — is read in a loop.
//!
//! # What is checked after the whole pattern is read
//!
//! A backreference may name a group that comes later — `\1(a)` is a pattern —
//! so whether `\5` names a group, and whether `\k<x>` names a name, is decided
//! once the last group is counted. Two groups of one name are refused while
//! reading: within one alternative the names of every term are kept apart,
//! and across alternatives they may repeat, which is the specification's
//! *might both participate* made exact.

mod class;
mod escape;

use std::collections::HashSet;

use crate::bounds;
use crate::unicode;

use super::flags::{Flag, Flags};
use super::tree::{Assertion, Modifiers, Node, Pattern, Reference};
use super::wrong::{Legacy, PatternError, Wrong};

/// Parse `body` under `flags`.
///
/// # Errors
///
/// [`PatternError`]: what is wrong, and the byte offset into `body`.
pub fn parse(body: &str, flags: &Flags) -> Result<Pattern, PatternError> {
    let mut reader = Reader::new(body, flags);
    let (node, _) = reader.disjunction()?;
    if reader.peek().is_some() {
        // The only thing a disjunction stops at, other than the end, is a `)`.
        return Err(reader.wrong(Wrong::UnopenedGroup, reader.at));
    }
    reader.resolve()?;
    Ok(Pattern {
        body: node,
        groups: reader.groups,
        names: reader.names,
    })
}

/// The names the groups in a piece of pattern define.
///
/// A set rather than a list: a pattern may have a hundred thousand groups,
/// and a check against a list for each would be quadratic in a stranger's
/// text.
type Names = HashSet<Vec<u16>>;

/// The parser's state.
pub(super) struct Reader {
    /// The pattern's characters, in the mode's reading.
    chars: Vec<u32>,
    /// The byte offset each character starts at.
    offsets: Vec<usize>,
    /// The byte length of the text, which is where its end is.
    length: usize,
    /// The character being looked at.
    at: usize,
    /// `u` or `v`.
    unicode: bool,
    /// `v`.
    sets: bool,
    /// Whether the pattern has a named group anywhere, which is what makes
    /// `\k` a backreference rather than Annex B's identity escape.
    named: bool,
    /// The groups counted so far.
    groups: u32,
    /// Each group's name, by its number less one.
    names: Vec<Option<Vec<u16>>>,
    /// Every backreference, and where it was, to check once all groups are
    /// counted.
    references: Vec<(Reference, usize)>,
    /// How deep the reading is.
    depth: usize,
}

impl Reader {
    /// Read `body` into the characters the mode sees.
    fn new(body: &str, flags: &Flags) -> Self {
        let unicode = flags.code_points();
        let mut chars = Vec::with_capacity(body.len());
        let mut offsets = Vec::with_capacity(body.len());
        for (offset, c) in body.char_indices() {
            if unicode {
                chars.push(u32::from(c));
                offsets.push(offset);
            } else {
                let mut units = [0_u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    chars.push(u32::from(*unit));
                    offsets.push(offset);
                }
            }
        }
        let mut reader = Self {
            chars,
            offsets,
            length: body.len(),
            at: 0,
            unicode,
            sets: flags.has(Flag::UnicodeSets),
            named: false,
            groups: 0,
            names: Vec::new(),
            references: Vec::new(),
            depth: 0,
        };
        reader.named = reader.has_a_named_group();
        reader
    }

    /// Whether `(?<name>` appears outside a class and an escape.
    ///
    /// Annex B reads `\k` as a `k` only in a pattern with no named group at
    /// all — before or after it — so this is asked of the whole text first.
    fn has_a_named_group(&self) -> bool {
        let mut at = 0;
        let mut in_a_class = false;
        while let Some(c) = self.char_at(at) {
            if c == u32::from('\\') {
                at = at.saturating_add(2);
                continue;
            }
            if c == u32::from('[') {
                in_a_class = true;
            } else if c == u32::from(']') {
                in_a_class = false;
            } else if c == u32::from('(')
                && !in_a_class
                && self.char_at(at.saturating_add(1)) == Some(u32::from('?'))
                && self.char_at(at.saturating_add(2)) == Some(u32::from('<'))
                && !matches!(
                    self.char_at(at.saturating_add(3)).and_then(char::from_u32),
                    Some('=' | '!')
                )
            {
                return true;
            }
            at = at.saturating_add(1);
        }
        false
    }

    // --- Looking at characters ----------------------------------------------

    /// The character at an index.
    fn char_at(&self, at: usize) -> Option<u32> {
        self.chars.get(at).copied()
    }

    /// The character being looked at.
    fn peek(&self) -> Option<u32> {
        self.char_at(self.at)
    }

    /// The one after it.
    fn peek_second(&self) -> Option<u32> {
        self.char_at(self.at.saturating_add(1))
    }

    /// Whether the character being looked at is `c`.
    fn sees(&self, c: char) -> bool {
        self.peek() == Some(u32::from(c))
    }

    /// Take the character being looked at.
    fn next(&mut self) -> Option<u32> {
        let c = self.peek()?;
        self.at = self.at.saturating_add(1);
        Some(c)
    }

    /// Take `c` if it is the character being looked at.
    fn eat(&mut self, c: char) -> bool {
        if self.sees(c) {
            self.at = self.at.saturating_add(1);
            return true;
        }
        false
    }

    /// An error at the character `at`.
    fn wrong(&self, wrong: Wrong, at: usize) -> PatternError {
        PatternError {
            wrong,
            at: self.offsets.get(at).copied().unwrap_or(self.length),
        }
    }

    /// Annex B's form under neither `u` nor `v`, and plainly wrong otherwise.
    fn legacy_or(&self, legacy: Legacy, otherwise: Wrong, at: usize) -> PatternError {
        if self.unicode {
            self.wrong(otherwise, at)
        } else {
            self.wrong(Wrong::Legacy(legacy), at)
        }
    }

    /// Go one level deeper, refusing past the bound.
    fn enter(&mut self, at: usize) -> Result<(), PatternError> {
        if self.depth >= bounds::DEEPEST_PATTERN {
            return Err(self.wrong(Wrong::TooDeep, at));
        }
        self.depth = self.depth.saturating_add(1);
        Ok(())
    }

    /// Come back out.
    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    // --- The grammar ----------------------------------------------------------

    /// `Disjunction`: alternatives separated by `|`, up to a `)` or the end.
    fn disjunction(&mut self) -> Result<(Node, Names), PatternError> {
        let mut alternatives = Vec::new();
        let mut names = Names::new();
        loop {
            let (alternative, defined) = self.alternative()?;
            alternatives.push(alternative);
            names.extend(defined);
            if !self.eat('|') {
                break;
            }
        }
        let node = if alternatives.len() == 1 {
            alternatives.pop().unwrap_or(Node::Sequence(Vec::new()))
        } else {
            Node::Alternatives(alternatives)
        };
        Ok((node, names))
    }

    /// `Alternative`: terms, up to a `|`, a `)` or the end.
    fn alternative(&mut self) -> Result<(Node, Names), PatternError> {
        let mut terms = Vec::new();
        let mut names = Names::new();
        while let Some(c) = self.peek() {
            if c == u32::from('|') || c == u32::from(')') {
                break;
            }
            let start = self.at;
            let (term, defined) = self.term()?;
            for name in defined {
                if !names.insert(name) {
                    return Err(self.wrong(Wrong::RepeatedName, start));
                }
            }
            terms.push(term);
        }
        Ok((Node::Sequence(terms), names))
    }

    /// `Term`: an assertion, or an atom and perhaps how often it repeats.
    fn term(&mut self) -> Result<(Node, Names), PatternError> {
        let start = self.at;
        let before = self.groups;
        let (atom, names) = match self.peek().and_then(char::from_u32) {
            Some('^') => {
                self.at = self.at.saturating_add(1);
                (Node::Assertion(Assertion::Start), Names::new())
            }
            Some('$') => {
                self.at = self.at.saturating_add(1);
                (Node::Assertion(Assertion::End), Names::new())
            }
            Some('(') => self.group()?,
            Some('[') => (Node::Class(self.class()?), Names::new()),
            Some('\\') => {
                self.at = self.at.saturating_add(1);
                (self.atom_escape(start)?, Names::new())
            }
            Some('.') => {
                self.at = self.at.saturating_add(1);
                (Node::Any, Names::new())
            }
            Some('*' | '+' | '?') => return Err(self.wrong(Wrong::NothingToRepeat, start)),
            Some('{') => {
                if self.braced().is_some() {
                    return Err(self.wrong(Wrong::NothingToRepeat, start));
                }
                return Err(self.legacy_or(Legacy::LoneBracket, Wrong::LoneBracket, start));
            }
            Some('}' | ']') => {
                return Err(self.legacy_or(Legacy::LoneBracket, Wrong::LoneBracket, start));
            }
            _ => {
                let c = self
                    .next()
                    .ok_or_else(|| self.wrong(Wrong::NothingToRepeat, start))?;
                (Node::Char(c), Names::new())
            }
        };

        let quantifier_at = self.at;
        let Some((min, max, greedy)) = self.quantifier()? else {
            return Ok((atom, names));
        };
        match &atom {
            Node::Assertion(_) => return Err(self.wrong(Wrong::NothingToRepeat, quantifier_at)),
            Node::Look { behind: true, .. } => {
                return Err(self.wrong(Wrong::RepeatedLookbehind, quantifier_at));
            }
            Node::Look { behind: false, .. } => {
                return Err(self.legacy_or(
                    Legacy::RepeatedLookahead,
                    Wrong::NothingToRepeat,
                    quantifier_at,
                ));
            }
            _ => {}
        }
        Ok((
            Node::Repeat {
                body: Box::new(atom),
                min,
                max,
                greedy,
                before,
                inside: self.groups.saturating_sub(before),
            },
            names,
        ))
    }

    /// `Quantifier`, if one is here: its least, its most and whether it is
    /// greedy.
    fn quantifier(&mut self) -> Result<Option<(u32, u32, bool)>, PatternError> {
        let start = self.at;
        let (min, max) = match self.peek().and_then(char::from_u32) {
            Some('*') => {
                self.at = self.at.saturating_add(1);
                (0, u32::MAX)
            }
            Some('+') => {
                self.at = self.at.saturating_add(1);
                (1, u32::MAX)
            }
            Some('?') => {
                self.at = self.at.saturating_add(1);
                (0, 1)
            }
            Some('{') => {
                let Some(braced) = self.braced() else {
                    return Err(self.legacy_or(Legacy::LoneBracket, Wrong::LoneBracket, start));
                };
                if braced.out_of_order {
                    return Err(self.wrong(Wrong::RepeatOutOfOrder, start));
                }
                self.at = braced.end;
                (braced.min, braced.max)
            }
            _ => return Ok(None),
        };
        let greedy = !self.eat('?');
        Ok(Some((min, max, greedy)))
    }

    /// `{n}`, `{n,}` or `{n,m}` at the character being looked at, without
    /// taking it.
    fn braced(&self) -> Option<Braced> {
        let mut at = self.at.saturating_add(1);
        let (min_digits, after) = self.digits(at);
        if min_digits.is_empty() {
            return None;
        }
        at = after;
        let (max_digits, comma) = if self.char_at(at) == Some(u32::from(',')) {
            let (digits, after) = self.digits(at.saturating_add(1));
            at = after;
            (Some(digits), true)
        } else {
            (None, false)
        };
        if self.char_at(at) != Some(u32::from('}')) {
            return None;
        }
        let min = value_of(&min_digits);
        let (max, out_of_order) = match (comma, max_digits) {
            (false, _) => (min, false),
            (true, Some(digits)) if !digits.is_empty() => {
                (value_of(&digits), more(&min_digits, &digits))
            }
            (true, _) => (u32::MAX, false),
        };
        Some(Braced {
            min,
            max,
            out_of_order,
            end: at.saturating_add(1),
        })
    }

    /// The decimal digits from `at`, and the index past them.
    fn digits(&self, mut at: usize) -> (Vec<u32>, usize) {
        let mut digits = Vec::new();
        while let Some(c) = self.char_at(at) {
            if !(u32::from('0')..=u32::from('9')).contains(&c) {
                break;
            }
            digits.push(c - u32::from('0'));
            at = at.saturating_add(1);
        }
        (digits, at)
    }

    /// A group of any kind, at its `(`.
    fn group(&mut self) -> Result<(Node, Names), PatternError> {
        let start = self.at;
        self.at = self.at.saturating_add(1);
        self.enter(start)?;
        let outcome = self.group_inside(start);
        self.leave();
        outcome
    }

    /// [`Reader::group`], one level deeper.
    fn group_inside(&mut self, start: usize) -> Result<(Node, Names), PatternError> {
        if !self.eat('?') {
            return self.capture(start, None);
        }
        match self.peek().and_then(char::from_u32) {
            Some('=') => self.look(start, false, false, 1),
            Some('!') => self.look(start, false, true, 1),
            Some('<') => match self.peek_second().and_then(char::from_u32) {
                Some('=') => self.look(start, true, false, 2),
                Some('!') => self.look(start, true, true, 2),
                _ => {
                    self.at = self.at.saturating_add(1);
                    let name = self.group_name()?;
                    self.capture(start, Some(name))
                }
            },
            Some(':') => {
                self.at = self.at.saturating_add(1);
                let (body, names) = self.closed(start)?;
                Ok((
                    Node::Group {
                        capture: None,
                        body: Box::new(body),
                    },
                    names,
                ))
            }
            Some('i' | 'm' | 's' | '-') => self.modified(start),
            _ => Err(self.wrong(Wrong::BadGroup, start)),
        }
    }

    /// A disjunction and the `)` that closes the group opened at `start`.
    fn closed(&mut self, start: usize) -> Result<(Node, Names), PatternError> {
        let inside = self.disjunction()?;
        if !self.eat(')') {
            return Err(self.wrong(Wrong::UnclosedGroup, start));
        }
        Ok(inside)
    }

    /// A capturing group, numbered by its `(`.
    fn capture(
        &mut self,
        start: usize,
        name: Option<Vec<u16>>,
    ) -> Result<(Node, Names), PatternError> {
        self.groups = self.groups.saturating_add(1);
        let number = self.groups;
        self.names.push(name.clone());
        let (body, mut names) = self.closed(start)?;
        if let Some(name) = name {
            if !names.insert(name) {
                return Err(self.wrong(Wrong::RepeatedName, start));
            }
        }
        Ok((
            Node::Group {
                capture: Some(number),
                body: Box::new(body),
            },
            names,
        ))
    }

    /// A lookaround, with `skip` characters of its opening left to take.
    fn look(
        &mut self,
        start: usize,
        behind: bool,
        negated: bool,
        skip: usize,
    ) -> Result<(Node, Names), PatternError> {
        self.at = self.at.saturating_add(skip);
        let (body, names) = self.closed(start)?;
        Ok((
            Node::Look {
                behind,
                negated,
                body: Box::new(body),
            },
            names,
        ))
    }

    /// `(?ims-ims:…)`.
    fn modified(&mut self, start: usize) -> Result<(Node, Names), PatternError> {
        let add = self.modifiers(start, Modifiers::default())?;
        let remove = if self.eat('-') {
            let remove = self.modifiers(start, add)?;
            if add.is_empty() && remove.is_empty() {
                return Err(self.wrong(Wrong::BadModifiers, start));
            }
            remove
        } else {
            Modifiers::default()
        };
        if !self.eat(':') {
            return Err(self.wrong(Wrong::BadModifiers, start));
        }
        let (body, names) = self.closed(start)?;
        Ok((
            Node::Modified {
                add,
                remove,
                body: Box::new(body),
            },
            names,
        ))
    }

    /// The letters of one side of a modified group's flags, none of which may
    /// already be in `taken`.
    fn modifiers(&mut self, start: usize, taken: Modifiers) -> Result<Modifiers, PatternError> {
        let mut these = Modifiers::default();
        loop {
            let (mine, theirs) = match self.peek().and_then(char::from_u32) {
                Some('i') => (&mut these.ignore_case, taken.ignore_case),
                Some('m') => (&mut these.multiline, taken.multiline),
                Some('s') => (&mut these.dot_all, taken.dot_all),
                _ => return Ok(these),
            };
            if *mine || theirs {
                return Err(self.wrong(Wrong::BadModifiers, start));
            }
            *mine = true;
            self.at = self.at.saturating_add(1);
        }
    }

    /// Check every backreference against the groups the pattern has.
    fn resolve(&self) -> Result<(), PatternError> {
        let named: HashSet<&Vec<u16>> = self.names.iter().flatten().collect();
        for (reference, at) in &self.references {
            match reference {
                Reference::Number(number) => {
                    if *number > self.groups {
                        return Err(self.legacy_or(Legacy::OctalEscape, Wrong::NoSuchGroup, *at));
                    }
                }
                Reference::Name(name) => {
                    if !named.contains(name) {
                        return Err(self.wrong(Wrong::NoSuchName, *at));
                    }
                }
            }
        }
        Ok(())
    }
}

/// A braced quantifier, read.
struct Braced {
    min: u32,
    max: u32,
    /// Whether its least is more than its most, compared exactly.
    out_of_order: bool,
    /// The index past its `}`.
    end: usize,
}

/// The value of some decimal digits, as far as a `u32` goes: no string is
/// long enough for a repeat past four thousand million to differ from one
/// without limit.
fn value_of(digits: &[u32]) -> u32 {
    digits.iter().fold(0_u32, |value, digit| {
        value.saturating_mul(10).saturating_add(*digit)
    })
}

/// Whether the number `a` spells is more than the one `b` does, however many
/// digits either has — `{99999999999,99999999998}` is out of order, and two
/// saturated values would have said it was not.
fn more(a: &[u32], b: &[u32]) -> bool {
    let a = significant(a);
    let b = significant(b);
    a.len() > b.len() || (a.len() == b.len() && a > b)
}

/// Digits without their leading zeros.
fn significant(digits: &[u32]) -> &[u32] {
    let first = digits
        .iter()
        .position(|digit| *digit != 0)
        .unwrap_or(digits.len());
    digits.get(first..).unwrap_or(&[])
}

/// Whether a character is `UnicodeIDContinue`, which is what Annex B's
/// identity escapes are not allowed to be in the main grammar.
fn is_id_continue(c: u32) -> bool {
    char::from_u32(c).is_some_and(unicode::is_id_continue)
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::regexp::flags::Flags;
    use crate::regexp::tree::{Assertion, Node, Reference};
    use crate::regexp::wrong::{Legacy, Wrong};

    fn read(body: &str, flags: &str) -> Result<super::Pattern, Wrong> {
        let Some(flags) = Flags::of(flags) else {
            panic!("{flags} are flags");
        };
        parse(body, &flags).map_err(|error| error.wrong)
    }

    #[test]
    fn alternatives_terms_and_repeats_make_the_tree_they_should() {
        let Ok(pattern) = read("a|b*?c{2,}", "") else {
            panic!("a pattern");
        };
        let Node::Alternatives(alternatives) = pattern.body else {
            panic!("two alternatives");
        };
        assert_eq!(alternatives.len(), 2);
        let Some(Node::Sequence(terms)) = alternatives.get(1) else {
            panic!("the second is a sequence");
        };
        assert!(matches!(
            terms.first(),
            Some(Node::Repeat {
                min: 0,
                max: u32::MAX,
                greedy: false,
                ..
            })
        ));
        assert!(matches!(
            terms.get(1),
            Some(Node::Repeat {
                min: 2,
                max: u32::MAX,
                greedy: true,
                ..
            })
        ));
    }

    #[test]
    fn groups_are_numbered_by_their_opening_and_a_name_is_kept() {
        let Ok(pattern) = read("((a)(?<x>b))(?:c)\\2\\k<x>", "") else {
            panic!("a pattern");
        };
        assert_eq!(pattern.groups, 3);
        assert_eq!(
            pattern.names,
            vec![None, None, Some("x".encode_utf16().collect())]
        );
        let Node::Sequence(terms) = pattern.body else {
            panic!("a sequence");
        };
        assert!(matches!(
            terms.get(2),
            Some(Node::Backreference(Reference::Number(2)))
        ));
    }

    #[test]
    fn a_name_may_repeat_across_alternatives_and_not_within_one() {
        assert!(read("(?<a>x)|(?<a>y)", "").is_ok());
        assert!(read("(?:(?<a>x)|(?<a>y))\\k<a>", "").is_ok());
        assert_eq!(read("(?<a>x)(?<a>y)", "").err(), Some(Wrong::RepeatedName));
        assert_eq!(read("(?<a>(?<a>x))", "").err(), Some(Wrong::RepeatedName));
        assert_eq!(
            read("(?:(?<a>x)|y)(?<a>z)", "").err(),
            Some(Wrong::RepeatedName)
        );
    }

    #[test]
    fn what_cannot_repeat_is_refused() {
        assert_eq!(read("*a", "").err(), Some(Wrong::NothingToRepeat));
        assert_eq!(read("a**", "").err(), Some(Wrong::NothingToRepeat));
        assert_eq!(read("^*", "").err(), Some(Wrong::NothingToRepeat));
        assert_eq!(read("\\b+", "").err(), Some(Wrong::NothingToRepeat));
        assert_eq!(read("(?<=a)*", "").err(), Some(Wrong::RepeatedLookbehind));
        assert_eq!(read("(?=a)*", "u").err(), Some(Wrong::NothingToRepeat));
        assert_eq!(read("a{3,1}", "").err(), Some(Wrong::RepeatOutOfOrder));
        assert_eq!(
            read("a{99999999999,99999999998}", "").err(),
            Some(Wrong::RepeatOutOfOrder)
        );
        assert!(read("a{99999999999}", "").is_ok());
    }

    #[test]
    fn annex_b_forms_are_refused_by_name_and_plainly_under_u() {
        for (body, legacy) in [
            ("]", Legacy::LoneBracket),
            ("a{", Legacy::LoneBracket),
            ("}", Legacy::LoneBracket),
            ("\\c1", Legacy::ControlEscape),
            ("\\01", Legacy::OctalEscape),
            ("\\1", Legacy::OctalEscape),
            ("(a)\\2", Legacy::OctalEscape),
            ("\\a", Legacy::IdentityEscape),
            ("\\k", Legacy::IdentityEscape),
            ("\\p{L}", Legacy::IdentityEscape),
            ("\\u{41}", Legacy::IdentityEscape),
            ("(?=a)+", Legacy::RepeatedLookahead),
            ("[\\d-z]", Legacy::ClassEscapeInARange),
            ("[\\1]", Legacy::OctalEscape),
        ] {
            assert_eq!(read(body, "").err(), Some(Wrong::Legacy(legacy)), "{body}");
            let under_u = read(body, "u");
            assert!(
                !matches!(under_u, Err(Wrong::Legacy(_))),
                "{body} under u is {under_u:?}"
            );
        }
    }

    #[test]
    fn the_modern_grammar_is_read_whole() {
        for (body, flags) in [
            ("(?<=\\$)\\d+(?<!x)", ""),
            ("(?i:a)(?-s:.)(?m-s:^)", ""),
            ("\\u{1F600}\\uD83D\\uDE00\\p{Script=Greek}\\P{Lu}", "u"),
            ("[\\p{L}--[a-z]]", "v"),
            ("[[a-z]&&[aeiou]]", "v"),
            ("[\\q{abc|d}x-z]", "v"),
            ("\\k<a>(?<a>.)", ""),
            ("[\\-]", "u"),
            ("[^]", ""),
            ("", ""),
            ("a|", ""),
        ] {
            assert!(read(body, flags).is_ok(), "{body} /{flags}");
        }
    }

    #[test]
    fn malformed_patterns_say_what_is_wrong() {
        for (body, flags, wrong) in [
            ("(", "", Wrong::UnclosedGroup),
            (")", "", Wrong::UnopenedGroup),
            ("[a", "", Wrong::UnclosedClass),
            ("[z-a]", "", Wrong::RangeOutOfOrder),
            ("[\\d-z]", "u", Wrong::ClassEscapeInARange),
            ("\\2(a)", "u", Wrong::NoSuchGroup),
            ("\\k<b>(?<a>.)", "", Wrong::NoSuchName),
            ("(?<1a>.)", "", Wrong::BadGroupName),
            ("(?x)", "", Wrong::BadGroup),
            ("(?ii:a)", "", Wrong::BadModifiers),
            ("(?i-i:a)", "", Wrong::BadModifiers),
            ("(?-:a)", "", Wrong::BadModifiers),
            ("\\u{110000}", "u", Wrong::CodePointOutOfRange),
            ("\\a", "u", Wrong::BadEscape),
            ("\\-", "u", Wrong::BadEscape),
            ("\\p", "u", Wrong::BadProperty),
            ("[a&&&b]", "v", Wrong::BadSetOperation),
            ("[a&&b--c]", "v", Wrong::BadSetOperation),
            ("[ab--c]", "v", Wrong::BadSetOperation),
            ("[a-z&&b]", "v", Wrong::BadSetOperation),
            ("[a!!b]", "v", Wrong::ReservedPunctuator),
            ("[(]", "v", Wrong::UnescapedInASet),
            ("[^\\q{ab}]", "v", Wrong::NegatedStrings),
            ("\\P{RGI_Emoji}", "v", Wrong::NegatedStrings),
            ("\\", "", Wrong::EndsInABackslash),
        ] {
            assert_eq!(read(body, flags).err(), Some(wrong), "{body} /{flags}");
        }
    }

    #[test]
    fn a_pattern_nested_past_the_bound_is_refused_and_one_inside_it_is_not() {
        let deep = crate::bounds::DEEPEST_PATTERN;
        let inside = format!("{}{}", "(".repeat(deep), ")".repeat(deep));
        assert!(read(&inside, "").is_ok());
        let past = format!("{}{}", "(".repeat(deep + 1), ")".repeat(deep + 1));
        assert_eq!(read(&past, "").err(), Some(Wrong::TooDeep));
        let sets = format!("{}{}", "[".repeat(deep + 1), "]".repeat(deep + 1));
        assert_eq!(read(&sets, "v").err(), Some(Wrong::TooDeep));
    }

    #[test]
    fn without_u_a_pattern_is_code_units_and_with_it_code_points() {
        let Ok(units) = read("😀", "") else {
            panic!("a pattern");
        };
        assert_eq!(
            units.body,
            Node::Sequence(vec![Node::Char(0xD83D), Node::Char(0xDE00)])
        );
        let Ok(points) = read("😀", "u") else {
            panic!("a pattern");
        };
        assert_eq!(points.body, Node::Sequence(vec![Node::Char(0x1F600)]));
        let Ok(anchored) = read("^$", "") else {
            panic!("a pattern");
        };
        assert_eq!(
            anchored.body,
            Node::Sequence(vec![
                Node::Assertion(Assertion::Start),
                Node::Assertion(Assertion::End)
            ])
        );
    }

    #[test]
    fn an_error_points_at_the_byte_it_is_at() {
        let Some(flags) = Flags::of("") else {
            panic!("no flags are flags");
        };
        let Err(error) = parse("é(?x)", &flags) else {
            panic!("refused");
        };
        assert_eq!(error.at, 2, "after the two bytes of the é");
    }
}
