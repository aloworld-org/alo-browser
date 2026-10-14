/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page laid out and drawn for a screen of one and a half device pixels to
//! the CSS pixel is where Chrome puts it and what Chrome draws (ADR 0057 § 2,
//! *Measured since*; queue item 593).
//!
//! Every number and picture here is Chrome 154's, headless, started with
//! `--force-device-scale-factor=1.5` and measured on 2026-10-14, the source
//! not read. At 1.5 Chrome's numbers are whole sixty-fourths of a device
//! pixel, 1/96 of a CSS pixel, which no float holds; this engine's are
//! within a float's error of them, so a number is compared in sixty-fourths
//! of a device pixel to within [`SLACK`] of one.
//!
//! - **The rules one at a time**, at one, two and 1.5:
//!   `references/rules-at-a-scale.html` holds ADR 0057 § 2's line, leading,
//!   length and face-size probes, and a line height written as a number at
//!   eleven sizes, and `rules-at-a-scale.chrome-at-*.txt` is what Chrome made
//!   of them.
//! - **The corpus**: `chrome-at-1.5.txt` beside 75 cases is Chrome's box for
//!   every element, and every box this engine places exactly where Chrome
//!   does at one, it places where Chrome does at 1.5, but for the boxes
//!   [`NOT_THE_SCALE`] names.
//! - **Pictures**: six cases drawn at 1.5 (`render-at-1.5.png`) are Chrome's
//!   (`chrome-at-1.5.png`) wherever both are one of Chrome's plain colours.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use alo_corpus::{Case, Rendering, cases_directory, check_picture, corpus_fonts};
use alo_dom::{Document, NodeId};
use alo_layout::Size;
use alo_paint::Canvas;
use alo_renderer::{Screen, render_at};

/// An element's border box in sixty-fourths of a CSS pixel: left, top,
/// width, height.
type Units = [f32; 4];

/// How far from Chrome's a number may be, in sixty-fourths of a device
/// pixel: a twentieth of one. A float's error at the corpus's deepest box,
/// 2,089 px down, is 0.023 of one; a rule rounded the other way is a whole
/// one.
const SLACK: f32 = 0.05;

/// The same number at `scale`, to within [`SLACK`] of a device pixel's
/// sixty-fourth.
fn same(left: f32, right: f32, scale: f32) -> bool {
    ((left - right) * scale).abs() <= SLACK
}

fn references() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("references")
}

/// Every element's border box, by its path from the root (`/html0/body1/p0`
/// is the first element child of the second element child of the first),
/// the same path the Chrome files name.
fn boxes_by_path(document: &Document, drawing: &alo_renderer::Drawing) -> BTreeMap<String, Units> {
    let mut first = BTreeMap::new();
    for id in drawing.boxes.ids() {
        if let Some(alo_box::BoxKind::Element { node, .. }) =
            drawing.boxes.get(id).map(|found| &found.kind)
        {
            first.entry(*node).or_insert(id);
        }
    }
    let mut paths = Vec::new();
    walk(document, document.root(), "", &mut paths);
    let mut found = BTreeMap::new();
    for (path, node) in paths {
        let Some(geometry) = first.get(&node).and_then(|id| drawing.layout.get(*id)) else {
            continue;
        };
        let rect = geometry.border_box;
        found.insert(
            path,
            [
                rect.origin.x * 64.0,
                rect.origin.y * 64.0,
                rect.size.width * 64.0,
                rect.size.height * 64.0,
            ],
        );
    }
    found
}

fn walk(document: &Document, node: NodeId, path: &str, out: &mut Vec<(String, NodeId)>) {
    let mut index = 0;
    for child in document.children(node) {
        if let Some(element) = document.element(child) {
            let here = format!("{path}/{}{index}", element.name.local);
            index += 1;
            walk(document, child, &here, out);
            out.push((here, child));
        }
    }
}

/// Numbers after a name on each line of `text` that is not a comment.
fn numbered_lines(text: &str) -> BTreeMap<String, Vec<f32>> {
    let mut found = BTreeMap::new();
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let mut words = line.split_whitespace();
        let Some(name) = words.next() else {
            continue;
        };
        let numbers: Option<Vec<f32>> = words.map(|word| word.parse().ok()).collect();
        if let Some(numbers) = numbers {
            found.insert(name.to_owned(), numbers);
        }
    }
    found
}

/// The probes as this engine lays them out at `scale`, named and ordered as
/// Chrome's file names them: a section's letter and the probe's index, then
/// its top within the section, width and height, then the baseline mark's
/// top within the line (B and E) or the run's width (D).
fn probes_at(html: &str, scale: f32) -> BTreeMap<String, Vec<f32>> {
    let screen = Screen::new(Size::new(1700.0, 3000.0)).at_scale(scale);
    let rendered = render_at(html, "", screen, &corpus_fonts(), &[], &[]);
    let boxes = boxes_by_path(&rendered.document, &rendered.drawing);
    let sections = [
        ("A", "p", ""),
        ("B", "div", "i1"),
        ("C", "div", ""),
        ("D", "div", "span0"),
        ("E", "div", "i1"),
    ];
    let mut found = BTreeMap::new();
    for (index, (name, tag, inner)) in sections.into_iter().enumerate() {
        let section = format!("/html0/body1/div{index}");
        let Some(top) = boxes.get(&section) else {
            continue;
        };
        for probe in 0.. {
            let path = format!("{section}/{tag}{probe}");
            let Some(units) = boxes.get(&path) else {
                break;
            };
            let mut numbers = vec![units[1] - top[1], units[2], units[3]];
            if let Some(mark) = boxes.get(&format!("{path}/{inner}")) {
                numbers.push(if name == "D" {
                    mark[2]
                } else {
                    mark[1] - units[1]
                });
            }
            found.insert(format!("{name}{probe}"), numbers);
        }
    }
    found
}

#[test]
fn each_rule_rounds_in_device_pixels_at_one_two_and_one_and_a_half() {
    let Ok(page) = std::fs::read_to_string(references().join("rules-at-a-scale.html")) else {
        panic!("the probe page is beside the tests");
    };
    // The page's script is Chrome's, which reads the boxes out; this engine
    // runs none and is read directly.
    let html = page.split("<script>").next().unwrap_or_default();
    let mut wrong = Vec::new();
    let mut compared = 0_usize;
    for (scale, name) in [(1.0, "1"), (2.0, "2"), (1.5, "1.5")] {
        let file = references().join(format!("rules-at-a-scale.chrome-at-{name}.txt"));
        let Ok(text) = std::fs::read_to_string(&file) else {
            panic!("{} is beside the tests", file.display());
        };
        let chrome = numbered_lines(&text);
        assert_eq!(chrome.len(), 252, "every probe Chrome measured at {name}");
        let ours = probes_at(html, scale);
        for (probe, theirs) in &chrome {
            let Some(mine) = ours.get(probe) else {
                wrong.push(format!("{probe} at {name}: not laid out"));
                continue;
            };
            // D's block takes Chrome's default face around the run, which
            // this engine does not have; the run's width is what D measures.
            let fields: &[usize] = if probe.starts_with('D') {
                &[3]
            } else {
                &[0, 1, 2, 3]
            };
            for field in fields {
                let (Some(a), Some(b)) = (mine.get(*field), theirs.get(*field)) else {
                    continue;
                };
                compared += 1;
                if !same(*a, *b, scale) {
                    wrong.push(format!(
                        "{probe} at {name}, number {field}: Chrome {b}, this engine {a}"
                    ));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // At each scale: 65 lines of three numbers, 45 set line heights and 121
    // written as numbers of four, 12 lengths of three, and 9 runs' widths.
    assert_eq!(compared, 3 * 904, "every number Chrome measured");
}

/// The boxes Chrome places otherwise from this engine at 1.5 for a reason
/// that is not one of ADR 0057 § 2's rules, each with the queue item that
/// owns it. A box here that comes to match is said, so the list cannot
/// outlive its reason.
///
/// - `border-widths`' two flex items shrink by a share Chrome hands out
///   otherwise, at two as at 1.5 (item 564).
///
/// `alo-offline`'s centred button and `alo-meet-greeting`'s line holding a
/// picture aligned `middle` were here until both halves were put on the grid
/// as Chrome puts them (item 598).
const NOT_THE_SCALE: [(&str, &str, u16); 2] = [
    ("border-widths", "/html0/body1/div0", 564),
    ("border-widths", "/html0/body1/div1", 564),
];

/// Chrome's boxes beside a case at one scale, by path, or [`None`] when the
/// case has no such file.
fn chrome_boxes(directory: &Path, scale: &str) -> Option<BTreeMap<String, Units>> {
    let text = std::fs::read_to_string(directory.join(format!("chrome-at-{scale}.txt"))).ok()?;
    let mut found = BTreeMap::new();
    for (path, numbers) in numbered_lines(&text) {
        let [left, top, width, height] = numbers.as_slice() else {
            return None;
        };
        found.insert(path, [*left, *top, *width, *height]);
    }
    Some(found)
}

/// The case's boxes at `scale`, by path.
fn ours(case: &Case, scale: f32) -> Result<BTreeMap<String, Units>, String> {
    let rendering = Rendering::at_scale(case, scale)?;
    let (Some(document), Some(drawing)) = (rendering.document(), rendering.drawing()) else {
        return Err(format!("drew nothing at {scale}"));
    };
    Ok(boxes_by_path(document, &drawing))
}

#[test]
fn every_box_placed_as_chrome_places_it_at_one_is_placed_as_chrome_places_it_at_one_and_a_half() {
    let mut compared = 0_usize;
    let mut cases = 0_usize;
    let mut wrong = Vec::new();
    let mut excused = Vec::new();
    let Ok(entries) = std::fs::read_dir(cases_directory()) else {
        panic!("no corpus at {}", cases_directory().display());
    };
    let mut directories: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
    directories.sort();
    for directory in directories {
        let (Some(chrome_one), Some(chrome_there)) = (
            chrome_boxes(&directory, "1"),
            chrome_boxes(&directory, "1.5"),
        ) else {
            continue;
        };
        let Some(case) = Case::read(&directory) else {
            wrong.push(format!("{}: not a case", directory.display()));
            continue;
        };
        let (one, there) = match (ours(&case, 1.0), ours(&case, 1.5)) {
            (Ok(one), Ok(there)) => (one, there),
            (Err(why), _) | (_, Err(why)) => {
                wrong.push(format!("{}: {why}", case.name));
                continue;
            }
        };
        cases += 1;
        for (path, chrome) in &chrome_one {
            if path.starts_with("/html0/head") {
                continue;
            }
            let (Some(mine_one), Some(mine), Some(theirs)) =
                (one.get(path), there.get(path), chrome_there.get(path))
            else {
                continue;
            };
            if !mine_one.iter().zip(chrome).all(|(a, b)| same(*a, *b, 1.0)) {
                continue;
            }
            compared += 1;
            let matches = mine.iter().zip(theirs).all(|(a, b)| same(*a, *b, 1.5));
            let excuse = NOT_THE_SCALE
                .iter()
                .find(|(name, at, _)| *name == case.name && at == path);
            match (matches, excuse) {
                (true, None) => {}
                (true, Some((_, _, item))) => wrong.push(format!(
                    "{} {path} now matches at 1.5: take it off item {item}'s list",
                    case.name
                )),
                (false, Some(_)) => excused.push(path.clone()),
                (false, None) => wrong.push(format!(
                    "{} {path}: Chrome {theirs:?} at 1.5, this engine {mine:?}",
                    case.name
                )),
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert_eq!(cases, 75, "every case Chrome was measured on at 1.5");
    assert!(
        compared >= 985,
        "{compared} boxes compared, where 985 were exact at one on 2026-10-14"
    );
    assert_eq!(excused.len(), NOT_THE_SCALE.len(), "{excused:?}");
}

/// The cases drawn at 1.5 against Chrome's pictures, and how many pixels of
/// each may be more than 40 of 255 from Chrome's: the four corners of each
/// rounded shape, where Chrome covers a pixel coarsely at 1.5 as at one
/// (ADR 0057, *Measured since*, item 588). Measured: 8, 8, 0, 5, 9 and 4.
const PICTURES: [(&str, usize); 6] = [
    ("backgrounds-on-device-pixels", 8),
    ("clips-on-device-pixels", 8),
    ("pictures-on-device-pixels", 0),
    ("svgs-on-device-pixels", 5),
    ("rounded-clips-inside-borders", 9),
    ("spread-shadow-corners", 4),
];

/// How many of a picture's pixels a colour must fill to be one of its plain
/// colours: what a box is filled with rather than a blend at its edge.
const PLAIN: usize = 200;

/// Chrome's plain colours in `picture`.
fn plain_colours(picture: &Canvas) -> Vec<(u8, u8, u8, u8)> {
    let mut counts: BTreeMap<(u8, u8, u8, u8), usize> = BTreeMap::new();
    for y in 0..picture.height() {
        for x in 0..picture.width() {
            if let Some(colour) = picture.at(x, y) {
                *counts.entry(colour.to_rgba8()).or_default() += 1;
            }
        }
    }
    counts
        .into_iter()
        .filter(|(_, count)| *count >= PLAIN)
        .map(|(colour, _)| colour)
        .collect()
}

#[test]
fn a_page_drawn_at_one_and_a_half_is_chromes_in_every_plain_pixel() {
    for (name, corners) in PICTURES {
        let Some(case) = Case::read(&cases_directory().join(name)) else {
            panic!("{name} reads");
        };
        let Some(drawing) = Rendering::at_scale(&case, 1.5)
            .ok()
            .and_then(|rendering| rendering.drawing())
        else {
            panic!("{name} is drawn at 1.5");
        };
        let ours = &drawing.canvas;
        let Some(chrome) = std::fs::read(case.expectation("chrome-at-1.5.png"))
            .ok()
            .and_then(|bytes| alo_paint::from_png(&bytes).ok())
        else {
            panic!("{name}'s chrome-at-1.5.png is beside it");
        };
        assert_eq!(
            (ours.width(), ours.height()),
            (chrome.width(), chrome.height()),
            "{name}"
        );
        let plain = plain_colours(&chrome);
        let mut both_plain = 0_usize;
        let mut differ = Vec::new();
        let mut far = 0_usize;
        for y in 0..chrome.height() {
            for x in 0..chrome.width() {
                let (Some(mine), Some(theirs)) = (ours.at(x, y), chrome.at(x, y)) else {
                    continue;
                };
                let (mine, theirs) = (mine.to_rgba8(), theirs.to_rgba8());
                let apart = [
                    mine.0.abs_diff(theirs.0),
                    mine.1.abs_diff(theirs.1),
                    mine.2.abs_diff(theirs.2),
                ]
                .into_iter()
                .max()
                .unwrap_or(u8::MAX);
                far += usize::from(apart > 40);
                if plain.contains(&mine) && plain.contains(&theirs) {
                    both_plain += 1;
                    if mine != theirs {
                        differ.push((x, y));
                    }
                }
            }
        }
        assert!(
            differ.is_empty(),
            "{name}: {} plain pixels differ, first {:?}",
            differ.len(),
            differ.first()
        );
        let pixels = usize::try_from(u64::from(chrome.width()) * u64::from(chrome.height()))
            .unwrap_or(usize::MAX);
        assert!(
            both_plain * 10 >= pixels * 9,
            "{name}: only {both_plain} pixels are plain in both"
        );
        assert!(
            far <= corners,
            "{name}: {far} pixels more than 40 from Chrome's"
        );
        let difference = check_picture(&case, "render-at-1.5.png", ours);
        assert!(difference.is_none(), "{name}: {difference:?}");
    }
}
