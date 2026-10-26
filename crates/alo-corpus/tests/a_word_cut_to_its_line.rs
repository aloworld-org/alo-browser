/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A word too wide for its line is cut between its letters where
//! `overflow-wrap` says, in numbers: the case that closes queue item 666.
//!
//! `cases/a-word-cut-to-its-line` pins the page as files. This says what
//! Chrome 155.0.8059.86 gives for the same bytes, headless, its sheet linked
//! after the doctype and every element set in the corpus's `DejaVu Sans`,
//! measured 2026-10-26: each box's border box in sixty-fourths of a pixel,
//! and on each of its lines the text and where its last letter ends, read
//! from a `Range` around each character. Until item 666 nothing here read
//! `overflow-wrap`, so every long word ran past its box on one line, and a
//! `<textarea>` was scrolled across to it.

use alo_box::{BoxId, BoxKind};
use alo_corpus::{Case, Rendering, cases_directory};
use alo_layout::Rect;

/// A box: its `id`, Chrome's border box (left, top, width, height in
/// sixty-fourths), and each line's text with where its last letter ends.
type Row = (&'static str, [u16; 4], &'static [(&'static str, u16)]);

/// Each box, as Chrome 155 lays it out.
const ROWS: [Row; 16] = [
    (
        "break-word",
        [512, 512, 3200, 7680],
        &[
            ("aaaaa", 3650),
            ("aaaaa", 3650),
            ("aaaaa", 3650),
            ("aaaaa", 3650),
            ("aaaa ", 3022),
            ("bb", 1812),
        ],
    ),
    (
        "after-a-word",
        [4608, 512, 3200, 5120],
        &[
            ("ab ", 5886),
            ("ccccc", 7423),
            ("ccccc", 7423),
            ("cc dd", 7360),
        ],
    ),
    (
        "anywhere",
        [8704, 512, 3200, 7680],
        &[
            ("aaaaa", 11842),
            ("aaaaa", 11842),
            ("aaaaa", 11842),
            ("aaaaa", 11842),
            ("aaaa ", 11214),
            ("bb", 10004),
        ],
    ),
    (
        "normal",
        [12800, 512, 3200, 2560],
        &[("aaaaaaaaaaaaaaaaaaaaaaaa ", 27860), ("bb", 14100)],
    ),
    (
        "narrowest-break-word",
        [512, 8448, 6756, 2560],
        &[("ab ", 1790), ("cccccccccccc", 7268)],
    ),
    (
        "narrowest-anywhere",
        [4608, 8448, 650, 17920],
        &[
            ("a", 5236),
            ("b ", 5258),
            ("c", 5171),
            ("c", 5171),
            ("c", 5171),
            ("c", 5171),
            ("c", 5171),
            ("c", 5171),
            ("c", 5171),
            ("c", 5171),
            ("c", 5171),
            ("c", 5171),
            ("c", 5171),
            ("c", 5171),
        ],
    ),
    (
        "nowrap",
        [8704, 8448, 3200, 1280],
        &[("aaaaaaaaaaaaaaaa", 18744)],
    ),
    (
        "pre-wrap",
        [12800, 8448, 3200, 5120],
        &[
            ("aaaaa", 15938),
            ("aaaaa", 15938),
            ("aa  ", 14055),
            ("bb", 14100),
        ],
    ),
    (
        "across-a-box",
        [512, 26624, 3200, 3840],
        &[("aaabb", 3695), ("bbbb", 3112), ("bbbb", 3112)],
    ),
    (
        "after-a-picture",
        [4608, 26624, 3200, 5120],
        &[("ab", 5886), ("ccccc", 7423), ("ccccc", 7423), ("cc", 5734)],
    ),
    (
        "full-stop",
        [8704, 26624, 3200, 3840],
        &[("ccccc", 11519), ("ccccc", 11519), ("cc.", 10156)],
    ),
    (
        "spaced",
        [12800, 26624, 3200, 3840],
        &[("cccc", 15564), ("cccc", 15564), ("cccc", 15564)],
    ),
    (
        "nought",
        [512, 32000, 0, 3840],
        &[("a", 1140), ("b", 1162), ("c", 1075)],
    ),
    (
        "item-break-word",
        [4608, 32000, 6756, 1280],
        &[("cccccccccccc", 11364)],
    ),
    (
        "item-anywhere",
        [8704, 32000, 3200, 3840],
        &[("ccccc", 11519), ("ccccc", 11519), ("cc", 9830)],
    ),
    (
        "shrunk",
        [12800, 32000, 6756, 2560],
        &[("ab ", 14078), ("cccccccccccc", 19556)],
    ),
];

/// The case, rendered.
fn case() -> Option<Rendering> {
    let case = Case::read(&cases_directory().join("a-word-cut-to-its-line"))?;
    Rendering::of(&case).ok()
}

/// The box of the element whose `id` is `id`.
fn box_of(rendering: &Rendering, id: &str) -> Option<BoxId> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let node = document
        .descendants(document.root())
        .find(|node| document.element(*node).and_then(|e| e.attr("id")) == Some(id))?;
    drawing.boxes.ids().find(|box_id| {
        matches!(
            drawing.boxes.get(*box_id).map(|held| &held.kind),
            Some(BoxKind::Element { node: of, .. }) if *of == node
        )
    })
}

/// The lines of the element whose `id` is `id`, each its text and where
/// its last letter ends, with its border box: its text boxes' pieces put
/// together by the line each stands on, top to bottom.
fn lines_of(rendering: &Rendering, id: &str) -> Option<(Rect, Vec<(String, f32)>)> {
    let drawing = rendering.drawing()?;
    let held = box_of(rendering, id)?;
    let border_box = drawing.layout.get(held)?.border_box;
    let mut lines: Vec<(f32, String, f32)> = Vec::new();
    for text_box in drawing.boxes.descendants(held) {
        let Some(text) = drawing.boxes.get(text_box).and_then(|node| node.text()) else {
            continue;
        };
        for fragment in drawing.layout.fragments(text_box) {
            let Some(piece) = fragment.text.clone().and_then(|range| text.get(range)) else {
                continue;
            };
            let top = fragment.rect.origin.y;
            let right = fragment.rect.right();
            match lines.iter_mut().find(|line| (line.0 - top).abs() < 1.0) {
                Some(line) => {
                    line.1.push_str(piece);
                    line.2 = line.2.max(right);
                }
                None => lines.push((top, piece.to_owned(), right)),
            }
        }
    }
    lines.sort_by(|left, right| left.0.total_cmp(&right.0));
    let lines = lines
        .into_iter()
        .map(|(_, text, right)| (text, right))
        .collect();
    Some((border_box, lines))
}

/// Within Chrome's layout unit.
fn near(left: f32, right: f32) -> bool {
    (left - right).abs() <= 1.0 / 64.0
}

fn pixels(units: u16) -> f32 {
    f32::from(units) / 64.0
}

#[test]
fn each_word_is_cut_where_chrome_cuts_it() {
    let rendering = case().expect("the case renders");
    for (id, [left, top, width, height], expected) in ROWS {
        let (border_box, lines) = lines_of(&rendering, id).expect("the box is laid out");
        let texts: Vec<&str> = lines.iter().map(|(text, _)| text.as_str()).collect();
        let chrome: Vec<&str> = expected.iter().map(|(text, _)| *text).collect();
        assert_eq!(texts, chrome, "{id}: the lines");
        for ((text, right), (_, chrome)) in lines.iter().zip(expected) {
            assert!(
                near(*right, pixels(*chrome)),
                "{id}: {text:?} ends at {right}, Chrome's at {}",
                pixels(*chrome)
            );
        }
        for (ours, chrome, what) in [
            (border_box.origin.x, left, "left"),
            (border_box.origin.y, top, "top"),
            (border_box.size.width, width, "width"),
            (border_box.size.height, height, "height"),
        ] {
            assert!(
                near(ours, pixels(chrome)),
                "{id}: its {what} is {ours}, Chrome's {}",
                pixels(chrome)
            );
        }
    }
}

#[test]
fn a_text_area_cuts_its_words_and_has_nothing_to_scroll_across() {
    // Chrome 155: `scrollHeight` 120 and `scrollWidth` 50 in a box of 50 by
    // 60 under its sheet's `overflow-wrap: break-word`; 60 and 235 under an
    // author's `normal`.
    let rendering = case().expect("the case renders");
    let drawing = rendering.drawing().expect("it drew");
    for (id, across, down) in [
        ("text-area", 50.0, 120.0),
        ("text-area-normal", 235.0, 60.0),
    ] {
        let held = box_of(&rendering, id).expect("a box");
        let area = drawing.layout.get(held).expect("laid out");
        let scrolled = area.scrolling_area();
        assert!(near(scrolled.height, down), "{id}: {scrolled:?}");
        // Chrome's `scrollWidth` is whole pixels; the area here is not.
        assert!(
            (scrolled.width - across).abs() < 1.0,
            "{id}: {scrolled:?}, Chrome's {across}"
        );
    }
}
