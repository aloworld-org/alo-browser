/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 752: a variable font's optical size set by the size its text
//! is set at. Newsreader, the interface's serif (ADR 0084), varies along
//! `opsz` from 6 to 72, its own default 18; with `font-optical-sizing:
//! auto` — the initial value — a run at 40 px is measured, shaped and
//! outlined at `opsz` 40, at 90 px at 72 (the axis's end), and with `none`
//! at 18 whatever its size.
//!
//! Every Chrome number here was measured on this Mac on 1 November 2026 in
//! Chrome 156.0.8078.76, headless, from the same `Newsreader.ttf` as a web
//! font (`@font-face { font-weight: 200 800 }`), after
//! `document.fonts.load` resolved: a run's width as a `white-space: pre`
//! span's `getBoundingClientRect().width` (Chrome's layout unit, 1/64), a
//! glyph's advance as a canvas's `measureText` (no layout unit). Chrome's
//! source was not read.
//!
//! What closes it, each a test below: each glyph's advance at `opsz` 40 is
//! Chrome's to half a font unit, and at 72 exactly; a run set with `none` is
//! Chrome's `none` to its layout unit at every size; a run set with `auto`
//! is Chrome's `auto` to its layout unit where no kerning pair in it varies
//! along `opsz`; a page's `font-optical-sizing: none` read, its boxes in
//! numbers; a reference render, `references/a-typeface-at-its-optical-size.png`.
//!
//! **What still differs, and why** (cut to queue items 795, 796 and 797):
//! a line's words are measured apart, losing the kerning across a space
//! (797, older than this item and at every optical size); and the
//! shaper this engine rents, `rustybuzz` 0.20.1, gives a varied glyph's
//! advance in whole font units, where Chrome keeps the fraction (≤ 0.01 px
//! a glyph at 40 px); and it applies a kerning pair's value without the
//! pair's own variation along the axis, which HarfBuzz 11.2.1 (`hb-shape
//! --variations=opsz=72`) and Chrome apply — `iv` is kerned −14 units at
//! 18 and 0 at 72, and this engine kerns it −14 at both.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_typeface_at_its_optical_size`
//! rewrites the reference. A rewritten reference is a diff somebody reads.

use alo_layout::Size;
use alo_paint::{Canvas, from_png, to_png};
use alo_renderer::Screen;
use alo_renderer::typefaces::{self, NEWSREADER};
use alo_text::{Direction, FontDatabase, FontRequest, OpticalSizing, Weight};
use std::path::{Path, PathBuf};

/// The interface's typefaces, as a renderer is handed them.
fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for font in typefaces::fonts() {
        database.add(font);
    }
    database
}

/// Newsreader at `weight`, its optical size following its size or not.
fn newsreader(weight: u16, sizing: OpticalSizing) -> FontRequest {
    FontRequest {
        families: vec![NEWSREADER.to_owned()],
        weight: Weight::new(weight),
        optical_sizing: sizing,
        ..FontRequest::default()
    }
}

/// How wide `text` is at `size` as this engine shapes it.
fn shaped(text: &str, size: f32, request: &FontRequest) -> Option<f32> {
    let database = fonts();
    let font = database.chain(request).into_iter().next()?;
    Some(alo_text::shape::shape(text, &font, size, Direction::LeftToRight).width)
}

/// Chrome 156's canvas advance of each glyph of the greeting at 40 px,
/// `font-optical-sizing: auto`, weight 400.
const AT_40: [(char, f32); 13] = [
    ('G', 29.842_224),
    ('o', 21.231_11),
    ('d', 22.274_81),
    (' ', 9.042_953),
    ('a', 18.534_82),
    ('f', 13.481_476),
    ('t', 13.511_841),
    ('e', 18.451_843),
    ('r', 15.458_511),
    ('n', 22.690_369),
    (',', 9.293_32),
    ('A', 28.143_707),
    ('.', 9.011_841),
];

/// The same at 72 px, the axis's end and one of the font's own masters.
const AT_72: [(char, f32); 6] = [
    ('G', 55.187_99),
    ('o', 40.967_99),
    ('a', 36.647_99),
    ('n', 42.911_99),
    ('A', 52.343_99),
    ('l', 19.439_99),
];

#[test]
fn each_glyphs_advance_at_opsz_40_and_72_is_chromes() {
    let auto = newsreader(400, OpticalSizing::Auto);
    // Half of Newsreader's 2,000-unit em's unit, at 40 px.
    let half_a_unit = 0.5 * 40.0 / 2000.0;
    for (glyph, chrome) in AT_40 {
        let ours = shaped(&glyph.to_string(), 40.0, &auto).unwrap_or(0.0);
        assert!(
            (ours - chrome).abs() <= half_a_unit + 1e-4,
            "{glyph:?} at 40: {ours} where Chrome's is {chrome}"
        );
    }
    for (glyph, chrome) in AT_72 {
        let ours = shaped(&glyph.to_string(), 72.0, &auto).unwrap_or(0.0);
        assert!(
            (ours - chrome).abs() < 1e-3,
            "{glyph:?} at 72: {ours} where Chrome's is {chrome}"
        );
    }
    // Not the 18-point cut's: `G` at 40 with `none` is narrower by more
    // than a pixel, so the axis is what moved it.
    let none = shaped("G", 40.0, &newsreader(400, OpticalSizing::None)).unwrap_or(0.0);
    let auto_g = shaped("G", 40.0, &auto).unwrap_or(0.0);
    assert!((auto_g - none - 0.56).abs() < 1e-3, "{auto_g} and {none}");
}

/// Chrome 156's layout width of "Good afternoon, Ada." at each size, weight
/// 400: `(size, auto, none)`.
const GREETING: [(f32, f32, f32); 6] = [
    (13.0, 126.343_75, 113.609_375),
    (13.337, 128.671_88, 116.484_375),
    (18.0, 157.296_88, 157.296_88),
    (22.0, 193.671_88, 192.25),
    (40.0, 363.828_12, 349.531_25),
    (90.0, 865.406_25, 786.421_9),
];

/// Chrome's layout unit.
const UNIT: f32 = 1.0 / 64.0;

#[test]
fn a_run_set_with_none_is_chromes_none_at_every_size_and_with_auto_its_auto() {
    let greeting = "Good afternoon, Ada.";
    for (size, auto, none) in GREETING {
        let ours = shaped(greeting, size, &newsreader(400, OpticalSizing::None)).unwrap_or(0.0);
        assert!(
            (ours - none).abs() <= UNIT,
            "none at {size}: {ours} where Chrome's is {none}"
        );
        let ours = shaped(greeting, size, &newsreader(400, OpticalSizing::Auto)).unwrap_or(0.0);
        // At and under 22 the greeting's kerning pairs vary too little to
        // show; at 40 and 90 their variation, which the rented shaper does
        // not apply (796), leaves this engine within an eighth of a pixel.
        let within = if size <= 22.0 { UNIT } else { 0.125 };
        assert!(
            (ours - auto).abs() <= within,
            "auto at {size}: {ours} where Chrome's is {auto}"
        );
    }
    // Runs with no kerning pair that varies, at 40: Chrome's layout widths.
    for (text, chrome) in [("alo", 50.25), ("Go", 51.078_125), ("Ad", 49.859_375)] {
        let ours = shaped(text, 40.0, &newsreader(400, OpticalSizing::Auto)).unwrap_or(0.0);
        assert!(
            (ours - chrome).abs() <= UNIT,
            "{text} at 40: {ours} where Chrome's is {chrome}"
        );
    }
}

#[test]
fn diverges_a_kerning_pair_varied_along_the_axis_is_kerned_as_at_18() {
    // Queue item 796: `iv` at 72, `opsz` 72. Chrome's layout width is
    // 60.671875 — the pair unkerned, 553 + 1132 units of 2,000 — and this
    // engine's 60.156, the pair still kerned its 18-point −14.
    let ours = shaped("iv", 72.0, &newsreader(400, OpticalSizing::Auto)).unwrap_or(0.0);
    assert!(
        (ours - (553.0 + 1132.0 - 14.0) * 72.0 / 2000.0).abs() < 1e-3,
        "{ours}"
    );
    assert!((ours - 60.671_875).abs() > 0.5, "Chrome's differs");
}

/// The page: the greeting at 40 px with `auto`, then with `none`, then the
/// mark's word at 72.
const PAGE: &str = "<!DOCTYPE html><html><body>\
    <div><span class=auto>Good afternoon, Ada.</span></div>\
    <div><span class=none>Good afternoon, Ada.</span></div>\
    <div><span class=big>alo</span></div></body></html>";

/// Its style.
const STYLE: &str = "body { margin: 0; background: #f4f1ec; color: #1b2433 }
div { margin: 8px; font-family: Newsreader; font-size: 40px; line-height: 46px }
span { white-space: pre }
.none { font-optical-sizing: none }
.big { font-size: 72px; line-height: 80px }";

/// The width of the box of the element of class `class`.
fn width_of(rendered: &alo_renderer::Rendered, class: &str) -> Option<f32> {
    let drawing = &rendered.drawing;
    drawing.boxes.ids().find_map(|id| {
        let made = drawing.boxes.get(id)?;
        if matches!(made.kind, alo_box::BoxKind::Text { .. }) {
            return None;
        }
        let node = made.kind.node()?;
        (rendered.document.attribute_by_name(node, "class") == Some(class))
            .then(|| drawing.layout.get(id).map(|at| at.border_box.size.width))
            .flatten()
    })
}

fn drawn() -> alo_renderer::Rendered {
    alo_renderer::render_at(
        PAGE,
        STYLE,
        Screen::new(Size::new(420.0, 230.0)),
        &fonts(),
        &[],
        &[],
    )
}

fn reference() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references/a-typeface-at-its-optical-size.png")
}

/// `canvas` against the committed reference, or written as it when
/// `ALO_UPDATE_REFERENCES` is set.
fn holds(canvas: &Canvas) -> Result<(), String> {
    let drawn = to_png(canvas).map_err(|why| format!("not encoded: {why}"))?;
    let path = reference();
    if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
        return std::fs::write(&path, &drawn).map_err(|why| why.to_string());
    }
    let committed = std::fs::read(&path).map_err(|why| format!("{}: {why}", path.display()))?;
    let expected = from_png(&committed).map_err(|why| format!("not a picture: {why}"))?;
    let actual = from_png(&drawn).map_err(|why| format!("not a picture: {why}"))?;
    if (actual.width(), actual.height()) != (expected.width(), expected.height()) {
        return Err("the reference is another size".to_owned());
    }
    for y in 0..actual.height() {
        for x in 0..actual.width() {
            let (a, b) = (actual.at(x, y), expected.at(x, y));
            if a.map(alo_value::Rgba::to_rgba8) != b.map(alo_value::Rgba::to_rgba8) {
                return Err(format!(
                    "pixels moved, the first at ({x}, {y}); ALO_UPDATE_REFERENCES=1 rewrites it"
                ));
            }
        }
    }
    Ok(())
}

#[test]
fn a_page_sets_its_text_at_its_size_and_reads_none() {
    let rendered = drawn();
    let auto = width_of(&rendered, "auto").unwrap_or(0.0);
    let none = width_of(&rendered, "none").unwrap_or(0.0);
    let big = width_of(&rendered, "big").unwrap_or(0.0);
    // `font-optical-sizing: none` is read: the greeting at 40 is 14.5
    // narrower, its 18-point cut.
    assert!(
        (auto - none - 14.5).abs() < 1e-3,
        "auto {auto}, none {none}"
    );
    // `alo` at 72, `opsz` 72: Chrome's 97.0625.
    assert!((big - 97.0625).abs() <= UNIT, "big {big}");
    // DIVERGES (queue item 797): a line's words are measured apart, so the
    // kerning between a space and the word after it is lost — `" A"` is
    // kerned −1.48 at `opsz` 40 and −0.63 at 18 — and each greeting box is
    // its three words' widths, 103.64 + 184.36 + 77.42 = 365.42 and 99.76
    // + 176.74 + 74.42 = 350.92, on the layout unit, where Chrome's are
    // 363.83 and 349.53. The runs themselves are Chrome's (above).
    assert!((auto - 365.421_88).abs() < 1e-3, "auto {auto}");
    assert!((none - 350.921_88).abs() < 1e-3, "none {none}");
    assert_eq!(holds(&rendered.drawing.canvas), Ok(()));
}
