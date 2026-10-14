/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Decoration lines under text that casts a shadow, drawn as Chrome draws
//! them: the case that closes queue item 584.
//!
//! `cases/text-shadow-on-lines` holds `jumpy gypsy` at 20 px, its lines red
//! and its letters blue, under green and purple shadows: an underline that
//! steps aside under a shadow 4 px below, one drawn whole under a shadow
//! 6 px above (the line's shadow crosses the letters), a line through and
//! an overline under a shadow 4 px below, an underline under two shadows,
//! one under a blurred shadow, a `<u>` with no shadow over a child that
//! casts one and a `<u>` that casts one over a child set `none`, an
//! underline running under a space between two `<span>`s, and an underline
//! and an overline whose shadows, 23 px below, reach the other line and the
//! next paragraph's.
//!
//! Chrome 154 draws, in order: the letters' shadows, the shadows of every
//! underline and overline piece, those lines, the letters, the line
//! through's shadows and the line through; each line casts the text's own
//! `text-shadow`, as the piece is drawn, moved by its offset.
//!
//! `chrome.png` and `chrome-at-2.png` beside it are Chrome 154's screenshots
//! of the same bytes, headless, started with `--force-device-scale-factor=1`
//! and `=2`, without `--disable-gpu`, `DejaVu Sans` given by `@font-face`
//! and the root 300 px wide as the case's viewport is, taken 2026-10-14 and
//! cut to the case's size.
//!
//! A pixel is read as one of the case's plain colours — white, the letters'
//! blue, the lines' red, a shadow's green or purple — or as a blend of
//! them, which says nothing. Wherever both pictures have a plain colour, it
//! is the same one at one: so every line, shadow and letter is where
//! Chrome's is and on top of what Chrome's is on top of. At two, a pixel may
//! differ only on an edge, each picture's colour a device pixel away in the
//! other, as single pixels at the letters' edges are, which Chrome thickens
//! (queue items 576 and 577), and only as many as were counted.
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
    Case::read(&cases_directory().join("text-shadow-on-lines"))
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

/// Which of the case's plain colours a pixel is, or [`None`] for a blend.
fn plain(canvas: &Canvas, x: u32, y: u32) -> Option<char> {
    let (red, green, blue, _) = canvas.at(x, y)?.to_rgba8();
    match (red, green, blue) {
        (201.., 201.., 201..) => Some('W'),
        (..70, ..70, 151..) => Some('B'),
        (151.., ..70, ..70) => Some('R'),
        (..70, 131.., ..70) => Some('G'),
        (131.., ..70, 131..) => Some('P'),
        _ => None,
    }
}

/// Chrome's picture and this engine's, at `scale`.
fn both(scale: u32, reference: &str) -> Result<(Canvas, Canvas), String> {
    let case = case().ok_or("the case reads")?;
    let bytes = std::fs::read(case.expectation(reference))
        .map_err(|_| format!("Chrome's {reference} is beside the case"))?;
    let chrome = alo_paint::from_png(&bytes).map_err(|why| format!("{reference}: {why}"))?;
    #[expect(clippy::cast_precision_loss, reason = "one or two")]
    let drawing = drawn(&case, scale as f32);
    if (drawing.canvas.width(), drawing.canvas.height()) != (chrome.width(), chrome.height()) {
        return Err(format!(
            "{reference} is not the size of the picture at {scale}"
        ));
    }
    Ok((chrome, drawing.canvas.clone()))
}

/// Whether `canvas` has `colour` plainly at a pixel next to `(x, y)`, sides
/// and corners.
fn beside(canvas: &Canvas, x: u32, y: u32, colour: char) -> bool {
    (x.saturating_sub(1)..=x + 1)
        .flat_map(|x| (y.saturating_sub(1)..=y + 1).map(move |y| (x, y)))
        .any(|(x, y)| plain(canvas, x, y) == Some(colour))
}

/// What the two pictures say: how many pixels of each plain colour
/// Chrome's has, how many are one plain colour in Chrome's and another in
/// ours on an edge — each picture's colour a pixel away in the other, so
/// one picture's edge is a device pixel from the other's — and the rest,
/// each said as where and which colours.
struct Reading {
    theirs: BTreeMap<char, usize>,
    beside: usize,
    elsewhere: Vec<(u32, u32, char, char)>,
}

fn read(chrome: &Canvas, ours: &Canvas) -> Reading {
    let mut reading = Reading {
        theirs: BTreeMap::new(),
        beside: 0,
        elsewhere: Vec::new(),
    };
    for y in 0..chrome.height() {
        for x in 0..chrome.width() {
            let (a, b) = (plain(chrome, x, y), plain(ours, x, y));
            if let Some(a) = a {
                *reading.theirs.entry(a).or_insert(0) += 1;
            }
            if let (Some(a), Some(b)) = (a, b)
                && a != b
            {
                if beside(ours, x, y, a) || beside(chrome, x, y, b) {
                    reading.beside += 1;
                } else {
                    reading.elsewhere.push((x, y, a, b));
                }
            }
        }
    }
    reading
}

#[test]
fn every_plain_pixel_is_chromes_at_one() {
    let (chrome, ours) = match both(1, "chrome.png") {
        Ok(pictures) => pictures,
        Err(why) => panic!("{why}"),
    };
    let reading = read(&chrome, &ours);
    // The case is what it says: every colour is drawn plainly somewhere.
    assert!(
        ['W', 'B', 'R', 'G', 'P']
            .iter()
            .all(|colour| reading.theirs.get(colour).is_some_and(|count| *count > 0)),
        "{:?}",
        reading.theirs
    );
    assert!(
        reading.beside == 0 && reading.elsewhere.is_empty(),
        "{} beside, {:?}",
        reading.beside,
        reading.elsewhere
    );
}

#[test]
fn at_two_a_plain_pixel_is_chromes_or_a_letters_edge_beside_it() {
    let (chrome, ours) = match both(2, "chrome-at-2.png") {
        Ok(pictures) => pictures,
        Err(why) => panic!("{why}"),
    };
    let reading = read(&chrome, &ours);
    assert!(
        ['W', 'B', 'R', 'G', 'P']
            .iter()
            .all(|colour| reading.theirs.get(colour).is_some_and(|count| *count > 0)),
        "{:?}",
        reading.theirs
    );
    // Taken 2026-10-14: 155 single pixels at the edges of letters and of
    // the letters' shadows, which are copies of them, where Chrome's
    // thickened letters (queue item 576), and their tops pulled onto a
    // whole pixel (577), reach a pixel this engine's leave white or to
    // another colour. A line or a line's shadow a device pixel out of place
    // would be hundreds.
    assert!(reading.beside <= 160, "{} beside", reading.beside);
    assert!(reading.elsewhere.is_empty(), "{:?}", reading.elsewhere);
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
