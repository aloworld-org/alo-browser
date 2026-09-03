/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Carrying a verb into the document.
//!
//! [`crate::verb::perform`] **decides**: it finds the one thing a description
//! names, refuses what cannot be operated, and says what it would do. This is
//! the other half — the part that changes the page — and it is deliberately a
//! second step rather than the same one.
//!
//! # Why deciding and changing are two things
//!
//! The agent tree borrows the document, so nothing holding one can change it.
//! That is not an inconvenience to work around: **the decision has to be made
//! against the tree the agent read**, and the change has to be made to the
//! document afterwards. Splitting them says so in the types.
//!
//! It is also what the boundary needs. ADR 0005's renderer decides against the
//! tree it has, applies to the document it owns, and renders again — three
//! steps that a single mutating call could not have been.
//!
//! # What a verb can and cannot change
//!
//! An **attribute** — which is what a field's text and a checkbox's state are.
//! Not the shape of the tree: adding and removing nodes belongs with the DOM
//! APIs, and nothing an agent does needs it.
//!
//! **Activating a button changes nothing**, and that is correct rather than
//! missing: what a button does is run a script, and a page without one does
//! nothing when it is pressed. **Following a link changes nothing here**
//! either — where a page goes is the browser process's, and the outcome says
//! where.

use crate::verb::Outcome;
use alo_box::{BoxId, BoxTree};
use alo_dom::{Document, NodeId};

/// What changing the document actually did.
///
/// A verb can be carried out, or be a verb there is nothing to carry out —
/// pressing a button on a page with no script. Both are results; neither is a
/// failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// An attribute was set to this.
    Set {
        /// Which element.
        node: NodeId,
        /// Which attribute.
        attribute: String,
        /// What it says now.
        value: String,
    },
    /// An attribute was taken away.
    Removed {
        /// Which element.
        node: NodeId,
        /// Which attribute.
        attribute: String,
    },
    /// Nothing about the document changed, and nothing should have.
    Nothing,
}

/// Carry an outcome into the document, and say what it changed.
///
/// The outcome came from [`crate::verb::perform`] against a tree of these same
/// boxes. Handing it a different document is a caller error that answers
/// [`Change::Nothing`] rather than changing the wrong node — ids are minted per
/// document (ADR 0003), so one from elsewhere resolves to nothing.
pub fn apply(document: &mut Document, boxes: &BoxTree, outcome: &Outcome) -> Vec<Change> {
    let Some(node) = source_of(boxes, outcome.node()) else {
        return vec![Change::Nothing];
    };
    match outcome {
        Outcome::TextPut { text, .. } => {
            // A field's text is its `value`, which is where it was going to be
            // read from anyway — an `<input>` shows what it holds.
            document.set_attribute(node, "value", text);
            vec![Change::Set {
                node,
                attribute: "value".to_owned(),
                value: text.clone(),
            }]
        }
        Outcome::Activated { .. } => toggle(document, node),
        // Where a page goes is the browser process's, and scrolling is a fact
        // about the view rather than about the document.
        Outcome::Followed { .. } | Outcome::Scrolled { .. } => vec![Change::Nothing],
    }
}

/// The document node a box came from.
fn source_of(boxes: &BoxTree, id: BoxId) -> Option<NodeId> {
    boxes.get(id).and_then(|node| node.kind.node())
}

/// Activating something that holds a state, which is the only activation a
/// page without script has an answer for.
fn toggle(document: &mut Document, node: NodeId) -> Vec<Change> {
    let Some(element) = document.element(node) else {
        return vec![Change::Nothing];
    };
    // An author who declared the state with ARIA is the one who decides what
    // it means, so the same attribute is the one to change.
    if element.attr("aria-checked").is_some() {
        let now = if element.attr("aria-checked") == Some("true") {
            "false"
        } else {
            "true"
        };
        document.set_attribute(node, "aria-checked", now);
        return vec![Change::Set {
            node,
            attribute: "aria-checked".to_owned(),
            value: now.to_owned(),
        }];
    }
    if !element.name.is_html("input") {
        return vec![Change::Nothing];
    }
    let kind = element
        .attr("type")
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match kind.as_str() {
        "checkbox" => {
            if element.attr("checked").is_some() {
                document.remove_attribute(node, "checked");
                vec![Change::Removed {
                    node,
                    attribute: "checked".to_owned(),
                }]
            } else {
                document.set_attribute(node, "checked", "");
                vec![Change::Set {
                    node,
                    attribute: "checked".to_owned(),
                    value: String::new(),
                }]
            }
        }
        // A radio does not toggle: choosing one un-chooses the rest of its
        // group, which is the whole reason a group exists.
        "radio" => choose_radio(document, node),
        _ => vec![Change::Nothing],
    }
}

/// Choose one radio, and un-choose the others that share its name.
fn choose_radio(document: &mut Document, node: NodeId) -> Vec<Change> {
    let group = document
        .element(node)
        .and_then(|element| element.attr("name"))
        .map(str::to_owned);
    let mut changes = Vec::new();

    if let Some(group) = group {
        let siblings: Vec<NodeId> = document
            .descendants(document.root())
            .filter(|held| *held != node)
            .filter(|held| {
                document.element(*held).is_some_and(|element| {
                    element.name.is_html("input")
                        && element
                            .attr("type")
                            .is_some_and(|kind| kind.eq_ignore_ascii_case("radio"))
                        && element.attr("name") == Some(group.as_str())
                        && element.attr("checked").is_some()
                })
            })
            .collect();
        for sibling in siblings {
            document.remove_attribute(sibling, "checked");
            changes.push(Change::Removed {
                node: sibling,
                attribute: "checked".to_owned(),
            });
        }
    }

    if document
        .element(node)
        .is_some_and(|element| element.attr("checked").is_none())
    {
        document.set_attribute(node, "checked", "");
        changes.push(Change::Set {
            node,
            attribute: "checked".to_owned(),
            value: String::new(),
        });
    }
    if changes.is_empty() {
        changes.push(Change::Nothing);
    }
    changes
}
