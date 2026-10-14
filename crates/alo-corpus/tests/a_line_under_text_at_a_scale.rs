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
//! A decoration is the only thing on its row that runs unbroken for twelve
//! CSS pixels or more, so a row with such a run is a line's row. Chrome
//! leaves a gap where a descender crosses an underline, which this engine
//! does not (`text-decoration-skip-ink`), so a row's runs are compared by
//! where they start and end, not one by one.
//!
//! `render-at-2.png` is this engine's picture at two, a reference beside
//! `render.png` and never in its place (ADR 0057 § 5).

use alo_corpus::{Case, cases_directory, check_picture, corpus_fonts};
use alo_layout::Size;
use alo_paint::Canvas;
use alo_renderer::{Drawing, Screen, render_at};
use std::collections::BTreeMap;
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

/// Whether a pixel is ink: black or the case's red, not the page's white
/// and not a letter's soft edge.
fn ink(canvas: &Canvas, x: u32, y: u32) -> bool {
    canvas.at(x, y).is_some_and(|pixel| {
        let (red, green, blue, _) = pixel.to_rgba8();
        red.min(green).min(blue) < 100
    })
}

/// Each row holding a run of ink at least `run` device pixels long: where
/// its first such run starts and its last ends.
fn lines(canvas: &Canvas, run: u32) -> Lines {
    let mut rows = BTreeMap::new();
    for y in 0..canvas.height() {
        let mut x = 0;
        while x < canvas.width() {
            if !ink(canvas, x, y) {
                x += 1;
                continue;
            }
            let start = x;
            while x < canvas.width() && ink(canvas, x, y) {
                x += 1;
            }
            if x - start >= run {
                rows.entry(y)
                    .and_modify(|(_, end)| *end = x)
                    .or_insert((start, x));
            }
        }
    }
    rows
}

/// Chrome's lines and ours at `scale`: each a row and where its line
/// starts and ends, Chrome's first.
type Lines = BTreeMap<u32, (u32, u32)>;

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
    assert_eq!(
        ours.keys().collect::<Vec<_>>(),
        theirs.keys().collect::<Vec<_>>(),
        "every line on Chrome's rows, as thick"
    );
    for ((row, (start, end)), (_, (our_start, our_end))) in theirs.iter().zip(ours) {
        assert!(
            start.abs_diff(*our_start) <= 1 && end.abs_diff(*our_end) <= 1,
            "row {row}: Chrome's line runs {start}..{end}, ours {our_start}..{our_end}"
        );
    }
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
    // the other four.
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
    // in Chrome's picture as in ours.
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
