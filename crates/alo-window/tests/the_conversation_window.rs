/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 709: the chats panel and the first screen in the engine's
//! window (ADR 0074), composed as the strip and the page are (ADR 0024 § 2).
//!
//! The window `alo --conversation` opens is the design's, 1280 by 820: the
//! panel down the left, 240 wide, and the first screen in the 1040 beside
//! it. Each part is drawn here by the engine from the state the browser
//! process sends ([`Conversation`]), with the corpus's faces, and the window
//! composed from the two frames is compared with a committed reference at
//! one and at two — and checked in numbers: every device pixel of each room
//! is its part's frame's own, one to one.
//!
//! Then the two parts are painted by two **confined** renderers, each at a
//! site of its own, held by the thread the window holds
//! ([`ConversationDrawing`]) — and each frame is the one painted here, pixel
//! for pixel, so the window composed from them is the reference.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-window --test
//! the_conversation_window` rewrites the references. A rewritten reference
//! is a diff somebody reads.

use alo_layout::Size;
use alo_paint::{Canvas, from_png, to_png};
use alo_renderer::host::Renderers;
use alo_renderer::{Frame, FromRenderer, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_window::beside::renderer_beside;
use alo_window::conversation::{Conversation, Pressed};
use alo_window::conversation_compose::{ConversationScene, compose_conversation};
use alo_window::conversation_drawing::{Asked, ConversationDrawing, Drawn, Part};
use alo_window::conversation_place::{
    ConversationPlacement, PANEL_WIDTH, Parts, RAIL_WIDTH, Side, panel_viewport, panel_viewport_as,
    start_viewport, start_viewport_as,
};
use alo_window::notice::Lettering;
use alo_window::place::PixelRect;
use alo_window::scale::Scale;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::time::Duration;

/// The design's window at one, in device pixels.
const AT_ONE: (u32, u32) = (1280, 820);

/// The same window at two.
const AT_TWO: (u32, u32) = (2560, 1640);

/// The corpus's faces, and what `system-ui`, `sans-serif` and `serif` mean:
/// the panel's and the first screen's stylesheets ask for all three.
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
    // The design's typefaces, as the browser process hands them to the
    // interface's renderers (ADR 0084).
    for font in alo_renderer::typefaces::fonts() {
        database.add(font);
    }
    database
}

/// What the window asks to be drawn at `window` and `scale`, as its event
/// loop asks it.
fn asked(window: (u32, u32), scale: Scale) -> Asked {
    let conversation = Conversation::default();
    Asked {
        panel: conversation.panel(panel_viewport(window, scale), scale),
        start: conversation.start(start_viewport(window, scale), scale),
        showing: conversation.showing(),
    }
}

/// Build one part in a renderer of this process and paint it.
fn painted(work: ToRenderer) -> Result<Frame, String> {
    let mut renderer = Renderer::new(fonts());
    match renderer.handle(work) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("not built: {other:?}")),
    }
    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Ok(frame),
        other => Err(format!("not painted: {other:?}")),
    }
}

/// Both parts as the engine paints them for a window of `window` at
/// `scale`: the panel's frame, then the first screen's.
fn parts(window: (u32, u32), scale: Scale) -> Result<(Frame, Frame), String> {
    let Asked { panel, start, .. } = asked(window, scale);
    Ok((
        painted(ToRenderer::Panel(Box::new(panel)))?,
        painted(ToRenderer::Start(Box::new(start)))?,
    ))
}

fn lettering() -> Result<Lettering, String> {
    Lettering::compiled_in().ok_or_else(|| "DejaVu Sans is not compiled in".to_owned())
}

/// The window composed from `panel` and `start`.
fn composed(
    window: (u32, u32),
    scale: Scale,
    panel: &Frame,
    start: &Frame,
) -> Result<Canvas, String> {
    Ok(compose_conversation(
        &ConversationScene {
            window,
            scale,
            panel: Some(panel),
            panel_sentence: None,
            start: Some(start),
            start_sentence: None,
            side: Side::Panel,
        },
        &lettering()?,
    ))
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
        .collect::<Vec<_>>();
    match differing.first() {
        None => Ok(()),
        Some(first) => Err(format!(
            "{name}: {} pixels moved, the first at {first:?}. If this is intended, \
             ALO_UPDATE_REFERENCES=1 rewrites the reference",
            differing.len(),
        )),
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

/// Compose the design's window at `scale`, check each part fills its room
/// one to one, and compare it with `name`.
fn the_window_at(window: (u32, u32), scale: Scale, name: &str) -> Result<(), String> {
    let (panel, start) = parts(window, scale)?;
    let device = |css: u32| scale.device(css);
    // Each part painted exactly its room: the panel 240 by 820, the first
    // screen 1040 by 820, at the scale.
    assert_eq!(
        (panel.width, panel.height),
        (device(PANEL_WIDTH), device(820))
    );
    assert_eq!((start.width, start.height), (device(1040), device(820)));
    let canvas = composed(window, scale, &panel, &start)?;
    let placed = ConversationPlacement::of(
        window,
        scale,
        ConversationScene {
            window,
            scale,
            panel: Some(&panel),
            panel_sentence: None,
            start: Some(&start),
            start_sentence: None,
            side: Side::Panel,
        }
        .parts(),
    );
    assert_eq!(placed.panel, placed.panel_room, "the panel fills its room");
    assert_eq!(placed.start, placed.start_room, "and the first screen its");
    assert_eq!(not_one_to_one(&canvas, &panel, placed.panel), None);
    assert_eq!(not_one_to_one(&canvas, &start, placed.start), None);
    holds(name, &canvas)
}

#[test]
fn the_panel_and_the_first_screen_in_the_designs_window_at_one() {
    let window = AT_ONE;
    assert_eq!(
        (
            panel_viewport(window, Scale::ONE),
            start_viewport(window, Scale::ONE)
        ),
        (Size::new(240.0, 820.0), Size::new(1040.0, 820.0))
    );
    let placed = ConversationPlacement::of(window, Scale::ONE, Parts::default());
    assert_eq!(
        (placed.panel_room, placed.start_room),
        (
            PixelRect {
                x: 0,
                y: 0,
                width: 240,
                height: 820
            },
            PixelRect {
                x: 240,
                y: 0,
                width: 1040,
                height: 820
            }
        )
    );
    the_window_at(window, Scale::ONE, "the-conversation.png").unwrap_or_else(|why| panic!("{why}"));
}

#[test]
fn the_panel_and_the_first_screen_in_the_designs_window_at_two() {
    let window = AT_TWO;
    assert_eq!(
        (
            panel_viewport(window, Scale::TWO),
            start_viewport(window, Scale::TWO)
        ),
        (Size::new(240.0, 820.0), Size::new(1040.0, 820.0))
    );
    let placed = ConversationPlacement::of(window, Scale::TWO, Parts::default());
    assert_eq!(
        (placed.panel_room, placed.start_room),
        (
            PixelRect {
                x: 0,
                y: 0,
                width: 480,
                height: 1640
            },
            PixelRect {
                x: 480,
                y: 0,
                width: 2080,
                height: 1640
            }
        )
    );
    the_window_at(window, Scale::TWO, "the-conversation-at-two.png")
        .unwrap_or_else(|why| panic!("{why}"));
}

/// Queue item 711: the panel hidden by its button, in the design's window
/// at `scale` — the rail 64 wide and the first screen in all the rest, each
/// filling its room one to one — compared with `name`.
fn the_window_hidden_at(window: (u32, u32), scale: Scale, name: &str) -> Result<(), String> {
    let mut conversation = Conversation::default();
    assert!(conversation.act(Pressed::HideThePanel));
    let side = conversation.side();
    assert_eq!(side, Side::Rail);
    let panel = painted(ToRenderer::Panel(Box::new(
        conversation.panel(panel_viewport_as(window, scale, side), scale),
    )))?;
    let start = painted(ToRenderer::Start(Box::new(
        conversation.start(start_viewport_as(window, scale, side), scale),
    )))?;
    let device = |css: u32| scale.device(css);
    assert_eq!(
        (panel.width, panel.height),
        (device(RAIL_WIDTH), device(820))
    );
    assert_eq!((start.width, start.height), (device(1216), device(820)));
    let scene = ConversationScene {
        window,
        scale,
        panel: Some(&panel),
        panel_sentence: None,
        start: Some(&start),
        start_sentence: None,
        side,
    };
    let canvas = compose_conversation(&scene, &lettering()?);
    let placed = ConversationPlacement::of(window, scale, scene.parts());
    assert_eq!(
        placed.panel_room,
        PixelRect {
            x: 0,
            y: 0,
            width: device(64),
            height: device(820)
        }
    );
    assert_eq!(
        placed.start_room,
        PixelRect {
            x: device(64),
            y: 0,
            width: device(1216),
            height: device(820)
        }
    );
    assert_eq!(placed.panel, placed.panel_room, "the rail fills its room");
    assert_eq!(placed.start, placed.start_room, "and the first screen its");
    assert_eq!(not_one_to_one(&canvas, &panel, placed.panel), None);
    assert_eq!(not_one_to_one(&canvas, &start, placed.start), None);
    holds(name, &canvas)
}

#[test]
fn the_panel_hidden_is_the_rail_and_the_first_screen_the_rest_at_one_and_two() {
    the_window_hidden_at(AT_ONE, Scale::ONE, "the-conversation-panel-hidden.png")
        .unwrap_or_else(|why| panic!("{why}"));
    the_window_hidden_at(
        AT_TWO,
        Scale::TWO,
        "the-conversation-panel-hidden-at-two.png",
    )
    .unwrap_or_else(|why| panic!("{why}"));
}

/// How long the confined renderers may take to paint both parts.
const AT_MOST: Duration = Duration::from_secs(60);

fn renderer() -> Result<PathBuf, String> {
    let path = renderer_beside(Path::new(env!("CARGO_BIN_EXE_alo")));
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "no renderer at {}: `cargo test --workspace` builds it, `cargo test -p alo-window` \
             alone does not",
            path.display()
        ))
    }
}

/// The two frames the window is told the confined renderers painted, at
/// `window` and `scale`: the panel's, then the first screen's.
fn painted_confined(window: (u32, u32), scale: Scale) -> Result<(Frame, Frame), String> {
    let renderers = Renderers::running(renderer()?, &[]).with_machine(alo_window::fonts::frozen());
    let (tell, told) = channel();
    let mut drawing =
        ConversationDrawing::start(renderers, Box::new(move |drawn| tell.send(drawn).is_ok()))
            .map_err(|why| format!("no thread: {why}"))?;
    drawing.draw(asked(window, scale));
    // The same again is not drawn again: two answers, and no more.
    drawing.draw(asked(window, scale));
    let mut panel = None;
    let mut start = None;
    while panel.is_none() || start.is_none() {
        match told.recv_timeout(AT_MOST) {
            Ok(Drawn::Painted(Part::Panel, frame)) => panel = Some(frame),
            Ok(Drawn::Painted(Part::Start, frame)) => start = Some(frame),
            Ok(Drawn::Said(part, said)) => return Err(format!("{part}: {said}")),
            Ok(Drawn::Pressed(part, pressed)) => {
                return Err(format!("{part}: nothing was pressed, and {pressed:?}"));
            }
            // A first screen with no turns is not measured (queue item 703).
            Ok(Drawn::Reached { .. }) => return Err("a greeting was measured".to_owned()),
            Err(why) => return Err(format!("nothing was painted: {why}")),
        }
    }
    if !drawing.finish() {
        return Err("the drawing thread did not finish".to_owned());
    }
    if let Ok(more) = told.try_recv() {
        return Err(format!("drawn again: {more:?}"));
    }
    match (panel, start) {
        (Some(panel), Some(start)) => Ok((panel, start)),
        _ => Err("a part was not painted".to_owned()),
    }
}

#[test]
fn two_confined_renderers_paint_the_same_window() {
    let (panel, start) = painted_confined(AT_ONE, Scale::ONE).unwrap_or_else(|why| panic!("{why}"));
    let (here_panel, here_start) = parts(AT_ONE, Scale::ONE).unwrap_or_else(|why| panic!("{why}"));
    assert!(
        panel == here_panel,
        "the confined renderer painted another panel"
    );
    assert!(
        start == here_start,
        "the confined renderer painted another first screen"
    );
}
