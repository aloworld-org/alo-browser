/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The window's pixels, against committed references.
//!
//! ADR 0024 § 2: composition *"is tested like every other picture here: a
//! reference render, so a change that moves a pixel of the window's layout
//! says so."* Item 296 names three: a frozen alo page, a resize before its new
//! frame, and a gone tab's sentence. A fourth is the sentence for a tab that
//! never painted at all.
//!
//! The frame in each is a frozen alo page **as the corpus committed it** —
//! `alo-corpus/cases/alo-offline/render.png` — so what is pinned here is the
//! window's arrangement of a page, not the page; the page's own reference is
//! the corpus's to keep.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-window` rewrites them, as it
//! does the corpus's. A rewritten reference is a diff somebody reads.

use alo_paint::{Canvas, from_png, to_png};
use alo_renderer::Frame;
use alo_renderer::host::Gone;
use alo_window::compose::{Scene, compose};
use alo_window::notice::Lettering;
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
    let page = frozen_page().unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(
        (page.width, page.height),
        (480, 400),
        "the corpus case changed size"
    );
    let canvas = compose(
        &Scene {
            window: (960, 800),
            replication: 2,
            frame: Some(&page),
            sentence: None,
        },
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    assert_eq!(holds("a-frozen-page-at-scale-two.png", &canvas), Ok(()));
}

#[test]
fn a_window_grown_before_its_new_frame_shows_the_old_one_at_its_old_size() {
    let page = frozen_page().unwrap_or_else(|why| panic!("{why}"));
    let canvas = compose(
        &Scene {
            window: (560, 460),
            replication: 1,
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
            window: (480, 400),
            replication: 1,
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
            window: (480, 120),
            replication: 2,
            frame: None,
            sentence: Some(&sentence),
        },
        &lettering().unwrap_or_else(|why| panic!("{why}")),
    );
    assert_eq!(holds("a-gone-tab-that-never-painted.png", &canvas), Ok(()));
}
