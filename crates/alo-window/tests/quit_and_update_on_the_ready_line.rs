/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 990: the first screen's line saying an update is ready, with
//! *Quit and update* at its right end (ADR 0116 § 4) — the browser's own
//! drawing, over whatever the first screen painted, never inside a page.
//!
//! A small window, composed with no frame painted, so the line and its
//! button are all there is: checked in numbers — the button's ring where
//! the placement puts it, its ivory inside, the line's tint either side —
//! and compared with a committed reference at one and at two.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-window --test
//! quit_and_update_on_the_ready_line` rewrites the references. A rewritten
//! reference is a diff somebody reads.

use alo_paint::{Canvas, from_png, to_png};
use alo_window::colours::{button_ground, notice_ground, notice_ink};
use alo_window::conversation_compose::{ConversationScene, compose_conversation};
use alo_window::conversation_place::{ConversationPlacement, Side};
use alo_window::line_button::LineButton;
use alo_window::notice::Lettering;
use alo_window::place::PixelRect;
use alo_window::scale::Scale;
use std::path::{Path, PathBuf};

/// What the line says, as `alo-update` says it.
const READY: &str = "alo 0.4.2 is ready. It installs when you quit alo.";

fn lettering() -> Result<Lettering, String> {
    Lettering::compiled_in().ok_or_else(|| "DejaVu Sans is not compiled in".to_owned())
}

/// A window of `window` at `scale` saying an update is ready, its button
/// at the end of the line when `button`.
fn scene(window: (u32, u32), scale: Scale, button: bool) -> ConversationScene<'static> {
    ConversationScene {
        window,
        scale,
        panel: None,
        panel_sentence: None,
        start: None,
        start_sentence: Some(READY),
        start_button: button.then_some(LineButton::QuitAndUpdate),
        side: Side::Panel,
        named: false,
        sheet: None,
        beneath: None,
    }
}

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

/// Compare `canvas` with the committed reference, or write it when
/// updating. What differs, in words, if anything does.
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
    let differing = (0..canvas.height())
        .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
        .filter(|(x, y)| {
            canvas.at(*x, *y).map(alo_value::Rgba::to_rgba8)
                != expected.at(*x, *y).map(alo_value::Rgba::to_rgba8)
        })
        .collect::<Vec<_>>();
    match differing.first() {
        None => Ok(()),
        Some(first) => Err(format!(
            "{name}: {} pixels moved, the first at {first:?}. If this is intended, \
             ALO_UPDATE_REFERENCES=1 rewrites the reference",
            differing.len(),
        )),
    }
}

/// The colour at `x`, `y` of `canvas`.
fn at(canvas: &Canvas, x: u32, y: u32) -> Option<(u8, u8, u8, u8)> {
    canvas.at(x, y).map(alo_value::Rgba::to_rgba8)
}

/// Compose the window at `scale`, check the button stands at `expected`
/// and is drawn there in numbers, and compare it with `name`.
fn the_line_at(
    window: (u32, u32),
    scale: Scale,
    expected: PixelRect,
    name: &str,
) -> Result<(), String> {
    let scene = scene(window, scale, true);
    let placed = ConversationPlacement::of(window, scale, scene.parts());
    assert_eq!(placed.start_button, Some(expected), "{name}");
    let canvas = compose_conversation(&scene, &lettering()?);
    let ring = scale.device(1);
    let PixelRect {
        x,
        y,
        width,
        height,
    } = expected;
    let (right, bottom) = (x + width - 1, y + height - 1);
    let middle = y + height / 2;
    // Its ring on each side, its ivory just inside it, and the line's tint
    // just outside.
    for (column, row) in [(x, middle), (right, middle), (x + 4, y), (x + 4, bottom)] {
        assert_eq!(
            at(&canvas, column, row),
            Some(notice_ink().to_rgba8()),
            "{name}: ring at {column},{row}"
        );
    }
    assert_eq!(
        at(&canvas, x + ring, y + ring),
        Some(button_ground().to_rgba8()),
        "{name}"
    );
    for (column, row) in [(x - 1, middle), (right + 1, middle), (x + 4, y - 1)] {
        assert_eq!(
            at(&canvas, column, row),
            Some(notice_ground().to_rgba8()),
            "{name}: the line at {column},{row}"
        );
    }
    // The same window without the button differs only where the button,
    // and the words cut short of it, are.
    let plain = compose_conversation(&self::scene(window, scale, false), &lettering()?);
    let line_top = placed.start_notice.map_or(0, |line| line.y);
    for row in 0..line_top {
        for column in 0..window.0 {
            assert_eq!(
                at(&canvas, column, row),
                at(&plain, column, row),
                "{name}: above the line at {column},{row}"
            );
        }
    }
    holds(name, &canvas)
}

#[test]
fn the_ready_line_and_its_button_at_one() {
    // The line 640 by 32 along the bottom of a window 640 by 72; the
    // button 128 by 24, 12 in from its right end and 4 down.
    the_line_at(
        (640, 72),
        Scale::ONE,
        PixelRect {
            x: 500,
            y: 44,
            width: 128,
            height: 24,
        },
        "the-ready-line-and-its-button.png",
    )
    .unwrap_or_else(|why| panic!("{why}"));
}

#[test]
fn the_ready_line_and_its_button_at_two() {
    the_line_at(
        (1280, 144),
        Scale::TWO,
        PixelRect {
            x: 1000,
            y: 88,
            width: 256,
            height: 48,
        },
        "the-ready-line-and-its-button-at-two.png",
    )
    .unwrap_or_else(|why| panic!("{why}"));
}
