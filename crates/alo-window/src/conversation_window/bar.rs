/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The conversation's window and its bar pressed and dragged (queue item
//! 845, [`crate::conversation_bar_press`]): a press in the bar's strip is
//! the window's own, decided here against the bar the window itself
//! computes and sends to be drawn — never handed to the renderer to claim —
//! and the pointer moved with the button down after one on the thumb
//! scrolls the conversation with it until the button comes up.

use super::ConversationWindow;
use crate::conversation_bar_press::{self, OnTheBar};
use crate::conversation_keys::{self, Scroll};
use crate::keys::{Key, Press};
use alo_renderer::conversation_scroll;
use alo_renderer::split_place::Stands;
use std::time::Instant;

/// A drag of the bar's thumb under way: where the pointer went down, down
/// the window in device pixels, and how far the conversation was scrolled
/// then.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BarDrag {
    /// The pointer's height as it was pressed, in device pixels.
    from_y: f64,
    /// How far the conversation was scrolled as it was pressed.
    from_down: f32,
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
        // Only where the bar is drawn: the conversation on screen, no
        // *Settings* over it and no split view, which scrolls nothing.
        if self.conversation.settings().is_open()
            || !self.conversation.conversing()
            || self.drawn_stands == Stands::Split
        {
            return false;
        }
        let (Some(viewport), Some((_, scale))) = (self.start_room(), self.measured()) else {
            return false;
        };
        let Some(thread) = self.conversation.thread() else {
            return false;
        };
        let Some(bar) = thread.bar(viewport, scale.factor()) else {
            return false;
        };
        let Some(part) = conversation_bar_press::pressed(on.x, on.y, viewport, &bar) else {
            return false;
        };
        let down = thread.down();
        // A person scrolling is followed, not the agent.
        self.following = false;
        match part {
            OnTheBar::Thumb => {
                self.bar_drag = Some(BarDrag {
                    from_y: at.1,
                    from_down: down,
                });
            }
            OnTheBar::Above | OnTheBar::Below => {
                let key = if part == OnTheBar::Above {
                    Key::PageUp
                } else {
                    Key::PageDown
                };
                let room = conversation_scroll::room(viewport);
                if let Some(scroll @ Scroll::By(_)) =
                    conversation_keys::scroll(&Press::of(key), room)
                    && self.conversation.scroll_by_key(scroll, viewport)
                {
                    self.redraw = true;
                    self.ask_drawn();
                }
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
        let Some(thread) = self.conversation.thread() else {
            return true;
        };
        let Some(reach) = thread.reach() else {
            return true;
        };
        let factor = f64::from(scale.factor());
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a pointer's travel down a window, in CSS pixels, far inside an f32's range"
        )]
        let moved = ((at.1 - drag.from_y) / factor) as f32;
        let to =
            conversation_bar_press::dragged(drag.from_down, moved, reach, viewport, scale.factor());
        let by = to - thread.down();
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
        self.bar_drag.is_some()
    }
}
