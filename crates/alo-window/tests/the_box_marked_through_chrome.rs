/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The box an agent's verb acted on is marked over a page in Chrome, by
//! Chrome, and seen on the screen (queue item 680, ADR 0066).
//!
//! Chrome's own highlight is drawn in its compositor and is not in its own
//! picture of the page (`Page.captureScreenshot` has no pixel of it, ADR
//! 0066 § 1), so this test reads the screen: Chrome's window, by the window
//! server's number for it, captured with `screencapture -l` in its own
//! device pixels, title bar and all, whatever is in front of it.
//!
//! A shown Chrome — placed as `alo --chrome` places it — holds a frozen page
//! of four bordered boxes and a swatch filled with alo's accent, the colour
//! the mark's border is asked in: the swatch is what that colour is on this
//! screen, so the mark is compared with it rather than with a number a
//! screen's colour profile would move. An agent in a process of its own,
//! `alo-ask`, reads the page and acts on it, and after each answer the
//! window is captured until it shows what the answer should leave:
//!
//! - a verb that ran marks its box — each side of its border the accent —
//!   and Chrome's label is beside it, dark letters where the page has none;
//! - the next verb's mark replaces the last, which goes;
//! - a refused verb leaves no mark and no label;
//! - a page that goes to another document on its own, a moment after a verb
//!   marked a box in it, is left with no mark: measured here, because ADR
//!   0066 left it open;
//! - a verb that follows a link in place leaves no mark on the page it went
//!   to.
//!
//! The page counts its own loads in `sessionStorage` and widens the swatch
//! by 20 CSS pixels for each, so the capture says which document it shows.
//!
//! One test, alone in its file: a shown Chrome brought in front sends any
//! other test's Chrome window into Stage Manager's strip (item 643).

mod marked;

use alo_window::beside::Chromium;

#[test]
fn the_box_a_verb_acted_on_is_marked_by_chrome_on_the_screen() -> Result<(), String> {
    marked::on_the_screen(Chromium::Machines)
}
