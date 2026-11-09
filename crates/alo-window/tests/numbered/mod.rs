/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a test's page tells its server, read in the order the page told it
//! (queue item 889, as item 828 did for one test).
//!
//! A page tells the server what it heard with a fetch per report, and two
//! fetches are two connections that may reach the server in either order:
//! under the gate's load the field's `typed?ne` was kept after its
//! `typed?new`, and a test reading the last report it kept said a letter
//! was lost when every letter had reached the page. So each page numbers
//! its reports from one —
//!
//! ```js
//! let told = 0;
//! const said = (what) => fetch('/said/' + (told += 1) + '/' + what, {cache: 'no-store'});
//! ```
//!
//! — and [`Said`] keeps each by its number. Every report with an even
//! number is **held back** before it is kept ([`HELD_BACK`]), so the order
//! reports are kept in is never the page's, in any run and under any load:
//! a reading that leant on it would fail every time, not one gate in many.

use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How long a report with an even number waits before it is kept: longer
/// than a page takes between two keys' reports, so the next one, sent
/// after it, is kept before it.
pub const HELD_BACK: Duration = Duration::from_millis(150);

/// Every report the page told the server, each with its number.
#[derive(Clone, Default)]
pub struct Said(Arc<Mutex<Vec<(u64, String)>>>);

impl Said {
    /// Keep the report a `/said/` path carries (`N/what`, without the
    /// `/said/`), holding back one with an even number first. A path with
    /// no number is not one of the page's reports and is not kept.
    pub fn keep(&self, path: &str) {
        let Some((number, what)) = path
            .split_once('/')
            .and_then(|(number, what)| Some((number.parse::<u64>().ok()?, what)))
        else {
            return;
        };
        if number % 2 == 0 {
            std::thread::sleep(HELD_BACK);
        }
        if let Ok(mut kept) = self.0.lock() {
            kept.push((number, what.to_owned()));
        }
    }

    /// Every report kept so far, in the order the page told them.
    pub fn in_order(&self) -> Vec<String> {
        let mut kept = self.0.lock().map(|kept| kept.clone()).unwrap_or_default();
        kept.sort_by_key(|(number, _)| *number);
        kept.into_iter().map(|(_, what)| what).collect()
    }

    /// Whether the page has told `wanted`.
    pub fn has(&self, wanted: &str) -> bool {
        self.in_order().iter().any(|each| each == wanted)
    }

    /// The reports that begin `kind`, in the order the page told them.
    pub fn of(&self, kind: &str) -> Vec<String> {
        self.in_order()
            .into_iter()
            .filter(|each| each.starts_with(kind))
            .collect()
    }
}
