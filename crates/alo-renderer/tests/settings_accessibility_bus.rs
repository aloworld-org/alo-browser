/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 989 (ADR 0115, *Phase (c)'s guard*, (a); `docs/design/shell.md`,
//! *Settings*): *Settings*' fifth section, **Screen readers** — what a
//! screen reader, or any other program the person lets read the screen,
//! may read of alo and do in it: *Read and act* (the default), *Read only*
//! or *Off*, one radio group.
//!
//! Each a test below: its layout in numbers under *Privacy*, scrolled to its
//! foot; its reference (`references/settings-accessibility-bus.png`, *Off*
//! chosen — its line saying a screen reader will not be able to read alo —
//! and the keyboard on the group); the keyboard's ring moving nothing; an
//! agent reading it as a radio group, each choice a radio by its name, the
//! one made checked, and what *Off* costs read beside it; and a press on
//! each choice claimed by its place, as the bus's own, never as *Privacy*'s.
//!
//! The section is drawn here without *Search and answers*, which stands
//! between *alo at work* and *Privacy* in the window.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test settings_accessibility_bus`
//! rewrites the render reference. A rewritten reference is a diff somebody
//! reads.

use alo_box::state::Checked;
use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::conversation_claim::Claim;
use alo_renderer::frame::frame_to_png;
use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::settings_bus::Bus;
use alo_renderer::settings_bus_document::{PROGRAMS, TITLE};
use alo_renderer::settings_privacy::Privacy;
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

/// The first screen's room in the design's window: 1280 less the panel.
const DESIGN: Size = Size {
    width: 1040.0,
    height: 820.0,
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

/// How far *Settings* scrolls in its panel at the design's room with
/// *Privacy* and *Screen readers* drawn under *alo at work*: a page of
/// 1468 in a room of 692.
const AT_ITS_FOOT: f32 = 776.0;

/// *Settings* at the design's room, scrolled to its foot, *Accessibility
/// bus* `bus`, the keyboard on `keyed`.
fn bus(bus: Bus, keyed: Option<SettingsKeyed>) -> Start {
    Start {
        privacy: Some(Privacy::default()),
        bus: Some(bus),
        settings_keyed: keyed,
        settings_down: AT_ITS_FOOT,
        ..settings(InterfaceFace::Inter, DESIGN)
    }
}

#[test]
fn the_accessibility_bus_stands_under_privacy_in_numbers() {
    let drawn = draw(ToRenderer::Start(Box::new(bus(Bus::default(), None))));
    let at = |class: &str| boxes_of(&drawn.renderer, class);
    // The page, 1468 tall, scrolled up 776 to its foot.
    assert_eq!(at("spage"), vec![px(160.0, -680.0, 720.0, 1468.0)]);
    // 24 under *Privacy*: its heading, its line of two, then the box 14
    // under it, 664 wide.
    assert_eq!(at("pgroup").get(3), Some(&px(188.0, 518.0, 664.0, 242.0)));
    assert_eq!(at("pabout").get(3), Some(&px(188.0, 542.0, 664.0, 36.0)));
    assert_eq!(at("limits").get(2), Some(&px(188.0, 592.0, 664.0, 168.0)));
    // One row: 10 above, its name 20, 4, its three choices, 10 under.
    assert_eq!(
        at("lrow prow").get(3),
        Some(&px(205.0, 593.0, 630.0, 166.0))
    );
    assert_eq!(at("privopts").get(2), Some(&px(205.0, 627.0, 630.0, 122.0)));
    // Each choice 28 tall, 2 apart, its room 8 past the row's either
    // side; *Read only* and *Off*, each with a line under its words, 45.
    assert_eq!(at("privopt busopt on"), vec![px(197.0, 627.0, 646.0, 28.0)]);
    assert_eq!(
        at("privopt busopt"),
        vec![px(197.0, 657.0, 646.0, 45.0), px(197.0, 704.0, 646.0, 45.0)]
    );
    // The lines under *Read only* and *Off*, 22 in from the radio's edge.
    let notes: Vec<String> = at("pnote").into_iter().skip(2).collect();
    assert_eq!(
        notes,
        vec![
            px(227.0, 680.0, 355.734_38, 17.0),
            px(227.0, 727.0, 256.4375, 17.0)
        ]
    );
    // Each radio 16 across, 6 down in its choice: after the faces', alo
    // at work's and *Privacy*'s.
    let radios: Vec<String> = at("dot").into_iter().skip(14).collect();
    assert_eq!(
        radios,
        vec![
            px(205.0, 633.0, 16.0, 16.0),
            px(205.0, 663.0, 16.0, 16.0),
            px(205.0, 710.0, 16.0, 16.0)
        ]
    );
    // The section's foot inside the panel's room, which ends at 788.
    assert!(
        rects_of(&drawn.renderer, "pgroup")
            .last()
            .is_some_and(|section| section.bottom() <= 788.0)
    );
}

#[test]
fn off_as_the_reference() {
    let drawn = draw(ToRenderer::Start(Box::new(bus(
        Bus::Off,
        Some(SettingsKeyed::Bus),
    ))));
    assert_eq!(
        boxes_of(&drawn.renderer, "privopt busopt on keyed"),
        vec![px(197.0, 704.0, 646.0, 45.0)]
    );
    assert_eq!(
        holds("settings-accessibility-bus.png", drawn.frame.as_ref()),
        Ok(())
    );
}

#[test]
fn the_keyboards_ring_on_the_group_moves_nothing() {
    let unkeyed = draw(ToRenderer::Start(Box::new(bus(Bus::ReadOnly, None))));
    let drawn = draw(ToRenderer::Start(Box::new(bus(
        Bus::ReadOnly,
        Some(SettingsKeyed::Bus),
    ))));
    assert_eq!(
        boxes_of(&drawn.renderer, "privopt busopt on keyed"),
        boxes_of(&unkeyed.renderer, "privopt busopt on")
    );
    assert_eq!(
        boxes_of(&drawn.renderer, "privopt on keyed"),
        Vec::<String>::new(),
        "no group of Privacy's ringed"
    );
    // The ring: 2 outside the choice's left edge, 197, at its middle,
    // 657 + 22, darker than the white it is on; 6 outside, nothing.
    assert_eq!(
        boxes_of(&drawn.renderer, "privopt busopt on keyed"),
        vec![px(197.0, 657.0, 646.0, 45.0)]
    );
    let (Some(frame), Some(before)) = (drawn.frame.as_ref(), unkeyed.frame.as_ref()) else {
        panic!("nothing painted");
    };
    let ring = pixel(frame, 195, 679);
    let paper = pixel(before, 195, 679);
    assert!(ring[0] < paper[0], "{ring:?} {paper:?}");
    assert_eq!(pixel(frame, 191, 679), pixel(before, 191, 679));
}

#[test]
fn an_agent_reads_one_group_each_choice_by_its_name() {
    let drawn = draw(ToRenderer::Start(Box::new(bus(Bus::Off, None))));
    let outline = drawn.tree.to_outline();
    let region = drawn
        .tree
        .nodes()
        .into_iter()
        .find(|node| node.role.as_str() == "region" && node.name.as_deref() == Some(TITLE))
        .unwrap_or_else(|| panic!("no region {TITLE}: {outline}"));
    let inside = Snapshot {
        root: Some(region.clone()),
    };
    let groups: Vec<Option<String>> = read(&inside, "radiogroup")
        .iter()
        .map(|node| node.name.clone())
        .collect();
    assert_eq!(groups, vec![Some(PROGRAMS.into())]);
    let radios: Vec<(Option<String>, Option<Checked>)> = read(&inside, "radio")
        .iter()
        .map(|node| (node.name.clone(), node.states.checked))
        .collect();
    assert_eq!(
        radios,
        vec![
            (Some("Read and act".into()), Some(Checked::No)),
            (Some("Read only".into()), Some(Checked::No)),
            (Some("Off".into()), Some(Checked::Yes)),
        ]
    );
    // What *Off* costs is read beside it; the words drawn beside each
    // radio are not read twice.
    let inside_outline = inside.to_outline();
    assert!(
        inside_outline.contains("A screen reader will not be able to read alo."),
        "{inside_outline}"
    );
    assert!(!inside_outline.contains("Default"), "{inside_outline}");
}

#[test]
fn a_press_on_each_choice_is_claimed_by_its_place_as_the_buss() {
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Start(Box::new(bus(Bus::default(), None))));
    let _ = renderer.handle(ToRenderer::Paint);
    for (x, y, claim) in [
        (213.0, 641.0, Some(Claim::Bus { at: 0 })),
        (400.0, 680.0, Some(Claim::Bus { at: 1 })),
        (800.0, 726.0, Some(Claim::Bus { at: 2 })),
        // The row's name and the room between two choices are nothing to
        // press.
        (230.0, 613.0, None),
        (300.0, 656.0, None),
    ] {
        assert_eq!(
            renderer.handle(ToRenderer::ConversationPress(Point::new(x, y))),
            FromRenderer::ConversationClaimed(claim),
            "({x}, {y})"
        );
    }
    // What the browser process lists for the same places.
    assert_eq!(Bus::ALL, [Bus::ReadAndAct, Bus::ReadOnly, Bus::Off]);
}

/// The pixel at `x`, `y` of `frame`, in whole device pixels.
fn pixel(frame: &Frame, x: u32, y: u32) -> [u8; 4] {
    let i = usize::try_from((y * frame.width + x) * 4).unwrap_or(0);
    let mut pixel = [0; 4];
    pixel.copy_from_slice(frame.pixels.get(i..i + 4).unwrap_or(&[0; 4]));
    pixel
}
