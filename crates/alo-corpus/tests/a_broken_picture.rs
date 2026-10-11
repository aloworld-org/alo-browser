/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A broken picture is its text, and an agent reads that it broke (queue item
//! 413, ADR 0046).
//!
//! Whether an `<img>` shows a picture is decided once, before the boxes are
//! built (§ 1). Without one, a non-empty `alt` is a run of text in the
//! `<img>`'s own style at the `display` its style gives, `inline` by default
//! (§ 2); without an `alt` it is a replaced box of nothing (§ 3); and either
//! way an agent reads `broken` and the `alt` once, as the name (§ 5).
//!
//! Here rather than in `alo-renderer` because the numbers are text widths,
//! and the corpus is where the fonts every machine draws the same are.

use alo_agent::AgentTree;
use alo_box::BoxId;
use alo_corpus::{cases_directory, corpus_fonts};
use alo_layout::{Rect, Size};
use alo_paint::DisplayItem;
use alo_renderer::{Rendered, Resource, render_with_resources};

/// The face every page here is set in, so a width is `DejaVu Sans`' own.
const FACE: &str = "body { margin: 0; font-family: 'DejaVu Sans'; font-size: 16px }";

/// `DejaVu Sans`' `normal` line at 16 px: 1.1640625 em.
const LINE: f32 = 18.625;

/// "Orange notebook" in `DejaVu Sans` at 16 px, as HarfBuzz's own `hb-shape`
/// shapes it: 17 824 font units of 2048, kerning included.
const ORANGE_NOTEBOOK: f32 = 17_824.0 / 2048.0 * 16.0;

fn render(html: &str, css: &str, resources: &[Resource]) -> Rendered {
    render_with_resources(
        html,
        &format!("{FACE}\n{css}"),
        Size::new(400.0, 300.0),
        &corpus_fonts(),
        &[],
        resources,
    )
}

/// The committed raster the `a-picture` case draws, as a resource named
/// `a.png`.
fn stripes() -> Option<Resource> {
    let bytes = std::fs::read(cases_directory().join("a-picture/stripes.png")).ok()?;
    Some(Resource::new("a.png", Some("image/png"), bytes))
}

/// Every `<img>` box, in document order.
fn img_boxes(rendered: &Rendered) -> Vec<BoxId> {
    let drawing = &rendered.drawing;
    drawing
        .boxes
        .ids()
        .filter(|id| {
            matches!(
                drawing.boxes.get(*id).map(|node| &node.kind),
                Some(alo_box::BoxKind::Element { node, .. })
                    if rendered.document.element(*node).is_some_and(|element| element.name.is_html("img"))
            )
        })
        .collect()
}

/// The border box of the page's `n`th `<img>`.
fn img_rect(rendered: &Rendered, n: usize) -> Option<Rect> {
    let id = *img_boxes(rendered).get(n)?;
    rendered.drawing.layout.border_box(id)
}

/// Whether the page's `n`th `<img>` carries `broken`.
fn is_broken(rendered: &Rendered, n: usize) -> Option<bool> {
    let id = *img_boxes(rendered).get(n)?;
    Some(rendered.drawing.boxes.get(id)?.semantics.states.broken)
}

/// A colour as its four bytes.
type Rgba8 = (u8, u8, u8, u8);

/// Every run of text drawn, as `(text, x, baseline, colour)`.
fn runs(rendered: &Rendered) -> Vec<(String, f32, f32, Rgba8)> {
    rendered
        .drawing
        .display
        .items()
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Text {
                text,
                origin,
                color,
                ..
            } => Some((text.clone(), origin.0, origin.1, color.to_rgba8())),
            _ => None,
        })
        .collect()
}

fn outline(rendered: &Rendered) -> String {
    AgentTree::new(
        &rendered.document,
        &rendered.drawing.boxes,
        &rendered.drawing.layout,
    )
    .to_outline()
}

/// Equal to within a thousandth of a pixel.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

#[test]
fn every_img_without_a_picture_is_broken_and_one_with_is_not() {
    let Some(png) = stripes() else {
        panic!("the a-picture case's stripes.png");
    };
    let refused = Resource::new("bad.svg", Some("image/svg+xml"), b"<svg".to_vec());
    let rendered = render(
        concat!(
            "<p><img src=a.png></p>",
            "<p><img></p>",
            "<p><img src=''></p>",
            "<p><img src=never-handed.png></p>",
            "<p><img src=not-a-picture.png></p>",
            "<p><img src=bad.svg></p>",
        ),
        "",
        &[
            png,
            Resource::new("not-a-picture.png", Some("image/png"), b"\x89PNG".to_vec()),
            refused,
        ],
    );
    assert_eq!(is_broken(&rendered, 0), Some(false), "a decoded picture");
    for n in 1..=5 {
        assert_eq!(is_broken(&rendered, n), Some(true), "img {n}");
    }
    let Some(picture) = img_rect(&rendered, 0) else {
        panic!("the picture's box");
    };
    assert_eq!(
        (picture.size.width, picture.size.height),
        (24.0, 24.0),
        "a picture is still its own size, inline rather than inline-block",
    );
}

#[test]
fn an_alt_is_inline_text_on_its_line_in_the_imgs_style() {
    let rendered = render(
        "<p>A <img alt='Orange notebook' width=640 height=100> here</p>",
        "img { color: rgb(255, 0, 0); width: 200px; height: 100px }",
        &[],
    );
    let Some(img) = img_rect(&rendered, 0) else {
        panic!("the img's box");
    };
    assert!(
        near(img.size.width, ORANGE_NOTEBOOK),
        "as wide as its text, its attributes and its style's size doing nothing: {img:?}",
    );
    assert!(near(img.size.height, LINE), "one line tall: {img:?}");
    let runs = runs(&rendered);
    let Some((_, x, baseline, colour)) = runs
        .iter()
        .find(|(text, ..)| text == "Orange notebook")
        .cloned()
    else {
        panic!("the alt is drawn: {runs:?}");
    };
    assert!(near(x, img.left()), "drawn where its box starts");
    assert_eq!(colour, (255, 0, 0, 255), "in the img's colour");
    assert!(
        runs.iter()
            .filter(|(text, ..)| text.contains("here"))
            .all(|(_, after, line, _)| near(*line, baseline)
                && *after >= img.left() + img.size.width),
        "on the same line as the text around it, after it: {runs:?}",
    );
}

#[test]
fn an_alt_wraps_with_its_line() {
    let rendered = render(
        "<div style='width: 100px'><img alt='Orange notebook front cover'></div>",
        "",
        &[],
    );
    // In DejaVu Sans at 16 px, by `hb-shape`: "Orange" is 59.117 and
    // "Orange notebook" 139.25; "notebook" 75.047 and "notebook front"
    // 118.195; "front cover" 87.625. So three lines in 100.
    let runs = runs(&rendered);
    let words: Vec<&str> = runs.iter().map(|(text, ..)| text.trim_end()).collect();
    assert_eq!(words, ["Orange", "notebook", "front cover"], "{runs:?}");
    for pair in runs.windows(2) {
        let [above, below] = pair else { continue };
        assert!(near(below.2 - above.2, LINE), "a line apart: {runs:?}");
        assert!(near(below.1, 0.0), "each line starts at the left: {runs:?}");
    }
    let tree = outline(&rendered);
    assert!(
        tree.contains(
            "image \"Orange notebook front cover\" [broken] at (0, 0) 87.625×55.875 in 3 pieces"
        ),
        "the image is the union of its three lines:\n{tree}",
    );
}

#[test]
fn a_block_alt_fills_its_container() {
    let rendered = render(
        "<div style='width: 300px'><img alt='Orange notebook' style='display: block'></div>",
        "",
        &[],
    );
    let Some(img) = img_rect(&rendered, 0) else {
        panic!("the img's box");
    };
    assert_eq!((img.size.width, img.size.height), (300.0, LINE));
}

#[test]
fn without_an_alt_it_is_a_box_of_nothing_or_of_its_style() {
    let rendered = render(
        concat!(
            "<p><img src=gone.png></p>",
            "<p><img src=gone.png alt=''></p>",
            "<p><img></p>",
            "<p><img src=gone.png class=sized></p>",
            "<p><img src=gone.png alt='' class=sized></p>",
            "<p><img src=gone.png class=wide></p>",
        ),
        ".sized { width: 40px; height: 20px } .wide { width: 40px }",
        &[],
    );
    let sizes: Vec<(f32, f32)> = (0..6)
        .filter_map(|n| img_rect(&rendered, n))
        .map(|rect| (rect.size.width, rect.size.height))
        .collect();
    assert_eq!(
        sizes,
        vec![
            (0.0, 0.0),
            (0.0, 0.0),
            (0.0, 0.0),
            (40.0, 20.0),
            (40.0, 20.0),
            // A width and no ratio: the height is the content's, nothing —
            // never CSS's default object size, 300 × 150.
            (40.0, 0.0),
        ],
    );
    assert!(runs(&rendered).is_empty(), "nothing is drawn as text");
    assert!((0..6).all(|n| is_broken(&rendered, n) == Some(true)));
}

#[test]
fn an_agent_reads_broken_and_the_alt_once() {
    let Some(png) = stripes() else {
        panic!("the a-picture case's stripes.png");
    };
    let rendered = render(
        "<p><img src=a.png alt='Stripes'></p><p><img src=gone.png alt='Orange notebook'></p><p><img src=gone.png></p>",
        "",
        &[png],
    );
    let tree = outline(&rendered);
    let lines: Vec<&str> = tree.lines().map(str::trim_start).collect();
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("image \"Stripes\" at")),
        "a shown picture carries nothing:\n{tree}",
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("image \"Orange notebook\" [broken] at")),
        "{tree}",
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("image [broken] at")),
        "{tree}",
    );
    assert_eq!(
        tree.matches("Orange notebook").count(),
        1,
        "the alt is the name, and no text node repeats it:\n{tree}",
    );
}

#[test]
fn a_hostile_alt_is_a_long_run_of_text() {
    let long = "notebook ".repeat(12_000);
    let controls: String = (0u8..32).map(char::from).chain("tail".chars()).collect();
    for alt in [long.as_str(), controls.as_str()] {
        let html = format!("<p><img alt=\"{}\"></p>", alt.replace('"', "&quot;"));
        let rendered = render(&html, "", &[]);
        assert_eq!(is_broken(&rendered, 0), Some(true));
        assert!(img_rect(&rendered, 0).is_some(), "it is laid out");
    }
}
