/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 721 (ADR 0116 § 4; `docs/design/shell.md`, *Settings*):
//! *Settings*' sixth section, **Updates** — alo's version and its
//! Chromium's, the switch *Check for updates automatically* (*On*, the
//! default, or *Off*), when alo last checked and what it found, and *Check
//! now*; or, in a copy that does not update itself, the sentence saying so.
//!
//! Each a test below: its layout in numbers under *Screen readers*,
//! scrolled to its foot; its references
//! (`references/settings-updates.png`, *Off* chosen and the keyboard on
//! *Check now*; `references/settings-updates-not-itself.png`, today's
//! builds); an agent reading it as a radio group and a button; and a press
//! on each choice and on *Check now* claimed by its place, as the
//! section's own, never as *Privacy*'s.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test settings_updates`
//! rewrites the render references. A rewritten reference is a diff
//! somebody reads.

use alo_box::state::Checked;
use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::conversation_claim::Claim;
use alo_renderer::frame::frame_to_png;
use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::settings_bus::Bus;
use alo_renderer::settings_privacy::Privacy;
use alo_renderer::settings_updates::{Automatic, CHECK_NOW, UpdatesShown, UpdatesState};
use alo_renderer::settings_updates_document::{AUTOMATIC, TITLE};
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
/// *Privacy*, *Screen readers* and *Updates* drawn under *alo at work*.
/// A page of 1784 in a room of 692.
const AT_ITS_FOOT: f32 = 1092.0;

/// The same for a copy that does not update itself, whose section is one
/// row: a page of 1611.
const NOT_ITSELF_FOOT: f32 = 919.0;

/// What a copy that updates itself shows: the switch `automatic`.
fn itself(automatic: Automatic) -> UpdatesShown {
    UpdatesShown {
        version: "alo 0.4.1, with Chromium 156.0.8078.96.".to_owned(),
        state: UpdatesState::Itself {
            automatic,
            checked: "14 November 2026 at 09:00: alo 0.4.1 is up to date.".to_owned(),
        },
    }
}

/// What every build from this repository shows today.
fn not_itself() -> UpdatesShown {
    UpdatesShown {
        version: "alo 0.0.0, with Chromium 156.0.8078.96.".to_owned(),
        state: UpdatesState::Not("This copy of alo does not update itself.".to_owned()),
    }
}

/// *Settings* at the design's room, scrolled `down`, *Updates* `shown`,
/// the keyboard on `keyed`.
fn updates(shown: UpdatesShown, keyed: Option<SettingsKeyed>, down: f32) -> Start {
    Start {
        privacy: Some(Privacy::default()),
        bus: Some(Bus::default()),
        updates: Some(shown),
        settings_keyed: keyed,
        settings_down: down,
        ..settings(InterfaceFace::Inter, DESIGN)
    }
}

#[test]
fn updates_stands_under_screen_readers_in_numbers() {
    let drawn = draw(ToRenderer::Start(Box::new(updates(
        itself(Automatic::On),
        None,
        AT_ITS_FOOT,
    ))));
    let at = |class: &str| boxes_of(&drawn.renderer, class);
    // The page, 1784 tall, scrolled up 1092 to its foot.
    assert_eq!(at("spage"), vec![px(160.0, -996.0, 720.0, 1784.0)]);
    // 24 under *Screen readers*: its heading, its version line, then the
    // box 14 under it, 664 wide.
    assert_eq!(at("pgroup").get(4), Some(&px(188.0, 468.0, 664.0, 292.0)));
    assert_eq!(at("pabout").get(4), Some(&px(188.0, 492.0, 664.0, 18.0)));
    assert_eq!(at("limits").get(3), Some(&px(188.0, 524.0, 664.0, 236.0)));
    // Two rows: the switch's, then *Last checked*, ruled off.
    let rows: Vec<String> = at("lrow prow").into_iter().skip(4).collect();
    assert_eq!(
        rows,
        vec![
            px(205.0, 525.0, 630.0, 136.0),
            px(205.0, 661.0, 630.0, 98.0)
        ]
    );
    assert_eq!(at("privopts").get(3), Some(&px(205.0, 559.0, 630.0, 92.0)));
    // Each choice 45 tall with the line under its words, 2 apart, its room
    // 8 past the row's either side.
    assert_eq!(at("privopt updopt on"), vec![px(197.0, 559.0, 646.0, 45.0)]);
    assert_eq!(at("privopt updopt"), vec![px(197.0, 606.0, 646.0, 45.0)]);
    // The lines under *On* and *Off*, 22 in from the radio's edge; then
    // what the last check found, the row's width.
    let notes: Vec<String> = at("pnote").into_iter().skip(4).collect();
    assert_eq!(
        notes,
        vec![
            px(227.0, 582.0, 234.078_13, 17.0),
            px(227.0, 629.0, 510.656_25, 17.0),
            px(205.0, 696.0, 630.0, 17.0)
        ]
    );
    // *Check now* under it, 8 below, as *Forget all sign-ins* is.
    assert_eq!(at("pbtn updbtn"), vec![px(205.0, 721.0, 96.0625, 28.0)]);
    // Each radio 16 across, 6 down in its choice.
    let radios: Vec<String> = at("dot").into_iter().skip(17).collect();
    assert_eq!(
        radios,
        vec![px(205.0, 565.0, 16.0, 16.0), px(205.0, 612.0, 16.0, 16.0)]
    );
    // The section's foot inside the panel's room, which ends at 788.
    assert!(
        rects_of(&drawn.renderer, "pgroup")
            .last()
            .is_some_and(|section| section.bottom() <= 788.0)
    );
}

#[test]
fn off_and_the_keyboard_on_check_now_as_the_reference() {
    let drawn = draw(ToRenderer::Start(Box::new(updates(
        itself(Automatic::Off),
        Some(SettingsKeyed::CheckNow),
        AT_ITS_FOOT,
    ))));
    assert_eq!(
        boxes_of(&drawn.renderer, "pbtn updbtn keyed"),
        vec![px(205.0, 721.0, 96.0625, 28.0)]
    );
    assert_eq!(
        boxes_of(&drawn.renderer, "privopt updopt on"),
        vec![px(197.0, 606.0, 646.0, 45.0)]
    );
    assert_eq!(holds("settings-updates.png", drawn.frame.as_ref()), Ok(()));
    // The ring: 2 outside *Check now*'s left edge, 205, at its middle,
    // 721 + 14, darker than the white it is on; 6 outside, nothing.
    let unkeyed = draw(ToRenderer::Start(Box::new(updates(
        itself(Automatic::Off),
        None,
        AT_ITS_FOOT,
    ))));
    let (Some(frame), Some(before)) = (drawn.frame.as_ref(), unkeyed.frame.as_ref()) else {
        panic!("nothing painted");
    };
    let ring = pixel(frame, 203, 735);
    let paper = pixel(before, 203, 735);
    assert!(ring[0] < paper[0], "{ring:?} {paper:?}");
    assert_eq!(pixel(frame, 199, 735), pixel(before, 199, 735));
}

#[test]
fn a_copy_that_does_not_update_itself_says_so_as_the_reference() {
    let drawn = draw(ToRenderer::Start(Box::new(updates(
        not_itself(),
        None,
        NOT_ITSELF_FOOT,
    ))));
    let at = |class: &str| boxes_of(&drawn.renderer, class);
    // The page, 1611 tall, scrolled up 919 to its foot; the section one
    // row: its name over the sentence, nothing to press.
    assert_eq!(at("spage"), vec![px(160.0, -823.0, 720.0, 1611.0)]);
    assert_eq!(at("pgroup").get(4), Some(&px(188.0, 641.0, 664.0, 119.0)));
    assert_eq!(at("limits").get(3), Some(&px(188.0, 697.0, 664.0, 63.0)));
    assert_eq!(at("lrow prow").get(4), Some(&px(205.0, 698.0, 630.0, 61.0)));
    assert_eq!(at("pnote").get(4), Some(&px(205.0, 732.0, 630.0, 17.0)));
    assert_eq!(at("pbtn updbtn"), Vec::<String>::new());
    assert_eq!(at("privopt updopt"), Vec::<String>::new());
    assert_eq!(
        holds("settings-updates-not-itself.png", drawn.frame.as_ref()),
        Ok(())
    );
}

#[test]
fn an_agent_reads_one_group_and_a_button() {
    let drawn = draw(ToRenderer::Start(Box::new(updates(
        itself(Automatic::On),
        None,
        AT_ITS_FOOT,
    ))));
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
    assert_eq!(groups, vec![Some(AUTOMATIC.into())]);
    let radios: Vec<(Option<String>, Option<Checked>)> = read(&inside, "radio")
        .iter()
        .map(|node| (node.name.clone(), node.states.checked))
        .collect();
    assert_eq!(
        radios,
        vec![
            (Some("On".into()), Some(Checked::Yes)),
            (Some("Off".into()), Some(Checked::No)),
        ]
    );
    let buttons: Vec<Option<String>> = read(&inside, "button")
        .iter()
        .map(|node| node.name.clone())
        .collect();
    assert_eq!(buttons, vec![Some(CHECK_NOW.into())]);
    let inside_outline = inside.to_outline();
    for said in [
        "alo 0.4.1, with Chromium 156.0.8078.96.",
        "alo 0.4.1 is up to date.",
        "It sends nothing about you.",
    ] {
        assert!(inside_outline.contains(said), "{said}: {inside_outline}");
    }
}

#[test]
fn a_press_on_each_choice_and_on_check_now_is_claimed_by_its_place_as_the_sections() {
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Start(Box::new(updates(
        itself(Automatic::On),
        None,
        AT_ITS_FOOT,
    ))));
    let _ = renderer.handle(ToRenderer::Paint);
    for (x, y, claim) in [
        (213.0, 573.0, Some(Claim::Updates { at: 0 })),
        (800.0, 640.0, Some(Claim::Updates { at: 1 })),
        (250.0, 735.0, Some(Claim::Updates { at: 2 })),
        // The row's name, the line of what was found, and the room beside
        // the button are nothing to press.
        (230.0, 545.0, None),
        (400.0, 704.0, None),
        (600.0, 735.0, None),
    ] {
        assert_eq!(
            renderer.handle(ToRenderer::ConversationPress(Point::new(x, y))),
            FromRenderer::ConversationClaimed(claim),
            "({x}, {y})"
        );
    }
}

/// The pixel at `x`, `y` of `frame`, in whole device pixels.
fn pixel(frame: &Frame, x: u32, y: u32) -> [u8; 4] {
    let i = usize::try_from((y * frame.width + x) * 4).unwrap_or(0);
    let mut pixel = [0; 4];
    pixel.copy_from_slice(frame.pixels.get(i..i + 4).unwrap_or(&[0; 4]));
    pixel
}
