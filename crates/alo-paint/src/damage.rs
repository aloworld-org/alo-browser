/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which pixels two display lists can draw differently (queue item 870,
//! ADR 0101).
//!
//! A canvas holding `before` drawn is made `after` drawn by painting again
//! only where they differ ([`crate::repaint()`]). This file says where that
//! is, and it is the part that has to be right: a pixel it leaves out keeps
//! `before`'s colour where `after` would have drawn another.
//!
//! # Why the answer is right
//!
//! [`crate::render()`] draws a list in order. What each item does to a pixel
//! depends only on the item, on the clips, transforms and groups open
//! around it, and on what that pixel already held — never on another pixel
//! — and, for a picture, on the pictures drawn before it (a copy's mipmap
//! levels are decided by its first draw, [`crate::picture_filter`]).
//!
//! So the two lists, less the areas a pointer finds boxes by, are lined up
//! in order (`aligned`): an item of one is matched to an item of the
//! other only when the two draw alike (`same`) **inside pushes that draw
//! alike, in the same order** — the same clips under the same transforms
//! into the same groups. Every item left unmatched is counted where it can
//! reach ([`crate::extent`]). Take a pixel none of them can reach. The items
//! that touch it, in either list, are matched items, in the same order,
//! each drawn the same way into a canvas holding the same colour there, so
//! it ends the same. That needs two more things, and without them the
//! answer is the whole canvas:
//!
//! - **Every group pushed or popped is matched**, so the same groups are
//!   drawn back in the same order, each holding the same colour there.
//! - **Every picture is matched**, the very same picture, so the same
//!   pictures are first drawn in the same order.
//!
//! A clip or a transform pushed or popped unmatched draws nothing itself;
//! what it changes is how the items inside it are drawn, and those, inside
//! pushes the other list has none of, are unmatched and counted.
//!
//! # Inside a clip that differs
//!
//! An item may also be matched inside pushes that draw alike **but for
//! clips** (queue item 871): the same transforms and groups in the same
//! order, and in some places one clip's shape where the other list has
//! another's — the conversation's clip ending higher once the offers open
//! under the box, around a card that is drawn exactly as it was. A clip
//! changes an item's pixels only through its mask at each pixel
//! ([`crate::clip`]): `render` multiplies the item's coverage by every
//! clip's mask at that pixel and by nothing else of the clip. So where each
//! pair of clips that differ has equal masks, the item draws alike, and it
//! is counted only where it can reach *and* some pair's masks differ
//! ([`crate::clip::differ`], measured on the very masks `render` makes).
//! A group or a picture matched so is the whole canvas, since neither is
//! drawn only through a mask. The whole
//! canvas is also the answer when the lists are nested otherwise than a
//! stack, when a reach cannot be said, at another scale, when what is left
//! to line up is too long to line up cheaply, and when the damage reaches
//! half the canvas, which costs less to paint whole than to keep finding
//! out.

use crate::clip::{differ, mask};
use crate::display::{DisplayItem, DisplayList};
use crate::extent::{Reach, reach_of};
use crate::glyph_shadow::Cast;
use crate::kept::{Kept, Key, Made};
use crate::region::Region;
use alo_value::Matrix;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The most pairs of items the middle of two lists is lined up over: two
/// runs of five hundred items, which no edit of the first screen comes near.
const MOST_PAIRS: usize = 1 << 18;

/// The deepest two stacks of pushes are compared: deeper ones are taken to
/// differ, which costs a repaint and never a wrong pixel.
const DEEPEST: usize = 64;

/// What has to be painted again to make a canvas holding one list drawn
/// hold another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Damage {
    /// Nothing: the two draw every pixel alike.
    None,
    /// The pixels of this region, and no others.
    Within(Region),
    /// Every pixel.
    Whole,
}

/// Where `before` and `after`, each drawn onto a canvas of `size` at its
/// own scale, can differ. The masks of clips that differ are taken from
/// `kept`, and kept in it, as `render` takes them ([`crate::kept`]).
pub fn damage(
    before: &DisplayList,
    after: &DisplayList,
    size: (u32, u32),
    kept: &mut Kept,
) -> Damage {
    damage_standing(before, after, size, kept).0
}

/// [`damage`], and the backdrops of `after` that stand (queue item 885):
/// each matched to one of `before`, nothing drawn before either changed
/// any pixel of its crop, and its blur kept in `kept` — so the blur kept is
/// this drawing's, every bit, and a repaint takes it rather than reading a
/// crop it does not paint whole. Every other backdrop's crop is damaged
/// whole, since its blur reads all of it.
pub(crate) fn damage_standing(
    before: &DisplayList,
    after: &DisplayList,
    size: (u32, u32),
    kept: &mut Kept,
) -> (Damage, HashSet<Key>) {
    let damage = |found: Damage| (found, HashSet::new());
    if before.scale().to_bits() != after.scale().to_bits() {
        return damage(Damage::Whole);
    }
    let (Some(old), Some(new)) = (Walked::of(before), Walked::of(after)) else {
        return damage(Damage::Whole);
    };
    let Some(lined) = aligned(&old, &new) else {
        return damage(Damage::Whole);
    };
    let (old_matched, new_matched) = (&lined.old, &lined.new);
    let mut found = Found::new(size);
    for (walked, matched, scale, was) in [
        (&old, old_matched, before.scale(), true),
        (&new, new_matched, after.scale(), false),
    ] {
        for (index, item) in walked.items.iter().enumerate() {
            if matched.get(index).copied().unwrap_or(false) {
                continue;
            }
            match item {
                DisplayItem::PushGroup { .. }
                | DisplayItem::PopGroup
                | DisplayItem::Picture { .. } => return damage(Damage::Whole),
                DisplayItem::PushClip { .. }
                | DisplayItem::PopClip
                | DisplayItem::PushTransform { .. }
                | DisplayItem::PopTransform => {}
                _ => found.add((was, index), item, walked.at.get(index).copied(), scale),
            }
            if found.whole {
                return damage(Damage::Whole);
            }
        }
    }
    // What is matched inside clips that differ, where their masks do.
    let mut masks = Masks {
        old: &old,
        new: &new,
        size,
        kept,
        differing: HashMap::new(),
    };
    let mut clips = Vec::new();
    for &(i, j) in &lined.pairs {
        clips.clear();
        let context = (
            old.inside.get(i).copied().flatten(),
            new.inside.get(j).copied().flatten(),
        );
        if !inside_alike(&old, &new, context, lined.first, &mut clips) {
            return damage(Damage::Whole);
        }
        if clips.is_empty() {
            continue;
        }
        let Some(&item) = new.items.get(j) else {
            return damage(Damage::Whole);
        };
        match item {
            DisplayItem::PushGroup { .. } | DisplayItem::PopGroup | DisplayItem::Picture { .. } => {
                return damage(Damage::Whole);
            }
            DisplayItem::PushClip { .. }
            | DisplayItem::PopClip
            | DisplayItem::PushTransform { .. }
            | DisplayItem::PopTransform => continue,
            _ => {}
        }
        let mut within: Option<Region> = None;
        for &pair in &clips {
            if let Some(region) = masks.differ(pair) {
                within = Some(within.map_or(region, |was| was.joined(region)));
            }
        }
        if let Some(within) = within {
            found.add_within(
                (false, j),
                item,
                new.at.get(j).copied(),
                after.scale(),
                Some(within),
            );
            if found.whole {
                return damage(Damage::Whole);
            }
        }
    }
    let standing = stood(&old, &new, &lined, &mut found, kept);
    if found.whole {
        return damage(Damage::Whole);
    }
    (found.damage(), standing)
}

/// Which backdrops of `new` stand, and every crop of one that does not
/// added to what `found` holds (queue item 885), taken in the order they are
/// drawn: a backdrop whose blur changes changes its whole crop, which is a
/// change drawn before every backdrop after it.
fn stood(
    old: &Walked,
    new: &Walked,
    lined: &Lined,
    found: &mut Found,
    kept: &Kept,
) -> HashSet<Key> {
    let mut pairs: Vec<(usize, usize)> = (0..lined.first)
        .map(|index| (index, index))
        .chain(lined.pairs.iter().copied())
        .filter(|&(_, j)| matches!(new.items.get(j), Some(DisplayItem::Backdrop { .. })))
        .collect();
    pairs.sort_by_key(|&(_, j)| j);
    let keys_of = |walked: &Walked| -> Vec<Key> {
        walked
            .items
            .iter()
            .zip(&walked.at)
            .filter_map(|(item, at)| backdrop_key(item, *at, found.size).map(|(_, key)| key))
            .collect()
    };
    let (old_keys, new_keys) = (keys_of(old), keys_of(new));
    let twice = |keys: &[Key], key: &Key| keys.iter().filter(|each| *each == key).count() > 1;
    let mut standing = HashSet::new();
    for (i, j) in pairs {
        let (Some(item), Some(at)) = (new.items.get(j), new.at.get(j)) else {
            continue;
        };
        let Some((crop, key)) = backdrop_key(item, *at, found.size) else {
            continue;
        };
        let changed = found.log.iter().any(|&((before, index), region)| {
            (if before { index < i } else { index < j }) && region.meeting(crop).is_some()
        });
        // A key held by two backdrops of a list stands for neither: what is
        // kept under it is one crop's blur.
        if changed || !kept.holds_backdrop(&key) || twice(&old_keys, &key) || twice(&new_keys, &key)
        {
            found.add_region((false, j), crop);
        } else {
            standing.insert(key);
        }
    }
    standing
}

/// A backdrop's crop and the key its blur is kept under, drawn under `at`
/// on a canvas of `size`; [`None`] for any other item, or one off the
/// canvas.
fn backdrop_key(item: &DisplayItem, at: Matrix, size: (u32, u32)) -> Option<(Region, Key)> {
    let DisplayItem::Backdrop { rect, blur, .. } = item else {
        return None;
    };
    let crop = crate::extent::crop_under(*rect, at, size)?;
    Some((crop, crate::backdrop::key(crop, blur * at.scale_factor())))
}

/// Where pairs of clips, one of each list, let different amounts through:
/// each pair measured once, on the masks `render` makes for them.
struct Masks<'a, 'b> {
    old: &'a Walked<'b>,
    new: &'a Walked<'b>,
    size: (u32, u32),
    kept: &'a mut Kept,
    differing: HashMap<(usize, usize), Option<Region>>,
}

impl Masks<'_, '_> {
    fn differ(&mut self, (i, j): (usize, usize)) -> Option<Region> {
        if let Some(&found) = self.differing.get(&(i, j)) {
            return found;
        }
        let found = match (self.mask(true, i), self.mask(false, j)) {
            (Some(one), Some(other)) => differ(&one, &other, self.size),
            // Not a clip after all: every pixel may differ.
            _ => Region::whole(self.size.0, self.size.1),
        };
        self.differing.insert((i, j), found);
        found
    }

    /// The mask of the clip pushed at `index` of the list before (`old`) or
    /// after, under the transform in force there, as `render` makes it.
    fn mask(&mut self, old: bool, index: usize) -> Option<Arc<crate::Coverage>> {
        let walked = if old { self.old } else { self.new };
        let DisplayItem::PushClip { path, .. } = walked.items.get(index)? else {
            return None;
        };
        let shape = crate::render::moved(path, *walked.at.get(index)?);
        let key = Key::of(Made::Mask, &[], &shape);
        Some(self.kept.coverage(&key, Made::Mask, || mask(&shape)))
    }
}

/// A list's drawing items with, for each, the transform it is drawn under
/// and the push it is drawn inside, as [`crate::render()`] keeps them.
struct Walked<'a> {
    items: Vec<&'a DisplayItem>,
    /// The transform each item is drawn under, the screen's growth outside
    /// every other.
    at: Vec<Matrix>,
    /// The innermost push still open around each item, of any kind, by its
    /// place in `items`: pushes nest as a stack, so this and the pushes
    /// open around it say every clip, transform and group it is drawn in.
    inside: Vec<Option<usize>>,
}

impl<'a> Walked<'a> {
    /// The list walked, or [`None`] when its pushes and pops do not nest as
    /// one stack — a pop of another kind than the last push still open, or
    /// of nothing — since `render` keeps a stack for each kind and such a
    /// list is drawn in a way this walk does not follow.
    fn of(list: &'a DisplayList) -> Option<Self> {
        let items: Vec<&DisplayItem> = list
            .items()
            .iter()
            .filter(|item| !matches!(item, DisplayItem::Area { .. }))
            .collect();
        let screen = Matrix::scaling(list.scale());
        let mut open: Vec<(usize, Matrix)> = Vec::new();
        let mut at = Vec::with_capacity(items.len());
        let mut inside = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            let current = open
                .iter()
                .rev()
                .find_map(|(push, after)| {
                    matches!(items.get(*push), Some(DisplayItem::PushTransform { .. }))
                        .then_some(*after)
                })
                .unwrap_or(screen);
            at.push(current);
            inside.push(open.last().map(|(push, _)| *push));
            match item {
                DisplayItem::PushTransform { matrix, .. } => {
                    open.push((index, matrix.then(current)));
                }
                DisplayItem::PushClip { .. } | DisplayItem::PushGroup { .. } => {
                    open.push((index, current));
                }
                DisplayItem::PopClip | DisplayItem::PopTransform | DisplayItem::PopGroup => {
                    let (push, _) = open.pop()?;
                    if !pops(items.get(push)?, item) {
                        return None;
                    }
                }
                _ => {}
            }
        }
        Some(Self { items, at, inside })
    }
}

/// Whether `pop` is the pop of `push`'s kind.
fn pops(push: &DisplayItem, pop: &DisplayItem) -> bool {
    matches!(
        (push, pop),
        (DisplayItem::PushClip { .. }, DisplayItem::PopClip)
            | (DisplayItem::PushTransform { .. }, DisplayItem::PopTransform)
            | (DisplayItem::PushGroup { .. }, DisplayItem::PopGroup)
    )
}

/// Two lists lined up.
struct Lined {
    /// Which items of the list before are matched.
    old: Vec<bool>,
    /// Which items of the list after are.
    new: Vec<bool>,
    /// Each pair matched, past the items both lists begin with alike.
    pairs: Vec<(usize, usize)>,
    /// How many items both begin with alike, inside the very same pushes.
    first: usize,
}

/// The two lists lined up: which items of each are matched to one of the
/// other, in order — all those they start with alike, inside the very same
/// pushes, then those they end with and the most of what is left between
/// ([`MOST_PAIRS`]), inside pushes alike but for clips. [`None`] when what
/// is left is too long to line up.
fn aligned(old: &Walked, new: &Walked) -> Option<Lined> {
    let (n, m) = (old.items.len(), new.items.len());
    let mut scratch = Vec::new();
    let mut alike = |i: usize, j: usize, start: usize, exactly: bool| {
        if !matches!((old.items.get(i), new.items.get(j)), (Some(was), Some(is)) if same(was, is)) {
            return false;
        }
        scratch.clear();
        inside_alike(
            old,
            new,
            (
                old.inside.get(i).copied().flatten(),
                new.inside.get(j).copied().flatten(),
            ),
            start,
            &mut scratch,
        ) && (!exactly || scratch.is_empty())
    };
    let mut first = 0;
    while first < n.min(m) && alike(first, first, first, true) {
        first += 1;
    }
    let mut last = 0;
    while last < n.min(m) - first && alike(n - 1 - last, m - 1 - last, first, false) {
        last += 1;
    }
    let mut pairs: Vec<(usize, usize)> = (0..last).map(|k| (n - 1 - k, m - 1 - k)).collect();
    let mut old_matched = vec![false; n];
    let mut new_matched = vec![false; m];
    for index in (0..first).chain(n - last..n) {
        if let Some(flag) = old_matched.get_mut(index) {
            *flag = true;
        }
    }
    for index in (0..first).chain(m - last..m) {
        if let Some(flag) = new_matched.get_mut(index) {
            *flag = true;
        }
    }
    let (rows, columns) = (n - last - first, m - last - first);
    if rows == 0 || columns == 0 {
        return Some(Lined {
            old: old_matched,
            new: new_matched,
            pairs,
            first,
        });
    }
    if rows.saturating_mul(columns) > MOST_PAIRS {
        return None;
    }
    // The longest run of items alike in order, by the classic table: `best`
    // at `(row, column)` is how many of the items from there on line up.
    let width = columns + 1;
    let mut best = vec![0_u32; (rows + 1) * width];
    for row in (0..rows).rev() {
        for column in (0..columns).rev() {
            let value = if alike(first + row, first + column, first, false) {
                best.get((row + 1) * width + column + 1)
                    .copied()
                    .unwrap_or(0)
                    + 1
            } else {
                let down = best.get((row + 1) * width + column).copied().unwrap_or(0);
                let across = best.get(row * width + column + 1).copied().unwrap_or(0);
                down.max(across)
            };
            if let Some(cell) = best.get_mut(row * width + column) {
                *cell = value;
            }
        }
    }
    let (mut row, mut column) = (0, 0);
    while row < rows && column < columns {
        let here = best.get(row * width + column).copied().unwrap_or(0);
        let next = best
            .get((row + 1) * width + column + 1)
            .copied()
            .unwrap_or(0);
        if here == next + 1 && alike(first + row, first + column, first, false) {
            if let Some(flag) = old_matched.get_mut(first + row) {
                *flag = true;
            }
            if let Some(flag) = new_matched.get_mut(first + column) {
                *flag = true;
            }
            pairs.push((first + row, first + column));
            row += 1;
            column += 1;
        } else if best.get((row + 1) * width + column).copied().unwrap_or(0) >= here {
            row += 1;
        } else {
            column += 1;
        }
    }
    Some(Lined {
        old: old_matched,
        new: new_matched,
        pairs,
        first,
    })
}

/// Whether two items are drawn inside pushes that draw alike but for clips,
/// in the same order: the pushes open around them compared outwards, as
/// far as the first `start` items both lists begin with alike, where two of
/// the same place are the same push inside the same pushes. Each pair of
/// clips that differ, one pushed at `i` before and one at `j` after, is put
/// in `clips`; none are when the pushes draw alike.
fn inside_alike(
    old: &Walked,
    new: &Walked,
    (mut was, mut is): (Option<usize>, Option<usize>),
    start: usize,
    clips: &mut Vec<(usize, usize)>,
) -> bool {
    for _ in 0..DEEPEST {
        match (was, is) {
            (None, None) => return true,
            (Some(i), Some(j)) => {
                if i == j && i < start {
                    return true;
                }
                let (Some(push), Some(other)) = (old.items.get(i), new.items.get(j)) else {
                    return false;
                };
                if !same(push, other) {
                    if !matches!(
                        (push, other),
                        (DisplayItem::PushClip { .. }, DisplayItem::PushClip { .. })
                    ) {
                        return false;
                    }
                    clips.push((i, j));
                }
                was = old.inside.get(i).copied().flatten();
                is = new.inside.get(j).copied().flatten();
            }
            _ => return false,
        }
    }
    false
}

/// The damage found so far.
struct Found {
    size: (u32, u32),
    region: Option<Region>,
    whole: bool,
    /// Each region added, and the item it was added for: whether of the
    /// list before, and its place there — so that a backdrop can be asked
    /// whether anything drawn before it changed under it (queue item 885).
    log: Vec<((bool, usize), Region)>,
}

impl Found {
    fn new(size: (u32, u32)) -> Self {
        Self {
            size,
            region: None,
            whole: false,
            log: Vec::new(),
        }
    }

    /// What `item`, at `place` — of the list before or not, and where in
    /// it — can reach, drawn under `current`, added; a reach that cannot be
    /// said, or a damage of half the canvas or more, is the whole.
    fn add(
        &mut self,
        place: (bool, usize),
        item: &DisplayItem,
        current: Option<Matrix>,
        scale: f32,
    ) {
        self.add_within(place, item, current, scale, None);
    }

    /// The same, of only what is inside `limit` when there is one.
    fn add_within(
        &mut self,
        place: (bool, usize),
        item: &DisplayItem,
        current: Option<Matrix>,
        scale: f32,
        limit: Option<Region>,
    ) {
        let Some(current) = current else {
            self.whole = true;
            return;
        };
        let reach = match (reach_of(item, current, scale, self.size), limit) {
            (Reach::Within(region), Some(limit)) => {
                region.meeting(limit).map_or(Reach::Nothing, Reach::Within)
            }
            (Reach::Unknown, Some(limit)) => Reach::Within(limit),
            (reach, _) => reach,
        };
        match reach {
            Reach::Nothing => {}
            Reach::Unknown => self.whole = true,
            Reach::Within(region) => self.add_region(place, region),
        }
    }

    /// `region` added, for the item at `place`.
    fn add_region(&mut self, place: (bool, usize), region: Region) {
        self.log.push((place, region));
        let joined = self.region.map_or(region, |was| was.joined(region));
        self.region = Some(joined);
        let canvas = u64::from(self.size.0) * u64::from(self.size.1);
        if joined.area().saturating_mul(2) >= canvas {
            self.whole = true;
        }
    }

    fn damage(&self) -> Damage {
        match (self.whole, self.region) {
            (true, _) => Damage::Whole,
            (false, Some(region)) => Damage::Within(region),
            (false, None) => Damage::None,
        }
    }
}

/// Whether two items draw alike wherever they are drawn alike: every field
/// that reaches a pixel the same — a number the very same number, bit for
/// bit — a picture the very same picture (since its first draw decides its
/// copy's levels), and a face the very same face.
fn same(was: &DisplayItem, is: &DisplayItem) -> bool {
    use DisplayItem as D;
    match (was, is) {
        (
            D::Fill {
                path, paint, rule, ..
            },
            D::Fill {
                path: p,
                paint: q,
                rule: r,
                ..
            },
        ) => path == p && paint == q && rule == r,
        (
            D::Shadow {
                path, blur, color, ..
            },
            D::Shadow {
                path: p,
                blur: b,
                color: c,
                ..
            },
        ) => path == p && exactly(*blur, *b) && color == c,
        (
            D::LettersShadow {
                glyphs,
                blur,
                color,
                ..
            },
            D::LettersShadow {
                glyphs: g,
                blur: b,
                color: c,
                ..
            },
        ) => glyphs == g && exactly(*blur, *b) && color == c,
        (
            D::GlyphShadow {
                cast, blur, color, ..
            },
            D::GlyphShadow {
                cast: k,
                blur: b,
                color: c,
                ..
            },
        ) => same_cast(cast, k) && exactly(*blur, *b) && color == c,
        (
            D::PaintedGlyph {
                steps,
                pen,
                to_css,
                fade,
                ..
            },
            D::PaintedGlyph {
                steps: s,
                pen: p,
                to_css: t,
                fade: f,
                ..
            },
        ) => same_program((steps, *pen, *to_css), (s, *p, *t)) && fade == f,
        (
            D::LayeredGlyph { layers, fade, .. },
            D::LayeredGlyph {
                layers: l, fade: f, ..
            },
        ) => layers == l && fade == f,
        (
            D::Picture {
                rect,
                picture,
                fade,
                ..
            },
            D::Picture {
                rect: r,
                picture: p,
                fade: f,
                ..
            },
        ) => rect == r && Arc::ptr_eq(picture, p) && fade == f,
        (D::Beneath { rect, .. }, D::Beneath { rect: r, .. }) => rect == r,
        (D::Backdrop { .. }, D::Backdrop { .. }) => same_backdrop(was, is),
        (D::Text { .. }, D::Text { .. }) => same_text(was, is),
        (D::PushClip { path, .. }, D::PushClip { path: p, .. }) => path == p,
        (D::PushTransform { matrix, .. }, D::PushTransform { matrix: m, .. }) => matrix == m,
        (D::PushGroup { opacity, .. }, D::PushGroup { opacity: o, .. }) => exactly(*opacity, *o),
        (D::PopClip, D::PopClip)
        | (D::PopTransform, D::PopTransform)
        | (D::PopGroup, D::PopGroup) => true,
        _ => false,
    }
}

/// Whether two backdrops draw alike (queue item 885): the same crop, shape
/// and blur — and so, where nothing under them changed, the same pixels.
fn same_backdrop(was: &DisplayItem, is: &DisplayItem) -> bool {
    match (was, is) {
        (
            DisplayItem::Backdrop {
                rect, path, blur, ..
            },
            DisplayItem::Backdrop {
                rect: r,
                path: p,
                blur: b,
                ..
            },
        ) => rect == r && path == p && exactly(*blur, *b),
        _ => false,
    }
}

/// Whether two runs of text draw alike: the same letters from the same pen
/// in the very same face, set and coloured alike, casting the same shadows.
fn same_text(was: &DisplayItem, is: &DisplayItem) -> bool {
    let (
        DisplayItem::Text {
            text,
            origin,
            font,
            oblique,
            size,
            letter_spacing,
            color,
            shadows,
            ..
        },
        DisplayItem::Text {
            text: other,
            origin: from,
            font: face,
            oblique: leaning,
            size: sized,
            letter_spacing: spacing,
            color: coloured,
            shadows: cast,
            ..
        },
    ) = (was, is)
    else {
        return false;
    };
    text == other
        && exactly(origin.0, from.0)
        && exactly(origin.1, from.1)
        && font.is(face)
        && exactly(*oblique, *leaning)
        && exactly(*size, *sized)
        && exactly(*letter_spacing, *spacing)
        && color == coloured
        && shadows == cast
}

/// Whether two colour glyphs cast alike.
fn same_cast(was: &Cast, is: &Cast) -> bool {
    match (was, is) {
        (Cast::Picture(rect, pixels), Cast::Picture(r, p)) => {
            rect == r && (Arc::ptr_eq(pixels, p) || pixels == p)
        }
        (Cast::Layers(layers), Cast::Layers(l)) => layers == l,
        (Cast::Painted(steps, pen, to_css), Cast::Painted(s, p, t)) => {
            same_program((steps, *pen, *to_css), (s, *p, *t))
        }
        _ => false,
    }
}

/// A colour glyph's paint program, its pen, and CSS pixels to a font unit.
type Program<'a> = (&'a Arc<Vec<crate::colour_paint::Step>>, (f32, f32), f32);

/// Whether two colour glyphs' programs draw alike.
fn same_program((steps, pen, to_css): Program, (other, from, unit): Program) -> bool {
    (Arc::ptr_eq(steps, other) || steps == other)
        && exactly(pen.0, from.0)
        && exactly(pen.1, from.1)
        && exactly(to_css, unit)
}

/// The very same number, bit for bit: what draws alike, where a nearly
/// equal one may not.
fn exactly(one: f32, other: f32) -> bool {
    one.to_bits() == other.to_bits()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fill_rule::FillRule;
    use crate::paint::Paint;
    use crate::path::Path;
    use alo_box::BoxId;
    use alo_value::Rgba;

    fn fill(x: f32, width: f32, color: Rgba) -> DisplayItem {
        DisplayItem::Fill {
            box_id: BoxId::from_index_for_tests(0),
            path: Path::rectangle(x, 10.0, width, 10.0),
            paint: Paint::Solid(color),
            rule: FillRule::NonZero,
        }
    }

    fn list(items: Vec<DisplayItem>) -> DisplayList {
        let mut list = DisplayList::default();
        for item in items {
            list.push(item);
        }
        list
    }

    fn clip() -> DisplayItem {
        DisplayItem::PushClip {
            box_id: BoxId::from_index_for_tests(0),
            path: Path::rectangle(0.0, 0.0, 50.0, 50.0),
        }
    }

    #[test]
    fn the_same_list_differs_nowhere() {
        let one = || {
            list(vec![
                fill(0.0, 100.0, Rgba::WHITE),
                fill(10.0, 5.0, Rgba::BLACK),
            ])
        };
        assert_eq!(
            damage(&one(), &one(), (200, 100), &mut Kept::new()),
            Damage::None
        );
    }

    #[test]
    fn a_changed_item_damages_where_it_was_and_where_it_is() {
        let before = list(vec![
            fill(0.0, 200.0, Rgba::WHITE),
            fill(10.0, 5.0, Rgba::BLACK),
        ]);
        let after = list(vec![
            fill(0.0, 200.0, Rgba::WHITE),
            fill(30.0, 5.0, Rgba::BLACK),
        ]);
        assert_eq!(
            damage(&before, &after, (200, 100), &mut Kept::new()),
            Damage::Within(Region {
                left: 9,
                top: 9,
                right: 36,
                bottom: 21
            })
        );
    }

    #[test]
    fn an_item_added_after_a_clip_damages_only_itself() {
        let before = list(vec![
            clip(),
            fill(0.0, 5.0, Rgba::BLACK),
            DisplayItem::PopClip,
        ]);
        let after = list(vec![
            clip(),
            fill(0.0, 5.0, Rgba::BLACK),
            fill(10.0, 5.0, Rgba::BLACK),
            DisplayItem::PopClip,
        ]);
        assert_eq!(
            damage(&before, &after, (200, 100), &mut Kept::new()),
            Damage::Within(Region {
                left: 9,
                top: 9,
                right: 16,
                bottom: 21
            })
        );
    }

    #[test]
    fn a_run_put_in_before_and_a_change_after_damage_only_themselves() {
        let at = |x: f32| fill(x, 5.0, Rgba::BLACK);
        let before = list(vec![
            at(0.0),
            at(100.0),
            at(110.0),
            at(120.0),
            at(130.0),
            at(20.0),
        ]);
        let after = list(vec![
            at(0.0),
            clip(),
            at(40.0),
            DisplayItem::PopClip,
            at(100.0),
            at(110.0),
            at(120.0),
            at(130.0),
            at(30.0),
        ]);
        assert_eq!(
            damage(&before, &after, (200, 100), &mut Kept::new()),
            Damage::Within(Region {
                left: 19,
                top: 9,
                right: 46,
                bottom: 21
            })
        );
    }

    #[test]
    fn a_clipped_run_put_in_damages_only_what_it_draws() {
        let before = list(vec![fill(0.0, 200.0, Rgba::WHITE)]);
        let after = list(vec![
            fill(0.0, 200.0, Rgba::WHITE),
            clip(),
            fill(10.0, 5.0, Rgba::BLACK),
            DisplayItem::PopClip,
        ]);
        assert_eq!(
            damage(&before, &after, (200, 100), &mut Kept::new()),
            Damage::Within(Region {
                left: 9,
                top: 9,
                right: 16,
                bottom: 21
            })
        );
        let crossed = list(vec![
            fill(0.0, 200.0, Rgba::WHITE),
            clip(),
            DisplayItem::PushTransform {
                box_id: BoxId::from_index_for_tests(0),
                matrix: Matrix::scaling(2.0),
            },
            DisplayItem::PopClip,
            DisplayItem::PopTransform,
        ]);
        assert_eq!(
            damage(&before, &crossed, (200, 100), &mut Kept::new()),
            Damage::Whole,
            "pushes that do not nest as a stack are drawn otherwise than walked",
        );
    }

    #[test]
    fn a_clip_that_differs_damages_what_is_drawn_inside_it() {
        let narrower = DisplayItem::PushClip {
            box_id: BoxId::from_index_for_tests(0),
            path: Path::rectangle(0.0, 0.0, 2.0, 50.0),
        };
        let inside = |push| {
            list(vec![
                fill(0.0, 200.0, Rgba::WHITE),
                push,
                fill(0.0, 5.0, Rgba::BLACK),
                DisplayItem::PopClip,
                fill(100.0, 5.0, Rgba::BLACK),
            ])
        };
        assert_eq!(
            damage(
                &inside(clip()),
                &inside(narrower),
                (200, 100),
                &mut Kept::new()
            ),
            Damage::Within(Region {
                left: 2,
                top: 9,
                right: 6,
                bottom: 21
            }),
            "the fill inside is drawn inside another clip, and differs only where that \
             clip's mask does; the one after it is not",
        );
    }

    #[test]
    fn a_group_or_a_picture_that_differs_is_the_whole_canvas() {
        let group = |opacity| DisplayItem::PushGroup {
            box_id: BoxId::from_index_for_tests(0),
            opacity,
        };
        let grouped = |opacity| {
            list(vec![
                group(opacity),
                fill(0.0, 5.0, Rgba::BLACK),
                DisplayItem::PopGroup,
            ])
        };
        assert_eq!(
            damage(&grouped(0.5), &grouped(0.25), (200, 100), &mut Kept::new()),
            Damage::Whole
        );
        assert_eq!(
            damage(&grouped(0.5), &grouped(0.5), (200, 100), &mut Kept::new()),
            Damage::None
        );
        let picture = |pixels: Arc<crate::Canvas>| DisplayItem::Picture {
            box_id: BoxId::from_index_for_tests(0),
            rect: alo_layout::Rect::new(0.0, 0.0, 2.0, 2.0),
            picture: pixels,
            fade: u8::MAX,
        };
        let pixels = || Arc::new(crate::Canvas::new(2, 2, Rgba::BLACK));
        assert_eq!(
            damage(
                &list(vec![picture(pixels())]),
                &list(vec![picture(pixels())]),
                (8, 8),
                &mut Kept::new()
            ),
            Damage::Whole,
            "equal pixels in another picture are another picture's first draw",
        );
    }

    #[test]
    fn another_scale_or_most_of_the_canvas_is_the_whole_canvas() {
        let one = list(vec![fill(0.0, 5.0, Rgba::BLACK)]);
        let two = DisplayList::from_parts(Vec::new(), Vec::new(), 2.0);
        assert_eq!(
            damage(&one, &two, (200, 100), &mut Kept::new()),
            Damage::Whole
        );
        let wide = list(vec![fill(0.0, 200.0, Rgba::BLACK)]);
        let tall = list(vec![DisplayItem::Fill {
            box_id: BoxId::from_index_for_tests(0),
            path: Path::rectangle(0.0, 0.0, 200.0, 100.0),
            paint: Paint::Solid(Rgba::BLACK),
            rule: FillRule::NonZero,
        }]);
        assert_eq!(
            damage(&wide, &tall, (200, 100), &mut Kept::new()),
            Damage::Whole
        );
    }
}
