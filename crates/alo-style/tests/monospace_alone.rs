/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `monospace` alone is thirteen pixels, and a font size remembers where it
//! came from (ADR 0045, queue item 415).
//!
//! What is asserted: every row of the ADR's § 4 table, measured in Chrome
//! 154, with the engine's sheet in front as a page gets it; § 1's origins,
//! one unit at a time, a math function and inheriting; § 2's test against
//! each spelling measured, and one written through `var()`; § 3's two
//! mediums with a keyword in each.

use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};

/// The computed font size of the element whose `id` is `wanted`, with the
/// engine's sheet first, in a 1280-pixel window.
fn size_of(html: &str, css: &str, wanted: &str) -> Option<f32> {
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(css);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let id = document
        .descendants(document.root())
        .find(|id| document.element(*id).and_then(|e| e.attr("id")) == Some(wanted))?;
    Some(styles.get(id)?.font_size())
}

fn assert_size(html: &str, css: &str, wanted: &str, expected: f32) {
    let got = size_of(html, css, wanted);
    assert!(
        got.is_some_and(|got| (got - expected).abs() < 0.001),
        "{html} with {css:?}: #{wanted} is {got:?}, not {expected}",
    );
}

#[test]
fn code_and_pre_under_the_bodys_medium_are_thirteen() {
    let html = "<body><p id=p>a <code id=c>b</code></p><pre id=pre>c</pre>\
                <kbd id=k>d</kbd><samp id=s>e</samp></body>";
    for wanted in ["c", "pre", "k", "s"] {
        assert_size(html, "", wanted, 13.0);
    }
    assert_size(html, "", "p", 16.0);
}

#[test]
fn em_a_percentage_and_medium_on_code_are_thirteen() {
    for css in [
        "code { font-size: 1em }",
        "code { font-size: 100% }",
        "code { font-size: medium }",
    ] {
        assert_size("<p><code id=c>x</code></p>", css, "c", 13.0);
    }
}

#[test]
fn smaller_on_code_steps_from_the_parent_and_is_then_carried_across() {
    // 16 / 1.2, then × 13/16.
    assert_size(
        "<p><code id=c>x</code></p>",
        "code { font-size: smaller }",
        "c",
        16.0 / 1.2 * 13.0 / 16.0,
    );
}

#[test]
fn code_under_a_relative_parent_is_scaled() {
    for css in ["p { font-size: 1.25em }", "p { font-size: 125% }"] {
        assert_size("<p><code id=c>x</code></p>", css, "c", 16.25);
    }
}

#[test]
fn a_keyword_parent_is_read_again_and_one_em_of_it_is_scaled() {
    // Chrome draws 16 and 14.625: its `large` is 18 where CSS's ratio is
    // 19.2, which is queue item 416. The rule is the same.
    assert_size(
        "<p><code id=c>x</code></p>",
        "p { font-size: large }",
        "c",
        15.6,
    );
    assert_size(
        "<p><span id=s>x</span></p>",
        "p { font-size: large } span { font-family: monospace; font-size: 1em }",
        "s",
        19.2 * 13.0 / 16.0,
    );
}

#[test]
fn code_under_a_fixed_parent_keeps_its_size() {
    for (css, expected) in [
        ("p { font-size: 20px }", 20.0),
        ("p { font-size: 2rem }", 32.0),
        ("p { font-size: 3vw }", 38.4),
    ] {
        assert_size("<p><code id=c>x</code></p>", css, "c", expected);
    }
}

#[test]
fn a_heading_inside_monospace_is_two_of_its_em_and_out_again_is_scaled_back() {
    let html = "<div id=d><h1 id=h>x</h1><span id=s>y</span></div>";
    let css = "div { font-family: monospace } \
               span { font-size: 2em; font-family: sans-serif }";
    assert_size(html, css, "d", 13.0);
    assert_size(html, css, "h", 26.0);
    assert_size(html, css, "s", 32.0);
}

#[test]
fn coming_out_of_pre_reads_the_keyword_again_and_a_fixed_size_stays() {
    assert_size(
        "<pre><span id=s>x</span></pre>",
        "span { font-family: sans-serif }",
        "s",
        16.0,
    );
    assert_size(
        "<div><span id=s>x</span></div>",
        "div { font-family: monospace; font-size: 14px } \
         span { font-family: sans-serif }",
        "s",
        14.0,
    );
}

#[test]
fn bold_inside_pre_stays_on_the_same_side() {
    assert_size("<pre><b id=b>x</b></pre>", "", "b", 13.0);
}

#[test]
fn a_monospace_root_is_thirteen_and_so_is_its_rem() {
    let html = "<html id=r><body><p id=p>x</p><div id=d>y</div></body></html>";
    let css = "html { font-family: monospace } div { font-size: 1rem }";
    assert_size(html, css, "r", 13.0);
    assert_size(html, css, "p", 13.0);
    assert_size(html, css, "d", 13.0);
}

#[test]
fn ch_on_code_is_of_the_parents_face_and_then_scaled() {
    // No faces here, so a `ch` is half an em: 2ch of the body's 16 is 16.
    let html = "<p><code id=c>x</code><span id=s>y</span></p>";
    let css = "code, span { font-size: 2ch }";
    assert_size(html, css, "c", 13.0);
    assert_size(html, css, "s", 16.0);
}

#[test]
fn every_unit_of_the_parents_font_is_relative() {
    // Under the body's medium, then carried into `monospace` × 13/16.
    // `lh` is the parent's `normal` line, 1.2 em.
    for (size, reached) in [
        ("1.5em", 24.0),
        ("2ex", 16.0),
        ("2ch", 16.0),
        ("1lh", 19.2),
        ("150%", 24.0),
        ("larger", 19.2),
    ] {
        let css = format!("code {{ font-size: {size} }}");
        assert_size(
            "<p><code id=c>x</code></p>",
            &css,
            "c",
            reached * 13.0 / 16.0,
        );
    }
}

#[test]
fn every_absolute_root_and_window_unit_is_fixed() {
    for (size, px) in [
        ("13.76px", 13.76),
        ("0.25in", 24.0),
        ("1cm", 96.0 / 2.54),
        ("5mm", 96.0 / 25.4 * 5.0),
        ("40q", 96.0 / 25.4 * 10.0),
        ("12pt", 16.0),
        ("1pc", 16.0),
        ("0.86rem", 13.76),
        ("1rlh", 19.2),
        ("1vw", 12.8),
        ("2vh", 16.0),
        ("1vmin", 8.0),
        ("1vmax", 12.8),
    ] {
        let css = format!("code {{ font-size: {size} }}");
        assert_size("<p><code id=c>x</code></p>", &css, "c", px);
    }
}

#[test]
fn a_math_function_is_fixed_whatever_it_holds() {
    for (size, px) in [
        ("calc(1em + 0px)", 16.0),
        ("calc(1em)", 16.0),
        ("min(1em, 20px)", 16.0),
        ("max(1em, 10px)", 16.0),
        ("clamp(10px, 1em, 20px)", 16.0),
    ] {
        let css = format!("code {{ font-size: {size} }}");
        assert_size("<p><code id=c>x</code></p>", &css, "c", px);
    }
    // And a parent that used one is fixed, so code under it keeps its size.
    assert_size(
        "<p><code id=c>x</code></p>",
        "p { font-size: max(1em, 10px) }",
        "c",
        16.0,
    );
}

#[test]
fn a_relative_size_of_a_fixed_parent_is_fixed_and_inheriting_keeps_that() {
    // The span is 1.25em of 16px: fixed, so neither it nor what inherits it
    // is scaled into the code.
    let html = "<div><span><code id=c>x</code></span></div>";
    assert_size(
        html,
        "div { font-size: 16px } span { font-size: 1.25em }",
        "c",
        20.0,
    );
    // And one level further: a relative parent, inherited through an
    // element that says nothing, is still relative.
    assert_size(html, "span { font-size: 1.25em }", "c", 16.25);
}

#[test]
fn only_monospace_alone_is_monospace_sized() {
    assert_size(
        "<p><span id=s>x</span></p>",
        "span { font-family: MONOSPACE }",
        "s",
        13.0,
    );
    for family in [
        "monospace, monospace",
        "\"Courier New\", monospace",
        "ui-monospace",
        "\"monospace\"",
    ] {
        let css = format!("code {{ font-family: {family} }}");
        assert_size("<p><code id=c>x</code></p>", &css, "c", 16.0);
    }
    // The test is on the computed value, after `var()`.
    assert_size(
        "<p><span id=s>x</span></p>",
        ":root { --code: monospace } span { font-family: var(--code) }",
        "s",
        13.0,
    );
}

#[test]
fn a_keyword_is_its_ratio_of_each_familys_medium() {
    for (keyword, sans, mono) in [
        ("x-small", 12.0, 9.75),
        ("large", 19.2, 15.6),
        ("xx-large", 32.0, 26.0),
    ] {
        let css = format!("span {{ font-size: {keyword} }} code {{ font-size: {keyword} }}");
        let html = "<p><span id=s>x</span><code id=c>y</code></p>";
        assert_size(html, &css, "s", sans);
        assert_size(html, &css, "c", mono);
    }
}

#[test]
fn font_size_inherit_is_inheriting_and_keeps_how_the_size_was_reached() {
    // § 1: inheriting keeps the origin, so an explicit `inherit` is the
    // body's `medium`, read again, rather than a fixed 16px.
    for keyword in ["inherit", "unset"] {
        let css = format!("code {{ font-size: {keyword} }}");
        assert_size("<p><code id=c>x</code></p>", &css, "c", 13.0);
    }
}
