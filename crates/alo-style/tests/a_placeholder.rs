/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A field's `::placeholder`, styled as its field's child (ADR 0043 § 1,
//! queue item 389).
//!
//! The first pseudo-element this engine styles, so what is asserted here is
//! the model every later one inherits: its rules are the ones naming it, it
//! inherits from its element — custom properties included — it is computed
//! only for an element that makes one, and of what applies to it only
//! `color` is read, the rest said as issues.

use alo_css::{IssueKind, MediaContext, PseudoElement, StyleIssue, parse_stylesheet};
use alo_dom::{Document, NodeId, parse_document};
use alo_style::{Origin, SourcedSheet, StyleTree, USER_AGENT_STYLE_SHEET, resolve};

/// The document and its styles, the engine's sheet first.
fn styled(html: &str, css: &str) -> (Document, StyleTree, Vec<StyleIssue>) {
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(css);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    (document, styles, author.issues().to_vec())
}

/// The element whose `id` attribute is `wanted`.
fn by_id(document: &Document, wanted: &str) -> Option<NodeId> {
    document
        .descendants(document.root())
        .find(|id| document.element(*id).and_then(|e| e.attr("id")) == Some(wanted))
}

/// The colour of the placeholder of the element `wanted`, as eight-bit
/// channels, or [`None`] when it makes none.
fn hint_colour(document: &Document, styles: &StyleTree, wanted: &str) -> Option<(u8, u8, u8, u8)> {
    let id = by_id(document, wanted)?;
    let style = styles.pseudo(id, PseudoElement::Placeholder)?;
    Some(style.color("color")?.to_rgba8())
}

#[test]
fn the_hint_is_grey_unless_a_page_says_otherwise() {
    let html = "<input id=plain placeholder=a><input id=coloured class=c placeholder=b>";
    let (document, styles, _) = styled(html, ".c::placeholder { color: #102a43 }");
    assert_eq!(
        hint_colour(&document, &styles, "plain"),
        Some((0x75, 0x75, 0x75, 255))
    );
    assert_eq!(
        hint_colour(&document, &styles, "coloured"),
        Some((0x10, 0x2a, 0x43, 255)),
        "an author's colour beats the engine's #757575"
    );
}

#[test]
fn the_hint_inherits_its_fields_custom_properties_and_font() {
    // alo's `Input`: Tailwind's `placeholder:text-tertiary`, against a token
    // declared on the root and one declared on the field itself.
    let html = "<div class=page><input id=f class=input placeholder=you@company.eu></div>";
    let css = ":root { --text-tertiary: #7a6f62 } \
               .input { --own: #010203; font-size: 14px; color: #102a43 } \
               .input::placeholder { color: var(--text-tertiary) } \
               .page .input::placeholder { border-color: var(--own) }";
    let (document, styles, _) = styled(html, css);
    assert_eq!(
        hint_colour(&document, &styles, "f"),
        Some((0x7a, 0x6f, 0x62, 255))
    );
    let Some(field) = by_id(&document, "f") else {
        panic!("the field");
    };
    let (Some(own), Some(hint)) = (
        styles.get(field),
        styles.pseudo(field, PseudoElement::Placeholder),
    ) else {
        panic!("the field and its hint are styled");
    };
    assert_eq!(hint.variables().get("--own"), Some("#010203"));
    assert!(
        (hint.font_size() - 14.0).abs() < 1e-6,
        "the field's font, inherited"
    );
    assert!((hint.font_size() - own.font_size()).abs() < 1e-6);
    // The field's own colour is its own: a pseudo-element's rules never
    // reach the element.
    assert_eq!(
        own.color("color").map(alo_value::Rgba::to_rgba8),
        Some((0x10, 0x2a, 0x43, 255))
    );
}

#[test]
fn only_an_element_that_makes_one_has_one() {
    let html = "<input id=none><input id=empty placeholder=''><input id=held placeholder=a value=x>\
         <input id=day type=date placeholder=a><div id=div placeholder=a></div>\
         <textarea id=area placeholder=a></textarea><input id=shown type=search placeholder=a>";
    let (document, styles, _) = styled(html, "");
    for wanted in ["none", "empty", "held", "day", "div"] {
        assert_eq!(hint_colour(&document, &styles, wanted), None, "{wanted}");
    }
    for wanted in ["area", "shown"] {
        assert!(
            hint_colour(&document, &styles, wanted).is_some(),
            "{wanted}"
        );
    }
}

#[test]
fn of_what_applies_only_colour_is_read_and_the_rest_is_said() {
    let html = "<input id=f class=f placeholder=a>";
    let css = ".f { font-style: normal } \
               .f::placeholder { color: red; font-style: italic; width: 9px; \
               letter-spacing: 2px }";
    let (document, styles, _) = styled(html, css);
    let Some(hint) =
        by_id(&document, "f").and_then(|f| styles.pseudo(f, PseudoElement::Placeholder))
    else {
        panic!("the hint is styled");
    };
    assert_eq!(
        hint.get("font-style"),
        Some("normal"),
        "the field's, inherited"
    );
    assert_eq!(hint.get("letter-spacing"), None);
    assert_eq!(hint.get("width"), None);
    assert_eq!(
        hint.color("color").map(alo_value::Rgba::to_rgba8),
        Some((255, 0, 0, 255))
    );
    let said: Vec<&str> = styles
        .issues()
        .iter()
        .filter(|issue| issue.kind == IssueKind::PropertyNotReadOnPseudoElement)
        .map(|issue| issue.source.as_str())
        .collect();
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(
        said.iter()
            .any(|source| source.starts_with("font-style: italic on ::placeholder"))
    );
    assert!(
        said.iter()
            .any(|source| source.starts_with("letter-spacing: 2px on ::placeholder"))
    );
    assert!(
        !said.iter().any(|source| source.contains("width")),
        "a property that does not apply to it is ignored, as CSS says"
    );
}

#[test]
fn a_pseudo_element_this_engine_does_not_make_is_still_recorded() {
    let html = "<input id=f class=f placeholder=a><p id=p class=f>x</p>";
    let (document, styles, issues) = styled(
        html,
        ".f::before { color: red } .f::placeholder { color: blue }",
    );
    let refused: Vec<&StyleIssue> = issues
        .iter()
        .filter(|issue| issue.kind == IssueKind::PseudoElementNotProduced)
        .collect();
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(refused[0].source.contains("::before"));
    // And it styles nothing: neither the element nor its placeholder.
    let Some(paragraph) = by_id(&document, "p").and_then(|p| styles.get(p)) else {
        panic!("the paragraph is styled");
    };
    assert_eq!(
        paragraph.color("color").map(alo_value::Rgba::to_rgba8),
        Some((0, 0, 0, 255))
    );
    assert_eq!(hint_colour(&document, &styles, "f"), Some((0, 0, 255, 255)));
}
