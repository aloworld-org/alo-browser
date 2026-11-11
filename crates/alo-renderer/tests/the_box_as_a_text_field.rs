/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue items 861 and 848: the one box and *Search chats* drawn as text
//! fields — the caret where the browser process says it stands, a
//! selection on the ink's tint, an input method's composition underlined,
//! and words longer than the box scrolled to keep the caret in view
//! (`docs/design/shell.md`, *The one box as a text field*).
//!
//! What closes it here, each a test below: a reference render of each of
//! the caret mid-text, a selection, long words scrolled, and *Search chats*
//! with a selection; the caret's and the selection's boxes in numbers, laid
//! out by the engine, beside where the renderer says the field's words were
//! laid out ([`FieldLaidOut`]); and the agent reading the field's value and
//! its selection as a range of characters, never a place.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test
//! the_box_as_a_text_field` rewrites the references. A rewritten reference
//! is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::from_png;
use alo_renderer::field::{Composing, Editing, FieldLaidOut};
use alo_renderer::frame::frame_to_png;
use alo_renderer::panel::{Chat, Doing, Panel};
use alo_renderer::start::Start;
use alo_renderer::typed::offers;
use alo_renderer::{Frame, FromRenderer, Renderer, Snapshot, SnapshotNode, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::{Path, PathBuf};

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
    // The interface's faces, as the browser process hands them over.
    for font in alo_renderer::typefaces::fonts() {
        database.add(font);
    }
    database
}

/// What a renderer holding a part answers: its picture, its tree, the
/// boxes of its drawing by class, and where its field was laid out.
struct Drawn {
    frame: Option<Frame>,
    tree: Snapshot,
    classed: Vec<(String, Rect)>,
    laid: Option<FieldLaidOut>,
}

fn drawn(work: ToRenderer) -> Drawn {
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
    let laid = match renderer.handle(ToRenderer::FieldLaidOut) {
        FromRenderer::FieldLaidOut(laid) => laid,
        _ => None,
    };
    Drawn {
        frame,
        tree,
        classed: classed(&renderer),
        laid,
    }
}

/// Every element's box with a class in the last drawing, by class, in
/// document order: how the caret and the selection — which an agent does
/// not read, being drawn on a line `aria-hidden` — are found.
fn classed(renderer: &Renderer) -> Vec<(String, Rect)> {
    let (Some(drawing), Some(document)) = (renderer.rendered(), renderer.document()) else {
        return Vec::new();
    };
    drawing
        .boxes
        .ids()
        .filter_map(|id| {
            let made = drawing.boxes.get(id)?;
            if matches!(made.kind, alo_box::BoxKind::Text { .. }) {
                return None;
            }
            let class = document.attribute_by_name(made.kind.node()?, "class")?;
            let geometry = drawing.layout.get(id)?;
            Some((class.to_owned(), geometry.border_box))
        })
        .collect()
}

fn of_class(drawn: &Drawn, class: &str) -> Vec<String> {
    boxes_of(drawn, class).into_iter().map(units).collect()
}

fn boxes_of(drawn: &Drawn, class: &str) -> Vec<Rect> {
    drawn
        .classed
        .iter()
        .filter(|(each, _)| each == class)
        .map(|(_, rect)| *rect)
        .collect()
}

/// Whether `laid` and `measured` are one box to within two of layout's
/// 1/64 px: a run laid out alone is shaped alone, and its width differs
/// from the measure of the same letters inside the whole words by the
/// kerning at its ends.
fn near(laid: Rect, measured: Rect) -> bool {
    let close = |one: f32, other: f32| (one - other).abs() <= 2.0 / 64.0;
    close(laid.left(), measured.left())
        && close(laid.top(), measured.top())
        && close(laid.size.width, measured.size.width)
        && close(laid.size.height, measured.size.height)
}

/// A rectangle as the outline writes it: layout holds whole 1/64 px.
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

/// The field the agent reads: the one `textbox`.
fn field(tree: &Snapshot) -> Option<&SnapshotNode> {
    tree.nodes()
        .into_iter()
        .find(|node| node.role.as_str() == "textbox")
}

/// What the field holds, as the agent reads it: the text read next after
/// it.
fn value(tree: &Snapshot) -> Option<String> {
    let nodes = tree.nodes();
    let at = nodes
        .iter()
        .position(|node| node.role.as_str() == "textbox")?;
    nodes.get(at + 1).and_then(|node| node.name.clone())
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

/// The pixel column `x` CSS pixels in, at a scale of one.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a column of a screen 1,040 wide, clamped to nought below"
)]
fn column(x: f32) -> u32 {
    x.max(0.0) as u32
}

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    frame.pixels.get(at..at + 4)?.try_into().ok()
}

/// The first screen in the design's window with `typed` in the box, which
/// has the keyboard, standing as `editing` says.
fn typed_into(typed: &str, editing: Editing) -> Start {
    Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(1040.0, 820.0),
        typed: typed.to_owned(),
        focused: true,
        editing,
        offers: offers(typed),
        ..Start::default()
    }
}

const ADDRESS: &str = "en.wikipedia.org/wiki/Kampala";

/// How wide `text` is, in the box's words, measured alone.
fn measured(text: &str) -> f32 {
    use alo_layout::MeasureText as _;
    let fonts = fonts();
    let measurer = alo_text::TextMeasurer::new(&fonts);
    let style = alo_renderer::field_line::style(alo_renderer::typefaces::MANROPE, 16.0, 1.0);
    measurer.measure(text, &style, None).width
}

#[test]
fn the_caret_stands_inside_the_words_where_it_was_put() {
    // After `en.wikipedia.org`, sixteen characters in.
    let shown = drawn(ToRenderer::Start(Box::new(typed_into(
        ADDRESS,
        Editing::caret_at(16),
    ))));
    let laid = shown.laid.clone().unwrap_or_default();
    // The field's words stand where the input's own do: 183 in (the box
    // at 160, its border and 22 inside), its top 266, 40 tall and 632 wide
    // (the box's 720 less 22 and 10 inside, Go's 44 and the gap of 10, and
    // the border), not scrolled.
    assert_eq!(
        (laid.left, laid.top, laid.height, laid.room, laid.scrolled),
        (183.0, 266.0, 40.0, 632.0, 0.0)
    );
    assert_eq!(laid.stops.len(), 30, "a stop either side of every letter");
    // The stop sixteen in is as far as `en.wikipedia.org` is wide.
    let caret_x = laid.x_of(16);
    assert!(
        (caret_x - measured("en.wikipedia.org")).abs() < 1.0 / 64.0,
        "{caret_x}"
    );
    // The caret: 1 wide and 20 tall, on the stop rounded to a pixel, 10
    // into the field's 40.
    assert_eq!(
        of_class(&shown, "caret"),
        vec![px(laid.left + caret_x.round(), 276.0, 1.0, 20.0)]
    );
    // The agent reads the value, and the caret as an empty selection.
    let node = field(&shown.tree);
    assert_eq!(value(&shown.tree).as_deref(), Some(ADDRESS));
    assert_eq!(node.and_then(|node| node.states.selection), Some((16, 16)));
    assert!(shown.tree.to_outline().contains("caret=16"));
    let frame = shown.frame.as_ref();
    // The caret is the ink, over the box's white.
    let ink = [0x10, 0x2a, 0x43, 0xff];
    let x = 183 + column(caret_x.round());
    assert_eq!(frame.and_then(|frame| pixel(frame, x, 286)), Some(ink));
    assert_eq!(holds("field-caret-mid-text.png", frame), Ok(()));

    // Its blink's hidden half: the same words, no caret, and still read.
    let hidden = drawn(ToRenderer::Start(Box::new(typed_into(
        ADDRESS,
        Editing {
            caret_shown: false,
            ..Editing::caret_at(16)
        },
    ))));
    assert!(of_class(&hidden, "caret").is_empty());
    assert_eq!(
        field(&hidden.tree).and_then(|node| node.states.selection),
        Some((16, 16))
    );
    assert_ne!(
        hidden.frame.as_ref().and_then(|frame| pixel(frame, x, 286)),
        Some(ink)
    );
    // Without the keyboard, no caret and no selection read.
    let resting = drawn(ToRenderer::Start(Box::new(Start {
        focused: false,
        ..typed_into(ADDRESS, Editing::caret_at(16))
    })));
    assert!(of_class(&resting, "caret").is_empty());
    assert_eq!(
        field(&resting.tree).and_then(|node| node.states.selection),
        None
    );
}

#[test]
fn a_selection_is_drawn_on_the_inks_tint_and_read_as_a_range() {
    let typed = "Kampala population 2024";
    // `population` selected, from its end back to its start.
    let shown = drawn(ToRenderer::Start(Box::new(typed_into(
        typed,
        Editing::caret_at(8).selecting_from(18),
    ))));
    let laid = shown.laid.clone().unwrap_or_default();
    let (start, end) = (laid.x_of(8), laid.x_of(18));
    let selected = boxes_of(&shown, "selected");
    // As wide as `population` where its stops are, 22 tall in the
    // middle of the field's 40: (249.16, 275) 79.66 × 22, the line of
    // Manrope at 16 (queue item 975; Inter's was 20).
    assert_eq!(selected.len(), 1, "{:?}", shown.classed);
    assert!(
        selected.first().is_some_and(|laid_out| near(
            *laid_out,
            Rect::new(laid.left + start, 275.0, end - start, 22.0)
        )),
        "{selected:?}"
    );
    assert!(
        of_class(&shown, "caret").is_empty(),
        "no caret over a selection"
    );
    let node = field(&shown.tree);
    assert_eq!(node.and_then(|node| node.states.selection), Some((8, 18)));
    assert!(shown.tree.to_outline().contains("selection=8..18"));
    // The tint: the ink at 18 % over white, behind the words' gaps.
    let frame = shown.frame.as_ref();
    let x = column(laid.left + start + 1.0);
    assert_eq!(
        frame.and_then(|frame| pixel(frame, x, 277)),
        Some([0xd4, 0xd9, 0xdd, 0xff])
    );
    assert_eq!(holds("field-selection.png", frame), Ok(()));
}

#[test]
fn words_longer_than_the_box_scroll_to_keep_the_caret_in_view() {
    let typed = "what is the population of Kampala, the capital of Uganda, and how has it \
                 grown since the city was founded on its seven hills";
    let count = typed.chars().count();
    // Laid out first with no room known, then with the room it has, as
    // the browser process sends it back.
    let first = drawn(ToRenderer::Start(Box::new(typed_into(
        typed,
        Editing::caret_at(count),
    ))));
    let room = first.laid.as_ref().map(|laid| laid.room);
    assert_eq!(room, Some(632.0));
    let second = drawn(ToRenderer::Start(Box::new(typed_into(
        typed,
        Editing {
            room,
            ..Editing::caret_at(count)
        },
    ))));
    let laid = second.laid.clone().unwrap_or_default();
    let width = laid.stops.last().map_or(0.0, |(_, x)| *x);
    // Scrolled so the caret, one wide, stands at the room's right edge,
    // on a whole pixel.
    assert!((laid.scrolled - (width + 1.0 - 632.0).round()).abs() < f32::EPSILON);
    assert!(laid.scrolled > 0.0, "{laid:?}");
    let line = of_class(&second, "field-line");
    assert_eq!(line.len(), 1);
    assert!(
        line.first()
            .is_some_and(|line| line.starts_with(&format!("({}, ", laid.left - laid.scrolled))),
        "{line:?}"
    );
    let caret = of_class(&second, "caret");
    let caret_left = laid.left + width.round() - laid.scrolled;
    assert_eq!(caret, vec![px(caret_left, 276.0, 1.0, 20.0)]);
    assert!(caret_left + 1.0 <= laid.left + laid.room, "inside the room");
    // The agent reads the whole value however far it is scrolled.
    assert_eq!(value(&second.tree).as_deref(), Some(typed));
    assert_eq!(
        holds("field-long-scrolled.png", second.frame.as_ref()),
        Ok(())
    );

    // Back at the start, ⌘← put the caret there: nothing scrolled.
    let back = drawn(ToRenderer::Start(Box::new(typed_into(
        typed,
        Editing {
            room,
            scrolled: laid.scrolled,
            ..Editing::caret_at(0)
        },
    ))));
    assert_eq!(
        back.laid.map(|laid| laid.scrolled.to_bits()),
        Some(0.0_f32.to_bits())
    );
}

#[test]
fn a_composition_is_underlined_where_the_caret_is_and_read_in_the_value() {
    let shown = drawn(ToRenderer::Start(Box::new(typed_into(
        "Kampala ",
        Editing {
            // Pinyin composes in letters before a person chooses 你好: the
            // test's faces hold Latin's, and the drawing is the same.
            composing: Some(Composing {
                text: "nihao".to_owned(),
                caret: 5,
            }),
            ..Editing::caret_at(8)
        },
    ))));
    let laid = shown.laid.clone().unwrap_or_default();
    let composed = boxes_of(&shown, "composed");
    let (start, end) = (laid.x_of(8), laid.x_of(13));
    // `nihao`, after `Kampala `: (249.16, 275) 41.11 × 22.
    assert!(
        composed.len() == 1
            && composed.first().is_some_and(|laid_out| near(
                *laid_out,
                Rect::new(laid.left + start, 275.0, end - start, 22.0)
            )),
        "{composed:?}"
    );
    // The caret at the composition's end, where the input method's is.
    assert_eq!(
        of_class(&shown, "caret"),
        vec![px(laid.left + end.round(), 276.0, 1.0, 20.0)]
    );
    // Underlined, in the ink, on row 293 under its letters (which end at
    // 291), and nowhere else.
    let frame = shown.frame.as_ref();
    let under = |x: f32| {
        frame.and_then(|frame| {
            (292..297).find(|&y| pixel(frame, column(x), y) == Some([0x10, 0x2a, 0x43, 0xff]))
        })
    };
    assert!(under(laid.left + start + 2.0).is_some(), "under n");
    assert!(
        under(laid.left + start - 6.0).is_none(),
        "not under Kampala"
    );
    assert_eq!(value(&shown.tree).as_deref(), Some("Kampala nihao"));
}

/// The panel holding the chats [`Search chats`] finds for `searched`, with
/// the keyboard, standing as `editing` says.
fn searching(searched: &str, editing: Editing) -> Panel {
    Panel {
        chats: vec![
            Chat::new("Lisbon trip", "lisbon.example").current(),
            Chat::new("Lisbon hotels", "").doing(Doing::Done),
        ],
        viewport: Size::new(240.0, 560.0),
        scale: 1.0,
        searched: searched.to_owned(),
        searching: true,
        editing,
        ..Panel::default()
    }
}

#[test]
fn search_chats_selected_is_drawn_and_read_as_the_box_is() {
    // `lisbon` selected in `lisbon trip`, from its start.
    let shown = drawn(ToRenderer::Panel(Box::new(searching(
        "lisbon trip",
        Editing::caret_at(6).selecting_from(0),
    ))));
    let laid = shown.laid.clone().unwrap_or_default();
    // After the glass, 13.5 px words in a field 20 tall.
    assert_eq!(
        (laid.left, laid.top, laid.height, laid.room),
        (46.0, 111.0, 20.0, 174.0)
    );
    let end = laid.x_of(6);
    let selected = boxes_of(&shown, "selected");
    assert!(
        selected.len() == 1
            && selected
                .first()
                .is_some_and(|laid_out| near(*laid_out, Rect::new(laid.left, 112.0, end, 18.0))),
        "{selected:?}"
    );
    let node = field(&shown.tree);
    assert_eq!(node.and_then(|node| node.states.selection), Some((0, 6)));
    assert_eq!(
        holds("panel-search-selected.png", shown.frame.as_ref()),
        Ok(())
    );

    // Its caret, after `lisbon`, is 16 tall in its 20.
    let caret = drawn(ToRenderer::Panel(Box::new(searching(
        "lisbon trip",
        Editing::caret_at(6),
    ))));
    assert_eq!(
        of_class(&caret, "caret"),
        vec![px(laid.left + end.round(), 113.0, 1.0, 16.0)]
    );
}

#[test]
fn at_two_the_caret_stands_on_whole_device_pixels() {
    let shown = drawn(ToRenderer::Start(Box::new(
        typed_into(ADDRESS, Editing::caret_at(16)).at_scale(2.0),
    )));
    let laid = shown.laid.clone().unwrap_or_default();
    let x = laid.x_of(16);
    let caret = of_class(&shown, "caret");
    assert_eq!(
        caret,
        vec![px(
            laid.left + (x * 2.0).round() / 2.0,
            laid.top + 10.0,
            1.0,
            20.0
        )]
    );
}
