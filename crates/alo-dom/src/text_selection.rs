/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A text field's selection: which of its characters are selected, or
//! where its caret stands when none are (queue item 861).
//!
//! HTML keeps a text control's selection as the control's own state —
//! `selectionStart` and `selectionEnd` — never as an attribute a page's
//! markup could write, and so does this: the document holds it beside its
//! nodes, as it holds what is hovered ([`crate::hover`]), and the box tree
//! reads it into the field's states, so an agent reads the selection of the
//! very field it reads the value of (ADR 0002).
//!
//! # Counted in characters
//!
//! A selection is a start and an end in **characters** — Unicode scalar
//! values — of the field's text, the start never after the end: `(3, 3)`
//! is the caret after the third character with nothing selected. HTML
//! counts UTF-16 code units, and Chrome's `selectionStart` would say 4 for
//! the caret after `😀a`; an agent counting a string's characters would say
//! 2, and is told 2.
//!
//! Only alo's own documents set one, today: its fields are drawn by its
//! renderers from what the browser process holds (`alo-renderer`'s
//! `field_line`). Read back, a selection is clamped to the text the field
//! holds at that moment, so text changed under it never yields a range
//! past its end.

use crate::document::Document;
use crate::node::NodeId;

/// Whether `node` holds text a person selects in: an `<input>` or a
/// `<textarea>`.
fn takes_text(document: &Document, node: NodeId) -> bool {
    document
        .element(node)
        .is_some_and(|element| element.name.is_html("input") || element.name.is_html("textarea"))
}

/// How many characters the field `node` holds.
fn length(document: &Document, node: NodeId) -> usize {
    let Some(element) = document.element(node) else {
        return 0;
    };
    if crate::field::is_held_as_content(element) {
        document.text_content(node).chars().count()
    } else {
        element
            .attr(crate::field::TEXT)
            .map_or(0, |text| text.chars().count())
    }
}

/// Set the selection of the field `node` to run from `start` to `end`, in
/// characters, either way round: whether `node` is a field that takes one.
pub fn set(document: &mut Document, node: NodeId, start: usize, end: usize) -> bool {
    if !takes_text(document, node) {
        return false;
    }
    let range = (start.min(end), start.max(end));
    document.selections.retain(|(held, _)| *held != node);
    document.selections.push((node, range));
    true
}

/// The selection of the field `node`, if one was set: its start and end
/// in characters, clamped to the text it holds now.
pub fn of(document: &Document, node: NodeId) -> Option<(usize, usize)> {
    if !takes_text(document, node) {
        return None;
    }
    let (start, end) = document
        .selections
        .iter()
        .find(|(held, _)| *held == node)
        .map(|(_, range)| *range)?;
    let most = length(document, node);
    Some((start.min(most), end.min(most)))
}

/// Forget the selection of the field `node`: whether it had one.
pub fn clear(document: &mut Document, node: NodeId) -> bool {
    let before = document.selections.len();
    document.selections.retain(|(held, _)| *held != node);
    document.selections.len() != before
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_document;

    fn by_id(document: &Document, id: &str) -> NodeId {
        document
            .descendants(document.root())
            .find(|&node| document.attribute_by_name(node, "id") == Some(id))
            .unwrap_or_else(|| document.root())
    }

    #[test]
    fn a_fields_selection_is_kept_either_way_round_and_clamped_to_its_text() {
        let mut document =
            parse_document("<input id=f value=kampala><textarea id=t>ab</textarea><p id=p>x</p>");
        let field = by_id(&document, "f");
        assert_eq!(of(&document, field), None, "none set");
        assert!(set(&mut document, field, 4, 1));
        assert_eq!(of(&document, field), Some((1, 4)));
        assert!(set(&mut document, field, 2, 99));
        assert_eq!(of(&document, field), Some((2, 7)), "clamped to seven");
        let area = by_id(&document, "t");
        assert!(set(&mut document, area, 1, 1));
        assert_eq!(of(&document, area), Some((1, 1)));
        let paragraph = by_id(&document, "p");
        assert!(!set(&mut document, paragraph, 0, 1), "not a field");
        assert_eq!(of(&document, paragraph), None);
        assert!(clear(&mut document, field));
        assert!(!clear(&mut document, field));
        assert_eq!(of(&document, field), None);
    }

    #[test]
    fn characters_are_counted_not_bytes_or_code_units() {
        let mut document = parse_document("<input id=f value=\"😀a€\">");
        let field = by_id(&document, "f");
        assert!(set(&mut document, field, 2, 9));
        assert_eq!(of(&document, field), Some((2, 3)));
    }
}
