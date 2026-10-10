/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's sign-in screen, in numbers: the page that opened queue item 389.
//!
//! `cases/alo-sign-in` pins the whole screen as files. This says out loud
//! what item 389 is closed by (ADR 0043 § 6). The email field has
//! `placeholder="you@company.eu"`, and alo's `Input` draws it
//! `placeholder:text-tertiary` — `.input::placeholder { color:
//! var(--text-tertiary) }`, which the case's sheet now carries. Until this
//! engine styled a pseudo-element the field drew nothing. It draws the hint
//! in `--text-tertiary` now, where its text will be; an agent reads the
//! field as empty with a `placeholder`; and a value hides it.

use alo_agent::{AgentTree, Target, Verb};
use alo_corpus::{Case, Rendering, cases_directory, corpus_fonts};
use alo_layout::{Rect, Size};
use alo_paint::{Canvas, DisplayItem, DisplayList, render};
use alo_renderer::{FromRenderer, Page, Renderer, Snapshot, ToRenderer};
use alo_value::Rgba;

/// `--text-tertiary`, alo's token for a hint.
const TERTIARY: (u8, u8, u8) = (0x7a, 0x6f, 0x62);

/// The case, read and rendered.
fn sign_in() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sign-in"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some((case, rendering))
}

/// The border box of the element whose `id` attribute is `wanted`.
fn element(rendering: &Rendering, wanted: &str) -> Option<Rect> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let node = document
        .descendants(document.root())
        .find(|id| document.element(*id).and_then(|e| e.attr("id")) == Some(wanted))?;
    let boxes = &drawing.boxes;
    let id = boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|held| &held.kind),
            Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
        )
    })?;
    Some(drawing.layout.get(id)?.border_box)
}

/// The rectangle of the email field's hint: the box the box tree made for
/// its `::placeholder`.
fn hint(rendering: &Rendering) -> Option<Rect> {
    let drawing = rendering.drawing()?;
    let boxes = &drawing.boxes;
    let id = boxes.ids().find(|id| boxes.is_placeholder(*id))?;
    Some(drawing.layout.get(id)?.border_box)
}

/// The page drawn from its display list with every run of text taken out.
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
    reason = "a rectangle on a 1000 × 640 page"
)]
fn pixels_inside(rect: Rect, inset: f32) -> impl Iterator<Item = (u32, u32)> {
    let left = (rect.origin.x + inset).ceil() as u32;
    let top = (rect.origin.y + inset).ceil() as u32;
    let right = (rect.origin.x + rect.size.width - inset).floor() as u32;
    let bottom = (rect.origin.y + rect.size.height - inset).floor() as u32;
    (top..bottom).flat_map(move |y| (left..right).map(move |x| (x, y)))
}

/// Whether the pixel `(x, y)` is inside `rect`.
fn within(rect: Rect, x: u32, y: u32) -> bool {
    let (x, y) = (f64::from(x), f64::from(y));
    x >= f64::from(rect.origin.x)
        && x < f64::from(rect.origin.x + rect.size.width)
        && y >= f64::from(rect.origin.y)
        && y < f64::from(rect.origin.y + rect.size.height)
}

/// Equal to within a thousandth of a pixel.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() < 1e-3
}

/// The case loaded into a renderer of its own, which takes verbs.
fn loaded(case: &Case) -> Renderer {
    let mut renderer = Renderer::new(corpus_fonts());
    let size = Size::new(case.size.0, case.size.1);
    renderer.handle(ToRenderer::Load(Box::new(
        Page::new(case.html.clone(), size).with_sheet(case.css.clone()),
    )));
    renderer
}

/// The tree as the renderer sends it, or an empty one when it answered
/// something else, so the assertion that asked is what fails.
fn read(renderer: &mut Renderer) -> Snapshot {
    match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => *snapshot,
        _ => Snapshot::default(),
    }
}

#[test]
fn the_case_carries_the_rule_alos_input_writes() {
    let Some((case, _)) = sign_in() else {
        panic!("the case is read");
    };
    assert!(
        case.css
            .contains(".input::placeholder { color: var(--text-tertiary) }"),
        "Tailwind's `placeholder:text-tertiary`, as `Input.tsx` writes it"
    );
    assert!(case.html.contains("placeholder=\"you@company.eu\""));
}

#[test]
fn the_email_field_draws_its_hint_where_its_text_will_be() {
    let Some((_, rendering)) = sign_in() else {
        panic!("the case renders");
    };
    let (Some(field), Some(hint)) = (element(&rendering, "email"), hint(&rendering)) else {
        panic!("the field and its hint are laid out");
    };
    // The field is as it was before it had a hint: the form's 400 wide,
    // 46 tall, at (510, 213.64).
    assert!(near(field.origin.x, 510.0), "{field:?}");
    assert!(near(field.origin.y, 213.640_63), "{field:?}");
    assert_eq!(field.size, Size::new(400.0, 46.0));
    // At the content edge, a border and `--space-4` in: 527. Down it, the
    // field's 16.8 line in the middle of its 44 of content, 13.6 below the
    // border, and the run at that line's top.
    assert!(near(hint.origin.x, 527.0), "{hint:?}");
    assert!(near(hint.origin.y, 213.640_63 + 1.0 + 13.6), "{hint:?}");
    assert!(near(hint.size.height, 16.296_875), "{hint:?}");
}

#[test]
fn the_hint_is_drawn_in_text_tertiary_and_nothing_else_is_in_the_field() {
    let Some((_, rendering)) = sign_in() else {
        panic!("the case renders");
    };
    let (Some(drawing), Some(field), Some(hint)) = (
        rendering.drawing(),
        element(&rendering, "email"),
        hint(&rendering),
    ) else {
        panic!("the field and its hint are drawn");
    };
    let Some(color) = drawing.display.items().iter().find_map(|item| match item {
        DisplayItem::Text { text, color, .. } if text == "you@company.eu" => Some(*color),
        _ => None,
    }) else {
        panic!("the hint is in the display list");
    };
    assert_eq!(
        color.to_rgba8(),
        (TERTIARY.0, TERTIARY.1, TERTIARY.2, 255),
        "`--text-tertiary`, not the field's `--text-primary` and not #757575"
    );

    // In pixels: the darkest ink is the hint's colour, inside its own
    // rectangle, and there is no ink anywhere else in the field.
    let canvas = &drawing.canvas;
    let darkest = pixels_inside(hint, 0.0)
        .filter_map(|(x, y)| canvas.at(x, y))
        .map(Rgba::to_rgba8)
        .min_by_key(|(red, green, blue, _)| u32::from(*red) + u32::from(*green) + u32::from(*blue));
    let Some((red, green, blue, _)) = darkest else {
        panic!("the hint's rectangle is on the canvas");
    };
    let close = |ink: u8, wanted: u8| ink.abs_diff(wanted) <= 24;
    assert!(
        close(red, TERTIARY.0) && close(green, TERTIARY.1) && close(blue, TERTIARY.2),
        "the darkest ink is ({red}, {green}, {blue})"
    );
    let around = Rect::new(
        hint.origin.x - 1.0,
        hint.origin.y - 1.0,
        hint.size.width + 2.0,
        hint.size.height + 2.0,
    );
    let Some(bare) = without_text(&rendering) else {
        panic!("the page draws");
    };
    // Three in from the border box, clear of the 1 px border and its 6 px
    // corners.
    for (x, y) in pixels_inside(field, 3.0) {
        if !within(around, x, y) {
            assert_eq!(
                canvas.at(x, y).map(Rgba::to_rgba8),
                bare.at(x, y).map(Rgba::to_rgba8),
                "ink at ({x}, {y}), outside the hint"
            );
        }
    }
}

#[test]
fn an_agent_reads_the_field_as_empty_with_a_hint() {
    let Some((_, rendering)) = sign_in() else {
        panic!("the case renders");
    };
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let outline = AgentTree::new(document, &drawing.boxes, &drawing.layout).to_outline();
    let lines: Vec<&str> = outline.lines().map(str::trim_start).collect();
    let Some(email) = lines
        .iter()
        .position(|line| line.starts_with("textbox \"Email\" [placeholder=\"you@company.eu\"] at"))
    else {
        panic!("the field is named by its label and carries its hint:\n{outline}");
    };
    // No text beneath it: the hint is not the field's text.
    assert!(
        lines
            .get(email + 1)
            .is_some_and(|next| !next.starts_with("text")),
        "{outline}"
    );
    assert_eq!(
        outline.matches("you@company.eu").count(),
        1,
        "the hint is read once, as the property:\n{outline}"
    );
}

#[test]
fn a_value_put_into_the_field_hides_the_hint() {
    let Some((case, rendering)) = sign_in() else {
        panic!("the case renders");
    };
    let mut renderer = loaded(&case);

    // The renderer's snapshot reads as the tree does, hint and all.
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        panic!("the case has a document and a drawing");
    };
    let tree = AgentTree::new(document, &drawing.boxes, &drawing.layout).to_outline();
    assert_eq!(read(&mut renderer).to_outline(), tree);

    let answer = renderer.handle(ToRenderer::Act {
        target: Target::Named("Email".to_owned()),
        verb: Verb::PutText("someone@alo.build".to_owned()),
    });
    assert!(
        matches!(answer, FromRenderer::Acted { .. }),
        "the verb ran: {answer:?}"
    );
    let after = read(&mut renderer).to_outline();
    let lines: Vec<&str> = after.lines().map(str::trim_start).collect();
    let Some(email) = lines
        .iter()
        .position(|line| line.starts_with("textbox \"Email\" at"))
    else {
        panic!("the field has no hint once it holds a value:\n{after}");
    };
    assert!(
        lines
            .get(email + 1)
            .is_some_and(|next| next.starts_with("text \"someone@alo.build\"")),
        "{after}"
    );
    assert!(!after.contains("you@company.eu"), "{after}");

    renderer.handle(ToRenderer::Paint);
    let Some(drawn) = renderer.rendered() else {
        panic!("the page is drawn again");
    };
    let display = drawn.display.to_outline();
    assert!(
        display.contains("\"someone@alo.build\" rgb(16 42 67)"),
        "{display}"
    );
    assert!(!display.contains("you@company.eu"), "{display}");
    // The value stands where the hint did.
    let Some(value) = drawn
        .boxes
        .ids()
        .find(|id| {
            drawn.boxes.get(*id).and_then(alo_box::BoxNode::text) == Some("someone@alo.build")
        })
        .and_then(|id| drawn.layout.get(id))
    else {
        panic!("the value is laid out");
    };
    let Some(hint) = hint(&rendering) else {
        panic!("the hint was laid out");
    };
    assert_eq!(value.border_box.origin, hint.origin);
}
