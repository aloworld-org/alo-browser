/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 775: in a window narrower than 760 the chats panel folds to
//! its 64 px rail on its own (`docs/design/shell.md`, *The window*), and
//! the conversation takes the rest; widened again it unfolds, as it was.
//!
//! Each part is drawn here by the engine from the state the browser
//! process sends ([`Conversation`]), told the window's width as the window
//! tells it ([`Conversation::fit`]), with the corpus's faces and the
//! design's typefaces; the window composed from the two frames 759 wide —
//! a search's All tab beside the rail — is compared with a committed
//! reference, and checked in numbers: the rail's room 64 and the first
//! screen's 695, each frame placed one to one.
//!
//! Queue item 776: in such a window *Show the panel* slides the whole panel
//! over the conversation as a sheet, over a shade — composed here at 700,
//! all the way out and part way — and queue item 760: the rail shows the
//! chats, and the name of the one the pointer is over stands past its edge
//! over the first screen — composed here at 759.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-window --test
//! the_panel_folded_in_a_narrow_window` rewrites the references. A
//! rewritten reference is a diff somebody reads.

use alo_paint::{Canvas, from_png, to_png};
use alo_renderer::results::{Found, SetUp, Showing};
use alo_renderer::{Frame, FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_window::conversation::{Conversation, Pressed};
use alo_window::conversation_compose::{ConversationScene, SheetScene, compose_conversation};
use alo_window::conversation_fold::FOLD_WIDTH;
use alo_window::conversation_place::{
    ConversationPlacement, PANEL_WIDTH, RAIL_WIDTH, Side, panel_viewport_as, panel_viewport_of,
    sheet_left, start_viewport_as,
};
use alo_window::conversation_search::Searched;
use alo_window::conversation_sheet::{SHADE, Slide};
use alo_window::notice::Lettering;
use alo_window::place::PixelRect;
use alo_window::scale::Scale;
use alo_window::typing::Edit;
use std::path::{Path, PathBuf};

/// The window one narrower than the fold, at one, in device pixels.
const FOLDED: (u32, u32) = (FOLD_WIDTH - 1, 820);

/// The corpus's faces, and what `system-ui`, `sans-serif` and `serif` mean,
/// and the design's typefaces as the browser process hands them over (ADR
/// 0084).
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
    database.map_generic("system-ui", "DejaVu Sans");
    database.map_generic("sans-serif", "DejaVu Sans");
    database.map_generic("serif", "DejaVu Serif");
    for font in alo_renderer::typefaces::fonts() {
        database.add(font);
    }
    database
}

/// Build one part in a renderer of this process and paint it.
fn painted(work: ToRenderer) -> Result<Frame, String> {
    let mut renderer = Renderer::new(fonts());
    match renderer.handle(work) {
        FromRenderer::Loaded { issues, .. } if issues.is_empty() => {}
        other => return Err(format!("not built: {other:?}")),
    }
    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Ok(frame),
        other => Err(format!("not painted: {other:?}")),
    }
}

/// A conversation that searched *rust ownership* and was answered with
/// three sites, as a provider might answer, written for this test.
fn searched() -> Result<Conversation, String> {
    let mut conversation = Conversation::default();
    conversation.set_search(SetUp::Ready);
    let _ = conversation.act(Pressed::TheBox);
    for letter in "rust ownership".chars() {
        let _ = conversation.edit(Edit::Put(letter.to_string()));
    }
    let _ = conversation.edit(Edit::Choose);
    let site = |title: &str, address: &str, description: &str| {
        alo_url::parse(address)
            .map(|url| Found::new(title, url, description))
            .map_err(|why| format!("{address}: {why:?}"))
    };
    let sites = vec![
        site(
            "Understanding Ownership - The Rust Programming Language",
            "https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html",
            "Ownership is Rust's most unique feature and has deep implications for the \
             rest of the language.",
        )?,
        site(
            "What is Ownership? - The Rust Programming Language",
            "https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html",
            "Ownership is a set of rules that govern how a Rust program manages memory.",
        )?,
        site(
            "Rust ownership, explained",
            "https://ownership.example/",
            "Who owns a value, and when it is dropped.",
        )?,
    ];
    if !conversation.searched(Searched::sites(1, "rust ownership", Showing::Found(sites))) {
        return Err("the search's sites were not shown".to_owned());
    }
    Ok(conversation)
}

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

/// Compare `canvas` with the committed reference, or write it when
/// updating. What differs, in words, if anything does.
fn holds(name: &str, canvas: &Canvas) -> Result<(), String> {
    let path = reference(name);
    let drawn = to_png(canvas).map_err(|why| format!("{name} could not be encoded: {why}"))?;
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
    if (canvas.width(), canvas.height()) != (expected.width(), expected.height()) {
        return Err(format!(
            "{name}: the window is {}×{}, and the reference {}×{}",
            canvas.width(),
            canvas.height(),
            expected.width(),
            expected.height()
        ));
    }
    let differing = (0..canvas.height())
        .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
        .filter(|(x, y)| {
            canvas.at(*x, *y).map(alo_value::Rgba::to_rgba8)
                != expected.at(*x, *y).map(alo_value::Rgba::to_rgba8)
        })
        .count();
    if differing == 0 {
        Ok(())
    } else {
        Err(format!(
            "{name}: {differing} pixels moved. If this is intended, \
             ALO_UPDATE_REFERENCES=1 rewrites the reference"
        ))
    }
}

/// The first device pixel of `rect` in `canvas` that is not `frame`'s own
/// pixel at the same place from the rect's corner, if any is not.
fn not_one_to_one(canvas: &Canvas, frame: &Frame, rect: PixelRect) -> Option<(u32, u32)> {
    (0..rect.height)
        .flat_map(|y| (0..rect.width).map(move |x| (x, y)))
        .find(|(x, y)| {
            canvas
                .at(rect.x + x, rect.y + y)
                .map(alo_value::Rgba::to_rgba8)
                != frame.at(*x, *y)
        })
}

/// What the panel and the first screen paint as `conversation` holds them
/// in a `window` at one.
fn both(conversation: &Conversation, window: (u32, u32)) -> Result<(Frame, Frame), String> {
    let side = conversation.side();
    let scale = Scale::ONE;
    Ok((
        painted(ToRenderer::Panel(Box::new(conversation.panel(
            panel_viewport_of(window, scale, conversation.panel_width()),
            scale,
        ))))?,
        painted(ToRenderer::Start(Box::new(
            conversation.start(start_viewport_as(window, scale, side), scale),
        )))?,
    ))
}

#[test]
fn a_window_narrowed_past_760_is_the_rail_and_the_conversation_beside_it() {
    let mut conversation = searched().unwrap_or_else(|why| panic!("{why}"));
    assert!(!conversation.fit(FOLD_WIDTH), "760 holds the panel");
    assert_eq!(conversation.side(), Side::Panel);
    assert!(conversation.fit(FOLDED.0), "759 folds it");
    let side = conversation.side();
    assert_eq!(side, Side::Rail);
    let (panel, start) = both(&conversation, FOLDED).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((panel.width, panel.height), (RAIL_WIDTH, 820));
    assert_eq!((start.width, start.height), (695, 820));
    let scene = ConversationScene {
        window: FOLDED,
        scale: Scale::ONE,
        panel: Some(&panel),
        panel_sentence: None,
        start: Some(&start),
        start_sentence: None,
        side,
        named: false,
        sheet: None,
    };
    let canvas = compose_conversation(
        &scene,
        &Lettering::compiled_in().unwrap_or_else(|| panic!("DejaVu Sans is not compiled in")),
    );
    let placed = ConversationPlacement::of(FOLDED, Scale::ONE, scene.parts());
    assert_eq!(
        placed.panel_room,
        PixelRect {
            x: 0,
            y: 0,
            width: 64,
            height: 820
        }
    );
    assert_eq!(
        placed.start_room,
        PixelRect {
            x: 64,
            y: 0,
            width: 695,
            height: 820
        }
    );
    assert_eq!(placed.panel, placed.panel_room, "the rail fills its room");
    assert_eq!(placed.start, placed.start_room, "and the first screen its");
    assert_eq!(not_one_to_one(&canvas, &panel, placed.panel), None);
    assert_eq!(not_one_to_one(&canvas, &start, placed.start), None);
    // The paper in the column's gutters, 64 + 16 and 759 - 16 across; the
    // chosen tab's ink underline from the column's left, 64 + 32.
    let at = |x: u32, y: u32| canvas.at(x, y).map(alo_value::Rgba::to_rgba8);
    assert_eq!(at(80, 177), Some((0xf4, 0xf1, 0xec, 0xff)));
    assert_eq!(at(743, 177), Some((0xf4, 0xf1, 0xec, 0xff)));
    assert_eq!(at(64 + 50, 143), Some((0x1b, 0x24, 0x33, 0xff)));
    assert_eq!(holds("the-panel-folded-at-759.png", &canvas), Ok(()));
}

#[test]
fn widened_past_the_fold_the_panel_is_back_as_it_was() {
    let mut conversation = searched().unwrap_or_else(|why| panic!("{why}"));
    let _ = conversation.act(Pressed::SearchChats);
    let _ = conversation.edit(Edit::Put("n".to_owned()));
    assert!(conversation.fit(FOLDED.0));
    assert!(conversation.fit(FOLD_WIDTH));
    assert_eq!(conversation.side(), Side::Panel);
    let window = (FOLD_WIDTH, 820);
    let (panel, start) = both(&conversation, window).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((panel.width, panel.height), (PANEL_WIDTH, 820));
    assert_eq!((start.width, start.height), (520, 820));
    let drawn = conversation.panel(
        panel_viewport_as(window, Scale::ONE, Side::Panel),
        Scale::ONE,
    );
    assert!(!drawn.hidden);
    assert_eq!(drawn.searched, "n", "what was searched is kept");
}

/// Each channel of `got` within a level of `want`.
fn near(got: Option<(u8, u8, u8, u8)>, want: (u8, u8, u8, u8)) -> bool {
    got.is_some_and(|got| {
        got.0.abs_diff(want.0) <= 1
            && got.1.abs_diff(want.1) <= 1
            && got.2.abs_diff(want.2) <= 1
            && got.3 == want.3
    })
}

/// `under` with the ink at the sheet's shade over it.
fn shaded(under: (u8, u8, u8)) -> (u8, u8, u8, u8) {
    let mix = |under: u8, ink: u8| {
        let mixed = f32::from(under) * (1.0 - SHADE) + f32::from(ink) * SHADE;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a mix of two channels, inside 0..=255"
        )]
        let mixed = mixed.round() as u8;
        mixed
    };
    (
        mix(under.0, 0x1b),
        mix(under.1, 0x24),
        mix(under.2, 0x33),
        0xff,
    )
}

#[test]
fn shown_at_700_the_panel_slides_over_the_conversation_as_a_sheet_over_a_shade() {
    let lettering =
        Lettering::compiled_in().unwrap_or_else(|| panic!("DejaVu Sans is not compiled in"));
    let window = (700, 820);
    let mut conversation = searched().unwrap_or_else(|why| panic!("{why}"));
    assert!(conversation.fit(window.0));
    assert!(conversation.act(Pressed::ShowThePanel));
    assert!(conversation.sheet());
    // The first screen still beside the rail; the panel laid out whole.
    assert_eq!(conversation.side(), Side::Rail);
    assert_eq!(conversation.panel_width(), PANEL_WIDTH);
    let (panel, start) = both(&conversation, window).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((panel.width, panel.height), (PANEL_WIDTH, 820));
    assert_eq!((start.width, start.height), (636, 820));
    // All the way out: the sheet from the window's left edge, 240 wide.
    let at = std::time::Instant::now();
    let mut slide = Slide::default();
    slide.open(at);
    let out = at + std::time::Duration::from_millis(200);
    assert_eq!(sheet_left(slide.left(out), Scale::ONE), 0);
    let scene = ConversationScene {
        window,
        scale: Scale::ONE,
        panel: Some(&panel),
        panel_sentence: None,
        start: Some(&start),
        start_sentence: None,
        side: Side::Rail,
        named: false,
        sheet: Some(SheetScene {
            frame: &panel,
            left: sheet_left(slide.left(out), Scale::ONE),
            shade: slide.shade(out),
        }),
    };
    let placed = ConversationPlacement::of(window, Scale::ONE, scene.parts());
    assert_eq!(
        placed.start_room,
        PixelRect {
            x: 64,
            y: 0,
            width: 636,
            height: 820
        },
        "the conversation laid out beside the rail, under the sheet"
    );
    let canvas = compose_conversation(&scene, &lettering);
    let at_xy = |x: u32, y: u32| canvas.at(x, y).map(alo_value::Rgba::to_rgba8);
    // The sheet is the panel's frame one to one, 0 to 239.
    assert_eq!(
        not_one_to_one(
            &canvas,
            &panel,
            PixelRect {
                x: 0,
                y: 0,
                width: 240,
                height: 820
            }
        ),
        None
    );
    // Past it, the conversation's paper under the shade: in its gutter,
    // 64 + 16 across, and at the window's right.
    let paper = shaded((0xf4, 0xf1, 0xec));
    assert!(near(at_xy(240, 177), paper), "{:?}", at_xy(240, 177));
    assert!(near(at_xy(699, 177), paper), "{:?}", at_xy(699, 177));
    assert_eq!(holds("the-sheet-at-700.png", &canvas), Ok(()));
    // Part way, 50 ms out: its left edge at −74, its right at 166, the
    // shade lighter.
    let part = at + std::time::Duration::from_millis(50);
    let left = sheet_left(slide.left(part), Scale::ONE);
    assert_eq!(left, -74);
    let canvas = compose_conversation(
        &ConversationScene {
            sheet: Some(SheetScene {
                frame: &panel,
                left,
                shade: slide.shade(part),
            }),
            ..scene
        },
        &lettering,
    );
    let at_xy = |x: u32, y: u32| canvas.at(x, y).map(alo_value::Rgba::to_rgba8);
    assert_eq!(at_xy(165, 5), panel.at(239, 5));
    assert_ne!(at_xy(166, 5), panel.at(239, 5));
    assert_eq!(holds("the-sheet-sliding-at-700.png", &canvas), Ok(()));
}

#[test]
fn the_rail_at_759_names_the_chat_the_pointer_is_over_past_its_edge() {
    let lettering =
        Lettering::compiled_in().unwrap_or_else(|| panic!("DejaVu Sans is not compiled in"));
    let mut conversation = searched().unwrap_or_else(|why| panic!("{why}"));
    assert!(conversation.fit(FOLDED.0));
    let chat = conversation.showing().chats[0];
    assert!(conversation.hover(Some(chat)));
    assert!(conversation.rail_named());
    assert_eq!(conversation.panel_width(), 320);
    let (panel, start) = both(&conversation, FOLDED).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((panel.width, panel.height), (320, 820));
    let scene = ConversationScene {
        window: FOLDED,
        scale: Scale::ONE,
        panel: Some(&panel),
        panel_sentence: None,
        start: Some(&start),
        start_sentence: None,
        side: Side::Rail,
        named: true,
        sheet: None,
    };
    let placed = ConversationPlacement::of(FOLDED, Scale::ONE, scene.parts());
    assert_eq!(
        placed.panel,
        PixelRect {
            x: 0,
            y: 0,
            width: 320,
            height: 820
        },
        "over the first screen"
    );
    let canvas = compose_conversation(&scene, &lettering);
    let at_xy = |x: u32, y: u32| canvas.at(x, y).map(alo_value::Rgba::to_rgba8);
    // The chat, 44 square at (10, 121), a shade lighter; its name's dark
    // pill past the rail's edge, from 68, centred on it; the first screen
    // where the rail's frame draws nothing.
    assert_eq!(at_xy(12, 143), Some((0xf6, 0xf3, 0xee, 0xff)));
    assert_eq!(at_xy(70, 143), Some((0x2b, 0x27, 0x22, 0xff)));
    assert_eq!(at_xy(66, 143), start.at(2, 143));
    assert_eq!(at_xy(400, 400), start.at(336, 400));
    assert_eq!(holds("the-rail-naming-a-chat-at-759.png", &canvas), Ok(()));
}
