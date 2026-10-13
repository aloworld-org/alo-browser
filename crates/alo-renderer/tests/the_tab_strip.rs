/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 297: the tab strip is a document, rendered by the engine (ADR
//! 0024 § 4).
//!
//! What closes it, each a test below: reference renders of one, four and a
//! gone tab's strip; a layout assertion of each tab's box; the agent reading
//! *"four tabs, the second selected"* with each title; and hostile titles —
//! `<b>`, `</style>`, a NUL, 100 000 characters, a right-to-left override —
//! each staying text in the tree and in the render, the long one cut by the
//! stylesheet, none panicking. Last, the same strip through the real,
//! confined renderer at the strip's own site, to show it crosses and draws
//! the same there.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test the_tab_strip`
//! rewrites the references. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{DisplayItem, from_png};
use alo_renderer::face::Face;
use alo_renderer::frame::frame_to_png;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::strip::{self, Condition, LONGEST_TITLE, Strip, StripTab};
use alo_renderer::strip_document::GONE;
use alo_renderer::{Frame, FromRenderer, Renderer, Snapshot, SnapshotNode, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::{Path, PathBuf};

/// The corpus's sans-serif, for both weights, and what `system-ui` and
/// `sans-serif` mean: the strip's stylesheet asks for them in that order.
fn faces() -> Vec<(Weight, &'static [u8])> {
    vec![
        (Weight::NORMAL, dejavu::sans::regular()),
        (Weight::BOLD, dejavu::sans::bold()),
    ]
}

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (weight, bytes) in faces() {
        if let Some(font) = Font::load("DejaVu Sans", weight, Slant::Normal, bytes.to_vec()) {
            database.add(font);
        }
    }
    database.map_generic("system-ui", "DejaVu Sans");
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

/// The strip's height in the window: 6 above the tabs and 30 of tab.
const HEIGHT: f32 = 36.0;

fn strip_of(tabs: Vec<StripTab>, width: f32) -> Strip {
    Strip {
        tabs,
        viewport: Size::new(width, HEIGHT),
        scale: 1.0,
    }
}

/// Four tabs, the second selected, the third loading.
fn four() -> Vec<StripTab> {
    vec![
        StripTab::new("Inbox", "https://mail.example/"),
        StripTab::new("Invoices", "https://books.example/invoices").selected(),
        StripTab::new("Calendar", "https://cal.example/").in_condition(Condition::Loading),
        StripTab::new("", "https://notes.example/today"),
    ]
}

/// What a renderer holding a strip answers to a paint and a reading.
struct Drawn {
    answer: FromRenderer,
    frame: Option<Frame>,
    tree: Snapshot,
    /// Every text the strip's drawing put down, in order.
    texts: Vec<String>,
}

fn draw(strip: Strip) -> Drawn {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Strip(Box::new(strip)));
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
    Drawn {
        answer,
        frame,
        tree,
        texts,
    }
}

/// Every tab the agent reads, in order.
fn tabs(tree: &Snapshot) -> Vec<&SnapshotNode> {
    tree.nodes()
        .into_iter()
        .filter(|node| node.role.as_str() == "tab")
        .collect()
}

/// A rectangle as the outline writes it, the shortest decimal of each
/// number: layout holds whole 1/64 px, so two rectangles that write alike
/// are the same, compared exactly without comparing floats.
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
            "{name}: the strip is {}×{}, and the reference {}×{}",
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

#[test]
fn a_strip_of_one_tab_is_one_tab_its_full_width() {
    let drawn = draw(strip_of(
        vec![StripTab::new("alo", "file:///alo/offline.html").selected()],
        480.0,
    ));
    assert!(
        matches!(&drawn.answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{:?}",
        drawn.answer
    );
    let read = tabs(&drawn.tree);
    let rects: Vec<_> = read.iter().map(|tab| units(tab.rect)).collect();
    // The strip's padding is 8 across and 6 down; a tab is 200 wide when
    // there is room, and 30 tall to the strip's foot.
    assert_eq!(rects, vec![px(8.0, 6.0, 200.0, 30.0)]);
    assert_eq!(holds("strip-one-tab.png", drawn.frame.as_ref()), Ok(()));
}

#[test]
fn four_tabs_share_the_strip_and_the_agent_reads_them_by_title() {
    let drawn = draw(strip_of(four(), 640.0));
    let outline = drawn.tree.to_outline();

    // "Tab list, four tabs, the second selected", each by its title, and the
    // one with no title by its address.
    let list: Vec<_> = drawn
        .tree
        .nodes()
        .into_iter()
        .filter(|node| node.role.as_str() == "tablist")
        .collect();
    assert_eq!(list.len(), 1, "{outline}");
    assert_eq!(
        list.first().and_then(|node| node.name.as_deref()),
        Some("Tabs")
    );
    let read = tabs(&drawn.tree);
    let names: Vec<Option<&str>> = read.iter().map(|tab| tab.name.as_deref()).collect();
    assert_eq!(
        names,
        vec![
            Some("Inbox"),
            Some("Invoices"),
            Some("Calendar"),
            Some("https://notes.example/today")
        ],
        "{outline}"
    );
    let selected: Vec<Option<bool>> = read.iter().map(|tab| tab.states.selected).collect();
    assert_eq!(
        selected,
        vec![Some(false), Some(true), Some(false), Some(false)]
    );
    let busy: Vec<bool> = read.iter().map(|tab| tab.states.busy).collect();
    assert_eq!(busy, vec![false, false, true, false]);
    assert!(
        outline.contains("tab \"Invoices\" [selected=true]"),
        "{outline}"
    );

    // 640 less 16 of padding and three gaps of 4 is 612, which four tabs
    // asking 200 each shrink into equally: 153 each, 157 apart.
    let rects: Vec<_> = read.iter().map(|tab| units(tab.rect)).collect();
    assert_eq!(
        rects,
        vec![
            px(8.0, 6.0, 153.0, 30.0),
            px(165.0, 6.0, 153.0, 30.0),
            px(322.0, 6.0, 153.0, 30.0),
            px(479.0, 6.0, 153.0, 30.0),
        ]
    );
    assert_eq!(holds("strip-four-tabs.png", drawn.frame.as_ref()), Ok(()));
}

#[test]
fn a_gone_tab_says_so_in_words_a_person_and_an_agent_both_read() {
    let drawn = draw(strip_of(
        vec![
            StripTab::new("Inbox", "https://mail.example/"),
            StripTab::new("Reports", "https://reports.example/")
                .selected()
                .in_condition(Condition::Gone),
        ],
        480.0,
    ));
    let read = tabs(&drawn.tree);
    let names: Vec<Option<&str>> = read.iter().map(|tab| tab.name.as_deref()).collect();
    let gone = format!("Reports {GONE}");
    assert_eq!(names, vec![Some("Inbox"), Some(gone.as_str())]);
    assert!(drawn.texts.iter().any(|text| text.trim() == GONE));
    let rects: Vec<_> = read.iter().map(|tab| units(tab.rect)).collect();
    assert_eq!(
        rects,
        vec![px(8.0, 6.0, 200.0, 30.0), px(212.0, 6.0, 200.0, 30.0)]
    );
    assert_eq!(holds("strip-a-gone-tab.png", drawn.frame.as_ref()), Ok(()));
}

#[test]
fn an_empty_strip_is_the_rail_and_no_tabs() {
    let drawn = draw(strip_of(Vec::new(), 200.0));
    assert!(matches!(drawn.answer, FromRenderer::Loaded { .. }));
    assert!(tabs(&drawn.tree).is_empty());
    let frame = drawn.frame.unwrap_or_else(|| panic!("nothing was painted"));
    // The rail's warm charcoal, #1f2529, everywhere.
    assert!(
        frame
            .pixels
            .chunks(4)
            .all(|pixel| pixel == [0x1f, 0x25, 0x29, 0xff])
    );
}

/// Each hostile title on its own, between two plain tabs.
fn hostile_titles() -> Vec<String> {
    vec![
        "<b>bold</b>".to_owned(),
        "</style><script>steal()</script>".to_owned(),
        "a\0b".to_owned(),
        "x".repeat(100_000),
        "\u{202e}gnp.exe".to_owned(),
    ]
}

#[test]
fn a_hostile_title_stays_text_in_the_tree_and_in_the_render() {
    for title in hostile_titles() {
        let shown: String = title.chars().take(LONGEST_TITLE).collect();
        let drawn = draw(strip_of(
            vec![
                StripTab::new("Inbox", "https://mail.example/"),
                StripTab::new(title.clone(), "https://hostile.example/").selected(),
                StripTab::new("Calendar", "https://cal.example/"),
            ],
            640.0,
        ));
        let said = format!("{:?}", title.chars().take(40).collect::<String>());
        assert!(
            matches!(drawn.answer, FromRenderer::Loaded { .. }),
            "{said}: {:?}",
            drawn.answer
        );

        // In the tree: still three tabs, the hostile one named by exactly
        // its characters, and nothing it wrote became an element.
        let read = tabs(&drawn.tree);
        assert_eq!(read.len(), 3, "{said}");
        let name = read.get(1).and_then(|tab| tab.name.clone());
        assert_eq!(name.as_deref(), Some(shown.as_str()), "{said}");
        let roles: Vec<&str> = drawn
            .tree
            .nodes()
            .iter()
            .map(|node| node.role.as_str())
            .collect();
        assert_eq!(
            roles,
            vec!["document", "tablist", "tab", "tab", "tab"],
            "{said}"
        );

        // In the render: its characters are drawn as text, from the start,
        // and the other two tabs are still drawn.
        let drawn_title = drawn
            .texts
            .iter()
            .find(|text| shown.starts_with(text.as_str()) && !text.is_empty());
        assert!(drawn_title.is_some(), "{said}: {:?}", drawn.texts);
        assert!(drawn.texts.iter().any(|text| text == "Inbox"), "{said}");
        assert!(drawn.texts.iter().any(|text| text == "Calendar"), "{said}");
        assert!(drawn.frame.is_some(), "{said}");
    }
}

#[test]
fn a_long_title_is_cut_by_the_stylesheet_to_its_tab() {
    let drawn = draw(strip_of(
        vec![StripTab::new("x".repeat(100_000), "https://long.example/").selected()],
        480.0,
    ));
    // Cut to the tab: some x's, then the ellipsis, and nothing past it.
    let texts: Vec<&str> = drawn.texts.iter().map(String::as_str).collect();
    let [xs, ellipsis] = texts.as_slice() else {
        panic!("one run and its ellipsis, not {:?}", drawn.texts);
    };
    assert!(xs.chars().all(|c| c == 'x') && !xs.is_empty(), "{xs:?}");
    // A tab of 200, less 24 of padding, holds 176 px of x's and the ellipsis:
    // far fewer than the thousand characters the strip keeps.
    assert!(xs.len() < 40, "{} x's were drawn", xs.len());
    assert_eq!(*ellipsis, "…");
    assert_eq!(
        tabs(&drawn.tree).first().map(|tab| units(tab.rect)),
        Some(px(8.0, 6.0, 200.0, 30.0))
    );
}

/// The renderer binary, confined as every renderer is, with the corpus's
/// fonts.
fn renderers() -> Renderers {
    let faces = faces()
        .into_iter()
        .filter_map(|(weight, bytes)| {
            Face::new("DejaVu Sans", weight, Slant::Normal, bytes.to_vec())
        })
        .collect();
    let generics = Generics::stating(vec![
        ("system-ui".to_owned(), "DejaVu Sans".to_owned()),
        ("sans-serif".to_owned(), "DejaVu Sans".to_owned()),
    ]);
    Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[])
        .with_fonts(faces)
        .with_generics(generics)
}

#[test]
fn the_strip_is_drawn_the_same_by_a_confined_renderer_at_its_own_site() {
    let mut renderers = renderers();
    let site = strip::site();
    let mut hostile = four();
    hostile.push(StripTab::new(
        "<b>a\0b</b>\u{202e}",
        "https://hostile.example/",
    ));
    let strip = strip_of(hostile, 800.0);

    let loaded = renderers.ask(&site, &ToRenderer::Strip(Box::new(strip.clone())));
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    assert_eq!(renderers.len(), 1);
    assert!(renderers.process_of(&site).is_some());

    let here = draw(strip);
    let painted = renderers.ask(&site, &ToRenderer::Paint);
    assert_eq!(
        painted.as_ref().ok(),
        here.frame.map(FromRenderer::Painted).as_ref(),
        "the confined renderer drew another strip"
    );
    let read = renderers.ask(&site, &ToRenderer::ReadTree);
    let Ok(FromRenderer::Tree(tree)) = read else {
        panic!("not a tree: {read:?}");
    };
    assert_eq!(tree.to_outline(), here.tree.to_outline());
    renderers.stop_everything();
}
