/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's main button on a page (ADR 0024 § 5, ADR 0018 § 3, queue item
//! 535).
//!
//! The browser process sent where the button went, in the page's CSS
//! pixels ([`crate::pointer`]). Which element that is over is the hit
//! test's, against the drawing the person sees ([`crate::hit`]); what is
//! fired at it, and in what order, is the event loop's, measured from
//! Chrome 154 (`event_loop/pointed.rs`).
//!
//! # A press, then a release
//!
//! A **press** over nothing — outside the viewport — fires nothing and is
//! not remembered. Over an element it is remembered: the element, and
//! whether its `pointerdown` was cancelled.
//!
//! A **release** goes to the element under it, or to the root element when
//! it is over nothing, as Chrome's does outside its viewport. Its `click`
//! goes to the nearest node that both the pressed and the released element
//! are inside, the pressed one included — and is not fired when there was
//! no press, or when the pressed element has left its document since.
//!
//! # A page that never ran script
//!
//! Nobody listens, so nothing is dispatched and no heap is made to find
//! that out (ADR 0018 § 3). The click still does what a click does to the
//! document — `alo-dom`'s activation steps: a box ticked, a `<details>`
//! opened, a link followed — and the renderer asks for the link itself, as
//! an agent's `Activate` on such a page does. ARIA state is not changed:
//! that is the stage 1 accommodation for an agent's verbs (ADR 0018 § 7),
//! and a person's click is not one.
//!
//! # A page whose script has stopped
//!
//! As for an agent's press ([`crate::press`]): nobody hears it, the click's
//! activation steps still run, and the answer says so.
//!
//! # A form
//!
//! A click whose activation would submit a form submits nothing yet, and
//! says so: a person's submission is queue item 543.

use alo_bindings::navigating::{self, By, Navigation, NotFollowed};
use alo_dom::activation::{self, Follows};
use alo_dom::{Document, NodeId};
use alo_paint::Point;
use alo_url::Url;

use crate::event_loop::Unqueued;
use crate::held::Held;
use crate::hit;
use crate::pipeline::Drawing;
use crate::run_to::{Said, run_to};

/// What a renderer remembers of a press until its release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Pressing {
    /// The element pressed.
    node: NodeId,
    /// Whether the release may fire a `mouseup`: not when the press's
    /// `pointerdown` was cancelled.
    mouse: bool,
}

/// What the line about a submission that did not happen says.
const NOT_SUBMITTED: &str =
    "it would have submitted a form, and a person's click submits none yet (queue item 543)";

/// Press the main button at `at` on the page `held`, drawn as `drawing`:
/// what to remember until the release, and what was said.
pub(crate) fn down(
    held: &mut Held,
    drawing: &Drawing,
    at: Point,
) -> (Option<Pressing>, Vec<String>) {
    let Some(node) = held
        .document()
        .and_then(|document| hit::element_at(drawing, document, at))
    else {
        return (None, Vec::new());
    };
    let mut said = Said::about("the person's press");
    let mouse = match held.press(node, at) {
        Ok(Some(seq)) => run_to(held, seq, &mut said)
            .and_then(|turn| turn.pressed)
            .is_none_or(|pressed| !pressed.canceled),
        Ok(None) => true,
        Err(Unqueued::Stopped(why)) => {
            said.say(&format_args!("nobody heard it: {why}"));
            true
        }
        Err(why @ (Unqueued::NoSuchNode(_) | Unqueued::NotADocument)) => {
            said.say(&why);
            return (None, said.lines());
        }
    };
    (Some(Pressing { node, mouse }), said.lines())
}

/// Let go of the main button at `at` on the page `held`, at `url`, drawn as
/// `drawing`, after `pressing`: what was said, and — on a page that never
/// ran script — the link its click followed, for the renderer to ask for.
pub(crate) fn up(
    held: &mut Held,
    drawing: &Drawing,
    url: &Url,
    at: Point,
    pressing: Option<Pressing>,
) -> (Vec<String>, Option<Result<Navigation, NotFollowed>>) {
    let Some((node, click)) = held.document().and_then(|document| {
        let node =
            hit::element_at(drawing, document, at).or_else(|| document.document_element())?;
        let click = pressing.and_then(|pressed| shared(document, pressed.node, node));
        Some((node, click))
    }) else {
        return (Vec::new(), None);
    };
    let mouse = pressing.is_none_or(|pressed| pressed.mouse);
    let mut said = Said::about("the person's click");
    let mut followed = None;
    match held.release(node, at, mouse, click) {
        Ok(Some(seq)) => {
            let follows = run_to(held, seq, &mut said)
                .and_then(|turn| turn.clicked)
                .filter(|clicked| !clicked.canceled)
                .map(|clicked| clicked.follows);
            if matches!(follows, Some(Follows::Submit { .. })) {
                said.say(&NOT_SUBMITTED);
            }
        }
        Ok(None) => {
            if let Some(click) = click {
                followed = activated(held, click, &mut said).and_then(|link| {
                    held.document()
                        .map(|document| navigating::follow(document, url, link, By::Browser))
                });
            }
        }
        Err(Unqueued::Stopped(why)) => {
            if let Some(click) = click
                && let Some(link) = activated(held, click, &mut said)
            {
                held.follow(link);
            }
            said.say(&format_args!("nobody heard it: {why}"));
        }
        Err(why @ (Unqueued::NoSuchNode(_) | Unqueued::NotADocument)) => said.say(&why),
    }
    (said.lines(), followed)
}

/// Run the activation steps of a click at `click` that nobody heard: the
/// link it follows, if it is one.
fn activated(held: &mut Held, click: NodeId, said: &mut Said) -> Option<NodeId> {
    let follows = held.change(|document| {
        let done = activation::before(document, click);
        activation::after(document, &done)
    })?;
    match follows {
        Follows::Link { node, .. } => Some(node),
        Follows::Submit { .. } => {
            said.say(&NOT_SUBMITTED);
            None
        }
        Follows::Nothing | Follows::InputAndChange(_) | Follows::Toggled { .. } => None,
    }
}

/// Where the click of a press on `pressed` let go over `released` goes: the
/// nearest node both are inside, each counting as inside itself.
///
/// Nowhere, when `pressed` has left `document` since: `released` was found
/// by the hit test, so it is in the document, and a node that has left it
/// shares no ancestor with anything that has not.
fn shared(document: &Document, pressed: NodeId, released: NodeId) -> Option<NodeId> {
    let above_pressed = inclusive_ancestors(document, pressed);
    inclusive_ancestors(document, released)
        .into_iter()
        .find(|node| above_pressed.contains(node))
}

/// `node`, its parent, and so on up to the node with none.
fn inclusive_ancestors(document: &Document, node: NodeId) -> Vec<NodeId> {
    let mut found = vec![node];
    let mut next = document.parent(node);
    while let Some(parent) = next {
        found.push(parent);
        next = document.parent(parent);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document of `html`, and the element whose `id` is each of `ids`.
    fn parsed(html: &str, ids: &[&str]) -> (Document, Vec<NodeId>) {
        let document = alo_dom::parse_document(html);
        let found = ids
            .iter()
            .map(|id| {
                document
                    .descendants(document.root())
                    .find(|node| {
                        document
                            .element(*node)
                            .is_some_and(|element| element.attr("id") == Some(*id))
                    })
                    .unwrap_or_else(|| panic!("no element {id}"))
            })
            .collect();
        (document, found)
    }

    #[test]
    fn a_click_goes_to_the_nearest_node_both_are_inside() {
        let (document, found) = parsed(
            "<body id=body><div id=b><span id=s>a</span><i id=i>b</i></div><p id=o>c</p>",
            &["body", "b", "s", "i", "o"],
        );
        let [body, b, s, i, o] = found.as_slice() else {
            panic!("five elements");
        };
        assert_eq!(shared(&document, *s, *s), Some(*s), "the same element");
        assert_eq!(shared(&document, *s, *i), Some(*b), "two siblings");
        assert_eq!(shared(&document, *s, *b), Some(*b), "its own parent");
        assert_eq!(shared(&document, *b, *s), Some(*b), "its own child");
        assert_eq!(shared(&document, *s, *o), Some(*body), "across the page");
    }

    #[test]
    fn no_click_once_the_pressed_element_has_left_its_document() {
        let (mut document, found) = parsed("<div id=b><span id=s>a</span></div>", &["b", "s"]);
        let [b, s] = found.as_slice() else {
            panic!("two elements");
        };
        assert!(document.remove(*s));
        assert_eq!(shared(&document, *s, *b), None);
    }
}
