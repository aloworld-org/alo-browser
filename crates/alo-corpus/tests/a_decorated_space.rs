/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A space that is a text box of its own, or a piece of one with no letters,
//! is decorated as Chrome decorates it: the case that closes queue item 582.
//!
//! `cases/decorated-spaces` holds the space between two `<a>`s under an
//! underline its paragraph declares, under one a `<span>` declares (and the
//! space after that span, outside it, bare), under an overline and a line
//! through, the space a `<span>`'s own text begins with, a narrow
//! paragraph whose first line ends in such a space, where it hangs bare, and
//! two lines ended in an ellipsis: one cut inside `two`, the space before it
//! underlined, and one cut where `two` ends, the space after it bare though
//! Chrome's "…" stands after it.
//!
//! `chrome.png` and `chrome-at-2.png` beside it are Chrome 154's screenshots
//! of the same bytes, headless, started with `--force-device-scale-factor=1`
//! and `=2` (never the emulated scale of Chrome's developer tools), without
//! `--disable-gpu`, `DejaVu Sans` given by `@font-face` and the root 300 px
//! wide as the case's viewport is, taken 2026-10-14 and cut to the case's
//! size. Lines are read and compared as `alo_corpus::decoration_rows` reads
//! them: every line's every piece is Chrome's, within a device pixel.
//!
//! `render-at-2.png` is this engine's picture at two, a reference beside
//! `render.png` and never in its place (ADR 0057 § 5).

use alo_corpus::decoration_rows::{Lines, compare, lines};
use alo_corpus::{Case, cases_directory, check_picture, corpus_fonts};
use alo_layout::Size;
use alo_paint::Canvas;
use alo_renderer::{Drawing, Screen, render_at};
use std::rc::Rc;

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("decorated-spaces"))
}

/// The case drawn on a screen at `scale`.
fn drawn(case: &Case, scale: f32) -> Rc<Drawing> {
    let screen = Screen::new(Size::new(case.size.0, case.size.1)).at_scale(scale);
    render_at(
        &case.html,
        &case.css,
        screen,
        &corpus_fonts(),
        &case.linked,
        &case.resources,
    )
    .drawing
}

fn chrome(case: &Case, name: &str) -> Option<Canvas> {
    let bytes = std::fs::read(case.expectation(name)).ok()?;
    alo_paint::from_png(&bytes).ok()
}

/// Chrome's lines and this engine's, at `scale`.
fn both(scale: u32, reference: &str) -> Result<(Lines, Lines), String> {
    let case = case().ok_or("the case reads")?;
    let chrome =
        chrome(&case, reference).ok_or(format!("Chrome's {reference} is beside the case"))?;
    #[expect(clippy::cast_precision_loss, reason = "one or two")]
    let drawing = drawn(&case, scale as f32);
    if (drawing.canvas.width(), drawing.canvas.height()) != (chrome.width(), chrome.height()) {
        return Err(format!(
            "{reference} is not the size of the picture at {scale}"
        ));
    }
    Ok((
        lines(&chrome, 12 * scale),
        lines(&drawing.canvas, 12 * scale),
    ))
}

/// The rows that hold one piece of line and no more. No letter in the case
/// has a descender to step around, so a row of two pieces is a line broken
/// at a space.
fn single_runs(lines: &Lines) -> Vec<(u32, (u32, u32))> {
    lines
        .iter()
        .filter_map(|(row, pieces)| match pieces.as_slice() {
            [only] => Some((*row, *only)),
            _ => None,
        })
        .collect()
}

#[test]
fn at_one_every_space_is_decorated_as_chromes_is() {
    let (theirs, ours) = match both(1, "chrome.png") {
        Ok(lines) => lines,
        Err(why) => panic!("{why}"),
    };
    // Nine lines, one a row at 14 px: one under each of the first two
    // paragraphs, over the third, through the fourth, under the fifth, under
    // each of the narrow paragraph's two lines, and under what each ended
    // line keeps. Every one is a single piece running across its spaces.
    assert_eq!(theirs.len(), 9, "Chrome's rows: {theirs:?}");
    assert_eq!(single_runs(&theirs).len(), 9, "Chrome's rows: {theirs:?}");
    if let Err(why) = compare(&theirs, &ours) {
        panic!("{why}");
    }
}

#[test]
fn at_two_every_space_is_decorated_as_chromes_is() {
    let (theirs, ours) = match both(2, "chrome-at-2.png") {
        Ok(lines) => lines,
        Err(why) => panic!("{why}"),
    };
    // The same nine lines, each two device rows thick.
    assert_eq!(theirs.len(), 18, "Chrome's rows: {theirs:?}");
    if let Err(why) = compare(&theirs, &ours) {
        panic!("{why}");
    }
}

#[test]
fn the_picture_at_two_is_its_reference() {
    let Some(case) = case() else {
        panic!("the case reads");
    };
    let drawing = drawn(&case, 2.0);
    let difference = check_picture(&case, "render-at-2.png", &drawing.canvas);
    assert!(difference.is_none(), "{difference:?}");
}
