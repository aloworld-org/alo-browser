/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a click does to the element it activates (ADR 0018 § 6, queue item
//! 256).
//!
//! The HTML standard gives some elements an **activation behaviour** — what
//! a click nobody cancelled does — and a checkbox and a radio a
//! **legacy-pre-activation behaviour** as well: the box is changed *before*
//! the click's listeners run, so they read the new state, and put back if one
//! of them cancels. The DOM standard's dispatch runs them around one
//! dispatch of a `click`, in three steps, and so does whoever dispatches one
//! here:
//!
//! 1. [`before`] — find the **activation target** (the click's target, or
//!    the nearest ancestor with an activation behaviour, since a click
//!    bubbles) and run its pre-activation, answering an [`Activation`] that
//!    remembers what it changed;
//! 2. the dispatch;
//! 3. [`cancelled`] if a listener cancelled the click, which puts back what
//!    step 1 changed — or [`after`] if nobody did, which says what follows
//!    ([`Follows`]): the `input` and `change` a toggled box fires, or a link
//!    to follow.
//!
//! These are rules about the document with two callers — an agent's
//! `Activate`, which the renderer dispatches, and a script's `el.click()`
//! (queue item 261) — so they live here once, for ADR 0017 § 5's reason.
//!
//! # Checkedness is the `checked` attribute, for now
//!
//! The standard keeps a box's *checkedness* apart from its `checked`
//! attribute, which only sets its default. That split is item 82's (forms);
//! until it lands the attribute is the state, as it has been since stage 1,
//! and only where it is read and written changes when it does — not this
//! sequence. *Indeterminateness* is not held at all yet: nothing can set it
//! without the `indeterminate` property, which is item 82's too.
//!
//! # A radio button group, without a form
//!
//! A radio's group is the radios in the same tree with the same non-empty
//! `name` **and the same form owner**. Form owners are item 82's, so the
//! group here is the same tree and the same name; on a page whose radios
//! sharing a name are split across two forms, choosing one unchecks the
//! other, and item 82 is where that stops.
//!
//! # What has an activation behaviour
//!
//! An `input` that is a checkbox or a radio, an `a` or `area` with an
//! `href` (following it), and a `button`, whose activation behaviour —
//! submitting or resetting its form — is item 82's, and which so does
//! nothing yet. A `button` still counts for the search in step 1: a click
//! on a `<span>` inside a button inside a link is the button's, not the
//! link's, as in every browser.

use crate::document::Document;
use crate::node::NodeId;

/// What [`before`] did, to be undone by [`cancelled`] or finished by
/// [`after`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Activation {
    /// No element on the way up has an activation behaviour: the click is
    /// only a click.
    None,
    /// A checkbox, whose checkedness was turned over.
    Checkbox {
        /// The box.
        node: NodeId,
        /// Whether it was checked before.
        was: bool,
    },
    /// A radio, now checked, and every other radio in its group unchecked.
    Radio {
        /// The radio.
        node: NodeId,
        /// Whether it was checked before.
        was: bool,
        /// The radios in its group that were checked and now are not, in
        /// document order. The standard remembers one — the group can only
        /// hold one checked radio — but markup can check several, and each
        /// was unchecked.
        unchecked: Vec<NodeId>,
    },
    /// A link, which nothing changes before the click.
    Link {
        /// The `a` or `area`.
        node: NodeId,
    },
    /// A button, whose activation behaviour is its form's (item 82).
    Button {
        /// The button.
        node: NodeId,
    },
}

impl Activation {
    /// The element whose activation this is, if any.
    pub const fn target(&self) -> Option<NodeId> {
        match self {
            Activation::None => None,
            Activation::Checkbox { node, .. }
            | Activation::Radio { node, .. }
            | Activation::Link { node }
            | Activation::Button { node } => Some(*node),
        }
    }
}

/// What follows a click nobody cancelled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Follows {
    /// Nothing.
    Nothing,
    /// A box that was toggled and is in its document: fire `input` (bubbling
    /// and composed) and then `change` (bubbling) at it.
    InputAndChange(NodeId),
    /// Follow the link to `href`, as written.
    Link {
        /// The `a` or `area`.
        node: NodeId,
        /// Its `href`, as written.
        href: String,
    },
}

/// Which kind of activation behaviour an element has.
enum Behaviour {
    Checkbox,
    Radio,
    Link,
    Button,
}

/// The activation behaviour `node` has, if any.
fn behaviour(document: &Document, node: NodeId) -> Option<Behaviour> {
    let element = document.element(node)?;
    if element.name.is_html("input") {
        let kind = element.attr("type").unwrap_or_default();
        if kind.eq_ignore_ascii_case("checkbox") {
            return Some(Behaviour::Checkbox);
        }
        if kind.eq_ignore_ascii_case("radio") {
            return Some(Behaviour::Radio);
        }
        return None;
    }
    if (element.name.is_html("a") || element.name.is_html("area")) && element.attr("href").is_some()
    {
        return Some(Behaviour::Link);
    }
    if element.name.is_html("button") {
        return Some(Behaviour::Button);
    }
    None
}

/// The DOM standard's *activation target* for a click at `target`: `target`
/// itself if it has an activation behaviour, or else the nearest ancestor
/// that has one — a click bubbles, so its parents are asked.
pub fn activation_target(document: &Document, target: NodeId) -> Option<NodeId> {
    let mut at = Some(target);
    while let Some(node) = at {
        if behaviour(document, node).is_some() {
            return Some(node);
        }
        at = document.parent(node);
    }
    None
}

/// Step 1: the legacy-pre-activation behaviour of the activation target of
/// a click at `target` — a checkbox turned over, a radio checked and its
/// group unchecked — answering what was done.
pub fn before(document: &mut Document, target: NodeId) -> Activation {
    let Some(node) = activation_target(document, target) else {
        return Activation::None;
    };
    match behaviour(document, node) {
        Some(Behaviour::Checkbox) => {
            let was = checked(document, node);
            set_checked(document, node, !was);
            Activation::Checkbox { node, was }
        }
        Some(Behaviour::Radio) => {
            let was = checked(document, node);
            let unchecked: Vec<NodeId> = group(document, node)
                .into_iter()
                .filter(|other| *other != node && checked(document, *other))
                .collect();
            for other in &unchecked {
                set_checked(document, *other, false);
            }
            set_checked(document, node, true);
            Activation::Radio {
                node,
                was,
                unchecked,
            }
        }
        Some(Behaviour::Link) => Activation::Link { node },
        Some(Behaviour::Button) => Activation::Button { node },
        None => Activation::None,
    }
}

/// Step 3, when a listener cancelled the click: the legacy-canceled-
/// activation behaviour, putting back what [`before`] changed.
///
/// A checkbox goes back to what it was. A radio's previously checked
/// radio is checked again if it is still in the radio's group — which
/// unchecks the radio — and otherwise the radio is unchecked, as the
/// standard says. With several previously checked (markup can say so), each
/// still in the group is checked again.
pub fn cancelled(document: &mut Document, activation: &Activation) {
    match activation {
        Activation::Checkbox { node, was } => set_checked(document, *node, *was),
        Activation::Radio {
            node,
            was,
            unchecked,
        } => {
            let group = group(document, *node);
            let back: Vec<NodeId> = unchecked
                .iter()
                .copied()
                .filter(|other| group.contains(other))
                .collect();
            if back.is_empty() {
                // No previously checked radio still in the group. It was
                // unchecked before unless it was the checked one itself —
                // in which case it was in no list, and checking it again
                // changes nothing.
                set_checked(document, *node, *was);
            } else {
                set_checked(document, *node, false);
                for other in back {
                    set_checked(document, other, true);
                }
            }
        }
        Activation::None | Activation::Link { .. } | Activation::Button { .. } => {}
    }
}

/// Step 3, when nobody cancelled the click: what the activation behaviour
/// says follows.
///
/// A checkbox or radio fires `input` and `change` **only if it is in its
/// document** (the standard's *connected*): a box a listener took out of
/// the page changes, and nobody is told. A link is followed wherever it is.
pub fn after(document: &Document, activation: &Activation) -> Follows {
    match activation {
        Activation::Checkbox { node, .. } | Activation::Radio { node, .. } => {
            if document.is_attached(*node) {
                Follows::InputAndChange(*node)
            } else {
                Follows::Nothing
            }
        }
        Activation::Link { node } => match document.element(*node).and_then(|e| e.attr("href")) {
            Some(href) => Follows::Link {
                node: *node,
                href: href.to_owned(),
            },
            // A listener took the `href` away: there is nowhere to go.
            None => Follows::Nothing,
        },
        Activation::None | Activation::Button { .. } => Follows::Nothing,
    }
}

/// Whether a box is checked: its `checked` attribute, until item 82 holds
/// checkedness apart from it.
fn checked(document: &Document, node: NodeId) -> bool {
    document
        .element(node)
        .is_some_and(|element| element.attr("checked").is_some())
}

/// Check or uncheck a box. Setting it to what it already is changes
/// nothing and is not counted as a change.
fn set_checked(document: &mut Document, node: NodeId, to: bool) {
    if checked(document, node) == to {
        return;
    }
    if to {
        document.set_attribute(node, "checked", "");
    } else {
        document.remove_attribute(node, "checked");
    }
}

/// The root of the tree `node` is in.
fn root_of(document: &Document, node: NodeId) -> NodeId {
    let mut at = node;
    while let Some(parent) = document.parent(at) {
        at = parent;
    }
    at
}

/// The radio button group `node` is in, itself included, in tree order —
/// empty if it has no group: no `name`, or an empty one.
fn group(document: &Document, node: NodeId) -> Vec<NodeId> {
    let Some(name) = document
        .element(node)
        .and_then(|element| element.attr("name"))
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
    else {
        return Vec::new();
    };
    let root = root_of(document, node);
    core::iter::once(root)
        .chain(document.descendants(root))
        .filter(|other| {
            document.element(*other).is_some_and(|element| {
                element.name.is_html("input")
                    && element
                        .attr("type")
                        .is_some_and(|kind| kind.eq_ignore_ascii_case("radio"))
                    && element.attr("name") == Some(name.as_str())
            })
        })
        .collect()
}
