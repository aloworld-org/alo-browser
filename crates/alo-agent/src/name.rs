/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a thing is called.
//!
//! An agent asks for "the Save button" and a screen reader says "Save,
//! button". Both need the same answer, and ADR 0002 is explicit that they get
//! it from the same place: *"A screen reader and an agent want the identical
//! facts, and building two would guarantee one is wrong."*
//!
//! # The order, and why it is an order
//!
//! ARIA prescribes it, and every step exists because the one before it can be
//! absent:
//!
//! 1. **`aria-labelledby`** — the author pointing at the text that names this.
//!    Pointing is more reliable than repeating, and it is what stays right
//!    when the text changes.
//! 2. **`aria-label`** — the author saying it outright.
//! 3. **The element's own** — an `<img>`'s `alt`, a `<label>` pointing at a
//!    field or wrapped around it.
//! 4. **Its content** — but only for the roles that take their name from what
//!    is inside them. A button says what is written on it; a paragraph does
//!    not have a name at all, and giving it one would fill an agent's view of
//!    the page with names nobody wrote.
//! 5. **`title`** — last, because it is a tooltip rather than a name.
//!
//! # Where this could not have been
//!
//! `alo-box` computes the *declared* name when a box is made — the attributes.
//! The rest needs the finished tree: step 4 walks a box's descendants, and step
//! 3 looks for a `<label>` somewhere else in the document. That is why the
//! algorithm lives here, at the point where there is a tree to walk, and why
//! `alo-box` says so where it stops.

use alo_box::{BoxId, BoxTree, KnownRole, Role};
use alo_dom::{Document, NodeId};

/// The roles that take their name from what is inside them.
///
/// A button is named by what is written on it; a paragraph has no name at all.
/// The list is ARIA's "name from content", trimmed to the roles this engine
/// has — and it is a list rather than a rule because ARIA makes it one.
pub(crate) fn names_itself_from_content(role: &Role) -> bool {
    matches!(
        role,
        Role::Known(
            KnownRole::Button
                | KnownRole::Link
                | KnownRole::Heading
                | KnownRole::ListItem
                | KnownRole::Option
                | KnownRole::Cell
                | KnownRole::ColumnHeader
                | KnownRole::RowHeader
                | KnownRole::Row
                | KnownRole::Tab
                | KnownRole::MenuItem
                | KnownRole::CheckBox
                | KnownRole::Radio
                | KnownRole::Switch
                | KnownRole::Summary
        )
    )
}

/// What a box is called.
///
/// [`None`] for a box with no name — which is a real answer and a common one.
/// A paragraph is not called anything, and saying so is better than inventing
/// a name out of its first sentence.
pub fn accessible_name(document: &Document, boxes: &BoxTree, id: BoxId) -> Option<String> {
    let node = boxes.get(id)?;
    let source = node.kind.node();

    // 1 and 2: what the author declared, which `alo-box` already worked out.
    if let Some(declared) = &node.semantics.label
        && !is_title_only(document, source)
    {
        return Some(declared.clone());
    }

    // 3: the element's own, which for a form control means a `<label>`.
    if let Some(source) = source
        && let Some(from_label) = label_for(document, source)
    {
        return normalise(&from_label);
    }

    // 4: what is inside it, for the roles that are named that way.
    if names_itself_from_content(&node.semantics.role) {
        let text = text_of(boxes, id);
        if let Some(name) = normalise(&text) {
            return Some(name);
        }
    }

    // 5: a `title`, last, because it is a tooltip rather than a name.
    source
        .and_then(|source| document.element(source))
        .and_then(|element| element.attr("title"))
        .and_then(normalise)
}

/// Whether the declared name came only from a `title`, which is the one
/// attribute that must wait until after the content.
fn is_title_only(document: &Document, source: Option<NodeId>) -> bool {
    let Some(element) = source.and_then(|source| document.element(source)) else {
        return false;
    };
    let has_stronger = ["aria-labelledby", "aria-label", "alt"].iter().any(|name| {
        element
            .attr(name)
            .is_some_and(|value| !value.trim().is_empty())
    });
    !has_stronger && element.attr("title").is_some()
}

/// The text of a `<label>` that names this element.
///
/// Both ways HTML allows: a `<label for=...>` pointing at it, and a `<label>`
/// wrapped around it.
fn label_for(document: &Document, id: NodeId) -> Option<String> {
    let element = document.element(id)?;
    if let Some(own_id) = element.attr("id") {
        let pointing = document.descendants(document.root()).find(|candidate| {
            document.element(*candidate).is_some_and(|label| {
                label.name.is_html("label") && label.attr("for") == Some(own_id)
            })
        });
        if let Some(label) = pointing {
            return Some(document.text_content(label));
        }
    }
    // Wrapped: the nearest `<label>` ancestor.
    let mut current = document.parent(id);
    while let Some(ancestor) = current {
        if document
            .element(ancestor)
            .is_some_and(|element| element.name.is_html("label"))
        {
            return Some(document.text_content(ancestor));
        }
        current = document.parent(ancestor);
    }
    None
}

/// Whether a `<label>` element actually names a control.
///
/// A label that points at nothing, or wraps nothing, is a piece of text like
/// any other and is read as one. A label that *does* name a control has
/// already been read — as that control's name — and reading it again is the
/// same words twice, with the second copy being a thing an agent could try to
/// act on.
pub(crate) fn label_names_something(document: &Document, id: NodeId) -> bool {
    let Some(element) = document.element(id) else {
        return false;
    };
    if !element.name.is_html("label") {
        return false;
    }
    if let Some(target) = element.attr("for") {
        return document.descendants(document.root()).any(|candidate| {
            document
                .element(candidate)
                .is_some_and(|held| held.attr("id") == Some(target) && is_labelable(held))
        });
    }
    // Wrapped: a control inside it.
    document
        .descendants(id)
        .any(|inner| document.element(inner).is_some_and(is_labelable))
}

/// The elements a `<label>` can name.
///
/// HTML's own list, trimmed to the controls this engine builds boxes for.
fn is_labelable(element: &alo_dom::Element) -> bool {
    [
        "input", "select", "textarea", "button", "meter", "progress", "output",
    ]
    .iter()
    .any(|name| element.name.is_html(name))
}

/// Everything a person would read inside a box, joined.
///
/// **Every piece of it.** An inline box broken around a block is several boxes
/// and one element, and what a person reads inside it is everything the
/// document put there — the block between the pieces included. Layout took it
/// apart; a name must not be taken apart with it.
pub fn text_of(boxes: &BoxTree, id: BoxId) -> String {
    let mut out = String::new();
    for member in boxes.whole_of(id) {
        gather(boxes, member, &mut out);
    }
    out
}

/// The text of a box and everything under it, with a space wherever a block
/// begins or ends.
///
/// Without the space, `<a>Read the<div>docs</div></a>` is called "Read thedocs".
/// A block-level box is a line of its own on the screen, and a name read out
/// has to sound like what a person sees.
fn gather(boxes: &BoxTree, id: BoxId, out: &mut String) {
    let Some(node) = boxes.get(id) else {
        return;
    };
    let is_block = node.kind.outside() == alo_box::Outside::Block;
    if is_block {
        separate(out);
    }
    if let Some(text) = node.text() {
        out.push_str(text);
    }
    for child in boxes.children(id) {
        gather(boxes, child, out);
    }
    if is_block {
        separate(out);
    }
}

/// A space, unless there is already one or there is nothing yet.
fn separate(out: &mut String) {
    if !out.is_empty() && !out.ends_with(char::is_whitespace) {
        out.push(' ');
    }
}

/// Trim and collapse whitespace, and report nothing for nothing.
///
/// A name of spaces is not a name, and `"Save   invoice\n"` and `"Save invoice"`
/// are the same name — an agent asking for one should not have to guess which
/// spelling the markup used.
pub fn normalise(text: &str) -> Option<String> {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!collapsed.is_empty()).then_some(collapsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_is_collapsed_and_nothing_is_nothing() {
        assert_eq!(
            normalise("  Save   invoice \n"),
            Some("Save invoice".to_owned())
        );
        assert_eq!(normalise("   "), None);
        assert_eq!(normalise(""), None);
        assert_eq!(normalise("one"), Some("one".to_owned()));
    }

    #[test]
    fn the_roles_named_by_their_content_are_the_ones_aria_names() {
        assert!(names_itself_from_content(&Role::Known(KnownRole::Button)));
        assert!(names_itself_from_content(&Role::Known(KnownRole::Link)));
        assert!(names_itself_from_content(&Role::Known(KnownRole::ListItem)));
        assert!(!names_itself_from_content(&Role::Known(
            KnownRole::Paragraph
        )));
        assert!(
            !names_itself_from_content(&Role::Generic),
            "a div is not called by its contents, or every page would be one long name",
        );
        assert!(!names_itself_from_content(&Role::Known(KnownRole::Main)));
    }
}
