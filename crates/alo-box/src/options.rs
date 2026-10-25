/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A list's options, held under it and not drawn (ADR 0064 § 5).
//!
//! An `<option>` is `display: none` in the user agent's sheet, so it
//! generates no box — and a box is what an agent names. Yet an agent asked
//! to choose has to read what there is to choose from, and Chrome's tree
//! and every platform's accessibility tree hold a closed list's options
//! under it. So the box tree makes a box for each option a person could
//! choose, and for each `<optgroup>` holding some, **under its list and
//! outside its children**: layout, paint and hit-testing walk children and
//! never meet one, so none has a rectangle, and the agent's tree reads them
//! as the list's ([`crate::BoxTree::undrawn`]). The same pass makes them as
//! every other box, never a second tree built for the agent (ADR 0002).
//!
//! What is held is HTML's *list of options* of the `<select>`: its `option`
//! children and those of its `optgroup` children, in tree order — less any
//! that is `hidden`, which a person cannot choose and which Chrome's tree
//! leaves out too.

use alo_dom::{Document, NodeId};

/// One thing a list holds that is not drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Held {
    /// An `<option>`.
    Option(NodeId),
    /// An `<optgroup>`, and the options in it.
    Group(NodeId, Vec<NodeId>),
}

/// What the `<select>` `list` holds that is not drawn, in tree order, or
/// [`None`] for any other node.
pub(crate) fn of(document: &Document, list: NodeId) -> Option<Vec<Held>> {
    let element = |node: NodeId| document.element(node);
    if !element(list).is_some_and(|list| list.name.is_html("select")) {
        return None;
    }
    let is = |node: NodeId, name: &str| {
        element(node)
            .is_some_and(|element| element.name.is_html(name) && element.attr("hidden").is_none())
    };
    let mut held = Vec::new();
    for child in document.children(list) {
        if is(child, "option") {
            held.push(Held::Option(child));
        } else if is(child, "optgroup") {
            let options: Vec<NodeId> = document
                .children(child)
                .filter(|option| is(*option, "option"))
                .collect();
            if !options.is_empty() {
                held.push(Held::Group(child, options));
            }
        }
    }
    Some(held)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    fn named(document: &Document, name: &str) -> Vec<NodeId> {
        document
            .descendants(document.root())
            .filter(|id| document.get(*id).is_some_and(|n| n.is_html_element(name)))
            .collect()
    }

    #[test]
    fn a_list_holds_its_options_and_its_groups_of_options() {
        let document = parse_document(
            "<select><option>a<option hidden>gone<optgroup label=G><option>b\
             </optgroup><optgroup label=Empty></optgroup><optgroup hidden><option>c\
             </optgroup></select>",
        );
        let options = named(&document, "option");
        let groups = named(&document, "optgroup");
        let list = named(&document, "select");
        let (Some(a), Some(b), Some(g), Some(list)) = (
            options.first(),
            options.get(2),
            groups.first(),
            list.first(),
        ) else {
            panic!("the markup has a list, options and a group");
        };
        assert_eq!(
            of(&document, *list),
            Some(vec![Held::Option(*a), Held::Group(*g, vec![*b])]),
        );
        assert_eq!(of(&document, *a), None, "an option holds nothing");
    }
}
