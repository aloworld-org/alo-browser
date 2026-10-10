/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' pricing section, in numbers: the page that opened queue item
//! 377.
//!
//! `cases/alo-sites-pricing` pins the whole page as files. This says out loud
//! what the item is closed by. The page is the `pricing-featured` variant:
//! one tier, `highlighted`, which the sheet lifts by `translateY(-.75rem)` and
//! puts a shadow under — `0 1.5rem 3rem color-mix(in srgb, var(--text) 12%,
//! transparent)`, the theme's text colour at twelve per cent over nothing.
//! Until item 377 that `color-mix()` was refused, which refused the whole
//! shadow, and the card was drawn lifted and flat.
//!
//! It is loaded as the call-to-action section is (`alo_sites_cta.rs`), from
//! the same frame, sheet and analytics script, so how its sheet is asked for
//! and answered is said once there and only checked here.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::DisplayItem;
use alo_value::Rgba;

/// The case, read and rendered.
fn pricing() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-pricing"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some((case, rendering))
}

/// The box of the first element whose `class` attribute is `class`, its
/// border box, and its computed `property` if the cascade set one.
fn found(
    rendering: &Rendering,
    class: &str,
    property: &str,
) -> Option<(alo_box::BoxId, Rect, Option<String>)> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let node = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.attr("class") == Some(class))
    })?;
    let boxes = &drawing.boxes;
    let id = boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|box_node| &box_node.kind),
            Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
        )
    })?;
    let value = drawing
        .styles
        .get(node)
        .and_then(|style| style.get(property))
        .map(ToOwned::to_owned);
    Some((id, drawing.layout.get(id)?.border_box, value))
}

/// The colour drawn at `(x, y)`, as RGBA bytes.
fn drawn_at(rendering: &Rendering, x: u32, y: u32) -> Option<[u8; 4]> {
    let rgba = rendering.drawing()?.canvas.at(x, y)?.to_rgba8();
    Some([rgba.0, rgba.1, rgba.2, rgba.3])
}

/// What the display list draws for `id` before its first fill: the transform
/// it is drawn through, and the shadows it casts, as `(colour, blur, bounds)`.
type Cast = (Option<(f32, f32)>, Vec<(Rgba, f32, (f32, f32, f32, f32))>);

fn cast_by(rendering: &Rendering, id: alo_box::BoxId) -> Option<Cast> {
    let drawing = rendering.drawing()?;
    let mut moved = None;
    let mut shadows = Vec::new();
    for item in drawing.display.items() {
        match item {
            DisplayItem::PushTransform { box_id, matrix } if *box_id == id => {
                moved = Some((matrix.e, matrix.f));
            }
            DisplayItem::Shadow {
                box_id,
                path,
                blur,
                color,
            } if *box_id == id => shadows.push((*color, *blur, path.bounds()?)),
            DisplayItem::Fill { box_id, .. } if *box_id == id => break,
            _ => {}
        }
    }
    Some((moved, shadows))
}

#[test]
fn its_sheet_is_asked_for_and_answered_from_what_was_frozen() {
    let Some((case, Rendering::Loaded(_, answered))) = pricing() else {
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
fn the_highlighted_tier_fills_the_grid_and_is_lifted() {
    let Some((_, rendering)) = pricing() else {
        panic!("the case renders");
    };
    // `repeat(auto-fit, minmax(16rem, 1fr))` in 760 px makes two tracks of
    // at least 256 with a 24 px gap; the one tier leaves the second empty,
    // auto-fit collapses it, and the tier is the whole width.
    let Some((id, tier, _)) = found(&rendering, "tier highlighted", "transform") else {
        panic!("the tier is laid out");
    };
    assert_eq!(
        (tier.origin.x, tier.origin.y, tier.size.width),
        (20.0, 139.8, 760.0)
    );
    assert!((tier.size.height - 350.8).abs() < 1e-3, "{tier:?}");
    // Its layout box stays where flow put it; the transform moves what is
    // drawn, by `-.75rem`.
    let Some((moved, _)) = cast_by(&rendering, id) else {
        panic!("the tier is drawn");
    };
    assert_eq!(moved, Some((0.0, -12.0)));
}

#[test]
fn the_highlighted_tier_casts_its_mixed_shadow() {
    let Some((_, rendering)) = pricing() else {
        panic!("the case renders");
    };
    let Some((id, tier, shadow)) = found(&rendering, "tier highlighted", "box-shadow") else {
        panic!("the tier is laid out");
    };
    // As the sheet wrote it, `var()` substituted: the mix is the value.
    assert_eq!(
        shadow.as_deref(),
        Some("0 1.5rem 3rem color-mix(in srgb, #17212b 12%, transparent)")
    );
    let Some((_, shadows)) = cast_by(&rendering, id) else {
        panic!("the tier is drawn");
    };
    // One shadow: `--text` (#17212b) at twelve per cent, blurred over 3rem,
    // the box's own shape 1.5rem below it.
    let [(color, blur, (left, top, right, bottom))] = shadows.as_slice() else {
        panic!("one shadow, not {shadows:?}");
    };
    assert_eq!(color.to_rgba8(), (0x17, 0x21, 0x2b, 31));
    assert!((color.alpha - 0.12).abs() < 1e-6, "{color:?}");
    assert!((blur - 48.0).abs() < f32::EPSILON, "{blur}");
    assert!((left - tier.origin.x).abs() < 1e-3, "{left}");
    assert!(
        (right - (tier.origin.x + tier.size.width)).abs() < 1e-3,
        "{right}"
    );
    assert!((top - (tier.origin.y + 24.0)).abs() < 1e-3, "{top}");
    assert!(
        (bottom - (tier.origin.y + 24.0 + tier.size.height)).abs() < 1e-3,
        "{bottom}"
    );
}

#[test]
fn the_shadow_darkens_the_page_below_the_card_and_fades_out() {
    let Some((_, rendering)) = pricing() else {
        panic!("the case renders");
    };
    // The card, lifted, ends at 139.8 - 12 + 350.8 = 478.6. Below it the
    // shadow is darkest nearest the card and gone 48 px past its own edge
    // (139.8 - 12 + 24 + 350.8 = 502.6). The page is white, so every grey
    // here is the shadow. Before item 377 all four were white.
    assert_eq!(drawn_at(&rendering, 400, 485), Some([234, 235, 236, 255]));
    assert_eq!(drawn_at(&rendering, 400, 510), Some([245, 245, 246, 255]));
    assert_eq!(drawn_at(&rendering, 400, 530), Some([252, 252, 252, 255]));
    assert_eq!(drawn_at(&rendering, 400, 590), Some([255, 255, 255, 255]));
    // And beside it, left of the card's edge at 20.
    assert_eq!(drawn_at(&rendering, 10, 300), Some([246, 246, 246, 255]));
}
