/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo Sites' testimonials section, in numbers: the page that opened queue
//! item 382.
//!
//! `cases/alo-sites-testimonials` pins the whole page as files. This says out
//! loud what the item is closed by. The page is the `testimonials-featured`
//! variant with one quote, which the sheet sets `font-style: italic` at
//! `clamp(1.4rem, 3vw, 2.25rem)`. The corpus's fonts have no slanted face, so
//! the quote is drawn in upright `DejaVu Sans` — and until item 382 it was drawn
//! upright, the page's emphasis lost without a word, where every browser
//! leans the upright face (CSS Fonts 4 § 3.3, `font-synthesis`).
//!
//! It is loaded as the call-to-action section is (`alo_sites_cta.rs`), from
//! the same frame, sheet and analytics script, so how its sheet is asked for
//! and answered is said once there and only checked here.

use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;
use alo_paint::{Canvas, DisplayItem, DisplayList, render};
use alo_text::Slant;
use alo_value::Rgba;

/// The case, read and rendered.
fn testimonials() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-sites-testimonials"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some((case, rendering))
}

/// The border box of the first element named `tag`.
fn found(rendering: &Rendering, tag: &str) -> Option<Rect> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let node = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| &*element.name.local == tag)
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

/// The text item that says `said`: how far it is leaned, the slant of the
/// face it is drawn in, its size and where its pen starts.
fn text_saying(rendering: &Rendering, said: &str) -> Option<(f32, Slant, f32, (f32, f32))> {
    let drawing = rendering.drawing()?;
    drawing.display.items().iter().find_map(|item| match item {
        DisplayItem::Text {
            text,
            oblique,
            font,
            size,
            origin,
            ..
        } if text == said => Some((*oblique, font.slant(), *size, *origin)),
        _ => None,
    })
}

/// The page drawn again from its own display list with every lean taken
/// out, and drawn as it is: what the lean alone changes.
fn upright_and_leaned(rendering: &Rendering) -> Option<(Canvas, Canvas)> {
    let drawing = rendering.drawing()?;
    let mut upright = DisplayList::default();
    for item in drawing.display.items() {
        let mut item = item.clone();
        if let DisplayItem::Text { oblique, .. } = &mut item {
            *oblique = 0.0;
        }
        upright.push(item);
    }
    let (width, height) = (drawing.canvas.width(), drawing.canvas.height());
    let mut straight = Canvas::new(width, height, Rgba::WHITE);
    render(&upright, &mut straight);
    let mut leaned = Canvas::new(width, height, Rgba::WHITE);
    render(&drawing.display, &mut leaned);
    Some((straight, leaned))
}

/// The leftmost column on row `y` darker than the card behind it.
fn first_ink(canvas: &Canvas, y: u32) -> Option<u32> {
    (0..canvas.width()).find(|x| {
        canvas
            .at(*x, y)
            .is_some_and(|pixel| pixel.to_rgba8().0 < 150)
    })
}

#[test]
fn its_sheet_is_asked_for_and_answered_from_what_was_frozen() {
    let Some((case, Rendering::Loaded(_, answered))) = testimonials() else {
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
fn the_featured_quote_is_laid_out_as_the_sheet_says() {
    let Some((_, rendering)) = testimonials() else {
        panic!("the case renders");
    };
    // `main > section` puts 3rem above and 1.25rem beside; the heading is
    // 28 px (3.5vw at 800) at a line height of 1.2 with 0.5em below it.
    let Some(figure) = found(&rendering, "figure") else {
        panic!("the testimonial is laid out");
    };
    assert_eq!((figure.origin.x, figure.size.width), (20.0, 760.0));
    assert!((figure.origin.y - 95.6).abs() < 1e-3, "{figure:?}");
    // 1 px of border and `clamp(2rem, 6vw, 4rem)`, 48 px, of padding.
    let Some(quote) = found(&rendering, "blockquote") else {
        panic!("the quote is laid out");
    };
    assert_eq!((quote.origin.x, quote.size.width), (69.0, 662.0));
    assert!((quote.origin.y - 144.6).abs() < 1e-3, "{quote:?}");
    // `clamp(1.4rem, 3vw, 2.25rem)` is 24 px at 800, at a line height of 1.3.
    assert!((quote.size.height - 31.2).abs() < 1e-3, "{quote:?}");
    // The paragraph's 1em (24 px) below collapses through the blockquote with
    // its 1rem, so the caption is 24 below the quote, and the card closes 48
    // and 1 below that.
    let Some(caption) = found(&rendering, "figcaption") else {
        panic!("the caption is laid out");
    };
    assert!((caption.origin.y - 199.8).abs() < 1e-3, "{caption:?}");
    assert!((caption.size.height - 27.2).abs() < 1e-3, "{caption:?}");
    assert!((figure.size.height - 180.4).abs() < 1e-3, "{figure:?}");
}

#[test]
fn the_quote_is_leaned_because_no_slanted_face_was_there_to_draw_it_in() {
    let Some((_, rendering)) = testimonials() else {
        panic!("the case renders");
    };
    let Some((oblique, face, size, _)) = text_saying(
        &rendering,
        "The freshest beans we've ever pulled shots with.",
    ) else {
        panic!("the quote is drawn");
    };
    assert_eq!(face, Slant::Normal, "the corpus has no slanted face");
    assert_eq!(oblique.to_bits(), 14.0_f32.to_bits());
    assert_eq!(size.to_bits(), 24.0_f32.to_bits());
    // The caption asked for no slant and is drawn as its face is.
    let Some((upright, ..)) = text_saying(&rendering, "Mara Lindqvist") else {
        panic!("the caption is drawn");
    };
    assert_eq!(upright.to_bits(), 0.0_f32.to_bits());
}

#[test]
fn the_lean_moves_the_quotes_ink_and_nothing_else() {
    let Some((_, rendering)) = testimonials() else {
        panic!("the case renders");
    };
    let Some((straight, leaned)) = upright_and_leaned(&rendering) else {
        panic!("the page draws");
    };
    let Some((_, _, _, (_, baseline))) = text_saying(
        &rendering,
        "The freshest beans we've ever pulled shots with.",
    ) else {
        panic!("the quote is drawn");
    };
    // Every pixel the lean changes is on the quote's own line — between the
    // top of its capitals and the bottom of its descenders — and none of
    // the heading, the caption or the card's edges moved.
    let mut changed_rows = Vec::new();
    for y in 0..straight.height() {
        let differs = (0..straight.width()).any(|x| straight.at(x, y) != leaned.at(x, y));
        if differs {
            changed_rows.push(y);
        }
    }
    let (Some(first), Some(last)) = (changed_rows.first(), changed_rows.last()) else {
        panic!("the lean changed nothing");
    };
    assert!(
        f64::from(*first) > f64::from(baseline) - 24.0,
        "{first} above the quote's line"
    );
    assert!(
        f64::from(*last) < f64::from(baseline) + 8.0,
        "{last} below the quote's line"
    );
    // The `T` at the start of the quote: on the row of its crossbar, 16 px
    // above the baseline, its ink starts tan 14° × 16 ≈ 4 px further right
    // leaned than upright, and on the baseline itself where its stem stands
    // it starts where it did.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a baseline on an 800 × 600 page"
    )]
    let foot = baseline.floor() as u32 - 1;
    let crossbar = foot - 15;
    let (Some(upright_bar), Some(leaned_bar)) =
        (first_ink(&straight, crossbar), first_ink(&leaned, crossbar))
    else {
        panic!("the T's crossbar is drawn");
    };
    assert!(
        (3..=5).contains(&(leaned_bar - upright_bar)),
        "the crossbar moved from {upright_bar} to {leaned_bar}"
    );
    let (Some(upright_foot), Some(leaned_foot)) =
        (first_ink(&straight, foot), first_ink(&leaned, foot))
    else {
        panic!("the T's stem is drawn");
    };
    assert!(
        leaned_foot.abs_diff(upright_foot) <= 1,
        "the stem's foot moved from {upright_foot} to {leaned_foot}"
    );
}
