/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 826 (`docs/design/shell.md`, *Paying*, approved by the owner
//! 2026-11-06; ADR 0099): before any press that pays, the agent's question
//! shows the amount, the merchant and the page's own summary lines, with
//! *Pay* and *Not now* — and says plainly what it could not find.
//!
//! Each a test below: layout assertions in numbers in the split view at
//! the design's window beside the rail (a first screen of 1216 × 820), the
//! details found and not found; *Pay* claimed as *Allow* is; what an agent
//! reads; and a reference render, `references/agent-paying.png`.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_payments_question`
//! rewrites the reference. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::agent_document::{ASKS, NOT_NOW, SUMMARY};
use alo_renderer::agent_paying::{NO_AMOUNT, NO_SUMMARY, PAY};
use alo_renderer::agent_work::{AtWork, Now, Paying, Step};
use alo_renderer::card::Card;
use alo_renderer::conversation_claim::Claim;
use alo_renderer::frame::frame_to_png;
use alo_renderer::split_place;
use alo_renderer::start::Start;
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, Snapshot, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::{Path, PathBuf};

/// The corpus's faces for the generics, and the design's typefaces as the
/// browser process hands them to the interface's renderers (ADR 0084).
fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (family, weight, bytes) in [
        ("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
        ("DejaVu Sans", Weight::BOLD, dejavu::sans::bold()),
        ("DejaVu Serif", Weight::NORMAL, dejavu::serif::regular()),
    ] {
        if let Some(font) = Font::load(family, weight, Slant::Normal, bytes.to_vec()) {
            database.add(font);
        }
    }
    for (generic, family) in [
        ("system-ui", "DejaVu Sans"),
        ("sans-serif", "DejaVu Sans"),
        ("serif", "DejaVu Serif"),
    ] {
        database.map_generic(generic, family);
    }
    for font in alo_renderer::typefaces::fonts() {
        database.add(font);
    }
    database
}

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

/// A frame Chrome would stream of a page laid out at `page` CSS pixels at
/// one: white, a band of the page's own blue 60 tall across its top, and a
/// grey line 12 tall every 28 under it, 40 in from either side — pixels
/// known in advance, so the reference shows where the page box is.
fn page_frame(page: (u32, u32)) -> Result<Vec<u8>, String> {
    let (width, height) = page;
    let mut pixels = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let lined = y >= 84 && (y - 84) % 28 < 12 && x >= 40 && x + 40 < width;
            let colour = if y < 60 {
                [0x36, 0x66, 0xcc, 0xff]
            } else if lined {
                [0xd8, 0xd8, 0xd8, 0xff]
            } else {
                [0xff, 0xff, 0xff, 0xff]
            };
            pixels.extend_from_slice(&colour);
        }
    }
    frame_to_png(&Frame {
        width,
        height,
        pixels,
    })
    .map_err(|why| format!("the page's frame could not be encoded: {why}"))
}

fn step(line: &str, why: &str) -> Step {
    Step {
        line: line.to_owned(),
        why: why.to_owned(),
    }
}

/// The first screen `viewport` big, a page shown earlier and then the live
/// card, its frame at the split's page box, the agent at work on it as
/// `work`.
fn with(work: AtWork, viewport: Size) -> Result<Start, String> {
    let url = alo_url::parse("http://127.0.0.1:8765/card.html")
        .map_err(|_| "the card's address".to_owned())?;
    let earlier = alo_url::parse("https://en.wikipedia.org/")
        .map_err(|_| "the earlier address".to_owned())?;
    let mut card = Card::new("127.0.0.1:8765/card.html", url)
        .holding(page_frame(split_place::page_size(viewport))?)
        .loaded();
    card.agent = Some(work);
    Ok(Start {
        greeting: "Hello.".to_owned(),
        viewport,
        turns: vec![
            Turn::Visited {
                said: "wikipedia".to_owned(),
                url: earlier,
                ran: Vec::new(),
            },
            Turn::Page(card),
        ],
        ..Start::default()
    })
}

/// A rectangle as the outline writes it.
fn units(rect: Rect) -> String {
    format!(
        "({}, {}) {}×{}",
        rect.left(),
        rect.top(),
        rect.size.width,
        rect.size.height
    )
}

fn px(left: f32, top: f32, width: f32, height: f32) -> String {
    units(Rect::new(left, top, width, height))
}

/// The border box of every element of `class`, in document order.
fn boxes_of(renderer: &Renderer, class: &str) -> Vec<String> {
    rects_of(renderer, class).into_iter().map(units).collect()
}

/// The border box of every element of `class`, in document order.
fn rects_of(renderer: &Renderer, class: &str) -> Vec<Rect> {
    let (Some(drawing), Some(document)) = (renderer.rendered(), renderer.document()) else {
        return Vec::new();
    };
    drawing
        .boxes
        .ids()
        .filter(|&id| {
            drawing
                .boxes
                .get(id)
                .filter(|made| !matches!(made.kind, alo_box::BoxKind::Text { .. }))
                .and_then(|made| made.kind.node())
                .is_some_and(|node| document.attribute_by_name(node, "class") == Some(class))
        })
        .filter_map(|id| drawing.layout.get(id))
        .map(|geometry| geometry.border_box)
        .collect()
}

fn laid_out(start: Start) -> Renderer {
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Start(Box::new(start)));
    renderer
}

/// What a renderer holding `start` claims a press at `x`, `y` is on.
fn pressed(start: Start, x: f32, y: f32) -> FromRenderer {
    let mut renderer = laid_out(start);
    let _ = renderer.handle(ToRenderer::Paint);
    renderer.handle(ToRenderer::ConversationPress(Point::new(x, y)))
}

/// What a renderer holding `start` paints, and the tree an agent reads.
fn drawn(start: Start) -> (Option<Frame>, Snapshot) {
    let mut renderer = laid_out(start);
    let frame = match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Some(frame),
        _ => None,
    };
    let tree = match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(tree) => *tree,
        _ => Snapshot::default(),
    };
    (frame, tree)
}

/// Compare `frame` with the committed reference, or write it when updating.
fn holds(name: &str, frame: Option<&Frame>) -> Result<(), String> {
    let frame = frame.ok_or_else(|| format!("{name}: nothing was painted"))?;
    let path = reference(name);
    let drawn = frame_to_png(frame).map_err(|why| format!("{name} could not be encoded: {why}"))?;
    if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
        return std::fs::write(&path, &drawn)
            .map_err(|why| format!("{} could not be written: {why}", path.display()));
    }
    let committed = std::fs::read(&path).map_err(|why| {
        format!(
            "{} is not there ({why}); ALO_UPDATE_REFERENCES=1 writes it",
            path.display()
        )
    })?;
    let expected = from_png(&committed).map_err(|why| format!("{name} is not a picture: {why}"))?;
    let expected = Frame::from_canvas(&expected);
    if (frame.width, frame.height) != (expected.width, expected.height) {
        return Err(format!(
            "{name}: the screen is {}×{}, and the reference {}×{}",
            frame.width, frame.height, expected.width, expected.height
        ));
    }
    let moved = frame
        .pixels
        .chunks(4)
        .zip(expected.pixels.chunks(4))
        .position(|(drawn, committed)| drawn != committed);
    match moved {
        None => Ok(()),
        Some(at) => {
            let width = usize::try_from(frame.width).unwrap_or(1).max(1);
            Err(format!(
                "{name}: pixels moved, the first at ({}, {}). If this is intended, \
                 ALO_UPDATE_REFERENCES=1 rewrites the reference",
                at % width,
                at / width
            ))
        }
    }
}

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    let rgba = frame.pixels.get(at..at + 4)?;
    rgba.try_into().ok()
}

/// The design's window, 1280 × 820 at one, beside the rail's 64.
const NORMAL: Size = Size {
    width: 1216.0,
    height: 820.0,
};

/// It asking before pressing *Pay now* on a shop's checkout, what it pays
/// `paying`.
fn paying(paying: Paying) -> AtWork {
    AtWork {
        plan: None,
        asked: "Buy two flat whites and the beans in my basket".to_owned(),
        steps: vec![step("Pressing Pay now", "")],
        now: Now::Asking {
            what: "Press \u{201c}Pay now\u{201d} on 127.0.0.1:8736? It may pay for something."
                .to_owned(),
            paying: Some(Box::new(paying)),
        },
        choosing: None,
    }
}

/// What the shop's checkout says it pays.
fn found() -> Paying {
    Paying {
        amount: Some("UGX 45,000".to_owned()),
        merchant: "Kampala Coffee \u{2014} Checkout (127.0.0.1:8736)".to_owned(),
        lines: vec![
            "Flat white \u{d7} 2 UGX 16,000".to_owned(),
            "Beans, 250 g UGX 29,000".to_owned(),
            "Total UGX 45,000".to_owned(),
        ],
    }
}

#[test]
fn the_amount_merchant_and_summary_stand_under_the_question_with_pay() -> Result<(), String> {
    let renderer = laid_out(with(paying(found()), NORMAL)?);
    let at = |class: &str| boxes_of(&renderer, class);
    // The question's box 10 under the step, 12 inside its border of 1 and
    // 14 across: the question on two lines of 20 in the 321 inside; 10
    // under it the amount (a line of 20) and who is paid (two lines), 4
    // apart; 4 under them the summary, 2 and its rule of 1 and 6 above
    // three lines of 18; 10 under it *Pay* and *Not now*: 1 + 12 + 40 + 10
    // + 131 + 10 + 28 + 12 + 1 = 245.
    assert_eq!(at("asks"), vec![px(849.0, 138.0, 351.0, 245.0)]);
    assert_eq!(at("question"), vec![px(864.0, 151.0, 321.0, 40.0)]);
    // 20 + 4 + 40 + 4 + 2 + 61 = 131.
    assert_eq!(at("paying"), vec![px(864.0, 201.0, 321.0, 131.0)]);
    assert_eq!(at("pamount"), vec![px(864.0, 201.0, 321.0, 20.0)]);
    assert_eq!(at("pmerchant"), vec![px(864.0, 225.0, 321.0, 40.0)]);
    assert_eq!(at("psummary"), vec![px(864.0, 271.0, 321.0, 61.0)]);
    let lines = at("pline");
    assert_eq!(lines.len(), 3);
    assert_eq!(lines.first(), Some(&px(864.0, 278.0, 321.0, 18.0)));
    // Each value 64 in, past its label's 56 and 8.
    let values = rects_of(&renderer, "pvalue");
    assert_eq!(values.len(), 2);
    assert!(
        values
            .iter()
            .all(|value| (value.left() - 928.0).abs() < 0.01),
        "{values:?}"
    );
    let pay = rects_of(&renderer, "abtn allow");
    assert_eq!(pay.len(), 1);
    assert!(
        pay.iter().all(|pay| (pay.top() - 342.0).abs() < 0.01),
        "{pay:?}"
    );
    assert_eq!(at("abtn notnow").len(), 1);
    // The page box unmoved by it.
    assert_eq!(at("page"), vec![units(split_place::page_box(NORMAL))]);
    Ok(())
}

#[test]
fn what_could_not_be_found_is_said_plainly_and_it_still_asks() -> Result<(), String> {
    let none = Paying {
        amount: None,
        merchant: "127.0.0.1:8736".to_owned(),
        lines: Vec::new(),
    };
    let (_, tree) = drawn(with(paying(none), NORMAL)?);
    let outline = tree.to_outline();
    for said in [NO_AMOUNT, NO_SUMMARY, "127.0.0.1:8736", PAY, NOT_NOW] {
        assert!(outline.contains(said), "{said} not read: {outline}");
    }
    let renderer = laid_out(with(
        paying(Paying {
            amount: None,
            merchant: "127.0.0.1:8736".to_owned(),
            lines: Vec::new(),
        }),
        NORMAL,
    )?);
    // Each said in the 321 inside the box, wrapping onto two lines of 18
    // rather than running past it (found by hand, `handrun36/14`) — the
    // summary's under its rule, 1 and 6 above it: 43.
    for (class, tall) in [("pamount none", 36.0), ("psummary none", 43.0)] {
        let said = rects_of(&renderer, class);
        let [said] = said.as_slice() else {
            return Err(format!("{class}: {said:?}"));
        };
        assert!(
            (said.left() - 864.0).abs() < 0.01 && (said.size.width - 321.0).abs() < 0.01,
            "{class}: {said:?}"
        );
        assert!((said.size.height - tall).abs() < 0.01, "{class}: {said:?}");
    }
    assert_eq!(boxes_of(&renderer, "abtn allow").len(), 1, "still asked");
    Ok(())
}

#[test]
fn an_agent_reads_the_question_its_details_and_pay() -> Result<(), String> {
    let (_, tree) = drawn(with(paying(found()), NORMAL)?);
    let outline = tree.to_outline();
    let nodes = tree.nodes();
    let asks = nodes
        .iter()
        .find(|node| node.name.as_deref() == Some(ASKS))
        .ok_or_else(|| format!("no group {ASKS}: {outline}"))?;
    let inside = Snapshot {
        root: Some((*asks).clone()),
    }
    .to_outline();
    for said in [
        "UGX 45,000",
        "Kampala Coffee",
        SUMMARY,
        "Beans, 250 g",
        PAY,
        NOT_NOW,
    ] {
        assert!(inside.contains(said), "{said} not read: {inside}");
    }
    assert!(!inside.contains("\"Allow\""), "{inside}");
    Ok(())
}

#[test]
fn pay_is_claimed_as_allow_is() -> Result<(), String> {
    let renderer = laid_out(with(paying(found()), NORMAL)?);
    let pay = rects_of(&renderer, "abtn allow");
    let [pay] = pay.as_slice() else {
        return Err(format!("{pay:?}"));
    };
    let claimed = pressed(
        with(paying(found()), NORMAL)?,
        pay.left() + 10.0,
        pay.top() + 10.0,
    );
    assert!(
        matches!(
            &claimed,
            FromRenderer::ConversationClaimed(Some(Claim::Allow))
        ),
        "{claimed:?}"
    );
    Ok(())
}

#[test]
fn the_payments_question_is_drawn_as_the_reference() -> Result<(), String> {
    let (frame, _) = drawn(with(paying(found()), NORMAL)?);
    let frame = frame.as_ref().ok_or("nothing was painted")?;
    // *Pay* in the ink.
    assert_eq!(pixel(frame, 868, 356), Some([0x10, 0x2a, 0x43, 0xff]));
    holds("agent-paying.png", Some(frame))
}
