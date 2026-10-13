/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page laid out for a screen of two device pixels to the CSS pixel is
//! where Chrome puts it on one (ADR 0057, queue item 560).
//!
//! Every number here is Chrome 154's, headless, started with
//! `--force-device-scale-factor=2` — never the emulated scale of Chrome's developer tools, which
//! reports the ratio and lays out as at one — and measured on 2026-10-13 as
//! `getBoundingClientRect()` in sixty-fourths of a CSS pixel, the source not
//! read. At two every one of them is a whole number of 1/128 px.
//!
//! The 21 cases whose page runs script carry Chrome's boxes too, measured
//! the same day, and are not laid out at two here: a renderer is told no
//! scale until queue item 563, which compares them.
//!
//! Four probes pin the rules one at a time, each at both scales: a line's
//! font figures rounded to whole device pixels, the leading's top half
//! floored to one, a length truncated onto 1/64 of one, and a face's size
//! cut to hundredths of one. Then the corpus:
//! `chrome-at-1.txt` and `chrome-at-2.txt` beside 73 cases are Chrome's
//! boxes for every element at each scale, and in the 52 of markup every box
//! this engine places
//! exactly where Chrome does at one, it places exactly where Chrome does at
//! two.

use std::collections::BTreeMap;
use std::path::Path;

use alo_corpus::{Case, Rendering, cases_directory, corpus_fonts};
use alo_dom::{Document, NodeId};
use alo_layout::Size;
use alo_renderer::{Rendered, Screen, render_at};

/// An element's border box in sixty-fourths of a CSS pixel: left, top,
/// width, height.
type Units = [f32; 4];

/// Lay `html` out in a window `width` across, on a screen at `scale`.
fn laid_out(html: &str, width: f32, scale: f32) -> Rendered {
    let screen = Screen::new(Size::new(width, 2000.0)).at_scale(scale);
    render_at(html, "", screen, &corpus_fonts(), &[], &[])
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

/// The boxes of `html` at `scale`, by path.
fn boxes_of(html: &str, width: f32, scale: f32) -> BTreeMap<String, Units> {
    let rendered = laid_out(html, width, scale);
    boxes_by_path(&rendered.document, &rendered.drawing)
}

/// The same number of sixty-fourths, to well within a unit of 1/128 px.
fn same(left: f32, right: f32) -> bool {
    (left - right).abs() < 0.01
}

/// The sizes the line and leading probes were measured at: 8 to 40 px by
/// halves, and 17 to 30.2 px by 0.3.
fn halves() -> Vec<f32> {
    (0_u8..65).map(|step| 8.0 + f32::from(step) * 0.5).collect()
}

fn line_heights() -> Vec<f32> {
    (0_u8..45)
        .map(|step| 17.0 + f32::from(step) * 0.3)
        .collect()
}

#[test]
fn a_normal_line_is_whole_device_pixels_of_its_font() {
    // A `normal` line of `Hxg` in `DejaVu Sans`: its ascent and descent
    // each rounded half up to a whole device pixel (ADR 0057, Evidence 3).
    const AT_ONE: [u16; 65] = [
        576, 640, 640, 704, 704, 768, 832, 896, 896, 960, 960, 1024, 1024, 1024, 1152, 1152, 1216,
        1216, 1280, 1280, 1344, 1344, 1408, 1472, 1536, 1536, 1536, 1600, 1600, 1664, 1664, 1792,
        1792, 1856, 1856, 1920, 1920, 1984, 1984, 2048, 2112, 2112, 2176, 2176, 2240, 2240, 2304,
        2304, 2432, 2432, 2496, 2496, 2560, 2560, 2560, 2624, 2624, 2752, 2752, 2816, 2816, 2880,
        2880, 2944, 2944,
    ];
    const AT_TWO: [u16; 65] = [
        608, 640, 672, 704, 768, 768, 800, 832, 896, 928, 960, 992, 1056, 1088, 1120, 1152, 1216,
        1248, 1280, 1280, 1312, 1376, 1408, 1440, 1472, 1536, 1568, 1600, 1632, 1696, 1728, 1760,
        1792, 1824, 1856, 1888, 1920, 1952, 2016, 2048, 2080, 2112, 2176, 2208, 2240, 2272, 2336,
        2336, 2368, 2400, 2464, 2496, 2528, 2560, 2624, 2656, 2688, 2720, 2752, 2816, 2848, 2848,
        2880, 2944, 2976,
    ];
    let paragraphs = halves()
        .iter()
        .map(|size| format!("<p style=\"font-size:{size}px\">Hxg</p>"))
        .collect::<Vec<_>>()
        .concat();
    let html = format!(
        "<!doctype html><style>body{{margin:0}} p{{margin:0;font-family:DejaVu Sans}}</style>\
         <body>{paragraphs}"
    );
    for (scale, chrome) in [(1.0, AT_ONE), (2.0, AT_TWO)] {
        let boxes = boxes_of(&html, 300.0, scale);
        for (index, (size, height)) in halves().iter().zip(chrome).enumerate() {
            let ours = boxes
                .get(&format!("/html0/body1/p{index}"))
                .map(|units| units[3]);
            assert!(
                ours.is_some_and(|ours| same(ours, f32::from(height))),
                "{size} px at {scale}: {ours:?}, Chrome {height}"
            );
        }
    }
}

#[test]
fn the_leadings_top_half_is_floored_in_device_pixels() {
    // `Hxg` at 16 px under a set `line-height`, with an empty inline block
    // on the baseline: the baseline is the floored half-leading plus the
    // rounded ascent, each in device pixels (ADR 0057, Evidence 4), and the
    // line is its height on the grid.
    const BASELINE_AT_ONE: [u16; 45] = [
        896, 896, 896, 896, 896, 896, 896, 960, 960, 960, 960, 960, 960, 960, 1024, 1024, 1024,
        1024, 1024, 1024, 1088, 1088, 1088, 1088, 1088, 1088, 1088, 1152, 1152, 1152, 1152, 1152,
        1152, 1152, 1216, 1216, 1216, 1216, 1216, 1216, 1280, 1280, 1280, 1280, 1280,
    ];
    const BASELINE_AT_TWO: [u16; 45] = [
        896, 896, 896, 896, 928, 928, 928, 960, 960, 960, 992, 992, 992, 992, 1024, 1024, 1024,
        1056, 1056, 1056, 1088, 1088, 1088, 1088, 1120, 1120, 1120, 1152, 1152, 1152, 1184, 1184,
        1184, 1184, 1216, 1216, 1216, 1248, 1248, 1248, 1280, 1280, 1280, 1280, 1312,
    ];
    // A set line height is rounded onto the grid, which at two is 1/128 px:
    // `17.6px` is 1,126 sixty-fourths at one and 1,126.5 at two.
    const HEIGHT_AT_ONE: [f32; 45] = [
        1088.0, 1107.0, 1126.0, 1146.0, 1165.0, 1184.0, 1203.0, 1222.0, 1242.0, 1261.0, 1280.0,
        1299.0, 1318.0, 1338.0, 1357.0, 1376.0, 1395.0, 1414.0, 1434.0, 1453.0, 1472.0, 1491.0,
        1510.0, 1530.0, 1549.0, 1568.0, 1587.0, 1606.0, 1626.0, 1645.0, 1664.0, 1683.0, 1702.0,
        1722.0, 1741.0, 1760.0, 1779.0, 1798.0, 1818.0, 1837.0, 1856.0, 1875.0, 1894.0, 1914.0,
        1933.0,
    ];
    const HEIGHT_AT_TWO: [f32; 45] = [
        1088.0, 1107.0, 1126.5, 1145.5, 1165.0, 1184.0, 1203.0, 1222.5, 1241.5, 1261.0, 1280.0,
        1299.0, 1318.5, 1337.5, 1357.0, 1376.0, 1395.0, 1414.5, 1433.5, 1453.0, 1472.0, 1491.0,
        1510.5, 1529.5, 1549.0, 1568.0, 1587.0, 1606.5, 1625.5, 1645.0, 1664.0, 1683.0, 1702.5,
        1721.5, 1741.0, 1760.0, 1779.0, 1798.5, 1817.5, 1837.0, 1856.0, 1875.0, 1894.5, 1913.5,
        1933.0,
    ];
    let lines = line_heights()
        .iter()
        .map(|height| {
            format!("<div style=\"line-height:{height:.1}px\"><span>xg</span><i></i></div>")
        })
        .collect::<Vec<_>>()
        .concat();
    let html = format!(
        "<!doctype html><style>body{{margin:0}} div{{width:400px;font-size:16px;font-family:DejaVu Sans}} \
         i{{display:inline-block;width:1px;height:0}}</style><body>{lines}"
    );
    for (scale, baselines, heights) in [
        (1.0, BASELINE_AT_ONE, HEIGHT_AT_ONE),
        (2.0, BASELINE_AT_TWO, HEIGHT_AT_TWO),
    ] {
        let boxes = boxes_of(&html, 500.0, scale);
        for (index, line_height) in line_heights().iter().enumerate() {
            let line = boxes.get(&format!("/html0/body1/div{index}"));
            let mark = boxes.get(&format!("/html0/body1/div{index}/i1"));
            let baseline = line.zip(mark).map(|(line, mark)| mark[1] - line[1]);
            let (Some(wanted), Some(height)) = (baselines.get(index), heights.get(index)) else {
                continue;
            };
            assert!(
                baseline.is_some_and(|ours| same(ours, f32::from(*wanted))),
                "line-height {line_height} at {scale}: baseline {baseline:?}, Chrome {wanted}"
            );
            assert!(
                line.is_some_and(|line| same(line[3], *height)),
                "line-height {line_height} at {scale}: {line:?}, Chrome {height} tall"
            );
        }
    }
}

#[test]
fn a_length_is_truncated_onto_a_sixty_fourth_of_a_device_pixel() {
    // A width and a padding each of the length: `0.31px` is 19 units at one
    // and 19.5 at two, so the two together are 38 and 39 (ADR 0057,
    // Evidence 2).
    const LENGTHS: [&str; 12] = [
        "0.31", "0.1", "7.2", "9.6", "13.3", "19.92", "33.3333", "0.6", "100.3", "5.55", "2.77",
        "1.01",
    ];
    const AT_ONE: [u16; 12] = [
        38, 12, 920, 1228, 1702, 2548, 4266, 76, 12838, 710, 354, 128,
    ];
    const AT_TWO: [u16; 12] = [
        39, 12, 921, 1228, 1702, 2549, 4266, 76, 12838, 710, 354, 129,
    ];
    let blocks = LENGTHS
        .iter()
        .map(|length| {
            format!("<div style=\"width:{length}px;height:1px;padding-left:{length}px\"></div>")
        })
        .collect::<Vec<_>>()
        .concat();
    let html = format!("<!doctype html><style>body{{margin:0}}</style><body>{blocks}");
    for (scale, chrome) in [(1.0, AT_ONE), (2.0, AT_TWO)] {
        let boxes = boxes_of(&html, 500.0, scale);
        for (index, (length, width)) in LENGTHS.iter().zip(chrome).enumerate() {
            let ours = boxes
                .get(&format!("/html0/body1/div{index}"))
                .map(|units| units[2]);
            assert!(
                ours.is_some_and(|ours| same(ours, f32::from(width))),
                "{length}px at {scale}: {ours:?}, Chrome {width}"
            );
        }
    }
}

#[test]
fn a_face_is_set_at_its_size_cut_to_hundredths_of_a_device_pixel() {
    // A hundred `x` in `DejaVu Sans`, as wide as Chrome makes the run at
    // each scale (ADR 0057's open question, settled by item 560): `13.337px`
    // is 26.674 device pixels at two, set at 26.67, so 13.335 CSS pixels,
    // where cut in CSS pixels it would be 13.33 and 0.3 px narrower.
    // `9.123px` is left out: it is a unit off at one as well (item 527).
    const RUNS: [(&str, f32, f32); 9] = [
        ("13.337", 50488.0, 50506.5),
        ("17.777", 67304.0, 67323.0),
        ("21.4949", 81394.0, 81393.5),
        ("11.1155", 42080.0, 42098.5),
        ("15.0051", 56813.0, 56831.5),
        ("12.3456", 46738.0, 46757.0),
        ("19.9999", 75713.0, 75731.5),
        ("8.765", 33179.0, 33197.5),
        ("14.2857", 54086.0, 54104.5),
    ];
    let lines = RUNS
        .iter()
        .map(|(size, ..)| {
            format!(
                "<div><span style=\"display:inline-block;font-size:{size}px;font-family:DejaVu Sans\">{}</span></div>",
                "x".repeat(100)
            )
        })
        .collect::<Vec<_>>()
        .concat();
    let html = format!(
        "<!doctype html><style>body{{margin:0}} div{{white-space:nowrap}}</style><body>{lines}"
    );
    for scale in [1.0, 2.0] {
        let boxes = boxes_of(&html, 1600.0, scale);
        for (index, (size, at_one, at_two)) in RUNS.iter().enumerate() {
            let chrome = if scale > 1.0 { at_two } else { at_one };
            let ours = boxes
                .get(&format!("/html0/body1/div{index}/span0"))
                .map(|units| units[2]);
            assert!(
                ours.is_some_and(|ours| same(ours, *chrome)),
                "{size}px at {scale}: {ours:?}, Chrome {chrome}"
            );
        }
    }
}

/// Chrome's boxes beside a case at one scale, by path, or [`None`] when the
/// case has no such file.
fn chrome_boxes(directory: &Path, scale: u8) -> Option<BTreeMap<String, Units>> {
    let text = std::fs::read_to_string(directory.join(format!("chrome-at-{scale}.txt"))).ok()?;
    let mut found = BTreeMap::new();
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let mut words = line.split_whitespace();
        let path = words.next()?;
        let mut units = [0.0; 4];
        for unit in &mut units {
            *unit = words.next()?.parse().ok()?;
        }
        found.insert(path.to_owned(), units);
    }
    Some(found)
}

/// The case's boxes at one and at two, by path, or [`None`] for a page
/// whose script a renderer runs: a renderer is told no scale until item 563.
fn ours(case: &Case) -> Result<Option<[BTreeMap<String, Units>; 2]>, String> {
    let at_one = Rendering::of(case)?;
    if matches!(at_one, Rendering::Loaded(..)) {
        return Ok(None);
    }
    let (Some(document), Some(drawing)) = (at_one.document(), at_one.drawing()) else {
        return Err("drew nothing at one".to_owned());
    };
    let screen = Screen::new(Size::new(case.size.0, case.size.1)).at_scale(2.0);
    let at_two = render_at(
        &case.html,
        &case.css,
        screen,
        &corpus_fonts(),
        &case.linked,
        &case.resources,
    );
    Ok(Some([
        boxes_by_path(document, &drawing),
        boxes_by_path(&at_two.document, &at_two.drawing),
    ]))
}

/// The boxes Chrome places a sixty-fourth from this engine at two, for a
/// reason that is not the scale, each with the queue item that owns it.
///
/// `border-widths` at two shrinks its five flex items by 0.4 px each, which
/// at one it never does, and Chrome hands the spare 1/128 px to another item
/// than this engine does; the same shrink at one is a unit off too (item
/// 564). A box here that comes to match is said, so the list cannot outlive
/// its reason.
const NOT_THE_SCALE: [(&str, &str, u16); 2] = [
    ("border-widths", "/html0/body1/div0", 564),
    ("border-widths", "/html0/body1/div1", 564),
];

#[test]
fn every_box_placed_as_chrome_places_it_at_one_is_placed_as_chrome_places_it_at_two() {
    let mut compared = 0_usize;
    let mut cases = 0_usize;
    let mut loaded = 0_usize;
    let mut wrong = Vec::new();
    let mut excused = Vec::new();
    let Ok(entries) = std::fs::read_dir(cases_directory()) else {
        panic!("no corpus at {}", cases_directory().display());
    };
    let mut directories: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
    directories.sort();
    for directory in directories {
        let (Some(chrome_one), Some(chrome_two)) =
            (chrome_boxes(&directory, 1), chrome_boxes(&directory, 2))
        else {
            continue;
        };
        let Some(case) = Case::read(&directory) else {
            wrong.push(format!("{}: not a case", directory.display()));
            continue;
        };
        let [ours_one, ours_two] = match ours(&case) {
            Ok(Some(both)) => both,
            Ok(None) => {
                loaded += 1;
                continue;
            }
            Err(why) => {
                wrong.push(format!("{}: {why}", case.name));
                continue;
            }
        };
        cases += 1;
        for (path, chrome) in &chrome_one {
            if path.starts_with("/html0/head") {
                continue;
            }
            let (Some(one), Some(two), Some(chrome_at_two)) =
                (ours_one.get(path), ours_two.get(path), chrome_two.get(path))
            else {
                continue;
            };
            if !one.iter().zip(chrome).all(|(a, b)| same(*a, *b)) {
                continue;
            }
            compared += 1;
            let matches = two.iter().zip(chrome_at_two).all(|(a, b)| same(*a, *b));
            let excuse = NOT_THE_SCALE
                .iter()
                .find(|(name, at, _)| *name == case.name && at == path);
            match (matches, excuse) {
                (true, None) => {}
                (true, Some((_, _, item))) => wrong.push(format!(
                    "{} {path} now matches at two: take it off item {item}'s list",
                    case.name
                )),
                (false, Some(_)) => excused.push(path.clone()),
                (false, None) => wrong.push(format!(
                    "{} {path}: Chrome {chrome_at_two:?} at two, this engine {two:?}",
                    case.name
                )),
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert_eq!(
        cases, 52,
        "every case of markup Chrome was measured on at two"
    );
    assert_eq!(
        loaded, 21,
        "the pages a renderer loads, measured and owed to item 563"
    );
    assert!(
        compared >= 580,
        "{compared} boxes compared, where 580 were exact at one on 2026-10-13"
    );
    assert_eq!(excused.len(), NOT_THE_SCALE.len(), "{excused:?}");
}
