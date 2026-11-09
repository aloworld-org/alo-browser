/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 905 (ADR 0106, `docs/design/shell.md`, *Settings*):
//! *Settings*' third section, **Search and answers** — which search
//! service and which AI service alo asks, chosen by name in two radio
//! groups, a `SearXNG`'s address, and a person's own key for each service
//! that takes one, drawn as dots.
//!
//! Each a test below: its layout in numbers under *alo at work*, scrolled
//! to its foot; its reference (`references/settings-search-and-answers.png`,
//! `SearXNG` chosen and its address kept, the keyboard in the AI key's
//! field, a key typed and a person's own key kept); the keyboard's ring on
//! a chooser, a field and a button moving nothing; an agent reading the
//! choosers as radio groups and the fields as text boxes — never a key's
//! dots; and a press on each claimed by its place.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test settings_search_and_answers`
//! rewrites the render reference. A rewritten reference is a diff somebody
//! reads.

use alo_box::state::Checked;
use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::conversation_claim::Claim;
use alo_renderer::frame::frame_to_png;
use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::settings_services::{Entry, EntryRow, ServicePress, ServicesShown, Typed, dots};
use alo_renderer::settings_services_document::{ANSWERS_BY, SEARCH_WITH, TITLE};
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

/// How far *Settings* scrolls in its panel at the design's room with the
/// section drawn as [`shown`] says: a page of 1095 in a room of 692.
const AT_ITS_FOOT: f32 = 403.0;

/// What the browser process would send: `SearXNG` chosen, its address
/// kept; the keyboard in the AI key's field, twelve characters typed, and
/// a person's own AI key kept — never a key, only dots.
fn shown() -> ServicesShown {
    ServicesShown {
        search: vec!["Brave Search".into(), "SearXNG".into()],
        search_on: 1,
        answers: vec!["OpenAI".into()],
        answers_on: 0,
        search_rows: vec![EntryRow {
            entry: Entry::Address,
            typed: Typed {
                shown: "https://search.example.org/".into(),
                focused: false,
            },
            state: "Searches go to https://search.example.org/.".into(),
            removable: false,
        }],
        answers_rows: vec![EntryRow {
            entry: Entry::AiKey,
            typed: Typed {
                shown: dots(12),
                focused: true,
            },
            state: "Your own key is used, kept in this Mac\u{2019}s keychain.".into(),
            removable: true,
        }],
        said: Some(
            "Your key for the AI service is kept in this Mac\u{2019}s keychain. alo uses it \
             from its next start."
                .into(),
        ),
    }
}

/// *Settings* at the design's room, scrolled to its foot, the keyboard on
/// `keyed`.
fn services(keyed: Option<ServicePress>) -> Start {
    Start {
        services: Some(shown()),
        settings_keyed: keyed.map(SettingsKeyed::Service),
        settings_down: AT_ITS_FOOT,
        ..settings(InterfaceFace::Inter, DESIGN)
    }
}

#[test]
fn search_and_answers_stands_under_alo_at_work_in_numbers() {
    let drawn = draw(ToRenderer::Start(Box::new(services(None))));
    let at = |class: &str| boxes_of(&drawn.renderer, class);
    // The page, 1095 tall, scrolled up 403 to its foot.
    assert_eq!(at("spage"), vec![px(160.0, -307.0, 720.0, 1095.0)]);
    // 24 under *alo at work*: its heading, its line, then the box 14 under
    // it, 664 wide, and what was said 10 under the box.
    assert_eq!(at("pgroup").get(2), Some(&px(188.0, 441.0, 664.0, 319.0)));
    assert_eq!(at("limits").get(1), Some(&px(188.0, 497.0, 664.0, 235.0)));
    assert_eq!(at("psaid"), vec![px(188.0, 742.0, 664.0, 18.0)]);
    // The two choosers' rows, 44 and 45 as *alo at work*'s are, and each
    // field's row 72: 10 above, the field 28, 6, its sentence 17, 10 under.
    assert_eq!(
        at("lrow").get(3..),
        Some(&[px(205.0, 498.0, 630.0, 44.0), px(205.0, 614.0, 630.0, 45.0)][..])
    );
    assert_eq!(
        at("lrow erow"),
        vec![px(205.0, 542.0, 630.0, 72.0), px(205.0, 659.0, 630.0, 72.0)]
    );
    // The services at their rows' right, 28 tall, the one chosen on.
    assert_eq!(at("svc"), vec![px(628.453_1, 506.0, 119.9375, 28.0)]);
    assert_eq!(
        at("svc on"),
        vec![
            px(748.390_6, 506.0, 94.609_375, 28.0),
            px(759.375, 623.0, 83.625, 28.0)
        ]
    );
    // Each field 240 × 28, *Keep* and *Remove* 8 beside it, at the row's
    // right; its sentence under them, the row's whole width.
    assert_eq!(at("efield"), vec![px(528.875, 553.0, 240.0, 28.0)]);
    assert_eq!(at("efield on"), vec![px(444.406_25, 670.0, 240.0, 28.0)]);
    assert_eq!(
        at("kbtn"),
        vec![
            px(776.875, 553.0, 58.125, 28.0),
            px(692.406_25, 670.0, 58.125, 28.0),
            px(758.531_25, 670.0, 76.468_75, 28.0)
        ]
    );
    assert_eq!(
        at("estate"),
        vec![px(205.0, 587.0, 630.0, 17.0), px(205.0, 704.0, 630.0, 17.0)]
    );
    // The caret, 1 × 16, after the twelve dots, in the field's middle.
    assert_eq!(at("ecaret"), vec![px(543.156_25, 676.0, 1.0, 16.0)]);
    assert!(
        rects_of(&drawn.renderer, "limits")
            .iter()
            .all(|limits| limits.bottom() <= 788.0)
    );
}

#[test]
fn search_and_answers_as_the_reference() {
    let drawn = draw(ToRenderer::Start(Box::new(services(Some(
        ServicePress::Field(Entry::AiKey),
    )))));
    assert_eq!(
        boxes_of(&drawn.renderer, "efield on keyed"),
        vec![px(444.406_25, 670.0, 240.0, 28.0)]
    );
    assert_eq!(
        holds("settings-search-and-answers.png", drawn.frame.as_ref()),
        Ok(())
    );
}

#[test]
fn the_keyboards_ring_on_a_chooser_a_field_or_a_button_moves_nothing() {
    let unkeyed = draw(ToRenderer::Start(Box::new(services(None))));
    for (keyed, class, unkeyed_class, at) in [
        (ServicePress::Search(1), "svc on keyed", "svc on", 0),
        (ServicePress::Answers(0), "svc on keyed", "svc on", 1),
        (
            ServicePress::Field(Entry::Address),
            "efield keyed",
            "efield",
            0,
        ),
        (ServicePress::Keep(Entry::Address), "kbtn keyed", "kbtn", 0),
        (ServicePress::Remove(Entry::AiKey), "kbtn keyed", "kbtn", 2),
    ] {
        let drawn = draw(ToRenderer::Start(Box::new(services(Some(keyed)))));
        assert_eq!(
            boxes_of(&drawn.renderer, class),
            boxes_of(&unkeyed.renderer, unkeyed_class)
                .get(at)
                .cloned()
                .into_iter()
                .collect::<Vec<_>>(),
            "{keyed:?}"
        );
        let ringed = [
            "svc on keyed",
            "efield keyed",
            "efield on keyed",
            "kbtn keyed",
        ]
        .into_iter()
        .map(|each| boxes_of(&drawn.renderer, each).len())
        .sum::<usize>();
        assert_eq!(ringed, 1, "{keyed:?}");
        // The ring: 2 outside the edge, darker than the white it is on.
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
    }
}

#[test]
fn an_agent_reads_the_choosers_and_the_fields_and_never_a_keys_dots() {
    let drawn = draw(ToRenderer::Start(Box::new(services(None))));
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
    assert_eq!(
        groups,
        vec![Some(SEARCH_WITH.into()), Some(ANSWERS_BY.into())]
    );
    let radios: Vec<(Option<String>, Option<Checked>)> = read(&inside, "radio")
        .iter()
        .map(|node| (node.name.clone(), node.states.checked))
        .collect();
    assert_eq!(
        radios,
        vec![
            (Some("Brave Search".into()), Some(Checked::No)),
            (Some("SearXNG".into()), Some(Checked::Yes)),
            (Some("OpenAI".into()), Some(Checked::Yes)),
        ]
    );
    let boxes: Vec<(Option<String>, String)> = read(&inside, "textbox")
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
    assert_eq!(boxes.len(), 2, "{outline}");
    assert_eq!(boxes[0].0.as_deref(), Some("Address"));
    assert!(
        boxes[0].1.contains("https://search.example.org/"),
        "{}",
        boxes[0].1
    );
    assert_eq!(boxes[1].0.as_deref(), Some("AI key"));
    assert!(
        !outline.contains('\u{2022}'),
        "a key's dots are read: {outline}"
    );
    let buttons: Vec<Option<String>> = read(&inside, "button")
        .iter()
        .map(|node| node.name.clone())
        .collect();
    assert_eq!(
        buttons,
        vec![
            Some("Keep the address".into()),
            Some("Keep your AI key".into()),
            Some("Remove your AI key".into())
        ]
    );
}

#[test]
fn a_press_on_a_service_a_field_or_a_button_is_claimed_by_its_place() {
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Start(Box::new(services(None))));
    let _ = renderer.handle(ToRenderer::Paint);
    for (x, y, claim) in [
        (688.0, 520.0, Some(Claim::Service { at: 0 })),
        (795.0, 520.0, Some(Claim::Service { at: 1 })),
        (648.0, 567.0, Some(Claim::Service { at: 2 })),
        (806.0, 567.0, Some(Claim::Service { at: 3 })),
        (801.0, 637.0, Some(Claim::Service { at: 4 })),
        (564.0, 684.0, Some(Claim::Service { at: 5 })),
        (721.0, 684.0, Some(Claim::Service { at: 6 })),
        (796.0, 684.0, Some(Claim::Service { at: 7 })),
        // A row's name, and the sentence under a field, are nothing to
        // press.
        (232.0, 567.0, None),
        (300.0, 595.0, None),
    ] {
        assert_eq!(
            renderer.handle(ToRenderer::ConversationPress(Point::new(x, y))),
            FromRenderer::ConversationClaimed(claim),
            "({x}, {y})"
        );
    }
    // What the browser process lists for the same places.
    assert_eq!(
        shown().pressables(),
        vec![
            ServicePress::Search(0),
            ServicePress::Search(1),
            ServicePress::Field(Entry::Address),
            ServicePress::Keep(Entry::Address),
            ServicePress::Answers(0),
            ServicePress::Field(Entry::AiKey),
            ServicePress::Keep(Entry::AiKey),
            ServicePress::Remove(Entry::AiKey),
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
