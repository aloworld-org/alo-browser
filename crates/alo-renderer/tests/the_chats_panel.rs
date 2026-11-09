/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 699: the chats panel is a document, rendered by the engine
//! (ADR 0074), as the tab strip is (ADR 0024 § 4).
//!
//! What closes it, each a test below: a reference render of a panel with
//! open and earlier chats; a layout assertion of every row; the agent
//! reading *"navigation Chats: Open, three chats, the first current, the
//! second at work, the third done"*; hostile names staying text in the tree
//! and the render; a long name cut by the stylesheet to its row; and the
//! same panel through the real, confined renderer at the panel's own site.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test the_chats_panel`
//! rewrites the references. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{DisplayItem, from_png};
use alo_renderer::face::Face;
use alo_renderer::frame::frame_to_png;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::panel::{self, Chat, Doing, Panel};
use alo_renderer::panel_document::{self, DONE};
use alo_renderer::{Frame, FromRenderer, Renderer, Snapshot, SnapshotNode, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::{Path, PathBuf};

/// The corpus's faces: its sans-serif for both weights, which is what
/// `system-ui` and `sans-serif` mean here, and its serif for the mark.
fn faces() -> Vec<(&'static str, Weight, &'static [u8])> {
    vec![
        ("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
        ("DejaVu Sans", Weight::BOLD, dejavu::sans::bold()),
        ("DejaVu Serif", Weight::NORMAL, dejavu::serif::regular()),
    ]
}

fn generics() -> Vec<(&'static str, &'static str)> {
    vec![
        ("system-ui", "DejaVu Sans"),
        ("sans-serif", "DejaVu Sans"),
        ("serif", "DejaVu Serif"),
    ]
}

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (family, weight, bytes) in faces() {
        if let Some(font) = Font::load(family, weight, Slant::Normal, bytes.to_vec()) {
            database.add(font);
        }
    }
    for (generic, family) in generics() {
        database.map_generic(generic, family);
    }
    // The design's typefaces, as the browser process hands them to the
    // interface's renderers (ADR 0084).
    for font in alo_renderer::typefaces::fonts() {
        database.add(font);
    }
    database
}

/// The panel's width in the design, and a height that leaves room below the
/// chats.
const WIDTH: f32 = 240.0;
const HEIGHT: f32 = 560.0;

fn panel_of(chats: Vec<Chat>) -> Panel {
    Panel {
        chats,
        viewport: Size::new(WIDTH, HEIGHT),
        scale: 1.0,
        ..Panel::default()
    }
}

/// Three open chats — the one on screen, one the agent is at work in, one it
/// finished — and two from before.
fn five() -> Vec<Chat> {
    vec![
        Chat::new("Charging my EV", "evhomeguide.org").current(),
        Chat::new("Weekend in Lisbon", "lisbon.example").doing(Doing::Working),
        Chat::new("Invoice for March", "books.example").doing(Doing::Done),
        Chat::new("Compare phone plans", "").earlier(),
        Chat::new("Birthday present ideas", "").earlier(),
    ]
}

/// What a renderer holding a panel answers to a paint and a reading.
struct Drawn {
    answer: FromRenderer,
    frame: Option<Frame>,
    tree: Snapshot,
    /// Every text the panel's drawing put down, in order.
    texts: Vec<String>,
    /// Where each icon was laid out, in document order. The agent does not
    /// read them — they are `aria-hidden` — so they are found in the
    /// drawing's own boxes, by the element each box is for.
    icons: Vec<String>,
    /// Every element's box with a class, by its class, in document order:
    /// how what an agent does not read — the rail's initials and the name
    /// beside a hovered chat — is found (queue item 760).
    classed: Vec<(String, String)>,
}

fn draw(panel: Panel) -> Drawn {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Panel(Box::new(panel)));
    let frame = match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Some(frame),
        _ => None,
    };
    let tree = match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(tree) => *tree,
        _ => Snapshot::default(),
    };
    let texts = renderer
        .rendered()
        .map(|drawing| {
            drawing
                .display
                .items()
                .iter()
                .filter_map(|item| match item {
                    DisplayItem::Text { text, .. } => Some(text.clone()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let icons = icons(&renderer);
    let classed = classed(&renderer);
    Drawn {
        answer,
        frame,
        tree,
        texts,
        icons,
        classed,
    }
}

/// The border box of every element with a class the renderer drew, by its
/// class, in the order the boxes were made.
fn classed(renderer: &Renderer) -> Vec<(String, String)> {
    let (Some(drawing), Some(document)) = (renderer.rendered(), renderer.document()) else {
        return Vec::new();
    };
    drawing
        .boxes
        .ids()
        .filter_map(|id| {
            let node = drawing.boxes.get(id).and_then(|made| made.kind.node())?;
            let class = document.attribute_by_name(node, "class")?;
            let geometry = drawing.layout.get(id)?;
            Some((class.to_owned(), units(geometry.border_box)))
        })
        .collect()
}

/// The boxes of `class` among `drawn`'s.
fn of_class(drawn: &Drawn, class: &str) -> Vec<String> {
    drawn
        .classed
        .iter()
        .filter(|(each, _)| each == class)
        .map(|(_, rect)| rect.clone())
        .collect()
}

/// The border box of every `<svg>` the renderer drew, in the order the
/// boxes were made, which is the document's.
fn icons(renderer: &Renderer) -> Vec<String> {
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
                .and_then(|made| made.kind.node())
                .and_then(|node| document.element(node))
                .is_some_and(|element| element.name.local.as_ref() == "svg")
        })
        .filter_map(|id| drawing.layout.get(id))
        .map(|geometry| units(geometry.border_box))
        .collect()
}

/// Every node the agent reads in `role`, in order.
fn read<'a>(tree: &'a Snapshot, role: &str) -> Vec<&'a SnapshotNode> {
    tree.nodes()
        .into_iter()
        .filter(|node| node.role.as_str() == role)
        .collect()
}

/// A rectangle as the outline writes it: layout holds whole 1/64 px, so two
/// rectangles that write alike are the same, compared without comparing
/// floats.
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

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

/// Compare `frame` with the committed reference, or write it when updating.
/// What differs, in words, if anything does.
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
            "{name}: the panel is {}×{}, and the reference {}×{}",
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

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    let rgba = frame.pixels.get(at..at + 4)?;
    rgba.try_into().ok()
}

#[test]
fn a_panel_of_open_and_earlier_chats_is_read_by_name_with_what_is_happening() {
    let drawn = draw(panel_of(five()));
    assert!(
        matches!(&drawn.answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{:?}",
        drawn.answer
    );
    let outline = drawn.tree.to_outline();

    // A navigation named Chats, its two groups headed Open and Earlier.
    let navigation = read(&drawn.tree, "navigation");
    assert_eq!(navigation.len(), 1, "{outline}");
    assert_eq!(
        navigation.first().and_then(|node| node.name.as_deref()),
        Some("Chats")
    );
    let headings: Vec<Option<&str>> = read(&drawn.tree, "heading")
        .iter()
        .map(|node| node.name.as_deref())
        .collect();
    assert_eq!(headings, vec![Some("Open"), Some("Earlier")], "{outline}");

    // Every link by its name: the site's letter is not part of one, and a
    // finished chat says so in words.
    let links = read(&drawn.tree, "link");
    let names: Vec<Option<&str>> = links.iter().map(|link| link.name.as_deref()).collect();
    let done = format!("Invoice for March {DONE}");
    assert_eq!(
        names,
        vec![
            Some("New chat"),
            Some("Charging my EV"),
            Some("Weekend in Lisbon"),
            Some(done.as_str()),
            Some("Compare phone plans"),
            Some("Birthday present ideas"),
            Some("Settings"),
        ],
        "{outline}"
    );
    let current: Vec<bool> = links
        .iter()
        .map(|link| link.states.current.is_some())
        .collect();
    assert_eq!(
        current,
        vec![false, true, false, false, false, false, false],
        "{outline}"
    );
    let busy: Vec<bool> = links.iter().map(|link| link.states.busy).collect();
    assert_eq!(
        busy,
        vec![false, false, true, false, false, false, false],
        "{outline}"
    );

    // In numbers: 6 above and 10 either side; the mark's row of 46 and 4
    // below it; New chat 40 tall at 56; Search chats 38 tall, 6 under it, at
    // 102 (queue item 711). The first group's heading 18 below that, 14
    // tall, 6 above its rows; rows 38 tall and 2 apart, 220 wide. The second
    // group 18 below the first's last row. Settings at the foot: 560 less 12
    // of padding, 38 of row, 8 above it and the line's 1.
    let rects: Vec<String> = links.iter().map(|link| units(link.rect)).collect();
    assert_eq!(
        rects,
        vec![
            px(10.0, 56.0, 220.0, 40.0),
            px(10.0, 178.0, 220.0, 38.0),
            px(10.0, 218.0, 220.0, 38.0),
            px(10.0, 258.0, 220.0, 38.0),
            px(10.0, 334.0, 220.0, 38.0),
            px(10.0, 374.0, 220.0, 38.0),
            px(10.0, 510.0, 220.0, 38.0),
        ],
        "{outline}"
    );
    // The button that hides the panel, 36 square at the head's end, named
    // in words; Search chats a search holding a field named so, after its
    // 12 of padding, 16 of glass and 8 of gap, 20 tall in the middle of 38.
    let buttons: Vec<(Option<&str>, String)> = read(&drawn.tree, "button")
        .iter()
        .map(|button| (button.name.as_deref(), units(button.rect)))
        .collect();
    assert_eq!(
        buttons,
        vec![(Some("Hide the panel"), px(194.0, 11.0, 36.0, 36.0))],
        "{outline}"
    );
    let searches: Vec<String> = read(&drawn.tree, "search")
        .iter()
        .map(|search| units(search.rect))
        .collect();
    assert_eq!(searches, vec![px(10.0, 102.0, 220.0, 38.0)], "{outline}");
    let fields: Vec<(Option<&str>, String)> = read(&drawn.tree, "textbox")
        .iter()
        .map(|field| (field.name.as_deref(), units(field.rect)))
        .collect();
    assert_eq!(
        fields,
        vec![(Some("Search chats"), px(46.0, 111.0, 174.0, 20.0))],
        "{outline}"
    );
    // The icons, inside rows that did not move for them (items 712, 711):
    // first the mark, the owner's icon 24 square (item 913), 8 into the
    // head after the panel's 10, and in the middle of the mark's line of
    // 26, itself in the middle of the head's 46 from 6: 6 + 10 + 1 = 17
    // down; the hide button's 18 square in the middle of its 36, 9 in and down;
    // New chat's plus after its 1 of border and 12 of padding, so 23 in,
    // and centred in the 38 inside the border, 1 + 11 = 12 down; Search
    // chats' glass 12 in and 11 down; Settings' sliders after its 10 of
    // padding, 20 in, and centred in its 38, 11 down.
    assert_eq!(
        drawn.icons,
        vec![
            px(18.0, 17.0, 24.0, 24.0),
            px(203.0, 20.0, 18.0, 18.0),
            px(23.0, 68.0, 16.0, 16.0),
            px(22.0, 113.0, 16.0, 16.0),
            px(20.0, 521.0, 16.0, 16.0)
        ],
        "{outline}"
    );
    assert_eq!(holds("panel-chats.png", drawn.frame.as_ref()), Ok(()));
}

/// What the field holds, as the agent reads it: the text the field's
/// value is drawn as, read next after the field.
fn value(tree: &Snapshot) -> Option<String> {
    let nodes = tree.nodes();
    let field = nodes
        .iter()
        .position(|node| node.role.as_str() == "textbox")?;
    nodes
        .get(field + 1)
        .filter(|node| node.role.as_str() == "text")
        .and_then(|node| node.name.clone())
}

/// The panel with `searched` typed into Search chats, which has the
/// keyboard, holding the chats of [`five`] the browser process finds for it.
fn searching(searched: &str) -> Panel {
    Panel {
        chats: five()
            .into_iter()
            .filter(|chat| panel::matching(chat, searched))
            .collect(),
        searched: searched.to_owned(),
        searching: true,
        ..panel_of(Vec::new())
    }
}

#[test]
fn search_chats_typed_into_shows_its_words_and_only_the_chats_named_so() {
    // Queue item 711: "in" is in Charging, Lisbon and Invoice, and in
    // neither earlier chat, whose group is then not drawn.
    let drawn = draw(searching("in"));
    let outline = drawn.tree.to_outline();
    assert_eq!(value(&drawn.tree).as_deref(), Some("in"), "{outline}");
    let headings: Vec<Option<&str>> = read(&drawn.tree, "heading")
        .iter()
        .map(|node| node.name.as_deref())
        .collect();
    assert_eq!(headings, vec![Some("Open")], "{outline}");
    let links: Vec<(Option<&str>, String)> = read(&drawn.tree, "link")
        .iter()
        .map(|link| (link.name.as_deref(), units(link.rect)))
        .collect();
    let done = format!("Invoice for March {DONE}");
    assert_eq!(
        links,
        vec![
            (Some("New chat"), px(10.0, 56.0, 220.0, 40.0)),
            (Some("Charging my EV"), px(10.0, 178.0, 220.0, 38.0)),
            (Some("Weekend in Lisbon"), px(10.0, 218.0, 220.0, 38.0)),
            (Some(done.as_str()), px(10.0, 258.0, 220.0, 38.0)),
            (Some("Settings"), px(10.0, 510.0, 220.0, 38.0)),
        ],
        "{outline}"
    );
    let frame = drawn.frame.unwrap_or_else(|| panic!("nothing was painted"));
    // With the keyboard, Search chats is white; the panel's grey beside it.
    assert_eq!(pixel(&frame, 120, 104), Some([0xff, 0xff, 0xff, 0xff]));
    assert_eq!(pixel(&frame, 5, 104), Some([0xee, 0xea, 0xe2, 0xff]));
    assert_eq!(holds("panel-searched.png", Some(&frame)), Ok(()));

    // Without it, the field keeps the words, on the panel's grey.
    let resting = draw(Panel {
        searching: false,
        ..searching("in")
    });
    assert_eq!(value(&resting.tree).as_deref(), Some("in"));
    let frame = resting
        .frame
        .unwrap_or_else(|| panic!("nothing was painted"));
    assert_eq!(pixel(&frame, 120, 104), Some([0xee, 0xea, 0xe2, 0xff]));
}

#[test]
fn search_chats_finding_nothing_says_so_in_words() {
    let drawn = draw(searching("zebra"));
    let outline = drawn.tree.to_outline();
    assert!(read(&drawn.tree, "heading").is_empty(), "{outline}");
    let said = panel_document::none_called("zebra");
    assert_eq!(said, "No chat is called \u{201c}zebra\u{201d}");
    assert!(
        drawn.texts.iter().any(|text| text.contains("zebra")),
        "{:?}",
        drawn.texts
    );
    let paragraphs: Vec<(Option<&str>, String)> = read(&drawn.tree, "paragraph")
        .iter()
        .map(|node| (node.name.as_deref(), units(node.rect)))
        .collect();
    // 18 under Search chats' foot at 140, one line of 18, 10 in.
    assert_eq!(
        paragraphs,
        vec![(None, px(10.0, 158.0, 220.0, 18.0))],
        "{outline}"
    );
    let texts: Vec<Option<&str>> = drawn
        .tree
        .nodes()
        .into_iter()
        .filter(|node| node.role.as_str() == "text")
        .filter_map(|node| node.name.as_deref())
        .map(Some)
        .collect();
    assert!(texts.contains(&Some(said.as_str())), "{outline}");
}

/// Three chats for the rail (queue item 760): the one on screen, one the
/// agent is at work in, one it finished.
fn three() -> Vec<Chat> {
    let mut chats = five();
    chats.truncate(3);
    chats
}

#[test]
fn hidden_the_panel_is_the_rail_at_one_and_at_two() {
    // Queue item 711: 64 wide, the mark 40 tall 10 down, New chat 44 square
    // 6 under it, centred, and Show the panel at the foot, 12 above it.
    // Queue item 760: under the line, each chat 44 square, 6 apart from
    // 121, its initials in a 32 circle in the middle.
    for (scale, factor, name) in [
        (1.0, 1, "panel-rail.png"),
        (2.0, 2, "panel-rail-at-two.png"),
    ] {
        let drawn = draw(
            Panel {
                hidden: true,
                viewport: Size::new(64.0, HEIGHT),
                ..panel_of(three())
            }
            .at_scale(scale),
        );
        let outline = drawn.tree.to_outline();
        let navigation = read(&drawn.tree, "navigation");
        assert_eq!(
            navigation.first().and_then(|node| node.name.as_deref()),
            Some("Chats"),
            "{outline}"
        );
        assert!(read(&drawn.tree, "link").is_empty(), "no row: {outline}");
        let buttons: Vec<(Option<&str>, String)> = read(&drawn.tree, "button")
            .iter()
            .map(|button| (button.name.as_deref(), units(button.rect)))
            .collect();
        let done = format!("Invoice for March {DONE}");
        assert_eq!(
            buttons,
            vec![
                (Some("New chat"), px(10.0, 56.0, 44.0, 44.0)),
                (Some("Charging my EV"), px(10.0, 121.0, 44.0, 44.0)),
                (Some("Weekend in Lisbon"), px(10.0, 171.0, 44.0, 44.0)),
                (Some(done.as_str()), px(10.0, 221.0, 44.0, 44.0)),
                (Some("Show the panel"), px(10.0, 504.0, 44.0, 44.0)),
            ],
            "{outline}"
        );
        assert_eq!(
            of_class(&drawn, "initials"),
            vec![
                px(16.0, 127.0, 32.0, 32.0),
                px(16.0, 177.0, 32.0, 32.0),
                px(16.0, 227.0, 32.0, 32.0),
            ],
            "{outline}"
        );
        // The working chat's dot at its circle's shoulder, 5 in.
        assert_eq!(
            of_class(&drawn, "working"),
            vec![px(42.0, 176.0, 7.0, 7.0)],
            "{outline}"
        );
        // The mark, the owner's icon 28 square (item 913), in the middle of
        // the rail's 64 and of its row of 40 from 10; then New chat's plus
        // and Show the panel's window.
        assert_eq!(
            drawn.icons,
            vec![
                px(18.0, 16.0, 28.0, 28.0),
                px(23.0, 69.0, 18.0, 18.0),
                px(23.0, 517.0, 18.0, 18.0)
            ],
            "{outline}"
        );
        let frame = drawn.frame.unwrap_or_else(|| panic!("nothing was painted"));
        let device = |css: u32| css * factor;
        assert_eq!((frame.width, frame.height), (device(64), device(560)));
        // The short line under New chat, 28 wide from 18, at 110.
        assert_eq!(
            pixel(&frame, device(32), device(110)),
            Some([0xe1, 0xda, 0xce, 0xff])
        );
        // The chat on screen is white in its circle's middle, beside its
        // letters; the others on the line's colour. The rail is opaque.
        assert_eq!(
            pixel(&frame, device(19), device(143)),
            Some([0xff, 0xff, 0xff, 0xff])
        );
        assert_eq!(
            pixel(&frame, device(19), device(193)),
            Some([0xe1, 0xda, 0xce, 0xff])
        );
        assert_eq!(
            pixel(&frame, device(63), device(300)),
            Some([0xee, 0xea, 0xe2, 0xff])
        );
        assert_eq!(holds(name, Some(&frame)), Ok(()));
    }
}

/// Queue item 760: the chat the pointer is over on the rail has its name
/// beside it, past the rail's edge, in a rail laid out 320 wide and drawn
/// on nothing — so the window composes only the rail and the name.
#[test]
fn a_hovered_chat_on_the_rail_has_its_name_beside_it_over_nothing() {
    let mut chats = three();
    if let Some(lisbon) = chats.get_mut(1) {
        lisbon.hovered = true;
    }
    let drawn = draw(Panel {
        hidden: true,
        viewport: Size::new(320.0, HEIGHT),
        ..panel_of(chats)
    });
    let outline = drawn.tree.to_outline();
    assert!(
        matches!(&drawn.answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{:?}",
        drawn.answer
    );
    // The name is the button's own; an agent reads it once.
    let buttons: Vec<Option<&str>> = read(&drawn.tree, "button")
        .iter()
        .map(|button| button.name.as_deref())
        .collect();
    assert_eq!(buttons.get(2).copied().flatten(), Some("Weekend in Lisbon"));
    let pills = of_class(&drawn, "named");
    let [pill] = pills.as_slice() else {
        panic!("one name beside the hovered chat: {pills:?}\n{outline}");
    };
    // 28 tall, centred on the 44 button at 171, from 68: 4 past the rail.
    assert!(
        pill.starts_with("(68, 179) ") && pill.ends_with("×28"),
        "{pill}"
    );
    assert!(
        drawn
            .texts
            .iter()
            .any(|text| text.contains("Weekend in Lisbon")),
        "{:?}",
        drawn.texts
    );
    let frame = drawn.frame.unwrap_or_else(|| panic!("nothing was painted"));
    assert_eq!((frame.width, frame.height), (320, 560));
    // The rail's ground to its edge, the hovered button a shade lighter,
    // the pill's dark ground past the edge, and nothing elsewhere.
    assert_eq!(pixel(&frame, 63, 300), Some([0xee, 0xea, 0xe2, 0xff]));
    assert_eq!(pixel(&frame, 12, 193), Some([0xf6, 0xf3, 0xee, 0xff]));
    assert_eq!(pixel(&frame, 70, 193), Some([0x2b, 0x27, 0x22, 0xff]));
    for (x, y) in [(64, 0), (66, 193), (200, 140), (319, 559), (100, 300)] {
        assert_eq!(
            pixel(&frame, x, y).map(|rgba| rgba[3]),
            Some(0),
            "nothing at ({x}, {y})"
        );
    }
    assert_eq!(holds("panel-rail-named.png", Some(&frame)), Ok(()));
}

/// Queue item 760: a press on each of the rail's chats claims it by its
/// place; on the name beside a hovered one, nothing.
#[test]
fn a_press_on_each_chat_of_the_rail_claims_its_place() {
    use alo_paint::Point;
    use alo_renderer::conversation_claim::Claim;
    let mut chats = three();
    if let Some(lisbon) = chats.get_mut(1) {
        lisbon.hovered = true;
    }
    let mut renderer = Renderer::new(fonts());
    let loaded = renderer.handle(ToRenderer::Panel(Box::new(Panel {
        hidden: true,
        viewport: Size::new(320.0, HEIGHT),
        ..panel_of(chats)
    })));
    assert!(matches!(loaded, FromRenderer::Loaded { .. }), "{loaded:?}");
    let _ = renderer.handle(ToRenderer::Paint);
    let mut at = |message: fn(Point) -> ToRenderer, x: f32, y: f32| match renderer
        .handle(message(Point::new(x, y)))
    {
        FromRenderer::ConversationClaimed(claim) => claim,
        other => panic!("{other:?}"),
    };
    for (place, top) in [(0, 121.0), (1, 171.0), (2, 221.0)] {
        for (x, y) in [(32.0, top + 22.0), (10.0, top), (53.0, top + 43.0)] {
            assert_eq!(
                at(ToRenderer::ConversationPress, x, y),
                Some(Claim::Chat { at: place }),
                "({x}, {y})"
            );
            assert_eq!(
                at(ToRenderer::ConversationHover, x, y),
                Some(Claim::Chat { at: place }),
                "({x}, {y})"
            );
        }
    }
    // Between two, and under the last.
    for (x, y) in [(32.0, 168.0), (32.0, 280.0)] {
        assert_eq!(at(ToRenderer::ConversationPress, x, y), None, "({x}, {y})");
    }
    // The name beside the hovered one is inside its button, so were a press
    // there sent it would be that chat's; the window sends none, since it
    // stands past the rail's room, over the first screen.
    assert_eq!(
        at(ToRenderer::ConversationPress, 120.0, 193.0),
        Some(Claim::Chat { at: 1 })
    );
}

/// Queue item 860: chats named by their hosts (859) have their sites'
/// letters on the rail — past *www*, *m* and a language — not each `H`
/// or `W`, so they can be told apart.
#[test]
fn chats_named_by_addresses_have_their_sites_letters_on_the_rail() {
    let chats = vec![
        Chat::new("en.wikipedia.org", "en.wikipedia.org").current(),
        Chat::new("www.bbc.com", "www.bbc.com").doing(Doing::Working),
        Chat::new("m.youtube.com", "m.youtube.com").doing(Doing::Done),
        // A name kept before 859 that a person gave: a whole address.
        Chat::new("https://www.theguardian.com/international", "").earlier(),
        Chat::new("127.0.0.1:8736", "").earlier(),
    ];
    let drawn = draw(
        Panel {
            hidden: true,
            viewport: Size::new(64.0, 600.0),
            ..panel_of(chats)
        }
        .at_scale(2.0),
    );
    let outline = drawn.tree.to_outline();
    // An agent reads each by its name, as before.
    let named: Vec<Option<&str>> = read(&drawn.tree, "button")
        .iter()
        .map(|button| button.name.as_deref())
        .collect();
    let done = format!("m.youtube.com {DONE}");
    assert_eq!(
        named,
        vec![
            Some("New chat"),
            Some("en.wikipedia.org"),
            Some("www.bbc.com"),
            Some(done.as_str()),
            Some("https://www.theguardian.com/international"),
            Some("127.0.0.1:8736"),
            Some("Show the panel"),
        ],
        "{outline}"
    );
    // The letters drawn, in the rail's order: five, all different.
    let letters: Vec<&str> = drawn
        .texts
        .iter()
        .map(String::as_str)
        .filter(|text| text.chars().count() <= 2 && *text != "alo")
        .collect();
    assert_eq!(letters, vec!["W", "B", "Y", "T", "1"], "{:?}", drawn.texts);
    // Each in its 32 circle where 760 put it.
    assert_eq!(
        of_class(&drawn, "initials"),
        vec![
            px(16.0, 127.0, 32.0, 32.0),
            px(16.0, 177.0, 32.0, 32.0),
            px(16.0, 227.0, 32.0, 32.0),
            px(16.0, 277.0, 32.0, 32.0),
            px(16.0, 327.0, 32.0, 32.0),
        ],
        "{outline}"
    );
    let frame = drawn.frame.unwrap_or_else(|| panic!("nothing was painted"));
    assert_eq!((frame.width, frame.height), (128, 1200));
    assert_eq!(holds("panel-rail-addresses.png", Some(&frame)), Ok(()));
}

/// Queue item 775: folded in a narrow window, the panel is the same rail
/// as hidden — in the window at the fold, 820 tall, and in the least
/// window, 600 tall — *Show the panel* 12 above its foot.
#[test]
fn folded_the_rail_is_laid_out_at_the_folds_height_and_the_least_windows() {
    for height in [820.0, 600.0] {
        let drawn = draw(Panel {
            hidden: true,
            viewport: Size::new(64.0, height),
            ..panel_of(five())
        });
        let outline = drawn.tree.to_outline();
        let buttons: Vec<(Option<&str>, String)> = read(&drawn.tree, "button")
            .iter()
            .map(|button| (button.name.as_deref(), units(button.rect)))
            .collect();
        // Five chats, the most the rail holds (queue item 760), still clear
        // of Show the panel in the least window.
        let done = format!("Invoice for March {DONE}");
        assert_eq!(
            buttons,
            vec![
                (Some("New chat"), px(10.0, 56.0, 44.0, 44.0)),
                (Some("Charging my EV"), px(10.0, 121.0, 44.0, 44.0)),
                (Some("Weekend in Lisbon"), px(10.0, 171.0, 44.0, 44.0)),
                (Some(done.as_str()), px(10.0, 221.0, 44.0, 44.0)),
                (Some("Compare phone plans"), px(10.0, 271.0, 44.0, 44.0)),
                (Some("Birthday present ideas"), px(10.0, 321.0, 44.0, 44.0)),
                (Some("Show the panel"), px(10.0, height - 56.0, 44.0, 44.0)),
            ],
            "{height}: {outline}"
        );
        assert!(read(&drawn.tree, "link").is_empty(), "no row: {outline}");
        let frame = drawn.frame.unwrap_or_else(|| panic!("nothing was painted"));
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "820 or 600, whole and positive"
        )]
        let tall = height as u32;
        assert_eq!((frame.width, frame.height), (64, tall));
    }
}

#[test]
fn an_empty_panel_is_the_panels_colour_new_chat_and_settings() {
    let drawn = draw(panel_of(Vec::new()));
    assert!(matches!(drawn.answer, FromRenderer::Loaded { .. }));
    assert!(read(&drawn.tree, "heading").is_empty());
    let names: Vec<Option<&str>> = read(&drawn.tree, "link")
        .iter()
        .map(|link| link.name.as_deref())
        .collect();
    assert_eq!(names, vec![Some("New chat"), Some("Settings")]);
    let frame = drawn.frame.unwrap_or_else(|| panic!("nothing was painted"));
    // The panel's warm grey, #eeeae2, where nothing is drawn on it.
    assert_eq!(pixel(&frame, 2, 2), Some([0xee, 0xea, 0xe2, 0xff]));
    assert_eq!(pixel(&frame, 120, 300), Some([0xee, 0xea, 0xe2, 0xff]));
}

#[test]
fn a_hostile_name_stays_text_in_the_tree_and_in_the_render() {
    for name in [
        "<b>bold</b>",
        "</style><script>steal()</script>",
        "a\0b",
        "\u{202e}gnp.exe",
    ] {
        let drawn = draw(panel_of(vec![
            Chat::new("Before", "a.example"),
            Chat::new(name, "<i>hostile</i>").current(),
            Chat::new("After", "c.example"),
        ]));
        let said = format!("{name:?}");
        assert!(
            matches!(drawn.answer, FromRenderer::Loaded { .. }),
            "{said}: {:?}",
            drawn.answer
        );
        let links = read(&drawn.tree, "link");
        let names: Vec<Option<&str>> = links.iter().map(|link| link.name.as_deref()).collect();
        assert_eq!(
            names,
            vec![
                Some("New chat"),
                Some("Before"),
                Some(name),
                Some("After"),
                Some("Settings")
            ],
            "{said}"
        );
        let roles: Vec<&str> = drawn
            .tree
            .nodes()
            .iter()
            .map(|node| node.role.as_str())
            .collect();
        assert!(!roles.contains(&"strong"), "{said}: {roles:?}");
        assert!(drawn.texts.iter().any(|text| text == "Before"), "{said}");
        assert!(drawn.texts.iter().any(|text| text == "After"), "{said}");
    }
}

#[test]
fn a_long_name_is_cut_by_the_stylesheet_to_its_row() {
    let drawn = draw(panel_of(vec![
        Chat::new("x".repeat(100_000), "long.example").current(),
    ]));
    let row = read(&drawn.tree, "link")
        .into_iter()
        .find(|link| link.states.current.is_some())
        .map(|link| units(link.rect));
    assert_eq!(row, Some(px(10.0, 178.0, 220.0, 38.0)));
    let xs: Vec<&String> = drawn
        .texts
        .iter()
        .filter(|text| text.starts_with('x'))
        .collect();
    let [xs] = xs.as_slice() else {
        panic!("one run of x's, not {:?}", drawn.texts);
    };
    // A row of 220, less 20 of padding, 16 of mark and 10 of gap, holds 174
    // px of x's and the ellipsis: far fewer than a thousand.
    assert!(xs.chars().all(|c| c == 'x') && xs.len() < 40, "{xs:?}");
    assert!(
        drawn.texts.iter().any(|text| text == "…"),
        "{:?}",
        drawn.texts
    );
}

/// The renderer binary, confined as every renderer is, with the corpus's
/// fonts.
fn renderers() -> Renderers {
    let faces = faces()
        .into_iter()
        .filter_map(|(family, weight, bytes)| {
            Face::new(family, weight, Slant::Normal, bytes.to_vec())
        })
        .collect();
    let generics = Generics::stating(
        generics()
            .into_iter()
            .map(|(generic, family)| (generic.to_owned(), family.to_owned()))
            .collect(),
    );
    Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[])
        .with_fonts([faces, alo_renderer::typefaces::faces()].concat())
        .with_generics(generics)
}

#[test]
fn the_panel_is_drawn_the_same_by_a_confined_renderer_at_its_own_site() {
    let mut renderers = renderers();
    let site = panel::site();
    let mut chats = five();
    chats.push(Chat::new("<b>a\0b</b>\u{202e}", "hostile.example"));
    let panel = panel_of(chats);

    let loaded = renderers.ask(&site, &ToRenderer::Panel(Box::new(panel.clone())));
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    assert_eq!(renderers.len(), 1);

    let here = draw(panel);
    let painted = renderers.ask(&site, &ToRenderer::Paint);
    assert_eq!(
        painted.as_ref().ok(),
        here.frame.map(FromRenderer::Painted).as_ref(),
        "the confined renderer drew another panel"
    );
    let read = renderers.ask(&site, &ToRenderer::ReadTree);
    let Ok(FromRenderer::Tree(tree)) = read else {
        panic!("not a tree: {read:?}");
    };
    assert_eq!(tree.to_outline(), here.tree.to_outline());
    renderers.stop_everything();
}

/// Queue item 789: the hovered row's *Delete* — an earlier chat's, since an
/// open one offers its × instead (queue item 791) — and the foot's *Undo*.
fn deleting() -> Panel {
    let mut chats = five();
    chats.truncate(4);
    if let Some(compare) = chats.get_mut(3) {
        compare.hovered = true;
    }
    Panel {
        deleted: Some("Birthday present ideas".to_owned()),
        ..panel_of(chats)
    }
}

#[test]
fn a_hovered_chat_offers_delete_and_a_chat_just_deleted_offers_undo() {
    let drawn = draw(deleting());
    assert!(
        matches!(&drawn.answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{:?}",
        drawn.answer
    );
    let outline = drawn.tree.to_outline();
    // The rows are where they were: the hovered one did not move for its
    // Delete, nor Settings for the Undo above it.
    let links: Vec<(Option<&str>, String)> = read(&drawn.tree, "link")
        .iter()
        .map(|link| (link.name.as_deref(), units(link.rect)))
        .collect();
    let done = format!("Invoice for March {DONE}");
    assert_eq!(
        links,
        vec![
            (Some("New chat"), px(10.0, 56.0, 220.0, 40.0)),
            (Some("Charging my EV"), px(10.0, 178.0, 220.0, 38.0)),
            (Some("Weekend in Lisbon"), px(10.0, 218.0, 220.0, 38.0)),
            (Some(done.as_str()), px(10.0, 258.0, 220.0, 38.0)),
            // The hovered row holds Delete, and is read with it, as a
            // finished chat is read with its Done.
            (
                Some("Compare phone plans Delete"),
                px(10.0, 334.0, 220.0, 38.0)
            ),
            (Some("Settings"), px(10.0, 510.0, 220.0, 38.0)),
        ],
        "{outline}"
    );
    // Delete: 24 tall in the middle of its 38-row (7 down), its right edge
    // at the row's padding (230 less 10); Undo: 26 tall in the middle of
    // the 36-pill, its right edge 6 inside the pill's; the pill 8 above
    // Settings, the whole panel's width less its padding.
    let buttons: Vec<(Option<&str>, Rect)> = read(&drawn.tree, "button")
        .iter()
        .map(|button| (button.name.as_deref(), button.rect))
        .collect();
    let [hide, delete, undo] = buttons.as_slice() else {
        panic!("three buttons, not {buttons:?}\n{outline}");
    };
    assert_eq!(hide.0, Some("Hide the panel"));
    assert_eq!(delete.0, Some("Delete Compare phone plans"));
    assert_eq!(
        (
            units(Rect::new(
                delete.1.right(),
                delete.1.top(),
                0.0,
                delete.1.size.height
            )),
            delete.1.size.width > 40.0 && delete.1.size.width < 70.0
        ),
        (px(220.0, 341.0, 0.0, 24.0), true),
        "{outline}"
    );
    assert_eq!(undo.0, Some("Undo"));
    assert_eq!(
        (
            units(Rect::new(
                undo.1.right(),
                undo.1.top(),
                0.0,
                undo.1.size.height
            )),
            undo.1.size.width > 30.0 && undo.1.size.width < 60.0
        ),
        (px(224.0, 471.0, 0.0, 26.0), true),
        "{outline}"
    );
    let status: Vec<String> = read(&drawn.tree, "status")
        .iter()
        .map(|status| units(status.rect))
        .collect();
    assert_eq!(status, vec![px(10.0, 466.0, 220.0, 36.0)], "{outline}");
    assert!(
        drawn
            .texts
            .iter()
            .any(|text| text.contains("Deleted \u{201c}Birthday")),
        "{:?}",
        drawn.texts
    );
    assert_eq!(holds("panel-deleting.png", drawn.frame.as_ref()), Ok(()));
}

#[test]
fn a_press_on_delete_or_undo_claims_it_and_a_hover_finds_the_row() {
    use alo_paint::Point;
    use alo_renderer::conversation_claim::Claim;
    let mut renderer = Renderer::new(fonts());
    let loaded = renderer.handle(ToRenderer::Panel(Box::new(deleting())));
    assert!(matches!(loaded, FromRenderer::Loaded { .. }), "{loaded:?}");
    let _ = renderer.handle(ToRenderer::Paint);
    let mut at =
        |message: fn(Point) -> ToRenderer, x, y| match renderer.handle(message(Point::new(x, y))) {
            FromRenderer::ConversationClaimed(claim) => claim,
            other => panic!("{other:?}"),
        };
    // Delete, at the hovered row's end; the row elsewhere; Undo; the pill
    // beside it, which is nothing.
    assert_eq!(
        at(ToRenderer::ConversationPress, 200.0, 353.0),
        Some(Claim::DeleteChat)
    );
    assert_eq!(
        at(ToRenderer::ConversationPress, 60.0, 353.0),
        Some(Claim::Chat { at: 3 })
    );
    assert_eq!(
        at(ToRenderer::ConversationPress, 210.0, 484.0),
        Some(Claim::Undo)
    );
    assert_eq!(at(ToRenderer::ConversationPress, 40.0, 484.0), None);
    // A hover is answered as a press is: what is under the pointer.
    assert_eq!(
        at(ToRenderer::ConversationHover, 60.0, 277.0),
        Some(Claim::Chat { at: 2 })
    );
    assert_eq!(
        at(ToRenderer::ConversationHover, 200.0, 353.0),
        Some(Claim::DeleteChat)
    );
    assert_eq!(at(ToRenderer::ConversationHover, 120.0, 20.0), None);
}

/// Queue item 791: an open chat's hovered row offers its × at the row's
/// end, where an earlier one's offers *Delete* — never both — and the
/// empty chat on screen, which would only come back as *New chat*, offers
/// none.
#[test]
fn a_hovered_open_chat_offers_its_close_and_the_empty_chat_on_screen_none() {
    use alo_paint::Point;
    use alo_renderer::conversation_claim::Claim;
    let mut chats = five();
    if let Some(lisbon) = chats.get_mut(1) {
        lisbon.hovered = true;
    }
    let drawn = draw(panel_of(chats.clone()));
    let outline = drawn.tree.to_outline();
    let links: Vec<(Option<&str>, String)> = read(&drawn.tree, "link")
        .iter()
        .map(|link| (link.name.as_deref(), units(link.rect)))
        .collect();
    assert_eq!(
        links.get(2),
        Some(&(Some("Weekend in Lisbon"), px(10.0, 218.0, 220.0, 38.0))),
        "the row did not move: {outline}"
    );
    let buttons: Vec<(Option<&str>, String)> = read(&drawn.tree, "button")
        .iter()
        .map(|button| (button.name.as_deref(), units(button.rect)))
        .collect();
    // 24 square, 7 down its 38-row, its right edge at the row's padding.
    assert_eq!(
        buttons,
        vec![
            (Some("Hide the panel"), px(194.0, 11.0, 36.0, 36.0)),
            (
                Some("Close Weekend in Lisbon"),
                px(196.0, 225.0, 24.0, 24.0)
            ),
        ],
        "{outline}"
    );
    assert!(
        !buttons
            .iter()
            .any(|(name, _)| name.is_some_and(|name| name.starts_with("Delete"))),
        "an open row offers no Delete: {outline}"
    );
    // Its cross, 12 square in the middle of it.
    assert!(
        drawn.icons.contains(&px(202.0, 231.0, 12.0, 12.0)),
        "{:?}",
        drawn.icons
    );
    let frame = drawn.frame.as_ref();
    // The ×'s ground beside its cross, and the hovered row's lighter one.
    assert_eq!(
        frame.and_then(|frame| pixel(frame, 199, 237)),
        Some([0xe1, 0xda, 0xce, 0xff])
    );
    assert_eq!(
        frame.and_then(|frame| pixel(frame, 150, 222)),
        Some([0xf6, 0xf3, 0xee, 0xff])
    );
    assert_eq!(holds("panel-closing.png", frame), Ok(()));

    // Pressed and hovered, it is the ×; the row beside it is the chat.
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Panel(Box::new(panel_of(chats))));
    let _ = renderer.handle(ToRenderer::Paint);
    let mut at = |message: fn(Point) -> ToRenderer, x: f32, y: f32| match renderer
        .handle(message(Point::new(x, y)))
    {
        FromRenderer::ConversationClaimed(claim) => claim,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        at(ToRenderer::ConversationPress, 208.0, 237.0),
        Some(Claim::CloseChat)
    );
    assert_eq!(
        at(ToRenderer::ConversationHover, 208.0, 237.0),
        Some(Claim::CloseChat)
    );
    assert_eq!(
        at(ToRenderer::ConversationPress, 120.0, 237.0),
        Some(Claim::Chat { at: 1 })
    );

    // The empty chat on screen, hovered and not closable, offers nothing.
    let empty = draw(panel_of(vec![
        Chat::new("", "").current().hovered().not_closable(),
        Chat::new("Weekend in Lisbon", ""),
    ]));
    let names: Vec<Option<&str>> = read(&empty.tree, "button")
        .iter()
        .map(|button| button.name.as_deref())
        .collect();
    assert_eq!(names, vec![Some("Hide the panel")]);
}
