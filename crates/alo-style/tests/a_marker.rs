/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A list item's `::marker`, styled as its item's child and made by the
//! item's own style (ADR 0044 §§ 1–3, queue item 401).
//!
//! What is asserted: a marker exists for exactly the items whose computed
//! style makes one, it inherits from its item, an author's `::marker {
//! color }` beats the item's, its font is read and the rest of what applies
//! to it is said, and its text is the counter style's representation of the
//! item's ordinal value.

use alo_css::{IssueKind, MediaContext, PseudoElement, StyleIssue, parse_stylesheet};
use alo_dom::{Document, NodeId, parse_document};
use alo_style::{Origin, SourcedSheet, StyleTree, USER_AGENT_STYLE_SHEET, resolve};

/// The document and its styles, the engine's sheet first.
fn styled(html: &str, css: &str) -> (Document, StyleTree) {
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(css);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    (document, styles)
}

/// The element whose `id` attribute is `wanted`.
fn by_id(document: &Document, wanted: &str) -> Option<NodeId> {
    document
        .descendants(document.root())
        .find(|id| document.element(*id).and_then(|e| e.attr("id")) == Some(wanted))
}

/// What the marker of the element `wanted` says, if it draws one.
fn said(document: &Document, styles: &StyleTree, wanted: &str) -> Option<String> {
    let id = by_id(document, wanted)?;
    styles.marker_text(id).map(str::to_owned)
}

/// The issues the style tree raised of one kind, as text.
fn issues_of(styles: &StyleTree, kind: IssueKind) -> Vec<String> {
    styles
        .issues()
        .iter()
        .filter(|issue| issue.kind == kind)
        .map(|issue: &StyleIssue| issue.source.clone())
        .collect()
}

#[test]
fn the_rich_text_lists_say_a_disc_a_circle_and_their_numbers() {
    // alo Sites' rich text, as frozen in `alo-sites-rich-text`.
    let html = "<ul><li id=a>Choose<li id=b>Make<ul><li id=c>Keep</ul></ul>\
                <ol start=3><li id=d>Draft<li id=e>Review</ol>\
                <ul class=check-list><li id=f><s>Publish</s></ul>";
    let (document, styles) = styled(html, "");
    for (id, text) in [
        ("a", "• "),
        ("b", "• "),
        ("c", "◦ "),
        ("d", "3. "),
        ("e", "4. "),
        ("f", "• "),
    ] {
        assert_eq!(said(&document, &styles, id).as_deref(), Some(text), "{id}");
    }
}

#[test]
fn a_marker_is_made_by_the_items_style_and_list_style_none_takes_it_off() {
    let html = "<ul class=nav><li id=a>a</ul><ul><li id=b class=plain>b</ul>\
                <p id=c class=item>c</p><ul><li id=d>d</ul>";
    let css = ".nav { list-style: none } .plain { display: block } \
               .item { display: list-item } #d { list-style: square }";
    let (document, styles) = styled(html, css);
    let makes = |wanted: &str| {
        by_id(&document, wanted)
            .and_then(|id| styles.pseudo(id, PseudoElement::Marker))
            .is_some()
    };
    assert!(!makes("a"), "`list-style: none` on the list, inherited");
    assert!(!makes("b"), "not a list item");
    assert!(makes("c"), "any element made a list item");
    assert!(makes("d"));
    assert_eq!(said(&document, &styles, "c").as_deref(), Some("• "));
    assert_eq!(said(&document, &styles, "d").as_deref(), Some("▪ "));
    assert_eq!(said(&document, &styles, "a"), None);
}

#[test]
fn an_authors_marker_colour_beats_the_items_and_its_font_is_read() {
    let html = "<ol class=steps><li id=a>a<li id=b class=quiet>b</ol>";
    let css = ":root { --accent: #b4532a } .steps { color: #102a43; font-size: 20px } \
               .steps li::marker { color: var(--accent); font-weight: bold; content: '*' } \
               .quiet::marker { width: 40px }";
    let (document, styles) = styled(html, css);
    let Some(a) = by_id(&document, "a") else {
        panic!("the first item");
    };
    let (Some(item), Some(marker)) = (styles.get(a), styles.pseudo(a, PseudoElement::Marker))
    else {
        panic!("the item and its marker are styled");
    };
    assert_eq!(
        marker.color("color").map(alo_value::Rgba::to_rgba8),
        Some((0xb4, 0x53, 0x2a, 255)),
        "the author's `::marker` colour, read through the item's variable"
    );
    assert_eq!(
        item.color("color").map(alo_value::Rgba::to_rgba8),
        Some((0x10, 0x2a, 0x43, 255)),
        "and the item keeps its own"
    );
    assert_eq!(marker.font_weight(), 700);
    assert!(
        (marker.font_size() - 20.0).abs() < f32::EPSILON,
        "inherited"
    );
    // `content` applies and is not read; `width` does not apply at all.
    let unread = issues_of(&styles, IssueKind::PropertyNotReadOnPseudoElement);
    assert!(
        unread.iter().any(|issue| issue.starts_with("content")),
        "{unread:?}"
    );
    assert!(!unread.iter().any(|issue| issue.starts_with("width")));
    assert_eq!(said(&document, &styles, "a").as_deref(), Some("1. "));
}

#[test]
fn an_unbuilt_style_and_inside_draw_nothing_and_are_said_once() {
    let html = "<ol class=roman><li id=a>a<li id=b>b<li id=c>c</ol>\
                <ul class=inside><li id=d>d</ul>";
    let css = ".roman { list-style-type: lower-roman } .inside { list-style-position: inside }";
    let (document, styles) = styled(html, css);
    for id in ["a", "b", "c", "d"] {
        assert_eq!(said(&document, &styles, id), None, "{id}");
    }
    let refused = issues_of(&styles, IssueKind::UnsupportedValue);
    assert_eq!(
        refused
            .iter()
            .filter(|issue| issue.contains("lower-roman"))
            .count(),
        1,
        "{refused:?}"
    );
    assert!(refused.iter().any(|issue| issue.contains("inside")));
}
