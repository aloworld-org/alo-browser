/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 877: an outer `box-shadow` is kept outside the box's border
//! box, as Backgrounds 3 says and Chrome draws it — never seen through the
//! box's own transparent or translucent background, nor under its
//! transparent border. This engine drew it whole under the box, so
//! `box-shadow: 0 0 0 3px` on a transparent button filled it.
//!
//! **Measured** in Chrome 157.0.8103.38 (`--version`), headless, its
//! default raster path, at one and two (`references/chrome-outer-shadows.png`
//! and `-at-2.png`, frozen as RGBA) and at eight (the session scratchpad's
//! `m877/`, not committed): seven boxes on whole pixels, white page —
//! (a) a transparent box rounded 8 ringed `0 0 0 3px` in the ink at a
//! third, as *Back to the chat*'s keyed ring is; (b) a transparent box
//! rounded 14 casting a blurred, offset shadow; (c) a translucent
//! background over a hard spread; (d) a transparent border of 4 under a
//! black ring; (e) the card's own white box and two shadows, which the clip
//! must not change; (f) an offset shadow shrunk by its spread, only its
//! edge outside the box; (g) a blurred glow and a ring of 1 on a box
//! rounded 20.
//!
//! In every box Chrome draws the page's white (or the translucent
//! background over white) and no shadow. Before: up to 255 off inside the
//! boxes. Now: inside each box Chrome's to a level; every pixel within 13
//! at one and 20 at two, the most at a rounded corner where the clip's
//! edge and the shadow's meet — at eight, (a) is within 6 everywhere and
//! of (d)'s pixels more than 6 off, 38 of 379 are at the box's edge, the
//! rest the spread's own corner (queue item 573's shape) — so at one and
//! two those are Chrome's coarse edge coverage, which it rounds to
//! quarters at one; and the blurred ones within 14, a box blur not
//! Chrome's.

use alo_layout::Size;
use alo_paint::{Canvas, DisplayItem, from_png};
use alo_renderer::Screen;
use alo_text::FontDatabase;
use std::path::Path;

const PAGE: &str = "<!DOCTYPE html><html><body>\
<div id=a></div><div id=b></div><div id=c></div><div id=d></div><div id=e></div><div id=f></div><div id=g></div>\
</body></html>";

const STYLE: &str = "html, body { margin: 0; background: #ffffff; }
body { width: 500px; height: 420px; display: flex; flex-wrap: wrap; align-content: flex-start; padding: 20px; box-sizing: border-box; gap: 30px; }
div { flex: none; width: 120px; height: 40px; }
#a { border-radius: 8px; box-shadow: 0 0 0 3px rgba(27, 36, 51, 0.33); }
#b { height: 60px; border-radius: 14px; box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5); }
#c { background: rgba(42, 157, 143, 0.5); box-shadow: 0 0 0 6px #e76f51; }
#d { width: 112px; height: 32px; border: 4px solid transparent; border-radius: 10px; box-shadow: 0 0 0 3px #000000; }
#e { height: 60px; border-radius: 12px; background: #ffffff; box-shadow: 0 1px 2px rgba(27, 36, 51, 0.05), 0 10px 28px rgba(27, 36, 51, 0.2); }
#f { box-shadow: 10px 10px 0 -2px #264653; border-radius: 6px; }
#g { height: 60px; border-radius: 20px; box-shadow: 0 0 6px 2px rgba(231, 111, 81, 0.8), 0 0 0 1px #264653; }";

/// Each box's border box, in CSS pixels: where it was laid out, three a
/// row, 30 apart, from 20 in.
const BOXES: [(char, u32, u32, u32, u32); 7] = [
    ('a', 20, 20, 120, 40),
    ('b', 170, 20, 120, 60),
    ('c', 320, 20, 120, 40),
    ('d', 20, 110, 120, 40),
    ('e', 170, 110, 120, 60),
    ('f', 320, 110, 120, 40),
    ('g', 20, 200, 120, 60),
];

fn drawn(scale: f32) -> alo_renderer::Rendered {
    alo_renderer::render_at(
        PAGE,
        STYLE,
        Screen::new(Size::new(500.0, 420.0)).at_scale(scale),
        &FontDatabase::new(),
        &[],
        &[],
    )
}

fn read(name: &str) -> Option<Canvas> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name);
    std::fs::read(path)
        .ok()
        .and_then(|bytes| from_png(&bytes).ok())
}

/// How far two pixels are apart, in the furthest of their colours.
fn apart(canvas: &Canvas, chrome: &Canvas, x: u32, y: u32) -> Option<u8> {
    let (ours, theirs) = (canvas.at(x, y)?.to_rgba8(), chrome.at(x, y)?.to_rgba8());
    Some(
        ours.0
            .abs_diff(theirs.0)
            .max(ours.1.abs_diff(theirs.1))
            .max(ours.2.abs_diff(theirs.2)),
    )
}

#[test]
fn each_outer_shadow_is_drawn_inside_a_clip_of_everything_but_its_box() {
    let rendered = drawn(1.0);
    let items = rendered.drawing.display.items();
    let mut outer = 0;
    for (at, item) in items.iter().enumerate() {
        let DisplayItem::Shadow { path, .. } = item else {
            continue;
        };
        // The clip it is drawn in is the nearest clip pushed before it
        // and not yet popped: a ring whose bounds hold the whole shadow.
        let mut depth = 0;
        let clip = items[..at].iter().rev().find_map(|before| match before {
            DisplayItem::PopClip => {
                depth += 1;
                None
            }
            DisplayItem::PushClip { path, .. } if depth == 0 => Some(path),
            DisplayItem::PushClip { .. } => {
                depth -= 1;
                None
            }
            _ => None,
        });
        let (Some(clip), Some(shadow)) = (clip.and_then(alo_paint::Path::bounds), path.bounds())
        else {
            panic!("shadow {at} drawn in no clip");
        };
        assert!(
            clip.0 <= shadow.0 && clip.1 <= shadow.1 && clip.2 >= shadow.2 && clip.3 >= shadow.3,
            "{clip:?} does not hold {shadow:?}"
        );
        outer += 1;
    }
    // Every box's every shadow: one each, two for (e) and (g).
    assert_eq!(outer, 9);
}

#[test]
fn inside_each_box_no_shadow_is_seen_as_chrome_draws_it() {
    for (scale, name) in [
        (1_u32, "chrome-outer-shadows.png"),
        (2, "chrome-outer-shadows-at-2.png"),
    ] {
        #[expect(clippy::cast_precision_loss, reason = "one or two")]
        let rendered = drawn(scale as f32);
        let canvas = &rendered.drawing.canvas;
        let Some(chrome) = read(name) else {
            panic!("Chrome's picture {name} is beside the references");
        };
        assert_eq!(
            (chrome.width(), chrome.height()),
            (canvas.width(), canvas.height())
        );
        for (id, x, y, width, height) in BOXES {
            // Inside its shape, 8 in from its edges — inside a corner
            // rounded up to 20, which leaves 5.9 of it out at most: Chrome's
            // own pixels.
            for down in (y + 8) * scale..(y + height - 8) * scale {
                for across in (x + 8) * scale..(x + width - 8) * scale {
                    let far = apart(canvas, &chrome, across, down).unwrap_or(u8::MAX);
                    assert!(
                        far <= 1,
                        "({id}) at ({across}, {down}) at {scale}: {far} off"
                    );
                }
            }
        }
        // The keyed ring's box, (a): the page's white in its middle, where
        // the ring's ink at a third had filled it.
        let middle = canvas.at(80 * scale, 40 * scale).map(|pixel| {
            let (red, green, blue, _) = pixel.to_rgba8();
            (red, green, blue)
        });
        assert_eq!(middle, Some((255, 255, 255)));
    }
}

#[test]
fn every_pixel_is_chromes_within_its_edges_and_the_blurs_difference() {
    for (scale, name, worst, past) in [
        (1_u32, "chrome-outer-shadows.png", 13_u8, 64_usize),
        (2, "chrome-outer-shadows-at-2.png", 20, 74),
    ] {
        #[expect(clippy::cast_precision_loss, reason = "one or two")]
        let rendered = drawn(scale as f32);
        let canvas = &rendered.drawing.canvas;
        let Some(chrome) = read(name) else {
            panic!("Chrome's picture {name} is beside the references");
        };
        let mut most = 0;
        let mut far = 0;
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                let off = apart(canvas, &chrome, x, y).unwrap_or(u8::MAX);
                most = most.max(off);
                far += usize::from(off > 8);
            }
        }
        assert!(most <= worst, "{most} off at {scale}");
        assert!(far <= past, "{far} pixels more than 8 off at {scale}");
    }
}
