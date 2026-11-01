/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 774: in a window too narrow for the design's 880 column and
//! its gutters, the conversation's column — the person's words, the tab
//! row, the results, a card, and the box at the window's foot — is the room
//! the panel or its rail leaves less 32 either side, centred there: never
//! under the panel, never past the window's right edge. Seen by hand in a
//! window 967 wide, where the 944 clip and the 880 box were centred in a
//! room of 727 and overflowed it by 108 either side.
//!
//! What closes its engine half, each a test below: a layout assertion, in
//! numbers, of the column, the tab row, the results and the box at the
//! rooms a window 967 and 720 wide leaves beside the panel (240) and beside
//! the rail (64); the card's page box, the box and the room held to what
//! the browser process computes (`card_place`) at the same rooms; the first
//! screen's 720 column likewise; and a reference render of the All tab at
//! 967, `references/narrow-all-tab-with-results.png`.
//!
//! Queue item 775 adds the widths the panel folds at and the least window:
//! the tab row inside the column at 760 beside the panel, at 759 beside the
//! rail, and at 600 × 600 beside the rail.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_conversation_in_a_narrow_window`
//! rewrites the reference. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::from_png;
use alo_renderer::card::Card;
use alo_renderer::card_place;
use alo_renderer::frame::frame_to_png;
use alo_renderer::results::{Found, Results, SetUp, Showing};
use alo_renderer::start::Start;
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::{Path, PathBuf};

/// The corpus's faces, as the other first-screen tests load them, and the
/// design's typefaces as the browser process hands them over (ADR 0084).
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

/// The window's height in every test here: the design's.
const HEIGHT: f32 = 820.0;

/// The rooms the first screen has, as `alo-window`'s `start_viewport_as`
/// gives them: a window 967 wide beside the panel's 240 and beside the
/// rail's 64, and one 720 wide beside each.
const ROOMS: [(&str, f32); 4] = [
    ("967 beside the panel", 727.0),
    ("967 beside the rail", 903.0),
    ("720 beside the panel", 480.0),
    ("720 beside the rail", 656.0),
];

/// A rectangle as the outline writes it, compared without comparing floats.
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

/// The border box of every element whose class is `class`, in document
/// order.
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

/// Every element of `class`'s border box, as numbers, in document order.
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
                .is_some_and(|node| {
                    document
                        .attribute_by_name(node, "class")
                        .is_some_and(|classes| classes.split(' ').any(|one| one == class))
                })
        })
        .filter_map(|id| drawing.layout.get(id))
        .map(|geometry| geometry.border_box)
        .collect()
}

fn site(title: &str, address: &str, description: &str, name: &str) -> Found {
    Found::new(
        title,
        alo_url::parse(address).unwrap_or_else(|_| alo_url::Url::about_blank()),
        description,
    )
    .named(name)
}

/// Three sites as a provider might answer a search, written for this test:
/// the person's words long enough that their line is cut at a narrow
/// column.
fn found(room: f32) -> Start {
    let sites = vec![
        site(
            "Understanding Ownership - The Rust Programming Language",
            "https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html",
            "Ownership is Rust's most unique feature and has deep implications for the rest \
             of the language. It enables Rust to make memory safety guarantees without \
             needing a garbage collector.",
            "The Rust Programming Language",
        ),
        site(
            "Ownership as explained in the Rust book",
            "https://www.reddit.com/r/rust/comments/yibdpi/ownership_as_explained/",
            "Because code written without ownership in mind results in huge hairball which \
             spews runtime errors left and right.",
            "Reddit",
        ),
        site(
            "What is Ownership? - The Rust Programming Language",
            "https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html",
            "Ownership is a set of rules that govern how a Rust program manages memory.",
            "The Rust Programming Language",
        ),
    ];
    Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(room, HEIGHT),
        turns: vec![Turn::Searched(Results::of(
            "rust ownership explained with examples for a beginner who knows python",
            Showing::Found(sites),
        ))],
        search: SetUp::Ready,
        ..Start::default()
    }
}

/// The first screen with a card for a page, no frame of it yet.
fn with_a_card(room: f32) -> Start {
    let url = alo_url::parse("http://127.0.0.1:8765/card.html")
        .unwrap_or_else(|_| alo_url::Url::about_blank());
    Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(room, HEIGHT),
        turns: vec![Turn::Page(Card::new("127.0.0.1:8765/card.html", url))],
        ..Start::default()
    }
}

fn laid_out(start: Start) -> Renderer {
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Start(Box::new(start)));
    renderer
}

/// Whether `inner` lies inside `outer`, edges included.
fn inside(inner: Rect, outer: Rect) -> bool {
    inner.left() >= outer.left()
        && inner.right() <= outer.right()
        && inner.top() >= outer.top()
        && inner.bottom() <= outer.bottom()
}

/// The person's line, one line of 22 and 10 above and below: as wide as
/// its words, or cut to the column.
const SAID_WIDE: f32 = 535.906_25;

#[test]
fn the_column_is_the_room_less_32_either_side_with_the_panel_or_the_rail() {
    // Each room: the clip the room's whole width, 40 down to 24 above the
    // box (820 - 24 - 56 - 24 - 40 = 676); the column 64 narrower from 32;
    // and the box as wide, 24 above the window's foot. The third site's
    // description is one line where the column holds it, and two at 416.
    for (said, room, column, third) in [
        ("967 beside the panel", 727.0, 663.0, 74.0),
        ("967 beside the rail", 903.0, 839.0, 74.0),
        ("720 beside the panel", 480.0, 416.0, 97.0),
        ("720 beside the rail", 656.0, 592.0, 74.0),
    ] {
        let renderer = laid_out(found(room));
        let at = |class: &str| boxes_of(&renderer, class);
        assert_eq!(at("thread"), vec![px(0.0, 40.0, room, 676.0)], "{said}");
        // The person's line at the column's right, cut to it at 416.
        let you = SAID_WIDE.min(column);
        assert_eq!(
            at("you"),
            vec![px(32.0 + column - you, 40.0, you, 42.0)],
            "{said}"
        );
        // 20 under it the results; the tab row as wide as the column, its
        // tabs where they are at the design's width.
        assert_eq!(
            at("results"),
            vec![px(32.0, 102.0, column, 395.0 + third - 51.0)],
            "{said}"
        );
        assert_eq!(at("rtabs"), vec![px(32.0, 102.0, column, 42.0)], "{said}");
        assert_eq!(at("rt on"), vec![px(32.0, 102.0, 45.546_875, 42.0)]);
        assert_eq!(
            at("rt"),
            vec![
                px(162.468_75, 102.0, 59.546_875, 42.0),
                px(304.906_25, 102.0, 72.5, 42.0),
                px(381.406_25, 102.0, 63.343_75, 42.0),
            ],
            "{said}: Sites, Videos and News, News ending at 444.75"
        );
        // The sites, 22 apart, each as wide as the column; a description
        // 740 at most.
        assert_eq!(
            at("found"),
            vec![
                px(32.0, 168.0, column, 97.0),
                px(32.0, 287.0, column, 97.0),
                px(32.0, 406.0, column, third),
            ],
            "{said}"
        );
        assert_eq!(
            at("t"),
            vec![
                px(32.0, 190.0, column, 25.0),
                px(32.0, 309.0, column, 25.0),
                px(32.0, 428.0, column, 25.0),
            ],
            "{said}"
        );
        let described = column.min(740.0);
        assert_eq!(
            at("d"),
            vec![
                px(32.0, 219.0, described, 46.0),
                px(32.0, 338.0, described, 46.0),
                px(32.0, 457.0, described, third - 51.0),
            ],
            "{said}"
        );
        assert_eq!(at("box"), vec![px(32.0, 740.0, column, 56.0)], "{said}");
        // And nothing of the results outside the column.
        let column_box = Rect::new(32.0, 0.0, column, HEIGHT);
        for class in ["found", "src", "t", "d", "rtabs", "rt", "more", "box"] {
            let rects = rects_of(&renderer, class);
            assert!(!rects.is_empty(), "{said}: no {class}");
            assert!(
                rects.iter().all(|rect| inside(*rect, column_box)),
                "{said}: a {class} outside the column: {rects:?}"
            );
        }
    }
}

#[test]
fn at_the_designs_room_and_one_just_wide_enough_the_column_is_880() {
    for (room, left) in [(1040.0, 80.0), (944.0, 32.0), (1216.0, 168.0)] {
        let renderer = laid_out(found(room));
        let at = |class: &str| boxes_of(&renderer, class);
        assert_eq!(
            at("thread"),
            vec![px(left - 32.0, 40.0, 944.0, 676.0)],
            "{room}"
        );
        assert_eq!(at("rtabs"), vec![px(left, 102.0, 880.0, 42.0)], "{room}");
        assert_eq!(at("box"), vec![px(left, 740.0, 880.0, 56.0)], "{room}");
    }
}

/// Queue item 775: the panel folds to its rail in a window narrower than
/// 760 (`alo-window`'s `conversation_fold`), and the window is never
/// narrower than 600 nor shorter than 600. At 760 beside the panel (a room
/// of 520), at 759 beside the rail it folds to (695), and in the least
/// window beside the rail (536 × 600), the column is the room less 32
/// either side and the tab row, *All* to *News*, is inside it.
#[test]
fn at_the_fold_and_in_the_least_window_the_tab_row_is_inside_the_column() {
    for (said, room, height, column) in [
        ("760 beside the panel", 520.0, HEIGHT, 456.0),
        ("759 beside the rail", 695.0, HEIGHT, 631.0),
        ("the least window beside the rail", 536.0, 600.0, 472.0),
    ] {
        let mut start = found(room);
        start.viewport = Size::new(room, height);
        let renderer = laid_out(start);
        let at = |class: &str| boxes_of(&renderer, class);
        // The clip 40 down to 24 over the box, which is 24 over the foot.
        assert_eq!(
            at("thread"),
            vec![px(0.0, 40.0, room, height - 144.0)],
            "{said}"
        );
        assert_eq!(at("rtabs"), vec![px(32.0, 102.0, column, 42.0)], "{said}");
        assert_eq!(at("rt on"), vec![px(32.0, 102.0, 45.546_875, 42.0)]);
        assert_eq!(
            at("rt"),
            vec![
                px(162.468_75, 102.0, 59.546_875, 42.0),
                px(304.906_25, 102.0, 72.5, 42.0),
                px(381.406_25, 102.0, 63.343_75, 42.0),
            ],
            "{said}: News ends at 444.75, inside the column's {}",
            32.0 + column
        );
        assert_eq!(
            at("box"),
            vec![px(32.0, height - 80.0, column, 56.0)],
            "{said}"
        );
        let column_box = Rect::new(32.0, 0.0, column, height);
        for class in ["rtabs", "rt", "found", "t", "box"] {
            let rects = rects_of(&renderer, class);
            assert!(!rects.is_empty(), "{said}: no {class}");
            assert!(
                rects.iter().all(|rect| inside(*rect, column_box)),
                "{said}: a {class} outside the column: {rects:?}"
            );
        }
        // And the card the browser computes is the one laid out here too.
        let viewport = Size::new(room, height);
        let carded = laid_out(Start {
            viewport,
            ..with_a_card(room)
        });
        assert_eq!(
            boxes_of(&carded, "page"),
            vec![units(card_place::page_box(viewport, 0.0))],
            "{said}"
        );
        assert_eq!(
            boxes_of(&carded, "thread"),
            vec![px(
                0.0,
                card_place::TOP,
                room,
                card_place::room_bottom(viewport, 0) - card_place::TOP
            )],
            "{said}"
        );
    }
}

/// The card's page box, the box and the room the conversation is seen in,
/// as the browser process computes them for itself (`card_place`), are the
/// ones laid out at every narrow room — so Chrome is asked for a page the
/// size of the box it is drawn in, and a press lands where it is seen.
#[test]
fn the_card_and_the_box_the_browser_computes_are_the_ones_laid_out() {
    for (said, room) in ROOMS {
        let viewport = Size::new(room, HEIGHT);
        let renderer = laid_out(with_a_card(room));
        let column = card_place::column(viewport);
        let left = card_place::column_left(viewport);
        assert_eq!(
            boxes_of(&renderer, "card"),
            vec![px(left, 98.0, column, 568.0)],
            "{said}"
        );
        assert_eq!(
            boxes_of(&renderer, "page"),
            vec![units(card_place::page_box(viewport, 0.0))],
            "{said}"
        );
        // The page box inside the card's border, under its bar: the size
        // Chrome is asked to lay the page out at.
        assert_eq!(
            boxes_of(&renderer, "page"),
            vec![px(left + 1.0, 145.0, column - 2.0, 520.0)],
            "{said}"
        );
        let (wide, tall) = card_place::page_size(viewport);
        assert!(
            (f64::from(wide) - f64::from(column - 2.0)).abs() < 0.01 && tall == 520,
            "{said}: {wide} × {tall}"
        );
        assert_eq!(
            boxes_of(&renderer, "box"),
            vec![px(
                left,
                card_place::box_top(viewport),
                column,
                card_place::BOX_HEIGHT
            )],
            "{said}"
        );
        assert_eq!(
            boxes_of(&renderer, "thread"),
            vec![px(
                left - card_place::GUTTER,
                card_place::TOP,
                column + 2.0 * card_place::GUTTER,
                card_place::room_bottom(viewport, 0) - card_place::TOP
            )],
            "{said}"
        );
        // Close, the bar's last button, inside the card: 6 in from its
        // right edge, 36 square.
        let close = rects_of(&renderer, "close");
        assert!(
            close
                .first()
                .is_some_and(|close| (close.right() - (left + column - 7.0)).abs() < 0.01),
            "{said}: {close:?}"
        );
    }
}

/// The first screen's own column, 720 at most, is the room less 32 either
/// side too: the box, and the sites and chats waiting under it.
#[test]
fn the_first_screens_column_is_the_room_less_32_either_side_when_narrow() {
    for (said, room, column) in [
        ("720 beside the panel", 480.0, 416.0),
        ("720 beside the rail", 656.0, 592.0),
        ("967 beside the panel", 727.0, 663.0),
        ("the design's", 1040.0, 720.0),
    ] {
        let start = Start {
            greeting: "Hello.".to_owned(),
            viewport: Size::new(room, HEIGHT),
            ..Start::default()
        };
        let renderer = laid_out(start);
        let left = (room - column) / 2.0;
        let the_box = rects_of(&renderer, "box");
        assert!(
            the_box.first().is_some_and(|the_box| {
                (the_box.left() - left).abs() < 0.01
                    && (the_box.size.width - column).abs() < 0.01
                    && (the_box.size.height - 64.0).abs() < 0.01
            }),
            "{said}: {the_box:?}"
        );
        let blocks = rects_of(&renderer, "blocks");
        assert!(
            blocks.first().is_some_and(|blocks| {
                (blocks.left() - left).abs() < 0.01 && (blocks.size.width - column).abs() < 0.01
            }),
            "{said}: {blocks:?}"
        );
    }
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
fn the_all_tab_in_a_window_967_wide_is_drawn_as_the_reference() {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Start(Box::new(found(727.0))));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{answer:?}"
    );
    let frame = match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Some(frame),
        _ => None,
    };
    assert!(frame.is_some(), "nothing was painted");
    if let Some(frame) = &frame {
        assert_eq!((frame.width, frame.height), (727, 820));
        // The chosen tab's underline in the ink from the column's left
        // edge; the first site's mark, not the paper, at 33; and the paper in
        // the gutters either side.
        assert_eq!(pixel(frame, 50, 143), Some([0x1b, 0x24, 0x33, 0xff]));
        assert_ne!(pixel(frame, 33, 177), Some([0xf4, 0xf1, 0xec, 0xff]));
        assert_eq!(pixel(frame, 16, 177), Some([0xf4, 0xf1, 0xec, 0xff]));
        assert_eq!(pixel(frame, 710, 768), Some([0xf4, 0xf1, 0xec, 0xff]));
        // The box white inside, 32 in from either side.
        assert_eq!(pixel(frame, 40, 768), Some([0xff, 0xff, 0xff, 0xff]));
        assert_eq!(pixel(frame, 690, 768), Some([0xff, 0xff, 0xff, 0xff]));
    }
    assert_eq!(
        holds("narrow-all-tab-with-results.png", frame.as_ref()),
        Ok(())
    );
}
