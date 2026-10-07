/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Meet's greeting, in numbers: the page that opened queue item 310.
//!
//! `cases/alo-meet-greeting` pins the whole page as files. This says the
//! part the item is closed by out loud: the `<img>` showing alo's waving hand
//! is the 20 × 20 box Meet's stylesheet asks for, and the hand is drawn into
//! it as the file's two shapes in the file's own two colours.

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
    // `.content`'s 24 px padding and `.header`'s 20 px top margin. Its
    // bottom sits on the line's baseline, because `vertical-align: middle`
    // is not read yet (queue item 312); the case moves when it is.
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
    assert!(
        (content.top() - 44.0).abs() < 0.001,
        "on the greeting's line"
    );
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
