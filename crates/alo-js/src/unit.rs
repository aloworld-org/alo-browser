/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A whole compiled program: one pool of strings, and every [`Chunk`] in it.
//!
//! Item 72 compiled a script to a single chunk, and its strings lived in the
//! chunk. A function has a chunk of its own (item 209), so there is now more
//! than one — and the question that decides this file's shape is *where the
//! strings go*.
//!
//! # Why one pool rather than one per chunk
//!
//! A run turns every string constant into a heap cell once, interns it, and
//! keeps the interned string as the constant — which is what stops a key naming
//! a cell the next collection takes away (see
//! [`Engine::run`](crate::interpret::Engine::run)). If each chunk carried its
//! own strings, that work would happen again on the first call of every
//! function, and `a.b` written in ten functions would be ten cells rather than
//! one.
//!
//! So the strings belong to the **program** and the instructions index into it.
//! A chunk then holds nothing but code, which is also what keeps a chunk
//! outside the collector's business entirely: there is no edge in one to trace.
//!
//! # The script is chunk zero
//!
//! Functions finish compiling before the script they are written in does — the
//! compiler is part way through the script's own chunk when it meets one — so
//! they are added first and would otherwise be numbered first. Chunk zero is
//! reserved when the unit is made and filled in at the end, so that *the
//! program* is always the same index however many functions it holds.

use std::fmt;

use crate::code::Chunk;

/// One compiled program.
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    texts: Vec<Vec<u16>>,
    chunks: Vec<Chunk>,
}

impl Default for Unit {
    fn default() -> Self {
        Self::new()
    }
}

impl Unit {
    /// A unit with nothing in it but the empty place chunk zero will take.
    pub fn new() -> Self {
        Self {
            texts: Vec::new(),
            chunks: vec![Chunk::new(false)],
        }
    }

    /// The script's own chunk, which is always chunk zero.
    pub fn script(&self) -> &Chunk {
        self.chunks.first().unwrap_or(&EMPTY)
    }

    /// Put the script's finished chunk in place.
    pub fn finish(&mut self, chunk: Chunk) {
        if let Some(place) = self.chunks.first_mut() {
            *place = chunk;
        }
    }

    /// The chunk at an index, or [`None`] if the index names none — which is
    /// this engine's own mistake rather than a script's.
    pub fn chunk(&self, at: u32) -> Option<&Chunk> {
        self.chunks.get(usize::try_from(at).ok()?)
    }

    /// How many chunks there are, the script's included.
    pub fn chunks(&self) -> usize {
        self.chunks.len()
    }

    /// Add a function's chunk, answering the index instructions name it by.
    ///
    /// [`None`] when there are more chunks than an index can name, which is a
    /// program with four thousand million functions in it.
    pub fn add(&mut self, chunk: Chunk) -> Option<u32> {
        let at = u32::try_from(self.chunks.len()).ok()?;
        self.chunks.push(chunk);
        Some(at)
    }

    /// The string constants, in the order the instructions index them.
    pub fn texts(&self) -> &[Vec<u16>] {
        &self.texts
    }

    /// The text at an index.
    pub fn text(&self, at: u32) -> Option<&[u16]> {
        self.texts.get(usize::try_from(at).ok()?).map(Vec::as_slice)
    }

    /// The index of these code units among the constants, adding them if they
    /// are new.
    ///
    /// Shared rather than repeated, which matters more than it looks: every
    /// property name and every global name is a text, so a loop reading `a.b`
    /// has one entry rather than one per instruction — and now that the pool is
    /// the program's, a name written in two functions is one entry as well.
    pub fn text_index(&mut self, units: &[u16]) -> Option<u32> {
        if let Some(at) = self.texts.iter().position(|held| held == units) {
            return u32::try_from(at).ok();
        }
        let at = u32::try_from(self.texts.len()).ok()?;
        self.texts.push(units.to_vec());
        Some(at)
    }
}

impl fmt::Display for Unit {
    /// How many chunks and how many strings, which is what a person debugging a
    /// compile wants first.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "a program of {} chunk(s) and {} string(s)",
            self.chunks.len(),
            self.texts.len()
        )
    }
}

/// The chunk a unit with no chunk zero would answer with, which cannot happen:
/// [`Unit::new`] puts one there. It is here so that [`Unit::script`] needs no
/// `unwrap` to say so.
static EMPTY: Chunk = Chunk::empty();

#[cfg(test)]
mod tests {
    use super::Unit;
    use crate::code::{Chunk, Op};

    #[test]
    fn the_script_is_chunk_zero_however_many_functions_came_first() {
        let mut unit = Unit::new();
        let mut inner = Chunk::new(true);
        inner.emit(Op::Undefined, 0);
        assert_eq!(unit.add(inner), Some(1), "a function is numbered after it");

        let mut script = Chunk::new(false);
        script.emit(Op::Null, 0);
        unit.finish(script);
        assert_eq!(unit.script().op(0), Some(Op::Null));
        assert_eq!(unit.chunk(0).map(Chunk::code), Some(&[Op::Null][..]));
        assert_eq!(unit.chunk(1).map(Chunk::strict), Some(true));
        assert_eq!(unit.chunk(2), None);
    }

    #[test]
    fn a_text_is_kept_once_however_often_it_is_named() {
        let mut unit = Unit::new();
        let units: Vec<u16> = "a".encode_utf16().collect();
        assert_eq!(unit.text_index(&units), Some(0));
        assert_eq!(unit.text_index(&units), Some(0));
        assert_eq!(unit.texts().len(), 1);
        assert_eq!(unit.text(0), Some(units.as_slice()));
        assert_eq!(unit.text(1), None);
    }
}
