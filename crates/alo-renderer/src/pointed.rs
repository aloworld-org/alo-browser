/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's pointer on a page: its main button (ADR 0024 § 5, ADR 0018
//! § 3, queue item 535) and its moving (queue item 542).
//!
//! The browser process sent where the pointer went, in the page's CSS
//! pixels ([`crate::pointer`]). Which element that is over is the hit
//! test's, against the drawing the person sees ([`crate::hit`]); what is
//! fired at it, and in what order, is the event loop's, measured from
//! Chrome 154 (`event_loop/pointed.rs`, `event_loop/crossed.rs`).
//!
//! # The node under the pointer
//!
//! The renderer remembers the node last under the pointer ([`Pointed`]).
//! Every message the pointer sends — a move, a press, a release — looks
//! again, and fires the crossing between the two if they differ
//! ([`crate::crossing`]): a press where nothing was said of the pointer
//! before enters everything above what it pressed, as Chrome's does.
//!
//! A **move** goes to the element under the point; outside the viewport it
//! goes nowhere, leaving everything and firing no move — unless the main
//! button is down, when, as Chrome's, it is over the root element, as a
//! release there is. **Leaving** the page — over the browser's own drawing,
//! or off the window — goes nowhere.
//!
//! A node last under the pointer that has **left its document** since is
//! not told the pointer left it: at the next message, before anything
//! else, a task of its own fires Chrome's crossing from its nearest
//! ancestor still in the document, at the point the pointer was last at
//! ([`Crossing::after_removal`]). Chrome fires that on a timer even when
//! the pointer does not move, which is queue item 552.
//!
//! # What is hovered
//!
//! The node remembered is the document's hovered element too, set before
//! the crossing to it is fired, so that `:hover` matches it and its
//! ancestors from the crossing's first listener on, and the page is drawn
//! again with it (queue item 550, [`alo_dom::hover`]). Taking it out of the
//! document hovers the parent it left at once — the document does that —
//! and the pointer's next look hovers what is under it then.
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
//! opened, a link followed, a form submitted — and the renderer asks for
//! the link or the form itself, as an agent's `Activate` on such a page
//! does. ARIA state is not changed:
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
//! A click whose activation submits a form submits it (queue item 543): in
//! the click's task on a page that runs script, and by `alo-dom`'s rules
//! alone on a page nobody listens on. It is never refused, as an agent's
//! press can be; a form not sent is said ([`crate::sent`]).

use alo_bindings::navigating::{self, By, Navigation, NotFollowed};
use alo_dom::activation::{self, Follows};
use alo_dom::{Document, NodeId};
use alo_paint::Point;
use alo_url::Url;

use crate::crossing::{self, Crossing};
use crate::event_loop::{Moving, Unqueued};
use crate::held::Held;
use crate::hit;
use crate::pipeline::Drawing;
use crate::run_to::{Said, run_to};
use crate::sent::{self, Unheard};

/// What a renderer remembers of a person's press until its release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pressing {
    /// The element pressed.
    node: NodeId,
    /// Whether the release may fire a `mouseup`, and a move before it a
    /// `mousemove`: not when the press's `pointerdown` was cancelled.
    mouse: bool,
}

/// What a renderer remembers of a person's pointer between messages: for
/// one page, and nothing once it goes.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Pointed {
    /// The press, until its release.
    pressing: Option<Pressing>,
    /// Whether the main button is down: from a press the page was sent
    /// over one of its elements until the release.
    down: bool,
    /// The node last under the pointer: [`None`] before anything was said
    /// of it, and once it has gone outside the viewport or off the page.
    under: Option<NodeId>,
    /// That node and its ancestors as they were when it was found under the
    /// pointer, innermost first: where to look for one still in the
    /// document if it leaves it.
    above: Vec<NodeId>,
    /// Where the pointer was last said to be.
    at: Option<Point>,
}

impl Pointed {
    /// Remember `node` as the node last under the pointer, in `held`'s
    /// document, with its ancestors as they are now — and tell the document,
    /// so that `:hover` matches it before anything is fired at it, as
    /// Chrome's does ([`alo_dom::hover`]).
    fn remember(&mut self, held: &mut Held, node: Option<NodeId>) {
        self.under = node;
        self.above = match (held.document(), node) {
            (Some(document), Some(node)) => crossing::inclusive_ancestors(document, node),
            _ => Vec::new(),
        };
        held.change(|document| document.hover(node));
    }

    /// `buttons`, as UI Events' bits.
    fn buttons(&self) -> u16 {
        u16::from(self.down)
    }

    /// Whether a move may fire a `mousemove`: not while a cancelled
    /// `pointerdown`'s button is down.
    fn mouse(&self) -> bool {
        self.pressing.is_none_or(|pressed| pressed.mouse)
    }

    /// The pointer moved to `at` on the page `held`, drawn as `drawing`:
    /// what was said.
    pub(crate) fn moved(&mut self, held: &mut Held, drawing: &Drawing, at: Point) -> Vec<String> {
        let mut said = Said::about("the person's pointer");
        self.settle(held, drawing, &mut said);
        let over = held.document().and_then(|document| {
            hit::element_at(drawing, document, at)
                .or_else(|| self.down.then(|| document.document_element()).flatten())
        });
        self.go(held, at, over, &mut said);
        said.lines()
    }

    /// The pointer left the page, last at `at`: what was said.
    pub(crate) fn left(&mut self, held: &mut Held, drawing: &Drawing, at: Point) -> Vec<String> {
        let mut said = Said::about("the person's pointer");
        self.settle(held, drawing, &mut said);
        self.go(held, at, None, &mut said);
        said.lines()
    }

    /// Fire the crossing from the node last under the pointer to `over`,
    /// and a move at `over` if it is a node, and remember them.
    fn go(&mut self, held: &mut Held, at: Point, over: Option<NodeId>, said: &mut Said) {
        let crossing = held
            .document()
            .and_then(|document| Crossing::between(document, self.under, over));
        self.remember(held, over);
        self.at = Some(at);
        if crossing.is_none() && over.is_none() {
            return;
        }
        let moving = Moving {
            at,
            crossing,
            over,
            buttons: self.buttons(),
            mouse: self.mouse(),
        };
        let queued = held.moved(moving);
        heard(held, queued, said);
    }

    /// Before anything else, if the node last under the pointer has left
    /// its document: Chrome's crossing from its nearest ancestor still in
    /// it, at the point the pointer was last at, as a task of its own.
    fn settle(&mut self, held: &mut Held, drawing: &Drawing, said: &mut Said) {
        let (Some(under), Some(at)) = (self.under, self.at) else {
            return;
        };
        let Some((crossing, now)) = held.document().and_then(|document| {
            if crossing::is_connected(document, under) {
                return None;
            }
            let now = hit::element_at(drawing, document, at)
                .or_else(|| self.down.then(|| document.document_element()).flatten());
            Some((Crossing::after_removal(document, &self.above, now), now))
        }) else {
            return;
        };
        self.remember(held, now);
        if crossing.is_empty() {
            return;
        }
        let moving = Moving {
            at,
            crossing: Some(crossing),
            over: None,
            buttons: self.buttons(),
            mouse: self.mouse(),
        };
        let queued = held.moved(moving);
        heard(held, queued, said);
    }

    /// Press the main button at `at` on the page `held`, drawn as `drawing`:
    /// what was said.
    pub(crate) fn down(&mut self, held: &mut Held, drawing: &Drawing, at: Point) -> Vec<String> {
        let mut said = Said::about("the person's press");
        self.settle(held, drawing, &mut said);
        let Some((node, crossing)) = held.document().and_then(|document| {
            let node = hit::element_at(drawing, document, at)?;
            Some((node, Crossing::between(document, self.under, Some(node))))
        }) else {
            return said.lines();
        };
        self.remember(held, Some(node));
        self.at = Some(at);
        self.down = true;
        let mouse = match held.press(node, at, crossing) {
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
                self.down = false;
                return said.lines();
            }
        };
        self.pressing = Some(Pressing { node, mouse });
        said.lines()
    }

    /// Let go of the main button at `at` on the page `held`, at `url`, drawn
    /// as `drawing`: what was said, and — on a page nobody listens on — the
    /// link its click followed or the form it submitted, for the renderer to
    /// ask for.
    pub(crate) fn up(
        &mut self,
        held: &mut Held,
        drawing: &Drawing,
        url: &Url,
        at: Point,
    ) -> (Vec<String>, Option<Result<Navigation, NotFollowed>>) {
        let mut said = Said::about("the person's click");
        self.settle(held, drawing, &mut said);
        let pressing = self.pressing.take();
        self.down = false;
        let Some((node, click, crossing)) = held.document().and_then(|document| {
            let node =
                hit::element_at(drawing, document, at).or_else(|| document.document_element())?;
            let click = pressing.and_then(|pressed| shared(document, pressed.node, node));
            Some((
                node,
                click,
                Crossing::between(document, self.under, Some(node)),
            ))
        }) else {
            return (said.lines(), None);
        };
        self.remember(held, Some(node));
        self.at = Some(at);
        let mouse = pressing.is_none_or(|pressed| pressed.mouse);
        let mut followed = None;
        match held.release(node, at, mouse, click, crossing) {
            Ok(Some(seq)) => {
                let submitted = run_to(held, seq, &mut said)
                    .and_then(|turn| turn.clicked)
                    .and_then(|clicked| clicked.submitted);
                if let Some(line) = submitted
                    .as_ref()
                    .and_then(|submitted| sent::said(submitted, held.document()))
                {
                    said.say(&line);
                }
            }
            Ok(None) => {
                if let Some(click) = click {
                    followed = activated(held, url, click, &mut said);
                }
            }
            Err(Unqueued::Stopped(why)) => {
                if let Some(click) = click {
                    followed = activated(held, url, click, &mut said);
                }
                said.say(&format_args!("nobody heard it: {why}"));
            }
            Err(why @ (Unqueued::NoSuchNode(_) | Unqueued::NotADocument)) => said.say(&why),
        }
        (said.lines(), followed)
    }
}

/// Run the task `queued` to its end, if it was queued, saying what its
/// script said — and, if the page has stopped, that nobody heard it.
fn heard(
    held: &mut Held,
    queued: Result<Option<crate::event_loop::Seq>, Unqueued>,
    said: &mut Said,
) {
    match queued {
        Ok(Some(seq)) => {
            run_to(held, seq, said);
        }
        Ok(None) => {}
        Err(Unqueued::Stopped(why)) => said.say(&format_args!("nobody heard it: {why}")),
        Err(why @ (Unqueued::NoSuchNode(_) | Unqueued::NotADocument)) => said.say(&why),
    }
}

/// Run the activation steps of a click at `click` that nobody heard, on the
/// page `held` at `url`: the link it follows or the form it submits, if it
/// is either, for the renderer to ask for; a form not sent is said.
fn activated(
    held: &mut Held,
    url: &Url,
    click: NodeId,
    said: &mut Said,
) -> Option<Result<Navigation, NotFollowed>> {
    let follows = held.change(|document| {
        let done = activation::before(document, click);
        activation::after(document, &done)
    })?;
    let document = held.document()?;
    match follows {
        Follows::Link { node, .. } => Some(navigating::follow(document, url, node, By::Browser)),
        Follows::Submit { form, submitter } => {
            match sent::unheard(document, url, (form, submitter)) {
                Unheard::Asked(asked) => Some(asked),
                Unheard::NotSent(why) => {
                    said.say(&why);
                    None
                }
            }
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
    let above_pressed = crossing::inclusive_ancestors(document, pressed);
    crossing::inclusive_ancestors(document, released)
        .into_iter()
        .find(|node| above_pressed.contains(node))
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
