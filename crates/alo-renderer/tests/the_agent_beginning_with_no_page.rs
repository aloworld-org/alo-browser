/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 821 (ADR 0103, `docs/design/shell.md`, *Beginning with no
//! page*; the owner's design of 8 November 2026): alo asked with no page
//! open. The box's second line, *Ask alo to do it* — *alo finds the site
//! and works on it*; the run's column in a turn of its own while it finds
//! the site — the working line *alo is finding the site to work on* with
//! *Stop*, what was asked, its plan and steps, and the question *alo wants
//! to open …* with *Allow* and *Not now*; the same run kept once a newer
//! turn comes; and a card's page handed to the person to sign in, with
//! *Let alo continue*.
//!
//! Each a test below: layout assertions in numbers, reference renders
//! (`references/agent-beginning.png`, `agent-asks-to-open.png`,
//! `agent-sign-in.png`, `start-ask-to-do-it.png`), the agent reading them,
//! and a press on *Stop*, *Allow* and *Not now* claimed as each.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test the_agent_beginning_with_no_page`
//! rewrites the render references. A rewritten reference is a diff
//! somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::agent_asked::Asked;
use alo_renderer::agent_document::{ALLOW, ASKS, AT_WORK, CONTINUE, NOT_NOW, STOP};
use alo_renderer::agent_plan::Plan;
use alo_renderer::agent_ran::{Ended, Ran};
use alo_renderer::agent_work::{AtWork, FINDING, Now, SIGN_IN, Step};
use alo_renderer::card::Card;
use alo_renderer::conversation_claim::Claim;
use alo_renderer::frame::frame_to_png;
use alo_renderer::results::SetUp;
use alo_renderer::start::Start;
use alo_renderer::start_document::{ASK_TO_DO_IT, FINDS_THE_SITE};
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

/// What was asked, and the plan it wrote.
const ASKED: &str = "Find the opening hours of the Uganda Museum";

fn plan(on: usize) -> Plan {
    Plan {
        steps: vec![
            "Find the museum\u{2019}s site".to_owned(),
            "Read its opening hours".to_owned(),
        ],
        on,
    }
}

/// The run at work with no page: a search done, the next step under way.
fn finding() -> AtWork {
    AtWork {
        plan: Some(plan(0)),
        asked: ASKED.to_owned(),
        steps: vec![step(
            "Searching for \u{201c}Uganda Museum opening hours\u{201d}",
            "",
        )],
        now: Now::Acting,
        choosing: None,
    }
}

/// It asking before opening a site neither found nor named.
fn asking_to_open() -> AtWork {
    AtWork {
        now: Now::Asking {
            what: "alo wants to open wikipedia.org. It is not a site a search found or one you \
                   named."
                .to_owned(),
            paying: None,
        },
        steps: vec![
            step(
                "Searching for \u{201c}Uganda Museum opening hours\u{201d}",
                "",
            ),
            step(
                "Opening Wikipedia",
                "the museum\u{2019}s own site did not say",
            ),
        ],
        ..finding()
    }
}

/// The first screen holding one turn asked with no page, its run `work`.
fn begun(work: AtWork) -> Start {
    Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        turns: vec![Turn::Asked(Asked {
            said: ASKED.to_owned(),
            agent: Some(work),
            ran: vec![Ran::starting(ASKED)],
        })],
        ..Start::default()
    }
}

#[test]
fn the_column_stands_in_its_own_turn_while_it_finds_the_site() {
    let renderer = laid_out(begun(finding()));
    let at = |class: &str| boxes_of(&renderer, class);
    // The turn at the room's top, 40: the person's bubble, 42 tall, at its
    // right; 16 under it the run's white box, the column's 880 at 80,
    // rounded 12 under a line of 1 — no page above it, so no rule.
    assert_eq!(at("turn"), vec![px(80.0, 40.0, 880.0, 238.0)]);
    assert_eq!(at("begun"), vec![px(80.0, 98.0, 880.0, 180.0)]);
    // Inside it the column as a card's: 14 in, the working line 36 tall,
    // 16 in either side; 10 apart what was asked on 20, the plan's two
    // lines of 20, 2 apart, and the step under way; 16 under: 14 + 36 + 10
    // + 20 + 10 + 42 + 10 + 20 + 16 = 178.
    assert_eq!(at("agent"), vec![px(81.0, 99.0, 878.0, 178.0)]);
    assert_eq!(at("working"), vec![px(97.0, 113.0, 846.0, 36.0)]);
    // alo's mark 12 in, then its words 8 after it (queue item 974).
    assert_eq!(at("acting-mark"), vec![px(109.0, 123.0, 16.0, 16.0)]);
    assert_eq!(at("said"), vec![px(133.0, 122.0, 738.640_6, 18.0)]);
    assert_eq!(
        at("abtn stop"),
        vec![px(883.640_6, 117.0, 55.359_375, 28.0)]
    );
    assert_eq!(at("asked"), vec![px(97.0, 159.0, 846.0, 20.0)]);
    assert_eq!(at("plan"), vec![px(97.0, 189.0, 846.0, 42.0)]);
    assert_eq!(at("step now"), vec![px(97.0, 241.0, 846.0, 20.0)]);
    // No page, so no card, no outline and no page box.
    for none in ["card", "card acting", "ring", "page"] {
        assert!(at(none).is_empty(), "{none}");
    }
}

#[test]
fn the_question_to_open_stands_under_its_steps() {
    let renderer = laid_out(begun(asking_to_open()));
    let at = |class: &str| boxes_of(&renderer, class);
    // Two steps, the second with why: 20, 6, 20 and 18. Then 10 under them
    // the question's box, 12 by 14 inside its line of 1: its words, then
    // *Allow* and *Not now* 28 tall, 8 apart, 10 under: 84.
    assert_eq!(at("agent"), vec![px(81.0, 99.0, 878.0, 316.0)]);
    assert_eq!(at("steps"), vec![px(97.0, 241.0, 846.0, 64.0)]);
    assert_eq!(at("asks"), vec![px(97.0, 315.0, 846.0, 84.0)]);
    assert_eq!(at("question"), vec![px(112.0, 328.0, 816.0, 20.0)]);
    assert_eq!(at("abtn allow"), vec![px(112.0, 358.0, 59.390_625, 28.0)]);
    assert_eq!(
        at("abtn notnow"),
        vec![px(179.390_63, 358.0, 77.062_5, 28.0)]
    );
    // *Stop* still drawn while it asks.
    assert_eq!(at("abtn stop").len(), 1);
}

#[test]
fn finding_and_asking_to_open_are_drawn_as_the_references() -> Result<(), String> {
    let (finding, _) = drawn(begun(finding()));
    let frame = finding.as_ref().ok_or("nothing was painted")?;
    // The working line's tint, clear of its words; the box's white beside
    // the column.
    assert_eq!(pixel(frame, 600, 116), Some([0xe2, 0xed, 0xee, 0xff]));
    assert_eq!(pixel(frame, 88, 200), Some([0xff, 0xff, 0xff, 0xff]));
    // No outline in the acting colour anywhere down its left edge.
    for y in (100..276).step_by(8) {
        assert_ne!(pixel(frame, 81, y), Some([0x0f, 0x6b, 0x72, 0xff]), "{y}");
    }
    assert_eq!(holds("agent-beginning.png", finding.as_ref()), Ok(()));
    let (asking, _) = drawn(begun(asking_to_open()));
    let frame = asking.as_ref().ok_or("nothing was painted")?;
    // *Allow* in the ink, clear of its word.
    assert_eq!(pixel(frame, 114, 372), Some([0x10, 0x2a, 0x43, 0xff]));
    assert_eq!(holds("agent-asks-to-open.png", asking.as_ref()), Ok(()));
    Ok(())
}

#[test]
fn an_agent_reads_the_run_before_any_page_as_what_it_is() -> Result<(), String> {
    let (_, tree) = drawn(begun(asking_to_open()));
    let outline = tree.to_outline();
    let at_work = tree
        .nodes()
        .into_iter()
        .find(|node| node.role.as_str() == "region" && node.name.as_deref() == Some(AT_WORK))
        .ok_or_else(|| format!("no region named {AT_WORK}: {outline}"))?;
    let inside = Snapshot {
        root: Some(at_work.clone()),
    }
    .to_outline();
    for said in [
        FINDING,
        STOP,
        ASKS,
        ALLOW,
        NOT_NOW,
        "alo wants to open wikipedia.org",
    ] {
        assert!(inside.contains(said), "{said} not read: {inside}");
    }
    assert!(inside.contains(&format!("button \"{ALLOW}\"")), "{inside}");
    // No page is read where there is none.
    assert!(!outline.contains("image"), "{outline}");
    Ok(())
}

#[test]
fn a_press_on_stop_allow_and_not_now_is_claimed_as_each() {
    for (x, y, claim) in [
        (911.0, 131.0, Claim::Stop),
        (142.0, 372.0, Claim::Allow),
        (218.0, 372.0, Claim::NotNow),
    ] {
        assert_eq!(
            pressed(begun(asking_to_open()), x, y),
            FromRenderer::ConversationClaimed(Some(claim)),
            "{claim} at {x}, {y}"
        );
    }
    // The question's words and the box around the column claim nothing.
    for (x, y) in [(400.0, 338.0), (85.0, 200.0)] {
        assert_eq!(
            pressed(begun(asking_to_open()), x, y),
            FromRenderer::ConversationClaimed(None),
            "{x}, {y}"
        );
    }
}

#[test]
fn put_away_or_read_back_it_is_drawn_as_it_is_kept() {
    let kept = Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        turns: vec![
            Turn::Asked(Asked {
                said: ASKED.to_owned(),
                agent: None,
                ran: vec![Ran {
                    asked: ASKED.to_owned(),
                    plan: Some(plan(1)),
                    steps: vec![step("Searching for the museum", "")],
                    ended: Ended::Stopped {
                        why: "Not done: you said not now. alo did not open wikipedia.org."
                            .to_owned(),
                    },
                }],
            }),
            Turn::Failed {
                said: "later".to_owned(),
                why: "a newer turn".to_owned(),
            },
        ],
        ..Start::default()
    };
    let renderer = laid_out(kept);
    let at = |class: &str| boxes_of(&renderer, class);
    // No live column: the run on a white box as a page's runs are kept,
    // under the bubble that says what was asked — once, not twice.
    assert!(at("begun").is_empty());
    assert!(at("agent").is_empty());
    assert_eq!(at("ran").len(), 1);
    assert!(at("you request").is_empty(), "the bubble is the turn's own");
    let (_, tree) = drawn(Start {
        turns: vec![Turn::Asked(Asked {
            said: ASKED.to_owned(),
            agent: None,
            ran: vec![Ran::starting(ASKED)],
        })],
        viewport: Size::new(WIDTH, HEIGHT),
        ..Start::default()
    });
    let outline = tree.to_outline();
    // Its words twice, as every turn's: the region named by the bubble, and
    // the bubble — never a third time over the kept run.
    assert_eq!(outline.matches(ASKED).count(), 2, "{outline}");
    assert!(
        outline.contains(alo_renderer::ran_document::RAN),
        "{outline}"
    );
}

#[test]
fn the_box_with_no_page_offers_to_ask_alo_to_do_it() {
    let start = Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        typed: ASKED.to_owned(),
        focused: true,
        offers: alo_renderer::typed::offers(ASKED),
        search: SetUp::Ready,
        asking: SetUp::Ready,
        ..Start::default()
    };
    let renderer = laid_out(start.clone());
    // Searching first, what Return chooses; asking alo the second line,
    // 52 under it — where a question's offers always stand.
    assert_eq!(
        boxes_of(&renderer, "offers"),
        vec![px(160.0, 326.0, 720.0, 118.0)]
    );
    assert_eq!(
        boxes_of(&renderer, "offer chosen"),
        vec![px(167.0, 333.0, 706.0, 52.0)]
    );
    assert_eq!(
        boxes_of(&renderer, "offer"),
        vec![px(167.0, 385.0, 706.0, 52.0)]
    );
    let (frame, tree) = drawn(start.clone());
    let outline = tree.to_outline();
    assert!(
        outline.contains(&format!("option \"{ASK_TO_DO_IT} {FINDS_THE_SITE}\"")),
        "{outline}"
    );
    assert_eq!(
        pressed(start, 300.0, 410.0),
        FromRenderer::ConversationClaimed(Some(Claim::Offer { at: 1 }))
    );
    assert_eq!(holds("start-ask-to-do-it.png", frame.as_ref()), Ok(()));
}

/// The first screen holding a card of Chrome's frame, loaded, its page
/// handed to the person to sign in as `now` says, scrolled so its column
/// is in the room.
fn on_a_card(now: Now) -> Result<Start, String> {
    let url = alo_url::parse("http://127.0.0.1:8765/card.html")
        .map_err(|_| "the card's address".to_owned())?;
    let mut card = Card::new("127.0.0.1:8765/card.html", url)
        .holding(frame()?)
        .loaded();
    card.agent = Some(AtWork { now, ..finding() });
    Ok(Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        turns: vec![Turn::Page(card)],
        down: 200.0,
        ..Start::default()
    })
}

#[test]
fn a_sign_in_page_is_handed_over_where_taking_over_is_drawn() -> Result<(), String> {
    let signing_in = laid_out(on_a_card(Now::SignIn)?);
    let taken_over = laid_out(on_a_card(Now::Paused)?);
    // Every box where the page taken over has it: only the words differ,
    // their line where its line is.
    let line = |renderer: &Renderer| {
        boxes_of(renderer, "pausedsaid")
            .into_iter()
            .map(|written| written.split(' ').next().unwrap_or_default().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(line(&signing_in), line(&taken_over));
    assert_eq!(line(&signing_in), vec!["(97,".to_owned()]);
    for class in ["card", "page", "agent", "paused", "abtn continue"] {
        assert_eq!(
            boxes_of(&signing_in, class),
            boxes_of(&taken_over, class),
            "{class}"
        );
        assert_eq!(boxes_of(&signing_in, class).len(), 1, "{class}");
    }
    // No outline and no *Stop*: the page is the person's.
    assert!(boxes_of(&signing_in, "ring").is_empty());
    assert!(boxes_of(&signing_in, "abtn stop").is_empty());
    let (frame, tree) = drawn(on_a_card(Now::SignIn)?);
    let outline = tree.to_outline();
    assert!(outline.contains(SIGN_IN), "{outline}");
    assert!(
        outline.contains(&format!("button \"{CONTINUE}\"")),
        "{outline}"
    );
    assert_eq!(holds("agent-sign-in.png", frame.as_ref()), Ok(()));
    Ok(())
}
