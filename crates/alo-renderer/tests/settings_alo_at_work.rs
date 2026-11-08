/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 878 (`docs/design/shell.md`, *Settings*; asked by the owner
//! with 821, 8 November 2026): *Settings*' second section, **alo at
//! work** — how many sites a task may open and how many actions it may
//! take, each a stepper, and whether alo asks before opening any site or
//! only one neither found nor named.
//!
//! Each a test below: its layout in numbers under *Font*, inside the
//! design's room; its reference (`references/settings-alo-at-work.png`,
//! the section with the keyboard on the sites); a stepper's button at its
//! end drawn off and claiming nothing; the keyboard's ring on each of the
//! three moving nothing; an agent reading the numbers as spinbuttons and
//! the choice as a radio group; and a press on each claimed by its place.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test settings_alo_at_work`
//! rewrites the render reference. A rewritten reference is a diff somebody
//! reads.

use alo_box::state::Checked;
use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::agent_limits::{Asks, Limits};
use alo_renderer::conversation_claim::Claim;
use alo_renderer::frame::frame_to_png;
use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::settings_work_document::{ACTIONS, ASK, AT_WORK, EVERY, SITES, UNNAMED};
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

/// *Settings* at the design's room, alo's agent kept to `limits`, the
/// keyboard on `keyed`.
fn at_work(limits: Limits, keyed: Option<SettingsKeyed>) -> Start {
    Start {
        settings_keyed: keyed,
        limits,
        ..settings(InterfaceFace::Inter, DESIGN)
    }
}

#[test]
fn alo_at_work_stands_under_font_inside_the_designs_room() {
    let drawn = draw(ToRenderer::Start(Box::new(at_work(
        Limits::default(),
        None,
    ))));
    let at = |class: &str| boxes_of(&drawn.renderer, class);
    // 24 under the faces' foot, 576: its heading 20, its line 4 under it,
    // one line of 18; 14 under it the box, 720 wide, a line of 1 round it.
    assert_eq!(at("pgroup").get(1), Some(&px(160.0, 600.0, 720.0, 192.0)));
    assert_eq!(at("pname").get(1), Some(&px(160.0, 600.0, 720.0, 20.0)));
    assert_eq!(at("pabout").get(1), Some(&px(160.0, 624.0, 720.0, 18.0)));
    assert_eq!(at("limits"), vec![px(160.0, 656.0, 720.0, 136.0)]);
    // Three rows 16 in from either side, 686 wide: 44, then 45 and 45, each
    // of those a rule of 1 over its 44.
    assert_eq!(
        at("lrow"),
        vec![
            px(177.0, 657.0, 686.0, 44.0),
            px(177.0, 701.0, 686.0, 45.0),
            px(177.0, 746.0, 686.0, 45.0),
        ]
    );
    // Each name on 20 in the middle of its row, on a whole pixel.
    let names: Vec<String> = at("lname")
        .iter()
        .map(|name| name.split(')').next().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(names, ["(177, 669", "(177, 714", "(177, 759"]);
    // The steppers at the rows' right: 102 wide, 28 tall — Fewer 30, the
    // number 40 between rules of 1, More 30 — in the middle of their rows.
    assert_eq!(
        at("stepper"),
        vec![px(761.0, 665.0, 102.0, 28.0), px(761.0, 710.0, 102.0, 28.0)]
    );
    assert_eq!(
        at("lbtn"),
        vec![
            px(762.0, 666.0, 30.0, 26.0),
            px(832.0, 666.0, 30.0, 26.0),
            px(762.0, 711.0, 30.0, 26.0),
            px(832.0, 711.0, 30.0, 26.0),
        ]
    );
    assert_eq!(
        at("lvalue"),
        vec![px(792.0, 666.0, 40.0, 26.0), px(792.0, 711.0, 40.0, 26.0)]
    );
    // The two choices at the last row's right, 28 tall, their radios 16
    // across as the faces' are, in its middle.
    assert_eq!(at("askopts").len(), 1);
    assert!(
        at("askopts")[0].ends_with("323.20313×28") && at("askopts")[0].contains(", 755)"),
        "{:?}",
        at("askopts")
    );
    assert_eq!(
        at("dot")
            .iter()
            .skip(InterfaceFace::ALL.len())
            .filter(|dot| dot.contains(", 761) 16×16"))
            .count(),
        2
    );
    // All of it inside the design's room, 820: the page ends at 792.
    assert_eq!(at("prefs"), vec![px(160.0, 32.0, 720.0, 760.0)]);
    assert!(
        rects_of(&drawn.renderer, "limits")
            .iter()
            .all(|limits| limits.bottom() <= DESIGN.height)
    );
}

#[test]
fn a_stepper_at_its_end_is_off_and_the_rest_as_the_reference() {
    let least = Limits::checked(1, 100, Asks::Every).unwrap_or_default();
    let drawn = draw(ToRenderer::Start(Box::new(at_work(least, None))));
    let at = |class: &str| boxes_of(&drawn.renderer, class);
    // Off where it would be on: nothing moves for it.
    assert_eq!(
        at("lbtn off"),
        vec![px(762.0, 666.0, 30.0, 26.0), px(832.0, 711.0, 30.0, 26.0)]
    );
    assert_eq!(
        at("lbtn"),
        vec![px(832.0, 666.0, 30.0, 26.0), px(762.0, 711.0, 30.0, 26.0)]
    );
    let names: Vec<(Option<String>, Option<bool>)> = read(&drawn.tree, "button")
        .iter()
        .skip(1)
        .map(|node| (node.name.clone(), Some(node.states.disabled)))
        .collect();
    assert_eq!(
        names,
        vec![
            (Some("Fewer sites".into()), Some(true)),
            (Some("More sites".into()), Some(false)),
            (Some("Fewer actions".into()), Some(false)),
            (Some("More actions".into()), Some(true)),
        ]
    );
    // The default, the keyboard on the sites, drawn.
    let keyed = draw(ToRenderer::Start(Box::new(at_work(
        Limits::default(),
        Some(SettingsKeyed::Sites),
    ))));
    assert_eq!(
        holds("settings-alo-at-work.png", keyed.frame.as_ref()),
        Ok(())
    );
}

#[test]
fn the_keyboards_ring_on_each_of_the_three_moves_nothing() {
    let unkeyed = draw(ToRenderer::Start(Box::new(at_work(
        Limits::default(),
        None,
    ))));
    for (keyed, class, unkeyed_class, at) in [
        (SettingsKeyed::Sites, "stepper keyed", "stepper", 0),
        (SettingsKeyed::Actions, "stepper keyed", "stepper", 1),
        (SettingsKeyed::Asks, "askopt on keyed", "askopt on", 0),
    ] {
        let drawn = draw(ToRenderer::Start(Box::new(at_work(
            Limits::default(),
            Some(keyed),
        ))));
        assert_eq!(
            boxes_of(&drawn.renderer, class),
            boxes_of(&unkeyed.renderer, unkeyed_class)
                .get(at)
                .cloned()
                .into_iter()
                .collect::<Vec<_>>(),
            "{keyed:?}"
        );
        // Only the one ringed.
        let ringed = [
            "stepper keyed",
            "askopt on keyed",
            "face on keyed",
            "pback keyed",
        ]
        .into_iter()
        .map(|each| boxes_of(&drawn.renderer, each).len())
        .sum::<usize>();
        assert_eq!(ringed, 1, "{keyed:?}");
        // The ring: 2 outside the edge, darker than the white box it is on.
        let frame = drawn.frame.as_ref().expect("painted");
        let before = unkeyed.frame.as_ref().expect("painted");
        let rect = rects_of(&drawn.renderer, class)
            .into_iter()
            .next()
            .expect("a box");
        let middle = rect.top() + rect.size.height / 2.0;
        let ring = pixel(frame, rect.left() - 2.0, middle);
        let paper = pixel(before, rect.left() - 2.0, middle);
        assert!(ring[0] < paper[0], "{keyed:?}: {ring:?} {paper:?}");
        assert_eq!(
            pixel(frame, rect.left() - 6.0, middle),
            pixel(before, rect.left() - 6.0, middle),
            "{keyed:?}"
        );
        // Inside its edge, the white it stood on: never filled by the
        // ring (found by hand, 878).
        assert_eq!(
            pixel(frame, rect.left() + 3.0, rect.top() + 3.0),
            pixel(before, rect.left() + 3.0, rect.top() + 3.0),
            "{keyed:?}"
        );
    }
}

#[test]
fn an_agent_reads_the_numbers_as_spinbuttons_and_the_choice_as_a_radio_group() {
    let limits = Limits::checked(5, 45, Asks::Every).unwrap_or_default();
    let drawn = draw(ToRenderer::Start(Box::new(at_work(limits, None))));
    let outline = drawn.tree.to_outline();
    let region = drawn
        .tree
        .nodes()
        .into_iter()
        .find(|node| node.role.as_str() == "region" && node.name.as_deref() == Some(AT_WORK))
        .unwrap_or_else(|| panic!("no region {AT_WORK}: {outline}"));
    let inside = Snapshot {
        root: Some(region.clone()),
    };
    let spins: Vec<(Option<String>, String)> = read(&inside, "spinbutton")
        .iter()
        .map(|node| {
            (
                node.name.clone(),
                Snapshot {
                    root: Some((*node).clone()),
                }
                .to_outline(),
            )
        })
        .collect();
    assert_eq!(spins.len(), 2, "{outline}");
    assert_eq!(spins[0].0.as_deref(), Some(SITES));
    assert!(spins[0].1.contains("\"5\""), "{}", spins[0].1);
    assert_eq!(spins[1].0.as_deref(), Some(ACTIONS));
    assert!(spins[1].1.contains("\"45\""), "{}", spins[1].1);
    let groups: Vec<Option<String>> = read(&inside, "radiogroup")
        .iter()
        .map(|node| node.name.clone())
        .collect();
    assert_eq!(groups, vec![Some(ASK.into())]);
    let radios: Vec<(Option<String>, Option<Checked>)> = read(&inside, "radio")
        .iter()
        .map(|node| (node.name.clone(), node.states.checked))
        .collect();
    assert_eq!(
        radios,
        vec![
            (Some(UNNAMED.into()), Some(Checked::No)),
            (Some(EVERY.into()), Some(Checked::Yes)),
        ]
    );
    // Never a position: no rectangle is in the outline's words but its own.
    assert!(!outline.contains("Default\""), "the shown words are hidden");
}

#[test]
fn a_press_on_a_button_or_a_choice_is_claimed_by_its_place() {
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Start(Box::new(at_work(
        Limits::checked(1, 20, Asks::Unnamed).unwrap_or_default(),
        None,
    ))));
    let _ = renderer.handle(ToRenderer::Paint);
    for (x, y, claim) in [
        // Fewer sites is off at one, and claims nothing; More sites is the
        // first that can be pressed.
        (777.0, 679.0, None),
        (847.0, 679.0, Some(Claim::Limit { at: 0 })),
        (777.0, 724.0, Some(Claim::Limit { at: 1 })),
        (847.0, 724.0, Some(Claim::Limit { at: 2 })),
        // The number between them is not a button.
        (812.0, 679.0, None),
        (600.0, 769.0, Some(Claim::Asks { at: 0 })),
        (830.0, 769.0, Some(Claim::Asks { at: 1 })),
        // The row's name is nothing to press.
        (200.0, 724.0, None),
    ] {
        assert_eq!(
            renderer.handle(ToRenderer::ConversationPress(Point::new(x, y))),
            FromRenderer::ConversationClaimed(claim),
            "({x}, {y})"
        );
    }
}

/// The pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: f32, y: f32) -> [u8; 4] {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a point inside the frame, in whole pixels"
    )]
    let (x, y) = (x as u32, y as u32);
    let i = usize::try_from((y * frame.width + x) * 4).unwrap_or(0);
    let mut pixel = [0; 4];
    pixel.copy_from_slice(frame.pixels.get(i..i + 4).unwrap_or(&[0; 4]));
    pixel
}
