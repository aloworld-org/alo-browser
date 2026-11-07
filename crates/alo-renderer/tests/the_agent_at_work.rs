/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 704 (ADR 0096, `docs/design/shell.md`, *The agent at work*):
//! the agent at work on a card's page, drawn by the first screen's renderer
//! under the page — the terracotta outline over the page and the line
//! *alo is working on this page · click anywhere to take over* with *Stop*,
//! what was asked, one line per step with why under it when it chose, and
//! the question it asks before pressing what sends, with *Allow* and *Not
//! now*; stuck with its choices; done.
//!
//! Each a test below: a layout assertion of the column and its parts in
//! numbers; reference renders of it acting, asking and stuck, holding the
//! committed frame Chrome streamed for `a_page_as_a_card.rs`
//! (`references/card-frame.png`, the input, never rewritten); the agent
//! reading the column; and a press on *Stop*, *Allow*, *Not now* and a
//! choice claimed as each.
//!
//! And queue item 820: each run kept with a page shown earlier, drawn under
//! its line as the person's words and what alo did — its layout in
//! numbers, its reference (`references/agent-runs-kept.png`), and the agent
//! reading it.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test the_agent_at_work`
//! rewrites the render references. A rewritten reference is a diff
//! somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::agent_document::{ALLOW, ASKS, AT_WORK, NOT_NOW, STEPS, STOP};
use alo_renderer::agent_ran::{CLOSED, Ended, Ran};
use alo_renderer::agent_work::{AtWork, Now, Step, WORKING};
use alo_renderer::card::Card;
use alo_renderer::conversation_claim::Claim;
use alo_renderer::frame::frame_to_png;
use alo_renderer::ran_document::RAN;
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

/// The room the first screen has in the design's window: 1280 wide less
/// the panel's 240, and 820 tall.
const WIDTH: f32 = 1040.0;
const HEIGHT: f32 = 820.0;

/// How far the conversation is scrolled to show the column: what the
/// browser process scrolls it to as the agent says each step, its newest
/// end at the room's foot.
const DOWN: f32 = 200.0;

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

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

/// It acting: two steps, the second under way with why it chose.
fn acting() -> AtWork {
    AtWork {
        asked: "Send my name, Ada, through the form".to_owned(),
        steps: vec![
            step("Typing \u{201c}Ada\u{201d} into Name", ""),
            step(
                "Pressing Say hello first",
                "the form asks for a greeting before it sends",
            ),
        ],
        now: Now::Acting,
        choosing: None,
    }
}

/// It asking before pressing *Send*.
fn asking() -> AtWork {
    AtWork {
        asked: "Send my name, Ada, through the form".to_owned(),
        steps: vec![
            step("Typing \u{201c}Ada\u{201d} into Name", ""),
            step("Pressing Send", ""),
        ],
        now: Now::Asking {
            what: "Press \u{201c}Send\u{201d} on 127.0.0.1:8765? It sends the form on this page."
                .to_owned(),
        },
        choosing: None,
    }
}

/// It stuck, with two choices.
fn stuck() -> AtWork {
    AtWork {
        asked: "Sign me in".to_owned(),
        steps: vec![step("Looking for the sign-in form", "")],
        now: Now::Stuck {
            why: "This page asks for a password, and alo never types one: signing in is yours."
                .to_owned(),
            choices: vec!["Open the sign-in page".to_owned(), "Stop here".to_owned()],
        },
        choosing: None,
    }
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

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    let rgba = frame.pixels.get(at..at + 4)?;
    rgba.try_into().ok()
}

#[test]
fn the_column_stands_under_the_page_as_the_design_draws_it() -> Result<(), String> {
    let renderer = laid_out(with(acting())?);
    let at = |class: &str| boxes_of(&renderer, class);
    // The card, 880 wide at 80, its newest turn's top 40 + 42 + 16 = 98
    // scrolled 200 up: -102. Its 1 of border, 46 of bar, 520 of page, then
    // the column, 171, and its 1 of border: 739 tall, outlined in the acting
    // colour while the agent works.
    assert_eq!(at("card acting"), vec![px(80.0, -102.0, 880.0, 739.0)]);
    assert!(at("card").is_empty());
    // The page box where it always is, and the outline over the whole of
    // it: the page box the browser process aims a person's press at does
    // not move for the agent.
    assert_eq!(at("page"), vec![px(81.0, -55.0, 878.0, 520.0)]);
    assert_eq!(at("ring"), vec![px(81.0, -55.0, 878.0, 520.0)]);
    // The column under it, the card's 878 inside: its line 1, 14 inside,
    // then the working line 36 tall, 16 in either side — 846 — and 10
    // apart, what was asked on 20, the steps (20, then 6, then a line of 20
    // and its why of 18), and 16 under: 1 + 14 + 36 + 10 + 20 + 10 + 64 +
    // 16 = 171.
    assert_eq!(at("agent"), vec![px(81.0, 465.0, 878.0, 171.0)]);
    assert_eq!(at("working"), vec![px(97.0, 480.0, 846.0, 36.0)]);
    // Its words 12 in, one line of 18 in its middle; *Stop* 28 tall at its
    // end, 4 in from its right.
    assert_eq!(at("said"), vec![px(109.0, 489.0, 762.843_75, 18.0)]);
    assert_eq!(
        at("abtn stop"),
        vec![px(883.843_75, 484.0, 55.156_25, 28.0)]
    );
    assert_eq!(at("asked"), vec![px(97.0, 526.0, 846.0, 20.0)]);
    assert_eq!(at("steps"), vec![px(97.0, 556.0, 846.0, 64.0)]);
    assert_eq!(at("step"), vec![px(97.0, 556.0, 846.0, 20.0)]);
    assert_eq!(at("step now"), vec![px(97.0, 582.0, 846.0, 38.0)]);
    assert_eq!(at("why"), vec![px(97.0, 602.0, 846.0, 18.0)]);
    // All of it inside the room the conversation is seen in, which reaches
    // 24 above the box: 40 + 676 = 716.
    assert_eq!(at("thread"), vec![px(48.0, 40.0, 944.0, 676.0)]);
    Ok(())
}

#[test]
fn the_question_stands_under_the_steps_with_allow_and_not_now() -> Result<(), String> {
    let renderer = laid_out(with(asking())?);
    let at = |class: &str| boxes_of(&renderer, class);
    // Two steps, neither under way while it waits on the person: 20, 6,
    // 20. Then 10 under them the question, 12 inside its border of 1 and
    // 14 across: its words on 20, then 10 under them *Allow* and *Not now*
    // 28 tall, 8 apart. 2 + 24 + 20 + 10 + 28 = 84.
    assert_eq!(at("card acting"), vec![px(80.0, -102.0, 880.0, 815.0)]);
    assert_eq!(at("agent"), vec![px(81.0, 465.0, 878.0, 247.0)]);
    assert_eq!(
        at("step"),
        vec![px(97.0, 556.0, 846.0, 20.0), px(97.0, 582.0, 846.0, 20.0)]
    );
    assert!(at("step now").is_empty());
    assert_eq!(at("asks"), vec![px(97.0, 612.0, 846.0, 84.0)]);
    assert_eq!(at("question"), vec![px(112.0, 625.0, 816.0, 20.0)]);
    assert_eq!(at("abtn allow"), vec![px(112.0, 655.0, 60.296_875, 28.0)]);
    assert_eq!(
        at("abtn notnow"),
        vec![px(180.296_88, 655.0, 76.796_875, 28.0)]
    );
    // *Stop* still drawn: it waits, and can be stopped.
    assert_eq!(
        at("abtn stop"),
        vec![px(883.843_75, 484.0, 55.156_25, 28.0)]
    );
    Ok(())
}

#[test]
fn stuck_it_draws_why_and_its_choices_and_no_outline() -> Result<(), String> {
    let renderer = laid_out(with(stuck())?);
    let at = |class: &str| boxes_of(&renderer, class);
    assert_eq!(at("card"), vec![px(80.0, -102.0, 880.0, 715.0)]);
    assert!(at("ring").is_empty() && at("working").is_empty());
    assert!(at("abtn stop").is_empty());
    // What was asked first, 14 inside, its one step, then why and, under
    // it, each choice 28 tall, 8 apart.
    assert_eq!(at("asked"), vec![px(97.0, 480.0, 846.0, 20.0)]);
    assert_eq!(at("because"), vec![px(97.0, 540.0, 846.0, 20.0)]);
    assert_eq!(
        at("achoice"),
        vec![
            px(97.0, 568.0, 161.265_63, 28.0),
            px(266.265_63, 568.0, 85.390_625, 28.0)
        ]
    );
    Ok(())
}

#[test]
fn acting_asking_and_stuck_are_drawn_as_the_references() -> Result<(), String> {
    let (acting, _) = drawn(with(acting())?);
    let frame = acting.as_ref().ok_or("nothing was painted")?;
    // The outline, 2 of the acting colour over the page's left edge, low
    // down where the room shows it: x 81 and 82, y 400.
    for x in [81, 82] {
        assert_eq!(pixel(frame, x, 400), Some([0xb5, 0x57, 0x3a, 0xff]), "{x}");
    }
    // The working line's tint, inside it, clear of its words.
    assert_eq!(pixel(frame, 300, 483), Some([0xf6, 0xe7, 0xe0, 0xff]));
    assert_eq!(holds("agent-acting.png", acting.as_ref()), Ok(()));
    let (asking, _) = drawn(with(asking())?);
    let frame = asking.as_ref().ok_or("nothing was painted")?;
    // *Allow* in the ink, clear of its word.
    assert_eq!(pixel(frame, 114, 669), Some([0x1b, 0x24, 0x33, 0xff]));
    assert_eq!(holds("agent-asking.png", asking.as_ref()), Ok(()));
    let (stuck, _) = drawn(with(stuck())?);
    let frame = stuck.as_ref().ok_or("nothing was painted")?;
    // No outline: the page's own pixel at its left edge.
    assert_ne!(pixel(frame, 82, 400), Some([0xb5, 0x57, 0x3a, 0xff]));
    assert_eq!(holds("agent-stuck.png", stuck.as_ref()), Ok(()));
    Ok(())
}

#[test]
fn an_agent_reads_the_column_as_what_it_is() -> Result<(), String> {
    let (_, tree) = drawn(with(asking())?);
    let outline = tree.to_outline();
    let at_work = tree
        .nodes()
        .into_iter()
        .find(|node| node.role.as_str() == "region" && node.name.as_deref() == Some(AT_WORK))
        .ok_or_else(|| format!("no region named {AT_WORK}: {outline}"))?;
    let inside = alo_renderer::Snapshot {
        root: Some(at_work.clone()),
    }
    .to_outline();
    for said in [WORKING, STOP, STEPS, ASKS, ALLOW, NOT_NOW, "Pressing Send"] {
        assert!(inside.contains(said), "{said} not read: {inside}");
    }
    assert!(inside.contains(&format!("button \"{ALLOW}\"")), "{inside}");
    assert!(inside.contains(&format!("group \"{ASKS}\"")), "{inside}");
    assert!(inside.contains(&format!("list \"{STEPS}\"")), "{inside}");
    // The outline over the page is decoration, and not read.
    assert!(!outline.contains("ring"), "{outline}");
    Ok(())
}

#[test]
fn a_press_on_each_of_its_buttons_is_claimed_as_it() -> Result<(), String> {
    for (x, y, claim) in [
        (911.0, 498.0, Claim::Stop),
        (142.0, 669.0, Claim::Allow),
        (218.0, 669.0, Claim::NotNow),
    ] {
        assert_eq!(
            pressed(with(asking())?, x, y),
            FromRenderer::ConversationClaimed(Some(claim)),
            "{claim} at {x}, {y}"
        );
    }
    // The question's words and the steps claim nothing.
    for (x, y) in [(400.0, 635.0), (400.0, 566.0)] {
        assert_eq!(
            pressed(with(asking())?, x, y),
            FromRenderer::ConversationClaimed(None),
            "{x}, {y}"
        );
    }
    for (x, at) in [(177.0, 0), (309.0, 1)] {
        assert_eq!(
            pressed(with(stuck())?, x, 582.0),
            FromRenderer::ConversationClaimed(Some(Claim::Choice { at })),
            "{x}"
        );
    }
    Ok(())
}

/// The first screen holding one page shown earlier, en.wikipedia.org's
/// article, with two runs kept under its line: one done in three steps, one
/// stopped as alo closed.
fn kept() -> Result<Start, String> {
    let url = alo_url::parse("https://en.wikipedia.org/wiki/Kampala")
        .map_err(|_| "the page's address".to_owned())?;
    Ok(Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        turns: vec![Turn::Visited {
            said: "en.wikipedia.org".to_owned(),
            url,
            ran: vec![
                Ran {
                    asked: "Search Wikipedia for Kampala and open the article".to_owned(),
                    steps: vec![
                        step("Typing Kampala into Search Wikipedia", ""),
                        step("Pressing Search", "it sends the search for Kampala"),
                        step("Opening the article on Kampala", ""),
                    ],
                    ended: Ended::Done {
                        said: "Opened the Wikipedia article on Kampala.".to_owned(),
                    },
                },
                Ran::starting("Find its population"),
            ],
        }],
        ..Start::default()
    })
}

#[test]
fn the_runs_kept_with_a_page_stand_under_its_line_as_turns() -> Result<(), String> {
    let renderer = laid_out(kept()?);
    let at = |class: &str| boxes_of(&renderer, class);
    // The newest turn's top at the room's top, 40: the person's bubble, 42
    // tall (10 + 22 + 10), then 16 under it the page's line, 880 wide at
    // 80 and 44 tall.
    assert_eq!(at("visit"), vec![px(80.0, 98.0, 880.0, 44.0)]);
    // 16 under it the first run asked, the person's bubble at the column's
    // right, 42 tall; 16 under that what alo did, on a box with a border
    // of 1, 14 above and 16 below inside: three steps (20, 6, 20 and a why
    // of 18, 6, 20 = 90), 10, then *Done in 3 steps* and the result, 20
    // each: 1 + 14 + 90 + 10 + 40 + 16 + 1 = 172.
    let requests = at("you request");
    assert_eq!(requests.len(), 2, "{requests:?}");
    let ran = at("ran");
    assert_eq!(
        ran,
        vec![px(80.0, 216.0, 880.0, 172.0), px(80.0, 462.0, 880.0, 52.0)]
    );
    // Inside, 16 in from either side through its border: 846 wide.
    assert_eq!(at("steps"), vec![px(97.0, 231.0, 846.0, 90.0)]);
    assert_eq!(
        at("step"),
        vec![
            px(97.0, 231.0, 846.0, 20.0),
            px(97.0, 257.0, 846.0, 38.0),
            px(97.0, 301.0, 846.0, 20.0)
        ]
    );
    assert_eq!(at("why"), vec![px(97.0, 277.0, 846.0, 18.0)]);
    assert_eq!(at("done"), vec![px(97.0, 331.0, 846.0, 20.0)]);
    assert_eq!(at("result"), vec![px(97.0, 351.0, 846.0, 20.0)]);
    // The second asked 16 under the first box, 42 tall, at 404; its box 16
    // under, at 462: 1 + 14 + its one line of 20 + 16 + 1 = 52.
    assert_eq!(at("stopped"), vec![px(97.0, 477.0, 846.0, 20.0)]);
    // Nothing of a live card, and nothing pressable.
    assert!(at("agent").is_empty() && at("abtn stop").is_empty() && at("achoice").is_empty());
    Ok(())
}

#[test]
fn the_runs_kept_are_drawn_as_the_reference_and_read_as_what_alo_did() -> Result<(), String> {
    let (frame, tree) = drawn(kept()?);
    let painted = frame.as_ref().ok_or("nothing was painted")?;
    // The box's border in the line's colour (`#ddd6ca`) at its left edge,
    // clear of its corners, and the surface inside it.
    assert_eq!(pixel(painted, 80, 300), Some([0xdd, 0xd6, 0xca, 0xff]));
    assert_eq!(pixel(painted, 90, 300), Some([0xff, 0xff, 0xff, 0xff]));
    assert_eq!(holds("agent-runs-kept.png", frame.as_ref()), Ok(()));
    let outline = tree.to_outline();
    let regions: Vec<String> = tree
        .nodes()
        .into_iter()
        .filter(|node| node.role.as_str() == "region" && node.name.as_deref() == Some(RAN))
        .map(|node| {
            alo_renderer::Snapshot {
                root: Some(node.clone()),
            }
            .to_outline()
        })
        .collect();
    assert_eq!(regions.len(), 2, "{outline}");
    let first = regions.first().ok_or("no region")?;
    for said in [
        STEPS,
        "Pressing Search",
        "it sends the search for Kampala",
        "Done in 3 steps",
        "Opened the Wikipedia article on Kampala.",
    ] {
        assert!(first.contains(said), "{said} not read: {first}");
    }
    assert!(regions.get(1).is_some_and(|second| second.contains(CLOSED)));
    for asked in [
        "Search Wikipedia for Kampala and open the article",
        "Find its population",
    ] {
        assert!(outline.contains(asked), "{asked} not read: {outline}");
    }
    // Nothing in the conversation is pressable but the page's line.
    let conversation = tree
        .nodes()
        .into_iter()
        .find(|node| node.role.as_str() == "region" && node.name.as_deref() == Some("Conversation"))
        .map(|node| {
            alo_renderer::Snapshot {
                root: Some(node.clone()),
            }
            .to_outline()
        })
        .ok_or_else(|| format!("no conversation: {outline}"))?;
    assert!(!conversation.contains("button"), "{conversation}");
    Ok(())
}
