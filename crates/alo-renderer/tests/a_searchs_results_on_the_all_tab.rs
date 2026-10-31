/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 708 (ADR 0083): a search's results, shown in the conversation
//! as the design's All tab draws them — the owner's canvas of 31 October
//! 2026, `Links.dc.html` — under what the person said.
//!
//! What closes its engine half, each a test below: a layout assertion of
//! the tab row and the top sites, in numbers; a reference render of the All
//! tab with results, `references/all-tab-with-results.png`, and of a search
//! not answered, `references/all-tab-not-answered.png`; the agent reading
//! the results as a region with a tab list and articles; every string a
//! stranger's — a title holding `<b>` and `&amp;` stays those characters;
//! a press on each title claimed by its place and on anything else of the
//! results as nothing; and the same results through the real, confined
//! renderer.
//!
//! The sites here are written by hand for this test, never captured from
//! the provider.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_searchs_results_on_the_all_tab`
//! rewrites the references. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::conversation_claim::Claim;
use alo_renderer::face::Face;
use alo_renderer::frame::frame_to_png;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::results::{Found, Results, SetUp, Showing};
use alo_renderer::results_document::MORE_SITES;
use alo_renderer::start::{self, Start};
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
    database
}

/// The room the first screen has in the design's window: 1280 wide less the
/// panel's 240, and the window's 820 tall.
const WIDTH: f32 = 1040.0;
const HEIGHT: f32 = 820.0;

/// What a renderer holding a first screen answers to a paint and a reading.
struct Drawn {
    answer: FromRenderer,
    frame: Option<Frame>,
    tree: Snapshot,
    /// Where each icon was laid out, in document order. The agent does not
    /// read them — they are `aria-hidden` — so they are found in the
    /// drawing's own boxes, by the element each box is for.
    icons: Vec<String>,
}

fn draw(start: Start) -> Drawn {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Start(Box::new(start)));
    let frame = match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Some(frame),
        _ => None,
    };
    let tree = match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(tree) => *tree,
        _ => Snapshot::default(),
    };
    let icons = icons(&renderer);
    Drawn {
        answer,
        frame,
        tree,
        icons,
    }
}

/// The border box of every `<svg>` the renderer drew, in the order the
/// boxes were made, which is the document's.
fn icons(renderer: &Renderer) -> Vec<String> {
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
                .and_then(|made| made.kind.node())
                .and_then(|node| document.element(node))
                .is_some_and(|element| element.name.local.as_ref() == "svg")
        })
        .filter_map(|id| drawing.layout.get(id))
        .map(|geometry| units(geometry.border_box))
        .collect()
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

/// What a renderer holding `start` claims a press at `x`, `y` is on.
fn pressed(start: Start, x: f32, y: f32) -> FromRenderer {
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Start(Box::new(start)));
    let _ = renderer.handle(ToRenderer::Paint);
    renderer.handle(ToRenderer::ConversationPress(Point::new(x, y)))
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
        .with_fonts(faces)
        .with_generics(generics)
}

/// What the person searched for.
const SAID: &str = "home ev charger installation";

fn site(title: &str, address: &str, description: &str, name: &str) -> Found {
    Found::new(
        title,
        alo_url::parse(address).unwrap_or_else(|_| alo_url::Url::about_blank()),
        description,
    )
    .named(name)
}

/// The design's two sites, and two more, as a provider might answer them:
/// the fourth is not shown on All, which shows three.
fn four() -> Vec<Found> {
    vec![
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
            "Cable length, smart scheduling and app control, explained in plain terms, with \
             what to skip if you only drive short distances. A third sentence that runs \
             past the two lines a description is given, so that it is cut off where the \
             second line ends and nothing under it moves.",
            "",
        ),
        site(
            "<b>Fish &amp; chips</b> and chargers",
            "https://chips.example/",
            "",
            "Chips",
        ),
        site("Not shown", "https://four.example/", "On Sites.", ""),
    ]
}

fn with(showing: Showing) -> Start {
    Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        results: Some(Results {
            said: SAID.to_owned(),
            showing,
        }),
        search: SetUp::Ready,
        ..Start::default()
    }
}

fn found() -> Start {
    with(Showing::Found(four()))
}

#[test]
fn the_all_tab_is_laid_out_as_the_design_draws_it() {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Start(Box::new(found())));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{answer:?}"
    );
    let at = |class: &str| boxes_of(&renderer, class);
    // The column, 880 wide, centred in 1040, 40 down; the person's line at
    // its right, 42 tall; 20 under it the results. The thread clipping them
    // is 32 wider either side and reaches down to 24 above the box (queue
    // item 750).
    assert_eq!(at("thread searched"), vec![px(48.0, 40.0, 944.0, 676.0)]);
    assert!(
        at("you")
            .first()
            .is_some_and(|you| you.ends_with("×42") && you.contains(", 40)")),
        "{:?}",
        at("you")
    );
    assert_eq!(at("results"), vec![px(80.0, 102.0, 880.0, 395.0)]);
    // The tab row: 42 tall with its line, each tab 42 and 1 over the line,
    // so the chosen tab's 2 of ink stand on it; 4 between tabs.
    assert_eq!(at("rtabs"), vec![px(80.0, 102.0, 880.0, 42.0)]);
    assert_eq!(at("rt on"), vec![px(80.0, 102.0, 48.4375, 42.0)]);
    let tabs = at("rt");
    assert_eq!(tabs.len(), 5);
    assert!(
        tabs.first()
            .is_some_and(|answer| answer.starts_with("(132.4375, 102)")),
        "{tabs:?}"
    );
    // 20 under the row the sites, 4 inside their list; each site its line
    // of 18 and 4 under it, its title's 25 and 4 under it, and its two
    // lines of 23: 97 tall, 22 apart. A site with no description is 51.
    assert_eq!(at("res"), vec![px(80.0, 164.0, 880.0, 293.0)]);
    assert_eq!(
        at("found"),
        vec![
            px(80.0, 168.0, 880.0, 97.0),
            px(80.0, 287.0, 880.0, 97.0),
            px(80.0, 406.0, 880.0, 51.0),
        ],
        "three sites on All, the fourth not drawn"
    );
    assert_eq!(
        at("src"),
        vec![
            px(80.0, 168.0, 880.0, 18.0),
            px(80.0, 287.0, 880.0, 18.0),
            px(80.0, 406.0, 880.0, 18.0),
        ]
    );
    // The mark 18 square at the line's start; the name 8 after it.
    assert_eq!(
        at("fav tint0"),
        vec![px(80.0, 168.0, 18.0, 18.0), px(80.0, 287.0, 18.0, 18.0)]
    );
    assert_eq!(at("fav tint1"), vec![px(80.0, 406.0, 18.0, 18.0)]);
    assert!(
        at("name")
            .iter()
            .all(|name| name.starts_with("(106, ") && name.ends_with("×18")),
        "{:?}",
        at("name")
    );
    assert_eq!(
        at("t"),
        vec![
            px(80.0, 190.0, 880.0, 25.0),
            px(80.0, 309.0, 880.0, 25.0),
            px(80.0, 428.0, 880.0, 25.0),
        ]
    );
    // The description: 740 wide at most and two lines of 23, however long
    // it is.
    assert_eq!(
        at("d"),
        vec![px(80.0, 219.0, 740.0, 46.0), px(80.0, 338.0, 740.0, 46.0)]
    );
    // More sites, 20 under the last site; and the box, 880 × 56, 24 above
    // the window's foot: 820 - 24 - 56 = 740 (queue item 750).
    assert_eq!(at("more"), vec![px(80.0, 477.0, 93.125, 20.0)]);
    assert_eq!(at("box"), vec![px(80.0, 740.0, 880.0, 56.0)]);
}

/// Queue item 750: with results under the person's words, the box is
/// pinned 24 above the window's foot at the design's 56 — its top the
/// viewport's height less 24 less 56 — however tall the window, the results
/// where they were above it; and in a window too short for them, they are
/// clipped above the box, never drawn over or under it.
#[test]
fn with_results_the_box_is_pinned_24_above_the_windows_foot() {
    for height in [820.0, 1000.0, 520.0] {
        let mut start = found();
        start.viewport = Size::new(WIDTH, height);
        let mut renderer = Renderer::new(fonts());
        let _ = renderer.handle(ToRenderer::Start(Box::new(start)));
        let at = |class: &str| boxes_of(&renderer, class);
        let top = height - 24.0 - 56.0;
        assert_eq!(at("box"), vec![px(80.0, top, 880.0, 56.0)], "at {height}");
        assert_eq!(
            at("thread searched"),
            vec![px(48.0, 40.0, 944.0, top - 24.0 - 40.0)],
            "at {height}"
        );
        // The results keep their place and size, whatever room there is.
        assert_eq!(
            at("results"),
            vec![px(80.0, 102.0, 880.0, 395.0)],
            "at {height}"
        );
    }
    // At 520 the room ends at 416, so the third site's mark, 406 to 424, is
    // drawn above that edge and clipped under it; and the box, at 440, is
    // drawn whole.
    let mut start = found();
    start.viewport = Size::new(WIDTH, 520.0);
    let drawn = draw(start);
    let frame = drawn.frame.as_ref();
    assert!(frame.is_some(), "nothing was painted");
    if let Some(frame) = frame {
        let paper = Some([0xf4, 0xf1, 0xec, 0xff]);
        assert_ne!(pixel(frame, 85, 410), paper, "the mark above the edge");
        assert_eq!(pixel(frame, 85, 420), paper, "the mark under the edge");
        assert_eq!(
            pixel(frame, 500, 470),
            Some([0xff, 0xff, 0xff, 0xff]),
            "the box"
        );
    }
}

#[test]
fn the_all_tab_with_results_is_drawn_as_the_reference() {
    let drawn = draw(found());
    assert!(
        matches!(&drawn.answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{:?}",
        drawn.answer
    );
    // No icon of the results' own: Go's arrow is the one drawn.
    assert_eq!(drawn.icons.len(), 1, "{:?}", drawn.icons);
    let frame = drawn.frame.as_ref();
    assert!(frame.is_some(), "nothing was painted");
    if let Some(frame) = frame {
        // The chosen tab's underline in the ink, over the row's line.
        assert_eq!(pixel(frame, 100, 143), Some([0x1b, 0x24, 0x33, 0xff]));
        // A site's mark in its tint, and the paper between two sites.
        assert_eq!(pixel(frame, 81, 177), Some([0x2f, 0x6b, 0x5e, 0xff]));
        assert_eq!(pixel(frame, 900, 275), Some([0xf4, 0xf1, 0xec, 0xff]));
    }
    assert_eq!(holds("all-tab-with-results.png", frame), Ok(()));
}

#[test]
fn a_search_not_answered_says_why_where_the_sites_would_be() {
    let unanswered = with(Showing::Unanswered(
        "Brave Search says alo has searched too often, or its allowance is used up (429). \
         Try again later."
            .to_owned(),
    ));
    let drawn = draw(unanswered.clone());
    let outline = drawn.tree.to_outline();
    assert!(read(&drawn.tree, "article").is_empty(), "{outline}");
    assert!(
        read(&drawn.tree, "button")
            .iter()
            .all(|b| b.name.as_deref() != Some(MORE_SITES))
    );
    assert!(outline.contains("allowance is used up (429)"), "{outline}");
    assert_eq!(
        holds("all-tab-not-answered.png", drawn.frame.as_ref()),
        Ok(())
    );
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Start(Box::new(unanswered)));
    assert_eq!(
        boxes_of(&renderer, "note"),
        vec![px(80.0, 168.0, 880.0, 23.0)]
    );
    // Searching, and nothing found, are each one sentence too.
    for (showing, says) in [
        (
            Showing::Searching,
            "Searching Brave Search for \u{201c}home ev charger installation\u{201d}\u{2026}",
        ),
        (
            Showing::Found(Vec::new()),
            "Brave Search found no sites for \u{201c}home ev charger installation\u{201d}.",
        ),
    ] {
        let outline = draw(with(showing)).tree.to_outline();
        assert!(outline.contains(says), "{outline}");
        assert!(!outline.contains("article"), "{outline}");
    }
}

#[test]
fn an_agent_reads_the_results_as_a_region_of_tabs_and_articles() {
    let drawn = draw(found());
    let outline = drawn.tree.to_outline();
    let some = |name: &str| Some(name.to_owned());
    assert_eq!(
        names(&read(&drawn.tree, "region")),
        vec![some("Search results")],
        "{outline}"
    );
    assert_eq!(
        names(&read(&drawn.tree, "tablist")),
        vec![some("Kinds of result")]
    );
    let tabs = read(&drawn.tree, "tab");
    assert_eq!(
        names(&tabs),
        ["All", "Answer", "Sites", "Images", "Videos", "News"]
            .map(some)
            .to_vec()
    );
    let disabled: Vec<bool> = tabs.iter().map(|tab| tab.states.disabled).collect();
    assert_eq!(
        disabled,
        vec![false, true, true, true, true, true],
        "{outline}"
    );
    assert!(outline.contains("tab \"All\" [selected=true]"), "{outline}");
    assert_eq!(read(&drawn.tree, "article").len(), 3, "{outline}");
    assert_eq!(
        names(&read(&drawn.tree, "link")),
        vec![
            some("Installing a charger at home: what to expect"),
            some("Home EV chargers compared: features that matter"),
            some("<b>Fish &amp; chips</b> and chargers"),
        ],
        "a title is a link, and one holding markup is those characters"
    );
    let more: Vec<_> = read(&drawn.tree, "button")
        .into_iter()
        .filter(|button| button.name.as_deref() == Some(MORE_SITES))
        .collect();
    assert!(
        more.first().is_some_and(|more| more.states.disabled),
        "{outline}"
    );
    // No short answer, and nothing in its place (queue item 707).
    assert!(!outline.to_lowercase().contains("answer\" at"), "{outline}");
    assert!(
        read(&drawn.tree, "heading")
            .iter()
            .all(|heading| heading.name.as_deref() != Some("Short answer"))
    );
    // In order: what the person said, the results, then the box.
    assert!(
        outline.find("paragraph").unwrap_or(usize::MAX) < outline.find("region").unwrap_or(0)
            && outline.find("region").unwrap_or(usize::MAX) < outline.find("search").unwrap_or(0),
        "{outline}"
    );
}

#[test]
fn a_press_on_each_title_claims_it_by_place_and_on_the_rest_nothing() {
    let claimed = |claim| FromRenderer::ConversationClaimed(Some(claim));
    let nothing = FromRenderer::ConversationClaimed(None);
    // At the middle of each title the layout assertion pins.
    assert_eq!(
        pressed(found(), 200.0, 202.5),
        claimed(Claim::Found { at: 0 })
    );
    assert_eq!(
        pressed(found(), 200.0, 321.5),
        claimed(Claim::Found { at: 1 })
    );
    assert_eq!(
        pressed(found(), 200.0, 440.5),
        claimed(Claim::Found { at: 2 })
    );
    assert_eq!(pressed(found(), 150.0, 177.0), nothing, "a site's name");
    assert_eq!(pressed(found(), 300.0, 240.0), nothing, "a description");
    assert_eq!(pressed(found(), 800.0, 202.5), nothing, "past the title");
    assert_eq!(pressed(found(), 100.0, 123.0), nothing, "the All tab");
    assert_eq!(pressed(found(), 170.0, 123.0), nothing, "a tab not built");
    assert_eq!(pressed(found(), 120.0, 487.0), nothing, "More sites");
    assert_eq!(pressed(found(), 800.0, 60.0), nothing, "what was said");
}

#[test]
fn the_results_are_drawn_the_same_by_a_confined_renderer() -> Result<(), String> {
    let mut renderers = renderers();
    let site = start::site();
    let screen = found();
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
        "the confined renderer drew other results"
    );
    let read = renderers.ask(&site, &ToRenderer::ReadTree);
    let Ok(FromRenderer::Tree(tree)) = read else {
        return Err(format!("not a tree: {read:?}"));
    };
    assert_eq!(tree.to_outline(), here.tree.to_outline());
    renderers.stop_everything();
    Ok(())
}
