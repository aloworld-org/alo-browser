/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 707 (ADR 0081, ADR 0095): a search's answer, written by the
//! model from the sites found, drawn as `docs/design/shell.md` asks — the
//! short answer at the top of All, two or three sentences with their
//! sources' numbers, *Full answer ›* and how many sites it drew on, a line
//! under it, and *alo used this* on the sites it drew on; and the Answer
//! tab, the answer in one line in the serif, its short points, and its
//! sources by name.
//!
//! What closes its engine half, each a test below: a layout assertion of
//! both in numbers; a reference render of each,
//! `references/all-tab-with-an-answer.png` and
//! `references/answer-tab.png`; the agent reading the answer and its
//! sources by name; one sentence in the answer's place while it is written,
//! when no reliable answer was found, and when it was not answered; a press
//! on *Full answer ›* claimed by its place; and the same through the real,
//! confined renderer, which refuses an answer citing a site it was not
//! given.
//!
//! The answer and the sites here are written by hand for this test, never
//! captured from a provider.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test a_searchs_answer`
//! rewrites the references. A rewritten reference is a diff somebody reads.

use alo_layout::{Rect, Size};
use alo_paint::{Point, from_png};
use alo_renderer::conversation_claim::Claim;
use alo_renderer::face::Face;
use alo_renderer::frame::frame_to_png;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::results::{Found, Results, SetUp, Showing, Tab};
use alo_renderer::results_answer::{Answer, Cited, Written};
use alo_renderer::start::{self, Start};
use alo_renderer::turn::Turn;
use alo_renderer::{Frame, FromRenderer, Renderer, Snapshot, SnapshotNode, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use std::path::{Path, PathBuf};

/// The corpus's faces: its sans-serif for both weights, which is what
/// `system-ui` and `sans-serif` mean here, and its serif for the greeting.
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

/// The room the first screen has in the design's window: 1280 wide less the
/// panel's 240, and the window's 820 tall.
const WIDTH: f32 = 1040.0;
const HEIGHT: f32 = 820.0;

/// What a renderer holding a first screen answers to a paint and a reading.
struct Drawn {
    answer: FromRenderer,
    frame: Option<Frame>,
    tree: Snapshot,
    /// Where each icon was laid out, in document order. The agent does not
    /// read them — they are `aria-hidden` — so they are found in the
    /// drawing's own boxes, by the element each box is for.
    icons: Vec<String>,
}

fn draw(start: Start) -> Drawn {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Start(Box::new(start)));
    let frame = match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Some(frame),
        _ => None,
    };
    let tree = match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(tree) => *tree,
        _ => Snapshot::default(),
    };
    let icons = icons(&renderer);
    Drawn {
        answer,
        frame,
        tree,
        icons,
    }
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

fn names(nodes: &[&SnapshotNode]) -> Vec<Option<String>> {
    nodes.iter().map(|node| node.name.clone()).collect()
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

/// The colour of the pixel at `x`, `y` of `frame`.
fn pixel(frame: &Frame, x: u32, y: u32) -> Option<[u8; 4]> {
    let at = usize::try_from((y * frame.width + x) * 4).ok()?;
    let rgba = frame.pixels.get(at..at + 4)?;
    rgba.try_into().ok()
}

/// The border box of every element of `class`, in document order.
fn boxes_of(renderer: &Renderer, class: &str) -> Vec<String> {
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
        .map(|geometry| units(geometry.border_box))
        .collect()
}

/// What a renderer holding `start` claims a press at `x`, `y` is on.
fn pressed(start: Start, x: f32, y: f32) -> FromRenderer {
    let mut renderer = Renderer::new(fonts());
    let _ = renderer.handle(ToRenderer::Start(Box::new(start)));
    let _ = renderer.handle(ToRenderer::Paint);
    renderer.handle(ToRenderer::ConversationPress(Point::new(x, y)))
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

/// What the person searched for.
const SAID: &str = "how long does a home ev charger take to install";

fn site(title: &str, address: &str, description: &str, name: &str) -> Found {
    Found::new(
        title,
        alo_url::parse(address).unwrap_or_else(|_| alo_url::Url::about_blank()),
        description,
    )
    .named(name)
}

/// Four sites as a provider might answer them: All draws three.
fn four() -> Vec<Found> {
    vec![
        site(
            "Installing a charger at home: what to expect",
            "https://evhomeguide.org/installing-a-charger",
            "A wall charger needs its own circuit from your fuse box. Most fittings take half a \
             day to a day.",
            "EV Home Guide",
        ),
        site(
            "Home EV chargers compared",
            "https://energyreport.example/home-chargers",
            "Cable length, smart scheduling and app control, explained in plain terms.",
            "",
        ),
        site(
            "Do I need a new circuit for a charger?",
            "https://electricians.example/new-circuit",
            "A new circuit or a panel upgrade can add a day or more, and a permit may be needed.",
            "Electricians",
        ),
        site(
            "Charger grants",
            "https://grants.example/",
            "What a grant covers.",
            "",
        ),
    ]
}

/// The answer, as the model might write it from [`four`]: it draws on the
/// first, third and fourth sites, not the second.
fn written() -> Written {
    Written {
        line: "Most home chargers take half a day to a day to install.".to_owned(),
        sentences: vec![
            Cited::new(
                "A wall charger on its own circuit usually takes half a day to a day to fit.",
                [1],
            ),
            Cited::new(
                "A new circuit, a panel upgrade or a permit can add a day or more.",
                [3],
            ),
            Cited::new("A grant may cover part of the cost.", [4]),
        ],
        points: vec![
            Cited::new("Half a day to a day for a straightforward fitting.", [1]),
            Cited::new("A day or more when a new circuit or panel is needed.", [3]),
            Cited::new("A permit may be needed first.", [3]),
        ],
    }
}

fn with(answer: Answer, tab: Tab) -> Start {
    Start {
        greeting: "Hello.".to_owned(),
        viewport: Size::new(WIDTH, HEIGHT),
        turns: vec![Turn::Searched(
            Results::of(SAID, Showing::Found(four()))
                .with_answer(answer)
                .on(tab),
        )],
        search: SetUp::Ready,
        ..Start::default()
    }
}

fn answered() -> Start {
    with(Answer::Written(written()), Tab::All)
}

fn answer_tab() -> Start {
    with(Answer::Written(written()), Tab::Answer)
}

#[test]
fn the_short_answer_is_laid_out_over_the_sites_as_the_design_draws_it() {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Start(Box::new(answered())));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{answer:?}"
    );
    let at = |class: &str| boxes_of(&renderer, class);
    // 20 under the tab row (102 + 42) the answer: its sentences, two lines
    // of 26 in 760; 10 under them Full answer and how many sites, a line of
    // 18; 20 under that the rule: 52 + 10 + 18 + 20 + 1 = 101.
    assert_eq!(at("ans"), vec![px(80.0, 164.0, 880.0, 101.0)]);
    assert_eq!(at("ansp"), vec![px(80.0, 164.0, 760.0, 52.0)]);
    assert_eq!(at("ansf"), vec![px(80.0, 226.0, 880.0, 18.0)]);
    assert_eq!(at("full"), vec![px(80.0, 226.0, 80.046_875, 18.0)]);
    assert_eq!(at("ansn"), vec![px(174.046_88, 226.0, 95.109_375, 18.0)]);
    // Each number 16 square, after its sentence, inside the sentence's line.
    assert_eq!(
        at("cite"),
        vec![
            px(609.109_4, 172.0, 16.0, 16.0),
            px(409.765_63, 198.0, 16.0, 16.0),
            px(695.312_5, 198.0, 16.0, 16.0),
        ]
    );
    // 20 under the rule the sites, where All without an answer puts them
    // 121 higher (`a_searchs_results_on_the_all_tab.rs`: 164).
    assert_eq!(at("res"), vec![px(80.0, 285.0, 880.0, 270.0)]);
    assert_eq!(
        at("found"),
        vec![
            px(80.0, 289.0, 880.0, 74.0),
            px(80.0, 385.0, 880.0, 74.0),
            px(80.0, 481.0, 880.0, 74.0),
        ]
    );
    // alo used this on the first and third, after each one's address, on
    // its line; the second was not used.
    assert_eq!(
        at("used"),
        vec![
            px(460.984_38, 290.0, 82.203_125, 16.0),
            px(403.109_38, 482.0, 82.203_125, 16.0),
        ]
    );
    assert_eq!(at("more"), vec![px(80.0, 575.0, 79.968_75, 20.0)]);
    // The Answer tab is open now, second in the row.
    assert_eq!(at("rt on"), vec![px(80.0, 102.0, 45.515_625, 42.0)]);
    assert!(
        at("rt")
            .first()
            .is_some_and(|tab| tab == &px(129.515_63, 102.0, 77.968_75, 42.0)),
        "{:?}",
        at("rt")
    );
}

#[test]
fn the_answer_tab_is_laid_out_as_the_design_draws_it() {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Start(Box::new(answer_tab())));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{answer:?}"
    );
    let at = |class: &str| boxes_of(&renderer, class);
    assert_eq!(at("answer"), vec![px(80.0, 164.0, 880.0, 295.0)]);
    // The line in the serif, 30 tall, 4 down; 14 under it three points of
    // 24, 6 apart, 20 in; 14 under them the sources.
    assert_eq!(at("ansline"), vec![px(80.0, 168.0, 760.0, 30.0)]);
    assert_eq!(at("pts"), vec![px(80.0, 212.0, 760.0, 90.0)]);
    assert_eq!(
        at("pt"),
        vec![
            px(100.0, 212.0, 740.0, 24.0),
            px(100.0, 242.0, 740.0, 24.0),
            px(100.0, 272.0, 740.0, 24.0),
        ]
    );
    // The rule, 14 above the heading; the heading 18; each source a line
    // of 18, 8 apart: its number, then 8 after it the site's name.
    assert_eq!(at("srcs"), vec![px(80.0, 316.0, 880.0, 111.0)]);
    assert_eq!(at("srch"), vec![px(80.0, 331.0, 880.0, 18.0)]);
    assert_eq!(
        at("srci"),
        vec![
            px(80.0, 357.0, 880.0, 18.0),
            px(80.0, 383.0, 880.0, 18.0),
            px(80.0, 409.0, 880.0, 18.0),
        ]
    );
    assert!(
        at("sname")
            .iter()
            .all(|name| name.starts_with("(104, ") && name.ends_with("×18")),
        "{:?}",
        at("sname")
    );
    // 14 under the sources, what was sent and to whom: a line of 18.
    assert_eq!(at("sent"), vec![px(80.0, 441.0, 880.0, 18.0)]);
    // No site, no short answer, no More sites on this tab.
    assert!(at("found").is_empty() && at("ans").is_empty() && at("more").is_empty());
    assert_eq!(at("rt on"), vec![px(128.453_13, 102.0, 79.828_125, 42.0)]);
}

#[test]
fn the_short_answer_and_the_answer_tab_are_drawn_as_the_references() {
    for (start, name) in [
        (answered(), "all-tab-with-an-answer.png"),
        (answer_tab(), "answer-tab.png"),
    ] {
        let drawn = draw(start);
        assert!(
            matches!(&drawn.answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
            "{:?}",
            drawn.answer
        );
        // No icon of the answer's own: Go's arrow is the one drawn.
        assert_eq!(drawn.icons.len(), 1, "{:?}", drawn.icons);
        let frame = drawn.frame.as_ref();
        assert!(frame.is_some(), "nothing was painted");
        if let (Some(frame), true) = (frame, name.starts_with("all")) {
            // The rule under the answer, in #e1dace; a number's pill in
            // #e9e3d8 at its corner's inside; alo used this in #e3ece8.
            assert_eq!(pixel(frame, 500, 264), Some([0xe1, 0xda, 0xce, 0xff]));
            assert_eq!(pixel(frame, 611, 180), Some([0xe9, 0xe3, 0xd8, 0xff]));
            assert_eq!(pixel(frame, 463, 298), Some([0xe3, 0xec, 0xe8, 0xff]));
        }
        assert_eq!(holds(name, frame), Ok(()));
    }
}

#[test]
fn an_agent_reads_the_answer_and_its_sources_by_name() {
    let some = |name: &str| Some(name.to_owned());
    let drawn = draw(answered());
    let outline = drawn.tree.to_outline();
    assert_eq!(
        names(&read(&drawn.tree, "region")),
        vec![
            some("Conversation"),
            some(SAID),
            some("Search results"),
            some("alo\u{2019}s answer")
        ],
        "{outline}"
    );
    for sentence in written().sentences {
        assert!(outline.contains(&sentence.text), "{outline}");
    }
    assert!(
        outline.contains("button \"Full answer \u{203a}\""),
        "{outline}"
    );
    assert!(outline.contains("text \"Drew on 3 sites\""), "{outline}");
    assert_eq!(
        outline.matches("text \"alo used this\"").count(),
        2,
        "{outline}"
    );
    let tabs = read(&drawn.tree, "tab");
    assert_eq!(
        tabs.iter()
            .map(|tab| tab.states.disabled)
            .collect::<Vec<_>>(),
        vec![false, false, false, true, false, false],
        "Answer open once an answer was asked for: {outline}"
    );

    // The Answer tab: the line, the points, then the sources, each by its
    // number, name, address and title.
    let drawn = draw(answer_tab());
    let outline = drawn.tree.to_outline();
    assert_eq!(
        names(&read(&drawn.tree, "region")),
        vec![
            some("Conversation"),
            some(SAID),
            some("Search results"),
            some("alo\u{2019}s answer"),
            some("Sources"),
        ],
        "{outline}"
    );
    assert!(outline.contains(&written().line), "{outline}");
    assert_eq!(read(&drawn.tree, "listitem").len(), 3, "{outline}");
    for said in [
        "text \"3 sources\"",
        "text \"EV Home Guide\"",
        "text \"Electricians\"",
        "text \"grants.example\"",
        "text \"Do I need a new circuit for a charger?\"",
        "text \"Your question and what the search said of its first 4 sites were sent to \
         OpenAI to write this answer.\"",
    ] {
        assert!(outline.contains(said), "{said}: {outline}");
    }
    assert!(
        !outline.contains("Home EV chargers compared"),
        "a site not cited is not a source: {outline}"
    );
}

#[test]
fn with_no_answer_to_draw_one_sentence_stands_in_its_place() {
    for (answer, says) in [
        (
            Answer::Asking,
            "alo is writing an answer from these sites\u{2026}",
        ),
        (
            Answer::NotFound,
            "I couldn\u{2019}t find a reliable answer. The sites found for \
             \u{201c}how long does a home ev charger take to install\u{201d} are below.",
        ),
        (
            Answer::Unanswered(
                "alo could not write an answer: OpenAI refused alo's key (401).".to_owned(),
            ),
            "alo could not write an answer: OpenAI refused alo's key (401).",
        ),
    ] {
        let start = with(answer.clone(), Tab::All);
        let drawn = draw(start.clone());
        let outline = drawn.tree.to_outline();
        assert!(outline.contains(says), "{outline}");
        assert!(!outline.contains("Full answer"), "{outline}");
        assert!(!outline.contains("alo used this"), "{outline}");
        let mut renderer = Renderer::new(fonts());
        let _ = renderer.handle(ToRenderer::Start(Box::new(start)));
        // One sentence of 23, then the rule 20 under it: the sites 20
        // under that.
        let note = boxes_of(&renderer, "ansnote");
        assert_eq!(note.len(), 1, "{note:?}");
        assert!(note.iter().all(|note| note.starts_with("(80, 164) 880×")));
        assert_eq!(boxes_of(&renderer, "ans").len(), 1);
        // And the Answer tab says the same where the answer would be.
        let outline = draw(with(answer, Tab::Answer)).tree.to_outline();
        assert!(
            outline.contains(&says.replace("are below", "are on the Sites tab")),
            "{outline}"
        );
    }
    // The sentence in its place, drawn: the note at its height, the rule.
    let drawn = draw(with(Answer::NotFound, Tab::All));
    assert_eq!(
        holds("all-tab-with-no-reliable-answer.png", drawn.frame.as_ref()),
        Ok(())
    );
}

#[test]
fn a_press_on_full_answer_is_claimed_by_its_place() {
    let claimed = |claim| FromRenderer::ConversationClaimed(Some(claim));
    assert_eq!(
        pressed(answered(), 110.0, 235.0),
        claimed(Claim::FullAnswer { at: 0 })
    );
    // The Answer tab, open, is the second open tab.
    assert_eq!(
        pressed(answered(), 160.0, 123.0),
        claimed(Claim::Tab { at: 1 })
    );
    let nothing = FromRenderer::ConversationClaimed(None);
    assert_eq!(pressed(answered(), 200.0, 180.0), nothing, "a sentence");
    assert_eq!(pressed(answered(), 589.0, 180.0), nothing, "a number");
    assert_eq!(pressed(answered(), 200.0, 235.0), nothing, "how many sites");
}

#[test]
fn the_answer_is_drawn_the_same_by_a_confined_renderer() -> Result<(), String> {
    let mut renderers = renderers();
    let site = start::site();
    for screen in [answered(), answer_tab()] {
        let loaded = renderers.ask(&site, &ToRenderer::Start(Box::new(screen.clone())));
        assert!(
            matches!(&loaded, Ok(FromRenderer::Loaded { issues, .. }) if issues.is_empty()),
            "{loaded:?}"
        );
        let here = draw(screen);
        let painted = renderers.ask(&site, &ToRenderer::Paint);
        assert_eq!(
            painted.as_ref().ok(),
            here.frame.map(FromRenderer::Painted).as_ref(),
            "the confined renderer drew another answer"
        );
        let read = renderers.ask(&site, &ToRenderer::ReadTree);
        let Ok(FromRenderer::Tree(tree)) = read else {
            return Err(format!("not a tree: {read:?}"));
        };
        assert_eq!(tree.to_outline(), here.tree.to_outline());
    }
    renderers.stop_everything();
    Ok(())
}
