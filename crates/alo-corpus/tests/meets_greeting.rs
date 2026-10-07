/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Meet's greeting, in numbers: the page that opened queue items 310 and 312.
//!
//! `cases/alo-meet-greeting` pins the whole page as files. This says the
//! parts the items are closed by out loud: the `<img>` showing alo's waving
//! hand is the 20 × 20 box Meet's stylesheet asks for, the hand is drawn into
//! it as the file's two shapes in the file's own two colours, and it stands
//! where `vertical-align: middle` puts it. And `.module`'s background, two
//! layers, is drawn as both of them: the tint in the top right corner over
//! the page's colour (queue item 313).

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::{DisplayItem, Paint};
use alo_value::Rgba;

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

#[test]
fn the_tint_is_drawn_in_the_top_right_corner_over_the_pages_colour() {
    let Some((rendering, _, _)) = greeting() else {
        panic!("the case renders and has an <img>");
    };
    let Some(drawing) = rendering.drawing() else {
        panic!("a drawing");
    };
    let pixel = |x: u32, y: u32| {
        drawing
            .canvas
            .at(x, y)
            .map_or_else(|| panic!("({x}, {y}) is on the canvas"), Rgba::to_rgba8)
    };
    let near = |found: (u8, u8, u8, u8), wanted: (f32, f32, f32), why: &str| {
        let off = [
            (f32::from(found.0) - wanted.0).abs(),
            (f32::from(found.1) - wanted.1).abs(),
            (f32::from(found.2) - wanted.2).abs(),
        ];
        assert!(
            off.iter().all(|off| *off <= 1.5),
            "{why}: {found:?}, wanted {wanted:?}"
        );
    };
    let tint = (248.0, 214.0, 204.0);
    let page = (244.0, 241.0, 236.0);

    // `circle at 92% 0` of the 800 × 220 `.module`: centred at (736, 0), and
    // its farthest corner is the bottom left, so the tint is transparent
    // 26% of √(736² + 220²) out.
    let reach = 736.0_f32.hypot(220.0) * 0.26;
    near(pixel(736, 0), tint, "the tint itself at its centre");
    // Half way out, half the tint over the page: premultiplied, so the
    // colour is the tint's and only its opacity has halved.
    let half = |tint: f32, page: f32| f32::midpoint(tint, page);
    #[expect(clippy::cast_possible_truncation, reason = "a pixel on the canvas")]
    #[expect(clippy::cast_sign_loss, reason = "left of the centre and positive")]
    let halfway = (736.0 - reach / 2.0).round() as u32;
    near(
        pixel(halfway, 0),
        (
            half(tint.0, page.0),
            half(tint.1, page.1),
            half(tint.2, page.2),
        ),
        "half way out",
    );
    #[expect(clippy::cast_possible_truncation, reason = "a pixel on the canvas")]
    #[expect(clippy::cast_sign_loss, reason = "left of the centre and positive")]
    let past = (736.0 - reach - 2.0).floor() as u32;
    near(
        pixel(past, 0),
        page,
        "just past the tint, the page's colour",
    );
    near(pixel(10, 210), page, "and in the far corner");

    let refused: Vec<String> = drawing
        .display
        .issues()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(
        refused.is_empty(),
        "nothing about the background is refused: {refused:?}"
    );
}
