/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The box an agent's verb acted on is marked over a page by the Chromium
//! alo carries, and seen on the screen (queue item 726, ADR 0066, ADR 0079
//! § 8): `the_box_marked_through_chrome.rs`'s check, run on a shown CEF
//! host started as the application starts it, in place of the machine's
//! Chrome. That file says what is seen and why.
//!
//! # When there is no CEF distribution here
//!
//! As `tests/alos_cef_host_driven_as_chrome.rs`: with `CEF_PATH` unset, or
//! not naming an unpacked distribution, the test says why on its error
//! output and returns.
//!
//! One test, alone in its file: a shown Chromium brought in front sends any
//! other test's window into Stage Manager's strip (item 643).

#![cfg(target_os = "macos")]

mod carried;
mod marked;

#[test]
fn the_box_a_verb_acted_on_is_marked_by_alos_cef_host_on_the_screen() -> Result<(), String> {
    let Some(host) = carried::host("726")? else {
        return Ok(());
    };
    marked::on_the_screen(host.chromium())
}
