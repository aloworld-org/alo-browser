/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An agent's `Activate`, as a task (ADR 0018 §§ 5–7, queue item 256).
//!
//! `Activate` is the one `click` keyboard activation fires: a trusted
//! `PointerEvent` with no position ([`Firing::CLICK`]). Around its dispatch
//! run HTML's activation steps, which are `alo-dom`'s
//! ([`alo_dom::activation`]) and are run here because this is who
//! dispatched:
//!
//! 1. **before** — the activation target's pre-activation: a checkbox
//!    turned over, a radio checked and its group cleared — so the click's
//!    listeners read the new state;
//! 2. the click, dispatched as [`dispatched`](super::dispatched) dispatches
//!    any event the browser makes, a checkpoint after every listener;
//! 3. **cancelled**, if a listener called `preventDefault`, which puts the
//!    box back; or else **after**, which says what follows — for a box
//!    still in its document, an `input` and then a `change` at it, each
//!    dispatched the same way, inside **this same task** as the standard
//!    has them; for a link, the link is followed — which is the browser
//!    process's to do, so it is **asked**: kept in the document cell beside
//!    any ask a listener's `click()` made before it (ADR 0020 §§ 1 and 5),
//!    and said in the answer.
//!
//! All of it is one task (ADR 0016 § 6), so no other task runs between the
//! box changing and its listeners hearing about it, and the agent's answer
//! comes after the whole of it (ADR 0018 § 3).
//!
//! # ARIA state is the page's
//!
//! Nothing here changes `aria-checked` (ADR 0018 § 7): on a page that runs
//! script, that is the author's promise and the author's listener keeps it.
//! The renderer's stage 1 accommodation applies only to a page that never
//! ran script, which never reaches this.

use alo_bindings::navigating::{self, By};
use alo_bindings::{Firing, change_document, document, node_of};
use alo_dom::activation::{self, Follows};
use alo_js::{Escape, Fault, Root};

use super::task;
use super::{EventLoop, Turn};

/// What an activation task did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clicked {
    /// Whether a listener cancelled the click.
    pub canceled: bool,
    /// What followed it: nothing when it was cancelled; the `input` and
    /// `change` that were fired at a box; or a link for the browser process
    /// to follow.
    pub follows: Follows,
    /// Whether following the link started a navigation: kept in the
    /// document cell as the page's ongoing one. `false` for a link that was
    /// not followed — a download, another window, an `href` that goes
    /// nowhere — which the cell says why of.
    pub navigated: bool,
}

impl EventLoop {
    /// Run a [`Work::Activate`](task::Work::Activate) task: the activation
    /// steps around one click.
    pub(super) fn activate(&mut self, list: &Root, turn: &mut Turn) -> Result<Clicked, Escape> {
        let (click, target) = task::dispatched(&mut self.engine, list)?;
        let (cell, node) =
            node_of(self.engine.objects(), target).ok_or(Escape::fault(Fault::NotAnObject))?;
        let done = change_document(self.engine.objects(), cell, |document| {
            activation::before(document, node)
        })
        .ok_or(Escape::fault(Fault::NotAnObject))?;

        let canceled = self.dispatch_event(click, target, turn)?;
        if canceled {
            change_document(self.engine.objects(), cell, |document| {
                activation::cancelled(document, &done);
            })
            .ok_or(Escape::fault(Fault::NotAnObject))?;
            return Ok(Clicked {
                canceled,
                follows: Follows::Nothing,
                navigated: false,
            });
        }

        let follows = document(self.engine.objects(), cell)
            .map(|document| activation::after(document, &done))
            .ok_or(Escape::fault(Fault::NotAnObject))?;
        if let Follows::InputAndChange(at) = follows {
            for firing in [Firing::INPUT, Firing::CHANGE] {
                self.awake()?;
                let (target, event) = task::add(&mut self.engine, list, cell, at, &firing)
                    .map_err(task::Unmade::into_escape)?;
                self.dispatch_event(event, target, turn)?;
            }
        }
        let navigated = match &follows {
            Follows::Link { node, .. } => {
                navigating::start(self.engine.objects(), cell, *node, By::Browser)
                    .ok_or(Escape::fault(Fault::NotAnObject))?
            }
            Follows::Nothing | Follows::InputAndChange(_) => false,
        };
        Ok(Clicked {
            canceled,
            follows,
            navigated,
        })
    }
}
