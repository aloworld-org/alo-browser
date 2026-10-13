/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `resolution`, `min-resolution` and `max-resolution` answer as Chrome 154
//! answers them, at a scale of one and of two (queue item 567, ADR 0057
//! § 3).
//!
//! `references/resolution.chrome.txt` is 3,032 answers of Chrome 154,
//! headless, started with `--force-device-scale-factor=1` and `=2` (never
//! the emulated scale of its developer tools), measured 2026-10-13, the
//! source not read: 88 queries across the units, cases, zero, negatives,
//! `infinite`, `calc()`, the range syntax and the prefixed ratio, then 1,428
//! of `dpcm` by hundredths from 37 to 38.59 and 74 to 76.99 and of `dppx`,
//! `x` and `dpi` around one and two by millionths, each as each feature.
//!
//! Every query this engine understands answers as Chrome's did. The ones it
//! refuses are named below, each with why; a refused query never matches,
//! so where Chrome said `false` there is nothing to name.

use alo_css::{ColorScheme, MediaContext, MediaQueryList};

/// The queries Chrome 154 matched at some scale and this engine refuses as
/// not understood, every one a form no frozen page has asked for:
/// `calc()` in a query, the boolean `(resolution)`, the range syntax (which
/// this engine refuses for `width` too), and the prefixed device pixel ratio
/// (stage 3's vendor prefixes, queue item 141).
const REFUSED: [&str; 9] = [
    "(resolution: calc(1dppx + 1dppx))",
    "(min-resolution: calc(1dppx + 1dppx))",
    "(max-resolution: calc(1dppx + 1dppx))",
    "(resolution)",
    "(resolution >= 2dppx)",
    "(resolution > 1dppx)",
    "(1dppx < resolution < 3dppx)",
    "(-webkit-min-device-pixel-ratio: 2)",
    "(-webkit-device-pixel-ratio: 2)",
];

/// Chrome's answers: the scale, whether it matched, and the query.
fn chrome() -> Vec<(f32, bool, String)> {
    let text = include_str!("references/resolution.chrome.txt");
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(3, ' ');
            let scale = parts.next()?.parse().ok()?;
            let matched = parts.next()? == "true";
            Some((scale, matched, parts.next()?.to_owned()))
        })
        .collect()
}

#[test]
fn every_query_this_engine_understands_answers_as_chrome_did() {
    let answers = chrome();
    assert_eq!(answers.len(), 3_032, "every answer measured is read");
    let mut understood = 0_usize;
    let mut wrong = Vec::new();
    let mut refused_where_chrome_matched = Vec::new();
    for (scale, matched, query) in &answers {
        let context = MediaContext::sized(800.0, 600.0, ColorScheme::Light).at_scale(*scale);
        let mut issues = Vec::new();
        let list = MediaQueryList::parse_text(query, &mut issues);
        let ours = list.matches(&context);
        if issues.is_empty() {
            understood += 1;
            if ours != *matched {
                wrong.push(format!(
                    "at {scale}: {query} is {ours}, Chrome said {matched}"
                ));
            }
        } else {
            assert!(!ours, "a refused query never matches: {query}");
            if *matched && !refused_where_chrome_matched.contains(query) {
                refused_where_chrome_matched.push(query.clone());
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    assert_eq!(
        understood, 2_984,
        "every answer but 24 queries at each scale: the 9 named below, a \
         `min-resolution` with no value, a number apart from its unit, \
         `min-device-pixel-ratio`, and for each feature a negative, a bare \
         number, `px`, `infinite` and `calc()`"
    );
    refused_where_chrome_matched.sort();
    let mut named = REFUSED.map(str::to_owned).to_vec();
    named.sort();
    assert_eq!(refused_where_chrome_matched, named);
}

#[test]
fn a_sheet_answers_as_its_queries_do() {
    let sheet = alo_css::parse_stylesheet(
        "@media (min-resolution: 2dppx) { p { color: red } }\
         @media (max-resolution: 1.5x) { p { color: blue } }",
    );
    assert!(sheet.issues().is_empty(), "{:?}", sheet.issues());
    let rules_at = |scale: f32| {
        sheet
            .style_rules_for(&MediaContext::default().at_scale(scale))
            .len()
    };
    assert_eq!(rules_at(1.0), 1, "the rule for a screen of 1.5 or less");
    assert_eq!(rules_at(2.0), 1, "the rule for a screen of two or more");
    assert_eq!(rules_at(1.75), 0, "neither");
}
