/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What activating an option does to its list (ADR 0064 § 2).
//!
//! An agent chooses an option of a `<select>` by activating the option, as
//! a person's click on it does:
//!
//! - in a list that takes **one** choice — closed, or with a `size` — the
//!   option becomes its choice and no other does;
//! - in a list that takes **several** (`multiple`) the option is
//!   **toggled** and the others are left as they are, as HTML's *toggle the
//!   selectedness* and a Meta-click on macOS do. A plain click's *this one
//!   alone* would make choosing two things impossible one verb at a time.
//!
//! Selectedness is the `selected` attribute until item 469 holds it apart
//! from markup, as [`crate::value::selected`] reads it, so this changes the
//! attribute and nothing else. Whoever carries the verb out asks this one
//! rule: `alo-agent`'s `apply` on a page that never ran script, and the
//! renderer's task, between nothing and the `input` and `change` it fires,
//! on one that has.

use crate::document::Document;
use crate::node::NodeId;
use crate::value;

/// The `<select>` whose list of options holds the `<option>` `option`: its
/// parent, or its `<optgroup>`'s. [`None`] for an option anywhere else — in
/// a `<datalist>`, or loose — and for any other node.
pub fn list_of(document: &Document, option: NodeId) -> Option<NodeId> {
    let is = |node: NodeId, name: &str| {
        document
            .element(node)
            .is_some_and(|element| element.name.is_html(name))
    };
    if !is(option, "option") {
        return None;
    }
    let parent = document.parent(option)?;
    if is(parent, "select") {
        return Some(parent);
    }
    if !is(parent, "optgroup") {
        return None;
    }
    document.parent(parent).filter(|list| is(*list, "select"))
}

/// Whether the `<option>` `option` is a choice its list holds, by HTML's
/// selectedness rule ([`value::selected`]) — which chooses the first option
/// that is not disabled in a closed list nobody marked.
pub fn is_chosen(document: &Document, option: NodeId) -> bool {
    list_of(document, option).is_some_and(|list| value::selected(document, list).contains(&option))
}

/// Activate the `<option>` `option` in its list, and say each option whose
/// `selected` attribute changed and whether it is set now, in tree order.
/// [`None`] for a node that is not an option of a list, which an agent's
/// decision never names.
///
/// Choosing what a list of one choice already holds changes nothing — but
/// an option chosen only because nobody marked any (the first) is marked,
/// so that the choice stays when another option is added before it.
pub fn choose(document: &mut Document, option: NodeId) -> Option<Vec<(NodeId, bool)>> {
    let list = list_of(document, option)?;
    let multiple = document
        .element(list)
        .is_some_and(|element| element.attr("multiple").is_some());
    let marked = |document: &Document, node: NodeId| {
        document
            .element(node)
            .is_some_and(|element| element.attr("selected").is_some())
    };
    let mut changed = Vec::new();
    if multiple {
        let now = !marked(document, option);
        mark(document, option, now);
        changed.push((option, now));
        return Some(changed);
    }
    for each in value::options(document, list) {
        let wanted = each == option;
        if marked(document, each) != wanted {
            mark(document, each, wanted);
            changed.push((each, wanted));
        }
    }
    Some(changed)
}

/// Set or take away the `selected` attribute of `option`.
fn mark(document: &mut Document, option: NodeId, set: bool) {
    if set {
        document.set_attribute(option, "selected", "");
    } else {
        document.remove_attribute(option, "selected");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_document;

    /// The option whose `value` is `wanted`.
    fn option(document: &Document, wanted: &str) -> NodeId {
        let Some(found) = document.descendants(document.root()).find(|id| {
            document
                .element(*id)
                .is_some_and(|e| e.name.is_html("option") && e.attr("value") == Some(wanted))
        }) else {
            panic!("no option {wanted}");
        };
        found
    }

    fn chosen(document: &Document, values: &[&str]) -> Vec<bool> {
        values
            .iter()
            .map(|wanted| is_chosen(document, option(document, wanted)))
            .collect()
    }

    #[test]
    fn an_option_belongs_to_its_select_or_its_groups() {
        let document = parse_document(
            "<select id=s><option value=a><optgroup><option value=b></optgroup></select>\
             <datalist><option value=c></datalist>",
        );
        let list = list_of(&document, option(&document, "a"));
        assert!(list.is_some());
        assert_eq!(list_of(&document, option(&document, "b")), list);
        assert_eq!(list_of(&document, option(&document, "c")), None);
        assert_eq!(list.and_then(|list| list_of(&document, list)), None);
    }

    #[test]
    fn in_a_list_of_one_choice_the_option_is_the_only_one_chosen() {
        let mut document = parse_document(
            "<select><option value=a selected><optgroup>\
             <option value=b selected><option value=c></optgroup></select>",
        );
        assert_eq!(chosen(&document, &["a", "b", "c"]), [false, true, false]);
        let c = option(&document, "c");
        let changed = choose(&mut document, c);
        let (a, b) = (option(&document, "a"), option(&document, "b"));
        assert_eq!(changed, Some(vec![(a, false), (b, false), (c, true)]));
        assert_eq!(chosen(&document, &["a", "b", "c"]), [false, false, true]);
        // Choosing it again changes nothing.
        assert_eq!(choose(&mut document, c), Some(Vec::new()));
    }

    #[test]
    fn the_option_chosen_because_nobody_marked_one_is_marked_when_chosen() {
        let mut document = parse_document("<select><option value=a><option value=b></select>");
        let a = option(&document, "a");
        assert_eq!(chosen(&document, &["a", "b"]), [true, false]);
        assert_eq!(choose(&mut document, a), Some(vec![(a, true)]));
        assert_eq!(chosen(&document, &["a", "b"]), [true, false]);
    }

    #[test]
    fn in_a_list_of_several_the_option_is_toggled_and_the_rest_left() {
        let mut document = parse_document(
            "<select multiple><option value=r selected><option value=g>\
             <option value=b selected></select>",
        );
        let (r, g) = (option(&document, "r"), option(&document, "g"));
        assert_eq!(choose(&mut document, g), Some(vec![(g, true)]));
        assert_eq!(chosen(&document, &["r", "g", "b"]), [true, true, true]);
        assert_eq!(choose(&mut document, r), Some(vec![(r, false)]));
        assert_eq!(chosen(&document, &["r", "g", "b"]), [false, true, true]);
    }

    #[test]
    fn a_node_that_is_not_an_option_of_a_list_is_not_chosen() {
        let mut document = parse_document("<datalist><option value=c></datalist><p>t</p>");
        let c = option(&document, "c");
        assert_eq!(choose(&mut document, c), None);
        assert!(!is_chosen(&document, c));
        let root = document.root();
        assert_eq!(choose(&mut document, root), None);
    }
}
