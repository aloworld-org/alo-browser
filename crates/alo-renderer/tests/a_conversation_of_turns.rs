/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 703 (ADR 0085): the conversation keeps every turn — a search's
//! results, a page shown earlier, a page that could not be opened, a page
//! live as a card — oldest first, and scrolls.
//!
//! What closes its engine half, each a test below: layout assertions of two
//! turns and of three, each turn's box in numbers and the newest at the
//! room's top; the same conversation scrolled back, every box moved by
//! exactly as much and the clip held above the box and under the room's
//! top; the card's page box where the browser process computes it, at
//! offsets either way; how far the conversation reaches, as the renderer
//! answers it; a press on a title of an earlier turn, and on a page shown
//! earlier, claimed by its place, scrolled; the agent reading each turn as
//! a region named by what was said; strangers' strings staying text;
//! reference renders unscrolled, scrolled back, and scrolled at two; and the
//! same through the real, confined renderer.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_conversation_of_turns`
//! rewrites the references. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::card::Card;
use alo_renderer::card_place;
use alo_renderer::conversation_claim::Claim;
use alo_renderer::conversation_scroll::{self, Reach};
use alo_renderer::face::Face;
use alo_renderer::frame::frame_to_png;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::results::{Found, Results, SetUp, Showing};
use alo_renderer::start::{self, Start};
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, Snapshot, SnapshotNode, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::{Path, PathBuf};

/// The corpus's faces: its sans-serif for both weights, which is what
/// `system-ui` and `sans-serif` mean here, and its serif for the greeting.
fn faces() -> Vec<(&'static str, Weight, &'static [u8])> {
    vec![
        ("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
        ("DejaVu Sans", Weight::BOLD, dejavu::sans::bold()),
        ("DejaVu Serif", Weight::NORMAL, dejavu::serif::regular()),
    ]
}

fn generics() -> Vec<(&'static str, &'static str)> {
    vec![
        ("system-ui", "DejaVu Sans"),
        ("sans-serif", "DejaVu Sans"),
        ("serif", "DejaVu Serif"),
    ]
}

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (family, weight, bytes) in faces() {
        if let Some(font) = Font::load(family, weight, Slant::Normal, bytes.to_vec()) {
            database.add(font);
        }
    }
    for (generic, family) in generics() {
        database.map_generic(generic, family);
    }
    // The design's typefaces, as the browser process hands them to the
    // interface's renderers (ADR 0084).
    for font in alo_renderer::typefaces::fonts() {
        database.add(font);
    }
    database
}

/// The room the first screen has in the design's window: 1280 wide less the
/// panel's 240, and the window's 820 tall.
const WIDTH: f32 = 1040.0;
const HEIGHT: f32 = 820.0;

fn viewport() -> Size {
    Size::new(WIDTH, HEIGHT)
}

/// A rectangle as the outline writes it: layout holds whole 1/64 px, so two
/// rectangles that write alike are the same, compared without comparing
/// floats.
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

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

/// Chrome's frame of a page of the card test's own, the input that test
/// committed and never rewrites.
fn frame() -> Result<Vec<u8>, String> {
    let path = reference("card-frame.png");
    std::fs::read(&path).map_err(|why| format!("{} is not there: {why}", path.display()))
}

/// Compare `frame` with the committed reference, or write it when updating.
/// What differs, in words, if anything does.
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

const PAPER: Option<[u8; 4]> = Some([0xf4, 0xf1, 0xec, 0xff]);
const WHITE: Option<[u8; 4]> = Some([0xff, 0xff, 0xff, 0xff]);

/// What a renderer holding a first screen answers to a paint, a reading
/// and the question of how far it reaches.
struct Drawn {
    renderer: Renderer,
    frame: Option<Frame>,
    tree: Snapshot,
    reach: Option<Reach>,
}

fn draw(start: Start) -> Drawn {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Start(Box::new(start)));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{answer:?}"
    );
    let frame = match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Some(frame),
        _ => None,
    };
    let tree = match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(tree) => *tree,
        _ => Snapshot::default(),
    };
    let reach = match renderer.handle(ToRenderer::ConversationReach) {
        FromRenderer::ConversationReached(reach) => Some(reach),
        _ => None,
    };
    Drawn {
        renderer,
        frame,
        tree,
        reach,
    }
}

impl Drawn {
    /// The border box of every element whose whole class is `class`, in
    /// document order.
    fn at(&self, class: &str) -> Vec<String> {
        let renderer = &self.renderer;
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

    /// Every turn's border box, oldest first: both classes a turn has.
    fn turns(&self) -> Vec<String> {
        let renderer = &self.renderer;
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
                    .and_then(|node| document.attribute_by_name(node, "class"))
                    .is_some_and(|class| class == "turn" || class == "turn searched")
            })
            .filter_map(|id| drawing.layout.get(id))
            .map(|geometry| units(geometry.border_box))
            .collect()
    }
}

/// Every node the agent reads in `role`, in order.
fn read<'a>(tree: &'a Snapshot, role: &str) -> Vec<&'a SnapshotNode> {
    tree.nodes()
        .into_iter()
        .filter(|node| node.role.as_str() == role)
        .collect()
}

fn names(nodes: &[&SnapshotNode]) -> Vec<Option<String>> {
    nodes.iter().map(|node| node.name.clone()).collect()
}

fn url(address: &str) -> alo_url::Url {
    alo_url::parse(address).unwrap_or_else(|_| alo_url::Url::about_blank())
}

/// What the person searched for first.
const SAID: &str = "home ev charger installation";

/// The first turn: a search, three sites on All, as the results test draws
/// them.
fn searched() -> Turn {
    let site = |title: &str, address: &str, description: &str, name: &str| {
        Found::new(title, url(address), description).named(name)
    };
    Turn::Searched(Results::of(
        SAID.to_owned(),
        Showing::Found(vec![
            site(
                "Installing a charger at home: what to expect",
                "https://evhomeguide.org/installing-a-charger",
                "A wall charger needs its own circuit from your fuse box. What an electrician \
                 checks, how long fitting takes, and what to ask before you book.",
                "EV Home Guide",
            ),
            site(
                "Home EV chargers compared: features that matter",
                "https://energyreport.example/home-chargers",
                "Cable length, smart scheduling and app control, explained in plain terms, \
                 with what to skip if you only drive short distances.",
                "",
            ),
            site("Chargers near you", "https://chips.example/", "", "Chips"),
        ]),
    ))
}

/// A page opened from the first site, and closed: kept as its address.
fn visited() -> Turn {
    Turn::Visited {
        said: "Installing a charger at home: what to expect".to_owned(),
        url: url("https://evhomeguide.org/installing-a-charger"),
    }
}

/// An address that could not be opened, and why.
fn failed() -> Turn {
    Turn::Failed {
        said: "reddit.com".to_owned(),
        why: "https://reddit.com/ could not be opened: Chrome said nothing for 30000 ms".to_owned(),
    }
}

/// The newest turn: a page live as a card, holding Chrome's frame, loaded.
fn live() -> Result<Turn, String> {
    Ok(Turn::Page(
        Card::new(
            "127.0.0.1:8765/card.html",
            url("http://127.0.0.1:8765/card.html"),
        )
        .holding(frame()?)
        .loaded(),
    ))
}

fn conversation(turns: Vec<Turn>, down: f32) -> Start {
    Start {
        greeting: "Hello.".to_owned(),
        viewport: viewport(),
        turns,
        down,
        search: SetUp::Ready,
        ..Start::default()
    }
}

fn two() -> Result<Vec<Turn>, String> {
    Ok(vec![searched(), live()?])
}

fn three() -> Result<Vec<Turn>, String> {
    Ok(vec![searched(), failed(), live()?])
}

/// The search's turn: what was said 42 tall at the column's right, 20 under
/// it the results, 395 tall: 457. The failure's: 42, 16, one line of 23: 81.
/// A live card's: 42, 16, 568: 626. 32 between turns.
const SEARCHED: f32 = 457.0;
const FAILED: f32 = 81.0;
const LIVE: f32 = 626.0;
const BETWEEN: f32 = 32.0;

#[test]
fn two_turns_stand_oldest_first_and_the_newest_is_at_the_rooms_top() -> Result<(), String> {
    let drawn = draw(conversation(two()?, 0.0));
    // The newest — the live card's turn — at the room's top, 40; the search
    // 32 above it, its whole 457.
    let top = 40.0 - BETWEEN - SEARCHED;
    assert_eq!(
        drawn.turns(),
        vec![px(80.0, top, 880.0, SEARCHED), px(80.0, 40.0, 880.0, LIVE),]
    );
    assert_eq!(drawn.at("earlier"), vec![px(80.0, top, 880.0, SEARCHED)]);
    // The clip is the thread, from 40 to 24 over the box at 740.
    assert_eq!(drawn.at("thread"), vec![px(48.0, 40.0, 944.0, 676.0)]);
    assert_eq!(drawn.at("box"), vec![px(80.0, 740.0, 880.0, 56.0)]);
    // The card where it stands with one turn (`a_page_as_a_card.rs`), and its
    // page box where the browser process computes it.
    assert_eq!(drawn.at("card"), vec![px(80.0, 98.0, 880.0, 568.0)]);
    assert_eq!(
        drawn.at("page"),
        vec![units(card_place::page_box(viewport(), 0.0))]
    );
    // How far it reaches: the search and the gap under it, then the newest.
    assert_eq!(
        drawn.reach,
        Some(Reach {
            earlier: SEARCHED + BETWEEN,
            newest: LIVE,
        })
    );
    Ok(())
}

#[test]
fn three_turns_stand_oldest_first_and_the_newest_is_at_the_rooms_top() -> Result<(), String> {
    let drawn = draw(conversation(three()?, 0.0));
    let failed = 40.0 - BETWEEN - FAILED;
    let searched = failed - BETWEEN - SEARCHED;
    assert_eq!(
        drawn.turns(),
        vec![
            px(80.0, searched, 880.0, SEARCHED),
            px(80.0, failed, 880.0, FAILED),
            px(80.0, 40.0, 880.0, LIVE),
        ]
    );
    assert_eq!(
        drawn.at("earlier"),
        vec![px(80.0, searched, 880.0, SEARCHED + BETWEEN + FAILED)]
    );
    // The failure, one line of 23 under what was said.
    assert_eq!(
        drawn.at("failed"),
        vec![px(80.0, failed + 42.0 + 16.0, 880.0, 23.0)]
    );
    assert_eq!(
        drawn.reach,
        Some(Reach {
            earlier: SEARCHED + BETWEEN + FAILED + BETWEEN,
            newest: LIVE,
        })
    );
    // Nothing of the earlier turns is drawn above the room: the failure's
    // line, at -36 to -13, is clipped at 40, and so the paper shows there.
    let frame = drawn.frame.as_ref().ok_or("nothing was painted")?;
    assert_eq!(pixel(frame, 100, 20), PAPER);
    Ok(())
}

#[test]
fn scrolled_back_every_box_moves_by_exactly_as_much_and_the_clip_holds() -> Result<(), String> {
    let still = draw(conversation(three()?, 0.0));
    // Back up until the first turn's top is at the room's top: as far as
    // the conversation reaches.
    let back = -(SEARCHED + BETWEEN + FAILED + BETWEEN);
    let (least, _) = conversation_scroll::bounds(still.reach.unwrap_or_default(), viewport());
    assert_eq!(least.to_bits(), back.to_bits());
    let scrolled = draw(conversation(three()?, back));
    let moved = |boxes: Vec<String>, by: f32| -> Vec<String> {
        boxes
            .into_iter()
            .map(|written| {
                let inner = written.trim_start_matches('(');
                let (left, rest) = inner.split_once(", ").unwrap_or_default();
                let (top, size) = rest.split_once(") ").unwrap_or_default();
                let left: f32 = left.parse().unwrap_or(f32::NAN);
                let top: f32 = top.parse().unwrap_or(f32::NAN);
                let (width, height) = size.split_once('×').unwrap_or_default();
                px(
                    left,
                    top + by,
                    width.parse().unwrap_or(f32::NAN),
                    height.parse().unwrap_or(f32::NAN),
                )
            })
            .collect()
    };
    for class in [
        "you", "results", "found", "title", "failed", "card", "page", "bar",
    ] {
        assert_eq!(
            scrolled.at(class),
            moved(still.at(class), -back),
            "{class} moved by exactly {}",
            -back
        );
    }
    // The first turn now at the room's top, and the rest under it.
    assert_eq!(
        scrolled.turns(),
        vec![
            px(80.0, 40.0, 880.0, SEARCHED),
            px(80.0, 40.0 + SEARCHED + BETWEEN, 880.0, FAILED),
            px(80.0, 40.0 - back, 880.0, LIVE),
        ]
    );
    // What scrolls is clipped to the room: the thread, the box and the
    // offers never move.
    for class in ["thread", "box"] {
        assert_eq!(scrolled.at(class), still.at(class), "{class}");
    }
    // The card's page box is where the browser process computes it, and
    // the part of it seen is what the room lets through.
    assert_eq!(
        scrolled.at("page"),
        vec![units(card_place::page_box(viewport(), back))]
    );
    assert_eq!(card_place::seen_page_box(viewport(), back, 0), None);
    let frame = scrolled.frame.as_ref().ok_or("nothing was painted")?;
    // The card's turn starts at 40 + 602 = 642: what was said, at the
    // column's right, then the card at 700, which would run on under the
    // box. Between the room's foot, 716, and the box, 740, the paper; the
    // box white; under the box, the paper again.
    assert_ne!(pixel(frame, 900, 660), PAPER, "what was said, in the room");
    assert_eq!(
        pixel(frame, 500, 728),
        PAPER,
        "the card clipped above the box"
    );
    assert_eq!(pixel(frame, 500, 768), WHITE, "the box, whole");
    assert_eq!(pixel(frame, 500, 810), PAPER, "nothing under the box");
    assert_eq!(holds("conversation-scrolled-back.png", Some(frame)), Ok(()));
    Ok(())
}

#[test]
fn scrolled_on_the_card_moves_up_and_its_page_box_follows() -> Result<(), String> {
    for down in [120.0, -200.0, 37.5] {
        let drawn = draw(conversation(two()?, down));
        let page = card_place::page_box(viewport(), down);
        assert_eq!(drawn.at("page"), vec![units(page)], "at {down}");
        assert_eq!(
            drawn.at("card"),
            vec![px(80.0, 98.0 - down, 880.0, 568.0)],
            "at {down}"
        );
    }
    // Scrolled 120 on, the page box from 25: the 15 above the room's top
    // clipped and the paper there; under it, the page's own picture.
    let drawn = draw(conversation(two()?, 120.0));
    let frame = drawn.frame.as_ref().ok_or("nothing was painted")?;
    assert_eq!(pixel(frame, 500, 30), PAPER, "above the room");
    assert_ne!(pixel(frame, 500, 45), PAPER, "the page, in the room");
    Ok(())
}

#[test]
fn a_press_on_an_earlier_turns_title_or_page_is_claimed_by_its_place_scrolled() {
    let turns = vec![searched(), failed(), visited()];
    // The newest is the page shown earlier, at 40; scrolled back until the
    // search's top is at 40: the titles where the results test puts them.
    let back = -(SEARCHED + BETWEEN + FAILED + BETWEEN);
    let pressed = |down: f32, x: f32, y: f32| {
        let mut renderer = Renderer::new(fonts());
        let _ = renderer.handle(ToRenderer::Start(Box::new(conversation(
            turns.clone(),
            down,
        ))));
        let _ = renderer.handle(ToRenderer::Paint);
        renderer.handle(ToRenderer::ConversationPress(Point::new(x, y)))
    };
    let claimed = |claim| FromRenderer::ConversationClaimed(Some(claim));
    let nothing = FromRenderer::ConversationClaimed(None);
    assert_eq!(pressed(back, 200.0, 202.5), claimed(Claim::Found { at: 0 }));
    assert_eq!(pressed(back, 200.0, 440.5), claimed(Claim::Found { at: 2 }));
    // Unscrolled, the same point is above the newest turn's top: on the
    // search's clipped part, which claims nothing, since it is not seen.
    assert_eq!(pressed(0.0, 200.0, 20.0), nothing, "above the room");
    // The page shown earlier, at 40 + 42 + 16 = 98 unscrolled, 44 tall: the
    // fourth link drawn.
    assert_eq!(pressed(0.0, 150.0, 120.0), claimed(Claim::Found { at: 3 }));
    // Scrolled back, the page shown earlier is under the room's foot.
    assert_eq!(
        pressed(back, 150.0, 120.0 - back),
        nothing,
        "under the room"
    );
    // The failure's words claim nothing.
    assert_eq!(
        pressed(back, 200.0, 40.0 + SEARCHED + BETWEEN + 69.0),
        nothing
    );
}

#[test]
fn an_agent_reads_every_turn_as_a_region_named_by_what_was_said() {
    let drawn = draw(conversation(vec![searched(), failed(), visited()], 0.0));
    let outline = drawn.tree.to_outline();
    let some = |name: &str| Some(name.to_owned());
    assert_eq!(
        names(&read(&drawn.tree, "region")),
        vec![
            some("Conversation"),
            some(SAID),
            some("Search results"),
            some("reddit.com"),
            some("Installing a charger at home: what to expect"),
        ],
        "{outline}"
    );
    // The titles and the page shown earlier, each a link, named by its
    // words: what a press on each opens.
    assert_eq!(
        names(&read(&drawn.tree, "link")),
        vec![
            some("Installing a charger at home: what to expect"),
            some("Home EV chargers compared: features that matter"),
            some("Chargers near you"),
            some("evhomeguide.org/installing-a-charger"),
        ],
        "{outline}"
    );
    // Why the address was not opened, in words.
    assert!(
        outline.contains("could not be opened: Chrome said nothing for 30000 ms"),
        "{outline}"
    );
    // Oldest first, then the box.
    let first = outline.find(SAID).unwrap_or(usize::MAX);
    let last = outline.find("evhomeguide.org/installing").unwrap_or(0);
    assert!(
        first < last && last < outline.find("search").unwrap_or(0),
        "{outline}"
    );
}

#[test]
fn every_turns_words_stay_text_through_the_drawing() {
    let hostile = "<b>bold</b></p><script>x()</script>";
    let drawn = draw(conversation(
        vec![
            Turn::Searched(Results::of(hostile.to_owned(), Showing::Found(Vec::new()))),
            Turn::Failed {
                said: hostile.to_owned(),
                why: hostile.to_owned(),
            },
            Turn::Visited {
                said: hostile.to_owned(),
                url: url("https://example.com/%3Cb%3E"),
            },
        ],
        0.0,
    ));
    let outline = drawn.tree.to_outline();
    assert!(!outline.contains("script\""), "{outline}");
    // Each turn named by those characters, and the failure's words them.
    let named = read(&drawn.tree, "region")
        .into_iter()
        .filter(|region| region.name.as_deref() == Some(hostile))
        .count();
    assert_eq!(named, 3, "{outline}");
    assert!(
        outline.contains(&format!("text \"{hostile}\"")),
        "{outline}"
    );
}

#[test]
fn a_conversation_is_drawn_as_the_references_unscrolled_and_at_two() -> Result<(), String> {
    let drawn = draw(conversation(three()?, 0.0));
    assert_eq!(
        holds("conversation-of-three-turns.png", drawn.frame.as_ref()),
        Ok(())
    );
    // A page shown earlier, as its address on a white line: scrolled back
    // until its turn, 42 + 16 + 44 = 102 tall, is at the room's top.
    let earlier = draw(conversation(vec![searched(), visited(), failed()], -134.0));
    assert_eq!(earlier.at("visit"), vec![px(80.0, 98.0, 880.0, 44.0)]);
    assert_eq!(
        holds(
            "conversation-a-page-shown-earlier.png",
            earlier.frame.as_ref()
        ),
        Ok(())
    );
    // At two, scrolled back to the failure: every pixel twice as many.
    let back = -(FAILED + BETWEEN);
    let at_two = draw(conversation(three()?, back).at_scale(2.0));
    let frame = at_two.frame.as_ref().ok_or("nothing was painted")?;
    assert_eq!((frame.width, frame.height), (2080, 1640));
    // The layout is in CSS pixels whatever the scale: the failure's turn at
    // the room's top.
    assert_eq!(at_two.turns().get(1), Some(&px(80.0, 40.0, 880.0, FAILED)));
    // In device pixels: above the room the paper, the box white at twice
    // its place, and the gap over it the paper.
    assert_eq!(pixel(frame, 1000, 40), PAPER, "above the room");
    assert_eq!(pixel(frame, 1000, 1536), WHITE, "the box");
    assert_eq!(pixel(frame, 1000, 1456), PAPER, "the gap over the box");
    assert_eq!(
        holds("conversation-scrolled-at-two.png", Some(frame)),
        Ok(())
    );
    Ok(())
}

/// The renderer binary, confined as every renderer is, with the corpus's
/// fonts.
fn renderers() -> Renderers {
    let faces = faces()
        .into_iter()
        .filter_map(|(family, weight, bytes)| {
            Face::new(family, weight, Slant::Normal, bytes.to_vec())
        })
        .collect();
    let generics = Generics::stating(
        generics()
            .into_iter()
            .map(|(generic, family)| (generic.to_owned(), family.to_owned()))
            .collect(),
    );
    Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[])
        .with_fonts([faces, alo_renderer::typefaces::faces()].concat())
        .with_generics(generics)
}

#[test]
fn the_conversation_is_drawn_and_reached_the_same_by_a_confined_renderer() -> Result<(), String> {
    let mut renderers = renderers();
    let site = start::site();
    let screen = conversation(three()?, -100.0);
    let loaded = renderers.ask(&site, &ToRenderer::Start(Box::new(screen.clone())));
    assert!(
        matches!(&loaded, Ok(FromRenderer::Loaded { issues, .. }) if issues.is_empty()),
        "{loaded:?}"
    );
    let here = draw(screen);
    let painted = renderers.ask(&site, &ToRenderer::Paint);
    assert_eq!(
        painted.as_ref().ok(),
        here.frame.map(FromRenderer::Painted).as_ref(),
        "the confined renderer drew another conversation"
    );
    let reached = renderers.ask(&site, &ToRenderer::ConversationReach);
    assert_eq!(
        reached.ok(),
        here.reach.map(FromRenderer::ConversationReached)
    );
    // A renderer holding the panel holds no conversation to measure.
    let panel = alo_renderer::panel::site();
    let _ = renderers.ask(
        &panel,
        &ToRenderer::Panel(Box::new(alo_renderer::panel::Panel {
            chats: Vec::new(),
            viewport: Size::new(240.0, 820.0),
            ..alo_renderer::panel::Panel::default()
        })),
    );
    assert_eq!(
        renderers.ask(&panel, &ToRenderer::ConversationReach).ok(),
        Some(FromRenderer::Failed(
            alo_renderer::message::Failure::NotTheConversation
        ))
    );
    renderers.stop_everything();
    Ok(())
}
