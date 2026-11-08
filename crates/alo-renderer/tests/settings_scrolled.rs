/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 880 (`docs/design/shell.md`, *Settings*): *Settings* scrolls
//! as the conversation does. In the least window, 600 tall, its page runs
//! past the room, so it reaches further than the room and is drawn moved up
//! by how far the browser process scrolled it, clipped to the screen, with
//! the sign that it goes on in the clip's right gutter; and where the thing
//! the keyboard is on stands is answered with its reach, so it can be
//! scrolled into view.
//!
//! Each a test below: how far it reaches and where it is drawn scrolled, in
//! numbers; where the keyed thing stands; the reference
//! (`references/settings-scrolled-short.png`, scrolled to its foot, the
//! keyboard on *Ask before opening*); and the bar read by an agent.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test settings_scrolled`
//! rewrites the render reference. A rewritten reference is a diff somebody
//! reads.

use alo_layout::{Rect, Size};
use alo_paint::from_png;
use alo_renderer::conversation_scroll::{self, Reach, Span};
use alo_renderer::frame::frame_to_png;
use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::start::{SettingsKeyed, Start};
use alo_renderer::{Frame, FromRenderer, Renderer, Snapshot, SnapshotNode, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::{Path, PathBuf};

/// The corpus's faces and generics, as every interface test has them, and
/// the interface's own after them, as the browser process hands them.
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

/// The first screen's room in the least window, 600 square, beside the
/// rail: 536 wide.
const LEAST: Size = Size {
    width: 536.0,
    height: 600.0,
};

/// *Settings*, set in `face`, laid out at `viewport`.
fn settings(face: InterfaceFace, viewport: Size) -> Start {
    Start {
        greeting: "Hello.".to_owned(),
        viewport,
        face,
        settings: true,
        ..Start::default()
    }
}

/// What a renderer holding a part answers to a paint and a reading.
struct Drawn {
    renderer: Renderer,
    frame: Option<Frame>,
    tree: Snapshot,
}

fn draw(work: ToRenderer) -> Drawn {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(work);
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
    Drawn {
        renderer,
        frame,
        tree,
    }
}

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
    rects_of(renderer, class).into_iter().map(units).collect()
}

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
                .is_some_and(|node| document.attribute_by_name(node, "class") == Some(class))
        })
        .filter_map(|id| drawing.layout.get(id))
        .map(|geometry| geometry.border_box)
        .collect()
}

/// Every node the agent reads in `role`, in order.
fn read<'a>(tree: &'a Snapshot, role: &str) -> Vec<&'a SnapshotNode> {
    tree.nodes()
        .into_iter()
        .filter(|node| node.role.as_str() == role)
        .collect()
}

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
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
            "{name}: {}×{}, and the reference {}×{}",
            frame.width, frame.height, expected.width, expected.height
        ));
    }
    let width = usize::try_from(frame.width).unwrap_or(1).max(1);
    match frame
        .pixels
        .chunks(4)
        .zip(expected.pixels.chunks(4))
        .position(|(drawn, committed)| drawn != committed)
    {
        None => Ok(()),
        Some(at) => Err(format!(
            "{name}: pixels moved, the first at ({}, {}). If this is intended, \
             ALO_UPDATE_REFERENCES=1 rewrites the reference",
            at % width,
            at / width
        )),
    }
}

/// How far *Settings* reaches in the least window: what its renderer
/// answered, read here once and checked against the layout above.
const REACH: f32 = 838.0;

/// *Settings* in the least window, scrolled `down`, the keyboard on
/// `keyed`, with the bar the browser process would send for `reach`.
fn scrolled(down: f32, keyed: Option<SettingsKeyed>, reach: Option<Reach>) -> Start {
    Start {
        down,
        bar: reach.and_then(|reach| conversation_scroll::bar_in(down, reach, LEAST.height, 1.0)),
        settings_keyed: keyed,
        ..settings(InterfaceFace::Inter, LEAST)
    }
}

/// What the renderer holding `start` answers it reaches, and where the
/// keyed thing stands.
fn reached(drawn: &mut Drawn) -> Option<(Reach, Option<Span>)> {
    match drawn.renderer.handle(ToRenderer::ConversationReach) {
        FromRenderer::ConversationReached(reach, keyed) => Some((reach, keyed)),
        _ => None,
    }
}

/// How far *Settings* reaches in the least window, as its renderer says.
fn least_reach() -> Option<Reach> {
    let mut drawn = draw(ToRenderer::Start(Box::new(scrolled(0.0, None, None))));
    reached(&mut drawn).map(|(reach, _)| reach)
}

#[test]
fn in_the_least_window_it_reaches_past_the_room_and_is_drawn_scrolled() {
    let reach = least_reach().expect("a reach");
    // The page from the screen's top: 32 above it, then its heading of 36,
    // *Font* and its faces, two a row in the column of 472, and *alo at
    // work*, and 24 under it: 838 — the 816 it is in the design's room, and
    // 22 more where the column is narrow (its line about it on two lines,
    // *Ask before opening*'s choices under its name).
    assert_eq!(reach.earlier.to_bits(), 0.0_f32.to_bits());
    assert_eq!(reach.newest.to_bits(), REACH.to_bits(), "{reach:?}");
    let (least, most) = conversation_scroll::bounds_in(reach, LEAST.height);
    assert_eq!(
        (least.abs().to_bits(), most.to_bits()),
        (0, (REACH - 600.0).to_bits())
    );
    // At its top: the page where it always was, and a bar 600 × 600 /
    // REACH long at the top of the gutter.
    let top = draw(ToRenderer::Start(Box::new(scrolled(
        0.0,
        None,
        Some(reach),
    ))));
    let at = |drawn: &Drawn, class: &str| boxes_of(&drawn.renderer, class);
    assert_eq!(at(&top, "sclip"), vec![px(0.0, 0.0, 536.0, 600.0)]);
    assert_eq!(at(&top, "spage"), vec![px(32.0, 0.0, 472.0, REACH)]);
    assert_eq!(at(&top, "prefs"), vec![px(32.0, 32.0, 472.0, REACH - 56.0)]);
    let bar = conversation_scroll::bar_in(0.0, reach, 600.0, 1.0).expect("a bar");
    assert_eq!(at(&top, "goes-on"), vec![px(517.0, 0.0, 6.0, bar.length)]);
    // Scrolled to its foot: everything up by as much, the box of *alo at
    // work* whole, 24 above the screen's foot, and the bar at the foot.
    let foot = draw(ToRenderer::Start(Box::new(scrolled(
        most,
        None,
        Some(reach),
    ))));
    assert_eq!(at(&foot, "spage"), vec![px(32.0, -most, 472.0, REACH)]);
    let limits = rects_of(&foot.renderer, "limits");
    let the_box = limits.first().expect("alo at work's box");
    assert_eq!(
        (the_box.top() + the_box.size.height).to_bits(),
        576.0_f32.to_bits()
    );
    let at_foot = conversation_scroll::bar_in(most, reach, 600.0, 1.0).expect("a bar");
    assert_eq!(
        at(&foot, "goes-on"),
        vec![px(517.0, 600.0 - at_foot.length, 6.0, at_foot.length)]
    );
}

#[test]
fn where_the_keyboard_is_stands_down_the_page_however_far_it_is_scrolled() {
    let reach = least_reach().expect("a reach");
    for down in [0.0, 120.0] {
        let mut drawn = draw(ToRenderer::Start(Box::new(scrolled(
            down,
            Some(SettingsKeyed::Asks),
            Some(reach),
        ))));
        let (_, keyed) = reached(&mut drawn).expect("a reach");
        let chosen = rects_of(&drawn.renderer, "askopt on keyed");
        let chosen = chosen.first().expect("the chosen option, keyed");
        // Measured from the page's top, which stands `down` above the
        // screen's: where it is on the page, wherever that is scrolled.
        let span = keyed.expect("the keyed thing");
        assert_eq!(
            span.top.to_bits(),
            (chosen.top() + down).to_bits(),
            "{down}"
        );
        assert_eq!(
            span.foot.to_bits(),
            (chosen.top() + chosen.size.height + down).to_bits()
        );
    }
    // Nothing keyed, nothing said.
    let mut drawn = draw(ToRenderer::Start(Box::new(scrolled(
        0.0,
        None,
        Some(reach),
    ))));
    assert_eq!(reached(&mut drawn).and_then(|(_, keyed)| keyed), None);
    // Into view: the chosen option, 16 clear of the foot, scrolled as
    // little as that takes.
    let mut drawn = draw(ToRenderer::Start(Box::new(scrolled(
        0.0,
        Some(SettingsKeyed::Asks),
        Some(reach),
    ))));
    let span = reached(&mut drawn)
        .and_then(|(_, keyed)| keyed)
        .expect("keyed");
    let down = conversation_scroll::into_view(0.0, span, 16.0, reach, 600.0);
    assert_eq!(down.to_bits(), (span.foot + 16.0 - 600.0).to_bits());
}

#[test]
fn scrolled_to_its_foot_it_is_drawn_as_the_reference_and_its_bar_read() {
    let reach = least_reach().expect("a reach");
    let (_, most) = conversation_scroll::bounds_in(reach, LEAST.height);
    let drawn = draw(ToRenderer::Start(Box::new(scrolled(
        most,
        Some(SettingsKeyed::Asks),
        Some(reach),
    ))));
    assert_eq!(
        holds("settings-scrolled-short.png", drawn.frame.as_ref()),
        Ok(())
    );
    let images = read(&drawn.tree, "image");
    let said: Vec<&str> = images
        .iter()
        .filter_map(|node| node.name.as_deref())
        .collect();
    assert!(
        said.contains(&"Settings goes on above; scrolled 100% of the way down"),
        "{said:?}"
    );
}
