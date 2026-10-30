/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 700: the first screen is a document, rendered by the engine
//! (ADR 0074), as the chats panel is.
//!
//! What closes it, each a test below: a reference render of a first screen
//! with a person's sites and chats waiting on them; a layout assertion of the
//! one box, its field and button, every tile and every waiting chat; the
//! agent reading the greeting, the box by its name, *Your sites* and *Pick up
//! where you left off* with each thing in them by name; a hostile name
//! staying text; and the same screen through the real, confined renderer.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test the_first_screen`
//! rewrites the references. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::from_png;
use alo_renderer::face::Face;
use alo_renderer::frame::frame_to_png;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::start::{self, KeptSite, Start, Waiting};
use alo_renderer::start_document::{ADD_A_SITE, WAITING, YOUR_SITES};
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

/// The design's screen 0a: four sites, two chats waiting.
fn screen_0a() -> Start {
    Start {
        greeting: "Good morning.".to_owned(),
        sites: vec![
            KeptSite::new("Mail", "https://mail.example/"),
            KeptSite::new("Calendar", "https://calendar.example/"),
            KeptSite::new("Bank", "https://bank.example/"),
            KeptSite::new("", "evhomeguide.org"),
        ],
        waiting: vec![
            Waiting::new(
                "Weekend in Lisbon",
                "found 3 hotels · waiting for you to choose",
            ),
            Waiting::new("Invoice for March", "draft ready to check"),
        ],
        viewport: Size::new(WIDTH, HEIGHT),
        scale: 1.0,
    }
}

/// What a renderer holding a first screen answers to a paint and a reading.
struct Drawn {
    answer: FromRenderer,
    frame: Option<Frame>,
    tree: Snapshot,
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
    Drawn {
        answer,
        frame,
        tree,
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

#[test]
fn the_first_screen_is_read_as_a_greeting_a_box_sites_and_chats_waiting() {
    let drawn = draw(screen_0a());
    assert!(
        matches!(&drawn.answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{:?}",
        drawn.answer
    );
    let outline = drawn.tree.to_outline();
    let some = |name: &str| Some(name.to_owned());

    assert_eq!(
        names(&read(&drawn.tree, "heading")),
        vec![some("Good morning."), some(YOUR_SITES), some(WAITING),],
        "{outline}"
    );
    assert_eq!(read(&drawn.tree, "search").len(), 1, "{outline}");
    assert_eq!(
        names(&read(&drawn.tree, "textbox")),
        vec![some("Ask anything, search, or type a website")],
        "{outline}"
    );
    assert_eq!(
        names(&read(&drawn.tree, "button")),
        vec![some("Go")],
        "{outline}"
    );
    assert_eq!(
        names(&read(&drawn.tree, "navigation")),
        vec![some(YOUR_SITES)],
        "{outline}"
    );
    assert_eq!(
        names(&read(&drawn.tree, "link")),
        vec![
            some("Mail"),
            some("Calendar"),
            some("Bank"),
            some("evhomeguide.org"),
            some(ADD_A_SITE),
            some("Weekend in Lisbon found 3 hotels · waiting for you to choose"),
            some("Invoice for March draft ready to check"),
        ],
        "{outline}"
    );

    // In numbers. The greeting's 46 starts 150 down; 10, the line's 20 and
    // 28 below it the box: 720 wide, centred in 1040, so 160 in, 254 down,
    // 64 tall. Inside its 1 of border, the button 44 square against the
    // right's 10 of padding and centred in 62; the field 22 in, 40 tall,
    // and as wide as the 10 of gap before the button leaves it.
    let rect = |role: &str| read(&drawn.tree, role).first().map(|node| units(node.rect));
    assert_eq!(
        rect("search"),
        Some(px(160.0, 254.0, 720.0, 64.0)),
        "{outline}"
    );
    assert_eq!(
        rect("button"),
        Some(px(825.0, 264.0, 44.0, 44.0)),
        "{outline}"
    );
    assert_eq!(
        rect("textbox"),
        Some(px(183.0, 266.0, 632.0, 40.0)),
        "{outline}"
    );

    // Below the box by 40: the sites' label of 16 and 12 under it, then six
    // tiles to the row, (720 - 5 × 12) / 6 = 110 wide and 84 tall. The
    // waiting chats 28 below that group, under their label likewise, two to
    // the row, (720 - 12) / 2 = 354 wide, 14 + 18 + 2 + 18 + 14 and their
    // border, 68 tall.
    let links: Vec<String> = read(&drawn.tree, "link")
        .iter()
        .map(|link| units(link.rect))
        .collect();
    assert_eq!(
        links,
        vec![
            px(160.0, 386.0, 110.0, 84.0),
            px(282.0, 386.0, 110.0, 84.0),
            px(404.0, 386.0, 110.0, 84.0),
            px(526.0, 386.0, 110.0, 84.0),
            px(648.0, 386.0, 110.0, 84.0),
            px(160.0, 526.0, 354.0, 68.0),
            px(526.0, 526.0, 354.0, 68.0),
        ],
        "{outline}"
    );
    assert_eq!(holds("start-screen.png", drawn.frame.as_ref()), Ok(()));
}

#[test]
fn a_first_screen_with_nothing_kept_offers_only_to_add_a_site() {
    let drawn = draw(Start {
        greeting: "Good evening.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        ..Start::default()
    });
    let links: Vec<Option<String>> = names(&read(&drawn.tree, "link"));
    assert_eq!(links, vec![Some(ADD_A_SITE.to_owned())]);
    let frame = drawn.frame.unwrap_or_else(|| panic!("nothing was painted"));
    // The paper, #f4f1ec, where nothing is drawn.
    assert_eq!(pixel(&frame, 4, 4), Some([0xf4, 0xf1, 0xec, 0xff]));
    assert_eq!(pixel(&frame, 520, 700), Some([0xf4, 0xf1, 0xec, 0xff]));
}

#[test]
fn a_hostile_name_stays_text_in_the_tree() {
    let hostile = "</h1><script>steal()</script>\u{202e}";
    let drawn = draw(Start {
        greeting: hostile.to_owned(),
        sites: vec![KeptSite::new(hostile, "a.example")],
        waiting: vec![Waiting::new(hostile, "ok")],
        viewport: Size::new(WIDTH, HEIGHT),
        scale: 1.0,
    });
    assert!(matches!(drawn.answer, FromRenderer::Loaded { .. }));
    let headings = names(&read(&drawn.tree, "heading"));
    assert_eq!(
        headings.first().cloned().flatten().as_deref(),
        Some(hostile)
    );
    let links = names(&read(&drawn.tree, "link"));
    assert_eq!(links.first().cloned().flatten().as_deref(), Some(hostile));
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
fn the_first_screen_is_drawn_the_same_by_a_confined_renderer_at_its_own_site() {
    let mut renderers = renderers();
    let site = start::site();
    let screen = screen_0a();
    let loaded = renderers.ask(&site, &ToRenderer::Start(Box::new(screen.clone())));
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    let here = draw(screen);
    let painted = renderers.ask(&site, &ToRenderer::Paint);
    assert_eq!(
        painted.as_ref().ok(),
        here.frame.map(FromRenderer::Painted).as_ref(),
        "the confined renderer drew another first screen"
    );
    let read = renderers.ask(&site, &ToRenderer::ReadTree);
    let Ok(FromRenderer::Tree(tree)) = read else {
        panic!("not a tree: {read:?}");
    };
    assert_eq!(tree.to_outline(), here.tree.to_outline());
    renderers.stop_everything();
}
