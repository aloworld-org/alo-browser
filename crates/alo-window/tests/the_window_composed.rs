/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The window's pixels, against committed references.
//!
//! ADR 0024 § 2: composition *"is tested like every other picture here: a
//! reference render, so a change that moves a pixel of the window's layout
//! says so."* Item 296 names three: a frozen alo page, a resize before its new
//! frame, and a gone tab's sentence. A fourth is the sentence for a tab that
//! never painted at all. Item 528 adds the strip across the top: a window
//! composed with a strip of one tab and of three, and a strip whose renderer
//! has gone, keeping its last frame and saying so. Item 529 adds a new tab
//! chosen over a page: the strip with the new tab selected, and nothing of
//! the page left under it.
//!
//! The frame in each window at scale one is a frozen alo page **as the
//! corpus committed it** — `alo-corpus/cases/alo-offline/render.png` — so
//! what is pinned here is the window's arrangement of a page, not the page;
//! the page's own reference is the corpus's to keep. A strip's frame is
//! drawn here by the engine, from `alo-renderer`'s own template and
//! stylesheet, with the corpus's fonts; the strip's own references are
//! `alo-renderer`'s.
//!
//! Item 562 has both renderers paint at the window's scale, in device pixels
//! (ADR 0057 § 3), and the window place their frames one to one. So a window
//! at two is composed from frames painted at two: the same frozen page
//! (`page.html`, which `alo-corpus`'s scale-two layout test puts where
//! Chrome 154 does at two) and the strip, each drawn here by the engine at
//! two. Each such window is also checked in numbers: every pixel below the
//! band is the page's frame's own.
//!
//! Item 599 has the window take a screen's scale of 1.5 as it is: a window
//! at 1.5 is composed from the strip and the frozen page each drawn at 1.5,
//! in a band of 54 device pixels, and checked the same way.
//!
//! Item 623 adds the line saying what an agent last did, in the sentence its
//! verb was answered with, over the frozen page: at one, and at 1.5 with a
//! gone tab's line below it.
//!
//! Item 624 adds the ring around the box an agent's verb acted on: the
//! frozen page's "Try again" button, ringed on its own rectangle as the
//! engine's tree gives it for the frame the engine painted — at one, with
//! the verb's sentence below, and at 1.5.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-window` rewrites them, as it
//! does the corpus's. A rewritten reference is a diff somebody reads.

use alo_agent::Outcome;
use alo_box::BoxId;
use alo_layout::Size;
use alo_paint::{Canvas, from_png, to_png};
use alo_renderer::host::Gone;
use alo_renderer::strip::{Condition, Strip, StripTab};
use alo_renderer::{Frame, FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_window::compose::{Scene, compose};
use alo_window::mark::{Marked, marked, ring};
use alo_window::message::News;
use alo_window::notice::Lettering;
use alo_window::place::{PixelRect, Placement, STRIP_HEIGHT, screen, strip_viewport};
use alo_window::scale::Scale;
use alo_window::showing::Showing;
use std::path::{Path, PathBuf};

/// The frozen page: alo's offline screen, as the corpus rendered it.
fn frozen_page() -> Result<Frame, String> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../alo-corpus/cases/alo-offline/render.png");
    let bytes = std::fs::read(&path)
        .map_err(|why| format!("{} could not be read: {why}", path.display()))?;
    let canvas = from_png(&bytes).map_err(|why| format!("not a picture: {why}"))?;
    Ok(Frame::from_canvas(&canvas))
}

/// The corpus's sans-serif, and what `system-ui` and `sans-serif` mean: the
/// strip's stylesheet asks for them in that order.
fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    for (weight, bytes) in [
        (Weight::NORMAL, dejavu::sans::regular()),
        (Weight::BOLD, dejavu::sans::bold()),
    ] {
        if let Some(font) = Font::load("DejaVu Sans", weight, Slant::Normal, bytes.to_vec()) {
            database.add(font);
        }
    }
    database.map_generic("system-ui", "DejaVu Sans");
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

/// The frozen page as the engine paints it at 480 × 400 on a screen of
/// `scale`, with the corpus's sans-serif: the frame its renderer sends a
/// window at that scale, in device pixels.
fn frozen_page_at(scale: f32) -> Result<Frame, String> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../alo-corpus/cases/alo-offline/page.html");
    let html = std::fs::read_to_string(&path)
        .map_err(|why| format!("{} could not be read: {why}", path.display()))?;
    let mut renderer = Renderer::new(fonts());
    let page = Page::new(html, Size::new(480.0, 400.0)).at_scale(scale);
    match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("the page was not loaded: {other:?}")),
    }
    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Ok(frame),
        other => Err(format!("the page was not painted: {other:?}")),
    }
}

/// Every device pixel of `canvas` below the band of a window at `scale`
/// that `frame` covers, compared with the frame's own: the first that
/// differs, if any does.
fn placed_one_to_one(canvas: &Canvas, frame: &Frame, scale: Scale) -> Option<(u32, u32)> {
    let top = scale.device(STRIP_HEIGHT);
    (0..frame.height)
        .flat_map(|y| (0..frame.width).map(move |x| (x, y)))
        .find(|(x, y)| {
            let shown = canvas.at(*x, top + y).map(alo_value::Rgba::to_rgba8);
            // Opaque, so blending over the background leaves it as it was.
            shown != frame.at(*x, *y)
        })
}

/// `tabs` as the engine draws them in a strip above a page `width` wide, on
/// a screen of `scale`.
fn strip_of(tabs: Vec<StripTab>, width: f32, scale: f32) -> Result<Frame, String> {
    let mut renderer = Renderer::new(fonts());
    let strip = Strip {
        tabs,
        viewport: strip_viewport(Size::new(width, 400.0)),
        scale,
    };
    match renderer.handle(ToRenderer::Strip(Box::new(strip))) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("the strip was not built: {other:?}")),
    }
    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Ok(frame),
        other => Err(format!("the strip was not painted: {other:?}")),
    }
}

/// The frozen page's own tab, selected: it is called "alo".
fn the_pages_tab() -> StripTab {
    StripTab::new("alo", "file:///alo-offline/page.html").selected()
}

fn lettering() -> Result<Lettering, String> {
    Lettering::compiled_in().ok_or_else(|| "DejaVu Sans is not compiled in".to_owned())
}

/// The sentence a tab whose renderer stopped answering is left with — made by
/// `alo-renderer`'s own types, so the reference shows the words a person is
/// actually shown rather than a sentence written for the test.
fn silent_sentence() -> String {
    Gone {
        site: "file:(1)".to_owned(),
        why: "it said nothing for 10s, so it was stopped".to_owned(),
    }
    .to_string()
}

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

/// Compare `canvas` with the committed reference, or write it when updating.
/// What differs, in words, if anything does.
fn holds(name: &str, canvas: &Canvas) -> Result<(), String> {
    let path = reference(name);
    let drawn = to_png(canvas).map_err(|why| format!("{name} could not be encoded: {why}"))?;
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
    if (canvas.width(), canvas.height()) != (expected.width(), expected.height()) {
        return Err(format!(
            "{name}: the window is {}×{}, and the reference {}×{}",
            canvas.width(),
            canvas.height(),
            expected.width(),
            expected.height()
        ));
    }
    let differing: Vec<(u32, u32)> = (0..canvas.height())
        .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
        .filter(|(x, y)| {
            canvas.at(*x, *y).map(alo_value::Rgba::to_rgba8)
                != expected.at(*x, *y).map(alo_value::Rgba::to_rgba8)
        })
        .collect();
    match differing.first() {
        None => Ok(()),
        Some(first) => Err(format!(
            "{name}: {} pixels moved, the first at {first:?}. If this is intended, \
             ALO_UPDATE_REFERENCES=1 rewrites the reference",
            differing.len(),
        )),
    }
}

#[test]
fn a_frozen_alo_page_in_a_window_at_scale_two() {
    let page = frozen_page_at(2.0).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        (page.width, page.height),
        (960, 800),
        "a page of 480 × 400 painted at two is 960 × 800 device pixels"
    );
    let canvas = compose(
        &Scene {
            window: (960, 800 + 2 * STRIP_HEIGHT),
            scale: Scale::TWO,
            strip: None,
            strip_sentence: None,
            frame: Some(&page),
            sentence: None,
            agent_said: None,
            marked: &[],
        },
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    assert_eq!(placed_one_to_one(&canvas, &page, Scale::TWO), None);
    assert_eq!(holds("a-frozen-page-at-scale-two.png", &canvas), Ok(()));
}

#[test]
fn a_window_grown_before_its_new_frame_shows_the_old_one_at_its_old_size() {
    let page = frozen_page().unwrap_or_else(|why| panic!("{why}"));
    let canvas = compose(
        &Scene {
            window: (560, 460 + STRIP_HEIGHT),
            scale: Scale::ONE,
            strip: None,
            strip_sentence: None,
            frame: Some(&page),
            sentence: None,
            agent_said: None,
            marked: &[],
        },
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    assert_eq!(holds("a-resize-before-its-frame.png", &canvas), Ok(()));
}

#[test]
fn a_gone_tab_keeps_its_frame_and_says_what_happened() {
    let page = frozen_page().unwrap_or_else(|why| panic!("{why}"));
    let sentence = silent_sentence();
    let canvas = compose(
        &Scene {
            window: (480, 400 + STRIP_HEIGHT),
            scale: Scale::ONE,
            strip: None,
            strip_sentence: None,
            frame: Some(&page),
            sentence: Some(&sentence),
            agent_said: None,
            marked: &[],
        },
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    assert_eq!(holds("a-gone-tab.png", &canvas), Ok(()));
}

#[test]
fn a_tab_gone_before_it_ever_painted_is_its_sentence_on_the_background() {
    let sentence = silent_sentence();
    let canvas = compose(
        &Scene {
            window: (480, 120 + 2 * STRIP_HEIGHT),
            scale: Scale::TWO,
            strip: None,
            strip_sentence: None,
            frame: None,
            sentence: Some(&sentence),
            agent_said: None,
            marked: &[],
        },
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    assert_eq!(holds("a-gone-tab-that-never-painted.png", &canvas), Ok(()));
}

#[test]
fn a_window_with_a_strip_of_one_tab_at_scale_two() {
    let page = frozen_page_at(2.0).unwrap_or_else(|why| panic!("{why}"));
    let window = (960, 800 + 2 * STRIP_HEIGHT);
    // The page is laid out at what the window leaves it below the strip, on
    // a screen of two.
    let told = screen(window, Scale::TWO);
    assert_eq!(told.size, Size::new(480.0, 400.0));
    assert_eq!(told.scale().to_bits(), 2.0_f32.to_bits());
    let strip =
        strip_of(vec![the_pages_tab()], 480.0, told.scale()).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((strip.width, strip.height), (960, 2 * STRIP_HEIGHT));
    let canvas = compose(
        &Scene {
            window,
            scale: Scale::TWO,
            strip: Some(&strip),
            strip_sentence: None,
            frame: Some(&page),
            sentence: None,
            agent_said: None,
            marked: &[],
        },
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    assert_eq!(placed_one_to_one(&canvas, &page, Scale::TWO), None);
    // And the band is the strip's frame, one to one from the corner.
    let in_band = (0..strip.height)
        .flat_map(|y| (0..strip.width).map(move |x| (x, y)))
        .find(|(x, y)| canvas.at(*x, *y).map(alo_value::Rgba::to_rgba8) != strip.at(*x, *y));
    assert_eq!(in_band, None);
    assert_eq!(holds("a-strip-of-one-tab.png", &canvas), Ok(()));
}

#[test]
fn a_window_with_a_strip_of_one_tab_at_one_and_a_half() {
    let half = Scale::ONE_AND_A_HALF;
    let page = frozen_page_at(half.factor()).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        (page.width, page.height),
        (720, 600),
        "a page of 480 × 400 painted at 1.5 is 720 × 600 device pixels"
    );
    let window = (720, 600 + 54);
    // The page is laid out at what the window leaves it below a band of
    // 36 CSS pixels at 1.5, which is 54 device pixels.
    let told = screen(window, half);
    assert_eq!(told.size, Size::new(480.0, 400.0));
    assert_eq!(told.scale().to_bits(), 1.5_f32.to_bits());
    let strip =
        strip_of(vec![the_pages_tab()], 480.0, told.scale()).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((strip.width, strip.height), (720, 54));
    let canvas = compose(
        &Scene {
            window,
            scale: half,
            strip: Some(&strip),
            strip_sentence: None,
            frame: Some(&page),
            sentence: None,
            agent_said: None,
            marked: &[],
        },
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    assert_eq!(placed_one_to_one(&canvas, &page, half), None);
    let in_band = (0..strip.height)
        .flat_map(|y| (0..strip.width).map(move |x| (x, y)))
        .find(|(x, y)| canvas.at(*x, *y).map(alo_value::Rgba::to_rgba8) != strip.at(*x, *y));
    assert_eq!(in_band, None);
    // Not the page at one enlarged: its device pixels are not each the CSS
    // pixel under them, as replicating would make them.
    let at_one = frozen_page_at(1.0).unwrap_or_else(|why| panic!("{why}"));
    let replicated = (0..page.height)
        .flat_map(|y| (0..page.width).map(move |x| (x, y)))
        .all(|(x, y)| page.at(x, y) == at_one.at(x * 2 / 3, y * 2 / 3));
    assert!(!replicated, "the page at 1.5 is the page at one, enlarged");
    assert_eq!(
        holds("a-strip-of-one-tab-at-one-and-a-half.png", &canvas),
        Ok(())
    );
}

#[test]
fn a_tab_gone_at_one_and_a_half_says_so_in_a_line_of_its_own_height_at_it() {
    let half = Scale::ONE_AND_A_HALF;
    let page = frozen_page_at(half.factor()).unwrap_or_else(|why| panic!("{why}"));
    let sentence = silent_sentence();
    let canvas = compose(
        &Scene {
            window: (720, 654),
            scale: half,
            strip: None,
            strip_sentence: None,
            frame: Some(&page),
            sentence: Some(&sentence),
            agent_said: None,
            marked: &[],
        },
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    // The line is 32 CSS pixels at 1.5 along the bottom, 48 device pixels,
    // and the page's frame is its own above it.
    let above = (0..606 - 54)
        .flat_map(|y| (0..720).map(move |x| (x, y)))
        .find(|(x, y)| canvas.at(*x, 54 + y).map(alo_value::Rgba::to_rgba8) != page.at(*x, *y));
    assert_eq!(above, None);
    assert_ne!(
        canvas.at(0, 606).map(alo_value::Rgba::to_rgba8),
        page.at(0, 606 - 54),
        "no line at the bottom"
    );
    assert_eq!(holds("a-gone-tab-at-one-and-a-half.png", &canvas), Ok(()));
}

#[test]
fn a_window_with_a_strip_of_three_tabs() {
    let page = frozen_page().unwrap_or_else(|why| panic!("{why}"));
    let strip = strip_of(
        vec![
            StripTab::new("Invoices", "https://books.example/invoices"),
            the_pages_tab(),
            StripTab::new("", "https://notes.example/today").in_condition(Condition::Loading),
        ],
        480.0,
        1.0,
    )
    .unwrap_or_else(|why| panic!("{why}"));
    let canvas = compose(
        &Scene {
            window: (480, 400 + STRIP_HEIGHT),
            scale: Scale::ONE,
            strip: Some(&strip),
            strip_sentence: None,
            frame: Some(&page),
            sentence: None,
            agent_said: None,
            marked: &[],
        },
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    assert_eq!(holds("a-strip-of-three-tabs.png", &canvas), Ok(()));
}

#[test]
fn a_strip_whose_renderer_has_gone_keeps_its_frame_and_says_so() {
    let page = frozen_page().unwrap_or_else(|why| panic!("{why}"));
    let strip = strip_of(vec![the_pages_tab()], 480.0, 1.0).unwrap_or_else(|why| panic!("{why}"));
    let sentence = Gone {
        site: "alo:(1)".to_owned(),
        why: "it said nothing for 10s, so it was stopped".to_owned(),
    }
    .to_string();
    let canvas = compose(
        &Scene {
            window: (480, 400 + STRIP_HEIGHT),
            scale: Scale::ONE,
            strip: Some(&strip),
            strip_sentence: Some(&sentence),
            frame: Some(&page),
            sentence: None,
            agent_said: None,
            marked: &[],
        },
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    assert_eq!(holds("a-gone-strip.png", &canvas), Ok(()));
}

#[test]
fn a_new_tab_chosen_over_a_page_shows_none_of_it() {
    let page = frozen_page().unwrap_or_else(|why| panic!("{why}"));
    let before = strip_of(vec![the_pages_tab()], 480.0, 1.0).unwrap_or_else(|why| panic!("{why}"));
    let after = strip_of(
        vec![
            StripTab::new("alo", "file:///alo-offline/page.html"),
            StripTab::new("", "about:blank").selected(),
        ],
        480.0,
        1.0,
    )
    .unwrap_or_else(|why| panic!("{why}"));
    // What the window hears, in the order the conductor says it after ⌘T.
    let mut showing = Showing::default();
    for news in [
        News::Painted(page),
        News::Strip(before),
        News::Chosen(None),
        News::Strip(after),
    ] {
        assert!(showing.hear(news));
    }
    let window = (480, 400 + STRIP_HEIGHT);
    let canvas = compose(
        &showing.scene(window, Scale::ONE),
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    // Below the strip, every pixel is the window's background.
    let ground = Some(alo_window::colours::background().to_rgba8());
    for y in STRIP_HEIGHT..window.1 {
        for x in 0..window.0 {
            assert_eq!(
                canvas.at(x, y).map(alo_value::Rgba::to_rgba8),
                ground,
                "({x}, {y}) is not the background"
            );
        }
    }
    assert_eq!(holds("a-new-tab.png", &canvas), Ok(()));
}

/// The sentence an agent is answered with for putting a name into the
/// field at box 10 — made by `alo-agent`'s own type, so the reference shows
/// the words a person is shown rather than a sentence written for the test.
fn agents_sentence() -> String {
    Outcome::TextPut {
        node: BoxId::from_index_for_tests(10),
        text: "Ada Lovelace".to_owned(),
    }
    .to_string()
}

#[test]
fn what_an_agent_did_is_a_line_of_the_browsers_over_the_page() {
    let page = frozen_page().unwrap_or_else(|why| panic!("{why}"));
    let sentence = agents_sentence();
    assert_eq!(sentence, "put \"Ada Lovelace\" into box#10");
    let mut showing = Showing::default();
    for news in [News::Painted(page.clone()), News::AgentSaid(sentence)] {
        assert!(showing.hear(news));
    }
    let window = (480, 400 + STRIP_HEIGHT);
    let scene = showing.scene(window, Scale::ONE);
    let placed = Placement::of(window, Scale::ONE, scene.contents());
    let line = PixelRect {
        x: 0,
        y: 404,
        width: 480,
        height: 32,
    };
    assert_eq!(placed.agent_line, Some(line));
    let canvas = compose(&scene, &lettering().unwrap_or_else(|why| panic!("{why}")));
    // Above the line every pixel is the page's own; the line is drawn over
    // it and nowhere else.
    for y in STRIP_HEIGHT..line.y {
        for x in 0..window.0 {
            assert_eq!(
                canvas.at(x, y).map(alo_value::Rgba::to_rgba8),
                page.at(x, y - STRIP_HEIGHT),
                "at {x},{y}"
            );
        }
    }
    assert_eq!(holds("an-agents-sentence.png", &canvas), Ok(()));
}

#[test]
fn what_an_agent_did_at_one_and_a_half_is_above_a_gone_tabs_line() {
    let page = frozen_page_at(1.5).unwrap_or_else(|why| panic!("{why}"));
    let gone = silent_sentence();
    let sentence = agents_sentence();
    let window = (720, 600 + 54);
    let scene = Scene {
        window,
        scale: Scale::ONE_AND_A_HALF,
        strip: None,
        strip_sentence: None,
        frame: Some(&page),
        sentence: Some(&gone),
        agent_said: Some(&sentence),
        marked: &[],
    };
    let placed = Placement::of(window, Scale::ONE_AND_A_HALF, scene.contents());
    // Each line 32 CSS pixels at 1.5, 48 device pixels: the tab's along the
    // bottom, the agent's above it.
    assert_eq!(
        placed.notice.map(|rect| (rect.y, rect.height)),
        Some((606, 48))
    );
    assert_eq!(
        placed.agent_line.map(|rect| (rect.y, rect.height)),
        Some((558, 48))
    );
    let canvas = compose(&scene, &lettering().unwrap_or_else(|why| panic!("{why}")));
    assert_eq!(
        holds("an-agents-sentence-at-one-and-a-half.png", &canvas),
        Ok(())
    );
}

/// The frozen page painted at `scale` by the engine, and the rectangles its
/// tree gives the "Try again" button in that same layout — what the
/// conductor reads after a verb on it (queue item 624).
fn page_and_its_button(scale: f32) -> Result<(Frame, Vec<alo_layout::Rect>), String> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../alo-corpus/cases/alo-offline/page.html");
    let html = std::fs::read_to_string(&path)
        .map_err(|why| format!("{} could not be read: {why}", path.display()))?;
    let mut renderer = Renderer::new(fonts());
    let page = Page::new(html, Size::new(480.0, 400.0)).at_scale(scale);
    match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("the page was not loaded: {other:?}")),
    }
    let frame = match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => frame,
        other => return Err(format!("the page was not painted: {other:?}")),
    };
    let FromRenderer::Tree(tree) = renderer.handle(ToRenderer::ReadTree) else {
        return Err("the tree was not read".to_owned());
    };
    let button = tree
        .nodes()
        .into_iter()
        .find(|node| node.name.as_deref() == Some("Try again"))
        .ok_or("no button called Try again")?;
    Ok((frame, button.rects.clone()))
}

/// The sentence an agent is answered with for activating the button.
fn activated_sentence() -> String {
    Outcome::Activated {
        node: BoxId::from_index_for_tests(12),
        name: Some("Try again".to_owned()),
    }
    .to_string()
}

#[test]
fn the_box_an_agent_acted_on_is_ringed_on_its_own_rect() {
    let (page, rects) = page_and_its_button(1.0).unwrap_or_else(|why| panic!("{why}"));
    // The corpus's own tree says (181.54688, 286) 116.90625×39: the ring's
    // edges are those, rounded to whole device pixels.
    let ringed = marked(&rects, 1.0);
    assert_eq!(
        ringed,
        vec![Marked {
            x: 182,
            y: 286,
            width: 116,
            height: 39
        }]
    );
    let mut showing = Showing::default();
    for news in [
        News::Painted(page.clone()),
        News::Marked(ringed),
        News::AgentSaid(activated_sentence()),
    ] {
        assert!(showing.hear(news));
    }
    let window = (480, 400 + STRIP_HEIGHT);
    let scene = showing.scene(window, Scale::ONE);
    let placed = Placement::of(window, Scale::ONE, scene.contents());
    let pieces = showing
        .marked()
        .iter()
        .flat_map(|each| ring(*each, placed.frame, Scale::ONE))
        .collect::<Vec<_>>();
    assert_eq!(
        pieces,
        vec![
            PixelRect {
                x: 178,
                y: 318,
                width: 124,
                height: 2
            },
            PixelRect {
                x: 178,
                y: 363,
                width: 124,
                height: 2
            },
            PixelRect {
                x: 178,
                y: 320,
                width: 2,
                height: 43
            },
            PixelRect {
                x: 300,
                y: 320,
                width: 2,
                height: 43
            },
        ]
    );
    let canvas = compose(&scene, &lettering().unwrap_or_else(|why| panic!("{why}")));
    // Everywhere above the agent's line and off the ring is the page's own.
    let on_ring = |x: u32, y: u32| {
        pieces
            .iter()
            .any(|p| (p.x..p.x + p.width).contains(&x) && (p.y..p.y + p.height).contains(&y))
    };
    let accent = Some(alo_window::colours::mark().to_rgba8());
    for y in STRIP_HEIGHT..404 {
        for x in 0..window.0 {
            let shown = canvas.at(x, y).map(alo_value::Rgba::to_rgba8);
            if on_ring(x, y) {
                assert_eq!(shown, accent, "the ring at {x},{y}");
            } else {
                assert_eq!(shown, page.at(x, y - STRIP_HEIGHT), "the page at {x},{y}");
            }
        }
    }
    assert_eq!(holds("the-acted-on-box.png", &canvas), Ok(()));
}

#[test]
fn the_box_an_agent_acted_on_at_one_and_a_half_is_ringed_three_device_pixels_out_and_thick() {
    let (page, rects) = page_and_its_button(1.5).unwrap_or_else(|why| panic!("{why}"));
    let ringed = marked(&rects, 1.5);
    let [button] = ringed.as_slice() else {
        panic!("not one rectangle: {ringed:?}");
    };
    // At 1.5 the button is laid out at (181.54167, 286.66666) 116.90625 ×
    // 38.666668, where Chrome puts it (`chrome-at-1.5.txt`): 272.3125 and
    // 447.67188 across round to 272 and 448, and 430 and 488 down are whole.
    assert_eq!(
        *button,
        Marked {
            x: 272,
            y: 430,
            width: 176,
            height: 58
        }
    );
    let window = (720, 600 + 54);
    let scene = Scene {
        window,
        scale: Scale::ONE_AND_A_HALF,
        strip: None,
        strip_sentence: None,
        frame: Some(&page),
        sentence: None,
        agent_said: None,
        marked: &ringed,
    };
    let placed = Placement::of(window, Scale::ONE_AND_A_HALF, scene.contents());
    let pieces = ring(*button, placed.frame, Scale::ONE_AND_A_HALF);
    assert_eq!(
        pieces.first(),
        Some(&PixelRect {
            x: 266,
            y: 478,
            width: 188,
            height: 3
        })
    );
    assert_eq!(
        pieces.get(2),
        Some(&PixelRect {
            x: 266,
            y: 481,
            width: 3,
            height: 64
        })
    );
    let canvas = compose(&scene, &lettering().unwrap_or_else(|why| panic!("{why}")));
    assert_eq!(
        holds("the-acted-on-box-at-one-and-a-half.png", &canvas),
        Ok(())
    );
}
