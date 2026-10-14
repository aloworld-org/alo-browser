/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A line under, over and through text, drawn where Chrome draws it at one
//! and at two: the case that closes queue item 566.
//!
//! `chrome.png` and `chrome-at-2.png` beside `cases/text-decorations` are
//! Chrome 154's screenshots of the same bytes, headless, started with
//! `--force-device-scale-factor=1` and `=2` (never the emulated scale of
//! Chrome's developer tools), without `--disable-gpu`, `DejaVu Sans` given
//! by `@font-face` and the root 300 px wide as the case's viewport is,
//! taken 2026-10-14 and cut to the case's size.
//!
//! Lines are read and compared as `alo_corpus::decoration_rows` reads them:
//! a row with a run of ink twelve CSS pixels long is a line's row, and each
//! such run a piece of it. What a letter inks inside a gap is never as long
//! here: two device pixels at one and ten at two in Chrome's pictures, where
//! the shortest piece of a line is 26 and 51.
//!
//! `render-at-2.png` is this engine's picture at two, a reference beside
//! `render.png` and never in its place (ADR 0057 § 5).

use alo_corpus::decoration_rows::{self, Lines, lines};
use alo_corpus::{Case, cases_directory, check_picture, corpus_fonts};
use alo_layout::Size;
use alo_paint::Canvas;
use alo_renderer::{Drawing, Screen, render_at};
use std::rc::Rc;

fn case() -> Option<Case> {
    Case::read(&cases_directory().join("text-decorations"))
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

fn compare(theirs: &Lines, ours: &Lines, rows: usize) {
    assert_eq!(theirs.len(), rows, "Chrome's rows: {theirs:?}");
    let compared = decoration_rows::compare(theirs, ours);
    assert!(compared.is_ok(), "{compared:?}");
}

// At 14 px a tenth of the size, cut down, is one device pixel per CSS
// pixel, so each line is `scale` rows.

#[test]
fn at_one_each_line_is_on_chromes_rows() {
    let (theirs, ours) = match both(1, "chrome.png") {
        Ok(lines) => lines,
        Err(why) => panic!("{why}"),
    };
    // Eleven lines: one each on the first three paragraphs, two on each of
    // the other four. The space after `with`, before the black child, is
    // underlined, as it is in Chrome; the one `wrap` ends its first line in
    // is not (queue item 580).
    compare(&theirs, &ours, 11);
}

#[test]
fn at_two_each_line_is_on_chromes_device_rows() {
    let (theirs, ours) = match both(2, "chrome-at-2.png") {
        Ok(lines) => lines,
        Err(why) => panic!("{why}"),
    };
    // Ten, of two rows each: a line at two stands taller than at one (ADR
    // 0057, Evidence 1), and the last underline is below the case's 220 px,
    // in Chrome's picture as in ours. The spaces are as at one.
    compare(&theirs, &ours, 20);
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
