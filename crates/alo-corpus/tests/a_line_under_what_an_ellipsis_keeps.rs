/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A line ended in "…" is underlined as far as its last kept letter, and not
//! under the space after it: the case that closes queue item 583.
//!
//! `cases/text-overflow-ellipsis`' `#underlined` is `aaaa bbbbbbbbb` in
//! 61 px. "aaaa " is kept and the "…" stands after the space, and Chrome
//! 154 draws the underline under "aaaa" alone, 8 to 47, where this engine
//! drew it to 52 by counting the kept space.
//!
//! `chrome.png` beside the case is Chrome 154's screenshot of the same bytes,
//! headless, started with `--force-device-scale-factor=1`, without
//! `--disable-gpu`, every element set in `DejaVu Sans` (regular and bold) by
//! `@font-face` as the corpus's fonts are, the root 200 px wide as the case's
//! viewport is, taken 2026-10-14 and cut to the case's size. Lines are read
//! and compared as `alo_corpus::decoration_rows` reads them.

use alo_corpus::decoration_rows::{compare, lines};
use alo_corpus::{Case, cases_directory, corpus_fonts};
use alo_layout::Size;
use alo_renderer::{Screen, render_at};

#[test]
fn the_underline_under_what_an_ellipsis_keeps_is_chromes() {
    let Some(case) = Case::read(&cases_directory().join("text-overflow-ellipsis")) else {
        panic!("the case reads");
    };
    let Some(chrome) = std::fs::read(case.expectation("chrome.png"))
        .ok()
        .and_then(|bytes| alo_paint::from_png(&bytes).ok())
    else {
        panic!("Chrome's chrome.png is beside the case");
    };
    let screen = Screen::new(Size::new(case.size.0, case.size.1));
    let drawing = render_at(
        &case.html,
        &case.css,
        screen,
        &corpus_fonts(),
        &case.linked,
        &case.resources,
    )
    .drawing;
    assert_eq!(
        (drawing.canvas.width(), drawing.canvas.height()),
        (chrome.width(), chrome.height()),
        "chrome.png is the size of the picture",
    );
    let (theirs, ours) = (lines(&chrome, 12), lines(&drawing.canvas, 12));
    // One line in the case, `#underlined`'s, on one row: under "aaaa",
    // never the space the "…" stands after.
    assert_eq!(
        theirs.values().collect::<Vec<_>>(),
        vec![&vec![(8, 47)]],
        "Chrome's rows: {theirs:?}"
    );
    if let Err(why) = compare(&theirs, &ours) {
        panic!("{why}");
    }
}
