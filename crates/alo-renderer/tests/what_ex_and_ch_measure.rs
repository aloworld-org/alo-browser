/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `ex` and `ch` are measured in the face an element's text is set in.
//!
//! Queue item 320. Until it, the cascade answered both with half the font
//! size, and alo's downloads page laid its `max-width: 44ch` paragraph out
//! at 380.16 where the corpus sans-serif's `0` makes it 483.74. These are the numbers
//! through the whole pipeline, with the face's own measurements as the
//! expectation rather than a figure copied from a run.

use alo_layout::geometry::Size;
use alo_renderer::pipeline::{Rendered, render};
use alo_text::{Font, FontDatabase, Slant, Weight};

/// Within far less than a pixel.
fn close(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.001
}

/// The `dejavu` crate's sans in two weights and its sans mono, with the
/// sans as the generic sans-serif.
fn dejavu_fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (family, weight, data) in [
        ("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
        ("DejaVu Sans", Weight::BOLD, dejavu::sans::bold()),
        (
            "DejaVu Sans Mono",
            Weight::NORMAL,
            dejavu::sans_mono::regular(),
        ),
    ] {
        if let Some(font) = Font::load(family, weight, Slant::Normal, data.to_vec()) {
            database.add(font);
        }
    }
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

/// A face this engine can read that maps no character at all, so it has
/// neither an `x` nor a `0`: the three tables a face cannot do without
/// (`head`, `hhea`, `maxp`) and nothing else.
fn a_face_with_no_characters() -> Vec<u8> {
    let mut head = vec![0u8; 54];
    head.splice(0..4, [0, 1, 0, 0]); // version 1.0
    head.splice(12..16, [0x5F, 0x0F, 0x3C, 0xF5]); // the magic number
    head.splice(18..20, 1000u16.to_be_bytes()); // units per em
    let mut hhea = vec![0u8; 36];
    hhea.splice(0..4, [0, 1, 0, 0]);
    hhea.splice(4..6, 800i16.to_be_bytes()); // ascender
    hhea.splice(6..8, (-200i16).to_be_bytes()); // descender
    let maxp = vec![0, 0, 0x50, 0, 0, 1]; // version 0.5, one glyph
    let tables: [(&[u8; 4], Vec<u8>); 3] = [(b"head", head), (b"hhea", hhea), (b"maxp", maxp)];

    let mut font = vec![0, 1, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0];
    let mut offset = 12 + 16 * tables.len();
    for (tag, data) in &tables {
        font.extend_from_slice(*tag);
        font.extend_from_slice(&[0; 4]); // checksum, which nothing verifies
        font.extend_from_slice(&u32::try_from(offset).unwrap_or(0).to_be_bytes());
        font.extend_from_slice(&u32::try_from(data.len()).unwrap_or(0).to_be_bytes());
        offset += data.len();
    }
    for (_, data) in &tables {
        font.extend_from_slice(data);
    }
    font
}

/// A page rendered 800 wide with these fonts.
fn rendered(html: &str, css: &str, fonts: &FontDatabase) -> Rendered {
    render(
        html,
        &format!("body {{ margin: 0 }}\n{css}"),
        Size {
            width: 800.0,
            height: 600.0,
        },
        fonts,
    )
}

/// The border box size of the element with this `id`.
fn size_of(page: &Rendered, id: &str) -> Option<(f32, f32)> {
    let boxes = &page.drawing.boxes;
    let found = boxes.ids().find(|box_id| {
        boxes
            .get(*box_id)
            .and_then(|boxed| boxed.kind.node())
            .and_then(|node| page.document.element(node))
            .is_some_and(|element| element.attr("id") == Some(id))
    })?;
    let laid = page.drawing.layout.border_box(found)?;
    Some((laid.size.width, laid.size.height))
}

/// The face's own measurements at a size: its `0`'s advance and its `x`.
fn measured(data: &[u8], size: f32) -> Option<(f32, f32)> {
    let metrics = Font::load("face", Weight::NORMAL, Slant::Normal, data.to_vec())?.metrics(size);
    Some((metrics.zero_width, metrics.x_height))
}

#[test]
fn ch_and_ex_are_the_faces_zero_and_x() {
    let Some((zero, x)) = measured(dejavu::sans::regular(), 17.28) else {
        panic!("DejaVu Sans loads");
    };
    assert!(
        !close(zero, 17.28 * 0.5) && !close(x, 17.28 * 0.5),
        "DejaVu's `0` and `x` are not half an em, so this test can tell"
    );
    let page = rendered(
        r#"<div id="ch"></div><div id="ex"></div>"#,
        "div { font-family: sans-serif; font-size: 17.28px } \
         #ch { width: 44ch; height: 1px } #ex { width: 1px; height: 10ex }",
        &dejavu_fonts(),
    );
    let Some(((ch_width, _), (_, ex_height))) = size_of(&page, "ch").zip(size_of(&page, "ex"))
    else {
        panic!("both boxes are laid out");
    };
    assert!(
        close(ch_width, 44.0 * zero),
        "{ch_width} is not 44 × {zero}"
    );
    assert!(close(ex_height, 10.0 * x), "{ex_height} is not 10 × {x}");
}

#[test]
fn the_face_is_the_one_the_text_is_set_in() {
    let (Some((sans, _)), Some((bold, _)), Some((mono, _))) = (
        measured(dejavu::sans::regular(), 20.0),
        measured(dejavu::sans::bold(), 20.0),
        measured(dejavu::sans_mono::regular(), 20.0),
    ) else {
        panic!("the faces load");
    };
    assert!(
        !close(sans, bold) && !close(sans, mono) && !close(bold, mono),
        "the three `0`s differ, so this can tell"
    );
    let page = rendered(
        r#"<div id="sans"></div><div id="bold"></div><div id="mono"></div>
           <div id="listed"></div>"#,
        "div { font-family: sans-serif; font-size: 20px; width: 10ch; height: 1px } \
         #bold { font-weight: bold } #mono { font-family: 'DejaVu Sans Mono' } \
         #listed { font-family: 'Nobody Has This', 'DejaVu Sans Mono', sans-serif }",
        &dejavu_fonts(),
    );
    for (id, zero) in [
        ("sans", sans),
        ("bold", bold),
        ("mono", mono),
        ("listed", mono),
    ] {
        let Some((width, _)) = size_of(&page, id) else {
            panic!("#{id} is laid out");
        };
        assert!(
            close(width, 10.0 * zero),
            "#{id}: {width} is not 10 × {zero}"
        );
    }
}

#[test]
fn a_font_size_in_ex_is_the_parents_and_a_line_height_the_elements_own() {
    let Some((_, x_at_parent)) = measured(dejavu::sans::regular(), 20.0) else {
        panic!("DejaVu Sans loads");
    };
    let page = rendered(
        r#"<div id="parent"><div id="child"><span>x</span></div></div>"#,
        "#parent { font-family: sans-serif; font-size: 20px } \
         #child { font-size: 2ex; line-height: 3ex; width: 4ex }",
        &dejavu_fonts(),
    );
    // Two of the parent's `x`, at the parent's 20 px.
    let child_size = 2.0 * x_at_parent;
    let Some((_, x_at_child)) = measured(dejavu::sans::regular(), child_size) else {
        panic!("DejaVu Sans loads");
    };
    let Some((width, height)) = size_of(&page, "child") else {
        panic!("the child is laid out");
    };
    assert!(
        close(width, 4.0 * x_at_child),
        "{width}: its own lengths are of its own face, at its own size"
    );
    assert!(
        close(height, 3.0 * x_at_child),
        "{height}: one line of three of its own `x`"
    );
}

#[test]
fn a_face_with_no_zero_and_no_x_is_half_an_em_each() {
    let bytes = a_face_with_no_characters();
    let Some(face) = Font::load("Blank", Weight::NORMAL, Slant::Normal, bytes) else {
        panic!("the blank face loads");
    };
    assert!(!face.has_glyph('0') && !face.has_glyph('x'));
    let mut fonts = FontDatabase::new();
    fonts.add(face);
    let page = rendered(
        r#"<div id="blank"></div>"#,
        "#blank { font-family: Blank; font-size: 30px; width: 10ch; height: 4ex }",
        &fonts,
    );
    let Some((width, height)) = size_of(&page, "blank") else {
        panic!("the box is laid out");
    };
    assert!(close(width, 150.0), "{width}");
    assert!(close(height, 60.0), "{height}");
}

#[test]
fn with_no_fonts_at_all_they_are_half_an_em_each() {
    let page = rendered(
        r#"<div id="none"></div>"#,
        "#none { font-size: 30px; width: 10ch; height: 4ex }",
        &FontDatabase::new(),
    );
    let Some((width, height)) = size_of(&page, "none") else {
        panic!("the box is laid out");
    };
    assert!(
        close(width, 150.0) && close(height, 60.0),
        "{width}×{height}"
    );
}
