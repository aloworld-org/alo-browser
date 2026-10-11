/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A checkbox and a radio beside text, in numbers: the case that closes
//! queue item 443.
//!
//! `cases/checkbox-beside-text` pins the page as files. This says what
//! Chrome 154 gives for the same bytes with the corpus's font: how far
//! across each control and the text after it stand, to within Chrome's
//! 1/64 px layout unit, which is the user-agent sheet's margins; and how far
//! down each control stands in its row, which is its bottom border edge on
//! the baseline. Until item 443 a checkbox had no margin and stood on its
//! bottom margin edge, which with no margin was the same place.
//!
//! Down is asserted to within 1/64 px of Chrome too, and so is each row's
//! height: the case that closes queue item 460. Chrome takes the font's
//! ascent and descent in whole pixels and floors the half-leading (ADR
//! 0050): at 16 px `DejaVu Sans`' ascent is 14.85 and a line takes 15, and
//! at 32 px 29.7 and 30. Until item 460 this engine kept them exact, and
//! these were asserted to within 0.75 px: by the last row a control stood at
//! y 160.05 here and 162 in Chrome.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case = Case::read(&cases_directory().join("checkbox-beside-text"))?;
    Rendering::of(&case).ok()
}

/// A row as laid out here: its border box, the border boxes of its
/// controls and labels, and the text boxes directly in it, in order.
struct Laid {
    row: Rect,
    controls: Vec<Rect>,
    texts: Vec<Rect>,
}

/// The row whose `id` is `wanted`, as laid out here.
fn row(rendering: &Rendering, wanted: &str) -> Option<Laid> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let boxes = &drawing.boxes;
    let node = document
        .descendants(document.root())
        .find(|id| document.element(*id).and_then(|e| e.attr("id")) == Some(wanted))?;
    let rect = |id| drawing.layout.get(id).map(|held| held.border_box);
    let of_node = |node| {
        boxes.ids().find(|id| {
            matches!(
                boxes.get(*id).map(|box_node| &box_node.kind),
                Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
            )
        })
    };
    let row = rect(of_node(node)?)?;
    let controls = document
        .descendants(node)
        .filter(|id| {
            document
                .element(*id)
                .is_some_and(|e| e.name.is_html("input") || e.name.is_html("label"))
        })
        .filter_map(|id| rect(of_node(id)?))
        .collect();
    let texts = document
        .children(node)
        .filter(|id| document.element(*id).is_none())
        .filter_map(|text| {
            let id = boxes.ids().find(|id| {
                matches!(
                    boxes.get(*id).map(|box_node| &box_node.kind),
                    Some(alo_box::BoxKind::Text { node: of, .. }) if *of == text
                )
            })?;
            rect(id)
        })
        .collect();
    Some(Laid {
        row,
        controls,
        texts,
    })
}

/// Within Chrome's layout unit.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() <= 1.0 / 64.0
}

/// A row as Chrome 154 lays it out: its `id`, its height, its font size,
/// each control's (or label's) across and down from the row's top, and
/// where across each text after the first starts.
type Row = (
    &'static str,
    f32,
    f32,
    &'static [(f32, f32)],
    &'static [f32],
);

/// Each row, as Chrome 154 lays it out.
const ROWS: [Row; 8] = [
    (
        "checkbox",
        20.0,
        16.0,
        &[(1534.0 / 64.0, 3.0)],
        &[2558.0 / 64.0],
    ),
    (
        "radio",
        20.0,
        16.0,
        &[(1598.0 / 64.0, 3.0)],
        &[2622.0 / 64.0],
    ),
    ("flex-row", 19.0, 16.0, &[(4.0, 3.0), (28.0, 0.0)], &[]),
    (
        "taller",
        24.0,
        16.0,
        &[(1534.0 / 64.0, 4.0), (4091.0 / 64.0, 4.0)],
        &[2558.0 / 64.0, 5115.0 / 64.0],
    ),
    (
        "larger",
        38.0,
        32.0,
        &[(2811.0 / 64.0, 17.0)],
        &[3835.0 / 64.0],
    ),
    (
        "no-margin",
        19.0,
        16.0,
        &[(1278.0 / 64.0, 2.0)],
        &[2110.0 / 64.0],
    ),
    (
        "radio-no-margin",
        19.0,
        16.0,
        &[(1278.0 / 64.0, 2.0)],
        &[2110.0 / 64.0],
    ),
    (
        "sized",
        27.0,
        16.0,
        &[(1534.0 / 64.0, 3.0)],
        &[3006.0 / 64.0],
    ),
];

/// `DejaVu Sans`' ascent, as a fraction of its size: 1901 of 2048 units.
/// A line takes it in whole pixels, half up (ADR 0050 § 1).
const ASCENT: f32 = 1901.0 / 2048.0;

#[test]
fn each_control_stands_where_chrome_puts_it() {
    let Some(rendering) = case() else {
        panic!("the case renders");
    };
    for (id, height, _, controls, texts) in ROWS {
        let Some(laid) = row(&rendering, id) else {
            panic!("#{id} is laid out");
        };
        assert!(
            near(laid.row.size.height, height),
            "#{id} is {} tall, where Chrome's is {height}",
            laid.row.size.height,
        );
        assert_eq!(laid.controls.len(), controls.len(), "#{id}'s controls");
        for (control, (across, down)) in laid.controls.iter().zip(controls) {
            assert!(
                near(control.origin.x, *across),
                "#{id}'s control is {} across, where Chrome's is {across}",
                control.origin.x,
            );
            let own = control.origin.y - laid.row.origin.y;
            assert!(
                near(own, *down),
                "#{id}'s control is {own} down its row, where Chrome's is {down}",
            );
        }
        // The text after each control: as far past it as its right margin.
        let after: Vec<f32> = laid.texts.iter().skip(1).map(|t| t.origin.x).collect();
        assert_eq!(after.len(), texts.len(), "#{id}'s texts");
        for (here, chrome) in after.iter().zip(texts) {
            assert!(
                near(*here, *chrome),
                "#{id}'s text is {here} across, where Chrome's is {chrome}",
            );
        }
    }
}

#[test]
fn a_control_beside_text_stands_on_its_bottom_border_edge() {
    let Some(rendering) = case() else {
        panic!("the case renders");
    };
    for (id, _, size, _, _) in ROWS {
        if id == "flex-row" {
            // Centred by its flex container, not on a baseline.
            continue;
        }
        let Some(laid) = row(&rendering, id) else {
            panic!("#{id} is laid out");
        };
        let Some(text) = laid.texts.first() else {
            panic!("#{id} has text");
        };
        let baseline = text.origin.y + (ASCENT * size + 0.5).floor();
        for control in &laid.controls {
            assert!(
                near(control.origin.y + control.size.height, baseline),
                "#{id}'s control ends at {}, where the baseline is {baseline}",
                control.origin.y + control.size.height,
            );
        }
    }
}
