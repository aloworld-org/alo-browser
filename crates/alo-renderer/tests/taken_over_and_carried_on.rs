/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 825 (`docs/design/shell.md`, *Taking over and carrying on*,
//! approved by the owner 2026-11-06): a person's press on the card's page
//! while alo's agent works takes the page over — the run is held, not
//! ended, the window is the conversation again, and the column under the
//! page says the page is the person's, with *Let alo continue*.
//!
//! Each a test below: a layout assertion of the column held, in numbers;
//! *Let alo continue* claimed as itself; what an agent reads; and a
//! reference render, `references/agent-paused.png`, holding the committed
//! frame Chrome streamed for `a_page_as_a_card.rs`
//! (`references/card-frame.png`, the input, never rewritten).
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test taken_over_and_carried_on`
//! rewrites the reference. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::agent_document::{AT_WORK, CONTINUE, STOP};
use alo_renderer::agent_plan::Plan;
use alo_renderer::agent_work::{AtWork, Now, PAUSED, Step};
use alo_renderer::card::Card;
use alo_renderer::conversation_claim::Claim;
use alo_renderer::frame::frame_to_png;
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

/// The room the first screen has in the design's window: 1280 wide less
/// the panel's 240, and 820 tall.
const WIDTH: f32 = 1040.0;
const HEIGHT: f32 = 820.0;

/// How far the conversation is scrolled to show the column: what the
/// browser process scrolls it to as the agent says each step, its newest
/// end at the room's foot.
const DOWN: f32 = 200.0;

fn frame() -> Result<Vec<u8>, String> {
    let path = reference("card-frame.png");
    std::fs::read(&path).map_err(|why| format!("{} is not there: {why}", path.display()))
}

fn step(line: &str, why: &str) -> Step {
    Step {
        line: line.to_owned(),
        why: why.to_owned(),
    }
}

/// The first screen holding a card of Chrome's frame, loaded, with the
/// agent at work on it as `work`, scrolled [`DOWN`].
fn with(work: AtWork) -> Result<Start, String> {
    let url = alo_url::parse("http://127.0.0.1:8765/card.html")
        .map_err(|_| "the card's address".to_owned())?;
    let mut card = Card::new("127.0.0.1:8765/card.html", url)
        .holding(frame()?)
        .loaded();
    card.agent = Some(work);
    Ok(Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        turns: vec![Turn::Page(card)],
        down: DOWN,
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
        .map(|geometry| units(geometry.border_box))
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

/// It held with the page the person's: two steps taken, on the second of
/// its plan.
fn paused() -> AtWork {
    AtWork {
        plan: Some(Plan {
            steps: vec!["Fill in your name".to_owned(), "Send the form".to_owned()],
            on: 1,
        }),
        asked: "Send my name, Ada, through the form".to_owned(),
        steps: vec![
            step("Typing \u{201c}Ada\u{201d} into Name", ""),
            step("Pressing Send", ""),
        ],
        now: Now::Paused,
        choosing: None,
    }
}

#[test]
fn held_the_column_says_the_page_is_yours_with_let_alo_continue() -> Result<(), String> {
    let renderer = laid_out(with(paused())?);
    let at = |class: &str| boxes_of(&renderer, class);
    // Not at work: no outline, no working line, no Stop — the page is the
    // person's — and the card as it stands in the conversation, 878 × 520.
    assert!(at("card acting").is_empty());
    assert!(at("ring").is_empty());
    assert!(at("working").is_empty() && at("abtn stop").is_empty());
    assert_eq!(at("page"), vec![px(81.0, -55.0, 878.0, 520.0)]);
    // The column: what was asked 15 under its rule, the plan, the steps,
    // then 10 under them what it says, a line of 20, and 10 under it *Let
    // alo continue* in the ink, 28 tall at the column's left.
    assert_eq!(at("asked"), vec![px(97.0, 480.0, 846.0, 20.0)]);
    assert_eq!(at("plan"), vec![px(97.0, 510.0, 846.0, 42.0)]);
    assert_eq!(at("steps"), vec![px(97.0, 562.0, 846.0, 46.0)]);
    assert_eq!(at("paused"), vec![px(97.0, 618.0, 846.0, 58.0)]);
    // Its words as wide as they are, at the column's left.
    assert_eq!(at("pausedsaid"), vec![px(97.0, 618.0, 443.546_88, 20.0)]);
    let carry_on = at("abtn continue");
    assert_eq!(carry_on.len(), 1);
    assert!(carry_on[0].starts_with("(97, 648) "), "{carry_on:?}");
    // No step is under way while it is held.
    assert!(at("step now").is_empty());
    Ok(())
}

#[test]
fn let_alo_continue_is_claimed_as_itself() -> Result<(), String> {
    assert_eq!(
        pressed(with(paused())?, 140.0, 662.0),
        FromRenderer::ConversationClaimed(Some(Claim::Continue))
    );
    // What it says claims nothing.
    assert_eq!(
        pressed(with(paused())?, 400.0, 628.0),
        FromRenderer::ConversationClaimed(None)
    );
    Ok(())
}

#[test]
fn an_agent_reads_that_the_page_is_the_persons_and_the_button() -> Result<(), String> {
    let (_, tree) = drawn(with(paused())?);
    let outline = tree.to_outline();
    let nodes = tree.nodes();
    let region = nodes
        .iter()
        .find(|node| node.name.as_deref() == Some(AT_WORK))
        .ok_or_else(|| format!("no region {AT_WORK}: {outline}"))?;
    let inside = Snapshot {
        root: Some((*region).clone()),
    }
    .to_outline();
    for said in [PAUSED, CONTINUE, "Send the form"] {
        assert!(inside.contains(said), "{said} not read: {inside}");
    }
    assert!(!inside.contains(STOP), "{inside}");
    Ok(())
}

#[test]
fn held_it_is_drawn_as_the_reference() -> Result<(), String> {
    let (frame, _) = drawn(with(paused())?);
    holds("agent-paused.png", frame.as_ref())
}
