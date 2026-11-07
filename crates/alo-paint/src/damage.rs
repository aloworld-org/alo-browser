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
//! pushes the other list has none of, are unmatched and counted. The whole
//! canvas is also the answer when the lists are nested otherwise than a
//! stack, when a reach cannot be said, at another scale, when what is left
//! to line up is too long to line up cheaply, and when the damage reaches
//! half the canvas, which costs less to paint whole than to keep finding
//! out.

use crate::display::{DisplayItem, DisplayList};
use crate::extent::{Reach, reach_of};
use crate::glyph_shadow::Cast;
use crate::region::Region;
use alo_value::Matrix;
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
/// own scale, can differ.
pub fn damage(before: &DisplayList, after: &DisplayList, size: (u32, u32)) -> Damage {
    if before.scale().to_bits() != after.scale().to_bits() {
        return Damage::Whole;
    }
    let (Some(old), Some(new)) = (Walked::of(before), Walked::of(after)) else {
        return Damage::Whole;
    };
    let Some((old_matched, new_matched)) = aligned(&old, &new) else {
        return Damage::Whole;
    };
    let mut found = Found::new(size);
    for (walked, matched, scale) in [
        (&old, &old_matched, before.scale()),
        (&new, &new_matched, after.scale()),
    ] {
        for (index, item) in walked.items.iter().enumerate() {
            if matched.get(index).copied().unwrap_or(false) {
                continue;
            }
            match item {
                DisplayItem::PushGroup { .. }
                | DisplayItem::PopGroup
                | DisplayItem::Picture { .. } => return Damage::Whole,
                DisplayItem::PushClip { .. }
                | DisplayItem::PopClip
                | DisplayItem::PushTransform { .. }
                | DisplayItem::PopTransform => {}
                _ => found.add(item, walked.at.get(index).copied(), scale),
            }
            if found.whole {
                return Damage::Whole;
            }
        }
    }
    found.damage()
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

/// The two lists lined up: which items of each are matched to one of the
/// other, in order — all those they start and end with alike, and the most
/// of what is left between ([`MOST_PAIRS`]). [`None`] when what is left is
/// too long to line up.
fn aligned(old: &Walked, new: &Walked) -> Option<(Vec<bool>, Vec<bool>)> {
    let (n, m) = (old.items.len(), new.items.len());
    let alike = |i: usize, j: usize, start: usize| {
        matches!((old.items.get(i), new.items.get(j)), (Some(was), Some(is)) if same(was, is))
            && same_inside(
                old,
                new,
                (
                    old.inside.get(i).copied().flatten(),
                    new.inside.get(j).copied().flatten(),
                ),
                start,
            )
    };
    let mut first = 0;
    while first < n.min(m) && alike(first, first, first) {
        first += 1;
    }
    let mut last = 0;
    while last < n.min(m) - first && alike(n - 1 - last, m - 1 - last, first) {
        last += 1;
    }
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
        return Some((old_matched, new_matched));
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
            let value = if alike(first + row, first + column, first) {
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
        if here == next + 1 && alike(first + row, first + column, first) {
            if let Some(flag) = old_matched.get_mut(first + row) {
                *flag = true;
            }
            if let Some(flag) = new_matched.get_mut(first + column) {
                *flag = true;
            }
            row += 1;
            column += 1;
        } else if best.get((row + 1) * width + column).copied().unwrap_or(0) >= here {
            row += 1;
        } else {
            column += 1;
        }
    }
    Some((old_matched, new_matched))
}

/// Whether two items are drawn inside pushes that draw alike, in the same
/// order: the pushes open around them compared outwards, as far as the
/// first `start` items both lists begin with alike, where two of the same
/// place are the same push inside the same pushes.
fn same_inside(
    old: &Walked,
    new: &Walked,
    (mut was, mut is): (Option<usize>, Option<usize>),
    start: usize,
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
                    return false;
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
}

impl Found {
    fn new(size: (u32, u32)) -> Self {
        Self {
            size,
            region: None,
            whole: false,
        }
    }

    /// What `item` can reach, drawn under `current`, added; a reach that
    /// cannot be said, or a damage of half the canvas or more, is the whole.
    fn add(&mut self, item: &DisplayItem, current: Option<Matrix>, scale: f32) {
        let Some(current) = current else {
            self.whole = true;
            return;
        };
        match reach_of(item, current, scale, self.size) {
            Reach::Nothing => {}
            Reach::Unknown => self.whole = true,
            Reach::Within(region) => {
                let joined = self.region.map_or(region, |was| was.joined(region));
                self.region = Some(joined);
                let canvas = u64::from(self.size.0) * u64::from(self.size.1);
                if joined.area().saturating_mul(2) >= canvas {
                    self.whole = true;
                }
            }
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
        assert_eq!(damage(&one(), &one(), (200, 100)), Damage::None);
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
            damage(&before, &after, (200, 100)),
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
            damage(&before, &after, (200, 100)),
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
            damage(&before, &after, (200, 100)),
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
            damage(&before, &after, (200, 100)),
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
            damage(&before, &crossed, (200, 100)),
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
            damage(&inside(clip()), &inside(narrower), (200, 100)),
            Damage::Within(Region {
                left: 0,
                top: 9,
                right: 6,
                bottom: 21
            }),
            "the fill inside is drawn inside another clip; the one after it is not",
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
            damage(&grouped(0.5), &grouped(0.25), (200, 100)),
            Damage::Whole
        );
        assert_eq!(
            damage(&grouped(0.5), &grouped(0.5), (200, 100)),
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
                (8, 8)
            ),
            Damage::Whole,
            "equal pixels in another picture are another picture's first draw",
        );
    }

    #[test]
    fn another_scale_or_most_of_the_canvas_is_the_whole_canvas() {
        let one = list(vec![fill(0.0, 5.0, Rgba::BLACK)]);
        let two = DisplayList::from_parts(Vec::new(), Vec::new(), 2.0);
        assert_eq!(damage(&one, &two, (200, 100)), Damage::Whole);
        let wide = list(vec![fill(0.0, 200.0, Rgba::BLACK)]);
        let tall = list(vec![DisplayItem::Fill {
            box_id: BoxId::from_index_for_tests(0),
            path: Path::rectangle(0.0, 0.0, 200.0, 100.0),
            paint: Paint::Solid(Rgba::BLACK),
            rule: FillRule::NonZero,
        }]);
        assert_eq!(damage(&wide, &tall, (200, 100)), Damage::Whole);
    }
}
