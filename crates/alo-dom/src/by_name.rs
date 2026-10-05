/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An element's attributes by the name a script writes — the DOM standard's
//! *get*, *set* and *remove an attribute by name*, behind `getAttribute`,
//! `setAttribute` and `removeAttribute` (ADR 0017 § 5).
//!
//! [`Document::set_attribute`] and [`Document::remove_attribute`] take a
//! local name with no namespace, which is what the agent and the parser
//! mean. A script means something else, and the standard spells it out:
//!
//! - the name is a **qualified name** — `xlink:href` on a foreign element
//!   parsed from markup is found by its prefix and local name together;
//! - on an HTML element it is **ASCII-lowercased** first, since an HTML
//!   document's attribute names were lowercased by the parser and a page
//!   writing `getAttribute('ID')` means `id`;
//! - a name set must be a **valid attribute local name**, refused with
//!   `InvalidCharacterError` otherwise;
//! - and only the **first** attribute with the name is read, changed or
//!   removed — a document can be handed to us with a name repeated.
//!
//! These live here rather than in the bindings for the reason the tree
//! operations do: the rules are the document's, whoever calls.

use std::borrow::Cow;

use crate::document::Document;
use crate::name::Namespace;
use crate::node::{Attribute, Element, NodeId};
use crate::validity::{self, Refusal};

impl Document {
    /// The value of the first attribute of element `id` whose qualified name
    /// is `name` — `getAttribute`.
    ///
    /// [`None`] when there is no such attribute, and when `id` is not an
    /// element.
    pub fn attribute_by_name(&self, id: NodeId, name: &str) -> Option<&str> {
        let element = self.element(id)?;
        let name = spelled(element, name);
        element
            .attrs
            .iter()
            .find(|attribute| is_named(attribute, &name))
            .map(|attribute| &*attribute.value)
    }

    /// Give element `id` the attribute `name` with `value` — `setAttribute`.
    ///
    /// The first attribute with that qualified name keeps its place and takes
    /// the value; with none, a new one with no namespace is added at the end.
    /// A change, counted, even when the value is the one it had. `Ok(None)`
    /// for an id that is not an element, which changes nothing.
    ///
    /// # Errors
    ///
    /// [`Refusal::InvalidCharacter`] for a name no attribute can have; nothing
    /// is changed.
    pub fn set_attribute_by_name(
        &mut self,
        id: NodeId,
        name: &str,
        value: &str,
    ) -> Result<Option<()>, Refusal> {
        if !validity::is_valid_attribute_local_name(name) {
            return Err(Refusal::InvalidCharacter(
                "an attribute's name must be a valid attribute local name",
            ));
        }
        let changed = self.edit_element(id, |element| {
            let name = spelled(element, name).into_owned();
            match element
                .attrs
                .iter_mut()
                .find(|attribute| is_named(attribute, &name))
            {
                Some(attribute) => value.clone_into(&mut attribute.value),
                None => element.attrs.push(Attribute::plain(&name, value)),
            }
        });
        if changed.is_some() {
            self.note_change();
        }
        Ok(changed)
    }

    /// Take away the first attribute of element `id` whose qualified name is
    /// `name` — `removeAttribute` — and say whether there was one.
    ///
    /// Taking away one that was not there changes nothing and is not counted.
    /// [`None`] for an id that is not an element.
    pub fn remove_attribute_by_name(&mut self, id: NodeId, name: &str) -> Option<bool> {
        let removed = self.edit_element(id, |element| {
            let name = spelled(element, name).into_owned();
            let at = element
                .attrs
                .iter()
                .position(|attribute| is_named(attribute, &name));
            at.map(|at| element.attrs.remove(at)).is_some()
        })?;
        if removed {
            self.note_change();
        }
        Some(removed)
    }
}

/// The name as the standard compares it on this element: ASCII-lowercased
/// on an HTML element, as written on any other.
fn spelled<'a>(element: &Element, name: &'a str) -> Cow<'a, str> {
    if element.name.ns == Namespace::Html && name.bytes().any(|b| b.is_ascii_uppercase()) {
        Cow::Owned(name.to_ascii_lowercase())
    } else {
        Cow::Borrowed(name)
    }
}

/// Whether an attribute's qualified name — `prefix:local`, or `local` with
/// no prefix — is `name`.
fn is_named(attribute: &Attribute, name: &str) -> bool {
    let local = &*attribute.name.local;
    match &attribute.name.prefix {
        Some(prefix) => {
            name.strip_prefix(&**prefix)
                .and_then(|rest| rest.strip_prefix(':'))
                == Some(local)
        }
        None => name == local,
    }
}

#[cfg(test)]
mod tests {
    use crate::{Refusal, parse_document};

    #[test]
    fn an_html_element_is_asked_in_lowercase_and_a_foreign_one_as_written() {
        let mut document =
            parse_document("<p ID=a></p><svg viewBox='0 0 1 1'><a xlink:href=b></a></svg>");
        let p = document.descendants(document.root()).find(|id| {
            document
                .element(*id)
                .is_some_and(|element| element.name.is_html("p"))
        });
        let p = p.unwrap();
        assert_eq!(document.attribute_by_name(p, "ID"), Some("a"));
        assert_eq!(document.attribute_by_name(p, "id"), Some("a"));
        let svg = document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|e| &*e.name.local == "svg")
            })
            .unwrap();
        assert_eq!(document.attribute_by_name(svg, "viewBox"), Some("0 0 1 1"));
        assert_eq!(document.attribute_by_name(svg, "viewbox"), None);
        let link = document.first_child(svg).unwrap();
        assert_eq!(document.attribute_by_name(link, "xlink:href"), Some("b"));
        assert_eq!(document.attribute_by_name(link, "href"), None);

        assert_eq!(
            document.set_attribute_by_name(p, "Title", "t"),
            Ok(Some(()))
        );
        assert_eq!(document.attribute_by_name(p, "title"), Some("t"));
        assert_eq!(document.change_count(), 1);
        assert_eq!(document.remove_attribute_by_name(p, "TITLE"), Some(true));
        assert_eq!(document.remove_attribute_by_name(p, "title"), Some(false));
        assert_eq!(document.change_count(), 2, "removing nothing is no change");
    }

    #[test]
    fn a_bad_name_is_refused_and_changes_nothing() {
        let mut document = parse_document("<p></p>");
        let p = document
            .descendants(document.root())
            .find(|id| document.element(*id).is_some_and(|e| e.name.is_html("p")))
            .unwrap();
        for bad in ["", "a b", "a=b", "a/b", "a>b"] {
            assert!(matches!(
                document.set_attribute_by_name(p, bad, "x"),
                Err(Refusal::InvalidCharacter(_))
            ));
        }
        assert_eq!(document.change_count(), 0);
        let text = document.create_text_node("x");
        assert_eq!(document.set_attribute_by_name(text, "a", "b"), Ok(None));
        assert_eq!(document.attribute_by_name(text, "a"), None);
        assert_eq!(document.remove_attribute_by_name(text, "a"), None);
        assert_eq!(document.change_count(), 0);
    }
}
