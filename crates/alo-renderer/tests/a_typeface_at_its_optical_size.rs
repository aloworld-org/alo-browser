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
//! What closes it, each a test below: each glyph's advance at `opsz` 40
//! and 72 is Chrome's canvas advance to a thousandth of a pixel; a run set
//! with `none` or `auto` is Chrome's to its layout unit at every size; a
//! kerning pair varied along `opsz` is varied as Chrome varies it; a page's
//! `font-optical-sizing: none` read, its boxes in numbers and Chrome's;
//! a reference render, `references/a-typeface-at-its-optical-size.png`.
//!
//! Three things made these Chrome's after 752 (queue items 797, 795 and
//! 796, ADR 0097): a line's text measured as one run, not word by word, so
//! the kerning across a space is kept; a varied glyph's advance kept to the
//! fraction `HVAR` gives it, where the rented shaper, `rustybuzz` 0.20.1,
//! rounds it to a whole unit; and a kerning pair's own variation along the
//! axis applied — `iv` kerned −14 units at 18 and 0 at 72 — which the
//! shaper leaves out of a pair listed glyph by glyph (`ttf-parser` 0.25.1
//! reads its table's offset from the wrong place) and rounds in one kerned
//! by classes. The last ten-thousandths of a pixel left are `ttf-parser`'s
//! coordinate on the axis, cut to 6,674 of 16,384 at 40 where Chrome keeps
//! 6,674.96.
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
    // A varied advance kept to its fraction (queue item 795): where it was
    // rounded to a whole unit it was up to 0.0096 px off at 40.
    for (glyph, chrome) in AT_40 {
        let ours = shaped(&glyph.to_string(), 40.0, &auto).unwrap_or(0.0);
        assert!(
            (ours - chrome).abs() < 1e-3,
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
    // than half a pixel, so the axis is what moved it.
    let none = shaped("G", 40.0, &newsreader(400, OpticalSizing::None)).unwrap_or(0.0);
    let auto_g = shaped("G", 40.0, &auto).unwrap_or(0.0);
    assert!(auto_g - none > 0.5, "{auto_g} and {none}");
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
        // Its kerning pairs varied along `opsz` as Chrome varies them
        // (796): at 40 and 90 they were an eighth of a pixel off.
        assert!(
            (ours - auto).abs() <= UNIT,
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
fn a_kerning_pair_varied_along_the_axis_is_varied_as_chrome_varies_it() {
    // Queue item 796: `iv` at 72, `opsz` 72. Chrome's layout width is
    // 60.671875 — the pair unkerned, 553 + 1132 units of 2,000 — where the
    // rented shaper alone kerns it its 18-point −14, 60.156.
    let ours = shaped("iv", 72.0, &newsreader(400, OpticalSizing::Auto)).unwrap_or(0.0);
    assert!((ours - 60.671_875).abs() <= UNIT, "{ours}");
    assert!(
        (ours - (553.0 + 1132.0) * 72.0 / 2000.0).abs() < 1e-3,
        "{ours}"
    );
    // At 18, the font's default, Chrome kerns it −14 too: 13.640625; and
    // at 40 partly, 31.6875.
    for (size, chrome) in [(18.0, 13.640_625), (40.0, 31.687_5)] {
        let ours = shaped("iv", size, &newsreader(400, OpticalSizing::Auto)).unwrap_or(0.0);
        assert!((ours - chrome).abs() <= UNIT, "{size}: {ours}");
    }
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
    // `font-optical-sizing: none` is read: the greeting at 40 is 14.30
    // narrower, its 18-point cut.
    assert!(
        (auto - none - 14.296_88).abs() < 1e-3,
        "auto {auto}, none {none}"
    );
    // `alo` at 72, `opsz` 72: Chrome's 97.0625.
    assert!((big - 97.0625).abs() <= UNIT, "big {big}");
    // Queue item 797: a line's text is measured as one run, as Chrome
    // shapes it, so the kerning between a space and the word after it is
    // kept — `" A"` is kerned −1.48 at `opsz` 40 and −0.63 at 18. Measured
    // word by word the boxes were 365.42 and 350.92; now `none` is
    // Chrome's 349.53 to its layout unit, and `auto` within an eighth of
    // its 363.83, the variation of the greeting's kerning pairs along
    // `opsz` that the rented shaper does not apply (796).
    assert!((none - 349.531_25).abs() <= UNIT, "none {none}");
    assert!((auto - 363.828_12).abs() <= UNIT, "auto {auto}");
    assert_eq!(holds(&rendered.drawing.canvas), Ok(()));
}
