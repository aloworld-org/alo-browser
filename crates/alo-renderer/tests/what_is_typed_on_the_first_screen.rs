/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 701: what a person typed into the one box, shown on the first
//! screen as the field's value, and what is offered for it under the box —
//! the design's screen 0b (ADR 0074, `docs/design/shell.md`).
//!
//! What closes it here, each a test below: a reference render of the first
//! screen with `example.com` typed and its three offers; the offers' boxes
//! in numbers, and the groups moved down by them; the agent reading the
//! field's value and a `listbox` named *Suggestions* of `option`s, the first
//! selected and those not set up disabled; a question offered without *Go
//! to the site*; and a hostile typed string — `<b>`, `</script>`, a NUL,
//! thousands of characters — staying a value and text, the same through the
//! real, confined renderer.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test
//! what_is_typed_on_the_first_screen` rewrites the reference. A rewritten
//! reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::from_png;
use alo_renderer::conversation_claim::Claim;
use alo_renderer::face::Face;
use alo_renderer::frame::frame_to_png;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::start::{self, KeptSite, Start};
use alo_renderer::start_document::{
    ADD_A_SITE, GO_TO_THE_SITE, NOT_SET_UP, SUGGESTIONS, ask_about, search_for,
};
use alo_renderer::typed::{LONGEST_TYPED, offers};
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

/// The room the first screen has in the design's window.
const WIDTH: f32 = 1040.0;
const HEIGHT: f32 = 820.0;

/// The design's screen 0b: the first screen with `typed` in the box, which
/// has the keyboard, and what is offered for it; two kept sites below.
fn screen_0b(typed: &str) -> Start {
    Start {
        greeting: "Hello.".to_owned(),
        sites: vec![
            KeptSite::new("Mail", "https://mail.example/"),
            KeptSite::new("", "evhomeguide.org"),
        ],
        viewport: Size::new(WIDTH, HEIGHT),
        typed: typed.to_owned(),
        focused: true,
        offers: offers(typed),
        ..Start::default()
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

/// What the field holds, as the agent reads it: the text the field's
/// value is drawn as, read next after the field.
fn value(tree: &Snapshot) -> Option<String> {
    let nodes = tree.nodes();
    let field = nodes
        .iter()
        .position(|node| node.role.as_str() == "textbox")?;
    nodes
        .get(field + 1)
        .filter(|node| node.role.as_str() == "text")
        .and_then(|node| node.name.clone())
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

fn rects(nodes: &[&SnapshotNode]) -> Vec<String> {
    nodes.iter().map(|node| units(node.rect)).collect()
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
fn an_address_typed_is_offered_to_go_to_first_then_search_and_ask() {
    let drawn = draw(screen_0b("example.com"));
    assert!(
        matches!(&drawn.answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{:?}",
        drawn.answer
    );
    let outline = drawn.tree.to_outline();
    let some = |name: &str| Some(name.to_owned());

    // The field reads what was typed as its value, still named as it was.
    let fields = read(&drawn.tree, "textbox");
    assert_eq!(
        names(&fields),
        vec![some("Ask anything, search, or type a website")],
        "{outline}"
    );
    assert_eq!(value(&drawn.tree), some("example.com"), "{outline}");

    // The offers, as a list a person chooses from.
    let lists = read(&drawn.tree, "listbox");
    assert_eq!(names(&lists), vec![some(SUGGESTIONS)], "{outline}");
    let options = read(&drawn.tree, "option");
    assert_eq!(
        names(&options),
        vec![
            some(&format!("{GO_TO_THE_SITE} https://example.com/")),
            some(&format!("{} {NOT_SET_UP}", search_for("example.com"))),
            some(&format!("{} {NOT_SET_UP}", ask_about("example.com"))),
        ],
        "{outline}"
    );
    let selected: Vec<bool> = options
        .iter()
        .map(|option| option.states.selected == Some(true))
        .collect();
    assert_eq!(
        selected,
        vec![true, false, false],
        "Return chooses the first"
    );
    let disabled: Vec<bool> = options
        .iter()
        .map(|option| option.states.disabled)
        .collect();
    assert_eq!(disabled, vec![false, true, true], "{outline}");

    // In numbers. The box, as the first screen has it: 720 × 64 at (160,
    // 254), its foot at 318. The list 8 under it, 720 wide; inside its 1
    // of border and 6 of padding, each offer 706 wide and 7 + 20 + 18 + 7
    // = 52 tall, one under the next: 2 + 12 + 3 × 52 = 170 tall in all.
    assert_eq!(
        rects(&read(&drawn.tree, "search")),
        vec![px(160.0, 254.0, 720.0, 64.0)],
        "{outline}"
    );
    assert_eq!(
        rects(&lists),
        vec![px(160.0, 326.0, 720.0, 170.0)],
        "{outline}"
    );
    assert_eq!(
        rects(&options),
        vec![
            px(167.0, 333.0, 706.0, 52.0),
            px(167.0, 385.0, 706.0, 52.0),
            px(167.0, 437.0, 706.0, 52.0),
        ],
        "{outline}"
    );
    // The groups moved down by the list and its 8: *Your sites*' label at
    // 496 + 40, its tiles 28 under that.
    assert_eq!(
        rects(&read(&drawn.tree, "link")),
        vec![
            px(160.0, 564.0, 110.0, 84.0),
            px(282.0, 564.0, 110.0, 84.0),
            px(404.0, 564.0, 110.0, 84.0),
        ],
        "{outline}"
    );
    assert_eq!(
        names(&read(&drawn.tree, "link")),
        vec![some("Mail"), some("evhomeguide.org"), some(ADD_A_SITE)],
        "{outline}"
    );

    let frame = drawn.frame.as_ref();
    // The offer Return chooses is on the paper, #f4f1ec; the next on white;
    // the focused box's border is the quiet text, #5f5a52.
    assert_eq!(
        frame.and_then(|frame| pixel(frame, 600, 340)),
        Some([0xf4, 0xf1, 0xec, 0xff])
    );
    assert_eq!(
        frame.and_then(|frame| pixel(frame, 600, 392)),
        Some([0xff, 0xff, 0xff, 0xff])
    );
    assert_eq!(
        frame.and_then(|frame| pixel(frame, 500, 254)),
        Some([0x5f, 0x5a, 0x52, 0xff])
    );
    assert_eq!(holds("start-typed-into.png", frame), Ok(()));
}

#[test]
fn a_question_is_offered_to_search_and_ask_and_never_to_go() {
    let drawn = draw(screen_0b("what is x"));
    let outline = drawn.tree.to_outline();
    let options = read(&drawn.tree, "option");
    assert_eq!(
        names(&options),
        vec![
            Some(format!("{} {NOT_SET_UP}", search_for("what is x"))),
            Some(format!("{} {NOT_SET_UP}", ask_about("what is x"))),
        ],
        "{outline}"
    );
    assert_eq!(
        options
            .iter()
            .map(|option| option.states.selected == Some(true))
            .collect::<Vec<bool>>(),
        vec![true, false]
    );
    // Two offers: 2 + 12 + 2 × 52 = 118 tall.
    assert_eq!(
        rects(&read(&drawn.tree, "listbox")),
        vec![px(160.0, 326.0, 720.0, 118.0)],
        "{outline}"
    );
}

#[test]
fn nothing_typed_offers_nothing_and_the_screen_is_as_it_was() {
    let drawn = draw(screen_0b(""));
    let outline = drawn.tree.to_outline();
    assert!(read(&drawn.tree, "listbox").is_empty(), "{outline}");
    assert!(read(&drawn.tree, "option").is_empty(), "{outline}");
    // The tiles where the first screen has them with nothing typed.
    assert_eq!(
        rects(&read(&drawn.tree, "link")).first().cloned(),
        Some(px(160.0, 386.0, 110.0, 84.0)),
        "{outline}"
    );
}

#[test]
fn a_hostile_typed_string_stays_a_value_and_text() {
    let long = "<b>".repeat(LONGEST_TYPED / 3);
    for hostile in [
        "<b>bold</b>",
        "</script><script>steal()</script>",
        "a\u{0}b",
        long.as_str(),
    ] {
        let drawn = draw(screen_0b(hostile));
        assert!(
            matches!(&drawn.answer, FromRenderer::Loaded { .. }),
            "{:?}",
            drawn.answer
        );
        let outline = drawn.tree.to_outline();
        assert_eq!(
            value(&drawn.tree).as_deref(),
            Some(hostile),
            "{hostile:?}: {outline}"
        );
        assert_eq!(
            names(&read(&drawn.tree, "option"))
                .into_iter()
                .flatten()
                .collect::<Vec<String>>(),
            vec![
                format!("{} {NOT_SET_UP}", search_for(hostile)),
                format!("{} {NOT_SET_UP}", ask_about(hostile)),
            ],
            "{hostile:?}"
        );
        // Nothing was made of it: the same roles as for a plain question.
        let roles = |tree: &Snapshot| {
            tree.nodes()
                .into_iter()
                .map(|node| node.role.as_str().to_owned())
                .collect::<Vec<String>>()
        };
        assert_eq!(
            roles(&drawn.tree),
            roles(&draw(screen_0b("what is x")).tree),
            "{outline}"
        );
        // And the box and its offers did not move or grow for it.
        assert_eq!(
            rects(&read(&drawn.tree, "listbox")),
            vec![px(160.0, 326.0, 720.0, 118.0)],
            "{hostile:?}"
        );
        assert_eq!(
            rects(&read(&drawn.tree, "textbox")),
            vec![px(183.0, 266.0, 632.0, 40.0)],
            "{hostile:?}"
        );
        // One line, inside the field: below the list, at 444, and above
        // *Your sites*, at 484, is the paper and nothing typed.
        for (x, y) in [(200, 460), (520, 470), (700, 450)] {
            assert_eq!(
                drawn.frame.as_ref().and_then(|frame| pixel(frame, x, y)),
                Some([0xf4, 0xf1, 0xec, 0xff]),
                "{hostile:?} at ({x}, {y})"
            );
        }
    }
}

#[test]
fn a_press_on_the_box_with_the_keyboard_is_the_box_and_on_an_offer_is_nothing_yet() {
    let mut renderer = Renderer::new(fonts());
    let built = renderer.handle(ToRenderer::Start(Box::new(screen_0b("example.com"))));
    assert!(matches!(built, FromRenderer::Loaded { .. }), "{built:?}");
    for (x, y, claim) in [
        // The field holding what was typed, and the box around it.
        (230.0, 285.0, Some(Claim::TheBox)),
        (165.0, 260.0, Some(Claim::TheBox)),
        (846.0, 285.0, Some(Claim::Go)),
        // Each offer, and the card's edge: queue item 714.
        (300.0, 350.0, None),
        (300.0, 410.0, None),
        (163.0, 490.0, None),
    ] {
        assert_eq!(
            renderer.handle(ToRenderer::ConversationPress(alo_paint::Point::new(x, y))),
            FromRenderer::ConversationClaimed(claim),
            "({x}, {y})"
        );
    }
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
fn what_is_typed_is_drawn_the_same_by_a_confined_renderer() {
    let mut renderers = renderers();
    let site = start::site();
    for typed in ["example.com", "</script>\u{0}<b>"] {
        let screen = screen_0b(typed);
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
            "the confined renderer drew another first screen for {typed:?}"
        );
        let read = renderers.ask(&site, &ToRenderer::ReadTree);
        let Ok(FromRenderer::Tree(tree)) = read else {
            panic!("not a tree: {read:?}");
        };
        assert_eq!(tree.to_outline(), here.tree.to_outline());
    }
    renderers.stop_everything();
}
