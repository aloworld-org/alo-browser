/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How a decoration line meets its letters (queue item 581): whether an
//! underline or an overline steps aside for them, which the text's own
//! `text-decoration-skip-ink` decides, and which is drawn first — an
//! underline and an overline before the letters and a line through after,
//! as Chrome 154 draws them, except under text that casts a shadow, where
//! every line is still drawn after the text and its shadow (queue item 584).

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

fn draw(body: &str, css: &str) -> DisplayList {
    let document = parse_document(&format!("<!DOCTYPE html><html><body>{body}</body></html>"));
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(&format!(
        "body {{ margin: 0; font-family: sans-serif; font-size: 20px }} {css}"
    ));
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(300.0, 100.0), &measurer);
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

/// The list's lines, each said as `text "…"` or as the row and the width of
/// a fill one CSS pixel thick or more, `line y w`, in the order drawn.
fn said(list: &DisplayList) -> Vec<String> {
    list.to_outline()
        .lines()
        .filter_map(|line| {
            if let Some(rest) = line.strip_prefix("text ") {
                let text = rest.split('"').nth(1)?;
                return Some(format!("text {text}"));
            }
            let at = line.strip_prefix("fill ")?.split(" at (").nth(1)?;
            let (place, size) = at.split_once(") ")?;
            let top = place.split(", ").nth(1)?;
            let width = size.split('×').next()?;
            Some(format!("line {top} {width}"))
        })
        .collect()
}

/// How many pieces each row's line is drawn in, in the order the rows are
/// first drawn.
fn pieces_per_line(said: &[String]) -> Vec<(String, usize)> {
    let mut rows: Vec<(String, usize)> = Vec::new();
    for line in said {
        let Some(row) = line
            .strip_prefix("line ")
            .and_then(|rest| rest.split(' ').next())
        else {
            continue;
        };
        match rows.iter_mut().find(|(seen, _)| seen == row) {
            Some((_, count)) => *count += 1,
            None => rows.push((row.to_owned(), 1)),
        }
    }
    rows
}

const LINES: &str = ".all3 { text-decoration-line: underline overline line-through }";

#[test]
fn an_underline_and_an_overline_are_drawn_before_the_letters_and_a_line_through_after() {
    let said = said(&draw("<p class=all3>gy</p>", LINES));
    let text = said.iter().position(|line| line == "text gy");
    let Some(text) = text else {
        panic!("the text is drawn: {said:?}");
    };
    // At 20 px, `gy` 24.53 wide on a line whose top is at 20: the
    // underline on row 40, the overline on 18, the line through on 32
    // (queue item 566's measured rule).
    let (before, after) = said.split_at(text);
    assert!(
        before.iter().any(|line| line.starts_with("line 40 "))
            && before.iter().any(|line| line.starts_with("line 18 ")),
        "{said:?}"
    );
    assert_eq!(
        after.get(1..),
        Some(&["line 32 24.53125".to_owned()][..]),
        "{said:?}"
    );
}

#[test]
fn under_a_text_shadow_every_line_is_drawn_after_the_text() {
    let css = format!("{LINES} p {{ text-shadow: 0 2px 0 #00ff00 }}");
    let said = said(&draw("<p class=all3>gy</p>", &css));
    assert_eq!(
        said.first().map(String::as_str),
        Some("text gy"),
        "{said:?}"
    );
    assert_eq!(pieces_per_line(&said).len(), 3, "{said:?}");
}

#[test]
fn none_draws_the_line_whole_and_auto_and_all_step_aside() {
    let css =
        "p { text-decoration-line: underline overline } .none { text-decoration-skip-ink: none }
               .all { text-decoration-skip-ink: all }";
    // `gy`, 24.53 wide: whole, an underline and an overline of one piece
    // each, the letters' width; stepping aside, the underline is two
    // pieces, before the `g`'s tail and between the tails, the `y`'s
    // running past its end, and the overline, which no letter at 20 px
    // reaches, one.
    let whole = said(&draw("<p class=none>gy</p>", css));
    assert_eq!(
        pieces_per_line(&whole),
        [("40".to_owned(), 1), ("18".to_owned(), 1)]
    );
    assert!(
        whole
            .iter()
            .filter(|line| line.starts_with("line "))
            .all(|line| line.ends_with(" 24.53125")),
        "{whole:?}"
    );
    for class in ["", "all"] {
        let stepping = said(&draw(&format!("<p class='{class}'>gy</p>"), css));
        assert_eq!(
            pieces_per_line(&stepping),
            [("40".to_owned(), 2), ("18".to_owned(), 1)],
            "{class:?}: {stepping:?}"
        );
    }
}

#[test]
fn the_texts_own_value_decides_whichever_element_asked_for_the_line() {
    let css = "u { text-decoration-skip-ink: none } .auto { text-decoration-skip-ink: auto }
               .bogus { text-decoration-skip-ink: bogus } .initial { text-decoration-skip-ink: initial }";
    for (child, steps) in [("auto", true), ("bogus", false), ("initial", true)] {
        let said = said(&draw(
            &format!("<p><u>gy<span class={child}>gy</span></u></p>"),
            css,
        ));
        let underline: Vec<_> = said
            .iter()
            .filter(|line| line.starts_with("line 40 "))
            .collect();
        // The parent's own `gy` is one piece; the child's is three when it
        // steps aside and one when it does not.
        let expected = if steps { 4 } else { 2 };
        assert_eq!(underline.len(), expected, "{child}: {said:?}");
    }
}
