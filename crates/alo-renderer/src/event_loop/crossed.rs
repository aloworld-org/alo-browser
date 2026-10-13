/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's pointer crossing from one node to another, and moving, as
//! tasks (ADR 0024 § 5, queue item 542).
//!
//! Which nodes are left and entered is [`crate::crossing`]'s; this fires
//! them, as Chrome 154 was measured to — headless, over CDP, listeners on
//! the window, the document and every element, the source not read.
//!
//! # The order
//!
//! A crossing fires its **pointer events first**: `pointerout`, each
//! `pointerleave`, `pointerover`, each `pointerenter`; **then the same as
//! mouse events**: `mouseout`, each `mouseleave`, `mouseover`, each
//! `mouseenter`. A move then fires `pointermove` and `mousemove` at the
//! node under the pointer. All of it is **one task**: a timer a listener
//! set ran after the last of them, and a microtask after each listener, as
//! for every dispatch the browser makes ([`super::dispatched`]).
//!
//! A press or a release whose node is not the one last under the pointer
//! fires the crossing first, in the same task as the button's own events;
//! it fires no move.
//!
//! # What each carries
//!
//! - `pointerover`, `pointerout`, `pointermove` and their mouse twins
//!   bubble, are cancelable and are composed; every enter and leave does
//!   none of the three. Cancelling any of them changed nothing Chrome did.
//! - A `PointerEvent` reports the point as it arrived; a `MouseEvent`
//!   floored. `detail` is `0` on all of them.
//! - **As the pointer moves**, a `PointerEvent`'s `button` is `-1` and a
//!   `MouseEvent`'s `0`; `buttons` is what is held. **At a press** both
//!   are `0` with `buttons` `1`, and **at a release** `0` and `0`: the
//!   crossing carries the button that changed.
//! - `relatedTarget` is the other node ([`crate::crossing`]), and `null` on
//!   a move.
//! - **A cancelled `pointerdown`** keeps every `mousemove` from being fired
//!   until the release, and nothing else: the crossings' mouse events are
//!   still fired.
//!
//! # What Chrome fires that is not here
//!
//! `pointerrawupdate`, which Chrome fired before everything else at each
//! move, bubbling, composed and not cancelable — in a secure context only
//! (queue item 551). The crossing Chrome fires on a timer when the page
//! changes under a pointer that is not moving (552).

use alo_bindings::{Device, Firing, Pointing, node_of};
use alo_dom::NodeId;
use alo_js::heap::Ref;
use alo_js::{Escape, Fault, Root};
use alo_paint::Point;

use super::task::{self, Unmade};
use super::{EventLoop, Turn};
use crate::crossing::Crossing;

/// What the pointer was doing as it crossed: what each event says of its
/// buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crossed {
    /// Moving, with `buttons` held.
    Moving {
        /// UI Events' bits: `1` the main button.
        buttons: u16,
    },
    /// The main button going down.
    Pressing,
    /// The main button coming up.
    Releasing,
}

impl Crossed {
    /// `button` on a `PointerEvent` and on a `MouseEvent`, then `buttons`.
    const fn buttons(self) -> (i16, i16, u16) {
        match self {
            Self::Moving { buttons } => (-1, 0, buttons),
            Self::Pressing => (0, 0, 1),
            Self::Releasing => (0, 0, 0),
        }
    }
}

/// The events a person's pointer fires as it crosses and moves, each as
/// Chrome 154 makes it.
pub(super) mod firing {
    use super::{Crossed, Device, Firing, Point, Pointing};
    use alo_bindings::Fired;

    /// What each of `kind`'s events reports of the pointer at `at` as it
    /// was `crossed`: as a `PointerEvent` if `pointer`, else a
    /// `MouseEvent`, floored.
    fn pointing(at: Point, crossed: Crossed, pointer: bool) -> Pointing {
        let (pointer_button, mouse_button, buttons) = crossed.buttons();
        let (x, y, button) = if pointer {
            (at.x, at.y, pointer_button)
        } else {
            (at.x.floor(), at.y.floor(), mouse_button)
        };
        Pointing {
            x,
            y,
            button,
            buttons,
            detail: 0,
            device: Device::Mouse,
        }
    }

    /// The event `kind`, a `PointerEvent` if `pointer` is, else a
    /// `MouseEvent`; bubbling, cancelable and composed when `bubbles`.
    pub(crate) fn made(
        kind: &'static str,
        at: Point,
        crossed: Crossed,
        pointer: bool,
        bubbles: bool,
    ) -> Firing<'static> {
        let pointing = pointing(at, crossed, pointer);
        Firing {
            interface: if pointer {
                Fired::PointerEvent(pointing)
            } else {
                Fired::MouseEvent(pointing)
            },
            kind,
            bubbles,
            cancelable: bubbles,
            composed: bubbles,
        }
    }
}

/// The names of a crossing's four events, in one family: out, leave, over,
/// enter.
const POINTER: [&str; 4] = ["pointerout", "pointerleave", "pointerover", "pointerenter"];
/// The same, as mouse events.
const MOUSE: [&str; 4] = ["mouseout", "mouseleave", "mouseover", "mouseenter"];

impl EventLoop {
    /// Run a [`Work::Moved`](task::Work::Moved) task: the crossing, if
    /// there is one, then — when the pointer is over a node — `pointermove`
    /// there, and a `mousemove` unless `mouse` says a cancelled
    /// `pointerdown` holds them back.
    pub(super) fn moved(
        &mut self,
        list: &Root,
        moving: &Moving,
        turn: &mut Turn,
    ) -> Result<(), Escape> {
        let crossed = Crossed::Moving {
            buttons: moving.buttons,
        };
        if let Some(crossing) = &moving.crossing {
            self.cross(list, crossing, moving.at, crossed, turn)?;
        }
        let Some(node) = moving.over else {
            return Ok(());
        };
        let at = moving.at;
        self.fire_at(
            list,
            node,
            &firing::made("pointermove", at, crossed, true, true),
            None,
            turn,
        )?;
        if moving.mouse {
            self.fire_at(
                list,
                node,
                &firing::made("mousemove", at, crossed, false, true),
                None,
                turn,
            )?;
        }
        Ok(())
    }

    /// Fire `crossing`'s events at `at`, as the pointer `crossed`: the
    /// pointer events, then the mouse events, each family out, leaves,
    /// over and enters.
    pub(super) fn cross(
        &mut self,
        list: &Root,
        crossing: &Crossing,
        at: Point,
        crossed: Crossed,
        turn: &mut Turn,
    ) -> Result<(), Escape> {
        for (names, pointer) in [(POINTER, true), (MOUSE, false)] {
            let [out, leave, over, enter] = names;
            let event = |kind, bubbles| firing::made(kind, at, crossed, pointer, bubbles);
            if let Some(node) = crossing.out {
                self.fire_at(list, node, &event(out, true), crossing.to, turn)?;
            }
            for node in &crossing.leave {
                self.fire_at(list, *node, &event(leave, false), crossing.to, turn)?;
            }
            if let Some(node) = crossing.over {
                self.fire_at(list, node, &event(over, true), crossing.from, turn)?;
            }
            for node in &crossing.enter {
                self.fire_at(list, *node, &event(enter, false), crossing.from, turn)?;
            }
        }
        Ok(())
    }

    /// Fire the event `firing` describes at `node`, its `relatedTarget`
    /// `related`, the event and both wrappers held by the task's `list` —
    /// whose first slot is a wrapper in the page's document.
    ///
    /// A node no longer in the document's store is not fired at, and a
    /// `relatedTarget` that is not is `null`: a listener earlier in the
    /// task may have taken either out, and that is the page's doing, not a
    /// reason to stop it.
    fn fire_at(
        &mut self,
        list: &Root,
        node: NodeId,
        firing: &Firing<'_>,
        related: Option<NodeId>,
        turn: &mut Turn,
    ) -> Result<bool, Escape> {
        self.awake()?;
        let cell = self.cell(list)?;
        let (target, event) = match task::add(&mut self.engine, list, cell, node, firing) {
            Ok(made) => made,
            Err(Unmade::NoSuchNode(_)) => return Ok(false),
            Err(unmade) => return Err(unmade.into_escape()),
        };
        if let Some(related) = related {
            let objects = self.engine.objects();
            match task::hold(objects, list, cell, related) {
                Ok(wrapper) => alo_bindings::event::relate(objects, event, wrapper)?,
                Err(Unmade::NoSuchNode(_)) => {}
                Err(unmade) => return Err(unmade.into_escape()),
            }
        }
        self.dispatch_event(event, target, turn)
    }

    /// The document cell the wrapper first in `list` is in.
    fn cell(&mut self, list: &Root) -> Result<Ref, Escape> {
        let first = task::first(&mut self.engine, list)?;
        node_of(self.engine.objects(), first)
            .map(|(cell, _)| cell)
            .ok_or(Escape::fault(Fault::NotAnObject))
    }
}

/// What a [`Work::Moved`](task::Work::Moved) task does, beyond its list.
#[derive(Debug, Clone, PartialEq)]
pub struct Moving {
    /// Where the pointer is, in the viewport's CSS pixels.
    pub at: Point,
    /// The crossing to fire first, if the node under the pointer changed.
    pub crossing: Option<Crossing>,
    /// The node `pointermove` and `mousemove` are fired at: [`None`] when
    /// the pointer is outside the viewport with no button held, where
    /// Chrome fires neither.
    pub over: Option<NodeId>,
    /// The buttons held, as UI Events' bits.
    pub buttons: u16,
    /// Whether `mousemove` is fired: not while a cancelled `pointerdown`'s
    /// button is held.
    pub mouse: bool,
}

#[cfg(test)]
mod tests {
    use super::{Crossed, firing};
    use alo_bindings::{Device, Fired};
    use alo_paint::Point;

    /// The numbers an event reports, as Chrome 154 reported them.
    fn reported(made: alo_bindings::Firing<'_>) -> (f32, f32, i16, u16, u32, bool) {
        let (pointer, at) = match made.interface {
            Fired::PointerEvent(at) => (true, at),
            Fired::MouseEvent(at) => (false, at),
            _ => panic!("{} is not a pointer's event", made.kind),
        };
        assert_eq!(at.device, Device::Mouse);
        (at.x, at.y, at.button, at.buttons, at.detail, pointer)
    }

    #[test]
    fn a_move_reports_no_button_as_minus_one_on_a_pointer_event_and_zero_on_a_mouse_event() {
        let at = Point::new(30.6, 30.3);
        let moving = Crossed::Moving { buttons: 0 };
        assert_eq!(
            reported(firing::made("pointerover", at, moving, true, true)),
            (30.6, 30.3, -1, 0, 0, true)
        );
        assert_eq!(
            reported(firing::made("mouseover", at, moving, false, true)),
            (30.0, 30.0, 0, 0, 0, false)
        );
        let dragging = Crossed::Moving { buttons: 1 };
        assert_eq!(
            reported(firing::made("pointermove", at, dragging, true, true)),
            (30.6, 30.3, -1, 1, 0, true)
        );
    }

    #[test]
    fn a_crossing_at_a_press_or_a_release_carries_the_button_that_changed() {
        let at = Point::new(130.0, 60.0);
        assert_eq!(
            reported(firing::made(
                "pointerover",
                at,
                Crossed::Pressing,
                true,
                true
            )),
            (130.0, 60.0, 0, 1, 0, true)
        );
        assert_eq!(
            reported(firing::made(
                "mouseout",
                at,
                Crossed::Releasing,
                false,
                true
            )),
            (130.0, 60.0, 0, 0, 0, false)
        );
    }

    #[test]
    fn an_enter_or_a_leave_neither_bubbles_nor_may_be_cancelled() {
        let at = Point::new(1.0, 2.0);
        let moving = Crossed::Moving { buttons: 0 };
        let over = firing::made("pointerover", at, moving, true, true);
        assert!(over.bubbles && over.cancelable && over.composed);
        let enter = firing::made("mouseenter", at, moving, false, false);
        assert!(!enter.bubbles && !enter.cancelable && !enter.composed);
    }
}
