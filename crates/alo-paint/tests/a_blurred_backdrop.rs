/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `backdrop-filter: blur()`, asserted rather than pictured (queue item 885,
//! ADR 0104): what the backdrop is — everything drawn before the box, and
//! not the box's own shadow; inside a faded group, only what the group
//! drew — that a value refused is said by name and still makes a stacking
//! context, and that nothing is blurred where nothing asks for it. How
//! close the blur is to Chrome's is `alo-corpus`' `backdrops-blurred`.

use alo_box::build as build_boxes;
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_paint::{Canvas, DisplayList, PaintContext, build, render};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};
use alo_value::Rgba;

const WIDTH: u32 = 120;
const HEIGHT: u32 = 120;

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
    database.map_generic("system-ui", "DejaVu Sans");
    database
}

fn draw(html: &str, css: &str) -> (DisplayList, Canvas) {
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
    let layout = compute(&boxes, &styles, Size::new(120.0, 120.0), &measurer);
    let list = build::build(
        &boxes,
        &layout,
        &styles,
        PaintContext {
            fonts: &database,
            pictures: &std::collections::BTreeMap::new(),
            drawings: &std::collections::BTreeMap::new(),
            beneath: &std::collections::BTreeSet::new(),
        },
    );

    let mut canvas = Canvas::new(WIDTH, HEIGHT, Rgba::WHITE);
    render(&list, &mut canvas);
    (list, canvas)
}

/// A page of black and white stripes 6 px wide, and a box over it.
const STRIPES: &str = "body { margin: 0; background: #fff }
                       .s { display: flex; height: 120px } .s i { width: 6px; flex: none; background: #000 }
                       .s i + i { margin-left: 6px }
                       #a { position: absolute; left: 20px; top: 20px; width: 60px; height: 60px }";

/// Ten stripes, and the box.
const STRIPED: &str =
    "<div class=s><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i></div>";

const BOX: &str = concat!(
    "<!DOCTYPE html><html><body>",
    "<div class=s><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i></div>",
    "<div id=a></div></body></html>"
);

/// The red channel of a pixel, in levels of 255.
fn red(canvas: &Canvas, x: u32, y: u32) -> u8 {
    canvas.at(x, y).map_or(0, |pixel| pixel.to_rgba8().0)
}

#[test]
fn what_is_behind_the_box_is_blurred_and_nothing_outside_it() {
    let (list, canvas) = draw(
        BOX,
        &format!("{STRIPES} #a {{ backdrop-filter: blur(4px) }}"),
    );
    assert!(
        list.to_outline().contains("backdrop box#"),
        "{}",
        list.to_outline()
    );
    // Inside, the stripes are grey; outside, as sharp as they were.
    for x in 30..70 {
        let level = red(&canvas, x, 50);
        assert!((100..=160).contains(&level), "at {x}: {level}");
    }
    assert_eq!(red(&canvas, 9, 50), 255);
    assert_eq!(red(&canvas, 3, 50), 0);
    assert_eq!(red(&canvas, 99, 50), 0);
    assert_eq!(red(&canvas, 57, 90), 255, "under the box, past it");
    assert_eq!(red(&canvas, 51, 90), 0);
}

#[test]
fn the_boxs_own_shadow_is_not_behind_it() {
    // Its shadow is drawn after its backdrop, which reads only what came
    // before the box: inside, every pixel is as it is with no shadow.
    let plain = format!("{STRIPES} #a {{ backdrop-filter: blur(4px) }}");
    let shadowed = format!("{plain} #a {{ box-shadow: 0 0 12px 8px #e00000 }}");
    let ((_, without), (list, with)) = (draw(BOX, &plain), draw(BOX, &shadowed));
    let outline = list.to_outline();
    let backdrop = outline.find("backdrop").expect("a backdrop");
    let shadow = outline.find("shadow").expect("a shadow");
    assert!(backdrop < shadow, "{outline}");
    for y in 21..79 {
        for x in 21..79 {
            assert_eq!(
                with.at(x, y).map(alo_value::Rgba::to_rgba8),
                without.at(x, y).map(alo_value::Rgba::to_rgba8),
                "at {x}, {y}"
            );
        }
    }
}

#[test]
fn inside_a_faded_group_only_what_the_group_drew_is_behind_it() {
    // Filter Effects 2's backdrop root: a group is drawn on a surface of its
    // own, and a box in it blurs that surface — here nothing, so the
    // stripes under the group stay sharp.
    let html = format!(
        "<!DOCTYPE html><html><body>{STRIPED}<section><div id=a></div></section></body></html>"
    );
    let css = format!("{STRIPES} section {{ opacity: 0.5 }} #a {{ backdrop-filter: blur(4px) }}");
    let (list, canvas) = draw(&html, &css);
    assert!(list.to_outline().contains("group"), "{}", list.to_outline());
    for x in 24..76 {
        let level = red(&canvas, x, 50);
        assert!(level == 0 || level == 255, "at {x}: {level}");
    }
}

#[test]
fn another_filter_is_refused_by_name_and_still_stacks_what_it_holds() {
    let html =
        format!("<!DOCTYPE html><html><body>{STRIPED}<div id=a><p id=b></p></div></body></html>");
    let css = format!(
        "{STRIPES} #a {{ backdrop-filter: blur(4px) sepia(1); background: #0000e0 }}
         #b {{ position: absolute; z-index: -1; left: 0; top: 0; width: 10px; height: 10px; margin: 0; background: #e00000 }}"
    );
    let (list, canvas) = draw(&html, &css);
    let said: Vec<String> = list.issues().iter().map(ToString::to_string).collect();
    assert_eq!(
        said,
        [
            "0:0: value not implemented, property left at its initial: backdrop-filter: blur(4px) sepia(1)"
        ]
    );
    assert!(
        !list.to_outline().contains("backdrop box#"),
        "{}",
        list.to_outline()
    );
    // The stacking context holds the red box over the blue background,
    // where a box without one would let it fall behind.
    assert_eq!(
        canvas.at(25, 25).map(alo_value::Rgba::to_rgba8),
        Some((0xe0, 0, 0, 255))
    );
    assert_eq!(
        canvas.at(50, 50).map(alo_value::Rgba::to_rgba8),
        Some((0, 0, 0xe0, 255))
    );
}

#[test]
fn none_and_a_blur_of_nothing_blur_nothing() {
    for filter in ["none", "blur(0)", "blur()"] {
        let (list, canvas) = draw(
            BOX,
            &format!("{STRIPES} #a {{ backdrop-filter: {filter} }}"),
        );
        assert!(!list.to_outline().contains("backdrop"), "{filter}");
        assert!(list.issues().is_empty(), "{filter}");
        assert_eq!(red(&canvas, 39, 50), 0, "{filter}");
        assert_eq!(red(&canvas, 45, 50), 255, "{filter}");
    }
}
