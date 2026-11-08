/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 755 (ADR 0093 § 2): a sign that the conversation goes on
//! above or below the room — a bar in the room's right gutter, as long
//! against the room as the room is against the whole conversation and as
//! far down it as it is scrolled, placed by the browser process's numbers.
//!
//! Each a test below: the bar's box in numbers at three offsets, at one and
//! two; its colour where it is and not past it; an agent reading which way
//! the conversation goes on and how far it is scrolled; a press on it
//! claiming nothing; none drawn when none is sent; and reference renders
//! at one and at two.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_sign_that_the_conversation_goes_on`
//! rewrites the references. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::card::Card;
use alo_renderer::conversation_scroll::{self, Reach};
use alo_renderer::frame::frame_to_png;
use alo_renderer::results::{Found, Results, SetUp, Showing};
use alo_renderer::start::Start;
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
        FromRenderer::ConversationReached(reach, _) => Some(reach),
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

/// The three turns of `a_conversation_of_turns.rs`: a search 457 tall, a
/// failure 81 and a live card 626, 32 apart — reaching 602 above the
/// newest turn's top, and 626 below it; in the room of 676 it scrolls from
/// −602 to 0.
fn three() -> Result<Vec<Turn>, String> {
    Ok(vec![searched(), failed(), live()?])
}

const REACH: Reach = Reach {
    earlier: 602.0,
    newest: 626.0,
};

/// The conversation scrolled `down`, at `scale`, with the bar the browser
/// process places for it.
fn scrolled(down: f32, scale: f32) -> Result<Start, String> {
    Ok(Start {
        bar: conversation_scroll::bar(down, REACH, viewport(), scale),
        ..conversation(three()?, down).at_scale(scale)
    })
}

/// The bar's colour, `--bar` in the sheet.
const BAR: Option<[u8; 4]> = Some([0xcb, 0xc3, 0xb6, 0xff]);

#[test]
fn the_bar_stands_in_the_gutter_as_long_as_the_room_is_against_the_whole() -> Result<(), String> {
    // The clip is 944 wide from 48: its right edge at 992, the bar 13 in
    // from it and 6 wide, at 973. 676 × 676 / (676 + 602) is 357.57 long,
    // and it travels the 318.43 left: on a device pixel at one, 358 long,
    // at 318 at the newest turn's top, 159 half way back, 0 at the first.
    for (down, top) in [(0.0, 318.0), (-301.0, 159.0), (-602.0, 0.0)] {
        let drawn = draw(scrolled(down, 1.0)?);
        assert_eq!(drawn.at("thread"), vec![px(48.0, 40.0, 944.0, 676.0)]);
        assert_eq!(
            drawn.at("goes-on"),
            vec![px(973.0, 40.0 + top, 6.0, 358.0)],
            "scrolled {down}"
        );
        // Its reach is the turns', whatever the bar.
        assert_eq!(drawn.reach, Some(REACH));
    }
    // At two, on a device pixel of two: 357.5 long, at 318.5.
    let at_two = draw(scrolled(0.0, 2.0)?);
    assert_eq!(at_two.at("goes-on"), vec![px(973.0, 358.5, 6.0, 357.5)]);
    Ok(())
}

#[test]
fn the_bar_is_drawn_where_it_stands_and_nowhere_else() -> Result<(), String> {
    let drawn = draw(scrolled(-301.0, 1.0)?);
    let frame = drawn.frame.as_ref().ok_or("nothing was painted")?;
    // From 199 to 557 down, 973 to 979 across.
    assert_eq!(pixel(frame, 976, 378), BAR, "its middle");
    assert_eq!(pixel(frame, 973, 378), BAR, "its left edge");
    assert_eq!(pixel(frame, 978, 378), BAR, "its right edge");
    assert_ne!(pixel(frame, 976, 194), BAR, "above it");
    assert_ne!(pixel(frame, 976, 562), BAR, "below it");
    assert_ne!(pixel(frame, 970, 378), BAR, "before it");
    assert_ne!(pixel(frame, 982, 378), BAR, "after it");
    assert_eq!(
        holds("conversation-goes-on-above-and-below.png", Some(frame)),
        Ok(())
    );
    let at_two = draw(scrolled(0.0, 2.0)?);
    let frame = at_two.frame.as_ref().ok_or("nothing was painted")?;
    assert_eq!((frame.width, frame.height), (2080, 1640));
    // At two, 1946 to 1958 across, 717 to 1432 down.
    assert_eq!(pixel(frame, 1952, 1000), BAR);
    assert_ne!(pixel(frame, 1952, 712), BAR, "above it");
    assert_eq!(
        holds("conversation-goes-on-at-two.png", Some(frame)),
        Ok(())
    );
    Ok(())
}

#[test]
fn an_agent_reads_which_way_it_goes_on_and_how_far_it_is_scrolled() -> Result<(), String> {
    for (down, said) in [
        (
            0.0,
            "The conversation goes on above; scrolled 100% of the way down",
        ),
        (
            -301.0,
            "The conversation goes on above and below; scrolled 50% of the way down",
        ),
        (
            -602.0,
            "The conversation goes on below; scrolled 0% of the way down",
        ),
    ] {
        let drawn = draw(scrolled(down, 1.0)?);
        let outline = drawn.tree.to_outline();
        // The card's picture of its page is an image too, read as before.
        assert_eq!(
            names(&read(&drawn.tree, "image")),
            vec![
                Some("The page, as Chrome last drew it".to_owned()),
                Some(said.to_owned())
            ],
            "{outline}"
        );
    }
    // None sent, none drawn and none read.
    let none = draw(conversation(three()?, 0.0));
    assert!(none.at("goes-on").is_empty());
    assert_eq!(
        read(&none.tree, "image").len(),
        1,
        "the card's picture alone"
    );
    Ok(())
}

#[test]
fn a_press_on_the_bar_claims_nothing() -> Result<(), String> {
    let mut drawn = draw(scrolled(-301.0, 1.0)?);
    assert_eq!(
        drawn
            .renderer
            .handle(ToRenderer::ConversationPress(Point::new(976.0, 378.0))),
        FromRenderer::ConversationClaimed(None)
    );
    Ok(())
}

/// Queue item 793: the bar fades once the conversation has stopped. Drawn
/// at what the browser process says is seen of it — half way through its
/// fade, then gone — where it stood, and still read by an agent, which
/// reads what the interface is, not how faint it is.
#[test]
fn a_bar_fading_is_drawn_at_what_is_seen_of_it_and_still_read() -> Result<(), String> {
    let at = |seen: f32| -> Result<Drawn, String> {
        let mut start = scrolled(-301.0, 1.0)?;
        if let Some(bar) = start.bar.as_mut() {
            bar.seen = seen;
        }
        Ok(draw(start))
    };
    let paper = draw(conversation(three()?, -301.0));
    let paper_frame = paper.frame.as_ref().ok_or("nothing was painted")?;
    let under = pixel(paper_frame, 976, 378).ok_or("no paper")?;
    let half = at(0.5)?;
    let frame = half.frame.as_ref().ok_or("nothing was painted")?;
    // Where it stands, as when whole: 973 to 979, 199 to 557.
    assert_eq!(half.at("goes-on"), vec![px(973.0, 199.0, 6.0, 358.0)]);
    // Half the bar over the paper, each channel within a level of the
    // mean, the paper the conversation's own under it.
    let seen = pixel(frame, 976, 378).ok_or("no bar")?;
    let bar = BAR.ok_or("no colour")?;
    for channel in 0..3 {
        let mean = u16::midpoint(u16::from(bar[channel]), u16::from(under[channel]));
        assert!(
            u16::from(seen[channel]).abs_diff(mean) <= 1,
            "{seen:?} is not half {bar:?} over {under:?}"
        );
    }
    assert_eq!(
        holds("conversation-goes-on-fading.png", Some(frame)),
        Ok(())
    );
    // Faded out: the paper, and nothing else, where it stood.
    let gone = at(0.0)?;
    let frame = gone.frame.as_ref().ok_or("nothing was painted")?;
    assert_eq!(pixel(frame, 976, 378), Some(under));
    assert_eq!(pixel(frame, 973, 300), pixel(paper_frame, 973, 300));
    // Read either way, by which way it goes on and how far down it is.
    for drawn in [&half, &gone] {
        assert_eq!(
            names(&read(&drawn.tree, "image")).last(),
            Some(&Some(
                "The conversation goes on above and below; scrolled 50% of the way down".to_owned()
            )),
            "{}",
            drawn.tree.to_outline()
        );
    }
    Ok(())
}

/// Queue item 793: where the bar's left edge stands, from which the browser
/// process measures the pointer near it: the clip's right edge less 13 and
/// the bar's 6 — 973 at the design's width, and in a narrow window too.
#[test]
fn the_bars_left_edge_is_where_it_is_drawn() -> Result<(), String> {
    assert_eq!(
        format!("{}", conversation_scroll::bar_left(viewport())),
        "973"
    );
    let narrow = Size::new(695.0, HEIGHT);
    // A column of 631 from 32, the clip to 695, the bar 19 in from it.
    assert_eq!(format!("{}", conversation_scroll::bar_left(narrow)), "676");
    let drawn = draw(Start {
        viewport: narrow,
        bar: conversation_scroll::bar(-301.0, REACH, narrow, 1.0),
        ..conversation(three()?, -301.0)
    });
    let placed = drawn.at("goes-on");
    assert!(
        placed
            .first()
            .is_some_and(|rect| rect.starts_with("(676, ")),
        "{placed:?}"
    );
    Ok(())
}
