/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A parsed pattern into a [`Program`], or the piece that is not built yet.
//!
//! ADR 0029 § 6 is the list of what is not: `i` and `\p{…}` need the rented
//! Unicode tables (queue item 322), and `v`'s set operations and `d`'s
//! indices are item 324. Each is refused **by name** rather than compiled as
//! something near it — `/a/i` that matched only `a` would be a page taking a
//! branch on an answer the language never gives (ADR 0013 § 3).
//!
//! # How each piece compiles
//!
//! An alternative is a [`Inst::Split`] to it and a jump past the rest. A group
//! saves where it began and ended — in that order forwards and the other way
//! round inside a lookbehind, where the end is reached first. A repeat of one
//! character is [`Inst::Many`]; any other repeat is a counter, a test, a mark
//! for the empty check and a loop, with its groups forgotten at the start of
//! each iteration, which is the specification's `RepeatMatcher` step for step.

use std::collections::HashMap;

use super::flags::{Flag, Flags};
use super::program::{Inst, Program, Single};
use super::set::{Named, Set};
use super::tree::{Assertion, Class, ClassBody, ClassEscape, ClassItem, Node, Pattern, Reference};

/// What the language has in a pattern and this engine has not built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unbuilt {
    /// `i`, or a group that turns it on.
    IgnoreCase,
    /// `\p{…}` or `\P{…}`.
    Property,
    /// `v`.
    UnicodeSets,
    /// `d`.
    Indices,
}

impl Unbuilt {
    /// The queue item that builds it.
    pub const fn item(self) -> u16 {
        match self {
            Unbuilt::IgnoreCase | Unbuilt::Property => 322,
            Unbuilt::UnicodeSets | Unbuilt::Indices => 324,
        }
    }

    /// What it is, in a person's words.
    pub const fn describe(self) -> &'static str {
        match self {
            Unbuilt::IgnoreCase => "a regular expression that ignores case (`i`)",
            Unbuilt::Property => "a Unicode property escape (`\\p{…}`)",
            Unbuilt::UnicodeSets => "a regular expression with set operations (`v`)",
            Unbuilt::Indices => "a regular expression that reports indices (`d`)",
        }
    }
}

/// Compile a pattern that parsed under `flags`.
///
/// # Errors
///
/// [`Unbuilt`], naming the first piece of the pattern this engine cannot
/// compile yet.
pub fn emit(pattern: Pattern, flags: Flags, source: &str) -> Result<Program, Unbuilt> {
    if flags.has(Flag::IgnoreCase) {
        return Err(Unbuilt::IgnoreCase);
    }
    if flags.has(Flag::UnicodeSets) {
        return Err(Unbuilt::UnicodeSets);
    }
    if flags.has(Flag::HasIndices) {
        return Err(Unbuilt::Indices);
    }
    let mut named: HashMap<Vec<u16>, Vec<u32>> = HashMap::new();
    for (number, name) in (1_u32..).zip(&pattern.names) {
        if let Some(name) = name {
            named.entry(name.clone()).or_default().push(number);
        }
    }
    let mut emitter = Emitter {
        code: Vec::new(),
        sets: Vec::new(),
        references: Vec::new(),
        counters: 0,
        code_points: flags.code_points(),
        multiline: flags.has(Flag::Multiline),
        dot_all: flags.has(Flag::DotAll),
        named,
    };
    emitter.node(&pattern.body, false)?;
    emitter.code.push(Inst::Match);
    Ok(Program::new(
        emitter.code,
        emitter.sets,
        emitter.references,
        emitter.counters,
        (pattern.groups, pattern.names),
        flags,
        source,
    ))
}

/// The compiler's state.
struct Emitter {
    code: Vec<Inst>,
    sets: Vec<Set>,
    references: Vec<Vec<u32>>,
    counters: u32,
    /// `u` or `v`, which decides the most a set's complement reaches.
    code_points: bool,
    /// `m` where the instruction being compiled is, after any modifiers.
    multiline: bool,
    /// `s`, the same way.
    dot_all: bool,
    /// The groups each name names, in order.
    named: HashMap<Vec<u16>, Vec<u32>>,
}

impl Emitter {
    /// Where the next instruction goes.
    ///
    /// A pattern is at most a script's length and every character of it is a
    /// handful of instructions, so this is far inside a `u32`; saturating is
    /// the answer to a question that cannot arise rather than a wrong jump.
    fn here(&self) -> u32 {
        u32::try_from(self.code.len()).unwrap_or(u32::MAX)
    }

    /// Point the jump or exit of the instruction at `at` to here.
    fn patch(&mut self, at: u32) {
        let here = self.here();
        let Some(inst) = usize::try_from(at)
            .ok()
            .and_then(|at| self.code.get_mut(at))
        else {
            return;
        };
        match inst {
            Inst::Split { second, .. } => *second = here,
            Inst::Jump(to) => *to = here,
            Inst::RepeatTest { exit, .. } | Inst::LookEnter { exit, .. } => *exit = here,
            _ => {}
        }
    }

    /// Compile one node, reading backwards when `back` is set.
    fn node(&mut self, node: &Node, back: bool) -> Result<(), Unbuilt> {
        match node {
            Node::Alternatives(alternatives) => self.alternatives(alternatives, back)?,
            Node::Sequence(terms) => {
                if back {
                    for term in terms.iter().rev() {
                        self.node(term, back)?;
                    }
                } else {
                    for term in terms {
                        self.node(term, back)?;
                    }
                }
            }
            Node::Char(c) => self.code.push(Inst::Read {
                single: Single::Char(*c),
                back,
            }),
            Node::Any => self.code.push(Inst::Read {
                single: Single::Any {
                    line_endings: self.dot_all,
                },
                back,
            }),
            Node::Class(class) => {
                let set = self.set(class)?;
                self.code.push(Inst::Read {
                    single: Single::Set(set),
                    back,
                });
            }
            Node::Assertion(assertion) => self.code.push(match assertion {
                Assertion::Start => Inst::Start {
                    multiline: self.multiline,
                },
                Assertion::End => Inst::End {
                    multiline: self.multiline,
                },
                Assertion::Boundary => Inst::Boundary { negated: false },
                Assertion::NotBoundary => Inst::Boundary { negated: true },
            }),
            Node::Look {
                behind,
                negated,
                body,
            } => {
                let enter = self.here();
                self.code.push(Inst::LookEnter {
                    behind: *behind,
                    negated: *negated,
                    exit: 0,
                });
                self.node(body, *behind)?;
                self.code.push(Inst::LookLeave);
                self.patch(enter);
            }
            Node::Group { capture, body } => match capture {
                Some(number) => {
                    let start = number.saturating_mul(2);
                    let end = start.saturating_add(1);
                    let (first, last) = if back { (end, start) } else { (start, end) };
                    self.code.push(Inst::Save(first));
                    self.node(body, back)?;
                    self.code.push(Inst::Save(last));
                }
                None => self.node(body, back)?,
            },
            Node::Modified { add, remove, body } => {
                if add.ignore_case {
                    return Err(Unbuilt::IgnoreCase);
                }
                let (multiline, dot_all) = (self.multiline, self.dot_all);
                self.multiline = (multiline || add.multiline) && !remove.multiline;
                self.dot_all = (dot_all || add.dot_all) && !remove.dot_all;
                let outcome = self.node(body, back);
                self.multiline = multiline;
                self.dot_all = dot_all;
                outcome?;
            }
            Node::Repeat {
                body,
                min,
                max,
                greedy,
                before,
                inside,
            } => self.repeat(body, (*min, *max, *greedy), (*before, *inside), back)?,
            Node::Backreference(reference) => {
                let groups = match reference {
                    Reference::Number(number) => vec![*number],
                    Reference::Name(name) => self.named.get(name).cloned().unwrap_or_default(),
                };
                let list = u32::try_from(self.references.len()).unwrap_or(u32::MAX);
                self.references.push(groups);
                self.code.push(Inst::Backreference { list, back });
            }
            Node::Property(_) => return Err(Unbuilt::Property),
        }
        Ok(())
    }

    /// `a|b|c`: a split before each but the last, and a jump past the rest
    /// after each but the last.
    fn alternatives(&mut self, alternatives: &[Node], back: bool) -> Result<(), Unbuilt> {
        let mut ends = Vec::new();
        let count = alternatives.len();
        for (which, alternative) in alternatives.iter().enumerate() {
            if which.saturating_add(1) < count {
                let split = self.here();
                self.code.push(Inst::Split {
                    first: split.saturating_add(1),
                    second: 0,
                });
                self.node(alternative, back)?;
                ends.push(self.here());
                self.code.push(Inst::Jump(0));
                self.patch(split);
            } else {
                self.node(alternative, back)?;
            }
        }
        for end in ends {
            self.patch(end);
        }
        Ok(())
    }

    /// A repeat: [`Inst::Many`] for one character, the counted loop for
    /// anything else.
    fn repeat(
        &mut self,
        body: &Node,
        (min, max, greedy): (u32, u32, bool),
        (before, inside): (u32, u32),
        back: bool,
    ) -> Result<(), Unbuilt> {
        if max == 0 {
            // `RepeatMatcher` with a most of zero is its continuation, and
            // nothing inside it is ever tried or reset.
            return Ok(());
        }
        if let Some(single) = self.single(body)? {
            self.code.push(Inst::Many {
                single,
                back,
                min,
                max,
                greedy,
            });
            return Ok(());
        }
        let counter = self.counters;
        self.counters = self.counters.saturating_add(1);
        self.code.push(Inst::RepeatEnter { counter });
        let test = self.here();
        self.code.push(Inst::RepeatTest {
            counter,
            min,
            max,
            greedy,
            exit: 0,
        });
        if inside > 0 {
            let from = before.saturating_add(1).saturating_mul(2);
            let to = before
                .saturating_add(inside)
                .saturating_add(1)
                .saturating_mul(2);
            self.code.push(Inst::Clear { from, to });
        }
        self.code.push(Inst::RepeatMark { counter });
        self.node(body, back)?;
        self.code.push(Inst::RepeatLoop { counter, min, test });
        self.patch(test);
        Ok(())
    }

    /// The one character a node matches, if it is one character — looking
    /// through a non-capturing group of one term, so `(?:a)*` is as cheap as
    /// `a*`.
    fn single(&mut self, node: &Node) -> Result<Option<Single>, Unbuilt> {
        Ok(Some(match node {
            Node::Char(c) => Single::Char(*c),
            Node::Any => Single::Any {
                line_endings: self.dot_all,
            },
            Node::Class(class) => Single::Set(self.set(class)?),
            Node::Group {
                capture: None,
                body,
            } => return self.single(body),
            Node::Sequence(terms) if terms.len() == 1 => match terms.first() {
                Some(term) => return self.single(term),
                None => return Ok(None),
            },
            _ => return Ok(None),
        }))
    }

    /// A class, as a set, answering its index.
    fn set(&mut self, class: &Class) -> Result<u32, Unbuilt> {
        let ClassBody::Ranges(items) = &class.body else {
            return Err(Unbuilt::UnicodeSets);
        };
        let most = if self.code_points { 0x10_FFFF } else { 0xFFFF };
        let mut set = Set::new();
        for item in items {
            match item {
                ClassItem::Char(c) => set.add(*c),
                ClassItem::Range(from, to) => set.add_range(*from, *to),
                ClassItem::Escape(escape) => {
                    let (named, complement) = match escape {
                        ClassEscape::Digit => (Named::Digit, false),
                        ClassEscape::NotDigit => (Named::Digit, true),
                        ClassEscape::Space => (Named::Space, false),
                        ClassEscape::NotSpace => (Named::Space, true),
                        ClassEscape::Word => (Named::Word, false),
                        ClassEscape::NotWord => (Named::Word, true),
                    };
                    set.add_named(named, complement, most);
                }
                ClassItem::Property(_) => return Err(Unbuilt::Property),
            }
        }
        if class.negated {
            set.negate();
        }
        let index = u32::try_from(self.sets.len()).unwrap_or(u32::MAX);
        self.sets.push(set.finished());
        Ok(index)
    }
}

#[cfg(test)]
mod tests {
    use super::{Unbuilt, emit};
    use crate::regexp::flags::Flags;
    use crate::regexp::parse::parse;
    use crate::regexp::program::{Inst, Program, Single};

    fn compiled(body: &str, flags: &str) -> Result<Program, Unbuilt> {
        let Some(flags) = Flags::of(flags) else {
            panic!("flags");
        };
        let Ok(pattern) = parse(body, &flags) else {
            panic!("{body} parses");
        };
        emit(pattern, flags, body)
    }

    #[test]
    fn what_is_not_built_is_refused_naming_its_item() {
        for (body, flags, unbuilt) in [
            ("a", "i", Unbuilt::IgnoreCase),
            ("(?i:a)", "", Unbuilt::IgnoreCase),
            ("\\p{L}", "u", Unbuilt::Property),
            ("[\\P{L}]", "u", Unbuilt::Property),
            ("a", "v", Unbuilt::UnicodeSets),
            ("a", "d", Unbuilt::Indices),
        ] {
            assert_eq!(compiled(body, flags).err(), Some(unbuilt), "{body}/{flags}");
        }
        assert_eq!(Unbuilt::IgnoreCase.item(), 322);
        assert_eq!(Unbuilt::Indices.item(), 324);
        assert!(
            compiled("(?-i:a)", "").is_ok(),
            "turning off what is off is nothing"
        );
    }

    #[test]
    fn a_repeated_character_is_one_instruction_and_a_group_is_a_loop() {
        let Ok(star) = compiled("(?:a)*", "") else {
            panic!("compiles");
        };
        assert_eq!(
            star.code.first(),
            Some(&Inst::Many {
                single: Single::Char(u32::from('a')),
                back: false,
                min: 0,
                max: u32::MAX,
                greedy: true,
            })
        );
        let Ok(group) = compiled("(ab)+", "") else {
            panic!("compiles");
        };
        assert!(matches!(group.code.first(), Some(Inst::RepeatEnter { .. })));
        assert!(group.code.contains(&Inst::Clear { from: 2, to: 4 }));
        assert_eq!(group.counters, 1);
        assert_eq!(group.code.last(), Some(&Inst::Match));
    }

    #[test]
    fn a_lookbehind_reads_backwards_and_saves_its_end_first() {
        let Ok(program) = compiled("(?<=(ab))", "") else {
            panic!("compiles");
        };
        let code: Vec<Inst> = program.code.clone();
        assert!(matches!(
            code.first(),
            Some(Inst::LookEnter { behind: true, .. })
        ));
        assert_eq!(code.get(1), Some(&Inst::Save(3)), "the end, first");
        assert_eq!(
            code.get(2),
            Some(&Inst::Read {
                single: Single::Char(u32::from('b')),
                back: true
            }),
            "and the last character, first"
        );
        assert_eq!(code.get(4), Some(&Inst::Save(2)));
    }
}
