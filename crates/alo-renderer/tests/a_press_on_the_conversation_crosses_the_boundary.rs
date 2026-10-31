/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 710: a person's press on the chats panel or the first screen
//! crosses the boundary, and its claim comes back (ADR 0074, ADR 0024 § 5).
//!
//! The press goes to the renderer that drew the part, which is confined and
//! reads it as it reads anything; the claim comes back from it, and is read
//! as bytes a stranger chose — *malformed, truncated and adversarial bytes
//! refused, never a panic*. A place nothing was shown at is read, because
//! it is a number, and refused by the decision rather than by the reader.

use alo_layout::geometry::Size;
use alo_paint::Point;
use alo_renderer::conversation_claim::{Claim, Part};
use alo_renderer::conversation_decide::{Decided, Listed, Refusal, Shown, decide};
use alo_renderer::face::Face;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::message::{Failure, FromRenderer, ToRenderer};
use alo_renderer::panel::{self, Chat, Panel};
use alo_renderer::start::{self, KeptSite, Start};
use alo_renderer::wire::{
    read_from_renderer, read_to_renderer, write_from_renderer, write_to_renderer,
};
use alo_text::{Slant, Weight};

/// The tag a claim on the conversation crosses under.
const CLAIMED: u8 = 11;

/// The tag a press on the conversation crosses under.
const PRESSED: u8 = 16;

fn claimed(claim: Claim) -> FromRenderer {
    FromRenderer::ConversationClaimed(Some(claim))
}

#[test]
fn a_press_and_every_claim_survive_the_crossing() {
    for at in [Point::new(0.0, 0.0), Point::new(120.5, 153.25)] {
        let original = ToRenderer::ConversationPress(at);
        let back = read_to_renderer(&write_to_renderer(&original));
        assert_eq!(back, Ok(original));
    }
    for answer in [
        claimed(Claim::NewChat),
        claimed(Claim::Chat { at: 0 }),
        claimed(Claim::Chat { at: usize::MAX }),
        claimed(Claim::Settings),
        claimed(Claim::Site { at: 5 }),
        claimed(Claim::AddASite),
        claimed(Claim::Waiting { at: 2 }),
        claimed(Claim::TheBox),
        claimed(Claim::Go),
        claimed(Claim::Back),
        claimed(Claim::Forward),
        claimed(Claim::Expand),
        claimed(Claim::CloseTheCard),
        FromRenderer::ConversationClaimed(None),
        FromRenderer::Failed(Failure::NotTheConversation),
    ] {
        let back = read_from_renderer(&write_from_renderer(&answer));
        assert_eq!(back, Ok(answer));
    }
}

#[test]
fn a_press_at_a_point_that_is_not_a_number_is_refused() {
    for x in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut bytes = vec![PRESSED];
        bytes.extend_from_slice(&x.to_be_bytes());
        bytes.extend_from_slice(&10f32.to_be_bytes());
        assert!(read_to_renderer(&bytes).is_err(), "{x}");
    }
    let bytes = write_to_renderer(&ToRenderer::ConversationPress(Point::new(1.0, 2.0)));
    for cut in 0..bytes.len() {
        let prefix = bytes.get(..cut).unwrap_or_default();
        assert!(read_to_renderer(prefix).is_err(), "cut at {cut} was read");
    }
}

#[test]
fn a_claim_written_by_hand_is_read_and_a_strange_one_is_refused() {
    let mut on_a_chat = vec![CLAIMED, 1, 1];
    on_a_chat.extend_from_slice(&2u64.to_be_bytes());
    assert_eq!(
        read_from_renderer(&on_a_chat),
        Ok(claimed(Claim::Chat { at: 2 }))
    );
    assert_eq!(read_from_renderer(&[CLAIMED, 1, 7]), Ok(claimed(Claim::Go)));
    assert_eq!(
        read_from_renderer(&[CLAIMED, 0]),
        Ok(FromRenderer::ConversationClaimed(None))
    );
    assert!(
        read_from_renderer(&[CLAIMED, 2]).is_err(),
        "a presence of 2"
    );
    assert!(
        read_from_renderer(&[CLAIMED, 1, 12]).is_err(),
        "a claim tagged 12"
    );
    assert_eq!(
        read_from_renderer(&[CLAIMED, 1, 11]),
        Ok(claimed(Claim::CloseTheCard))
    );
    assert!(
        read_from_renderer(&[CLAIMED, 1, 255]).is_err(),
        "tagged 255"
    );
    assert!(
        read_from_renderer(&[CLAIMED]).is_err(),
        "a claim with nothing"
    );
    assert!(
        read_from_renderer(&[CLAIMED, 1, 1]).is_err(),
        "a chat with no place"
    );
    for cut in 0..on_a_chat.len() {
        let prefix = on_a_chat.get(..cut).unwrap_or_default();
        assert!(read_from_renderer(prefix).is_err(), "cut at {cut} was read");
    }
    for whole in [on_a_chat, vec![CLAIMED, 1, 6], vec![CLAIMED, 0]] {
        let mut longer = whole.clone();
        longer.push(0);
        assert!(read_from_renderer(&longer).is_err(), "a byte left over");
    }
}

#[test]
fn the_largest_place_a_renderer_can_say_is_read_and_then_refused() {
    let mut bytes = vec![CLAIMED, 1, 1];
    bytes.extend_from_slice(&u64::MAX.to_be_bytes());
    let read = read_from_renderer(&bytes).unwrap_or_else(|why| panic!("{why}"));
    let nothing: Shown<u32, u32> = Shown::default();
    let decided = decide(read, Part::Panel, &nothing, &nothing);
    let Decided::Refused(Refusal::NotShown {
        listed: Listed::Chats,
        shown: 0,
        ..
    }) = decided
    else {
        panic!("a place nothing was shown at was not refused: {decided:?}");
    };
}

/// The renderer binary, confined as every renderer is, with the corpus's
/// fonts for everything the parts' stylesheets ask for.
fn renderers() -> Renderers {
    let faces = [
        ("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
        ("DejaVu Sans", Weight::BOLD, dejavu::sans::bold()),
        ("DejaVu Serif", Weight::NORMAL, dejavu::serif::regular()),
    ]
    .into_iter()
    .filter_map(|(family, weight, bytes)| Face::new(family, weight, Slant::Normal, bytes.to_vec()))
    .collect();
    let generics = Generics::stating(vec![
        ("system-ui".to_owned(), "DejaVu Sans".to_owned()),
        ("sans-serif".to_owned(), "DejaVu Sans".to_owned()),
        ("serif".to_owned(), "DejaVu Serif".to_owned()),
    ]);
    Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[])
        .with_fonts([faces, alo_renderer::typefaces::faces()].concat())
        .with_generics(generics)
}

#[test]
fn confined_renderers_claim_what_was_pressed_on_each_part() {
    let mut renderers = renderers();
    let panel_site = panel::site();
    let start_site = start::site();
    let press = |renderers: &mut Renderers, site, x: f32, y: f32| {
        renderers.ask(site, &ToRenderer::ConversationPress(Point::new(x, y)))
    };
    // Before it holds a part, it says so.
    assert_eq!(
        press(&mut renderers, &panel_site, 120.0, 150.0),
        Ok(FromRenderer::Failed(Failure::NotTheConversation))
    );
    let panel = Panel {
        chats: vec![
            Chat::new("<b>Charging</b>", "evhomeguide.org").current(),
            Chat::new("Weekend in Lisbon", ""),
        ],
        viewport: Size {
            width: 240.0,
            height: 560.0,
        },
        scale: 1.0,
        ..Panel::default()
    };
    let loaded = renderers.ask(&panel_site, &ToRenderer::Panel(Box::new(panel)));
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    let start = Start {
        greeting: "Hello.".to_owned(),
        sites: vec![KeptSite::new("Mail", "https://mail.example/")],
        viewport: Size {
            width: 1040.0,
            height: 820.0,
        },
        ..Start::default()
    };
    let loaded = renderers.ask(&start_site, &ToRenderer::Start(Box::new(start)));
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    // New chat at (10, 56), Search chats at (10, 102) and the button that
    // hides the panel at (194, 11) (queue item 711), rows 38 tall from 178,
    // 2 apart.
    for (x, y, claim) in [
        (60.0, 75.0, Claim::NewChat),
        (120.0, 121.0, Claim::SearchChats),
        (212.0, 29.0, Claim::HideThePanel),
        (120.0, 194.0, Claim::Chat { at: 0 }),
        (120.0, 234.0, Claim::Chat { at: 1 }),
        (120.0, 529.0, Claim::Settings),
    ] {
        assert_eq!(
            press(&mut renderers, &panel_site, x, y),
            Ok(claimed(claim)),
            "({x}, {y})"
        );
    }
    // The box at (160, 254) with Go at (825, 264); the one site's tile at
    // (160, 386) and Add a site beside it.
    for (x, y, claim) in [
        (400.0, 285.0, Claim::TheBox),
        (846.0, 285.0, Claim::Go),
        (215.0, 428.0, Claim::Site { at: 0 }),
        (337.0, 428.0, Claim::AddASite),
    ] {
        assert_eq!(
            press(&mut renderers, &start_site, x, y),
            Ok(claimed(claim)),
            "({x}, {y})"
        );
    }
    for (site, x, y) in [(&panel_site, 120.0, 216.5), (&start_site, 520.0, 700.0)] {
        assert_eq!(
            press(&mut renderers, site, x, y),
            Ok(FromRenderer::ConversationClaimed(None)),
            "({x}, {y})"
        );
    }
    renderers.stop_everything();
}
