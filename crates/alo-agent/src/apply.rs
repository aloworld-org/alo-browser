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
//! **What a click does to a box is not decided here.** It is `alo-dom`'s
//! activation rule ([`alo_dom::activation`], ADR 0018 § 6), which a script's
//! `el.click()` runs too; this only asks it. And this is only asked on a page
//! that has **never run script**: on one that has, `Activate` is a `click`
//! the renderer dispatches to the page's listeners, around which it runs the
//! same rule itself — and a listener may cancel it.
//!
//! **`aria-checked` is changed here, and only here**, as a stage 1
//! accommodation (ADR 0018 § 7): on a page without script nobody else keeps
//! that promise, and alo's own scriptless screens were tested against it. On
//! a page with script, the page's listener keeps it.
//!
//! **Activating a plain button changes nothing** on a page without script,
//! and that is correct rather than missing: what a button does is run a
//! script, and there is none. **Following a link changes nothing here**
//! either — where a page goes is the browser process's, and the outcome says
//! where.

use crate::verb::Outcome;
use alo_box::{BoxId, BoxTree};
use alo_dom::activation::{self, Activation};
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
        Outcome::Activated { .. } => activate(document, node),
        // Where a page goes is the browser process's, and scrolling is a fact
        // about the view rather than about the document.
        Outcome::Followed { .. } | Outcome::Scrolled { .. } => vec![Change::Nothing],
    }
}

/// The document node a box came from.
fn source_of(boxes: &BoxTree, id: BoxId) -> Option<NodeId> {
    boxes.get(id).and_then(|node| node.kind.node())
}

/// Activating something on a page that has never run script: the stage 1
/// accommodation for ARIA state (ADR 0018 § 7), or else `alo-dom`'s
/// activation rule run with nobody listening — before, then after, since
/// there is no listener to cancel it.
fn activate(document: &mut Document, node: NodeId) -> Vec<Change> {
    let Some(element) = document.element(node) else {
        return vec![Change::Nothing];
    };
    // An author who declared the state with ARIA is the one who decides what
    // it means, so the same attribute is the one to change — on a page with
    // no script, where nobody else will.
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
    let done = activation::before(document, node);
    // `after`'s `input` and `change` go to nobody on a page with no script,
    // and a link's destination is already the outcome's.
    let _follows = activation::after(document, &done);
    changes(&done)
}

/// What [`activation::before`] changed, as [`Change`]s.
fn changes(done: &Activation) -> Vec<Change> {
    let checked = |node: NodeId, now: bool| {
        if now {
            Change::Set {
                node,
                attribute: "checked".to_owned(),
                value: String::new(),
            }
        } else {
            Change::Removed {
                node,
                attribute: "checked".to_owned(),
            }
        }
    };
    let mut changes = match done {
        Activation::Checkbox { node, was } => vec![checked(*node, !was)],
        Activation::Radio {
            node,
            was,
            unchecked,
        } => {
            let mut changes: Vec<Change> = unchecked
                .iter()
                .map(|other| checked(*other, false))
                .collect();
            if !was {
                changes.push(checked(*node, true));
            }
            changes
        }
        Activation::None | Activation::Link { .. } | Activation::Button { .. } => Vec::new(),
    };
    if changes.is_empty() {
        changes.push(Change::Nothing);
    }
    changes
}
