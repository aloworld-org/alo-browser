/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An SVG file shown by an `<img>` (queue item 310, ADR 0027).
//!
//! It is recognised by its type and never by its bytes, sized from what the
//! file says of itself, drawn at the size layout gave the `<img>`, and it
//! sees nothing of the page and fetches nothing. Meet's greeting, the page
//! that opened it, is the corpus case `alo-meet-greeting`.

use alo_layout::{Rect, Size};
use alo_paint::{DisplayItem, Paint};
use alo_renderer::{Rendered, Resource, render_with_resources};
use alo_text::FontDatabase;

const SVG: &str = "image/svg+xml";

fn render(html: &str, css: &str, linked: &[(String, String)], resources: &[Resource]) -> Rendered {
    render_with_resources(
        html,
        css,
        Size::new(400.0, 300.0),
        &FontDatabase::new(),
        linked,
        resources,
    )
}

fn resource(src: &str, content_type: Option<&str>, svg: &str) -> Resource {
    Resource::new(src, content_type, svg.as_bytes().to_vec())
}

/// The content box of the page's `n`th `<img>`, in document order.
fn img_box(rendered: &Rendered, n: usize) -> Option<(alo_box::BoxId, Rect)> {
    let drawing = &rendered.drawing;
    let id = drawing
        .boxes
        .ids()
        .filter(|id| {
            matches!(
                drawing.boxes.get(*id).map(|node| &node.kind),
                Some(alo_box::BoxKind::Element { node, .. })
                    if rendered.document.element(*node).is_some_and(|element| element.name.is_html("img"))
            )
        })
        .nth(n)?;
    Some((id, drawing.layout.get(id)?.content_box()))
}

/// The solid colours filled for one box, in paint order.
fn fills_of(rendered: &Rendered, id: alo_box::BoxId) -> Vec<(u8, u8, u8, u8)> {
    rendered
        .drawing
        .display
        .items()
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Fill {
                box_id,
                paint: Paint::Solid(color),
                ..
            } if *box_id == id => Some(color.to_rgba8()),
            _ => None,
        })
        .collect()
}

fn issues(rendered: &Rendered) -> String {
    rendered.issues().join("\n")
}

const GREEN: (u8, u8, u8, u8) = (0, 255, 0, 255);
const BLACK: (u8, u8, u8, u8) = (0, 0, 0, 255);

#[test]
fn an_img_with_no_size_of_its_own_takes_the_files() {
    let sized = r#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="0.25in"/>"#;
    let shaped = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 1"/>"#;
    let neither = r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#;
    let rendered = render(
        r#"<div><img src="sized.svg"></div><div><img src="shaped.svg" class=w></div><div><img src="neither.svg"></div>"#,
        "body { margin: 0 } div { line-height: 0 } .w { width: 100px }",
        &[],
        &[
            resource("sized.svg", Some(SVG), sized),
            resource("shaped.svg", Some(SVG), shaped),
            resource("neither.svg", Some(SVG), neither),
        ],
    );
    let size = |n| img_box(&rendered, n).map(|(_, rect)| (rect.size.width, rect.size.height));
    assert_eq!(size(0), Some((48.0, 24.0)), "its width and height");
    assert_eq!(
        size(1),
        Some((100.0, 25.0)),
        "a width and the viewBox's ratio"
    );
    assert_eq!(
        size(2),
        Some((300.0, 150.0)),
        "nothing said is CSS's default"
    );
    assert!(!issues(&rendered).contains(".svg"), "{}", issues(&rendered));
}

#[test]
fn it_is_drawn_as_vectors_at_the_imgs_size_and_kept_inside_it() {
    // A square of user space, with a second square entirely outside the
    // viewBox to its right, which must not reach the page. The box is taller
    // than it is wide: one wider than the file's ratio meets queue item 314.
    let file = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10" width="10" height="10">
        <rect width="10" height="10" fill="#00ff00"/>
        <rect x="10" width="10" height="10" fill="#ff0000"/>
    </svg>"##;
    let rendered = render(
        r#"<img src="a.svg">"#,
        "body { margin: 0 } img { display: block; width: 40px; height: 80px; padding: 4px }",
        &[],
        &[resource("a.svg", Some(SVG), file)],
    );
    let Some((id, content)) = img_box(&rendered, 0) else {
        panic!("an <img> box");
    };
    assert_eq!(
        (
            content.left(),
            content.top(),
            content.size.width,
            content.size.height
        ),
        (4.0, 4.0, 40.0, 80.0)
    );
    let clip = rendered
        .drawing
        .display
        .items()
        .iter()
        .find_map(|item| match item {
            DisplayItem::PushClip { box_id, path } if *box_id == id => path.bounds(),
            _ => None,
        });
    assert_eq!(
        clip,
        Some((4.0, 4.0, 44.0, 84.0)),
        "clipped to its content box although an <img> is overflow: visible",
    );
    let canvas = &rendered.drawing.canvas;
    let at = |x, y| canvas.at(x, y).map(alo_value::Rgba::to_rgba8);
    let white = Some((255, 255, 255, 255));
    assert_eq!(at(24, 44), Some(GREEN), "the square, met into the middle");
    assert_eq!(at(24, 25), Some(GREEN), "its top at 4 + 20");
    assert_eq!(at(24, 22), white, "above it, the box's empty band");
    assert_eq!(
        at(50, 44),
        white,
        "nothing past the viewBox, nor past the box"
    );
}

#[test]
fn the_same_bytes_under_another_type_are_refused() {
    let file = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1"><rect width="1" height="1"/></svg>"#;
    for content_type in [None, Some("text/xml"), Some("image/png"), Some("text/html")] {
        let rendered = render(
            r#"<img src="a.svg">"#,
            "img { width: 20px; height: 20px }",
            &[],
            &[resource("a.svg", content_type, file)],
        );
        let Some((id, content)) = img_box(&rendered, 0) else {
            panic!("an <img> box");
        };
        assert!(
            fills_of(&rendered, id).is_empty(),
            "{content_type:?} drew something",
        );
        assert_eq!((content.size.width, content.size.height), (20.0, 20.0));
        assert!(
            issues(&rendered).contains("\"a.svg\" is not a picture this engine reads"),
            "{content_type:?}: {}",
            issues(&rendered),
        );
    }
}

#[test]
fn a_png_that_says_it_is_svg_is_never_decoded() {
    let png = include_bytes!("../../alo-corpus/cases/a-picture/stripes.png").to_vec();
    let rendered = render(
        r#"<img src="a.png">"#,
        "",
        &[],
        &[Resource::new("a.png", Some(SVG), png)],
    );
    let Some((id, _)) = img_box(&rendered, 0) else {
        panic!("an <img> box");
    };
    assert!(
        !rendered
            .drawing
            .display
            .items()
            .iter()
            .any(|item| matches!(item, DisplayItem::Picture { box_id, .. } if *box_id == id)),
        "decoded as a raster picture",
    );
    assert!(
        issues(&rendered).contains("\"a.png\" is not an SVG picture this engine reads"),
        "{}",
        issues(&rendered),
    );
}

#[test]
fn a_file_that_asks_for_everything_draws_its_own_shapes_and_fetches_nothing() {
    // Everything ADR 0027 § 4 names as a way out of a file: a script, an
    // `<image>`, an external `<use>`, an `@import`, a stylesheet processing
    // instruction, a linked sheet and an external DTD.
    let file = r##"<?xml version="1.0" encoding="UTF-8"?>
<?xml-stylesheet href="evil.css"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:h="http://www.w3.org/1999/xhtml" viewBox="0 0 10 10">
  <style>@import url(evil.css); @import "evil.css";</style>
  <h:link rel="stylesheet" href="evil.css"/>
  <script>document.title = "ran"</script>
  <rect width="10" height="10" fill="#00ff00" onload="alert(1)"/>
  <image href="evil.png" width="10" height="10"/>
  <image xlink:href="evil.png" width="10" height="10"/>
  <use href="evil.svg#a"/>
  <use xlink:href="evil.svg#a"/>
</svg>"##;
    // Everything the file names is on offer, and each would draw red if it
    // were ever taken: a sheet filling every rect, a picture, and a file
    // of red.
    let red = r#"<svg xmlns="http://www.w3.org/2000/svg" id="a" viewBox="0 0 1 1"><rect width="1" height="1" fill="red"/></svg>"#;
    let linked = vec![("evil.css".to_owned(), "rect { fill: #ff0000 }".to_owned())];
    let rendered = render(
        r#"<img src="file.svg" alt="">"#,
        "body { margin: 0 } img { display: block; width: 50px; height: 50px }",
        &linked,
        &[
            resource("file.svg", Some(SVG), file),
            resource("evil.svg", Some(SVG), red),
            resource("evil.css", Some(SVG), red),
            Resource::new(
                "evil.png",
                None,
                include_bytes!("../../alo-corpus/cases/a-picture/stripes.png").to_vec(),
            ),
        ],
    );
    let Some((id, _)) = img_box(&rendered, 0) else {
        panic!("an <img> box");
    };
    assert_eq!(fills_of(&rendered, id), vec![GREEN], "its own rect, once");
    assert!(
        !rendered
            .drawing
            .display
            .items()
            .iter()
            .any(|item| matches!(item, DisplayItem::Picture { .. })),
        "a picture inside the picture",
    );
    assert!(
        rendered
            .drawing
            .canvas
            .pixels()
            .iter()
            .all(|pixel| pixel.to_rgba8() != (255, 0, 0, 255)),
        "something the file named reached the page",
    );
    let said = issues(&rendered);
    for expected in [
        "\"file.svg\": a style sheet at \"evil.css\" is not fetched for an SVG picture",
        "\"file.svg\": <image>",
        "\"file.svg\": <use>",
    ] {
        assert!(said.contains(expected), "{expected:?} in {said}");
    }
    assert!(
        !said.contains("no picture was loaded") && !said.contains("no style sheet was loaded"),
        "something was looked for: {said}",
    );
}

#[test]
fn the_pages_style_does_not_reach_inside_the_file() {
    let file = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 3 1">
        <style>.var { fill: var(--leak, #00ff00) }</style>
        <rect width="1" height="1" fill="currentColor"/>
        <rect x="1" width="1" height="1" class="var"/>
        <rect x="2" width="1" height="1" class="page" fill="#00ff00"/>
    </svg>"##;
    let rendered = render(
        r#"<p style="color: #ff0000"><img src="a.svg"></p>"#,
        ":root { --leak: #ff0000 } img { color: #ff0000; width: 30px } .page, rect { fill: #ff0000 }",
        &[],
        &[resource("a.svg", Some(SVG), file)],
    );
    let Some((id, _)) = img_box(&rendered, 0) else {
        panic!("an <img> box");
    };
    assert_eq!(
        fills_of(&rendered, id),
        vec![BLACK, GREEN, GREEN],
        "its own colour, its own custom property and its own attribute",
    );
}

#[test]
fn the_agent_reads_the_img_by_its_alt_and_nothing_inside() {
    let file = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1" role="img" aria-label="from inside">
        <title>also from inside</title>
        <a href="https://elsewhere.example/"><rect width="1" height="1"/></a>
    </svg>"#;
    let rendered = render(
        r#"<p><img src="a.svg" alt="a waving hand"></p>"#,
        "img { width: 20px }",
        &[],
        &[resource("a.svg", Some(SVG), file)],
    );
    let tree = alo_agent::AgentTree::new(
        &rendered.document,
        &rendered.drawing.boxes,
        &rendered.drawing.layout,
    );
    let named = tree.named("a waving hand");
    assert_eq!(named.len(), 1, "{}", tree.to_outline());
    assert!(named.iter().all(|node| node.children().is_empty()));
    assert!(tree.named("from inside").is_empty());
    assert!(tree.named("also from inside").is_empty());
    assert!(!tree.to_outline().contains("link"), "{}", tree.to_outline());
}

#[test]
fn a_file_that_is_refused_leaves_the_box_its_style_gives_it() {
    for (bytes, why) in [
        (
            r#"<!DOCTYPE svg [<!ENTITY a "a">]><svg xmlns="http://www.w3.org/2000/svg"/>"#,
            "internal subset",
        ),
        (r#"<svg xmlns="http://www.w3.org/2000/svg">"#, "byte"),
        (
            r#"<html xmlns="http://www.w3.org/1999/xhtml"/>"#,
            "not an SVG <svg>",
        ),
        (
            r#"<svg xmlns="http://www.w3.org/2000/svg">&lol;</svg>"#,
            "&lol;",
        ),
    ] {
        let rendered = render(
            r#"<img src="a.svg">"#,
            "img { width: 30px; height: 10px }",
            &[],
            &[resource("a.svg", Some(SVG), bytes)],
        );
        let Some((id, content)) = img_box(&rendered, 0) else {
            panic!("an <img> box");
        };
        assert_eq!((content.size.width, content.size.height), (30.0, 10.0));
        assert!(fills_of(&rendered, id).is_empty());
        let said = issues(&rendered);
        assert!(
            said.contains("\"a.svg\" is not an SVG picture this engine reads")
                && said.contains(why),
            "{why:?}: {said}",
        );
    }
}
