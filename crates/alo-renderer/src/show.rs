/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page told whether it can be seen, run to its end before the browser
//! process is answered (ADR 0039 § 1, queue item 364).
//!
//! The browser process decides whether a page can be seen — it alone knows
//! which tab is selected and whether the window is covered — and tells the
//! renderer with [`crate::ToRenderer::Visibility`]. The renderer keeps the
//! state with the page, and on a page whose script has run it is a task
//! ([`Held::visibility`]) that sets the document's state and fires
//! `visibilitychange` if it changed. This runs that task, and any queued
//! before it, and says what the page's script said meanwhile.
//!
//! **A page that has never run script is told nothing**: nothing on it could
//! listen, and it is not given a heap to find that out. Its state is kept
//! with its [`crate::Page`] all the same, and a script it runs later — none
//! does, today, after its load — would read it from there.

use alo_bindings::Visibility;

use crate::held::Held;
use crate::run_to::{Said, run_to};

/// Tell the page `held` holds that its visibility state is `to`, and say
/// what came of it.
pub(crate) fn tell(held: &mut Held, to: Visibility) -> Vec<String> {
    let mut said = Said::about("the page's visibility");
    match held.visibility(to) {
        Ok(Some(seq)) => {
            run_to(held, seq, &mut said);
        }
        Ok(None) => {}
        Err(why) => said.say(&why),
    }
    said.lines()
}
