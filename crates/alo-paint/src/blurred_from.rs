/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a backdrop's blur was blurred from: the list drawn before it
//! (queue item 894, ADR 0104 § 2 amended).
//!
//! *Settings* opened over a card blurs the whole screen under its veil.
//! Closed and opened again, nothing under the veil need have changed; but
//! the list before — *Settings* closed — has no veil, so [`crate::damage`]
//! matches nothing to it and its blur would be made again (45 ms at two).
//!
//! A backdrop reads only its crop, as the items drawn before it left it
//! ([`crate::backdrop`]). Those pixels are a function of nothing but those
//! items, in order, drawn onto a canvas of one size made in one colour, at
//! one scale: ADR 0101 § 2's argument, made of a whole list rather than of
//! the pixels two lists share. So a blur is kept with the items drawn before
//! it, and a later drawing whose items before the same backdrop draw alike,
//! **every one, in order** — by the very comparison `damage` matches items
//! by — draws the very pixels there, and the blur kept is its blur, bit for
//! bit.
//!
//! A picture is compared as `damage` compares it, the very same picture
//! (its first draw decides its copy's levels). What is kept of it here is
//! not the picture but a weak hold on it: a blur laid aside keeps no
//! decoded picture alive, and a picture let go is never the same picture as
//! one decoded since, whatever address it is given.

use crate::display::DisplayItem;
use alo_value::Rgba;
use std::sync::{Arc, Weak};

/// The items drawn before a backdrop, and the canvas they were drawn onto.
#[derive(Debug, Clone)]
pub(crate) struct BlurredFrom {
    /// Each item, a picture by a weak hold, the areas a pointer finds boxes
    /// by left out — they draw nothing.
    items: Vec<Before>,
    /// The scale the list was drawn at, bit for bit.
    scale: u32,
    /// The canvas's size.
    size: (u32, u32),
    /// The colour the canvas was made in, each channel bit for bit.
    ground: [u32; 4],
}

/// One item drawn before a backdrop.
#[derive(Debug, Clone)]
enum Before {
    /// Any item but a picture, as it was.
    Item(DisplayItem),
    /// A picture: where, how strongly, and which picture.
    Picture {
        rect: alo_layout::Rect,
        fade: u8,
        picture: Weak<crate::canvas::Canvas>,
    },
}

impl BlurredFrom {
    /// What `items`, drawn at `scale` onto a canvas of `size` made in
    /// `ground`, are kept as.
    pub(crate) fn of(items: &[DisplayItem], scale: f32, size: (u32, u32), ground: Rgba) -> Self {
        let items = drawn(items)
            .map(|item| match item {
                DisplayItem::Picture {
                    rect,
                    picture,
                    fade,
                    ..
                } => Before::Picture {
                    rect: *rect,
                    fade: *fade,
                    picture: Arc::downgrade(picture),
                },
                other => Before::Item(other.clone()),
            })
            .collect();
        Self {
            items,
            scale: scale.to_bits(),
            size,
            ground: bits(ground),
        }
    }

    /// Whether `items`, drawn at `scale` onto a canvas of `size` made in
    /// `ground`, draw alike what these were: the same canvas, and every item
    /// drawing alike the one kept at its place, as many as were kept.
    pub(crate) fn is(
        &self,
        items: &[DisplayItem],
        scale: f32,
        size: (u32, u32),
        ground: Rgba,
    ) -> bool {
        if self.scale != scale.to_bits() || self.size != size || self.ground != bits(ground) {
            return false;
        }
        let mut items = drawn(items);
        for kept in &self.items {
            let Some(item) = items.next() else {
                return false;
            };
            if !alike(kept, item) {
                return false;
            }
        }
        items.next().is_none()
    }

    /// How many items it keeps.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }
}

/// The items of a list that draw: all but the areas a pointer finds boxes
/// by, as `damage` lines lists up without them.
fn drawn(items: &[DisplayItem]) -> impl Iterator<Item = &DisplayItem> {
    items
        .iter()
        .filter(|item| !matches!(item, DisplayItem::Area { .. }))
}

/// Whether `item` draws alike what was kept.
fn alike(kept: &Before, item: &DisplayItem) -> bool {
    match (kept, item) {
        (
            Before::Picture {
                rect,
                fade,
                picture,
            },
            DisplayItem::Picture {
                rect: r,
                picture: p,
                fade: f,
                ..
            },
        ) => rect == r && fade == f && picture.upgrade().is_some_and(|held| Arc::ptr_eq(&held, p)),
        (Before::Item(was), is) => crate::damage::same(was, is),
        (Before::Picture { .. }, _) => false,
    }
}

/// A colour's channels, bit for bit.
fn bits(color: Rgba) -> [u32; 4] {
    [color.red, color.green, color.blue, color.alpha].map(f32::to_bits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fill_rule::FillRule;
    use crate::paint::Paint;
    use crate::path::Path;
    use alo_box::BoxId;

    fn fill(x: f32, color: Rgba) -> DisplayItem {
        DisplayItem::Fill {
            box_id: BoxId::from_index_for_tests(0),
            path: Path::rectangle(x, 0.0, 10.0, 10.0),
            paint: Paint::Solid(color),
            rule: FillRule::NonZero,
        }
    }

    fn picture(picture: &Arc<crate::canvas::Canvas>) -> DisplayItem {
        DisplayItem::Picture {
            box_id: BoxId::from_index_for_tests(1),
            rect: alo_layout::Rect::new(0.0, 0.0, 4.0, 4.0),
            picture: Arc::clone(picture),
            fade: u8::MAX,
        }
    }

    fn area() -> DisplayItem {
        DisplayItem::Area {
            box_id: BoxId::from_index_for_tests(2),
            path: Path::rectangle(0.0, 0.0, 1.0, 1.0),
        }
    }

    #[test]
    fn the_same_items_on_the_same_canvas_are_what_a_blur_was_blurred_from() {
        let shown = Arc::new(crate::canvas::Canvas::new(4, 4, Rgba::BLACK));
        let items = vec![
            fill(0.0, Rgba::WHITE),
            picture(&shown),
            fill(20.0, Rgba::BLACK),
        ];
        let from = BlurredFrom::of(&items, 2.0, (80, 40), Rgba::WHITE);
        assert_eq!(from.len(), 3);
        assert!(from.is(&items, 2.0, (80, 40), Rgba::WHITE));
        // Areas draw nothing, wherever they are.
        let mut with_areas = items.clone();
        with_areas.insert(1, area());
        with_areas.push(area());
        assert!(from.is(&with_areas, 2.0, (80, 40), Rgba::WHITE));
    }

    #[test]
    fn anything_else_is_not() {
        let shown = Arc::new(crate::canvas::Canvas::new(4, 4, Rgba::BLACK));
        let items = vec![fill(0.0, Rgba::WHITE), picture(&shown)];
        let from = BlurredFrom::of(&items, 2.0, (80, 40), Rgba::WHITE);
        assert!(
            !from.is(&items, 1.0, (80, 40), Rgba::WHITE),
            "another scale"
        );
        assert!(!from.is(&items, 2.0, (80, 41), Rgba::WHITE), "another size");
        assert!(
            !from.is(&items, 2.0, (80, 40), Rgba::BLACK),
            "another ground"
        );
        let moved = vec![fill(0.5, Rgba::WHITE), picture(&shown)];
        assert!(
            !from.is(&moved, 2.0, (80, 40), Rgba::WHITE),
            "an item moved"
        );
        let more = vec![
            fill(0.0, Rgba::WHITE),
            picture(&shown),
            fill(9.0, Rgba::WHITE),
        ];
        assert!(!from.is(&more, 2.0, (80, 40), Rgba::WHITE), "one more");
        assert!(
            !from.is(&items[..1], 2.0, (80, 40), Rgba::WHITE),
            "one fewer"
        );
        let swapped = vec![picture(&shown), fill(0.0, Rgba::WHITE)];
        assert!(
            !from.is(&swapped, 2.0, (80, 40), Rgba::WHITE),
            "another order"
        );
        // The same pixels decoded again are another picture.
        let again = Arc::new(crate::canvas::Canvas::new(4, 4, Rgba::BLACK));
        let decoded = vec![fill(0.0, Rgba::WHITE), picture(&again)];
        assert!(
            !from.is(&decoded, 2.0, (80, 40), Rgba::WHITE),
            "another picture"
        );
    }

    #[test]
    fn a_picture_let_go_is_never_the_same_picture_again() {
        let shown = Arc::new(crate::canvas::Canvas::new(4, 4, Rgba::BLACK));
        let items = vec![picture(&shown)];
        let from = BlurredFrom::of(&items, 1.0, (8, 8), Rgba::WHITE);
        assert_eq!(Arc::strong_count(&shown), 2, "the list's, and this one");
        drop(items);
        assert_eq!(Arc::strong_count(&shown), 1, "nothing kept holds it");
        drop(shown);
        let next = Arc::new(crate::canvas::Canvas::new(4, 4, Rgba::BLACK));
        assert!(!from.is(&[picture(&next)], 1.0, (8, 8), Rgba::WHITE));
    }
}
