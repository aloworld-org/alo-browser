/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 883: a finished run's result keeps the lines and lists the
//! model wrote — the museum's hours as lines, the tickets as a list — on the
//! live card and in a run kept with a page shown earlier, where before they
//! were drawn as one run of text.
//!
//! Each a test below: the result's blocks in numbers on the card and in the
//! kept run; reference renders of both (`references/agent-done-in-lines.png`,
//! `references/agent-ran-in-lines.png`), the card holding the committed
//! frame Chrome streamed for `a_page_as_a_card.rs`
//! (`references/card-frame.png`, the input, never rewritten); and an agent
//! reading the result as a paragraph and a list.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test
//! the_agents_result_keeps_its_lines` rewrites the render references. A
//! rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::from_png;
use alo_renderer::agent_ran::{Ended, Ran};
use alo_renderer::agent_work::{AtWork, Now, Step};
use alo_renderer::card::Card;
use alo_renderer::frame::frame_to_png;
use alo_renderer::start::Start;
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, Snapshot, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::{Path, PathBuf};

/// The result as the model wrote it in 821's hand run, with a list after.
const HOURS: &str = "Monday\u{2013}Saturday: 8:00 am\u{2013}6:00 pm (last entry 5:00 pm)\n\
Sunday: Closed\n\
Public holidays: 10:00 am\u{2013}6:00 pm\n\
\n\
Tickets:\n\
- Adults: 10,000 shillings\n\
- Children: 5,000 shillings";

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

/// The room the first screen has in the design's window: 1280 wide less
/// the panel's 240, and 820 tall.
const WIDTH: f32 = 1040.0;
const HEIGHT: f32 = 820.0;

/// How far the conversation is scrolled to show the column.
const DOWN: f32 = 260.0;

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

fn step(line: &str) -> Step {
    Step {
        line: line.to_owned(),
        why: String::new(),
    }
}

/// The first screen holding a card of Chrome's frame, loaded, its agent
/// done with [`HOURS`].
fn done() -> Result<Start, String> {
    let path = reference("card-frame.png");
    let frame =
        std::fs::read(&path).map_err(|why| format!("{} is not there: {why}", path.display()))?;
    let url = alo_url::parse("http://127.0.0.1:8765/card.html")
        .map_err(|_| "the card's address".to_owned())?;
    let mut card = Card::new("127.0.0.1:8765/card.html", url)
        .holding(frame)
        .loaded();
    card.agent = Some(AtWork {
        plan: None,
        asked: "When is the Uganda Museum open?".to_owned(),
        steps: vec![step("Reading the museum\u{2019}s hours")],
        now: Now::Done {
            said: HOURS.to_owned(),
        },
        choosing: None,
    });
    Ok(Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        turns: vec![Turn::Page(card)],
        down: DOWN,
        ..Start::default()
    })
}

/// The first screen holding one page shown earlier with that run kept.
fn kept() -> Result<Start, String> {
    let url = alo_url::parse("https://ugandamuseum.go.ug/hours")
        .map_err(|_| "the page's address".to_owned())?;
    Ok(Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        turns: vec![Turn::Visited {
            said: "ugandamuseum.go.ug".to_owned(),
            url,
            ran: vec![Ran {
                plan: None,
                asked: "When is the Uganda Museum open?".to_owned(),
                steps: vec![step("Reading the museum\u{2019}s hours")],
                ended: Ended::Done {
                    said: HOURS.to_owned(),
                },
            }],
        }],
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

/// The border box of every element named `name` (any, when empty) of
/// `class`, in document order.
fn boxes_of(renderer: &Renderer, name: &str, class: &str) -> Vec<String> {
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
                .is_some_and(|node| {
                    document.attribute_by_name(node, "class") == Some(class)
                        && (name.is_empty()
                            || document
                                .element(node)
                                .is_some_and(|element| &*element.name.local == name))
                })
        })
        .filter_map(|id| drawing.layout.get(id))
        .map(|geometry| units(geometry.border_box))
        .collect()
}

/// The border box of every `li` inside an element of `class`.
fn items_of(renderer: &Renderer, class: &str) -> Vec<String> {
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
                .is_some_and(|node| {
                    document
                        .element(node)
                        .is_some_and(|element| &*element.name.local == "li")
                        && document
                            .parent(node)
                            .and_then(|list| document.attribute_by_name(list, "class"))
                            == Some(class)
                })
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

/// The outline of the tree an agent reads, from the region named `name`.
fn region(tree: &Snapshot, name: &str) -> Option<String> {
    tree.nodes()
        .into_iter()
        .find(|node| node.role.as_str() == "region" && node.name.as_deref() == Some(name))
        .map(|node| {
            Snapshot {
                root: Some(node.clone()),
            }
            .to_outline()
        })
}

#[test]
fn on_the_card_the_hours_are_lines_and_the_tickets_a_list() -> Result<(), String> {
    let renderer = laid_out(done()?);
    let result = |name: &str| boxes_of(&renderer, name, "result");
    // The card's top at 98 scrolled 260 up, -162; its border, bar and page
    // (1 + 46 + 520) put the column at 405; its line and 14 inside, what
    // was asked at 420, the one step 10 under at 450, and *Done in 1 step*
    // 10 under that, at 480, the column's 846 wide at 97.
    assert_eq!(
        boxes_of(&renderer, "p", "done"),
        vec![px(97.0, 480.0, 846.0, 20.0)]
    );
    // The hours' three lines of 20 right under it; 8 under them *Tickets:*,
    // one line.
    assert_eq!(
        result("p"),
        vec![px(97.0, 500.0, 846.0, 60.0), px(97.0, 568.0, 846.0, 20.0)]
    );
    // 8 under that the list, its items 18 in, 20 each and 2 apart.
    assert_eq!(result("ul"), vec![px(97.0, 596.0, 846.0, 42.0)]);
    assert_eq!(
        items_of(&renderer, "result"),
        vec![px(115.0, 596.0, 828.0, 20.0), px(115.0, 618.0, 828.0, 20.0)]
    );
    // And the column grows to hold it: 16 under the list and its line.
    let agent = boxes_of(&renderer, "", "agent");
    assert_eq!(agent, vec![px(81.0, 405.0, 878.0, 249.0)]);
    Ok(())
}

#[test]
fn in_a_run_kept_the_result_stands_as_on_the_card() -> Result<(), String> {
    let renderer = laid_out(kept()?);
    let result = |name: &str| boxes_of(&renderer, name, "result");
    // The kept run's box at 80, 880 wide, its top at 216 (the page's line
    // and the person's bubble above, as `the_agent_at_work.rs` has them):
    // 1 of border and 14 inside, its one step of 20, 10, then *Done in 1
    // step*, 16 in through its border — 846 wide at 97.
    let done = boxes_of(&renderer, "p", "done");
    assert_eq!(done, vec![px(97.0, 261.0, 846.0, 20.0)]);
    // The hours' three lines right under it, 8, *Tickets:*, 8, the list.
    assert_eq!(
        result("p"),
        vec![px(97.0, 281.0, 846.0, 60.0), px(97.0, 349.0, 846.0, 20.0)]
    );
    assert_eq!(result("ul"), vec![px(97.0, 377.0, 846.0, 42.0)]);
    assert_eq!(
        items_of(&renderer, "result"),
        vec![px(115.0, 377.0, 828.0, 20.0), px(115.0, 399.0, 828.0, 20.0)]
    );
    // The box holds it all: 1 + 14 + 20 + 10 + 20 + 60 + 8 + 20 + 8 + 42
    // + 16 + 1 = 220.
    assert_eq!(
        boxes_of(&renderer, "", "ran"),
        vec![px(80.0, 216.0, 880.0, 220.0)]
    );
    Ok(())
}

#[test]
fn both_are_drawn_as_the_references_and_read_as_lines_and_a_list() -> Result<(), String> {
    for (name, start, named) in [
        ("agent-done-in-lines.png", done()?, "alo at work"),
        ("agent-ran-in-lines.png", kept()?, "What alo did"),
    ] {
        let (frame, tree) = drawn(start);
        assert_eq!(holds(name, frame.as_ref()), Ok(()));
        let read = region(&tree, named).ok_or_else(|| format!("{name}: no {named}"))?;
        for said in [
            "Sunday: Closed",
            "Tickets:",
            "Adults: 10,000 shillings",
            "Children: 5,000 shillings",
        ] {
            assert!(read.contains(said), "{name}: {said} not read: {read}");
        }
        // The tickets as a list of two items, not a run of text.
        assert!(read.contains("list"), "{name}: {read}");
        assert_eq!(read.matches("listitem").count(), 2 + 1, "{name}: {read}");
    }
    Ok(())
}
