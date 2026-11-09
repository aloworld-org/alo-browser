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
//! What is kept is what the last drawing's list draws, or what was
//! compared to paint it ([`crate::damage`]); and, **laid aside**, the most
//! recently used of what earlier drawings made and this one did not use, up
//! to [`ASIDE_BYTES`] (queue item 892): *Settings* closed and opened again
//! finds its panel's two shadows, its fills and its clips as its last
//! opening made them, rather than blurring the shadows again — 42 ms of the
//! 54 at two. Everything else is let go at the end of each drawing
//! ([`Kept::end`]). And never more than [`MOST_BYTES`]: a coverage past
//! that is made and not kept, which costs time and never a pixel. What is
//! laid aside is let go first when a coverage of this drawing needs its
//! room.
//!
//! Laying aside is right for the reason keeping is: a coverage is a
//! function of its key alone, so what was made under a key a hundred
//! drawings ago is the very coverage this one would make under it. A
//! backdrop's blur is a function of the pixels under it as well (ADR 0104
//! § 2): it is kept with the items drawn before it ([`BlurredFrom`], queue
//! item 894), and laid aside with them, and taken only while what it reads
//! stands ([`Kept::stand`]) or a drawing's items before it draw alike every
//! one of those.

use crate::backdrop::Blurred;
use crate::blurred_from::BlurredFrom;
use crate::clip::Clip;
use crate::coverage::Coverage;
use crate::display::DisplayItem;
use crate::path::{Path, Segment};
use alo_value::Rgba;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The most bytes of coverage kept between drawings: a first screen at two
/// with a card in it keeps about 30 MB.
pub const MOST_BYTES: usize = 64 << 20;

/// The most bytes of coverage kept past the drawing that last used it
/// (queue item 892), inside [`MOST_BYTES`]: *Settings* closed over a card at
/// two lays aside about 31 MB — its panel's shadows, fills and clips, and
/// its veil's blur (queue item 894, 3.4 MB), for which this grew from 24
/// MiB.
pub const ASIDE_BYTES: usize = 32 << 20;

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
    backdrops: HashMap<Key, KeptBlur>,
    /// The colour the canvas being drawn was made in, when a repaint says
    /// so ([`Kept::on`]): what a backdrop's blur is kept as blurred from.
    ground: Option<Rgba>,
    /// The backdrops whose crops nothing drawn before them changed since
    /// the last drawing, so that the blur it kept is this one's
    /// ([`crate::damage`]): only for the drawing they were said for.
    standing: HashSet<Key>,
    /// What this drawing drew or compared: what is kept past it.
    used: HashSet<Key>,
    /// The runs of text this drawing outlined, by their place in its list
    /// and what they are drawn under: made once for every piece the
    /// drawing is painted in (queue item 895), let go as it ends.
    runs: HashMap<RunAt, Run>,
    /// The pieces this drawing was painted in (queue item 896): what of a
    /// frame crosses for it.
    pieces: Vec<crate::region::Region>,
    /// The runs' outlines this drawing made or took, by what they were set
    /// as ([`Set`]): kept for the next drawing, which takes those it sets
    /// alike rather than outlining them again.
    outlines: Outlines,
    /// The last drawing's.
    outlined_before: Outlines,
    /// Which drawing last used each coverage and clip kept, by its count
    /// ([`Kept::drawings`]): the newest of those this drawing did not use
    /// are laid aside, the oldest let go first.
    last_used: HashMap<Key, u64>,
    /// How many drawings have ended.
    drawings: u64,
    bytes: usize,
    made: Making,
}

/// A backdrop's blur, and the items drawn before it that it was blurred
/// from, when they could be said.
#[derive(Debug, Clone)]
struct KeptBlur {
    blurred: Arc<Blurred>,
    from: Option<BlurredFrom>,
}

/// Runs' outlines by what they were set as, found by [`Set::hashed`].
type Outlines = HashMap<u64, Vec<(Set, Arc<Vec<Path>>)>>;

/// The items of a list drawn before a backdrop, the list's scale and the
/// canvas's size: what a blur is kept as blurred from ([`BlurredFrom`]).
pub(crate) type DrawnBefore<'a> = (&'a [DisplayItem], f32, (u32, u32));

/// Where a run of text is in a drawing: its item's place in the list, the
/// transform in force, and the screen's scale, bit for bit.
type RunAt = (usize, [u32; 7]);

/// A run of text outlined for a drawing: its glyphs as they are filled, and
/// the bound of their ink once it was asked ([`crate::drawn_text`]).
#[derive(Debug, Clone)]
pub(crate) struct Run {
    glyphs: Arc<Vec<Path>>,
    reach: Option<crate::extent::Reach>,
}

impl Run {
    /// Glyphs outlined, their bound not yet asked.
    pub(crate) fn of(glyphs: Vec<Path>) -> Self {
        Self {
            glyphs: Arc::new(glyphs),
            reach: None,
        }
    }

    /// Glyphs outlined before, their bound not yet asked.
    fn taken(glyphs: Arc<Vec<Path>>) -> Self {
        Self {
            glyphs,
            reach: None,
        }
    }

    /// The glyphs, each where it is drawn.
    pub(crate) fn glyphs(&self) -> &[Path] {
        &self.glyphs
    }

    /// The bound of their ink, if it was asked.
    pub(crate) fn reach(&self) -> Option<crate::extent::Reach> {
        self.reach
    }
}

/// What a run of text is outlined from (queue item 895): its words, its
/// face — the very face — and its size, the room after each character,
/// where its pen starts and how far it leans, and the screen's scale, each
/// number bit for bit. Two runs set alike are outlined alike
/// ([`crate::drawn_text::run_glyphs`] reads nothing else).
#[derive(Debug, Clone)]
pub(crate) struct Set {
    text: String,
    font: alo_text::Font,
    numbers: [u32; 6],
}

impl Set {
    /// A run set as `set` on a screen of `scale`.
    pub(crate) fn of(set: &crate::drawn_text::Set<'_>, scale: f32) -> Self {
        Self {
            text: set.text.to_owned(),
            font: set.font.clone(),
            numbers: [
                set.size,
                set.letter_spacing,
                set.origin.0,
                set.origin.1,
                set.oblique,
                scale,
            ]
            .map(f32::to_bits),
        }
    }

    /// Whether `other` is set alike.
    fn is(&self, other: &Self) -> bool {
        self.numbers == other.numbers && self.text == other.text && self.font.is(&other.font)
    }

    /// What it is found by: its words and numbers, hashed.
    fn hashed(&self) -> u64 {
        use core::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.text.hash(&mut hasher);
        self.numbers.hash(&mut hasher);
        hasher.finish()
    }
}

/// [`RunAt`] of a place, a transform and a scale.
fn run_at((index, at, scale): (usize, alo_value::Matrix, f32)) -> RunAt {
    (
        index,
        [at.a, at.b, at.c, at.d, at.e, at.f, scale].map(f32::to_bits),
    )
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
    /// Backdrops' blurs taken because the items drawn before them drew
    /// alike, every one, those they were blurred from (queue item 894).
    pub blurs_alike: u32,
    /// Runs of text whose letters were filled, never kept (queue item
    /// 888): on a confined canvas, only the runs that could reach what was
    /// painted again.
    pub letters: u32,
    /// Runs of text outlined: each at most once a drawing, however many
    /// pieces it is painted in, and not at all when the drawing before set
    /// it alike (queue item 895).
    pub outlined: u32,
    /// Coverages and clips taken from what was laid aside, unused by the
    /// drawing before (queue item 892).
    pub aside: u32,
    /// Pieces painted again, each on its own (queue item 891): none for a
    /// drawing painted whole or not at all.
    pub pieces: u32,
    /// The pixels of those pieces, together.
    pub painted: u64,
}

impl core::fmt::Debug for Kept {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        out.debug_struct("Kept")
            .field("coverages", &self.coverages.len())
            .field("clips", &self.clips.len())
            .field("backdrops", &self.backdrops.len())
            .field("standing", &self.standing.len())
            .field("used", &self.used.len())
            .field("runs", &self.runs.len())
            .field("outlines", &self.outlines.len())
            .field("outlined_before", &self.outlined_before.len())
            .field("pieces", &self.pieces.len())
            .field("ground", &self.ground)
            .field("last_used", &self.last_used.len())
            .field("drawings", &self.drawings)
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

    /// The colour the canvas drawn next was made in — a new canvas, or the
    /// pieces of one put back to it ([`crate::repaint()`]) — or [`None`]
    /// when the canvas holds anything else: a backdrop's blur is kept as
    /// blurred from the items before it only on a canvas whose colour is
    /// said.
    pub(crate) fn on(&mut self, ground: Option<Rgba>) {
        self.ground = ground;
    }

    /// A drawing begun: nothing is yet used by it, or made for it.
    pub(crate) fn begin(&mut self) {
        self.used.clear();
        self.runs.clear();
        self.pieces.clear();
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

    /// The backdrop blurred under `key`, drawn after `before` — the
    /// list's items before it, at its scale, on a canvas of `size`: as
    /// kept, while what it reads stands or `before` draws alike every item
    /// it was blurred from (queue item 894); or blurred by `make` and kept,
    /// with `before`, while there is room.
    pub(crate) fn backdrop(
        &mut self,
        key: &Key,
        (before, scale, size): DrawnBefore<'_>,
        make: impl FnOnce() -> Blurred,
    ) -> Arc<Blurred> {
        if let Some(kept) = self.backdrops.get(key) {
            if self.standing.contains(key) {
                let blurred = Arc::clone(&kept.blurred);
                self.used.insert(key.clone());
                return blurred;
            }
            if let (Some(ground), Some(from)) = (self.ground, &kept.from)
                && from.is(before, scale, size, ground)
            {
                let blurred = Arc::clone(&kept.blurred);
                self.taken_aside(key);
                self.used.insert(key.clone());
                self.made.blurs_alike = self.made.blurs_alike.saturating_add(1);
                return blurred;
            }
        }
        self.used.insert(key.clone());
        self.count(Made::Backdrop);
        let fresh = Arc::new(make());
        if let Some(was) = self.backdrops.remove(key) {
            self.bytes = self.bytes.saturating_sub(was.blurred.bytes());
        }
        if self.room(fresh.bytes()) {
            self.bytes += fresh.bytes();
            let from = self
                .ground
                .map(|ground| BlurredFrom::of(before, scale, size, ground));
            self.backdrops.insert(
                key.clone(),
                KeptBlur {
                    blurred: Arc::clone(&fresh),
                    from,
                },
            );
        }
        fresh
    }

    /// How many bytes of what is kept the last drawing did not use: laid
    /// aside (queue item 892).
    pub fn aside_bytes(&self) -> usize {
        self.coverages
            .iter()
            .filter(|(key, _)| !self.used.contains(*key))
            .map(|(_, coverage)| coverage.data().len())
            .chain(
                self.clips
                    .iter()
                    .filter(|(key, _)| !self.used.contains(*key))
                    .map(|(_, clip)| clip.bytes()),
            )
            .chain(
                self.backdrops
                    .iter()
                    .filter(|(key, _)| !self.used.contains(*key))
                    .map(|(_, kept)| kept.blurred.bytes()),
            )
            .sum()
    }

    /// A drawing ended: what it used is kept, and of what it did not, the
    /// most recently used laid aside up to [`ASIDE_BYTES`].
    pub(crate) fn end(&mut self) {
        let used = std::mem::take(&mut self.used);
        self.drawings = self.drawings.saturating_add(1);
        for key in &used {
            self.last_used.insert(key.clone(), self.drawings);
        }
        let mut unused: Vec<(u64, Key, usize)> = self
            .coverages
            .iter()
            .map(|(key, coverage)| (key, coverage.data().len()))
            .chain(self.clips.iter().map(|(key, clip)| (key, clip.bytes())))
            // A blur is laid aside only with what it was blurred from: one
            // without could never be taken again (queue item 894).
            .chain(
                self.backdrops
                    .iter()
                    .filter(|(_, kept)| kept.from.is_some())
                    .map(|(key, kept)| (key, kept.blurred.bytes())),
            )
            .filter(|(key, _)| !used.contains(*key))
            .map(|(key, bytes)| {
                let last = self.last_used.get(key).copied().unwrap_or(0);
                (last, key.clone(), bytes)
            })
            .collect();
        // Newest first; of two last used by the same drawing, the smaller
        // first, and then by key, so what is laid aside never depends on a
        // map's order.
        unused.sort_by(|one, two| {
            two.0
                .cmp(&one.0)
                .then(one.2.cmp(&two.2))
                .then_with(|| one.1.0.cmp(&two.1.0))
        });
        let mut aside = 0_usize;
        let mut laid: HashSet<Key> = HashSet::new();
        for (_, key, bytes) in unused {
            if aside.saturating_add(bytes) <= ASIDE_BYTES {
                aside += bytes;
                laid.insert(key);
            }
        }
        let keeps = |key: &Key| used.contains(key) || laid.contains(key);
        self.coverages.retain(|key, _| keeps(key));
        self.clips.retain(|key, _| keeps(key));
        self.backdrops.retain(|key, _| keeps(key));
        self.last_used.retain(|key, _| keeps(key));
        self.bytes = self.counted();
        self.used = used;
        self.standing.clear();
        self.runs.clear();
        // The runs this drawing outlined or took are the next drawing's to
        // take; the rest are let go.
        self.outlined_before = std::mem::take(&mut self.outlines);
    }

    /// Every byte kept, counted.
    fn counted(&self) -> usize {
        self.coverages
            .values()
            .map(|coverage| coverage.data().len())
            .chain(self.clips.values().map(|clip| clip.bytes()))
            .chain(self.backdrops.values().map(|kept| kept.blurred.bytes()))
            .sum()
    }

    /// Room for `bytes` more, making it by letting go of what is laid aside,
    /// the oldest first: whether there is room.
    fn room(&mut self, bytes: usize) -> bool {
        if self.bytes.saturating_add(bytes) <= MOST_BYTES {
            return true;
        }
        let mut aside: Vec<(u64, Key)> = self
            .coverages
            .keys()
            .chain(self.clips.keys())
            .chain(self.backdrops.keys())
            .filter(|key| !self.used.contains(*key))
            .map(|key| (self.last_used.get(key).copied().unwrap_or(0), key.clone()))
            .collect();
        aside.sort_by(|one, two| one.0.cmp(&two.0).then_with(|| one.1.0.cmp(&two.1.0)));
        for (_, key) in aside {
            if self.bytes.saturating_add(bytes) <= MOST_BYTES {
                break;
            }
            if let Some(coverage) = self.coverages.remove(&key) {
                self.bytes = self.bytes.saturating_sub(coverage.data().len());
            }
            if let Some(clip) = self.clips.remove(&key) {
                self.bytes = self.bytes.saturating_sub(clip.bytes());
            }
            if let Some(kept) = self.backdrops.remove(&key) {
                self.bytes = self.bytes.saturating_sub(kept.blurred.bytes());
            }
            self.last_used.remove(&key);
        }
        self.bytes.saturating_add(bytes) <= MOST_BYTES
    }

    /// Whether `key` names what this drawing has not yet used and the last
    /// did not either: laid aside, and counted as taken from there.
    fn taken_aside(&mut self, key: &Key) {
        if !self.used.contains(key)
            && self
                .last_used
                .get(key)
                .is_some_and(|last| *last < self.drawings)
        {
            self.made.aside = self.made.aside.saturating_add(1);
        }
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
        if let Some(kept) = self.coverages.get(key).map(Arc::clone) {
            self.taken_aside(key);
            self.used.insert(key.clone());
            return kept;
        }
        self.used.insert(key.clone());
        self.count(kind);
        let fresh = Arc::new(make());
        if self.room(fresh.data().len()) {
            self.bytes += fresh.data().len();
            self.coverages.insert(key.clone(), Arc::clone(&fresh));
        }
        fresh
    }

    /// The combined clip `key` names: as kept, or made by `make` and kept
    /// while there is room.
    pub(crate) fn clip(&mut self, key: &Key, make: impl FnOnce() -> Clip) -> Arc<Clip> {
        if let Some(kept) = self.clips.get(key).map(Arc::clone) {
            self.taken_aside(key);
            self.used.insert(key.clone());
            return kept;
        }
        self.used.insert(key.clone());
        self.count(Made::Within);
        let fresh = Arc::new(make());
        if self.room(fresh.bytes()) {
            self.bytes += fresh.bytes();
            self.clips.insert(key.clone(), Arc::clone(&fresh));
        }
        fresh
    }

    /// The run of text at `at` in this drawing, set as `set`: as an
    /// earlier piece of it outlined it, or a drawing before it outlined a
    /// run set alike, or outlined by `make` now — and kept for the rest of
    /// the drawing and the next (queue item 895).
    pub(crate) fn run(
        &mut self,
        at: (usize, alo_value::Matrix, f32),
        set: &crate::drawn_text::Set<'_>,
        make: impl FnOnce() -> Vec<Path>,
    ) -> Run {
        let scale = at.2;
        let at = run_at(at);
        if let Some(run) = self.runs.get(&at) {
            return run.clone();
        }
        let set = Set::of(set, scale);
        let hashed = set.hashed();
        let found = |outlines: &Outlines| {
            outlines.get(&hashed).and_then(|alike| {
                alike
                    .iter()
                    .find(|(each, _)| each.is(&set))
                    .map(|(_, glyphs)| Arc::clone(glyphs))
            })
        };
        let glyphs = if let Some(glyphs) = found(&self.outlines) {
            glyphs
        } else {
            let glyphs = found(&self.outlined_before).unwrap_or_else(|| {
                self.made.outlined = self.made.outlined.saturating_add(1);
                Arc::new(make())
            });
            self.outlines
                .entry(hashed)
                .or_default()
                .push((set, Arc::clone(&glyphs)));
            glyphs
        };
        let run = Run::taken(glyphs);
        self.runs.insert(at, run.clone());
        run
    }

    /// The bound of the ink of the run at `at`, as asked of its outlines,
    /// kept for the rest of the drawing.
    pub(crate) fn run_reach(
        &mut self,
        at: (usize, alo_value::Matrix, f32),
        reach: crate::extent::Reach,
    ) {
        if let Some(run) = self.runs.get_mut(&run_at(at)) {
            run.reach = Some(reach);
        }
    }

    /// One more piece painted again (queue item 891).
    pub(crate) fn painted(&mut self, piece: crate::region::Region) {
        self.made.pieces = self.made.pieces.saturating_add(1);
        self.made.painted = self.made.painted.saturating_add(piece.area());
        self.pieces.push(piece);
    }

    /// The pieces the last drawing was painted again in, each on its own
    /// (queue item 896): none for one painted whole or not at all.
    pub fn pieces(&self) -> &[crate::region::Region] {
        &self.pieces
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
        assert_eq!(kept.made().aside, 0, "the drawing before used it");
    }

    /// A coverage `side` pixels square, of nothing, under its own key.
    fn square(kept: &mut Kept, side: u32) -> (Key, Arc<Coverage>) {
        let shape = Path::rectangle(0.0, 0.0, 1.0, 1.0);
        let key = Key::of(Made::Blurred, &[side], &shape);
        let made = kept.coverage(&key, Made::Blurred, || {
            Coverage::new(side, side, (0, 0), vec![0; (side * side) as usize])
        });
        (key, made)
    }

    #[test]
    fn what_a_drawing_did_not_use_is_laid_aside_and_taken_again_as_it_was() {
        // Queue item 892: a panel's shadow, made by one drawing, unused by
        // the next ten, is taken by the eleventh as it was made.
        let mut kept = Kept::new();
        kept.begin();
        let (key, first) = square(&mut kept, 64);
        kept.end();
        for _ in 0..10 {
            kept.begin();
            kept.end();
        }
        assert_eq!(kept.aside_bytes(), 64 * 64);
        kept.begin();
        let again = kept.coverage(&key, Made::Blurred, Coverage::empty);
        assert!(Arc::ptr_eq(&first, &again), "taken as laid aside");
        assert_eq!((kept.made().blurred, kept.made().aside), (0, 1));
        kept.end();
        assert_eq!(kept.aside_bytes(), 0, "used again: kept, not aside");
    }

    #[test]
    fn past_what_may_be_laid_aside_the_oldest_is_let_go_first() {
        // Four of a third of the room each, then a drawing of none: the
        // first made is let go, the newer three stay.
        let side = 3000; // 9 MB a square: three fit in 32 MiB, four do not.
        let mut kept = Kept::new();
        let mut keys = Vec::new();
        for drawing in 0..4 {
            kept.begin();
            let shape = Path::rectangle(0.0, 0.0, 1.0, 1.0);
            let key = Key::of(Made::Blurred, &[drawing], &shape);
            kept.coverage(&key, Made::Blurred, || {
                Coverage::new(side, side, (0, 0), vec![0; (side * side) as usize])
            });
            keys.push(key);
            kept.end();
        }
        kept.begin();
        kept.end();
        let bytes = (side * side) as usize;
        assert_eq!(kept.aside_bytes(), 3 * bytes, "the newest that fit");
        kept.begin();
        for (index, key) in keys.iter().enumerate() {
            let made_before = kept.made().blurred;
            kept.coverage(key, Made::Blurred, || Coverage::new(1, 1, (0, 0), vec![0]));
            let taken = kept.made().blurred == made_before;
            assert_eq!(taken, index >= 1, "drawing {index}");
        }
    }

    #[test]
    fn what_is_laid_aside_makes_room_for_what_a_drawing_makes() {
        // Laid aside to its bound, then a drawing that makes nearly the
        // whole of the room: what was aside is let go, and the new is kept.
        let mut kept = Kept::new();
        for drawing in 0..3 {
            kept.begin();
            let shape = Path::rectangle(0.0, 0.0, 1.0, 1.0);
            let key = Key::of(Made::Blurred, &[drawing], &shape);
            kept.coverage(&key, Made::Blurred, || {
                Coverage::new(2896, 2896, (0, 0), vec![0; 2896 * 2896])
            });
            kept.end();
        }
        kept.begin();
        kept.end();
        assert!(kept.aside_bytes() > 2 * 2896 * 2896);
        kept.begin();
        let shape = Path::rectangle(0.0, 0.0, 2.0, 2.0);
        let key = Key::of(Made::Blurred, &[9], &shape);
        let side = 7000;
        kept.coverage(&key, Made::Blurred, || {
            Coverage::new(side, side, (0, 0), vec![0; (side * side) as usize])
        });
        kept.end();
        assert!(kept.bytes() <= MOST_BYTES);
        assert!(kept.aside_bytes() < 3 * 2896 * 2896, "the oldest let go");
        kept.begin();
        kept.coverage(&key, Made::Blurred, Coverage::empty);
        assert_eq!(
            kept.made().blurred,
            0,
            "the new one kept, in room made for it"
        );
    }

    #[test]
    fn a_backdrop_is_laid_aside_with_what_it_was_blurred_from_and_taken_only_from_that() {
        // Queue item 894: a blur made on a canvas whose colour is said is
        // kept with the items before it, laid aside by a drawing that drew
        // none, and taken by one whose items before it draw alike; made
        // again for any other items, and never kept on a canvas not said.
        let crop = crate::region::Region::new(0, 0, 4, 4).expect("pixels");
        let key = crate::backdrop::key(crop, 2.0);
        let canvas = crate::canvas::Canvas::new(4, 4, Rgba::WHITE);
        let blur = || crate::backdrop::blurred(&canvas, crop, 2.0);
        let under = |x: f32| DisplayItem::Fill {
            box_id: alo_box::BoxId::from_index_for_tests(0),
            path: Path::rectangle(x, 0.0, 2.0, 2.0),
            paint: crate::paint::Paint::Solid(Rgba::BLACK),
            rule: crate::fill_rule::FillRule::NonZero,
        };
        let (one, other) = ([under(0.0)], [under(1.0)]);
        let mut kept = Kept::new();
        kept.on(Some(Rgba::WHITE));
        kept.begin();
        let first = kept.backdrop(&key, (&one, 1.0, (4, 4)), blur);
        kept.end();
        kept.begin();
        kept.end();
        assert!(kept.holds_backdrop(&key), "laid aside");
        assert!(kept.aside_bytes() >= first.bytes());
        kept.begin();
        let again = kept.backdrop(&key, (&one, 1.0, (4, 4)), blur);
        assert!(Arc::ptr_eq(&first, &again), "taken as it was blurred");
        let made = kept.made();
        assert_eq!((made.backdrops, made.blurs_alike, made.aside), (0, 1, 1));
        kept.end();
        kept.begin();
        let moved = kept.backdrop(&key, (&other, 1.0, (4, 4)), blur);
        assert!(!Arc::ptr_eq(&first, &moved), "another item under it");
        assert_eq!(kept.made().backdrops, 1);
        kept.end();
        kept.begin();
        kept.backdrop(&key, (&other, 2.0, (4, 4)), blur);
        assert_eq!(kept.made().backdrops, 1, "another scale");
        kept.end();
        kept.on(None);
        kept.begin();
        kept.backdrop(&key, (&other, 2.0, (4, 4)), blur);
        assert_eq!(kept.made().backdrops, 1, "a canvas not said");
        kept.end();
        kept.begin();
        kept.end();
        assert!(
            !kept.holds_backdrop(&key),
            "blurred from nothing said, it is not laid aside"
        );
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
    fn a_run_set_alike_is_outlined_once_and_any_other_again() {
        // Queue item 895: by its words, its very face and its numbers.
        let load = |family: &str, bytes: &[u8]| {
            alo_text::Font::load(
                family,
                alo_text::Weight::NORMAL,
                alo_text::Slant::Normal,
                bytes.to_vec(),
            )
            .expect("a face this crate is tested with")
        };
        let sans = load("DejaVu Sans", dejavu::sans::regular());
        let serif = load("DejaVu Serif", dejavu::serif::regular());
        let set = |text, font, size| crate::drawn_text::Set {
            text,
            font,
            size,
            letter_spacing: 0.0,
            origin: (10.0, 20.0),
            oblique: 0.0,
        };
        let at = (3, alo_value::Matrix::IDENTITY, 2.0);
        let mut kept = Kept::new();
        let outlined = |kept: &mut Kept, run: &crate::drawn_text::Set<'_>| {
            kept.begin();
            kept.run(at, run, || vec![Path::rectangle(0.0, 0.0, 1.0, 1.0)]);
            kept.run(at, run, || vec![Path::new()]);
            let made = kept.made().outlined;
            kept.end();
            made
        };
        assert_eq!(outlined(&mut kept, &set("word", &sans, 16.0)), 1);
        assert_eq!(outlined(&mut kept, &set("word", &sans, 16.0)), 0);
        let again = load("DejaVu Sans", dejavu::sans::regular());
        assert_eq!(
            outlined(&mut kept, &set("word", &again, 16.0)),
            1,
            "another face"
        );
        assert_eq!(outlined(&mut kept, &set("word", &serif, 16.0)), 1);
        assert_eq!(outlined(&mut kept, &set("words", &serif, 16.0)), 1);
        assert_eq!(outlined(&mut kept, &set("words", &serif, 16.5)), 1);
        assert_eq!(outlined(&mut kept, &set("words", &serif, 16.5)), 0);
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
