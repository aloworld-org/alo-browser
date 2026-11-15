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
//! Item 625 adds a refusal's line, in colours of its own: two refusals below
//! what the agent did, at one, and at 1.5 a refusal outlasting the verb that
//! ran after it, above a gone tab's line.
//!
//! Item 642 adds the frame of ours around Chrome's window, at two, and item
//! 679 draws in it: which renderer draws the page, alone at two; with what
//! an agent did beside it, at one; with two refusals after it, at one; and
//! at 1.5 a refusal outlasting the verb that ran after it. Each is also
//! checked in numbers, against where `frame_place` puts the lines.
//!
//! Item 991 draws that frame as alo OS's window band, light only: white, a
//! one-pixel line `#CBD5E1` along its foot, the page's title and the
//! application's name as the frame's own strip's renderer paints them —
//! drawn here by the engine from `alo-renderer`'s template and stylesheet,
//! in the interface's Manrope, for the band `frame_strip::band_of` makes of
//! what Chromium calls the page, with a title holding markup that is drawn
//! as the characters it is — and which renderer draws the page in plain
//! words at its right edge: in front at one and at two, a long title cut
//! beside an agent's line, and at 1.5 with an agent's line and a refusal.
//! Each is checked in numbers too: the band's frame one to one at the left,
//! the renderer's words against the right edge, the lines between, and the
//! foot's line under everything. The four frames of 679 were drawn again
//! in the band's look; 681's two of the page's tab are gone with the tab.
//! A window not in front is not drawn: the frame does not yet know whether
//! Chromium's window is in front.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-window` rewrites them, as it
//! does the corpus's. A rewritten reference is a diff somebody reads.

use alo_agent::{Outcome, Refusal, Target};
use alo_box::BoxId;
use alo_layout::Size;
use alo_paint::{Canvas, from_png, to_png};
use alo_renderer::host::Gone;
use alo_renderer::strip::{Condition, Strip, StripTab};
use alo_renderer::{Frame, FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_window::agent_lines::{AgentLine, AgentLines};
use alo_window::chrome_protocol::Named;
use alo_window::compose::{Scene, compose};
use alo_window::frame_compose::{FrameScene, compose_frame};
use alo_window::frame_place::FramePlacement;
use alo_window::frame_strip::band_of;
use alo_window::mark::{Marked, marked, ring};
use alo_window::message::News;
use alo_window::notice::Lettering;
use alo_window::pages::RenderedBy;
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
            agent_lines: &[],
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
            agent_lines: &[],
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
            agent_lines: &[],
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
            agent_lines: &[],
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
            agent_lines: &[],
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
            agent_lines: &[],
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
            agent_lines: &[],
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
            agent_lines: &[],
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
            agent_lines: &[],
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
    assert_eq!(placed.agent_lines, [Some(line), None, None]);
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
    // Queue item 984: alo's mark, 16 square, 12 in and in the middle of the
    // line, its navy arcs drawn; the 8 after it the line's own ground, the
    // words past them.
    let mark = alo_window::acting_mark::place(line, Scale::ONE);
    assert_eq!(
        mark,
        Some(PixelRect {
            x: 12,
            y: 412,
            width: 16,
            height: 16
        })
    );
    let pixel = |x: u32, y: u32| canvas.at(x, y).map(alo_value::Rgba::to_rgba8);
    let arcs = (412..428)
        .flat_map(|y| (12..28).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(x, y) == Some((0x1b, 0x3e, 0x5e, 0xff)))
        .count();
    assert!(arcs > 0, "the mark's arcs are drawn");
    let ground = Some(alo_window::colours::agent_ground().to_rgba8());
    for y in 405..line.y + line.height {
        for x in (0..12).chain(28..36) {
            assert_eq!(pixel(x, y), ground, "only the ground at {x},{y}");
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
        agent_lines: &[AgentLine {
            sentence,
            refused: false,
        }],
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
        placed.agent_lines[0].map(|rect| (rect.y, rect.height)),
        Some((558, 48))
    );
    let canvas = compose(&scene, &lettering().unwrap_or_else(|why| panic!("{why}")));
    assert_eq!(
        holds("an-agents-sentence-at-one-and-a-half.png", &canvas),
        Ok(())
    );
}

/// The sentences an agent is answered with when nothing on the page is
/// called "Nme", and when it asks to type into the "Try again" button —
/// made by `alo-agent`'s own type, as [`agents_sentence`] is.
fn refusals() -> (String, String) {
    let not_found = Refusal::NotFound {
        target: Target::Named("Nme".to_owned()),
    };
    let not_a_field = Refusal::NotAField {
        node: BoxId::from_index_for_tests(12),
        role: alo_box::Role::Known(alo_box::KnownRole::Button),
    };
    (not_found.to_string(), not_a_field.to_string())
}

#[test]
fn two_refusals_are_two_lines_of_their_own_below_what_the_agent_did() {
    let page = frozen_page().unwrap_or_else(|why| panic!("{why}"));
    let (not_found, not_a_field) = refusals();
    assert_eq!(not_found, "nothing on the page is \"Nme\"");
    assert_eq!(
        not_a_field,
        "box#12 is a button, which is not a thing text goes into"
    );
    let mut showing = Showing::default();
    for news in [
        News::Painted(page.clone()),
        News::AgentSaid(agents_sentence()),
        News::AgentRefused(not_found),
        News::AgentRefused(not_a_field),
    ] {
        assert!(showing.hear(news));
    }
    let window = (480, 400 + STRIP_HEIGHT);
    let scene = showing.scene(window, Scale::ONE);
    let placed = Placement::of(window, Scale::ONE, scene.contents());
    let line = |y| PixelRect {
        x: 0,
        y,
        width: 480,
        height: 32,
    };
    // Oldest at the top: what the agent did, then each refusal below it.
    assert_eq!(
        placed.agent_lines,
        [Some(line(340)), Some(line(372)), Some(line(404))]
    );
    let canvas = compose(&scene, &lettering().unwrap_or_else(|why| panic!("{why}")));
    let at = |x, y| canvas.at(x, y).map(alo_value::Rgba::to_rgba8);
    for y in STRIP_HEIGHT..340 {
        for x in 0..window.0 {
            assert_eq!(at(x, y), page.at(x, y - STRIP_HEIGHT), "at {x},{y}");
        }
    }
    // Each line in its own colours: the agent's ground, then the
    // refusals', each under a rule of its own.
    let (did, refused) = (
        alo_window::colours::agent_line(),
        alo_window::colours::refused_line(),
    );
    for (top, colours) in [(340, did), (372, refused), (404, refused)] {
        assert_eq!(at(0, top), Some(colours.rule.to_rgba8()), "rule at {top}");
        assert_eq!(
            at(479, top + 31),
            Some(colours.ground.to_rgba8()),
            "ground at {top}"
        );
        let inked = (0..480)
            .flat_map(|x| (top + 1..top + 32).map(move |y| (x, y)))
            .filter(|(x, y)| at(*x, *y) == Some(colours.ink.to_rgba8()))
            .count();
        assert!(inked > 0, "no words on the line at {top}");
    }
    assert_eq!(holds("two-refusals.png", &canvas), Ok(()));
}

#[test]
fn a_refusal_at_one_and_a_half_outlasts_the_verb_that_ran_after_it() {
    let page = frozen_page_at(1.5).unwrap_or_else(|why| panic!("{why}"));
    let gone = silent_sentence();
    let (not_found, _) = refusals();
    let mut showing = Showing::default();
    for news in [
        News::Painted(page),
        News::AgentRefused(not_found.clone()),
        News::AgentSaid(agents_sentence()),
        News::Said(gone),
    ] {
        assert!(showing.hear(news));
    }
    let told: Vec<(&str, bool)> = showing
        .agent_lines()
        .iter()
        .map(|line| (line.sentence.as_str(), line.refused))
        .collect();
    assert_eq!(
        told,
        [
            (not_found.as_str(), true),
            ("put \"Ada Lovelace\" into box#10", false)
        ]
    );
    let window = (720, 600 + 54);
    let scene = showing.scene(window, Scale::ONE_AND_A_HALF);
    let placed = Placement::of(window, Scale::ONE_AND_A_HALF, scene.contents());
    // 48 device pixels each: the tab's along the bottom, then what the
    // agent did, then the refusal before it.
    assert_eq!(
        placed.notice.map(|rect| (rect.y, rect.height)),
        Some((606, 48))
    );
    let agents = placed
        .agent_lines
        .map(|line| line.map(|rect| (rect.y, rect.height)));
    assert_eq!(agents, [Some((510, 48)), Some((558, 48)), None]);
    let canvas = compose(&scene, &lettering().unwrap_or_else(|why| panic!("{why}")));
    assert_eq!(holds("a-refusal-at-one-and-a-half.png", &canvas), Ok(()));
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
        agent_lines: &[],
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

/// What the frame says draws the page: the Chromium alo carries, by the
/// version it gives itself — made by the browser's own type, in the plain
/// words the frame draws.
fn chromiums_words() -> String {
    let carried = RenderedBy::Carried {
        product: "Chrome/156.0.8078.48".to_owned(),
    };
    // Agents, the socket and the terminal are told the exact words, which
    // the frame no longer draws.
    assert_eq!(
        carried.to_string(),
        "Chrome/156.0.8078.48, the Chromium alo carries, driven by this browser"
    );
    carried.plainly()
}

/// The band's white, and the line along its foot.
const WHITE: Option<(u8, u8, u8, u8)> = Some((0xff, 0xff, 0xff, 0xff));
const FOOT: Option<(u8, u8, u8, u8)> = Some((0xcb, 0xd5, 0xe1, 0xff));

/// A frame of `window` at `scale` showing `band`, if it was painted,
/// saying which renderer draws the page, with `lines`: composed, and where
/// its pieces were placed.
fn frame_of(
    window: (u32, u32),
    scale: Scale,
    band: Option<&Frame>,
    lines: &[AgentLine],
) -> Result<(Canvas, FramePlacement), String> {
    let lettering = lettering()?;
    let renderer = chromiums_words();
    let scene = FrameScene {
        window,
        scale,
        band,
        band_sentence: None,
        renderer: &renderer,
        agent_lines: lines,
    };
    Ok((compose_frame(&scene, &lettering), scene.placed(&lettering)))
}

/// That each of `lines` is drawn in `canvas` where `placed` put it: its
/// ground at its right end and its words in its ink, the band's white
/// between — or the first that is not, in words.
fn drawn_where_placed(
    canvas: &Canvas,
    placed: &FramePlacement,
    lines: &[AgentLine],
) -> Result<(), String> {
    let at = |x, y| canvas.at(x, y).map(alo_value::Rgba::to_rgba8);
    for (line, rect) in lines.iter().zip(placed.agent_lines) {
        let rect = rect.ok_or_else(|| format!("{:?} was not placed", line.sentence))?;
        let colours = if line.refused {
            alo_window::colours::refused_line()
        } else {
            alo_window::colours::agent_line()
        };
        let right = rect.x + rect.width - 1;
        let bottom = rect.y + rect.height - 1;
        if at(right, bottom) != Some(colours.ground.to_rgba8()) {
            return Err(format!("not the line's ground at the end of {rect:?}"));
        }
        let gap = at(rect.x - 1, bottom);
        if gap.is_some() && gap != WHITE {
            return Err(format!("not the band's white in the gap before {rect:?}"));
        }
        if at(right, bottom + 1) != FOOT {
            return Err(format!("not the foot's line under {rect:?}"));
        }
        let inked = (rect.x..=right)
            .flat_map(|x| (rect.y..=bottom).map(move |y| (x, y)))
            .any(|(x, y)| at(x, y) == Some(colours.ink.to_rgba8()));
        if !inked {
            return Err(format!("no words in {rect:?}"));
        }
    }
    Ok(())
}

/// That the renderer's words are against the frame's right edge, in alo
/// OS's muted grey, and the foot's line runs under the whole frame — or
/// the first that is not, in words.
fn the_renderer_and_the_foot(canvas: &Canvas, placed: &FramePlacement) -> Result<(), String> {
    let at = |x, y| canvas.at(x, y).map(alo_value::Rgba::to_rgba8);
    let rect = placed.renderer;
    if rect.x + rect.width != canvas.width() {
        return Err(format!(
            "the renderer's words end short of the edge: {rect:?}"
        ));
    }
    let muted = Some((0x59, 0x6b, 0x78, 0xff));
    let inked = (rect.x..rect.x + rect.width)
        .flat_map(|x| (rect.y..rect.y + rect.height).map(move |y| (x, y)))
        .any(|(x, y)| at(x, y) == muted);
    if !inked {
        return Err(format!("no muted words in {rect:?}"));
    }
    let foot = placed.line;
    let all = (0..canvas.width())
        .flat_map(|x| (foot.y..foot.y + foot.height).map(move |y| (x, y)))
        .all(|(x, y)| at(x, y) == FOOT);
    if !all || foot.y + foot.height != canvas.height() || foot.width != canvas.width() {
        return Err(format!(
            "the foot's line is not edge to edge at the bottom: {foot:?}"
        ));
    }
    Ok(())
}

#[test]
fn the_frame_around_chromiums_window_at_scale_two() {
    // A frame of 500 by the band's 54 points, at two, before its band has
    // painted: white, which renderer draws the page at its right, and the
    // foot's line.
    let renderer = chromiums_words();
    assert_eq!(renderer, "Page drawn by Chromium 156, carried inside alo");
    let (canvas, placed) =
        frame_of((1000, 108), Scale::TWO, None, &[]).unwrap_or_else(|why| panic!("{why}"));
    // Its whole sentence, with room to spare, above the foot's two device
    // pixels.
    assert!((500..900).contains(&placed.renderer.width), "{placed:?}");
    assert_eq!(
        placed.renderer,
        PixelRect {
            x: 1000 - placed.renderer.width,
            y: 0,
            width: placed.renderer.width,
            height: 106
        }
    );
    assert_eq!(
        placed.line,
        PixelRect {
            x: 0,
            y: 106,
            width: 1000,
            height: 2
        }
    );
    assert_eq!(placed.agent_lines, [None, None, None]);
    assert_eq!(the_renderer_and_the_foot(&canvas, &placed), Ok(()));
    assert_eq!(holds("the-frame-around-chrome.png", &canvas), Ok(()));
}

#[test]
fn the_frame_says_what_an_agent_did_beside_which_renderer() {
    let mut lines = AgentLines::default();
    lines.did(agents_sentence());
    let (canvas, placed) =
        frame_of((1000, 54), Scale::ONE, None, lines.lines()).unwrap_or_else(|why| panic!("{why}"));
    // The renderer's words as wide as they need at the right, the title's
    // room half of what is left of them, the agent's line the other half
    // less a pixel of white either side.
    let needs = placed.renderer.width;
    assert!((250..400).contains(&needs), "{placed:?}");
    let left = 1000 - needs;
    assert_eq!(placed.band.width, left / 2);
    assert_eq!(
        placed.agent_lines,
        [
            Some(PixelRect {
                x: left / 2 + 1,
                y: 0,
                width: left - 1 - (left / 2 + 1),
                height: 53
            }),
            None,
            None
        ]
    );
    assert_eq!(drawn_where_placed(&canvas, &placed, lines.lines()), Ok(()));
    assert_eq!(the_renderer_and_the_foot(&canvas, &placed), Ok(()));
    assert_eq!(
        holds("the-frame-with-an-agents-sentence.png", &canvas),
        Ok(())
    );
}

#[test]
fn the_frame_shows_two_refusals_after_what_the_agent_did() {
    let (not_found, not_a_field) = refusals();
    let mut lines = AgentLines::default();
    lines.did(agents_sentence());
    lines.refused(not_found);
    lines.refused(not_a_field);
    assert_eq!(lines.lines().len(), 3);
    let (canvas, placed) =
        frame_of((1200, 54), Scale::ONE, None, lines.lines()).unwrap_or_else(|why| panic!("{why}"));
    // Three equal shares of what the title's half leaves, a pixel apart,
    // the last to a pixel short of the renderer's words.
    let left = placed.renderer.x;
    let start = left / 2 + 1;
    let end = left - 1;
    let each = (end - start - 2) / 3;
    let xs: Vec<_> = placed
        .agent_lines
        .iter()
        .flatten()
        .map(|rect| (rect.x, rect.width))
        .collect();
    assert_eq!(
        xs,
        [
            (start, each),
            (start + each + 1, each),
            (start + 2 * each + 2, end - (start + 2 * each + 2)),
        ]
    );
    assert_eq!(drawn_where_placed(&canvas, &placed, lines.lines()), Ok(()));
    assert_eq!(holds("the-frame-with-two-refusals.png", &canvas), Ok(()));
}

#[test]
fn the_frame_at_one_and_a_half_keeps_a_refusal_past_the_verb_after_it() {
    let (not_found, _) = refusals();
    let mut lines = AgentLines::default();
    lines.did(activated_sentence());
    lines.refused(not_found.clone());
    lines.did(agents_sentence());
    assert_eq!(
        lines.lines(),
        [
            AgentLine {
                sentence: not_found,
                refused: true
            },
            AgentLine {
                sentence: agents_sentence(),
                refused: false
            }
        ]
    );
    // 800 points at 1.5, the band 81 device pixels tall, its foot two.
    let (canvas, placed) = frame_of((1200, 81), Scale::ONE_AND_A_HALF, None, lines.lines())
        .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(placed.renderer.height, 79);
    assert_eq!((placed.line.y, placed.line.height), (79, 2));
    // Two device pixels of white between pieces at 1.5.
    let first = placed.agent_lines[0].unwrap_or_else(|| panic!("{placed:?}"));
    let second = placed.agent_lines[1].unwrap_or_else(|| panic!("{placed:?}"));
    assert_eq!(first.x, placed.band.width + 2);
    assert_eq!(second.x, first.x + first.width + 2);
    assert_eq!(second.x + second.width + 2, placed.renderer.x);
    assert_eq!(drawn_where_placed(&canvas, &placed, lines.lines()), Ok(()));
    assert_eq!(the_renderer_and_the_foot(&canvas, &placed), Ok(()));
    assert_eq!(holds("the-frame-at-one-and-a-half.png", &canvas), Ok(()));
}

/// What Chromium calls the page in the frame's references: a title holding
/// markup, which the band shows as the characters it is.
fn invoices() -> Named {
    Named {
        title: "Invoices <b>2026</b>".to_owned(),
        address: "https://example.com/invoices".to_owned(),
    }
}

/// A title far longer than any frame has room for.
fn a_long_title() -> Named {
    Named {
        title: "Quarterly invoices for every client this organisation has ever had, \
                sorted by the date each was last paid, with the reminders sent"
            .to_owned(),
        address: "https://example.com/invoices".to_owned(),
    }
}

/// The interface's typefaces, as the frame's strip's renderer is handed
/// them, then the corpus's sans-serif for the generics.
fn band_fonts() -> FontDatabase {
    let mut database = fonts();
    for font in alo_renderer::typefaces::fonts() {
        database.add(font);
    }
    database
}

/// The band as the frame's strip's renderer paints it for `named`, `width`
/// CSS pixels wide on a screen of `scale` — the engine, from
/// `alo-renderer`'s own template and stylesheet, in the interface's faces.
fn the_frames_band(named: &Named, width: u32, scale: Scale) -> Result<Frame, String> {
    let mut renderer = Renderer::new(band_fonts());
    let band = band_of(named, width, scale.factor());
    match renderer.handle(ToRenderer::Band(Box::new(band))) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("the band was not built: {other:?}")),
    }
    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Ok(frame),
        other => Err(format!("the band was not painted: {other:?}")),
    }
}

/// A frame of `window` at `scale` for a page called `named`, with `lines`:
/// its band painted as wide as the frame leaves it, then composed — and the
/// band, and where the pieces were placed.
fn frame_with_band(
    window: (u32, u32),
    scale: Scale,
    named: &Named,
    lines: &[AgentLine],
) -> Result<(Canvas, FramePlacement, Frame), String> {
    let (_, before) = frame_of(window, scale, None, lines)?;
    let band = the_frames_band(named, before.band_width, scale)?;
    let (canvas, placed) = frame_of(window, scale, Some(&band), lines)?;
    if placed != before {
        return Err(format!("placed apart: {before:?} and {placed:?}"));
    }
    Ok((canvas, placed, band))
}

/// Whether any pixel of column `x` above `foot` is not the band's white.
fn inked_column(canvas: &Canvas, x: u32, foot: u32) -> bool {
    (0..foot).any(|y| canvas.at(x, y).map(alo_value::Rgba::to_rgba8) != WHITE)
}

/// The first device pixel of `band` above the foot's line that `canvas`
/// does not show one to one at the frame's left, if any.
fn band_one_to_one(canvas: &Canvas, band: &Frame, foot: u32) -> Option<(u32, u32)> {
    (0..band.height.min(foot))
        .flat_map(|y| (0..band.width).map(move |x| (x, y)))
        .find(|(x, y)| {
            // A band is opaque, so blending over the white leaves it as it
            // was painted.
            canvas.at(*x, *y).map(alo_value::Rgba::to_rgba8) != band.at(*x, *y)
        })
}

#[test]
fn the_frames_band_in_front_at_one() {
    // 1200 by 54 points at one.
    let (canvas, placed, band) = frame_with_band((1200, 54), Scale::ONE, &invoices(), &[])
        .unwrap_or_else(|why| panic!("{why}"));
    let needs = placed.renderer.width;
    assert!((250..400).contains(&needs), "{placed:?}");
    // The band all of what the renderer's words leave, laid out that many
    // CSS pixels wide and painted so, 54 tall.
    assert_eq!(
        placed.band,
        PixelRect {
            x: 0,
            y: 0,
            width: 1200 - needs,
            height: 54
        }
    );
    assert_eq!(placed.band_width, 1200 - needs);
    assert_eq!((band.width, band.height), (1200 - needs, 54));
    assert_eq!(band_one_to_one(&canvas, &band, 53), None);
    assert_eq!(the_renderer_and_the_foot(&canvas, &placed), Ok(()));
    // The title's letters begin at 22, where its box does
    // (`alo-renderer`'s `the_frames_band.rs` asserts the boxes), the
    // letter's own side bearing after it.
    let first_inked = (0..placed.band.width).find(|&x| inked_column(&canvas, x, 53));
    assert!(matches!(first_inked, Some(22..=24)), "{first_inked:?}");
    assert_eq!(holds("the-frames-band.png", &canvas), Ok(()));
}

#[test]
fn the_frames_band_in_front_at_two() {
    let (canvas, placed, band) = frame_with_band((2400, 108), Scale::TWO, &invoices(), &[])
        .unwrap_or_else(|why| panic!("{why}"));
    let needs = placed.renderer.width;
    // At two every piece is twice what it is at one, the band laid out at
    // the same CSS width.
    let (_, at_one) =
        frame_of((1200, 54), Scale::ONE, None, &[]).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(placed.band_width, at_one.band_width);
    assert_eq!(placed.band.width, 2 * at_one.band_width);
    assert_eq!(placed.band.height, 108);
    assert_eq!((band.width, band.height), (2400 - needs, 108));
    assert_eq!(
        (placed.line.y, placed.line.height, placed.renderer.height),
        (106, 2, 106)
    );
    assert_eq!(band_one_to_one(&canvas, &band, 106), None);
    assert_eq!(the_renderer_and_the_foot(&canvas, &placed), Ok(()));
    assert_eq!(holds("the-frames-band-at-two.png", &canvas), Ok(()));
}

#[test]
fn a_long_title_is_cut_beside_an_agents_line_and_the_name_kept() {
    let mut lines = AgentLines::default();
    lines.did(agents_sentence());
    // 1000 by 54 at one: the title has half of what the renderer's words
    // leave, and is cut there; the name after it whole.
    let (canvas, placed, band) =
        frame_with_band((1000, 54), Scale::ONE, &a_long_title(), lines.lines())
            .unwrap_or_else(|why| panic!("{why}"));
    let left = placed.renderer.x;
    assert_eq!(placed.band.width, left / 2);
    assert_eq!(band.width, left / 2);
    assert_eq!(band_one_to_one(&canvas, &band, 53), None);
    // The name's letters reach the band's right edge: the title gave way
    // first, and the name kept all its room.
    let last_inked = (0..band.width)
        .rev()
        .find(|&x| inked_column(&canvas, x, 53));
    assert!(
        last_inked.is_some_and(|x| x + 3 >= band.width),
        "{last_inked:?} of {}",
        band.width
    );
    assert_eq!(drawn_where_placed(&canvas, &placed, lines.lines()), Ok(()));
    assert_eq!(
        holds("the-frames-band-with-a-long-title.png", &canvas),
        Ok(())
    );
}

#[test]
fn the_frames_band_at_one_and_a_half_with_an_agents_line_and_a_refusal() {
    let (not_found, _) = refusals();
    let mut lines = AgentLines::default();
    lines.did(agents_sentence());
    lines.refused(not_found);
    // 1000 points at 1.5.
    let (canvas, placed, band) = frame_with_band(
        (1500, 81),
        Scale::ONE_AND_A_HALF,
        &invoices(),
        lines.lines(),
    )
    .unwrap_or_else(|why| panic!("{why}"));
    // The band a whole number of CSS pixels, its device pixels what 1.5
    // makes of them.
    assert_eq!(
        placed.band.width,
        Scale::ONE_AND_A_HALF.device(placed.band_width)
    );
    assert!(placed.band.width <= placed.renderer.x / 2, "{placed:?}");
    assert_eq!(band.height, 81);
    let first = placed.agent_lines[0].unwrap_or_else(|| panic!("{placed:?}"));
    assert_eq!(first.x, placed.band.width + 2);
    assert_eq!(band_one_to_one(&canvas, &band, 79), None);
    assert_eq!(drawn_where_placed(&canvas, &placed, lines.lines()), Ok(()));
    assert_eq!(the_renderer_and_the_foot(&canvas, &placed), Ok(()));
    assert_eq!(
        holds("the-frames-band-at-one-and-a-half.png", &canvas),
        Ok(())
    );
}
