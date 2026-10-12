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
//!    step 1 changed — or [`after`] if nobody did, which runs the activation
//!    behaviour that changes the document (a `<details>` opened or closed)
//!    and says what follows ([`Follows`]): the `input` and `change` a
//!    toggled box fires, a link to follow, or the `<details>` toggled.
//!
//! These are rules about the document with two callers — an agent's
//! `Activate`, which the renderer dispatches, and a script's `el.click()`
//! (queue item 261) — so they live here once, for ADR 0017 § 5's reason.
//!
//! # Checkedness is the `checked` attribute, for now
//!
//! The standard keeps a box's *checkedness* apart from its `checked`
//! attribute, which only sets its default. That split is item 469's (ADR
//! 0051 § 1); until it lands the attribute is the state, as it has been
//! since stage 1, read through [`crate::value::checked`], and only where it
//! is read and written changes when it does — not this sequence.
//! *Indeterminateness* is not held at all yet: nothing can set it without
//! the `indeterminate` property.
//!
//! # A radio button group
//!
//! A radio's group is the radios in the same tree with the same non-empty
//! `name` **and the same form owner** ([`crate::owner`], ADR 0051 § 2), so
//! radios sharing a name in two forms are two groups.
//!
//! # What has an activation behaviour
//!
//! An `input` that is a checkbox or a radio, an `a` or `area` with an
//! `href` (following it), a `summary` that is its `details`' summary
//! (toggling the `details`' `open`, ADR 0048 § 4), and a button — a
//! `button`, or an `input` of type `submit`, `image` or `reset`. A **submit
//! button** that is in its document, is not disabled and has a form owner
//! submits that form ([`Follows::Submit`], ADR 0051 § 4); submitting is
//! [`crate::submission`]'s, run by whoever ran the click. A **reset
//! button**'s activation behaviour puts back each control's default, which
//! is item 469's, since until then the defaults are the state; and a
//! `button` of type `button` has none. Any button still counts for the
//! search in step 1: a click on a `<span>` inside a button inside a link is
//! the button's, not the link's, as in every browser.

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
    /// A button, whose activation behaviour is its form's.
    Button {
        /// The button.
        node: NodeId,
    },
    /// A `details`' summary, which nothing changes before the click: its
    /// behaviour is activation behaviour, run by [`after`] only for a click
    /// nobody cancelled.
    Summary {
        /// The `summary`.
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
            | Activation::Button { node }
            | Activation::Summary { node } => Some(*node),
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
    /// A submit button was activated: submit its form owner from it (ADR
    /// 0051 § 4), which [`crate::submission`] and [`crate::constraint`]
    /// say how to do.
    Submit {
        /// The form.
        form: NodeId,
        /// The button that submits it.
        submitter: NodeId,
    },
    /// A `details` was opened or closed by its summary. HTML queues a
    /// `toggle` event at it, which is script's and queue item 434's; the
    /// page is to be drawn again either way.
    Toggled {
        /// The `details`.
        details: NodeId,
        /// Whether it is open now.
        open: bool,
    },
}

/// Which kind of activation behaviour an element has.
enum Behaviour {
    Checkbox,
    Radio,
    Link,
    Button,
    Summary,
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
        if crate::kind::of(element).is_some_and(crate::kind::Kind::is_button) {
            return Some(Behaviour::Button);
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
    if element.name.is_html("summary") && crate::details::details_of(document, node).is_some() {
        return Some(Behaviour::Summary);
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
        Some(Behaviour::Summary) => Activation::Summary { node },
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
        Activation::None
        | Activation::Link { .. }
        | Activation::Button { .. }
        | Activation::Summary { .. } => {}
    }
}

/// Step 3, when nobody cancelled the click: the activation behaviour, and
/// what it says follows.
///
/// A checkbox or radio fires `input` and `change` **only if it is in its
/// document** (the standard's *connected*): a box a listener took out of
/// the page changes, and nobody is told. A link is followed wherever it is.
///
/// A submit button that is in its document, is not disabled and has a form
/// owner says to submit that form.
///
/// A summary toggles its `details`' `open` attribute — **if it is still the
/// summary for its parent `details`**, which HTML asks now rather than when
/// the click began, so a listener that moved it decides.
pub fn after(document: &mut Document, activation: &Activation) -> Follows {
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
        Activation::Summary { node } => {
            let Some(details) = crate::details::details_of(document, *node) else {
                return Follows::Nothing;
            };
            match crate::details::toggle(document, details) {
                Some(open) => Follows::Toggled { details, open },
                None => Follows::Nothing,
            }
        }
        Activation::Button { node } => {
            submits(document, *node).map_or(Follows::Nothing, |(form, submitter)| Follows::Submit {
                form,
                submitter,
            })
        }
        Activation::None => Follows::Nothing,
    }
}

/// The form a press on the button `node` submits, and the button: when it
/// is a submit button, in its document, not disabled, with a form owner.
pub fn submits(document: &Document, node: NodeId) -> Option<(NodeId, NodeId)> {
    let element = document.element(node)?;
    if !crate::kind::is_submit_button(element)
        || !document.is_attached(node)
        || crate::disabled::is_disabled(document, node, element)
    {
        return None;
    }
    crate::owner::form_owner(document, node).map(|form| (form, node))
}

/// Whether a box is checked ([`crate::value::checked`]).
fn checked(document: &Document, node: NodeId) -> bool {
    crate::value::checked(document, node)
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

/// The radio button group `node` is in, itself included, in tree order —
/// empty if it has no group: no `name`, or an empty one. The same tree, the
/// same name and the same form owner.
pub(crate) fn group(document: &Document, node: NodeId) -> Vec<NodeId> {
    let Some(name) = document
        .element(node)
        .and_then(|element| element.attr("name"))
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
    else {
        return Vec::new();
    };
    let root = crate::owner::root_of(document, node);
    let owner = crate::owner::form_owner(document, node);
    core::iter::once(root)
        .chain(document.descendants(root))
        .filter(|other| {
            document.element(*other).is_some_and(|element| {
                element.name.is_html("input")
                    && element
                        .attr("type")
                        .is_some_and(|kind| kind.eq_ignore_ascii_case("radio"))
                    && element.attr("name") == Some(name.as_str())
            }) && crate::owner::form_owner(document, *other) == owner
        })
        .collect()
}
