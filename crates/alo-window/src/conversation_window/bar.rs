/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The conversation's window and its bar pressed and dragged (queue item
//! 845, [`crate::conversation_bar_press`]): a press in the bar's strip is
//! the window's own, decided here against the bar the window itself
//! computes and sends to be drawn — never handed to the renderer to claim —
//! and the pointer moved with the button down after one on the thumb
//! scrolls the conversation with it until the button comes up. A press
//! held beside the thumb pages on, as a Mac's scroll bar does, until the
//! thumb reaches the pointer or the button comes up (queue item 876).

use super::ConversationWindow;
use crate::conversation_bar_press::{self, OnTheBar};
use crate::conversation_keys::{self, Scroll};
use crate::keys::{Key, Press};
use alo_renderer::split_place::Stands;
use std::time::Instant;

/// The bar held: its thumb dragged, or a press beside it paging on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum BarDrag {
    /// A drag of the bar's thumb under way: where the pointer went down,
    /// down the window in device pixels, and how far the conversation was
    /// scrolled then.
    Thumb {
        /// The pointer's height as it was pressed, in device pixels.
        from_y: f64,
        /// How far the conversation was scrolled as it was pressed.
        from_down: f32,
    },
    /// A press held above or below the thumb (queue item 876): which, where
    /// the pointer is down the first screen now, and when it next pages.
    Paging {
        /// Above the thumb or below it.
        beside: OnTheBar,
        /// The pointer's height on the first screen, in its CSS pixels, and
        /// down the window in device pixels, as it was pressed.
        pressed: (f32, f64),
        /// The pointer's height on the first screen now, in its CSS pixels.
        y: f32,
        /// When it next pages.
        next: Instant,
    },
}

impl ConversationWindow {
    /// The main button went down `now` at `on`, a point on the first screen
    /// in its CSS pixels, with the pointer `at` in the window's device
    /// pixels: whether it was the bar's — a drag of its thumb begun, or the
    /// conversation paged back or on.
    pub(super) fn bar_pressed(
        &mut self,
        on: alo_paint::Point,
        at: (f64, f64),
        now: Instant,
    ) -> bool {
        // Only where the bar is drawn: the conversation on screen, or
        // *Settings* (queue item 880), and no split view, which scrolls
        // nothing.
        let settings = self.conversation.settings().is_open();
        if !settings && (!self.conversation.conversing() || self.drawn_stands == Stands::Split) {
            return false;
        }
        let (Some(viewport), Some((_, scale))) = (self.start_room(), self.measured()) else {
            return false;
        };
        let Some(scrolled) = self.conversation.scrolled() else {
            return false;
        };
        let room = self.conversation.room(viewport);
        let Some(bar) = scrolled.bar(room.height, scale.factor()) else {
            return false;
        };
        let Some(part) = conversation_bar_press::pressed(on.x, on.y, room, &bar) else {
            return false;
        };
        let down = scrolled.down();
        // A person scrolling is followed, not the agent.
        self.following = false;
        match part {
            OnTheBar::Thumb => {
                self.bar_drag = Some(BarDrag::Thumb {
                    from_y: at.1,
                    from_down: down,
                });
            }
            OnTheBar::Above | OnTheBar::Below => {
                self.page(part, room.height);
                // Held, it pages on (queue item 876).
                self.bar_drag = Some(BarDrag::Paging {
                    beside: part,
                    pressed: (on.y, at.1),
                    y: on.y,
                    next: now + conversation_bar_press::FIRST_REPEAT,
                });
            }
        }
        // The bar stays whole while it is held, as it is while it scrolls.
        self.bar.moved(now);
        true
    }

    /// The pointer moved to `at`, in device pixels, `now`: while the
    /// thumb is held, the conversation scrolled as far as it has moved.
    /// Whether a drag of the bar is under way, and the move its.
    pub(super) fn bar_dragged(&mut self, at: (f64, f64), now: Instant) -> bool {
        let Some(drag) = self.bar_drag else {
            return false;
        };
        let (Some(viewport), Some((_, scale))) = (self.start_room(), self.measured()) else {
            return true;
        };
        let factor = f64::from(scale.factor());
        let (from_y, from_down) = match drag {
            BarDrag::Thumb { from_y, from_down } => (from_y, from_down),
            // Paging on: where the pointer is, for when it next pages.
            BarDrag::Paging {
                beside,
                pressed,
                next,
                ..
            } => {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a pointer's travel down a window, in CSS pixels, far inside an f32's range"
                )]
                let y = pressed.0 + ((at.1 - pressed.1) / factor) as f32;
                self.bar_drag = Some(BarDrag::Paging {
                    beside,
                    pressed,
                    y,
                    next,
                });
                return true;
            }
        };
        let Some(scrolled) = self.conversation.scrolled() else {
            return true;
        };
        let Some(reach) = scrolled.reach() else {
            return true;
        };
        let room = self.conversation.room(viewport).height;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a pointer's travel down a window, in CSS pixels, far inside an f32's range"
        )]
        let moved = ((at.1 - from_y) / factor) as f32;
        let to = conversation_bar_press::dragged(from_down, moved, reach, room, scale.factor());
        let by = to - scrolled.down();
        if self.conversation.scroll(by, viewport) {
            self.redraw = true;
            self.ask_drawn();
        }
        self.bar.moved(now);
        true
    }

    /// The main button came up: a drag of the bar, if one was under way,
    /// is over. Whether one was.
    pub(super) fn bar_released(&mut self) -> bool {
        self.bar_drag.take().is_some()
    }

    /// Whether the bar's thumb is held now (queue item 845).
    pub fn dragging_the_bar(&self) -> bool {
        matches!(self.bar_drag, Some(BarDrag::Thumb { .. }))
    }

    /// When a press held beside the thumb next pages, while one is held
    /// (queue item 876).
    pub(super) fn bar_pages_at(&self) -> Option<Instant> {
        match self.bar_drag {
            Some(BarDrag::Paging { next, .. }) => Some(next),
            _ => None,
        }
    }

    /// The time is `now`: a press held beside the thumb pages on, if its
    /// time has come and the thumb has not yet reached the pointer, and is
    /// due again a [`conversation_bar_press::REPEAT`] later — or, once the
    /// thumb has reached it, pages no more while it is held.
    pub(super) fn bar_page_on(&mut self, now: Instant) {
        let Some(BarDrag::Paging {
            beside,
            pressed,
            y,
            next,
        }) = self.bar_drag
        else {
            return;
        };
        if now < next {
            return;
        }
        let (Some(viewport), Some((_, scale))) = (self.start_room(), self.measured()) else {
            return;
        };
        let room = self.conversation.room(viewport);
        let beyond = self
            .conversation
            .scrolled()
            .and_then(|scrolled| scrolled.bar(room.height, scale.factor()))
            .is_some_and(|bar| conversation_bar_press::still_beyond(beside, y, room, &bar));
        if beyond {
            self.page(beside, room.height);
        }
        self.bar_drag = Some(BarDrag::Paging {
            beside,
            pressed,
            y,
            next: now + conversation_bar_press::REPEAT,
        });
        // Whole while it is held, as while it scrolls.
        self.bar.moved(now);
    }

    /// One page back, `beside` the thumb above it, or on, below it, in a
    /// room `room` tall, as Page Up and Page Down are: drawn again if that
    /// scrolled it.
    fn page(&mut self, beside: OnTheBar, room: f32) {
        let key = if beside == OnTheBar::Above {
            Key::PageUp
        } else {
            Key::PageDown
        };
        let Some(viewport) = self.start_room() else {
            return;
        };
        if let Some(scroll @ Scroll::By(_)) = conversation_keys::scroll(&Press::of(key), room)
            && self.conversation.scroll_by_key(scroll, viewport)
        {
            self.redraw = true;
            self.ask_drawn();
        }
    }
}
