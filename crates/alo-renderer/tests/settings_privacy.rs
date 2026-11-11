/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 920 (ADR 0108, `docs/design/shell.md`, *Settings*):
//! *Settings*' fourth section, **Privacy** — what sites' cookies the
//! Chromium alo drives keeps, and whether alo answers a site's cookie
//! banner with its most private choice, each a radio group.
//!
//! Each a test below: its layout in numbers under *alo at work*, scrolled
//! to its foot; its reference (`references/settings-privacy.png`, *Refuse
//! all cookies* chosen — its line saying signing in will fail — and the
//! keyboard on *Cookies*); the keyboard's ring on either group moving
//! nothing; an agent reading both as radio groups, each choice a radio by
//! its name, the one made checked; and a press on each choice claimed by
//! its place. And the row *Sign-ins* (ADR 0111): its numbers, the ring on
//! *Forget all sign-ins*, the button read by its name and pressed at the
//! place after the choices, and its reference
//! (`references/settings-sign-ins.png`, just forgotten, the keyboard on
//! the button).
//!
//! The section is drawn here without *Search and answers*, which stands
//! between them in the window (its numbers are `settings_search_and_answers.rs`'s).
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test settings_privacy`
//! rewrites the render reference. A rewritten reference is a diff somebody
//! reads.

use alo_box::state::Checked;
use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::conversation_claim::Claim;
use alo_renderer::frame::frame_to_png;
use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::settings_privacy::{
    Banners, Cookies, FORGET_ALL, Privacy, PrivacyPress, SignInsShown,
};
use alo_renderer::settings_privacy_document::{BANNERS, COOKIES, TITLE};
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
/// *Privacy* drawn under *alo at work*: a page of 1202 in a room of 692.
const AT_ITS_FOOT: f32 = 510.0;

/// *Settings* at the design's room, scrolled to its foot, *Privacy*'s
/// choices `made`, the keyboard on `keyed`.
fn privacy(made: Privacy, keyed: Option<SettingsKeyed>) -> Start {
    Start {
        privacy: Some(made),
        settings_keyed: keyed,
        settings_down: AT_ITS_FOOT,
        ..settings(InterfaceFace::Inter, DESIGN)
    }
}

/// *Refuse all cookies* and *Leave them to me* chosen.
const OTHERWISE: Privacy = Privacy {
    cookies: Cookies::RefuseAll,
    banners: Banners::Leave,
};

#[test]
fn privacy_stands_under_alo_at_work_in_numbers() {
    let drawn = draw(ToRenderer::Start(Box::new(privacy(
        Privacy::default(),
        None,
    ))));
    let at = |class: &str| boxes_of(&drawn.renderer, class);
    // The page, 1202 tall, scrolled up 510 to its foot.
    assert_eq!(at("spage"), vec![px(160.0, -414.0, 720.0, 1202.0)]);
    // 24 under *alo at work*: its heading, its line of two, then the box
    // 14 under it, 664 wide.
    assert_eq!(at("pgroup").get(2), Some(&px(188.0, 334.0, 664.0, 426.0)));
    assert_eq!(at("pabout").get(2), Some(&px(188.0, 358.0, 664.0, 36.0)));
    assert_eq!(at("limits").get(1), Some(&px(188.0, 408.0, 664.0, 352.0)));
    // Three rows, each after the first ruled off by its own pixel: 10
    // above, the row's name 20, 4, its choices, 10 under — and *Sign-ins*
    // (ADR 0111): its name, 4, its line 17, 4, the button 28 four under
    // it, 10.
    assert_eq!(
        at("lrow prow"),
        vec![
            px(205.0, 409.0, 630.0, 149.0),
            px(205.0, 558.0, 630.0, 103.0),
            px(205.0, 661.0, 630.0, 98.0)
        ]
    );
    // *Forget all sign-ins* at the row's left, as wide as its words.
    assert_eq!(at("pbtn"), vec![px(205.0, 721.0, 138.421_88, 28.0)]);
    assert_eq!(
        at("privopts"),
        vec![
            px(205.0, 443.0, 630.0, 105.0),
            px(205.0, 593.0, 630.0, 58.0)
        ]
    );
    // Each choice 28 tall, 2 apart, its room 8 past the row's either
    // side; the one with a line under its words 45.
    assert_eq!(
        at("privopt on"),
        vec![px(197.0, 443.0, 646.0, 28.0), px(197.0, 593.0, 646.0, 28.0)]
    );
    assert_eq!(
        at("privopt"),
        vec![
            px(197.0, 473.0, 646.0, 45.0),
            px(197.0, 520.0, 646.0, 28.0),
            px(197.0, 623.0, 646.0, 28.0)
        ]
    );
    // The line under *Refuse all cookies*, 22 in from the radio's edge.
    // And *Sign-ins*' line, on one line at the row's left.
    assert_eq!(
        at("pnote"),
        vec![
            px(227.0, 496.0, 323.531_25, 17.0),
            px(205.0, 696.0, 630.0, 17.0)
        ]
    );
    // Each radio 16 across, 6 down in its choice: after the six faces'
    // (queue item 975) and alo at work's.
    let radios: Vec<String> = at("dot").into_iter().skip(9).collect();
    assert_eq!(
        radios,
        vec![
            px(205.0, 449.0, 16.0, 16.0),
            px(205.0, 479.0, 16.0, 16.0),
            px(205.0, 526.0, 16.0, 16.0),
            px(205.0, 599.0, 16.0, 16.0),
            px(205.0, 629.0, 16.0, 16.0)
        ]
    );
    assert!(
        rects_of(&drawn.renderer, "limits")
            .iter()
            .all(|limits| limits.bottom() <= 788.0)
    );
}

#[test]
fn privacy_as_the_reference() {
    let drawn = draw(ToRenderer::Start(Box::new(privacy(
        OTHERWISE,
        Some(SettingsKeyed::Cookies),
    ))));
    assert_eq!(
        boxes_of(&drawn.renderer, "privopt on keyed"),
        vec![px(197.0, 473.0, 646.0, 45.0)]
    );
    assert_eq!(holds("settings-privacy.png", drawn.frame.as_ref()), Ok(()));
}

#[test]
fn the_keyboards_ring_on_either_group_moves_nothing() {
    let unkeyed = draw(ToRenderer::Start(Box::new(privacy(OTHERWISE, None))));
    for (keyed, at) in [(SettingsKeyed::Cookies, 0), (SettingsKeyed::Banners, 1)] {
        let drawn = draw(ToRenderer::Start(Box::new(privacy(OTHERWISE, Some(keyed)))));
        assert_eq!(
            boxes_of(&drawn.renderer, "privopt on keyed"),
            boxes_of(&unkeyed.renderer, "privopt on")
                .get(at)
                .cloned()
                .into_iter()
                .collect::<Vec<_>>(),
            "{keyed:?}"
        );
        assert_eq!(
            boxes_of(&drawn.renderer, "privopt on").len(),
            1,
            "{keyed:?}: one group ringed"
        );
        // The ring: 2 outside the edge, darker than the white it is on;
        // 6 outside, nothing.
        let (Some(frame), Some(before)) = (drawn.frame.as_ref(), unkeyed.frame.as_ref()) else {
            panic!("{keyed:?}: nothing painted");
        };
        let Some(rect) = rects_of(&drawn.renderer, "privopt on keyed")
            .into_iter()
            .next()
        else {
            panic!("{keyed:?}: no ringed choice");
        };
        let middle = rect.top() + rect.size.height / 2.0;
        let ring = pixel(frame, rect.left() - 2.0, middle);
        let paper = pixel(before, rect.left() - 2.0, middle);
        assert!(ring[0] < paper[0], "{keyed:?}: {ring:?} {paper:?}");
        assert_eq!(
            pixel(frame, rect.left() - 6.0, middle),
            pixel(before, rect.left() - 6.0, middle),
            "{keyed:?}"
        );
    }
}

#[test]
fn an_agent_reads_both_groups_each_choice_by_its_name() {
    let drawn = draw(ToRenderer::Start(Box::new(privacy(OTHERWISE, None))));
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
    assert_eq!(groups, vec![Some(COOKIES.into()), Some(BANNERS.into())]);
    let radios: Vec<(Option<String>, Option<Checked>)> = read(&inside, "radio")
        .iter()
        .map(|node| (node.name.clone(), node.states.checked))
        .collect();
    assert_eq!(
        radios,
        vec![
            (
                Some("Refuse other sites\u{2019} cookies".into()),
                Some(Checked::No)
            ),
            (Some("Refuse all cookies".into()), Some(Checked::Yes)),
            (Some("Keep all cookies".into()), Some(Checked::No)),
            (
                Some("Answer with the most private choice".into()),
                Some(Checked::No)
            ),
            (Some("Leave them to me".into()), Some(Checked::Yes)),
        ]
    );
    // What refusing every site's costs is read beside it; the words drawn
    // beside each radio are not read twice.
    let inside_outline = inside.to_outline();
    assert!(
        inside_outline.contains("Signing in to sites will fail"),
        "{inside_outline}"
    );
    assert!(!inside_outline.contains("Default"), "{inside_outline}");
    // *Forget all sign-ins*, a button by its name, and the line saying
    // where the sign-ins are, read (ADR 0111).
    let buttons: Vec<Option<String>> = read(&inside, "button")
        .iter()
        .map(|node| node.name.clone())
        .collect();
    assert_eq!(buttons, vec![Some(FORGET_ALL.into())]);
    assert!(
        inside_outline.contains(SignInsShown::Kept.said()),
        "{inside_outline}"
    );
}

#[test]
fn the_keyboards_ring_on_forget_all_sign_ins_moves_nothing() {
    let unkeyed = draw(ToRenderer::Start(Box::new(privacy(OTHERWISE, None))));
    let drawn = draw(ToRenderer::Start(Box::new(privacy(
        OTHERWISE,
        Some(SettingsKeyed::ForgetSignIns),
    ))));
    assert_eq!(
        boxes_of(&drawn.renderer, "pbtn keyed"),
        boxes_of(&unkeyed.renderer, "pbtn")
    );
    assert_eq!(
        boxes_of(&drawn.renderer, "privopt on keyed"),
        Vec::<String>::new()
    );
    let (Some(frame), Some(before)) = (drawn.frame.as_ref(), unkeyed.frame.as_ref()) else {
        panic!("nothing painted");
    };
    let Some(rect) = rects_of(&drawn.renderer, "pbtn keyed").into_iter().next() else {
        panic!("no ringed button");
    };
    let middle = rect.top() + rect.size.height / 2.0;
    let ring = pixel(frame, rect.left() - 2.0, middle);
    let paper = pixel(before, rect.left() - 2.0, middle);
    assert!(ring[0] < paper[0], "{ring:?} {paper:?}");
    assert_eq!(
        pixel(frame, rect.left() - 6.0, middle),
        pixel(before, rect.left() - 6.0, middle)
    );
}

#[test]
fn sign_ins_forgotten_as_the_reference() {
    let drawn = draw(ToRenderer::Start(Box::new(Start {
        sign_ins: SignInsShown::Forgotten,
        ..privacy(Privacy::default(), Some(SettingsKeyed::ForgetSignIns))
    })));
    assert_eq!(
        boxes_of(&drawn.renderer, "pbtn keyed"),
        vec![px(205.0, 721.0, 138.421_88, 28.0)],
        "the longer line still on one"
    );
    assert_eq!(holds("settings-sign-ins.png", drawn.frame.as_ref()), Ok(()));
}

#[test]
fn a_press_on_each_choice_is_claimed_by_its_place() {
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Start(Box::new(privacy(
        Privacy::default(),
        None,
    ))));
    let _ = renderer.handle(ToRenderer::Paint);
    for (x, y, claim) in [
        (213.0, 457.0, Some(Claim::Privacy { at: 0 })),
        (400.0, 502.0, Some(Claim::Privacy { at: 1 })),
        (800.0, 534.0, Some(Claim::Privacy { at: 2 })),
        (300.0, 607.0, Some(Claim::Privacy { at: 3 })),
        (213.0, 637.0, Some(Claim::Privacy { at: 4 })),
        // *Forget all sign-ins*, after them all (ADR 0111).
        (260.0, 735.0, Some(Claim::Privacy { at: 5 })),
        // A row's name, the box between choices, and *Sign-ins*' line,
        // are nothing to press.
        (500.0, 704.0, None),
        (230.0, 429.0, None),
        (300.0, 519.0, None),
    ] {
        assert_eq!(
            renderer.handle(ToRenderer::ConversationPress(Point::new(x, y))),
            FromRenderer::ConversationClaimed(claim),
            "({x}, {y})"
        );
    }
    // What the browser process lists for the same places.
    assert_eq!(
        Privacy::pressables(),
        vec![
            PrivacyPress::Cookies(Cookies::RefuseOthers),
            PrivacyPress::Cookies(Cookies::RefuseAll),
            PrivacyPress::Cookies(Cookies::KeepAll),
            PrivacyPress::Banners(Banners::Answer),
            PrivacyPress::Banners(Banners::Leave),
            PrivacyPress::ForgetSignIns,
        ]
    );
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
