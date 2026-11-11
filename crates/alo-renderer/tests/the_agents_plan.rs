/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 824 (`docs/design/shell.md`, *The agent's plan*, approved by
//! the owner 2026-11-06): before it acts, alo's agent writes two to five
//! short steps in its column — the steps done ticked, the one it is on in
//! bold, the rest greyed.
//!
//! Each a test below: layout assertions in numbers in the split view at
//! the design's window beside the rail (a first screen of 1216 × 820) and
//! in the column under the page in a window too narrow to split (1040 of
//! first screen); every step done once the task is; the plan of a run kept
//! with its page; what an agent reads — a list named *Plan*, each step
//! named by where it stands, the one it is on `aria-current="step"`; and a
//! reference render, `references/agent-plan.png`.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test the_agents_plan`
//! rewrites the reference. A rewritten reference is a diff somebody reads.

use alo_box::Current;
use alo_layout::{Rect, Size};
use alo_paint::from_png;
use alo_renderer::agent_plan::{PLAN, Plan};
use alo_renderer::agent_ran::{Ended, Ran};
use alo_renderer::agent_work::{AtWork, Now, Step};
use alo_renderer::card::Card;
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

/// Its plan of three steps, on the second.
fn plan() -> Plan {
    Plan {
        steps: vec![
            "Search Wikipedia for Kampala".to_owned(),
            "Open the article on Kampala".to_owned(),
            "Read its population".to_owned(),
        ],
        on: 1,
    }
}

/// It acting on the second step of its plan, two steps taken.
fn planned() -> AtWork {
    AtWork {
        plan: Some(plan()),
        asked: "Find the population of Kampala on Wikipedia".to_owned(),
        steps: vec![
            step("Typing \u{201c}Kampala\u{201d} into Search Wikipedia", ""),
            step("Pressing Search", ""),
        ],
        now: Now::Acting,
        choosing: None,
    }
}

#[test]
fn in_the_split_view_the_plan_stands_under_what_was_asked_a_line_each() -> Result<(), String> {
    let renderer = laid_out(with(planned(), NORMAL)?);
    let at = |class: &str| boxes_of(&renderer, class);
    // What was asked as before, 10 under the working line; 10 under it the
    // plan, three lines of 20 2 apart: 64; 10 under it the steps.
    assert_eq!(at("asked"), vec![px(849.0, 78.0, 351.0, 20.0)]);
    assert_eq!(at("plan"), vec![px(849.0, 108.0, 351.0, 64.0)]);
    assert_eq!(at("pstep done"), vec![px(849.0, 108.0, 351.0, 20.0)]);
    assert_eq!(at("pstep now"), vec![px(849.0, 130.0, 351.0, 20.0)]);
    assert_eq!(at("pstep ahead"), vec![px(849.0, 152.0, 351.0, 20.0)]);
    // Each step's words 18 in, past the tick's room of 14 and 4 more.
    let words = rects_of(&renderer, "pwords");
    assert_eq!(words.len(), 3);
    assert!(
        words
            .iter()
            .all(|words| (words.left() - 867.0).abs() < 0.01
                && (words.size.width - 333.0).abs() < 0.01),
        "{words:?}"
    );
    assert_eq!(at("tick").first(), Some(&px(849.0, 108.0, 14.0, 20.0)));
    assert_eq!(at("steps"), vec![px(849.0, 182.0, 351.0, 46.0)]);
    // The page box unmoved by it.
    assert_eq!(at("page"), vec![units(split_place::page_box(NORMAL))]);
    Ok(())
}

#[test]
fn under_the_page_the_plan_stands_in_the_cards_column_too() -> Result<(), String> {
    let room = Size::new(1040.0, 820.0);
    let renderer = laid_out(with(planned(), room)?);
    // Too narrow to split: the column under the page, 16 in from the
    // card's 878 inside — 846 — and the plan 10 under what was asked.
    let asked = rects_of(&renderer, "asked");
    let plan = rects_of(&renderer, "plan");
    match (asked.as_slice(), plan.as_slice()) {
        ([asked], [plan]) => {
            assert!(
                (plan.top() - asked.bottom() - 10.0).abs() < 0.01,
                "{asked:?} {plan:?}"
            );
            assert!((plan.left() - 97.0).abs() < 0.01, "{plan:?}");
            assert!((plan.size.width - 846.0).abs() < 0.01, "{plan:?}");
            assert!((plan.size.height - 64.0).abs() < 0.01, "{plan:?}");
        }
        other => return Err(format!("{other:?}")),
    }
    assert_eq!(boxes_of(&renderer, "pstep now").len(), 1);
    Ok(())
}

#[test]
fn done_every_step_is_ticked_and_a_run_kept_keeps_its_plan() -> Result<(), String> {
    let mut done = planned();
    done.now = Now::Done {
        said: "Kampala has about 1.7 million people.".to_owned(),
    };
    let renderer = laid_out(with(done, NORMAL)?);
    assert_eq!(boxes_of(&renderer, "pstep done").len(), 3);
    assert!(boxes_of(&renderer, "pstep now").is_empty());
    assert!(boxes_of(&renderer, "pstep ahead").is_empty());
    // Kept with the page shown earlier, stopped on its second step: as it
    // stood.
    let url = alo_url::parse("https://en.wikipedia.org/wiki/Kampala")
        .map_err(|_| "the address".to_owned())?;
    let start = Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(1040.0, 820.0),
        turns: vec![Turn::Visited {
            said: "wikipedia".to_owned(),
            url,
            ran: vec![Ran {
                plan: Some(plan()),
                ended: Ended::Stopped {
                    why: "You stopped alo.".to_owned(),
                },
                ..Ran::starting("Find the population")
            }],
        }],
        ..Start::default()
    };
    let renderer = laid_out(start);
    assert_eq!(boxes_of(&renderer, "pstep done").len(), 1);
    assert_eq!(boxes_of(&renderer, "pstep now").len(), 1);
    assert_eq!(boxes_of(&renderer, "pstep ahead").len(), 1);
    Ok(())
}

#[test]
fn an_agent_reads_the_plan_as_a_named_list_each_step_by_where_it_stands() -> Result<(), String> {
    let (_, tree) = drawn(with(planned(), NORMAL)?);
    let outline = tree.to_outline();
    let nodes = tree.nodes();
    let list = nodes
        .iter()
        .find(|node| node.name.as_deref() == Some(PLAN))
        .ok_or_else(|| format!("no list named {PLAN}: {outline}"))?;
    assert_eq!(list.role.as_str(), "list", "{outline}");
    let items: Vec<(Option<&str>, Option<Current>)> = list
        .children
        .iter()
        .map(|item| (item.name.as_deref(), item.states.current))
        .collect();
    assert_eq!(
        items,
        [
            (Some("Done: Search Wikipedia for Kampala"), None),
            (
                Some("Now: Open the article on Kampala"),
                Some(Current::Step)
            ),
            (Some("Next: Read its population"), None),
        ],
        "{outline}"
    );
    Ok(())
}

#[test]
fn the_plan_is_drawn_as_the_reference() -> Result<(), String> {
    let (frame, _) = drawn(with(planned(), NORMAL)?);
    let frame = frame.as_ref().ok_or("nothing was painted")?;
    // The step it is on in the ink, the one ahead greyed, the one done in
    // the quiet colour: the darkest pixel of each line's words.
    let darkest = |top: u32| {
        (867..1100)
            .flat_map(|x| (top..top + 20).map(move |y| (x, y)))
            .filter_map(|(x, y)| pixel(frame, x, y))
            .min_by_key(|rgba| u32::from(rgba[0]) + u32::from(rgba[1]) + u32::from(rgba[2]))
    };
    assert_eq!(
        darkest(130),
        Some([0x10, 0x2a, 0x43, 0xff]),
        "now, in the ink"
    );
    assert_eq!(
        darkest(152),
        Some([0xbd, 0xb6, 0xaa, 0xff]),
        "ahead, greyed"
    );
    assert_eq!(darkest(108), Some([0x5f, 0x5a, 0x52, 0xff]), "done, quiet");
    holds("agent-plan.png", Some(frame))
}
