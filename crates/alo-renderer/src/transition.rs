/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page shown, and a page left (ADR 0039 §§ 2–4, queue item 373).
//!
//! # Shown
//!
//! HTML sets a document's *page showing* flag and fires `pageshow` at its
//! window as the last step of its load, after its own scripts have run.
//! [`shown`] is that step, and the renderer takes it before it answers
//! `Loaded` and before anything else can reach it — so **every page a
//! renderer holds is showing**, and the flag is the fact of holding one
//! rather than a field beside it. When the `load` event is built, it is
//! fired immediately before (item 351).
//!
//! # Left
//!
//! **Every way a page stops being held goes through [`leave`]**: a `Load`
//! into a renderer holding a page, and [`crate::ToRenderer::Leave`], which
//! the browser process sends when a tab is closed. Its leaving steps run as
//! one task (`event_loop/transition.rs`), and that task — with any queued
//! before it — is given [`LONGEST_LEAVING`] by a [`Deadline`] on the page's
//! own `Stop`. A page still running then is stopped, and the answer says so.
//!
//! Afterwards the renderer lets go of the page whole: every task still
//! waiting, every job, every root, the engine. So **nothing the page
//! queued while it was left ever runs** — not a timer (item 92 has none
//! yet), not the answer to a fetch, which arrives at a renderer holding no
//! page, and not a reaction waiting on one.
//!
//! **What it carries out** is what its script said and the fetches it
//! asked for, as claims. The browser process refuses each fetch, by name,
//! until keep-alive is decided (§ 4, item 369). Where it asked to go is not
//! carried: HTML ignores a navigation from a document being unloaded, and a
//! page being left has no tab to send anywhere.
//!
//! A renderer that crashes, or that the browser process stops for silence,
//! runs none of this. That is ADR 0005's dead renderer, and nothing can be
//! promised a last word by a process that has died.

use crate::deadline::{Deadline, LONGEST_LEAVING};
use crate::event_loop::Unqueued;
use crate::fetch::FetchAsk;
use crate::held::Held;
use crate::run_to::{Said, run_to};

/// What a page said, and asked to fetch, as it was left.
#[derive(Debug, Default)]
pub(crate) struct Left {
    /// What its script said, about "the page that was left".
    pub(crate) said: Vec<String>,
    /// Every fetch it asked for while it was left, in order.
    pub(crate) fetches: Vec<FetchAsk>,
}

/// Fire `pageshow` at the window of the page `held` holds, and say what
/// its listeners said. A page that has never run script is told nothing:
/// nothing on it could listen. A page whose script stopped as it loaded has
/// said so already, and is not made to say it twice.
pub(crate) fn shown(held: &mut Held) -> Vec<String> {
    let mut said = Said::about("the page being shown");
    match held.page_show() {
        Ok(Some(seq)) => {
            run_to(held, seq, &mut said);
        }
        Ok(None) | Err(Unqueued::Stopped(_)) => {}
        Err(why) => said.say(&why),
    }
    said.lines()
}

/// Run the leaving steps of the page `held` holds, within
/// [`LONGEST_LEAVING`], and say what came of it. The caller lets go of the
/// page afterwards.
pub(crate) fn leave(held: &mut Held) -> Left {
    let mut said = Said::about("the page that was left");
    match held.leave() {
        Ok(Some(seq)) => {
            let stop = held.event_loop().map(|page_loop| page_loop.stop_switch());
            let deadline = stop.map(|stop| (Deadline::arm(stop.clone(), LONGEST_LEAVING), stop));
            if let Some((Err(why), stop)) = &deadline {
                // Unbounded is the one thing leaving may not be: with no
                // thread to keep the time, it is given none.
                stop.ask();
                said.say(&format_args!(
                    "it was given no time to leave, since nothing could keep its deadline: {why}"
                ));
            }
            run_to(held, seq, &mut said);
            if let Some((Ok(deadline), _)) = deadline
                && deadline.disarm()
            {
                said.say(&format_args!(
                    "it was stopped, still running {} ms after it began to leave",
                    LONGEST_LEAVING.as_millis()
                ));
            }
        }
        Ok(None) | Err(Unqueued::Stopped(_)) => {}
        Err(why) => said.say(&why),
    }
    let fetches = held
        .take_fetches()
        .into_iter()
        .map(FetchAsk::from)
        .collect();
    // Taken and let go of, never carried (ADR 0039 § 3).
    drop(held.take_navigation());
    Left {
        said: said.lines(),
        fetches,
    }
}
