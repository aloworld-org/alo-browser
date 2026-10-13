/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 539: a person's press on the tab strip crosses the boundary,
//! and its claim comes back (ADR 0024 § 5).
//!
//! The press goes to the strip's renderer, which is confined and reads it
//! as it reads anything; the claim comes back from it, and is read as
//! bytes a stranger chose — *malformed, truncated and adversarial bytes
//! refused, never a panic*. A place no strip has is read, because it is a
//! number, and refused by the decision rather than by the reader.

use alo_layout::geometry::Size;
use alo_paint::Point;
use alo_renderer::face::Face;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::message::{Failure, FromRenderer, ToRenderer};
use alo_renderer::strip::{self, Strip, StripTab};
use alo_renderer::strip_claim::Claim;
use alo_renderer::strip_decide::{Decided, Refusal, decide};
use alo_renderer::wire::{
    read_from_renderer, read_to_renderer, write_from_renderer, write_to_renderer,
};
use alo_text::{Slant, Weight};

fn claimed(at: usize) -> FromRenderer {
    FromRenderer::Claimed(Some(Claim::Select { at }))
}

#[test]
fn a_press_and_every_claim_survive_the_crossing() {
    for at in [Point::new(0.0, 0.0), Point::new(212.5, 35.75)] {
        let original = ToRenderer::StripPress(at);
        let back = read_to_renderer(&write_to_renderer(&original));
        assert_eq!(back, Ok(original));
    }
    for answer in [
        claimed(0),
        claimed(3),
        claimed(usize::MAX),
        FromRenderer::Claimed(None),
        FromRenderer::Failed(Failure::NotTheStrip),
    ] {
        let back = read_from_renderer(&write_from_renderer(&answer));
        assert_eq!(back, Ok(answer));
    }
}

#[test]
fn a_press_at_a_point_that_is_not_a_number_is_refused() {
    for x in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut bytes = vec![13];
        bytes.extend_from_slice(&x.to_be_bytes());
        bytes.extend_from_slice(&10f32.to_be_bytes());
        assert!(read_to_renderer(&bytes).is_err(), "{x}");
    }
    let bytes = write_to_renderer(&ToRenderer::StripPress(Point::new(1.0, 2.0)));
    for cut in 0..bytes.len() {
        let prefix = bytes.get(..cut).unwrap_or_default();
        assert!(read_to_renderer(prefix).is_err(), "cut at {cut} was read");
    }
}

#[test]
fn a_claim_written_by_hand_is_read_and_a_strange_one_is_refused() {
    let mut on = vec![10, 1];
    on.extend_from_slice(&2u64.to_be_bytes());
    assert_eq!(read_from_renderer(&on), Ok(claimed(2)));
    assert_eq!(
        read_from_renderer(&[10, 0]),
        Ok(FromRenderer::Claimed(None))
    );
    assert!(read_from_renderer(&[10, 2]).is_err(), "a claim tagged 2");
    assert!(read_from_renderer(&[10]).is_err(), "a claim with nothing");
    for cut in 0..on.len() {
        let prefix = on.get(..cut).unwrap_or_default();
        assert!(read_from_renderer(prefix).is_err(), "cut at {cut} was read");
    }
    let mut longer = on.clone();
    longer.push(0);
    assert!(read_from_renderer(&longer).is_err(), "a byte left over");
    let mut nothing_more = vec![10, 0];
    nothing_more.push(0);
    assert!(
        read_from_renderer(&nothing_more).is_err(),
        "a byte after none"
    );
}

#[test]
fn the_largest_place_a_renderer_can_say_is_read_and_then_refused() {
    let mut bytes = vec![10, 1];
    bytes.extend_from_slice(&u64::MAX.to_be_bytes());
    let read = read_from_renderer(&bytes).unwrap_or_else(|why| panic!("{why}"));
    let decided = decide(read, &[], |_| true);
    let Decided::Refused(Refusal::NotShown { shown: 0, .. }) = decided else {
        panic!("a place no strip has was not refused: {decided:?}");
    };
}

/// The renderer binary, confined as every renderer is, with the corpus's
/// fonts.
fn renderers() -> Renderers {
    let faces = [
        (Weight::NORMAL, dejavu::sans::regular()),
        (Weight::BOLD, dejavu::sans::bold()),
    ]
    .into_iter()
    .filter_map(|(weight, bytes)| Face::new("DejaVu Sans", weight, Slant::Normal, bytes.to_vec()))
    .collect();
    let generics = Generics::stating(vec![
        ("system-ui".to_owned(), "DejaVu Sans".to_owned()),
        ("sans-serif".to_owned(), "DejaVu Sans".to_owned()),
    ]);
    Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[])
        .with_fonts(faces)
        .with_generics(generics)
}

#[test]
fn a_confined_strip_renderer_claims_the_tab_pressed() {
    let mut renderers = renderers();
    let site = strip::site();
    let press = |renderers: &mut Renderers, x: f32, y: f32| {
        renderers.ask(&site, &ToRenderer::StripPress(Point::new(x, y)))
    };
    // Before it holds a strip, it says so.
    assert_eq!(
        press(&mut renderers, 20.0, 20.0),
        Ok(FromRenderer::Failed(Failure::NotTheStrip))
    );
    let strip = Strip {
        tabs: vec![
            StripTab::new("Inbox", "https://mail.example/"),
            StripTab::new("<b>Invoices</b>", "https://books.example/").selected(),
        ],
        viewport: Size {
            width: 480.0,
            height: 36.0,
        },
    };
    let loaded = renderers.ask(&site, &ToRenderer::Strip(Box::new(strip)));
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    // Two tabs of 200 from x 8, 4 apart, from y 6.
    assert_eq!(press(&mut renderers, 100.0, 20.0), Ok(claimed(0)));
    assert_eq!(press(&mut renderers, 212.0, 35.0), Ok(claimed(1)));
    assert_eq!(press(&mut renderers, 411.5, 20.0), Ok(claimed(1)));
    for (x, y) in [(210.0, 20.0), (420.0, 20.0), (100.0, 2.0)] {
        assert_eq!(
            press(&mut renderers, x, y),
            Ok(FromRenderer::Claimed(None)),
            "({x}, {y})"
        );
    }
    renderers.stop_everything();
}
