/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Meet's greeting, in numbers: the page that opened queue items 310 and 312.
//!
//! `cases/alo-meet-greeting` pins the whole page as files. This says the
//! parts the items are closed by out loud: the `<img>` showing alo's waving
//! hand is the 20 × 20 box Meet's stylesheet asks for, the hand is drawn into
//! it as the file's two shapes in the file's own two colours, and it stands
//! where `vertical-align: middle` puts it.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::{DisplayItem, Paint};

/// The case, rendered, and the `<img>`'s box id and content box.
fn greeting() -> Option<(Rendering, alo_box::BoxId, Rect)> {
    let case = Case::read(&cases_directory().join("alo-meet-greeting"))?;
    let rendering = Rendering::of(&case).ok()?;
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let id = drawing.boxes.ids().find(|id| {
        matches!(
            drawing.boxes.get(*id).map(|node| &node.kind),
            Some(alo_box::BoxKind::Element { node, .. })
                if document.element(*node).is_some_and(|element| element.name.is_html("img"))
        )
    })?;
    let content = drawing.layout.get(id)?.content_box();
    Some((rendering, id, content))
}

#[test]
fn the_hand_is_a_twenty_pixel_square_on_the_greetings_line() {
    let Some((rendering, _, content)) = greeting() else {
        panic!("the case renders and has an <img>");
    };
    assert_eq!(
        (content.size.width, content.size.height),
        (20.0, 20.0),
        "--icon-size-control is 1.25rem",
    );
    // After "Good morning ", its space, and `margin-left: var(--space-1)`,
    // which is 4. A text box's width stops before its trailing space, and a
    // space in a 13 px face is less than half its size.
    let Some(drawing) = rendering.drawing() else {
        panic!("a drawing");
    };
    let text_right =
        drawing
            .boxes
            .ids()
            .find_map(|id| match drawing.boxes.get(id).map(|node| &node.kind) {
                Some(alo_box::BoxKind::Text { text, .. }) if text.starts_with("Good morning") => {
                    let rect = drawing.layout.get(id)?.border_box;
                    Some(rect.left() + rect.size.width)
                }
                _ => None,
            });
    let Some(text_right) = text_right else {
        panic!("the greeting's text");
    };
    let gap = content.left() - 4.0 - text_right;
    assert!(
        (0.0..6.5).contains(&gap),
        "a gap of {gap} before the margin"
    );
    // `.content`'s 24 px padding and `.header`'s 20 px top margin: the hand
    // is the tallest thing on the greeting's line, so it is the line's top.
    assert!(
        (content.top() - 44.0).abs() < 0.001,
        "on the greeting's line"
    );
}

#[test]
fn the_hand_is_middle_aligned_with_the_greetings_lowercase_letters() {
    let Some((rendering, _, content)) = greeting() else {
        panic!("the case renders and has an <img>");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("a drawing");
    };
    // `vertical-align: middle`: the hand's midpoint at the greeting's
    // baseline plus half the x-height of its 13 px semibold face, which is
    // DejaVu Sans Bold's own `x` (queue item 312).
    let Some((baseline, x_height)) = drawing.display.items().iter().find_map(|item| match item {
        DisplayItem::Text {
            text,
            origin,
            font,
            size,
            ..
        } if text.starts_with("Good morning") => Some((origin.1, font.metrics(*size).x_height)),
        _ => None,
    }) else {
        panic!("the greeting's text");
    };
    assert!(
        (x_height - 13.0 * 1120.0 / 2048.0).abs() < 0.001,
        "{x_height}"
    );
    let middle = content.top() + content.size.height / 2.0;
    assert!(
        (middle - (baseline - x_height / 2.0)).abs() < 0.001,
        "the hand's middle at {middle}, the baseline at {baseline}",
    );
    // The line is the hand's 20: what moved fits inside it, and nothing else
    // reaches past it.
    let line = drawing.boxes.ids().find_map(|id| {
        match drawing.boxes.get(id).map(|node| &node.kind) {
            Some(alo_box::BoxKind::Element { .. })
                if drawing.boxes.children(id).any(|child| {
                    matches!(
                        drawing.boxes.get(child).map(|node| &node.kind),
                        Some(alo_box::BoxKind::Text { text, .. }) if text.starts_with("Good morning")
                    )
                }) =>
            {
                drawing.layout.get(id).map(|held| held.content_box().size.height)
            }
            _ => None,
        }
    });
    assert_eq!(line, Some(20.0), "the greeting is one line of 20");
}

#[test]
fn the_hand_is_drawn_as_the_files_two_shapes_inside_the_box() {
    let Some((rendering, id, content)) = greeting() else {
        panic!("the case renders and has an <img>");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("a drawing");
    };
    let fills: Vec<_> = drawing
        .display
        .items()
        .iter()
        .filter_map(|item| match item {
            DisplayItem::Fill {
                box_id,
                path,
                paint: Paint::Solid(color),
                ..
            } if *box_id == id => Some((color.to_rgba8(), path.bounds())),
            _ => None,
        })
        .collect();
    let colours: Vec<_> = fills.iter().map(|(colour, _)| *colour).collect();
    assert_eq!(
        colours,
        vec![(0xE7, 0x6F, 0x51, 255), (0x10, 0x2A, 0x43, 255)],
        "terracotta, then navy, as the file paints them",
    );
    for (_, bounds) in &fills {
        let Some((left, top, right, bottom)) = *bounds else {
            panic!("a shape with no extent");
        };
        assert!(
            left >= content.left()
                && top >= content.top()
                && right <= content.left() + content.size.width
                && bottom <= content.top() + content.size.height,
            "{bounds:?} inside {content:?}",
        );
    }
    assert!(
        drawing.display.items().iter().any(|item| matches!(
            item,
            DisplayItem::PushClip { box_id, .. } if *box_id == id
        )),
        "confined to its box",
    );
    assert!(
        !drawing
            .display
            .items()
            .iter()
            .any(|item| matches!(item, DisplayItem::Picture { .. })),
        "drawn as vectors, never as a raster",
    );
}
