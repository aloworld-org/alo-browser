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
//! CSS pixels or more, so a row with such a run is a line's row, and each
//! such run is a piece of a line. An underline steps aside where a letter's
//! ink crosses it (`text-decoration-skip-ink`, queue item 579), so a line is
//! several pieces, and they are compared piece by piece. What a letter
//! inks inside a gap is never as long: two device pixels at one and ten at
//! two in Chrome's pictures, where the shortest piece of a line is 26 and
//! 51.
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

/// Each row holding a run of ink at least `run` device pixels long, and
/// every such run on it: where it starts and ends.
fn lines(canvas: &Canvas, run: u32) -> Lines {
    let mut rows: Lines = BTreeMap::new();
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
                rows.entry(y).or_default().push((start, x));
            }
        }
    }
    rows
}

/// The pieces of the lines in a picture: each row a line is on, and where
/// each of its pieces starts and ends.
type Lines = BTreeMap<u32, Vec<(u32, u32)>>;

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

/// Our pieces with one gap Chrome does not leave taken away: the two pieces
/// either side of it on each of `rows`, which must be exactly there, made
/// one. What is joined is said by name, so that the day the gap closes this
/// fails and the exception goes.
fn joined(mut ours: Lines, rows: &[u32], gap: (u32, u32)) -> Result<Lines, String> {
    for row in rows {
        let pieces = ours.get_mut(row).ok_or(format!("row {row} has no line"))?;
        let at = pieces
            .windows(2)
            .position(|pair| matches!(pair, [left, right] if (left.1, right.0) == gap))
            .ok_or(format!(
                "row {row}: the gap {gap:?} is not ours: {pieces:?}"
            ))?;
        let end = pieces.remove(at + 1).1;
        if let Some(piece) = pieces.get_mut(at) {
            piece.1 = end;
        }
    }
    Ok(ours)
}

fn compare(theirs: &Lines, ours: &Lines, rows: usize) {
    assert_eq!(theirs.len(), rows, "Chrome's rows: {theirs:?}");
    assert_eq!(
        ours.keys().collect::<Vec<_>>(),
        theirs.keys().collect::<Vec<_>>(),
        "every line on Chrome's rows, as thick"
    );
    for ((row, their_pieces), (_, our_pieces)) in theirs.iter().zip(ours) {
        assert_eq!(
            their_pieces.len(),
            our_pieces.len(),
            "row {row}: Chrome's pieces {their_pieces:?}, ours {our_pieces:?}"
        );
        for ((start, end), (our_start, our_end)) in their_pieces.iter().zip(our_pieces) {
            assert!(
                start.abs_diff(*our_start) <= 1 && end.abs_diff(*our_end) <= 1,
                "row {row}: Chrome's piece runs {start}..{end}, ours {our_start}..{our_end}"
            );
        }
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
    // the other four. The space after `with`, before the black child, is
    // not underlined here and is in Chrome: queue item 580.
    match joined(ours, &[200], (169, 174)) {
        Ok(ours) => compare(&theirs, &ours, 11),
        Err(why) => panic!("{why}"),
    }
}

#[test]
fn at_two_each_line_is_on_chromes_device_rows() {
    let (theirs, ours) = match both(2, "chrome-at-2.png") {
        Ok(lines) => lines,
        Err(why) => panic!("{why}"),
    };
    // Ten, of two rows each: a line at two stands taller than at one (ADR
    // 0057, Evidence 1), and the last underline is below the case's 220 px,
    // in Chrome's picture as in ours. The space before the black child is
    // queue item 580, as at one.
    match joined(ours, &[409, 410], (338, 348)) {
        Ok(ours) => compare(&theirs, &ours, 20),
        Err(why) => panic!("{why}"),
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
