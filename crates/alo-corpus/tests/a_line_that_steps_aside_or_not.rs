/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `text-decoration-skip-ink`'s three values, drawn as Chrome draws them:
//! the case that closes queue item 581.
//!
//! `cases/skip-ink-values` holds `jumpy gypsy` at 20 px underlined under
//! `auto`, `none` and `all` (written `ALL`), struck through under `all`, a
//! `none` underline over a child set `auto` and an `auto` one over a child
//! set `none` (the text's own value decides), a child set `bogus` and one
//! set `initial` under `none` (the first keeps the `none` it inherits, the
//! second is `auto`), a paragraph set `none auto` (no value, so `auto`), a
//! `none` inherited from a `<div>` around the element asking for the line,
//! and `ÉÅÎ` at 64 px overlined under each value, where the accents cross
//! the line. The lines are red and the letters blue.
//!
//! `chrome.png` and `chrome-at-2.png` beside it are Chrome 154's screenshots
//! of the same bytes, headless, started with `--force-device-scale-factor=1`
//! and `=2` (never the emulated scale of Chrome's developer tools), without
//! `--disable-gpu`, `DejaVu Sans` given by `@font-face` and the root 300 px
//! wide as the case's viewport is, taken 2026-10-14 and cut to the case's
//! size.
//!
//! A line's rows are the rows holding twelve CSS pixels of its red or more;
//! on them, its pieces are read as `alo_corpus::decoration_rows` reads them,
//! letters and line alike as ink, so a line that runs whole under the
//! letters is one piece and a line that steps aside is several. Where a line
//! runs whole, the letters' blue left on its rows says what is on top.
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
    Case::read(&cases_directory().join("skip-ink-values"))
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

/// Whether a pixel is the lines' red, `#cc0000`, and not a blend of it with
/// a letter's blue or the page's white.
fn red(canvas: &Canvas, x: u32, y: u32) -> bool {
    canvas.at(x, y).is_some_and(|pixel| {
        let (red, green, blue, _) = pixel.to_rgba8();
        red > 150 && green < 100 && blue < 100
    })
}

/// Every run of red on each row holding `least` red pixels or more.
fn red_runs(canvas: &Canvas, least: u32) -> Lines {
    let mut rows = Lines::new();
    for y in 0..canvas.height() {
        let mut runs = Vec::new();
        let mut x = 0;
        while x < canvas.width() {
            if !red(canvas, x, y) {
                x += 1;
                continue;
            }
            let start = x;
            while x < canvas.width() && red(canvas, x, y) {
                x += 1;
            }
            runs.push((start, x));
        }
        if runs.iter().map(|(start, end)| end - start).sum::<u32>() >= least {
            rows.insert(y, runs);
        }
    }
    rows
}

/// A picture's lines, read on their rows: the pieces of ink twelve CSS
/// pixels long or more on each row twelve CSS pixels of red are on.
fn lines_of(canvas: &Canvas, scale: u32) -> Lines {
    let rows = red_runs(canvas, 12 * scale);
    lines(canvas, 12 * scale)
        .into_iter()
        .filter(|(row, _)| rows.contains_key(row))
        .collect()
}

/// Chrome's picture and this engine's, at `scale`.
fn both(scale: u32, reference: &str) -> Result<(Canvas, Canvas), String> {
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
    Ok((chrome, drawing.canvas.clone()))
}

/// The rows of `theirs` on which a line runs whole across the paragraph:
/// one piece, from its start at 8 CSS pixels (a `j`'s tail may reach a
/// device pixel before it) to at least 100.
fn whole_rows(theirs: &Lines, scale: u32) -> Vec<u32> {
    theirs
        .iter()
        .filter(|(_, pieces)| {
            matches!(pieces.as_slice(), [(start, end)]
                if start.abs_diff(8 * scale) <= scale && *end >= 100 * scale)
        })
        .map(|(row, _)| *row)
        .collect()
}

#[test]
fn at_one_each_value_steps_aside_or_not_as_chromes_does() {
    let (chrome, ours) = match both(1, "chrome.png") {
        Ok(pictures) => pictures,
        Err(why) => panic!("{why}"),
    };
    let (theirs, ours) = (lines_of(&chrome, 1), lines_of(&ours, 1));
    // Ten lines two rows thick at 20 px and three six rows thick at 64 px.
    assert_eq!(theirs.len(), 38, "Chrome's rows: {theirs:?}");
    // Whole: the two `none` underlines, the line through under `all`, the
    // `none` from the `<div>`, and the `none` overline.
    assert_eq!(
        whole_rows(&theirs, 1).len(),
        14,
        "Chrome's rows: {theirs:?}"
    );
    if let Err(why) = compare(&theirs, &ours) {
        panic!("{why}");
    }
}

#[test]
fn at_two_each_value_steps_aside_or_not_as_chromes_does() {
    let (chrome, ours) = match both(2, "chrome-at-2.png") {
        Ok(pictures) => pictures,
        Err(why) => panic!("{why}"),
    };
    let (theirs, ours) = (lines_of(&chrome, 2), lines_of(&ours, 2));
    assert_eq!(theirs.len(), 76, "Chrome's rows: {theirs:?}");
    assert_eq!(
        whole_rows(&theirs, 2).len(),
        28,
        "Chrome's rows: {theirs:?}"
    );
    if let Err(why) = compare(&theirs, &ours) {
        panic!("{why}");
    }
}

/// How many of a row's pixels are a letter's blue rather than the line's
/// red, from the paragraphs' start.
fn letter_pixels(canvas: &Canvas, row: u32, scale: u32) -> usize {
    (8 * scale..canvas.width())
        .filter(|x| {
            canvas.at(*x, row).is_some_and(|pixel| {
                let (red, _, blue, _) = pixel.to_rgba8();
                blue > red
            })
        })
        .count()
}

#[test]
fn a_letter_is_on_top_of_an_underline_or_overline_and_under_a_line_through() {
    for (scale, reference) in [(1, "chrome.png"), (2, "chrome-at-2.png")] {
        let (chrome, ours) = match both(scale, reference) {
            Ok(pictures) => pictures,
            Err(why) => panic!("{why}"),
        };
        let rows = whole_rows(&lines_of(&chrome, scale), scale);
        // Drawn over the letters, a line leaves none of their blue on its
        // rows; drawn under them, it leaves whatever of them crosses it.
        // Chrome's letters are thicker than this engine's (queue item 576),
        // so as many is not asked, only some wherever Chrome has some, and
        // at least half as many.
        let counts: Vec<_> = rows
            .iter()
            .map(|row| {
                (
                    *row,
                    letter_pixels(&chrome, *row, scale),
                    letter_pixels(&ours, *row, scale),
                )
            })
            .collect();
        let on_top = counts.iter().filter(|(_, theirs, _)| *theirs > 0).count();
        let under = counts.iter().filter(|(_, theirs, _)| *theirs == 0).count();
        assert!(on_top > 0 && under > 0, "at {scale}: {counts:?}");
        // At one, Chrome pulls the 64 px capitals' accents up onto a whole
        // pixel, into the overline's last row, where this engine's stop
        // short of it (queue item 577): the letters' place, not the line's.
        // It is the one row excused, and it is held to being that, so that
        // closing 577 makes this test say so.
        let excused = (scale == 1).then(|| rows.last().copied()).flatten();
        for (row, theirs, ours) in &counts {
            if Some(*row) == excused {
                assert!(
                    *theirs > 0 && *ours == 0,
                    "at {scale}, row {row}: Chrome's {theirs}, ours {ours}"
                );
                continue;
            }
            assert!(
                (*theirs == 0) == (*ours == 0) && *ours * 2 >= *theirs,
                "at {scale}, row {row}: Chrome's letters on the line {theirs}, ours {ours}"
            );
        }
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
