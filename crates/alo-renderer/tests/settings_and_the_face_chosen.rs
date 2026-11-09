/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 854 (`docs/design/shell.md`, *Settings*; approved by the
//! owner 2026-11-07): *Settings*, with its section **Font** — the five
//! faces the interface's words may be set in, each a sample set in that
//! face (since queue item 901 the owner's one sentence, [`SAMPLE`]) — and both parts of the interface drawn in the face chosen;
//! since queue item 886 (approved 2026-11-08) a dialog over the chat on
//! screen, which is blurred and dimmed under a veil around it.
//!
//! What closes its engine half, each a test below: what an agent reads — a
//! dialog named *Settings*, its *Close*, a radio group named *Font*, a
//! radio named by each face, the one chosen checked, nothing of the chat
//! under it, and the panel's *Settings* current; a layout assertion in
//! numbers at the design's room and the least window's, and every face's
//! sample whole at the design's; a press on each face and on its × claimed
//! as itself, one on the veil as its × and never as what is under it, and
//! one on the dialog's own room as nothing; a reference render of it,
//! `references/settings.png` and `settings-in-the-least-window.png`; and
//! for each face a small reference of a search's result and of the panel
//! set in it, `references/face-<face>-result.png` and
//! `face-<face>-panel.png`, so a face that breaks a line says so.
//!
//! Queue item 856: the keyboard on its × or on the radio group is drawn as
//! a ring outside the edge of the button or of the face chosen, moving
//! nothing (a layout assertion), and as the references
//! `settings-keyed-close.png` and `settings-keyed-font.png`.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test settings_and_the_face_chosen`
//! rewrites the references. A rewritten reference is a diff somebody reads.

use alo_box::state::Checked;
use alo_layout::{Rect, Size};
use alo_paint::{DisplayItem, Point, from_png};
use alo_renderer::conversation_claim::Claim;
use alo_renderer::frame::frame_to_png;
use alo_renderer::interface_face::InterfaceFace;
use alo_renderer::panel::{Chat, Doing, Panel};
use alo_renderer::results::{Found, Results, SetUp, Showing};
use alo_renderer::settings_document::{CLOSE, FONT, SAMPLE, TITLE};
use alo_renderer::settings_work_document::AT_WORK;
use alo_renderer::start::{SettingsKeyed, Start};
use alo_renderer::turn::Turn;
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

/// Its room in the least window, 600 × 600 beside the rail.
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

/// Each run of text drawn: what it says, its font's family, and the
/// baseline it stands on.
fn runs(renderer: &Renderer) -> Vec<(String, String, f32)> {
    renderer
        .rendered()
        .map(|drawing| {
            drawing
                .display
                .items()
                .iter()
                .filter_map(|item| match item {
                    DisplayItem::Text {
                        text, font, origin, ..
                    } => Some((text.clone(), font.family().to_owned(), origin.1)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
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

#[test]
fn settings_is_read_as_a_radio_group_named_font_with_the_face_chosen_checked() {
    for chosen in InterfaceFace::ALL {
        let drawn = draw(ToRenderer::Start(Box::new(settings(chosen, DESIGN))));
        let named = |role: &str| -> Vec<Option<String>> {
            read(&drawn.tree, role)
                .iter()
                .map(|node| node.name.clone())
                .collect()
        };
        // Since queue item 878, alo at work under Font: its own test reads
        // it (`settings_alo_at_work.rs`).
        // A dialog named Settings over the chat (queue item 886).
        assert_eq!(named("dialog"), vec![Some(TITLE.into())]);
        assert_eq!(
            named("region"),
            vec![Some(FONT.into()), Some(AT_WORK.into())]
        );
        assert_eq!(
            named("heading"),
            vec![Some(TITLE.into()), Some(FONT.into()), Some(AT_WORK.into())]
        );
        assert_eq!(named("button").first(), Some(&Some(CLOSE.into())));
        assert_eq!(named("radiogroup").first(), Some(&Some(FONT.into())));
        let radios: Vec<(Option<String>, Option<Checked>, usize)> = read(&drawn.tree, "radio")
            .iter()
            .take(InterfaceFace::ALL.len())
            .map(|node| (node.name.clone(), node.states.checked, node.children.len()))
            .collect();
        assert_eq!(
            radios,
            InterfaceFace::ALL
                .iter()
                .map(|face| {
                    let checked = if *face == chosen {
                        Checked::Yes
                    } else {
                        Checked::No
                    };
                    (Some(face.name().to_owned()), Some(checked), 0)
                })
                .collect::<Vec<_>>(),
            "each named by its face alone, the sample hidden"
        );
        // Nothing of the chat under it: no box, no conversation, no
        // greeting.
        assert!(read(&drawn.tree, "textbox").is_empty() && read(&drawn.tree, "search").is_empty());
        assert!(
            !drawn
                .tree
                .nodes()
                .iter()
                .any(|node| node.name.as_deref() == Some("Hello."))
        );
    }
}

#[test]
fn settings_is_laid_out_in_a_panel_over_the_screen_two_faces_a_row() {
    let drawn = draw(ToRenderer::Start(Box::new(settings(
        InterfaceFace::Inter,
        DESIGN,
    ))));
    let at = |class: &str| boxes_of(&drawn.renderer, class);
    // The veil the whole screen; the panel 720 wide, centred in 1040, as
    // tall as the room less 32 above and below: 756 (queue item 886).
    assert_eq!(at("veil"), vec![px(0.0, 0.0, 1040.0, 820.0)]);
    assert_eq!(at("sdialog"), vec![px(160.0, 32.0, 720.0, 756.0)]);
    // *Settings* and its × in a row of 36, 20 down, 28 in and 20 from the
    // right; what scrolls from 96 down to the panel's foot, the room the
    // browser process scrolls it in.
    assert_eq!(at("phead"), vec![px(188.0, 52.0, 672.0, 36.0)]);
    assert_eq!(at("pclose"), vec![px(824.0, 52.0, 36.0, 36.0)]);
    let room = alo_renderer::conversation_scroll::Room::settings(DESIGN);
    assert_eq!(
        at("sclip"),
        vec![px(160.0, room.top, 720.0, room.height)],
        "{room:?}"
    );
    assert_eq!(at("spage"), vec![px(160.0, 96.0, 720.0, 752.0)]);
    // Font 24 down it, its line 4 under it, the faces 14 under that.
    assert_eq!(at("pname").first(), Some(&px(188.0, 120.0, 664.0, 20.0)));
    assert_eq!(at("pabout").first(), Some(&px(188.0, 144.0, 664.0, 18.0)));
    // Two a row, 10 apart: (664 - 10) / 2 = 327 each; each 15 + 18 + 10 +
    // 75 (the sample's three lines of 25) + 16 and its border, 136 tall;
    // three rows.
    assert_eq!(at("faces"), vec![px(188.0, 176.0, 664.0, 428.0)]);
    let faces: Vec<String> = at("face on").into_iter().chain(at("face")).collect();
    assert_eq!(
        faces,
        vec![
            px(188.0, 176.0, 327.0, 136.0),
            px(525.0, 176.0, 327.0, 136.0),
            px(188.0, 322.0, 327.0, 136.0),
            px(525.0, 322.0, 327.0, 136.0),
            px(188.0, 468.0, 327.0, 136.0),
        ]
    );
    // Inside each, 1 of border and 15 down, 16 in: the name's row of 18,
    // its circle 16 centred in it; 10 under it the sample's three lines of
    // 25 (queue item 901).
    assert_eq!(at("dot").first(), Some(&px(205.0, 193.0, 16.0, 16.0)));
    assert_eq!(
        at("sample"),
        vec![
            px(205.0, 220.0, 293.0, 75.0),
            px(542.0, 220.0, 293.0, 75.0),
            px(205.0, 366.0, 293.0, 75.0),
            px(542.0, 366.0, 293.0, 75.0),
            px(205.0, 512.0, 293.0, 75.0)
        ]
    );
}

#[test]
fn in_the_least_window_every_face_stands_inside_the_panel() {
    let drawn = draw(ToRenderer::Start(Box::new(settings(
        InterfaceFace::Inter,
        LEAST,
    ))));
    let at = |class: &str| boxes_of(&drawn.renderer, class);
    // The room less 32 either side and above and below: 472 by 536; two
    // faces of 203 a row; what scrolls 472 tall, as the browser process
    // scrolls it.
    assert_eq!(at("sdialog"), vec![px(32.0, 32.0, 472.0, 536.0)]);
    assert_eq!(at("phead"), vec![px(60.0, 52.0, 424.0, 36.0)]);
    assert_eq!(at("sclip"), vec![px(32.0, 96.0, 472.0, 472.0)]);
    assert_eq!(
        alo_renderer::conversation_scroll::Room::settings(LEAST)
            .height
            .to_bits(),
        472.0_f32.to_bits()
    );
    let faces: Vec<Rect> = rects_of(&drawn.renderer, "face on")
        .into_iter()
        .chain(rects_of(&drawn.renderer, "face"))
        .collect();
    assert_eq!(faces.len(), 5);
    for face in &faces {
        assert!(face.left() >= 60.0 && face.right() <= 476.0, "{face:?}");
    }
    assert_eq!(
        faces.last().map(|face| units(*face)),
        // Its sample on four lines of 25 there, whole, never cut: 161.
        Some(px(60.0, 536.0, 203.0, 161.0))
    );
}

#[test]
fn at_the_designs_room_each_faces_sample_is_whole_in_that_face() {
    // In whichever face the page is set, each sample is in its own, on
    // its lines, none of it cut off (queue item 901).
    for chosen in [InterfaceFace::Inter, InterfaceFace::Geist] {
        let drawn = draw(ToRenderer::Start(Box::new(settings(chosen, DESIGN))));
        let runs = runs(&drawn.renderer);
        for face in InterfaceFace::ALL {
            let in_face: Vec<&(String, String, f32)> = runs
                .iter()
                .filter(|(_, family, _)| family == face.family())
                .collect();
            let lines: Vec<f32> = in_face
                .iter()
                .filter(|(text, _, _)| SAMPLE.contains(text.trim()))
                .filter(|(text, _, _)| !text.trim().is_empty())
                .map(|(_, _, baseline)| *baseline)
                .fold(Vec::new(), |mut lines, baseline| {
                    if !lines.iter().any(|line: &f32| (line - baseline).abs() < 0.5) {
                        lines.push(baseline);
                    }
                    lines
                });
            // Wider faces take three lines of the room, narrower two; every face
            // is as tall, the room three lines'.
            let wide = matches!(face, InterfaceFace::Inter | InterfaceFace::DmSans);
            assert_eq!(
                lines.len(),
                if wide { 3 } else { 2 },
                "{face:?}: {in_face:?}"
            );
            let words: String = in_face
                .iter()
                .filter(|(text, _, _)| SAMPLE.contains(text.trim()))
                .map(|(text, _, _)| text.trim())
                .collect::<Vec<_>>()
                .join(" ");
            assert_eq!(words, SAMPLE, "{face:?}: none of it cut off");
        }
    }
}

#[test]
fn a_press_on_a_face_claims_its_place_and_the_veil_and_the_cross_close_it() {
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Start(Box::new(settings(
        InterfaceFace::Geist,
        DESIGN,
    ))));
    let _ = renderer.handle(ToRenderer::Paint);
    let mut press =
        |x: f32, y: f32| match renderer.handle(ToRenderer::ConversationPress(Point::new(x, y))) {
            FromRenderer::ConversationClaimed(claim) => claim,
            other => panic!("{other:?}"),
        };
    // Each face's middle, its circle and its sample, whichever is chosen.
    let middles = [
        (351.0, 244.0),
        (688.0, 244.0),
        (351.0, 390.0),
        (688.0, 390.0),
        (351.0, 536.0),
    ];
    for (at, (x, y)) in middles.into_iter().enumerate() {
        assert_eq!(press(x, y), Some(Claim::Face { at }), "{at}");
    }
    assert_eq!(
        press(213.0, 201.0),
        Some(Claim::Face { at: 0 }),
        "the circle"
    );
    // Its ×, and the veil around the panel — over the greeting, the box
    // and the room beside them, none of which is pressed (queue item 886).
    assert_eq!(press(842.0, 70.0), Some(Claim::LeaveSettings));
    for (x, y) in [
        (80.0, 400.0),
        (520.0, 10.0),
        (520.0, 805.0),
        (1000.0, 100.0),
    ] {
        assert_eq!(press(x, y), Some(Claim::LeaveSettings), "({x}, {y})");
    }
    // The heading, the section's words, the gap between faces and the room
    // below them, inside the panel, claim nothing.
    for (x, y) in [
        (230.0, 70.0),
        (300.0, 153.0),
        (520.0, 244.0),
        (700.0, 615.0),
        (170.0, 400.0),
    ] {
        assert_eq!(press(x, y), None, "({x}, {y})");
    }
}

#[test]
fn settings_is_drawn_as_the_references() {
    for (viewport, name) in [
        (DESIGN, "settings.png"),
        (LEAST, "settings-in-the-least-window.png"),
    ] {
        let drawn = draw(ToRenderer::Start(Box::new(settings(
            InterfaceFace::Inter,
            viewport,
        ))));
        assert_eq!(holds(name, drawn.frame.as_ref()), Ok(()));
    }
}

/// The panel while *Settings* is the page on screen: three chats, none
/// current — the browser process sends none current while Settings is on
/// screen (`alo-window`'s `conversation`) — and the Settings row white.
fn panel_in(face: InterfaceFace) -> Panel {
    Panel {
        chats: vec![
            Chat::new("Charging my EV", "evhomeguide.org"),
            Chat::new("Weekend in Lisbon", "lisbon.example").doing(Doing::Working),
            Chat::new("Compare phone plans", "").earlier(),
        ],
        viewport: Size::new(240.0, 420.0),
        face,
        settings: true,
        ..Panel::default()
    }
}

#[test]
fn while_settings_is_on_screen_its_row_is_current_in_the_panel() {
    let drawn = draw(ToRenderer::Panel(Box::new(panel_in(InterfaceFace::Inter))));
    let links: Vec<(Option<String>, bool)> = read(&drawn.tree, "link")
        .iter()
        .map(|node| (node.name.clone(), node.states.current.is_some()))
        .collect();
    assert!(
        links.contains(&(Some("Settings".to_owned()), true)),
        "{links:?}"
    );
    assert_eq!(
        links.iter().filter(|(_, current)| *current).count(),
        1,
        "Settings alone current: {links:?}"
    );
    // The row where it always is, 12 over the panel's foot.
    assert_eq!(
        boxes_of(&drawn.renderer, "settings"),
        vec![px(10.0, 370.0, 220.0, 38.0)]
    );
}

/// A search's turn, two sites found, in a small first screen set in
/// `face`.
fn result_in(face: InterfaceFace) -> Start {
    let url =
        |address: &str| alo_url::parse(address).unwrap_or_else(|_| alo_url::Url::about_blank());
    let sites = vec![
        Found::new(
            "Kampala \u{2013} Wikipedia",
            url("https://en.wikipedia.org/wiki/Kampala"),
            "Kampala is the capital and largest city of Uganda. The city proper has a \
             population of 1,875,834 and is divided into the five political divisions.",
        )
        .named("Wikipedia"),
        Found::new(
            "Things to do in Kampala, Uganda",
            url("https://travel.example/kampala"),
            "The Kasubi Tombs, the Owino market and a boat out on Lake Victoria.",
        )
        .named("Travel Guide"),
    ];
    Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(760.0, 520.0),
        turns: vec![Turn::Searched(Results::of(
            "kampala",
            Showing::Found(sites),
        ))],
        search: SetUp::Ready,
        face,
        ..Start::default()
    }
}

/// The face's name as its references are named: lower case, hyphens.
fn file_name(face: InterfaceFace) -> String {
    face.name().to_lowercase().replace(' ', "-")
}

#[test]
fn each_face_draws_a_result_and_the_panel_as_its_references() {
    for face in InterfaceFace::ALL {
        let result = draw(ToRenderer::Start(Box::new(result_in(face))));
        let name = format!("face-{}-result.png", file_name(face));
        assert_eq!(holds(&name, result.frame.as_ref()), Ok(()));
        // Each title one line, each description its two, in every face:
        // the boxes do not move from one face to another.
        assert_eq!(
            boxes_of(&result.renderer, "t"),
            vec![px(32.0, 190.0, 696.0, 25.0), px(32.0, 309.0, 696.0, 25.0)],
            "{face:?}"
        );
        assert_eq!(
            boxes_of(&result.renderer, "d"),
            vec![px(32.0, 219.0, 696.0, 46.0), px(32.0, 338.0, 696.0, 23.0)],
            "{face:?}"
        );
        let panel = draw(ToRenderer::Panel(Box::new(panel_in(face))));
        let name = format!("face-{}-panel.png", file_name(face));
        assert_eq!(holds(&name, panel.frame.as_ref()), Ok(()));
        assert_eq!(
            boxes_of(&panel.renderer, "chat").len() + boxes_of(&panel.renderer, "settings").len(),
            4,
            "{face:?}"
        );
    }
}

/// Queue item 856: the keyboard's ring on its ×, and on the face chosen
/// while it is on the radio group, outside each one's edge:
/// every box where it was without it, and the ring only around the one.
#[test]
fn the_keyboards_ring_moves_nothing_and_is_drawn_as_the_references() {
    let unkeyed = draw(ToRenderer::Start(Box::new(settings(
        InterfaceFace::Geist,
        DESIGN,
    ))));
    for (keyed, class, name) in [
        (
            SettingsKeyed::Close,
            "pclose keyed",
            "settings-keyed-close.png",
        ),
        (
            SettingsKeyed::Font,
            "face on keyed",
            "settings-keyed-font.png",
        ),
    ] {
        let drawn = draw(ToRenderer::Start(Box::new(Start {
            settings_keyed: Some(keyed),
            ..settings(InterfaceFace::Geist, DESIGN)
        })));
        let unkeyed_class = class.trim_end_matches(" keyed");
        // Moving nothing: the one keyed where it stood unkeyed, in numbers.
        assert_eq!(
            boxes_of(&drawn.renderer, class),
            boxes_of(&unkeyed.renderer, unkeyed_class),
            "{class}"
        );
        assert_eq!(boxes_of(&drawn.renderer, class).len(), 1, "{class}");
        let faces = |renderer: &Renderer| {
            ["face", "face on", "face on keyed"]
                .into_iter()
                .flat_map(|each| rects_of(renderer, each))
                .map(units)
                .collect::<Vec<_>>()
        };
        let mut keyed_faces = faces(&drawn.renderer);
        let mut faces_before = faces(&unkeyed.renderer);
        keyed_faces.sort();
        faces_before.sort();
        assert_eq!(keyed_faces, faces_before, "every face where it was");
        // Its × 36 square at 824, 52 down; Geist's face the second of the
        // first row, 327 wide at 525.
        let expected = match keyed {
            SettingsKeyed::Close => vec![px(824.0, 52.0, 36.0, 36.0)],
            SettingsKeyed::Font => vec![px(525.0, 176.0, 327.0, 136.0)],
            // alo at work's (queue item 878) are `settings_alo_at_work.rs`'s.
            other => panic!("{other:?} is not rung here"),
        };
        assert_eq!(boxes_of(&drawn.renderer, class), expected, "{class}");
        // The ring: 3 outside the edge, the ink at a third over the page.
        let frame = drawn.frame.as_ref().expect("painted");
        let rect = rects_of(&drawn.renderer, class)
            .into_iter()
            .next()
            .expect("a box");
        let at = |x: f32, y: f32| -> [u8; 4] {
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
        };
        let unkeyed_frame = unkeyed.frame.as_ref().expect("painted");
        let before = |x: f32, y: f32| -> [u8; 4] {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a point inside the frame, in whole pixels"
            )]
            let (x, y) = (x as u32, y as u32);
            let i = usize::try_from((y * unkeyed_frame.width + x) * 4).unwrap_or(0);
            let mut pixel = [0; 4];
            pixel.copy_from_slice(unkeyed_frame.pixels.get(i..i + 4).unwrap_or(&[0; 4]));
            pixel
        };
        let middle = rect.top() + rect.size.height / 2.0;
        // 2 px outside the left edge: ringed, darker than the page.
        let ring = at(rect.left() - 2.0, middle);
        let page = before(rect.left() - 2.0, middle);
        assert!(ring[0] < page[0], "{class}: ring {ring:?}, page {page:?}");
        // 5 px outside it: the page as it was.
        assert_eq!(
            at(rect.left() - 5.0, middle),
            before(rect.left() - 5.0, middle)
        );
        assert_eq!(holds(name, drawn.frame.as_ref()), Ok(()));
    }
}
