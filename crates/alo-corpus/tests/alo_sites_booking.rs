/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' booking section, in numbers: the page that opened queue items
//! 384 and 385.
//!
//! `cases/alo-sites-booking` pins the whole page as files. This says out loud
//! what items 384 and 385 are closed by. The page's form holds a day field
//! nobody has typed into, beside its label. Until item 384 a field with no
//! line stood on its bottom margin edge, so its line was a font's descent
//! taller than the field, and the form, centred down the grid, stood higher
//! than a browser draws it. Until item 385 the field drew nothing and an
//! agent read it as a `generic` box; it draws how a date is written now, and
//! reads as a `date` (ADR 0042).
//!
//! It is loaded as the call-to-action section is (`alo_sites_cta.rs`), from
//! the same frame, sheet and analytics script, so how its sheet is asked for
//! and answered is said once there and only checked here.

use alo_agent::AgentTree;
use alo_box::BoxKind;
use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::{Canvas, DisplayItem, DisplayList, render};
use alo_value::Rgba;

/// The case, read and rendered.
fn booking() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-booking"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some((case, rendering))
}

/// The border box of the first element named `tag` whose `class` attribute
/// starts with `class`.
fn found(rendering: &Rendering, tag: &str, class: &str) -> Option<Rect> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let node = document.descendants(document.root()).find(|id| {
        document.element(*id).is_some_and(|element| {
            &*element.name.local == tag
                && element.attr("class").unwrap_or_default().starts_with(class)
        })
    })?;
    let boxes = &drawing.boxes;
    let id = boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|box_node| &box_node.kind),
            Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
        )
    })?;
    Some(drawing.layout.get(id)?.border_box)
}

/// The rectangle of the text the day field shows, which the box tree made
/// for it: a text box whose node is the `<input>` itself.
fn field_text(rendering: &Rendering) -> Option<Rect> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let input = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.name.is_html("input"))
    })?;
    let boxes = &drawing.boxes;
    let id = boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|box_node| &box_node.kind),
            Some(BoxKind::Text { node, .. }) if *node == input
        )
    })?;
    Some(drawing.layout.get(id)?.border_box)
}

/// The page drawn from its display list with every run of text taken out:
/// what the boxes paint for themselves.
fn without_text(rendering: &Rendering) -> Option<Canvas> {
    let drawing = rendering.drawing()?;
    let mut boxes_only = DisplayList::default();
    for item in drawing.display.items() {
        if !matches!(item, DisplayItem::Text { .. }) {
            boxes_only.push(item.clone());
        }
    }
    let mut canvas = Canvas::new(drawing.canvas.width(), drawing.canvas.height(), Rgba::WHITE);
    render(&boxes_only, &mut canvas);
    Some(canvas)
}

/// The whole pixels inside `rect`, inset by `inset` on every side.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a rectangle on an 800 × 325 page"
)]
fn pixels_inside(rect: Rect, inset: f32) -> impl Iterator<Item = (u32, u32)> {
    let left = (rect.origin.x + inset).ceil() as u32;
    let top = (rect.origin.y + inset).ceil() as u32;
    let right = (rect.origin.x + rect.size.width - inset).floor() as u32;
    let bottom = (rect.origin.y + rect.size.height - inset).floor() as u32;
    (top..bottom).flat_map(move |y| (left..right).map(move |x| (x, y)))
}

/// Equal to within a thousandth of a pixel.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

#[test]
fn its_sheet_is_asked_for_and_answered_from_what_was_frozen() {
    let Some((case, Rendering::Loaded(_, answered))) = booking() else {
        panic!("the case is loaded by a renderer");
    };
    assert_eq!(
        case.address.as_deref(),
        Some("https://nordwind.alosites.com/")
    );
    assert_eq!(answered.sheets, 1);
    assert!(answered.unfrozen.is_empty(), "{:?}", answered.unfrozen);
    assert!(answered.said.is_empty(), "{:?}", answered.said);
    assert!(answered.issues.is_empty(), "{:?}", answered.issues);
}

#[test]
fn the_empty_day_field_makes_a_line_its_own_height() {
    let Some((_, rendering)) = booking() else {
        panic!("the case renders");
    };
    let (Some(form), Some(field), Some(button)) = (
        found(&rendering, "form", "booking-day"),
        found(&rendering, "input", ""),
        found(&rendering, "button", ""),
    ) else {
        panic!("the form, its field and its button are laid out");
    };
    // `width: 100%` of the second track: 343 wide at 432. A 27.2 line,
    // 0.6rem of padding above and below and a 1 px border: 48.4 tall.
    assert!(near(field.origin.x, 432.0), "{field:?}");
    assert!(near(field.size.width, 343.0), "{field:?}");
    assert!(near(field.size.height, 48.4), "{field:?}");
    // The field stands on the baseline of the line it would hold, so its line
    // is its own 48.4 and the button's paragraph starts the label's
    // paragraph's 17 px margin below the field. With the field on its bottom
    // edge the line was 56.11, the strut's 7.71 below the baseline added.
    assert!(
        near(button.origin.y, field.origin.y + field.size.height + 17.0),
        "{button:?} under {field:?}"
    );
    // So the form is 162 tall: 27.2 + 4 + 48.4 + 17 + 48.4 + 17. With its
    // 1rem top margin, 178 centred in the rows' 274.8 from 25: 89.4. Before
    // item 384 it was 169.71 at 85.54.
    assert!(near(form.size.height, 162.0), "{form:?}");
    assert!(near(form.origin.y, 89.4), "{form:?}");
    assert!(near(field.origin.y, 120.6), "{field:?}");
    assert!(near(button.origin.y, 186.0), "{button:?}");
}

#[test]
fn the_offer_is_laid_out_as_the_sheet_says() {
    let Some((_, rendering)) = booking() else {
        panic!("the case renders");
    };
    let (Some(section), Some(heading), Some(name), Some(place)) = (
        found(&rendering, "section", "s-booking"),
        found(&rendering, "h2", ""),
        found(&rendering, "h3", ""),
        found(&rendering, "p", "booking-where"),
    ) else {
        panic!("the section and its offer are laid out");
    };
    // `booking-layout-split`'s 62rem is wider than the page, so the section
    // is the whole 800; 1.5rem of padding in a 1 px border.
    assert_eq!(section, Rect::new(0.0, 0.0, 800.0, 324.8));
    assert_eq!((heading.origin.x, heading.origin.y), (25.0, 25.0));
    // Two `1fr` tracks either side of a 4rem gap in 750: 343 each.
    assert!(near(heading.size.width, 343.0), "{heading:?}");
    // 33.6 of heading, its 14 below, and the 1rem row gap.
    assert!(near(name.origin.y, 88.6), "{name:?}");
    // The last paragraph ends at 255.6 + 27.2 + 17 = 299.8, the content's
    // end, and the section's 24 + 1 below make it 324.8.
    assert!(near(place.origin.y, 255.6), "{place:?}");
}

#[test]
fn the_empty_day_field_draws_how_a_date_is_written_in_its_own_colour() {
    let Some((_, rendering)) = booking() else {
        panic!("the case renders");
    };
    let (Some(drawing), Some(field), Some(text)) = (
        rendering.drawing(),
        found(&rendering, "input", ""),
        field_text(&rendering),
    ) else {
        panic!("the field and the text it shows are laid out");
    };
    // ADR 0042 § 2: with no region chosen, ISO 8601's order. § 3: as the
    // field's own text, in its computed colour — `color: inherit`, so the
    // page's `--text`, #17212b — with its pen at the content edge, 432 + 1
    // of border + 0.75rem of padding.
    let Some((origin, color)) = drawing.display.items().iter().find_map(|item| match item {
        DisplayItem::Text {
            text,
            origin,
            color,
            ..
        } if text == "yyyy-mm-dd" => Some((*origin, *color)),
        _ => None,
    }) else {
        panic!("the format is drawn");
    };
    assert!(near(origin.0, 445.0), "{origin:?}");
    assert_eq!(color.to_rgba8(), (23, 33, 43, 255));
    // On the field's own line, which 384 already stood the field on: the
    // 27.2 line 0.6rem and a border below the field's top, the text's
    // 19.79 centred in it.
    assert!(near(text.origin.x, 445.0), "{text:?}");
    assert!(near(
        text.origin.y,
        120.6 + 1.0 + 9.6 + (27.2 - text.size.height) / 2.0
    ));

    // In pixels: the format's ink is in the text's colour and inside the
    // text's own rectangle…
    let canvas = &drawing.canvas;
    let darkest = pixels_inside(text, 0.0)
        .filter_map(|(x, y)| canvas.at(x, y))
        .map(Rgba::to_rgba8)
        .min_by_key(|(red, green, blue, _)| u32::from(*red) + u32::from(*green) + u32::from(*blue));
    let Some((red, green, blue, _)) = darkest else {
        panic!("the text's rectangle is on the canvas");
    };
    assert!(
        red <= 23 + 40 && green <= 33 + 40 && blue <= 43 + 40,
        "the darkest ink is ({red}, {green}, {blue})"
    );
    let outside = Rect::new(
        text.origin.x - 1.0,
        text.origin.y - 1.0,
        text.size.width + 2.0,
        text.size.height + 2.0,
    );
    // Three pixels in from the border box, which clears the 1 px border and
    // the curve of its 0.5rem corners.
    for (x, y) in pixels_inside(field, 3.0) {
        let within = f64::from(x) >= f64::from(outside.origin.x)
            && f64::from(x) < f64::from(outside.origin.x + outside.size.width)
            && f64::from(y) >= f64::from(outside.origin.y)
            && f64::from(y) < f64::from(outside.origin.y + outside.size.height);
        if !within {
            assert_eq!(
                canvas.at(x, y).map(Rgba::to_rgba8),
                Some((255, 255, 255, 255)),
                "ink at ({x}, {y}), outside the format's rectangle"
            );
        }
    }
    // …and the field paints nothing around it: with the text taken out,
    // everything inside the field's border is its white background. Until
    // item 385 found it, the text box painted the field's background and
    // border again, as a ring around whatever a field showed.
    let Some(bare) = without_text(&rendering) else {
        panic!("the page draws");
    };
    for (x, y) in pixels_inside(field, 3.0) {
        assert_eq!(
            bare.at(x, y).map(Rgba::to_rgba8),
            Some((255, 255, 255, 255)),
            "the field painted ({x}, {y}) for its text"
        );
    }
}

#[test]
fn an_agent_reads_the_day_field_as_a_date_with_nothing_in_it() {
    let Some((_, rendering)) = booking() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let outline = AgentTree::new(document, &drawing.boxes, &drawing.layout).to_outline();
    let lines: Vec<&str> = outline.lines().map(str::trim_start).collect();
    let Some(day) = lines
        .iter()
        .position(|line| line.starts_with("date \"Choose a day\" [required] at"))
    else {
        panic!("the field is a date, named and required:\n{outline}");
    };
    // No value beneath it: the format is how a date is written, not one.
    assert!(
        lines
            .get(day + 1)
            .is_none_or(|next| !next.starts_with("text")),
        "{outline}"
    );
    assert!(!outline.contains("yyyy-mm-dd"), "{outline}");
}
