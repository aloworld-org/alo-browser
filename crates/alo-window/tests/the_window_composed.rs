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
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-window` rewrites them, as it
//! does the corpus's. A rewritten reference is a diff somebody reads.

use alo_layout::Size;
use alo_paint::{Canvas, from_png, to_png};
use alo_renderer::host::Gone;
use alo_renderer::strip::{Condition, Strip, StripTab};
use alo_renderer::{Frame, FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_window::compose::{Scene, compose};
use alo_window::message::News;
use alo_window::notice::Lettering;
use alo_window::place::{STRIP_HEIGHT, screen, strip_viewport};
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
fn placed_one_to_one(canvas: &Canvas, frame: &Frame, scale: u32) -> Option<(u32, u32)> {
    let top = STRIP_HEIGHT * scale;
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
            scale: 2,
            strip: None,
            strip_sentence: None,
            frame: Some(&page),
            sentence: None,
        },
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    assert_eq!(placed_one_to_one(&canvas, &page, 2), None);
    assert_eq!(holds("a-frozen-page-at-scale-two.png", &canvas), Ok(()));
}

#[test]
fn a_window_grown_before_its_new_frame_shows_the_old_one_at_its_old_size() {
    let page = frozen_page().unwrap_or_else(|why| panic!("{why}"));
    let canvas = compose(
        &Scene {
            window: (560, 460 + STRIP_HEIGHT),
            scale: 1,
            strip: None,
            strip_sentence: None,
            frame: Some(&page),
            sentence: None,
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
            scale: 1,
            strip: None,
            strip_sentence: None,
            frame: Some(&page),
            sentence: Some(&sentence),
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
            scale: 2,
            strip: None,
            strip_sentence: None,
            frame: None,
            sentence: Some(&sentence),
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
    let told = screen(window, 2);
    assert_eq!(told.size, Size::new(480.0, 400.0));
    assert_eq!(told.scale().to_bits(), 2.0_f32.to_bits());
    let strip =
        strip_of(vec![the_pages_tab()], 480.0, told.scale()).unwrap_or_else(|why| panic!("{why}"));
    assert_eq!((strip.width, strip.height), (960, 2 * STRIP_HEIGHT));
    let canvas = compose(
        &Scene {
            window,
            scale: 2,
            strip: Some(&strip),
            strip_sentence: None,
            frame: Some(&page),
            sentence: None,
        },
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    assert_eq!(placed_one_to_one(&canvas, &page, 2), None);
    // And the band is the strip's frame, one to one from the corner.
    let in_band = (0..strip.height)
        .flat_map(|y| (0..strip.width).map(move |x| (x, y)))
        .find(|(x, y)| canvas.at(*x, *y).map(alo_value::Rgba::to_rgba8) != strip.at(*x, *y));
    assert_eq!(in_band, None);
    assert_eq!(holds("a-strip-of-one-tab.png", &canvas), Ok(()));
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
            scale: 1,
            strip: Some(&strip),
            strip_sentence: None,
            frame: Some(&page),
            sentence: None,
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
            scale: 1,
            strip: Some(&strip),
            strip_sentence: Some(&sentence),
            frame: Some(&page),
            sentence: None,
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
        &showing.scene(window, 1),
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
