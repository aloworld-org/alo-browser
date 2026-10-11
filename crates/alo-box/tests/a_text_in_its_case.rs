/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `text-transform`, as the box tree holds it (ADR 0049).
//!
//! The box holds the text as drawn — collapsed, then put in its case, in
//! its language — so these assert on the text boxes themselves: whatever
//! measures, draws or reads them reads this.

use alo_box::{BoxTree, build};
use alo_css::{IssueKind, MediaContext, PseudoElement, parse_stylesheet};
use alo_dom::parse_document;
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};

fn tree_of(html: &str, css: &str) -> BoxTree {
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(css);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    build(&document, &styles)
}

/// Every text box's text, in tree order, leaving out the white space
/// between elements.
fn texts(tree: &BoxTree) -> Vec<String> {
    let Some(root) = tree.root() else {
        return Vec::new();
    };
    tree.descendants(root)
        .into_iter()
        .filter_map(|id| tree.get(id)?.text().map(str::to_owned))
        .filter(|text| !text.trim().is_empty())
        .collect()
}

#[test]
fn a_heading_and_a_badge_are_held_in_their_case() {
    let tree = tree_of(
        "<h2 class=up>My calendars</h2><span class=down>Can edit</span><p>As written</p>",
        ".up { text-transform: uppercase } .down { text-transform: lowercase }",
    );
    assert_eq!(texts(&tree), ["MY CALENDARS", "can edit", "As written"]);
}

#[test]
fn the_case_is_inherited_and_none_undoes_it() {
    let tree = tree_of(
        "<div class=up>outer <b>bold</b> <span class=off>kept</span></div>",
        ".up { text-transform: uppercase } .off { text-transform: none }",
    );
    assert_eq!(texts(&tree), ["OUTER ", "BOLD", "kept"]);
}

#[test]
fn white_space_is_collapsed_first_and_kept_where_it_is_preserved() {
    let tree = tree_of(
        "<p class=up>  two   spaces  </p><p class='up pre'>  two  spaces  </p>",
        ".up { text-transform: uppercase } .pre { white-space: pre }",
    );
    assert_eq!(texts(&tree), [" TWO SPACES ", "  TWO  SPACES  "]);
}

#[test]
fn the_nearest_lang_decides_the_mapping() {
    let tree = tree_of(
        "<div class=up lang=tr><p>istanbul</p><p lang=en>istanbul</p>\
         <p lang=''>istanbul</p><p lang=tr-CY>istanbul</p></div>\
         <p class=up lang=az-Latn>istanbul</p><p class=up lang=el>Όσο άλλο</p>\
         <p class=up>straße</p>",
        ".up { text-transform: uppercase }",
    );
    assert_eq!(
        texts(&tree),
        [
            "İSTANBUL",
            "ISTANBUL",
            "ISTANBUL",
            "İSTANBUL",
            "İSTANBUL",
            "ΟΣΟ ΑΛΛΟ",
            "STRASSE"
        ]
    );
}

#[test]
fn a_fields_value_its_hint_and_a_broken_pictures_alt_are_drawn_in_case() {
    let tree = tree_of(
        "<input class=up value='typed value'><input class=up placeholder='hint inherits'>\
         <input class=own placeholder='hint own rule'><img class=up alt='alt text'>",
        ".up { text-transform: uppercase } .own::placeholder { text-transform: uppercase }",
    );
    assert_eq!(
        texts(&tree),
        ["TYPED VALUE", "HINT INHERITS", "HINT OWN RULE", "ALT TEXT"]
    );
}

#[test]
fn a_marker_keeps_its_case_unless_it_is_given_one() {
    // No marker this engine draws has a letter in it yet (item 406), so the
    // case is asserted where it is decided, in the marker's own style; the
    // marker's text is what that style makes it.
    let document = parse_document(
        "<ol class=up><li>item</li></ol><ol class='up own'><li>marker rule</li></ol>",
    );
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(
        ".up { text-transform: uppercase } .own li::marker { text-transform: lowercase }",
    );
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let tree = build(&document, &styles);
    let cases: Vec<Option<&str>> = document
        .descendants(document.root())
        .filter(|id| {
            document
                .element(*id)
                .is_some_and(|element| element.name.is_html("li"))
        })
        .map(|id| {
            styles
                .pseudo(id, PseudoElement::Marker)
                .and_then(|style| style.get("text-transform"))
        })
        .collect();
    assert_eq!(cases, [Some("none"), Some("lowercase")]);
    let markers: Vec<String> = tree
        .ids()
        .filter(|id| tree.is_marker(*id))
        .filter_map(|id| tree.get(id)?.text().map(str::to_owned))
        .collect();
    assert_eq!(markers, ["1. ", "1. "]);
    assert_eq!(texts(&tree), ["1. ", "ITEM", "1. ", "MARKER RULE"]);
}

#[test]
fn an_unbuilt_value_draws_as_written_and_is_recorded_once() {
    let tree = tree_of(
        "<div class=cap>ab.cd <b>can't</b> <i>o'neil</i></div><p class=wide>abc</p>",
        ".cap { text-transform: capitalize } .wide { text-transform: full-width }",
    );
    assert_eq!(texts(&tree), ["ab.cd ", "can't", "o'neil", "abc"]);
    let recorded: Vec<&str> = tree
        .issues()
        .iter()
        .filter(|issue| issue.kind == IssueKind::UnsupportedValue)
        .map(|issue| issue.source.as_str())
        .filter(|source| source.starts_with("text-transform"))
        .collect();
    assert_eq!(recorded.len(), 2, "{recorded:?}");
    assert!(recorded[0].starts_with("text-transform: capitalize (on "));
    assert!(recorded[1].starts_with("text-transform: full-width (on "));
}

#[test]
fn hostile_text_and_languages_are_mapped_without_a_panic() {
    let doubling = "ß".repeat(100_000);
    let lang = "x".repeat(10_000);
    let html =
        format!("<p class=up lang='{lang}'>{doubling}</p><p class=up lang='&#0;&#1;&#x7f;'>i</p>");
    let tree = tree_of(&html, ".up { text-transform: uppercase }");
    let texts = texts(&tree);
    assert_eq!(texts.len(), 2);
    assert_eq!(texts[0].len(), 200_000);
    assert_eq!(texts[1], "I");
}
