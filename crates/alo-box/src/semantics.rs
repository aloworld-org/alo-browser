/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a box means, in one place: what it is, what is true of it, and what it
//! is called.
//!
//! ADR 0002 says the layout tree *is* the agent's tree — a view of it, never a
//! second structure — so the semantics have to be on the box itself, put there
//! when the box is made. A box tree that kept only rectangles could not be
//! retrofitted into this, which is exactly why queue item 4 sits before layout
//! rather than after it.
//!
//! # The name, and what is deliberately not here
//!
//! The **declared** name is here: `aria-label`, `aria-labelledby`, an image's
//! `alt`, a `title`. Those are the author saying what a thing is called.
//!
//! The **accessible name** — the full algorithm, which falls back to a box's
//! own text content and to a `<label>` pointing at a field — is queue item 9's,
//! because it needs the finished tree to walk. Putting half of it here would
//! mean two implementations of naming, and ADR 0002's whole argument is that
//! the second implementation is the one that is wrong.

use crate::role::Role;
use crate::state::States;
use alo_dom::{Document, Element, NodeId};
use core::fmt;

/// What a box is, what is true of it, and what the author called it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Semantics {
    /// What the box is.
    pub role: Role,
    /// What is true of it.
    pub states: States,
    /// What the author called it outright, if anything.
    pub label: Option<String>,
}

impl Semantics {
    /// Read what an element declares about itself.
    pub fn of(document: &Document, id: NodeId, element: &Element) -> Self {
        Self {
            role: Role::of(document, id, element),
            states: States::of(document, id, element),
            label: declared_label(document, id, element),
        }
    }

    /// The semantics of a box no element asked for — an anonymous box the
    /// engine made to hold something. It is nothing, in no state, with no name.
    pub fn anonymous() -> Self {
        Self {
            role: Role::Presentational,
            states: States::default(),
            label: None,
        }
    }
}

impl fmt::Display for Semantics {
    /// `role "label" states`, with the parts that say nothing left out.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.role)?;
        if let Some(label) = &self.label {
            write!(f, " {label:?}")?;
        }
        if !self.states.is_unremarkable() {
            write!(f, " [{}]", self.states)?;
        }
        Ok(())
    }
}

/// The name the author gave an element outright, in the order ARIA asks.
fn declared_label(document: &Document, id: NodeId, element: &Element) -> Option<String> {
    // `aria-labelledby` first: pointing at the text that names a thing is more
    // reliable than repeating it, and it is what ARIA prefers.
    if let Some(references) = element.attr("aria-labelledby") {
        let named = text_of_referenced(document, references);
        if !named.is_empty() {
            return Some(named);
        }
    }
    for attribute in ["aria-label", "alt", "title"] {
        if let Some(value) = element.attr(attribute) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    // A `<fieldset>` is named by its `<legend>`, which is the same shape as a
    // `<label>` naming the control it wraps: an element named by something it
    // contains rather than by an attribute.
    //
    // Found by the first page with a form (queue item 181). Without it a
    // fieldset comes back as an unnamed `group` with the legend's words sitting
    // beside it as loose text — so an agent asked to tick "Large" under "Pizza
    // Size" has no way to tell which group is which, and neither does anybody
    // reading the tree.
    if element.name.local.eq_ignore_ascii_case("fieldset") {
        return legend_of(document, id);
    }
    None
}

/// The text of a `<fieldset>`'s first `<legend>`.
///
/// The *first*, because a fieldset with two legends is a page that has already
/// gone wrong and the first is what every browser uses. Direct children only: a
/// legend belonging to a nested fieldset names that one, not this one.
fn legend_of(document: &Document, id: NodeId) -> Option<String> {
    for child in document.children(id) {
        let Some(element) = document.element(child) else {
            continue;
        };
        if !element.name.local.eq_ignore_ascii_case("legend") {
            continue;
        }
        let text = document.text_content(child);
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !text.is_empty() {
            return Some(text);
        }
    }
    None
}

/// The text of the elements a space-separated list of ids points at, joined.
///
/// An id that names nothing contributes nothing — it is not an error, and a
/// label built from the rest is better than no label at all.
fn text_of_referenced(document: &Document, references: &str) -> String {
    let mut parts = Vec::new();
    for wanted in references.split_ascii_whitespace() {
        let Some(target) = document.descendants(document.root()).find(|id| {
            document
                .element(*id)
                .is_some_and(|element| element.attr("id") == Some(wanted))
        }) else {
            continue;
        };
        let text = document.text_content(target);
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if !text.is_empty() {
            parts.push(text);
        }
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    fn semantics_of(html: &str, wanted: &str) -> Semantics {
        let document = parse_document(html);
        let id = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.attr("id") == Some(wanted))
            })
            .unwrap_or_else(|| panic!("no element with id={wanted}"));
        let element = document.element(id).expect("an element");
        Semantics::of(&document, id, element)
    }

    #[test]
    fn a_box_describes_itself_as_what_it_is() {
        assert_eq!(
            semantics_of("<div id=x>t</div>", "x").to_string(),
            "generic"
        );
        assert_eq!(
            semantics_of("<button id=x>Save</button>", "x").to_string(),
            "button",
        );
    }

    #[test]
    fn a_declared_label_is_part_of_what_a_box_is() {
        assert_eq!(
            semantics_of("<button id=x aria-label='Save invoice'>S</button>", "x").to_string(),
            "button \"Save invoice\"",
        );
        assert_eq!(
            semantics_of("<img id=x alt='A chart'>", "x").to_string(),
            "image \"A chart\"",
        );
        assert_eq!(
            semantics_of("<div id=x title='A note'>t</div>", "x").to_string(),
            "generic \"A note\"",
        );
    }

    #[test]
    fn aria_labelledby_takes_the_text_it_points_at() {
        let html = "<h2 id=title>Invoices</h2><section id=x aria-labelledby=title></section>";
        let semantics = semantics_of(html, "x");
        assert_eq!(semantics.label.as_deref(), Some("Invoices"));
        assert_eq!(semantics.role.to_string(), "region");
    }

    #[test]
    fn several_references_are_joined_and_missing_ones_are_skipped() {
        let html = "<span id=a>Invoice</span><span id=b>  12  </span>\
             <div id=x aria-labelledby='a b nowhere'>d</div>";
        assert_eq!(semantics_of(html, "x").label.as_deref(), Some("Invoice 12"),);

        let none = "<div id=x aria-labelledby='nowhere'>d</div>";
        assert_eq!(semantics_of(none, "x").label, None);
    }

    #[test]
    fn the_more_explicit_name_wins() {
        let html = "<span id=t>Pointed at</span>\
             <img id=x aria-labelledby=t aria-label='Said outright' alt='Alt text' title='A title'>";
        assert_eq!(semantics_of(html, "x").label.as_deref(), Some("Pointed at"),);

        let without = "<img id=x aria-label='Said outright' alt='Alt text'>";
        assert_eq!(
            semantics_of(without, "x").label.as_deref(),
            Some("Said outright"),
        );
    }

    #[test]
    fn a_name_of_nothing_is_not_a_name() {
        assert_eq!(semantics_of("<img id=x alt=''>", "x").label, None);
        assert_eq!(
            semantics_of("<div id=x title='   '>t</div>", "x").label,
            None
        );
    }

    #[test]
    fn states_are_described_beside_the_role() {
        assert_eq!(
            semantics_of("<input id=x type=checkbox checked disabled>", "x").to_string(),
            "checkbox [disabled checked=true]",
        );
        assert_eq!(
            semantics_of("<h2 id=x aria-label=Totals>Totals</h2>", "x").to_string(),
            "heading \"Totals\" [level=2]",
        );
    }

    #[test]
    fn an_anonymous_box_is_nothing_and_says_so() {
        let anonymous = Semantics::anonymous();
        assert_eq!(anonymous.to_string(), "presentation");
        assert!(anonymous.states.is_unremarkable());
        assert_eq!(anonymous.label, None);
    }
}
