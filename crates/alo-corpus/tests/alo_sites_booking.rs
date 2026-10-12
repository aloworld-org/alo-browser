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
    // 0.6rem of padding above and below and a 1 px border: 48.3875, and
    // 48.375 on the grid (ADR 0054). The padding is Chrome's, 9.59375
    // truncated; the field's line is its computed `line-height`, not
    // Chrome's, which makes the field 50.375 there (queue item 467).
    assert!(near(field.origin.x, 432.0), "{field:?}");
    assert!(near(field.size.width, 343.0), "{field:?}");
    assert!(near(field.size.height, 48.375), "{field:?}");
    // The field stands on the baseline of the line it would hold, so its line
    // is its own 48.375 and the button's paragraph starts the label's
    // paragraph's 17 px margin below the field. With the field on its bottom
    // edge the line was 56.11, the strut's 7.71 below the baseline added.
    assert!(
        near(button.origin.y, field.origin.y + field.size.height + 17.0),
        "{button:?} under {field:?}"
    );
    // So the form is 161.9375 tall: the label's line, 17 px at 1.6 floored
    // to 1/64 (ADR 0050 § 3), 27.1875, + 4 + 48.375 + 17 + 48.375 + 17. With
    // its 1rem top margin, 177.9375 centred in the rows' 274.75 from 25:
    // 89.40625, the share above it truncated onto the grid (ADR 0054 § 3).
    // Chrome 154's form is 163.9375 tall at 88.40625: its field is 2 px
    // taller (item 467), half of which moves the centred form up. Before
    // item 466 it was 161.9875 at 89.38125, before item 384 169.71 at 85.54,
    // and before item 460 162 at 89.4.
    assert!(near(form.size.height, 161.9375), "{form:?}");
    assert!(near(form.origin.y, 89.406_25), "{form:?}");
    assert!(near(field.origin.y, 120.593_75), "{field:?}");
    assert!(near(button.origin.y, 185.968_75), "{button:?}");
}

/// The border box of the `nth` element named `tag`, in document order, in
/// sixty-fourths of a pixel.
fn units_of(rendering: &Rendering, tag: &str, nth: usize) -> Option<[f32; 4]> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let node = document
        .descendants(document.root())
        .filter(|id| {
            document
                .element(*id)
                .is_some_and(|element| &*element.name.local == tag)
        })
        .nth(nth)?;
    let boxes = &drawing.boxes;
    let id = boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|box_node| &box_node.kind),
            Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
        )
    })?;
    let rect = drawing.layout.get(id)?.border_box;
    Some([
        rect.origin.x * 64.0,
        rect.origin.y * 64.0,
        rect.size.width * 64.0,
        rect.size.height * 64.0,
    ])
}

/// Whether two boxes in units are the same to the bit: every number here is
/// a whole number of units, which an `f32` holds exactly.
fn same(left: [f32; 4], right: [f32; 4]) -> bool {
    left.map(f32::to_bits) == right.map(f32::to_bits)
}

/// Queue item 466's closing condition: every box on the page is Chrome
/// 154's to its 1/64 px layout unit (ADR 0054), except what the day field's
/// own line moves, which is item 467's.
///
/// Chrome's numbers were read over `DevTools` on 2026-10-12 from the frozen
/// page and sheet, given the corpus's `DejaVu Sans`, at 800 × 600, in units.
#[test]
fn every_box_is_chromes_but_what_the_fields_own_line_moves() {
    let Some((_, rendering)) = booking() else {
        panic!("the case renders");
    };
    let chrome: [(&str, usize, [f32; 4]); 9] = [
        ("a", 0, [-1_022_976.0, 0.0, 10312.0, 2764.0]),
        ("main", 0, [0.0, 0.0, 51200.0, 20784.0]),
        ("section", 0, [0.0, 0.0, 51200.0, 20784.0]),
        ("h2", 0, [1600.0, 1600.0, 21952.0, 2150.0]),
        ("h3", 0, [1600.0, 5670.0, 21952.0, 1382.0]),
        ("p", 0, [1600.0, 8652.0, 21952.0, 1740.0]),
        ("p", 1, [1600.0, 12504.0, 21952.0, 1740.0]),
        ("p", 2, [1600.0, 16356.0, 21952.0, 1740.0]),
        ("button", 0, [27648.0, 11966.0, 11499.0, 3096.0]),
    ];
    // The skip link's width is its text's, rounded up onto the grid (ADR
    // 0054 § 4); the button's is its text's and 0.6rem of padding each
    // side, truncated (§ 1).
    for (tag, nth, wanted) in chrome {
        let Some(ours) = units_of(&rendering, tag, nth) else {
            panic!("{tag} {nth} is laid out");
        };
        // Chrome's day field is 128 units taller (item 467): the form is
        // centred, so what is above the field's middle stands 64 lower here
        // and what is below it, the button, 64 higher.
        let [x, y, width, height] = wanted;
        let higher = if tag == "button" { 64.0 } else { 0.0 };
        let expected = [x, y - higher, width, height];
        assert!(
            same(ours, expected),
            "{tag} {nth}: {ours:?}, not {expected:?}"
        );
    }
    let Some(form) = units_of(&rendering, "form", 0) else {
        panic!("the form is laid out");
    };
    assert!(
        same(form, [27648.0, 5658.0 + 64.0, 21952.0, 10492.0 - 128.0]),
        "{form:?}"
    );
    let Some(label) = units_of(&rendering, "label", 0) else {
        panic!("the label is laid out");
    };
    assert!(
        same(label, [27648.0, 5658.0 + 64.0, 21952.0, 1740.0]),
        "{label:?}"
    );
    let Some(field) = units_of(&rendering, "input", 0) else {
        panic!("the field is laid out");
    };
    assert!(
        same(field, [27648.0, 7654.0 + 64.0, 21952.0, 3224.0 - 128.0]),
        "{field:?}"
    );
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
    assert_eq!(section, Rect::new(0.0, 0.0, 800.0, 324.75));
    assert_eq!((heading.origin.x, heading.origin.y), (25.0, 25.0));
    // Two `1fr` tracks either side of a 4rem gap in 750: 343 each.
    assert!(near(heading.size.width, 343.0), "{heading:?}");
    // Every line is floored to 1/64 as Chrome floors it (ADR 0050 § 3), and
    // every number here is Chrome 154's, measured with the corpus's font:
    // 33.59375 of heading (28 px at 1.2), its 14 below, and the 1rem row gap.
    assert!(near(name.origin.y, 88.593_75), "{name:?}");
    // The last paragraph ends at 255.5625 + 27.1875 + 17 = 299.75, the
    // content's end, and the section's 24 + 1 below make it 324.75.
    assert!(near(place.origin.y, 255.5625), "{place:?}");
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
    // 27.2 line 0.6rem (9.59375 on the grid) and a border below the field's
    // top, and the text's 20 in it, with the top half of the leading floored
    // above it (ADR 0050 § 4): 7.19 of leading, half of it 3.59, floored 3.
    assert!(near(text.origin.x, 445.0), "{text:?}");
    assert!(near(text.size.height, 20.0), "{text:?}");
    assert!(
        near(text.origin.y, 120.593_75 + 1.0 + 9.593_75 + 3.0),
        "{text:?}"
    );

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
        .position(|line| line.starts_with("date \"Choose a day\" [required invalid] at"))
    else {
        panic!("the field is a date, named, required and empty:\n{outline}");
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
