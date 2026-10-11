/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What paint draws of a line `text-overflow: ellipsis` ended (queue item
//! 442): the text it kept, the "…" after it in the block's colour, the
//! underline under what was kept and not under the "…", and nothing of an
//! atomic box it hid.

use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::{DisplayList, PaintContext, build};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    if let Some(font) = Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    ) {
        database.add(font);
    }
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

fn draw(html: &str, css: &str) -> DisplayList {
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(css);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(200.0, 100.0), &measurer);
    build::build(
        &boxes,
        &layout,
        &styles,
        PaintContext {
            fonts: &database,
            pictures: &std::collections::BTreeMap::new(),
            drawings: &std::collections::BTreeMap::new(),
        },
    )
}

const ROW: &str = "<!DOCTYPE html><html><body><div id=e>ab<span id=box></span>cdefghij</div>\
</body></html>";

/// A row of `width`, holding "ab", a red box 20 wide and "cdefghij". In
/// `DejaVu Sans` at 16 px "ab" is 19.96 and "…" 16.
fn row(width: u32) -> String {
    format!(
        "body {{ margin: 0; font-family: sans-serif }}
         #e {{ width: {width}px; white-space: nowrap; overflow: hidden;
               text-overflow: ellipsis; color: #1d4ed8; text-decoration: underline }}
         #box {{ display: inline-block; width: 20px; height: 10px; background: #ff0000 }}"
    )
}

fn texts(list: &DisplayList) -> Vec<String> {
    list.to_outline()
        .lines()
        .filter(|line| line.starts_with("text "))
        .map(str::to_owned)
        .collect()
}

#[test]
fn an_atomic_box_past_the_cut_is_not_drawn() {
    // In 50 the "…" has to start by 34, and the box ends at 39.96: hidden.
    let list = draw(ROW, &row(50));
    let outline = list.to_outline();
    assert!(!outline.contains("rgb(255 0 0)"), "{outline}");
    let texts = texts(&list);
    assert_eq!(texts.len(), 2, "{texts:#?}");
    assert!(texts[0].contains("\"ab\""), "{texts:#?}");
    // Straight after "ab", in the block's blue.
    assert!(
        texts[1].contains("\"…\" rgb(29 78 216) 16px at (19.96"),
        "{texts:#?}"
    );
}

#[test]
fn an_atomic_box_before_the_cut_is_drawn_and_the_ellipsis_follows_it() {
    // In 60 the "…" has to start by 44: the box stays, "c" does not.
    let list = draw(ROW, &row(60));
    let outline = list.to_outline();
    assert!(outline.contains("rgb(255 0 0)"), "{outline}");
    let texts = texts(&list);
    assert_eq!(texts.len(), 2, "{texts:#?}");
    assert!(texts[1].contains("\"…\""), "{texts:#?}");
    assert!(texts[1].contains("at (39.96"), "{texts:#?}");
}

#[test]
fn the_underline_runs_under_what_was_kept_and_not_under_the_ellipsis() {
    let list = draw(ROW, &row(50));
    let lines: Vec<String> = list
        .to_outline()
        .lines()
        .filter(|line| line.starts_with("fill ") && line.contains("rgb(29 78 216)"))
        .map(str::to_owned)
        .collect();
    // One underline, under "ab": 19.96 wide, never the 16 more of the "…".
    let [underline] = lines.as_slice() else {
        panic!("one underline: {lines:#?}");
    };
    assert!(underline.contains(" 19.96"), "{underline}");
}
