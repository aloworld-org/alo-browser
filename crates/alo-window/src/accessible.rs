/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The conversation's window on the platform's accessibility bus (ADR 0115
//! § 1, queue item 987): the one file that names `accesskit_winit`, and one
//! of the two that name AccessKit at all ([`crate::accessible_tree`] is the
//! other, and decides what the tree becomes).
//!
//! It makes the adapter for the window, hands it the window's events, sends
//! it what the window publishes and takes its requests. It is handed
//! `winit`'s event loop, window and events by [`crate::window`], as
//! [`crate::present_shown`] is.
//!
//! # Made before the window is shown
//!
//! `accesskit_winit` 0.34.2 panics if its adapter is made for a window
//! already visible, so the conversation's window is created hidden and
//! shown once this is made ([`Accessible::over`] refuses a window it can
//! see is shown, rather than letting the adapter panic).
//!
//! # Never waited on, and read only while something listens
//!
//! The handlers may be called on any thread — AccessKit's own, on Linux —
//! so each only tells the window, which never waits on them (ADR 0024 § 2).
//! Asked for the tree, the window has each part's renderer read it, and
//! sends it once it is read: AccessKit asks that the first tree be sent no
//! later than the next refresh, and until it is, the macOS adapter answers
//! with a placeholder of its own. Nothing is read before the bus asks
//! (ADR 0002's *reading is never watching*). On macOS the adapter never
//! says the bus stopped listening; once asked, the tree is read after each
//! frame for as long as the window is open.
//!
//! # No `unsafe` of ours
//!
//! `Adapter::with_direct_handlers`, `Adapter::process_event` and
//! `Adapter::update_if_active` are safe functions in `accesskit_winit`
//! 0.34.2's source, and `Window::is_visible` in `winit` 0.30.13's; the
//! `unsafe` that subclasses the window's view is `accesskit_winit`'s own
//! (ADR 0010).

use crate::accessible_tree::{self, Grafted};
use crate::assisted::{FromTheBus, Published, TellTheWindow};
use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

/// The adapter for one window, and what it has been sent.
pub struct Accessible {
    adapter: accesskit_winit::Adapter,
    /// Which parts' subtrees it has been sent.
    grafted: Grafted,
}

impl Accessible {
    /// The adapter for `window`, which must not have been shown yet,
    /// telling the window what the bus says with `tell`.
    ///
    /// # Errors
    ///
    /// That the window is already shown, which `accesskit_winit` would
    /// panic on.
    pub fn over(
        event_loop: &ActiveEventLoop,
        window: &Window,
        tell: &TellTheWindow,
    ) -> Result<Self, &'static str> {
        if window.is_visible() == Some(true) {
            return Err("the window was shown before it could be published to the bus");
        }
        let adapter = accesskit_winit::Adapter::with_direct_handlers(
            event_loop,
            window,
            Activation(tell.clone()),
            Actions(tell.clone()),
            Deactivation(tell.clone()),
        );
        Ok(Self {
            adapter,
            grafted: Grafted::default(),
        })
    }

    /// Hand the adapter `event`, before the window acts on it.
    pub fn hear(&mut self, window: &Window, event: &WindowEvent) {
        self.adapter.process_event(window, event);
    }

    /// Send the adapter `published`, if the bus is listening.
    pub fn publish(&mut self, published: &Published) {
        let (updates, grafted) = accessible_tree::updates(published, self.grafted);
        let mut sent = false;
        for update in updates {
            self.adapter.update_if_active(|| {
                sent = true;
                update
            });
        }
        // Nothing was sent to an adapter not yet asked, so nothing is held.
        if sent {
            self.grafted = grafted;
        }
    }
}

/// Asked for the tree: the window is told, and sends it once each part's
/// renderer has read it.
struct Activation(TellTheWindow);

impl ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        // A window gone has nothing to publish.
        let _ = (self.0)(FromTheBus::Active);
        None
    }
}

/// An action asked: the window is told what it is in this browser's
/// words, or what was refused as it was asked.
struct Actions(TellTheWindow);

impl ActionHandler for Actions {
    fn do_action(&mut self, request: ActionRequest) {
        let heard = match accessible_tree::asked(&request) {
            Ok(asked) => FromTheBus::Asked(asked),
            Err(refused) => FromTheBus::Refused(refused),
        };
        let _ = (self.0)(heard);
    }
}

/// Nothing listens any more.
struct Deactivation(TellTheWindow);

impl DeactivationHandler for Deactivation {
    fn deactivate_accessibility(&mut self) {
        let _ = (self.0)(FromTheBus::Inactive);
    }
}
