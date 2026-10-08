/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The answer to one of a page's fetches, run to its end before the browser
//! process is answered (ADR 0032 § 1, queue item 335).
//!
//! The answer is a task of its own on the page's loop ([`Held::deliver`]):
//! the promise waiting under the ask's number is settled, and the
//! checkpoint after it runs every reaction — a `.then` that marks a button,
//! a `.catch` that greys one, a `.then` that fetches again. This runs that
//! task, and any queued before it, and says what the page's script said
//! meanwhile. What the reactions asked for — a navigation, more fetches —
//! is taken by the renderer once this has run, and carried in the
//! delivery's answer.
//!
//! **An answer nothing waits for is answered by nobody**: an ask of a page
//! that has gone, whose promises went with its heap, or a number the page
//! never chose. It is said, and the page is not touched.

use crate::fetch::Fetched;
use crate::held::Held;
use crate::run_to::{Said, run_to};

/// Deliver `fetched` to the page `held` holds, and say what came of it.
pub(crate) fn deliver(held: &mut Held, fetched: &Fetched) -> Vec<String> {
    let mut said = Said::about("the answer to a fetch");
    match held.deliver(fetched.number, fetched.answer.responded()) {
        Ok(Some(seq)) => {
            run_to(held, seq, &mut said);
        }
        Ok(None) => said.say(&format_args!(
            "nothing on this page is waiting for fetch {}, so its answer was not delivered",
            fetched.number
        )),
        Err(why) => said.say(&why),
    }
    said.lines()
}
