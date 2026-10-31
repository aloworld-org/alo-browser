/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 717 (ADR 0076 B): a page opened from the conversation is shown
//! as a card in the first screen's document, drawn by this engine from
//! Chrome's last frame of the page.
//!
//! What closes its engine half, each a test below: a layout assertion of the
//! card — the person's line, the card, its bar, its four buttons, its
//! address and its page box — in numbers; a reference render of the card
//! holding a committed frame, `references/card-frame.png`, which Chrome
//! 156.0.8078.76 streamed of a page of this test's own at 878 × 520
//! (`Page.startScreencast`, PNG, at one) and which is the input, never
//! rewritten; the agent reading the card as a region named by its address,
//! its buttons by name, *Back* and *Forward* disabled; a press on each
//! button claimed as it, a greyed one and the picture claimed as nothing;
//! and the same card through the real, confined renderer.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_page_as_a_card`
//! rewrites the render references (not the frame). A rewritten reference is
//! a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::card::{Card, PAGE_SIZE};
use alo_renderer::card_document::PICTURED;
use alo_renderer::conversation_claim::Claim;
use alo_renderer::face::Face;
use alo_renderer::frame::frame_to_png;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
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

/// The frame Chrome streamed of the test's page, at one: the card's input.
fn frame() -> Result<Vec<u8>, String> {
    let path = reference("card-frame.png");
    std::fs::read(&path).map_err(|why| format!("{} is not there: {why}", path.display()))
}

/// The first screen with a card for `http://127.0.0.1:8765/card.html`,
/// answering what the person typed, holding `frame`.
fn with_a_card(frame: Option<Vec<u8>>) -> Start {
    let url = alo_url::parse("http://127.0.0.1:8765/card.html")
        .unwrap_or_else(|_| alo_url::Url::about_blank());
    let card = Card::new("127.0.0.1:8765/card.html", url);
    Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        card: Some(match frame {
            Some(png) => card.holding(png),
            None => card,
        }),
        ..Start::default()
    }
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

#[test]
fn the_card_is_laid_out_as_the_design_draws_it() -> Result<(), String> {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Start(Box::new(with_a_card(Some(frame()?)))));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{answer:?}"
    );
    let at = |class: &str| boxes_of(&renderer, class);
    // The column, 880 wide, centred in 1040, 40 down: the person's line at
    // its right, 10 + 22 + 10 = 42 tall; 16 under it the card, 880 wide and
    // its border, its 46 of bar and its 520 of page: 1 + 46 + 520 + 1 = 568.
    assert_eq!(at("thread"), vec![px(80.0, 40.0, 880.0, 626.0)]);
    assert_eq!(at("you").len(), 1);
    assert!(
        at("you")
            .first()
            .is_some_and(|you| you.starts_with("(736.34375, 40)") && you.ends_with("×42")),
        "{:?}",
        at("you")
    );
    assert_eq!(at("card"), vec![px(80.0, 98.0, 880.0, 568.0)]);
    // Inside the card's border: the bar 878 × 46, its line under it; then
    // the page, the size Chrome is asked to lay it out at.
    assert_eq!(at("bar"), vec![px(81.0, 99.0, 878.0, 46.0)]);
    assert_eq!(at("page"), vec![px(81.0, 145.0, 878.0, 520.0)]);
    assert_eq!(
        (PAGE_SIZE.0, PAGE_SIZE.1),
        (878, 520),
        "the page box is the size Chrome is asked for"
    );
    // The picture fills it, one CSS pixel to one of Chrome's at one.
    assert_eq!(at("shot"), vec![px(81.0, 145.0, 878.0, 520.0)]);
    // The bar's buttons, 36 square, 6 in from its sides and 2 apart, centred
    // in the 45 above its line: 99 + 4.5. The address takes what is left,
    // 8 after Forward, and is one line of 18, centred likewise.
    assert_eq!(at("ibtn back off"), vec![px(87.0, 103.5, 36.0, 36.0)]);
    assert_eq!(at("ibtn forward off"), vec![px(125.0, 103.5, 36.0, 36.0)]);
    assert_eq!(at("url"), vec![px(171.0, 112.5, 706.0, 18.0)]);
    assert_eq!(at("ibtn expand"), vec![px(879.0, 103.5, 36.0, 36.0)]);
    assert_eq!(at("ibtn close"), vec![px(917.0, 103.5, 36.0, 36.0)]);
    // Each icon 18 square in the middle of its 36: 9 in, 9 down.
    assert_eq!(
        icons(&renderer).get(..4).map(<[String]>::to_vec),
        Some(vec![
            px(96.0, 112.5, 18.0, 18.0),
            px(134.0, 112.5, 18.0, 18.0),
            px(888.0, 112.5, 18.0, 18.0),
            px(926.0, 112.5, 18.0, 18.0),
        ])
    );
    // And the one box, 24 under the column.
    assert_eq!(at("box"), vec![px(160.0, 690.0, 720.0, 64.0)]);
    Ok(())
}

#[test]
fn the_card_holding_chromes_frame_is_drawn_as_the_reference() -> Result<(), String> {
    let drawn = draw(with_a_card(Some(frame()?)));
    assert!(
        matches!(&drawn.answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "the frame was read as a picture: {:?}",
        drawn.answer
    );
    // The card's four icons and Go's arrow.
    assert_eq!(drawn.icons.len(), 5);
    let frame = drawn.frame.as_ref().ok_or("nothing was painted")?;
    // The picture is Chrome's pixels, one for one: the page's header,
    // #1d4ed8, 30 in and 30 down of the frame, is that colour at the page
    // box's corner plus 30.
    assert_eq!(
        pixel(frame, 81 + 30, 145 + 30),
        Some([0x1d, 0x4e, 0xd8, 0xff])
    );
    // Clipped by the card's rounded corners (a picture under a clip, queue
    // item 717): the frame is white to its bottom-left corner, and the
    // pixel at the page box's own corner, outside the card's curve, is not.
    assert_ne!(pixel(frame, 81, 664), Some([0xff, 0xff, 0xff, 0xff]));
    assert_eq!(pixel(frame, 100, 650), Some([0xff, 0xff, 0xff, 0xff]));
    assert_eq!(
        holds("card-holding-a-frame.png", drawn.frame.as_ref()),
        Ok(())
    );
    Ok(())
}

#[test]
fn a_card_before_its_first_frame_is_its_white_page() {
    let drawn = draw(with_a_card(None));
    assert!(read(&drawn.tree, "image").is_empty(), "no picture yet");
    let frame = drawn.frame.unwrap_or_else(|| panic!("nothing was painted"));
    assert_eq!(pixel(&frame, 500, 400), Some([0xff, 0xff, 0xff, 0xff]));
    assert_eq!(holds("card-before-a-frame.png", Some(&frame)), Ok(()));
}

#[test]
fn an_agent_reads_the_card_as_a_region_named_by_its_address() -> Result<(), String> {
    let drawn = draw(with_a_card(Some(frame()?)));
    let outline = drawn.tree.to_outline();
    let some = |name: &str| Some(name.to_owned());
    assert_eq!(
        names(&read(&drawn.tree, "region")),
        vec![some("127.0.0.1:8765/card.html")],
        "{outline}"
    );
    assert_eq!(
        names(&read(&drawn.tree, "button")),
        vec![
            some("Back"),
            some("Forward"),
            some("Expand to the whole window"),
            some("Close"),
            some("Go"),
        ],
        "{outline}"
    );
    let disabled: Vec<bool> = read(&drawn.tree, "button")
        .iter()
        .map(|button| button.states.disabled)
        .collect();
    assert_eq!(disabled, vec![true, true, false, false, false], "{outline}");
    assert_eq!(
        names(&read(&drawn.tree, "image")),
        vec![some(PICTURED)],
        "{outline}"
    );
    // The greeting and the groups have gone; the box stays under the card.
    assert!(read(&drawn.tree, "heading").is_empty(), "{outline}");
    assert_eq!(read(&drawn.tree, "search").len(), 1, "{outline}");
    // In order: what the person said, then the card, then the box.
    assert!(
        outline.find("paragraph").unwrap_or(usize::MAX) < outline.find("region").unwrap_or(0)
            && outline.find("region").unwrap_or(usize::MAX) < outline.find("search").unwrap_or(0),
        "{outline}"
    );
    Ok(())
}

#[test]
fn a_press_on_each_button_claims_it_and_on_a_greyed_one_or_the_page_nothing() -> Result<(), String>
{
    let claimed = |claim| FromRenderer::ConversationClaimed(Some(claim));
    let nothing = FromRenderer::ConversationClaimed(None);
    // At the middle of each box the layout assertion pins.
    let card = || with_a_card(Some(frame().unwrap_or_default()));
    assert_eq!(pressed(card(), 897.0, 121.5), claimed(Claim::Expand));
    assert_eq!(pressed(card(), 935.0, 121.5), claimed(Claim::CloseTheCard));
    assert_eq!(pressed(card(), 105.0, 121.5), nothing, "Back, greyed");
    assert_eq!(pressed(card(), 143.0, 121.5), nothing, "Forward, greyed");
    assert_eq!(pressed(card(), 500.0, 400.0), nothing, "the page is 718's");
    assert_eq!(pressed(card(), 500.0, 120.0), nothing, "the address");
    let mut open = with_a_card(Some(frame()?));
    if let Some(card) = open.card.as_mut() {
        card.back = true;
        card.forward = true;
    }
    assert_eq!(pressed(open.clone(), 105.0, 121.5), claimed(Claim::Back));
    assert_eq!(pressed(open, 143.0, 121.5), claimed(Claim::Forward));
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
        .with_fonts(faces)
        .with_generics(generics)
}

#[test]
fn the_card_is_drawn_the_same_by_a_confined_renderer_which_decodes_the_frame() -> Result<(), String>
{
    let mut renderers = renderers();
    let site = start::site();
    let screen = with_a_card(Some(frame()?));
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
        "the confined renderer drew another card"
    );
    let read = renderers.ask(&site, &ToRenderer::ReadTree);
    let Ok(FromRenderer::Tree(tree)) = read else {
        return Err(format!("not a tree: {read:?}"));
    };
    assert_eq!(tree.to_outline(), here.tree.to_outline());
    renderers.stop_everything();
    Ok(())
}
