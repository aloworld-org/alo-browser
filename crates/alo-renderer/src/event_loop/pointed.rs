/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's main button, as tasks (ADR 0024 § 5, ADR 0018 § 3, queue item
//! 535).
//!
//! # What Chrome fires, measured
//!
//! Measured in headless Chrome 154 over CDP, with `Input.dispatchMouseEvent`
//! pressing and letting go of the left button and capturing listeners on
//! the document recording each event, its members and when its microtasks
//! and timers ran — the source not read:
//!
//! - **A press is one task**: `pointerdown`, then `mousedown`, at the
//!   element under the point.
//! - **A release is the next**: `pointerup`, then `mouseup`, at the element
//!   under the point — the root element, when the point is outside the
//!   viewport — then `click`, at the nearest element both the pressed and
//!   the released element are inside. No `click` at all when the pressed
//!   element has left its document by then.
//! - Every one bubbles, is cancelable and is composed, and each listener is
//!   followed by a microtask checkpoint, as for every dispatch the browser
//!   makes ([`super::dispatched`]).
//! - **A cancelled `pointerdown`** keeps the `mousedown` and the `mouseup`
//!   from being fired; `pointerup` and `click` still are. Cancelling
//!   `mousedown`, `pointerup` or `mouseup` changes none of them.
//! - **Where**: a `PointerEvent` that is not a click reports the point as
//!   it arrived (`70.5999984741211` for 70.6); a `mousedown`, a `mouseup`
//!   and a `click` report it floored to whole pixels.
//! - **What was pressed**: `button` is `0` throughout; `buttons` is `1` on
//!   the way down and `0` on the way up; `detail` is `1` on `mousedown`,
//!   `mouseup` and `click`, and `0` on `pointerdown` and `pointerup`; a
//!   `PointerEvent`'s `pointerId` is `1` and its `pointerType` `"mouse"`,
//!   the `click` included.
//!
//! # The pointer arriving
//!
//! A press or a release over a node that is not the one last under the
//! pointer — a press where nothing was said of the pointer before, a
//! release somewhere else — fires the crossing to it first, in the same
//! task, carrying the button that changed (queue item 542,
//! [`super::crossed`]).
//!
//! # What Chrome fires that is not here
//!
//! Focus moving to what was pressed (item 258).
//! `selectstart` and `selectionchange` (item 88).
//!
//! # A submit button
//!
//! The click's activation submits its form inside the release's task, as
//! an agent's `Activate` does ([`super::submitted`], queue item 543):
//! `invalid` at each control that fails, or `submit` at the form, and then
//! the ask. Measured again for this, with `Input.dispatchMouseEvent` on the
//! frozen `alo-sites-booking` and `web-a-form` pages: Chrome 154's mouse
//! click sends what ADR 0051 measured for a press.

use alo_bindings::{Device, Fired, Firing, Pointing, node_of};
use alo_dom::NodeId;
use alo_js::{Escape, Fault, Root};
use alo_paint::Point;

use super::crossed::Crossed;
use super::task;
use super::{Clicked, EventLoop, Turn};
use crate::crossing::Crossing;

/// What a press's task did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressedDown {
    /// Whether a listener cancelled the `pointerdown`, which keeps the
    /// `mouseup` of the release that follows from being fired too.
    pub canceled: bool,
}

/// The events a person's button fires, each as Chrome 154 makes it.
pub(super) mod firing {
    use super::{Device, Fired, Firing, Point, Pointing};

    /// A `PointerEvent` or a `MouseEvent` of `kind`, bubbling, cancelable
    /// and composed, reporting `pointing`.
    const fn made(interface: Fired<'static>, kind: &'static str) -> Firing<'static> {
        Firing {
            interface,
            kind,
            bubbles: true,
            cancelable: true,
            composed: true,
        }
    }

    /// Where the mouse was and what it held: the point as it arrived, or
    /// floored, as each event reports it.
    fn pointing(at: Point, floored: bool, buttons: u16, detail: u32) -> Pointing {
        let (x, y) = if floored {
            (at.x.floor(), at.y.floor())
        } else {
            (at.x, at.y)
        };
        Pointing {
            x,
            y,
            button: 0,
            buttons,
            detail,
            device: Device::Mouse,
        }
    }

    /// `pointerdown`.
    pub(crate) fn pointer_down(at: Point) -> Firing<'static> {
        made(
            Fired::PointerEvent(pointing(at, false, 1, 0)),
            "pointerdown",
        )
    }

    /// `mousedown`.
    pub(crate) fn mouse_down(at: Point) -> Firing<'static> {
        made(Fired::MouseEvent(pointing(at, true, 1, 1)), "mousedown")
    }

    /// `pointerup`.
    pub(crate) fn pointer_up(at: Point) -> Firing<'static> {
        made(Fired::PointerEvent(pointing(at, false, 0, 0)), "pointerup")
    }

    /// `mouseup`.
    pub(crate) fn mouse_up(at: Point) -> Firing<'static> {
        made(Fired::MouseEvent(pointing(at, true, 0, 1)), "mouseup")
    }

    /// The `click` that ends a press.
    pub(crate) fn click(at: Point) -> Firing<'static> {
        made(Fired::PointerEvent(pointing(at, true, 0, 1)), "click")
    }
}

impl EventLoop {
    /// Run a [`Work::Pressed`](task::Work::Pressed) task: the `crossing`,
    /// if there is one, then the `pointerdown` first in its list, and then
    /// — unless a listener cancelled it — a `mousedown` at the same element.
    pub(super) fn pressed(
        &mut self,
        list: &Root,
        at: Point,
        crossing: Option<&Crossing>,
        turn: &mut Turn,
    ) -> Result<PressedDown, Escape> {
        if let Some(crossing) = crossing {
            self.cross(list, crossing, at, Crossed::Pressing, turn)?;
        }
        let (down, target) = task::dispatched(&mut self.engine, list)?;
        let canceled = self.dispatch_event(down, target, turn)?;
        if !canceled {
            self.fire(list, target, &firing::mouse_down(at), turn)?;
        }
        Ok(PressedDown { canceled })
    }

    /// Run a [`Work::Released`](task::Work::Released) task: the `crossing`,
    /// if there is one, then the `pointerup`
    /// first in its list, a `mouseup` at the same element when `mouse`
    /// says the press let one be fired, and then the `click` at `click`,
    /// if there is one to fire, with HTML's activation steps around it.
    pub(super) fn released(
        &mut self,
        list: &Root,
        at: Point,
        mouse: bool,
        click: Option<NodeId>,
        crossing: Option<&Crossing>,
        turn: &mut Turn,
    ) -> Result<Option<Clicked>, Escape> {
        if let Some(crossing) = crossing {
            self.cross(list, crossing, at, Crossed::Releasing, turn)?;
        }
        let (up, target) = task::dispatched(&mut self.engine, list)?;
        self.dispatch_event(up, target, turn)?;
        if mouse {
            self.fire(list, target, &firing::mouse_up(at), turn)?;
        }
        let Some(node) = click else {
            return Ok(None);
        };
        self.awake()?;
        let (cell, _) =
            node_of(self.engine.objects(), target).ok_or(Escape::fault(Fault::NotAnObject))?;
        let (target, click) = task::add(&mut self.engine, list, cell, node, &firing::click(at))
            .map_err(task::Unmade::into_escape)?;
        self.activation(list, click, target, turn).map(Some)
    }

    /// Fire the event `firing` describes at the node whose wrapper is
    /// `target`, the event held by the task's `list`.
    fn fire(
        &mut self,
        list: &Root,
        target: alo_js::heap::Ref,
        firing: &Firing<'_>,
        turn: &mut Turn,
    ) -> Result<bool, Escape> {
        self.awake()?;
        let (cell, node) =
            node_of(self.engine.objects(), target).ok_or(Escape::fault(Fault::NotAnObject))?;
        let (target, event) = task::add(&mut self.engine, list, cell, node, firing)
            .map_err(task::Unmade::into_escape)?;
        self.dispatch_event(event, target, turn)
    }
}

#[cfg(test)]
mod tests {
    use super::firing;
    use alo_bindings::{Device, Fired};
    use alo_paint::Point;

    /// The numbers each event reports, as Chrome 154 reported them.
    fn reported(made: alo_bindings::Firing<'_>) -> (f32, f32, i16, u16, u32) {
        let (Fired::PointerEvent(at) | Fired::MouseEvent(at)) = made.interface else {
            panic!("{} is not a pointer's event", made.kind);
        };
        assert_eq!(at.device, Device::Mouse);
        (at.x, at.y, at.button, at.buttons, at.detail)
    }

    #[test]
    fn each_event_reports_the_pointer_as_chrome_does() {
        let at = Point::new(70.6, 50.3);
        assert_eq!(
            reported(firing::pointer_down(at)),
            (70.6, 50.3, 0, 1, 0),
            "a PointerEvent keeps the point as it came"
        );
        assert_eq!(reported(firing::mouse_down(at)), (70.0, 50.0, 0, 1, 1));
        assert_eq!(reported(firing::pointer_up(at)), (70.6, 50.3, 0, 0, 0));
        assert_eq!(reported(firing::mouse_up(at)), (70.0, 50.0, 0, 0, 1));
        assert_eq!(
            reported(firing::click(at)),
            (70.0, 50.0, 0, 0, 1),
            "the click is floored, though it is a PointerEvent"
        );
    }

    #[test]
    fn which_interface_each_is_and_that_each_bubbles_and_may_be_cancelled() {
        let at = Point::new(1.0, 2.0);
        for (made, kind, pointer) in [
            (firing::pointer_down(at), "pointerdown", true),
            (firing::mouse_down(at), "mousedown", false),
            (firing::pointer_up(at), "pointerup", true),
            (firing::mouse_up(at), "mouseup", false),
            (firing::click(at), "click", true),
        ] {
            assert_eq!(made.kind, kind);
            assert_eq!(
                matches!(made.interface, Fired::PointerEvent(_)),
                pointer,
                "{kind}"
            );
            assert!(made.bubbles && made.cancelable && made.composed, "{kind}");
        }
    }
}
