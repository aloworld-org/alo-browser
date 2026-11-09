/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The coverage a drawing made, kept for the next drawing of the same
//! screen (queue item 871, ADR 0101).
//!
//! A shape's coverage, a shadow's blurred coverage and a clip's mask are
//! each a function of nothing but the shape where the transform in force
//! puts it, the fill rule, the page's size and the blur: the same numbers
//! in, the same bytes out. So a coverage made for one drawing is the very
//! coverage the next would make for the same shape, and is kept for it
//! rather than made again. A card's two shadows blurred at two took 49 ms
//! of the first screen's paint; the next drawing that paints any pixel of
//! them now takes them as they were made.
//!
//! What is kept is only what the last drawing's list draws, or what was
//! compared to paint it ([`crate::damage`]): everything else is let go at
//! the end of each drawing ([`Kept::end`]). And never more than
//! [`MOST_BYTES`]: a coverage past that is made and not kept, which costs
//! time and never a pixel.

use crate::backdrop::Blurred;
use crate::clip::Clip;
use crate::coverage::Coverage;
use crate::path::{Path, Segment};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The most bytes of coverage kept between drawings: a first screen at two
/// with a card in it keeps about 30 MB.
pub const MOST_BYTES: usize = 64 << 20;

/// What a coverage is of, as numbers compared bit for bit: its kind, what
/// was asked of it, and its shape point by point — two shapes alike to the
/// bit are filled alike, and two that are not are not taken to be.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Key(Vec<u32>);

/// The kinds of coverage kept, each its own first number in a key.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Made {
    /// A shape filled on a page, by a rule ([`crate::fill_on_page`]).
    Filled,
    /// A shape filled and blurred, a box's shadow.
    Blurred,
    /// A clip's mask ([`crate::clip::mask`]).
    Mask,
    /// The clips open around an item, combined ([`Clip::intersected`]).
    Within,
    /// A box's backdrop blurred ([`crate::backdrop`], queue item 885): kept
    /// as it was blurred, and taken only while what it was blurred from
    /// cannot have changed ([`Kept::stand`]).
    Backdrop,
    /// A run of text's letters filled ([`crate::drawn_text`], queue item
    /// 888): counted, never kept, so never a key.
    Letters,
}

impl Key {
    /// The key of a coverage of `kind`, asked with `asked`, of `shape`
    /// already under the transform in force.
    pub(crate) fn of(kind: Made, asked: &[u32], shape: &Path) -> Self {
        let mut words = Vec::with_capacity(2 + asked.len() + shape.segments().len() * 7);
        words.push(kind as u32);
        words.push(u32::try_from(asked.len()).unwrap_or(u32::MAX));
        words.extend_from_slice(asked);
        for segment in shape.segments() {
            match segment {
                Segment::MoveTo(point) => words.extend([0, point.x.to_bits(), point.y.to_bits()]),
                Segment::LineTo(point) => words.extend([1, point.x.to_bits(), point.y.to_bits()]),
                Segment::QuadTo(one, two) => words.extend([
                    2,
                    one.x.to_bits(),
                    one.y.to_bits(),
                    two.x.to_bits(),
                    two.y.to_bits(),
                ]),
                Segment::CubicTo(one, two, three) => words.extend([
                    3,
                    one.x.to_bits(),
                    one.y.to_bits(),
                    two.x.to_bits(),
                    two.y.to_bits(),
                    three.x.to_bits(),
                    three.y.to_bits(),
                ]),
                Segment::Close => words.push(4),
            }
        }
        Self(words)
    }

    /// The key of the clips `outer` combined with the mask `inner` inside
    /// them: both whole, each said with its length, so no two chains of
    /// clips share one.
    pub(crate) fn within(outer: Option<&Self>, inner: &Self) -> Self {
        let outer = outer.map_or(&[][..], |key| key.0.as_slice());
        let mut words = Vec::with_capacity(3 + outer.len() + inner.0.len());
        words.push(Made::Within as u32);
        words.push(u32::try_from(outer.len()).unwrap_or(u32::MAX));
        words.extend_from_slice(outer);
        words.extend_from_slice(&inner.0);
        Self(words)
    }
}

/// The coverage kept from one drawing to the next, and how much of it was
/// made rather than taken.
#[derive(Clone, Default)]
pub struct Kept {
    coverages: HashMap<Key, Arc<Coverage>>,
    clips: HashMap<Key, Arc<Clip>>,
    backdrops: HashMap<Key, Arc<Blurred>>,
    /// The backdrops whose crops nothing drawn before them changed since
    /// the last drawing, so that the blur it kept is this one's
    /// ([`crate::damage`]): only for the drawing they were said for.
    standing: HashSet<Key>,
    /// What this drawing drew or compared: what is kept past it.
    used: HashSet<Key>,
    bytes: usize,
    made: Making,
}

/// How many of each kind the last drawing made, rather than took as kept:
/// what a test holds a drawing to, never a clock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Making {
    /// Shapes filled.
    pub filled: u32,
    /// Shadows blurred.
    pub blurred: u32,
    /// Clips' masks made.
    pub masks: u32,
    /// Clips combined.
    pub combined: u32,
    /// Backdrops blurred (queue item 885).
    pub backdrops: u32,
    /// Runs of text whose letters were filled, never kept (queue item
    /// 888): on a confined canvas, only the runs that could reach what was
    /// painted again.
    pub letters: u32,
}

impl core::fmt::Debug for Kept {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        out.debug_struct("Kept")
            .field("coverages", &self.coverages.len())
            .field("clips", &self.clips.len())
            .field("backdrops", &self.backdrops.len())
            .field("standing", &self.standing.len())
            .field("used", &self.used.len())
            .field("bytes", &self.bytes)
            .field("made", &self.made)
            .finish()
    }
}

impl Kept {
    /// Nothing kept.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many bytes of coverage are kept.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// What the last drawing made rather than took as kept.
    pub fn made(&self) -> Making {
        self.made
    }

    /// A drawing begun: nothing is yet used by it, or made for it.
    pub(crate) fn begin(&mut self) {
        self.used.clear();
        self.standing.clear();
        self.made = Making::default();
    }

    /// The backdrops of this drawing whose blur the last drawing's is, by
    /// their keys: said by [`crate::damage`] for a repaint, and only for
    /// this drawing.
    pub(crate) fn stand(&mut self, standing: HashSet<Key>) {
        self.standing = standing;
    }

    /// Whether a backdrop's blur is kept under `key`.
    pub(crate) fn holds_backdrop(&self, key: &Key) -> bool {
        self.backdrops.contains_key(key)
    }

    /// The backdrop blurred under `key`: as kept, while it stands, or
    /// blurred by `make` and kept while there is room.
    pub(crate) fn backdrop(&mut self, key: &Key, make: impl FnOnce() -> Blurred) -> Arc<Blurred> {
        self.used.insert(key.clone());
        if self.standing.contains(key)
            && let Some(kept) = self.backdrops.get(key)
        {
            return Arc::clone(kept);
        }
        self.count(Made::Backdrop);
        let fresh = Arc::new(make());
        if let Some(was) = self.backdrops.remove(key) {
            self.bytes = self.bytes.saturating_sub(was.bytes());
        }
        if self.bytes + fresh.bytes() <= MOST_BYTES {
            self.bytes += fresh.bytes();
            self.backdrops.insert(key.clone(), Arc::clone(&fresh));
        }
        fresh
    }

    /// A drawing ended: only what it used is kept.
    pub(crate) fn end(&mut self) {
        let used = std::mem::take(&mut self.used);
        self.coverages.retain(|key, _| used.contains(key));
        self.clips.retain(|key, _| used.contains(key));
        self.backdrops.retain(|key, _| used.contains(key));
        self.bytes = self
            .coverages
            .values()
            .map(|coverage| coverage.data().len())
            .chain(self.clips.values().map(|clip| clip.bytes()))
            .chain(self.backdrops.values().map(|blurred| blurred.bytes()))
            .sum();
        self.used = used;
        self.standing.clear();
    }

    /// What `key` names is used by this drawing, though not drawn by it: a
    /// shape passed over on a confined canvas is kept for the next.
    pub(crate) fn touch(&mut self, key: &Key) {
        if self.coverages.contains_key(key)
            || self.clips.contains_key(key)
            || self.backdrops.contains_key(key)
        {
            self.used.insert(key.clone());
        }
    }

    /// The coverage `key` names: as kept, or made by `make` and kept while
    /// there is room.
    pub(crate) fn coverage(
        &mut self,
        key: &Key,
        kind: Made,
        make: impl FnOnce() -> Coverage,
    ) -> Arc<Coverage> {
        self.used.insert(key.clone());
        if let Some(kept) = self.coverages.get(key) {
            return Arc::clone(kept);
        }
        self.count(kind);
        let fresh = Arc::new(make());
        if self.bytes + fresh.data().len() <= MOST_BYTES {
            self.bytes += fresh.data().len();
            self.coverages.insert(key.clone(), Arc::clone(&fresh));
        }
        fresh
    }

    /// The combined clip `key` names: as kept, or made by `make` and kept
    /// while there is room.
    pub(crate) fn clip(&mut self, key: &Key, make: impl FnOnce() -> Clip) -> Arc<Clip> {
        self.used.insert(key.clone());
        if let Some(kept) = self.clips.get(key) {
            return Arc::clone(kept);
        }
        self.count(Made::Within);
        let fresh = Arc::new(make());
        if self.bytes + fresh.bytes() <= MOST_BYTES {
            self.bytes += fresh.bytes();
            self.clips.insert(key.clone(), Arc::clone(&fresh));
        }
        fresh
    }

    /// One more of `kind` made.
    pub(crate) fn count(&mut self, kind: Made) {
        let count = match kind {
            Made::Filled => &mut self.made.filled,
            Made::Blurred => &mut self.made.blurred,
            Made::Mask => &mut self.made.masks,
            Made::Within => &mut self.made.combined,
            Made::Backdrop => &mut self.made.backdrops,
            Made::Letters => &mut self.made.letters,
        };
        *count = count.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::fill;

    #[test]
    fn a_coverage_is_made_once_and_kept_while_it_is_used() {
        let mut kept = Kept::new();
        let shape = Path::rectangle(0.0, 0.0, 4.0, 4.0);
        let key = Key::of(Made::Filled, &[], &shape);
        kept.begin();
        let first = kept.coverage(&key, Made::Filled, || fill(&shape));
        kept.end();
        kept.begin();
        let again = kept.coverage(&key, Made::Filled, Coverage::empty);
        kept.end();
        assert!(Arc::ptr_eq(&first, &again), "taken as kept, not made again");
        assert_eq!(kept.made().filled, 0);
        kept.begin();
        kept.end();
        kept.begin();
        let after = kept.coverage(&key, Made::Filled, || fill(&shape));
        assert!(
            !Arc::ptr_eq(&first, &after),
            "let go by a drawing that used none of it"
        );
        assert_eq!(kept.made().filled, 1);
    }

    #[test]
    fn a_touched_coverage_is_kept_and_shapes_differing_by_a_bit_are_two() {
        let mut kept = Kept::new();
        let shape = Path::rectangle(0.0, 0.0, 4.0, 4.0);
        let key = Key::of(Made::Blurred, &[3], &shape);
        kept.begin();
        kept.coverage(&key, Made::Blurred, || fill(&shape));
        kept.end();
        kept.begin();
        kept.touch(&key);
        kept.end();
        assert!(kept.bytes() > 0, "passed over, and still kept");
        let zero = Path::rectangle(-0.0, 0.0, 4.0, 4.0);
        assert_ne!(Key::of(Made::Blurred, &[3], &zero), key);
        assert_ne!(Key::of(Made::Blurred, &[4], &shape), key);
        assert_ne!(Key::of(Made::Filled, &[3], &shape), key);
        assert_ne!(
            Key::within(None, &key),
            Key::within(Some(&key), &key),
            "a chain of clips is said with its length"
        );
    }

    #[test]
    fn nothing_past_the_most_is_kept() {
        let mut kept = Kept::new();
        kept.begin();
        let big = Path::rectangle(0.0, 0.0, 8193.0, 8192.0);
        let key = Key::of(Made::Filled, &[], &big);
        let made = kept.coverage(&key, Made::Filled, || {
            Coverage::new(8193, 8192, (0, 0), vec![0; 8193 * 8192])
        });
        assert_eq!(made.width(), 8193);
        assert_eq!(kept.bytes(), 0);
    }
}
