/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The backtracking machine, and the bound on what it may do (ADR 0029
//! §§ 2 and 3).
//!
//! # It never calls itself
//!
//! A choice is an entry on a stack this machine owns ([`Undo`]), and so is
//! every capture and counter it has changed since: going back is popping
//! entries until a choice comes off, undoing each change on the way. So a
//! pattern's nesting and an input's length choose how long that list is —
//! bounded by [`bounds::PLACES_IN_A_MATCH`] — and never how deep this
//! process's stack goes.
//!
//! # Every piece of work is counted
//!
//! A step is an instruction run or an entry popped, and a piece of work
//! longer than one — comparing a backreference, forgetting a repeat's
//! captures, copying them for a lookaround — counts as many steps as it is
//! long. One search may take [`bounds::STEPS_IN_A_MATCH`]. Running out is
//! [`Halt::Steps`], which the builtin turns into the `RangeError` ADR 0029 § 3
//! chose; it is never "no match", which would be an answer the language did
//! not give.
//!
//! # The embedder's stop is checked inside
//!
//! On every return to a choice, and every thousand-odd steps besides: the
//! budget bounds one search, and the stop is what bounds a script that runs
//! a bounded search for ever (ADR 0013 § 4).
//!
//! # A lookaround forgets its choices
//!
//! The specification's lookaround is atomic: once its body has matched, no
//! later failure comes back into it. Entering one writes a barrier and a copy
//! of the captures; leaving it cuts the stack back to the barrier, which is
//! every choice made inside, and writes one entry that puts the captures back
//! if the match fails further on. A negative lookaround inverts that: its body
//! matching is a failure, and the barrier coming off — every way through the
//! body tried — is a success.

use crate::bounds;

use super::flags::Flag;
use super::program::{Inst, Program, Single};
use super::surrogate::{combined, is_lead, is_trail};

/// No capture, in a slot.
const NONE: u32 = u32::MAX;

/// How often the stop is asked about between choices, in steps.
const ASK_EVERY: u64 = 1024;

/// Why a search did not finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Halt {
    /// It took [`bounds::STEPS_IN_A_MATCH`] steps.
    Steps,
    /// It held [`bounds::PLACES_IN_A_MATCH`] places to come back to.
    Places,
    /// The embedder asked for the script to stop, after this many steps.
    Stopped {
        /// How far it had got.
        steps: u64,
    },
    /// The program jumped outside itself, which is this engine's own bug.
    Broken,
}

/// A match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Where the match and each group began and ended, in code units: the
    /// whole match first, then each group by its number. A group that took no
    /// part is [`None`].
    pub captures: Vec<Option<(usize, usize)>>,
}

/// Search `input` from `last_index`, as `RegExpBuiltinExec` does: at that
/// index only if the pattern is sticky, and otherwise at each index from it
/// until one matches.
///
/// `stop` is asked whether the embedder wants the script stopped.
///
/// # Errors
///
/// [`Halt`], when the search did more than one may, or was stopped.
pub fn search(
    program: &Program,
    input: &[u16],
    last_index: usize,
    stop: &dyn Fn() -> bool,
) -> Result<Option<Found>, Halt> {
    let mut machine = Machine::new(program, input, stop);
    let sticky = program.flags().has(Flag::Sticky);
    let mut index = last_index;
    loop {
        if index > input.len() {
            return Ok(None);
        }
        // With `u` the input is code points, so an index inside a surrogate
        // pair names the pair: the match starts at its first half and is
        // still reported at `last_index`, which is the specification's.
        let start = if machine.code_points && splits_a_pair(input, index) {
            index.saturating_sub(1)
        } else {
            index
        };
        if let Some(end) = machine.attempt(start)? {
            return Ok(Some(machine.found(index, end)));
        }
        if sticky {
            return Ok(None);
        }
        index = machine.advance(index);
    }
}

/// Something to undo, or a choice to come back to.
#[derive(Debug, Clone, Copy)]
enum Undo {
    /// Come back to `pc` at `pos`.
    Branch { pc: u32, pos: u32 },
    /// A capture slot held `old`.
    Slot { slot: u32, old: u32 },
    /// A counter's register held `old`.
    Register { index: u32, old: u32 },
    /// A lookaround began at `pos`, with the captures copied at `snapshot`.
    Look {
        pos: u32,
        exit: u32,
        negated: bool,
        snapshot: u32,
    },
    /// A lookaround that matched set captures; put back the copy at
    /// `snapshot`.
    Restore { snapshot: u32 },
    /// A greedy [`Inst::Many`] at `at` read up to `pos`, and may give back
    /// characters until it is at `floor`.
    Retreat { at: u32, pos: u32, floor: u32 },
    /// A lazy [`Inst::Many`] at `at` has read `count` characters, to `pos`,
    /// and may read more.
    Advance { at: u32, pos: u32, count: u32 },
}

/// One search's state.
struct Machine<'a> {
    program: &'a Program,
    input: &'a [u16],
    code_points: bool,
    stop: &'a dyn Fn() -> bool,
    /// Capture slots: `2n` and `2n + 1` for group `n`.
    slots: Vec<u32>,
    /// Two per counter: the count, and where its iteration began.
    registers: Vec<u32>,
    undo: Vec<Undo>,
    /// Copies of the captures, for lookarounds.
    snapshots: Vec<u32>,
    /// The index in `undo` of each lookaround that has not ended.
    looks: Vec<usize>,
    steps: u64,
}

impl<'a> Machine<'a> {
    fn new(program: &'a Program, input: &'a [u16], stop: &'a dyn Fn() -> bool) -> Self {
        let groups = usize::try_from(program.groups()).unwrap_or(0);
        let counters = usize::try_from(program.counters).unwrap_or(0);
        Self {
            program,
            input,
            code_points: program.flags().code_points(),
            stop,
            slots: vec![NONE; groups.saturating_add(1).saturating_mul(2)],
            registers: vec![0; counters.saturating_mul(2)],
            undo: Vec::new(),
            snapshots: Vec::new(),
            looks: Vec::new(),
            steps: 0,
        }
    }

    /// The match, with the whole of it starting at `index`.
    fn found(&self, index: usize, end: usize) -> Found {
        let mut captures = vec![Some((index, end))];
        for pair in self.slots.chunks(2).skip(1) {
            captures.push(match pair {
                [start, end] if *start != NONE && *end != NONE => {
                    Some((as_index(*start), as_index(*end)))
                }
                _ => None,
            });
        }
        Found { captures }
    }

    /// `AdvanceStringIndex`.
    fn advance(&self, index: usize) -> usize {
        let next = index.saturating_add(1);
        if self.code_points && next < self.input.len() {
            if let Some((_, after)) = self.read(index, false) {
                return after;
            }
        }
        next
    }

    /// Count `count` steps of work.
    fn tick(&mut self, count: u64) -> Result<(), Halt> {
        let before = self.steps;
        self.steps = self.steps.saturating_add(count);
        if self.steps > bounds::STEPS_IN_A_MATCH {
            return Err(Halt::Steps);
        }
        if before / ASK_EVERY != self.steps / ASK_EVERY && (self.stop)() {
            return Err(Halt::Stopped { steps: self.steps });
        }
        Ok(())
    }

    /// Write something down to undo, if there is room.
    fn push(&mut self, undo: Undo) -> Result<(), Halt> {
        if self.undo.len().saturating_add(self.snapshots.len()) >= bounds::PLACES_IN_A_MATCH {
            return Err(Halt::Places);
        }
        self.undo.push(undo);
        Ok(())
    }

    /// Try to match at `start`, answering where the match ended.
    fn attempt(&mut self, start: usize) -> Result<Option<usize>, Halt> {
        self.slots.fill(NONE);
        self.registers.fill(0);
        self.undo.clear();
        self.snapshots.clear();
        self.looks.clear();
        let mut pc: usize = 0;
        let mut pos = start;
        loop {
            self.tick(1)?;
            let inst = *self.program.code.get(pc).ok_or(Halt::Broken)?;
            let next = match inst {
                Inst::Match => return Ok(Some(pos)),
                _ => self.run(inst, pc, pos)?,
            };
            match next {
                Some(going) => (pc, pos) = going,
                None => match self.backtrack()? {
                    Some(back) => (pc, pos) = back,
                    None => return Ok(None),
                },
            }
        }
    }

    /// Run one instruction, answering where to go next, or [`None`] to fail.
    fn run(&mut self, inst: Inst, pc: usize, pos: usize) -> Result<Option<(usize, usize)>, Halt> {
        let following = pc.saturating_add(1);
        Ok(match inst {
            Inst::Read { single, back } => self
                .read(pos, back)
                .filter(|(c, _)| self.is(single, *c))
                .map(|(_, after)| (following, after)),
            Inst::Many {
                single,
                back,
                min,
                max,
                greedy,
            } => self.many(pc, pos, (single, back), (min, max, greedy))?,
            Inst::Start { multiline } => {
                let at_start = pos == 0
                    || (multiline
                        && pos
                            .checked_sub(1)
                            .and_then(|before| self.input.get(before))
                            .is_some_and(|c| is_line_ending(u32::from(*c))));
                at_start.then_some((following, pos))
            }
            Inst::End { multiline } => {
                let at_end = pos == self.input.len()
                    || (multiline
                        && self
                            .input
                            .get(pos)
                            .is_some_and(|c| is_line_ending(u32::from(*c))));
                at_end.then_some((following, pos))
            }
            Inst::Boundary { negated } => {
                let before = pos
                    .checked_sub(1)
                    .is_some_and(|before| self.is_word(before));
                let boundary = before != self.is_word(pos);
                (boundary != negated).then_some((following, pos))
            }
            Inst::Split { first, second } => {
                self.push(Undo::Branch {
                    pc: second,
                    pos: as_position(pos)?,
                })?;
                Some((as_index(first), pos))
            }
            Inst::Jump(to) => Some((as_index(to), pos)),
            Inst::Save(slot) => {
                self.set_slot(slot, as_position(pos)?)?;
                Some((following, pos))
            }
            Inst::Clear { from, to } => {
                self.tick(u64::from(to.saturating_sub(from)))?;
                for slot in from..to {
                    self.set_slot(slot, NONE)?;
                }
                Some((following, pos))
            }
            Inst::Backreference { list, back } => self
                .backreference(list, back, pos)?
                .map(|after| (following, after)),
            Inst::RepeatEnter { counter } => {
                self.set_register(counter.saturating_mul(2), 0)?;
                Some((following, pos))
            }
            Inst::RepeatTest {
                counter,
                min,
                max,
                greedy,
                exit,
            } => self.repeat_test(counter, (min, max, greedy), exit, pc, pos)?,
            Inst::RepeatMark { counter } => {
                self.set_register(
                    counter.saturating_mul(2).saturating_add(1),
                    as_position(pos)?,
                )?;
                Some((following, pos))
            }
            Inst::RepeatLoop { counter, min, test } => {
                let count = self.register(counter.saturating_mul(2));
                let began = self.register(counter.saturating_mul(2).saturating_add(1));
                if count >= min && as_index(began) == pos {
                    None
                } else {
                    self.set_register(counter.saturating_mul(2), count.saturating_add(1))?;
                    Some((as_index(test), pos))
                }
            }
            Inst::LookEnter { negated, exit, .. } => {
                self.look_enter(negated, exit, pos)?;
                Some((following, pos))
            }
            Inst::LookLeave => self.look_leave()?,
            Inst::Match => Some((pc, pos)),
        })
    }

    /// [`Inst::Many`]: read the least, then as many more as greediness says,
    /// and write down how to change its mind.
    fn many(
        &mut self,
        pc: usize,
        pos: usize,
        (single, back): (Single, bool),
        (min, max, greedy): (u32, u32, bool),
    ) -> Result<Option<(usize, usize)>, Halt> {
        let following = pc.saturating_add(1);
        let mut here = pos;
        let mut count: u32 = 0;
        while count < min {
            let Some((c, after)) = self.read(here, back) else {
                return Ok(None);
            };
            if !self.is(single, c) {
                return Ok(None);
            }
            self.tick(1)?;
            here = after;
            count = count.saturating_add(1);
        }
        if !greedy {
            if count < max {
                self.push(Undo::Advance {
                    at: as_position(pc)?,
                    pos: as_position(here)?,
                    count,
                })?;
            }
            return Ok(Some((following, here)));
        }
        let floor = here;
        while count < max {
            let Some((c, after)) = self.read(here, back) else {
                break;
            };
            if !self.is(single, c) {
                break;
            }
            self.tick(1)?;
            here = after;
            count = count.saturating_add(1);
        }
        if here != floor {
            self.push(Undo::Retreat {
                at: as_position(pc)?,
                pos: as_position(here)?,
                floor: as_position(floor)?,
            })?;
        }
        Ok(Some((following, here)))
    }

    /// [`Inst::RepeatTest`].
    fn repeat_test(
        &mut self,
        counter: u32,
        (min, max, greedy): (u32, u32, bool),
        exit: u32,
        pc: usize,
        pos: usize,
    ) -> Result<Option<(usize, usize)>, Halt> {
        let body = pc.saturating_add(1);
        let count = self.register(counter.saturating_mul(2));
        if count < min {
            return Ok(Some((body, pos)));
        }
        if max != u32::MAX && count >= max {
            return Ok(Some((as_index(exit), pos)));
        }
        let at = as_position(pos)?;
        if greedy {
            self.push(Undo::Branch { pc: exit, pos: at })?;
            Ok(Some((body, pos)))
        } else {
            self.push(Undo::Branch {
                pc: as_position(body)?,
                pos: at,
            })?;
            Ok(Some((as_index(exit), pos)))
        }
    }

    /// [`Inst::LookEnter`]: copy the captures and write the barrier.
    fn look_enter(&mut self, negated: bool, exit: u32, pos: usize) -> Result<(), Halt> {
        let snapshot = as_position(self.snapshots.len())?;
        self.tick(self.slots.len() as u64)?;
        if self
            .undo
            .len()
            .saturating_add(self.snapshots.len())
            .saturating_add(self.slots.len())
            >= bounds::PLACES_IN_A_MATCH
        {
            return Err(Halt::Places);
        }
        self.snapshots.extend_from_slice(&self.slots);
        self.push(Undo::Look {
            pos: as_position(pos)?,
            exit,
            negated,
            snapshot,
        })?;
        self.looks.push(self.undo.len().saturating_sub(1));
        Ok(())
    }

    /// [`Inst::LookLeave`]: the body matched.
    fn look_leave(&mut self) -> Result<Option<(usize, usize)>, Halt> {
        let barrier = self.looks.pop().ok_or(Halt::Broken)?;
        let Some(Undo::Look {
            pos,
            exit,
            negated,
            snapshot,
        }) = self.undo.get(barrier).copied()
        else {
            return Err(Halt::Broken);
        };
        self.undo.truncate(barrier);
        let snapshot_at = as_index(snapshot);
        if negated {
            // Its body matching is its failure: the captures go back to what
            // they were before it, and so does everything else.
            self.restore(snapshot_at)?;
            return Ok(None);
        }
        // Every choice inside is forgotten. The captures it set stay, and
        // the copy it took is kept to put back if the match fails further on.
        self.snapshots
            .truncate(snapshot_at.saturating_add(self.slots.len()));
        self.push(Undo::Restore { snapshot })?;
        Ok(Some((as_index(exit), as_index(pos))))
    }

    /// Put the captures back from the copy at `snapshot`, and let go of it.
    fn restore(&mut self, snapshot: usize) -> Result<(), Halt> {
        self.tick(self.slots.len() as u64)?;
        let copy = self
            .snapshots
            .get(snapshot..snapshot.saturating_add(self.slots.len()))
            .ok_or(Halt::Broken)?;
        self.slots.copy_from_slice(copy);
        self.snapshots.truncate(snapshot);
        Ok(())
    }

    /// Go back to the most recent choice, undoing everything since.
    fn backtrack(&mut self) -> Result<Option<(usize, usize)>, Halt> {
        while let Some(entry) = self.undo.pop() {
            self.tick(1)?;
            match entry {
                Undo::Branch { pc, pos } => {
                    self.ask()?;
                    return Ok(Some((as_index(pc), as_index(pos))));
                }
                Undo::Slot { slot, old } => {
                    if let Some(held) = usize::try_from(slot)
                        .ok()
                        .and_then(|at| self.slots.get_mut(at))
                    {
                        *held = old;
                    }
                }
                Undo::Register { index, old } => {
                    if let Some(held) = usize::try_from(index)
                        .ok()
                        .and_then(|at| self.registers.get_mut(at))
                    {
                        *held = old;
                    }
                }
                Undo::Look {
                    pos,
                    exit,
                    negated,
                    snapshot,
                } => {
                    // Every way through its body failed.
                    self.looks.pop();
                    self.snapshots.truncate(as_index(snapshot));
                    if negated {
                        self.ask()?;
                        return Ok(Some((as_index(exit), as_index(pos))));
                    }
                }
                Undo::Restore { snapshot } => self.restore(as_index(snapshot))?,
                Undo::Retreat { at, pos, floor } => {
                    self.ask()?;
                    if let Some(going) = self.retreat(at, pos, floor)? {
                        return Ok(Some(going));
                    }
                }
                Undo::Advance { at, pos, count } => {
                    self.ask()?;
                    if let Some(going) = self.go_further(at, pos, count)? {
                        return Ok(Some(going));
                    }
                }
            }
        }
        Ok(None)
    }

    /// A greedy repeat gives back one character.
    fn retreat(&mut self, at: u32, pos: u32, floor: u32) -> Result<Option<(usize, usize)>, Halt> {
        let Some(Inst::Many { back, .. }) = self.program.code.get(as_index(at)).copied() else {
            return Err(Halt::Broken);
        };
        // The way it read, reversed: a character it took, put back.
        let Some((_, before)) = self.read(as_index(pos), !back) else {
            return Err(Halt::Broken);
        };
        let before = as_position(before)?;
        if before != floor {
            self.push(Undo::Retreat {
                at,
                pos: before,
                floor,
            })?;
        }
        Ok(Some((as_index(at).saturating_add(1), as_index(before))))
    }

    /// A lazy repeat takes one character more, if it can.
    fn go_further(
        &mut self,
        at: u32,
        pos: u32,
        count: u32,
    ) -> Result<Option<(usize, usize)>, Halt> {
        let Some(Inst::Many {
            single, back, max, ..
        }) = self.program.code.get(as_index(at)).copied()
        else {
            return Err(Halt::Broken);
        };
        let Some((c, after)) = self.read(as_index(pos), back) else {
            return Ok(None);
        };
        if !self.is(single, c) {
            return Ok(None);
        }
        let count = count.saturating_add(1);
        if count < max {
            self.push(Undo::Advance {
                at,
                pos: as_position(after)?,
                count,
            })?;
        }
        Ok(Some((as_index(at).saturating_add(1), after)))
    }

    /// Ask whether the embedder wants the script stopped.
    fn ask(&self) -> Result<(), Halt> {
        if (self.stop)() {
            return Err(Halt::Stopped { steps: self.steps });
        }
        Ok(())
    }

    /// [`Inst::Backreference`]: what the first group of the list that took
    /// part captured, matched again; nothing at all if none did.
    fn backreference(&mut self, list: u32, back: bool, pos: usize) -> Result<Option<usize>, Halt> {
        let groups = self
            .program
            .references
            .get(as_index(list))
            .ok_or(Halt::Broken)?;
        let captured = groups.iter().find_map(|group| {
            let start = self.slot(group.saturating_mul(2));
            let end = self.slot(group.saturating_mul(2).saturating_add(1));
            (start != NONE && end != NONE).then(|| (as_index(start), as_index(end)))
        });
        let Some((start, end)) = captured else {
            return Ok(Some(pos));
        };
        let length = end.saturating_sub(start);
        self.tick(length as u64)?;
        let wanted = self.input.get(start..end).ok_or(Halt::Broken)?;
        let (from, after) = if back {
            let Some(from) = pos.checked_sub(length) else {
                return Ok(None);
            };
            (from, from)
        } else {
            (pos, pos.saturating_add(length))
        };
        let here = self.input.get(from..from.saturating_add(length));
        Ok((here == Some(wanted)).then_some(after))
    }

    /// Read the character at `pos`, or before it when `back` is set,
    /// answering it and the position past it.
    fn read(&self, pos: usize, back: bool) -> Option<(u32, usize)> {
        if back {
            let before = pos.checked_sub(1)?;
            let unit = u32::from(*self.input.get(before)?);
            if self.code_points && is_trail(unit) {
                if let Some(lead) = before
                    .checked_sub(1)
                    .and_then(|at| self.input.get(at))
                    .map(|lead| u32::from(*lead))
                    .filter(|lead| is_lead(*lead))
                {
                    return Some((combined(lead, unit), before.saturating_sub(1)));
                }
            }
            return Some((unit, before));
        }
        let unit = u32::from(*self.input.get(pos)?);
        if self.code_points && is_lead(unit) {
            if let Some(trail) = self
                .input
                .get(pos.saturating_add(1))
                .map(|trail| u32::from(*trail))
                .filter(|trail| is_trail(*trail))
            {
                return Some((combined(unit, trail), pos.saturating_add(2)));
            }
        }
        Some((unit, pos.saturating_add(1)))
    }

    /// Whether `c` is what `single` matches.
    fn is(&self, single: Single, c: u32) -> bool {
        match single {
            Single::Char(wanted) => c == wanted,
            Single::Any { line_endings } => line_endings || !is_line_ending(c),
            Single::Set(set) => self
                .program
                .sets
                .get(as_index(set))
                .is_some_and(|set| set.matches(c)),
        }
    }

    /// `IsWordChar`: whether the code unit at `at` is a letter, a digit or
    /// `_`.
    fn is_word(&self, at: usize) -> bool {
        self.input
            .get(at)
            .and_then(|c| u8::try_from(*c).ok())
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_')
    }

    fn slot(&self, slot: u32) -> u32 {
        self.slots.get(as_index(slot)).copied().unwrap_or(NONE)
    }

    fn set_slot(&mut self, slot: u32, value: u32) -> Result<(), Halt> {
        let old = self.slot(slot);
        if old == value {
            return Ok(());
        }
        self.push(Undo::Slot { slot, old })?;
        let held = self.slots.get_mut(as_index(slot)).ok_or(Halt::Broken)?;
        *held = value;
        Ok(())
    }

    fn register(&self, index: u32) -> u32 {
        self.registers.get(as_index(index)).copied().unwrap_or(0)
    }

    fn set_register(&mut self, index: u32, value: u32) -> Result<(), Halt> {
        let old = self.register(index);
        if old == value {
            return Ok(());
        }
        self.push(Undo::Register { index, old })?;
        let held = self
            .registers
            .get_mut(as_index(index))
            .ok_or(Halt::Broken)?;
        *held = value;
        Ok(())
    }
}

/// A `u32` from the program or the stack, as an index.
fn as_index(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// An index, as the `u32` the stack keeps. A string is at most
/// [`bounds::LONGEST_STRING`] code units and a program far shorter than four
/// thousand million instructions, so this always fits; it is answered rather
/// than assumed.
fn as_position(value: usize) -> Result<u32, Halt> {
    u32::try_from(value).map_err(|_| Halt::Broken)
}

/// Whether `index` falls between the halves of a surrogate pair.
fn splits_a_pair(input: &[u16], index: usize) -> bool {
    let after = input.get(index).is_some_and(|c| is_trail(u32::from(*c)));
    let before = index
        .checked_sub(1)
        .and_then(|at| input.get(at))
        .is_some_and(|c| is_lead(u32::from(*c)));
    after && before
}

/// `LineTerminator`.
const fn is_line_ending(c: u32) -> bool {
    matches!(c, 0x0A | 0x0D | 0x2028 | 0x2029)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::{Found, Halt, search};
    use crate::bounds;
    use crate::regexp::compile;

    fn units(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    fn found(body: &str, flags: &str, input: &str, from: usize) -> Result<Option<Found>, Halt> {
        let Ok(program) = compile(body, flags) else {
            panic!("{body} compiles");
        };
        search(&program, &units(input), from, &|| false)
    }

    #[test]
    fn a_search_answers_where_the_match_and_each_group_are() {
        assert_eq!(
            found("(b)(x)?", "", "abc", 0),
            Ok(Some(Found {
                captures: vec![Some((1, 2)), Some((1, 2)), None]
            }))
        );
        assert_eq!(
            found("b", "y", "abc", 0),
            Ok(None),
            "sticky looks only at 0"
        );
        assert_eq!(found("b", "", "abc", 2), Ok(None));
        assert_eq!(found("", "", "abc", 3).map(|m| m.is_some()), Ok(true));
        assert_eq!(
            found("", "", "abc", 4),
            Ok(None),
            "past the end finds nothing"
        );
    }

    #[test]
    fn a_catastrophic_pattern_runs_out_of_steps_rather_than_running_for_ever() {
        let input = format!("{}b", "a".repeat(30));
        assert_eq!(found("(a+)+$", "", &input, 0), Err(Halt::Steps));
        // The same pattern on an input it can match at once is a few steps.
        assert!(matches!(found("(a+)+$", "", "aaa", 0), Ok(Some(_))));
    }

    #[test]
    fn the_stop_ends_a_search_well_inside_the_budget() {
        let Ok(program) = compile("(a+)+$", "") else {
            panic!("compiles");
        };
        let input = units(&format!("{}b", "a".repeat(30)));
        let asked = Cell::new(0_u32);
        let stop = || {
            asked.set(asked.get().saturating_add(1));
            true
        };
        let Err(Halt::Stopped { steps }) = search(&program, &input, 0, &stop) else {
            panic!("a stop asked for is a stop");
        };
        assert!(
            steps <= 1024,
            "it is asked on every choice, so it ends within a thousand steps: {steps}"
        );
        assert!(steps < bounds::STEPS_IN_A_MATCH);
        assert_eq!(asked.get(), 1, "and it stopped the first time it asked");
    }

    #[test]
    fn a_repeated_group_reaches_the_ceiling_on_places_and_a_repeated_character_does_not() {
        let long = "a".repeat(bounds::PLACES_IN_A_MATCH);
        assert_eq!(found("(?:a|b)*c", "", &long, 0), Err(Halt::Places));
        assert!(matches!(found("^.*$", "", &long, 0), Ok(Some(_))));
        assert!(matches!(found("^a*?$", "", &long, 0), Ok(Some(_))));
    }

    #[test]
    fn a_lookaround_forgets_its_choices_and_keeps_or_drops_its_captures() {
        // A lookahead is atomic: `(?=(a+))a*b\1` cannot come back into it to
        // make the group shorter.
        assert_eq!(
            found("(?=(a+))a*b\\1", "", "baaabac", 0),
            Ok(Some(Found {
                captures: vec![Some((3, 6)), Some((3, 4))]
            }))
        );
        // A negative lookahead keeps nothing.
        assert_eq!(
            found("(?!(a)c)a", "", "ab", 0),
            Ok(Some(Found {
                captures: vec![Some((0, 1)), None]
            }))
        );
        // A capture set in a lookahead is put back when the match fails
        // further on and tries the next start.
        assert_eq!(
            found("(?=(\\w))\\w-", "", "ab-", 0),
            Ok(Some(Found {
                captures: vec![Some((1, 3)), Some((1, 2))]
            }))
        );
    }
}
