/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ★ The verbs, through Chrome (ADR 0058 § 4, queue item 621).
//!
//! A verb on a page in Chrome is **decided here and carried out there**. The
//! decision is `alo-agent`'s, made against a [`ChromeReading`] of the page as
//! it is now — the same [`perform`] this engine's renderer calls, so every
//! refusal in `alo_agent::verb` is reached by the same rule, and a verb that
//! is refused is refused **before a single command goes to Chrome**. What is
//! carried out is what was decided, and nothing else.
//!
//! # A real click and real typing, never script
//!
//! Carried out through CDP's `Input` domain, so the page hears what it hears
//! from a person, as trusted events, and runs its own checks — its listeners,
//! its validation, its `beforeinput`. No script is ever put into the page:
//! script skips the page's own checks, and a page can tell.
//!
//! - **`activate`** is a click at the middle of the box an agent named, after
//!   the box is scrolled into the window. The point is **found from the node,
//!   never given** (ADR 0002): no verb takes a coordinate, and before the
//!   click Chrome is asked what a person pressing that point would press. If
//!   it is not the node or something inside it — an overlay over it, a box
//!   moved over it — nothing is pressed and the answer is
//!   [`Unacted::Covered`].
//! - **`put_text`** gives the field the keyboard (`DOM.focus`), selects what
//!   it holds with the key a person would use, and replaces it as typing
//!   does (`Input.insertText`), which the page hears as `beforeinput` and
//!   `input`; `change` follows when the keyboard moves on, as it does for a
//!   person. The empty text is a Backspace over the selection.
//!
//!   A **date field** is three segments in Chrome that typing's text does
//!   not reach, so a date goes in as a person puts one in: each segment, in
//!   the order Chrome lays them out, given the keyboard and its part typed
//!   as digits, and the empty text is a Backspace in each
//!   ([`crate::chrome_date`], item 632). The page hears a person's keys,
//!   and an `input` and a `change` as each segment changes the field's
//!   value. A date Chrome's field would not hold as written is not typed
//!   at all ([`Unacted::Untypable`]).
//! - **`scroll`** is never decided on Chrome's page: its tree cannot say that
//!   a box scrolls ([`crate::chrome_tree`]), so every scroll is refused as
//!   `DoesNotScroll`, by the rule that refuses it on this engine's page.
//!
//! # What came of a press
//!
//! A press that sends a form answers [`Outcome::Submitted`], and one that
//! follows a link [`Outcome::Followed`] — each only when Chrome told of the
//! page's frame asking to navigate (`Page.frameRequestedNavigation`) to the
//! tab it is in. Otherwise it answers [`Outcome::Activated`], as the engine
//! does for a click the page cancelled.
//!
//! A link to a part of the page asks Chrome for no navigation: Chrome moves
//! the frame within its document in the click's task and tells of it after
//! (`Page.navigatedWithinDocument`, measured against Chrome 155, item 640).
//! So a press of a link whose frame moved, as a `fragment`, to the address
//! the link goes to answers [`Outcome::Followed`] too, as the engine does.
//!
//! Chrome tells of that request from the task the click ran in, on the same
//! channel the renderer answers on; so once the renderer has answered a
//! question asked after the click, a request the click made has been told
//! of. Measured against Chrome 155, including a page that queued half a
//! second of its own work in the click before the form was sent (item 621).
//!
//! # Whose requests they are
//!
//! A verb is an action from the moment it is accepted, refused or not, and
//! [`Tab::acted`] names it. While it is carried out, what the page asks for
//! is the agent's in the record, and so is a navigation the page's frame
//! asked for in that time, whenever Chrome pauses its first hop
//! ([`crate::chrome_record`], ADR 0012 § 4).
//!
//! A press on a submit button that sends nothing, and after which Chrome
//! shows the bubble it draws over a control failing its checks, answers
//! [`Outcome::NotSubmitted`]: the control the bubble is about, and its
//! words, read from Chrome's tree ([`crate::chrome_bubble`], item 631).
//! Chrome tells of one control, the first, and of no constraint, so the
//! answer names one and says [`None`] for which constraint, where the
//! engine names each control that failed and its constraint.
//!
//! Beside it, and alone when there is no bubble, each failing control
//! Chrome could not give the keyboard — a `hidden` one, one inside an
//! `inert` part of the page — which Chrome logs rather than shows
//! ([`crate::chrome_unfocusable`], item 646): with Chrome's words about it
//! and no constraint, and with no box when the tree read before the press
//! does not hold it. So `Log` is on only while a press on a submit button
//! is carried out, and what it logged before is forgotten first.
//!
//! # What the page's script said
//!
//! From the moment a verb is decided until it is answered, the tab hears
//! what the page's script writes to its console and what it throws and
//! nobody catches ([`Tab::act_and_hear`], [`crate::chrome_said`], queue
//! item 639), in its own document and in each frame from another site and
//! each worker of it — dedicated, shared or service — which Chrome tells of
//! in targets of their own (items 652 and 654) — and only then: Chrome is told to forget what was said
//! before, so nothing earlier is told of again, and to stop telling once
//! the verb is done, which the page's own renderer answers after everything
//! it told of before, as each frame and worker does for its own. A verb
//! refused is refused before anything is heard.
//!
//! # What it cannot say yet
//!
//! - **A form whose failing control's `invalid` the page cancels** answers
//!   [`Outcome::Activated`], as one whose `submit` it cancels does: Chrome
//!   tells of the two alike, in its tree, its log and its events, and in
//!   both the page took the press (ADR 0051's amendment of 25 October,
//!   item 646). The engine, which runs the checks itself, answers
//!   [`Outcome::NotSubmitted`].
//! - **Text the page would not take** is answered [`Outcome::TextCanceled`]
//!   when the field holds afterwards what it held before. Whether the page
//!   cancelled the `beforeinput` or put the old text back in its `input`
//!   listener, Chrome does not say, and both leave the field as it was.

use crate::chrome::Failed;
use crate::chrome_bubble;
use crate::chrome_date::{self, Digits, Part};
use crate::chrome_protocol::{self, AxNode, Chord, Command, Digit, Mouse, Within};
use crate::chrome_reading::ChromeReading;
use crate::chrome_tab::Tab;
use crate::chrome_tree::Unread;
use crate::chrome_unfocusable::Unfocusable;
use alo_agent::verb::{Invalid, Target, Verb, find, perform};
use alo_agent::{Outcome, Reading, Refusal};
use alo_box::BoxId;
use alo_dom::submission::Method;
use alo_net::cause::ActionId;
use alo_url::Url;
use core::fmt;
use std::collections::HashSet;

/// Why a verb on a page in Chrome was not carried out.
#[derive(Debug, Clone, PartialEq)]
pub enum Unacted {
    /// It was refused, by the same rule that refuses it on this engine's
    /// page, and nothing was sent to Chrome.
    Refused(Refusal),
    /// The page could not be read to decide it.
    Unread(Unread),
    /// Asking Chrome failed.
    Failed(Failed),
    /// Chrome drew the node nowhere, so there is nothing a person could
    /// press.
    Undrawn {
        /// Which node.
        node: BoxId,
    },
    /// A person pressing the middle of the node would press something else,
    /// so nothing was pressed.
    Covered {
        /// Which node.
        node: BoxId,
        /// What is there instead, by Chrome's `backendNodeId`, or [`None`]
        /// when Chrome finds nothing at the point.
        by: Option<u64>,
    },
    /// The tab's frame committed another document while the verb was being
    /// decided, so it was decided on a page that is not there and nothing
    /// was done (queue item 630). A page may go elsewhere on its own — a
    /// script, a refresh — while it is read for the verb, and the verb would
    /// otherwise be carried out on whatever arrived, on a site that may not
    /// be the one the agent was let reach.
    Moved {
        /// Where the tab is now, as Chrome spells it, if this browser can
        /// read it as an address.
        to: Option<String>,
    },
    /// A date was decided for a date field, and Chrome's field would not
    /// hold it as it was written, or shows segments a date's parts cannot be
    /// typed into without guessing which is which; so no key was sent
    /// (queue item 632).
    Untypable {
        /// Which node.
        node: BoxId,
        /// Why.
        why: chrome_date::Untypable,
    },
    /// It was decided, and is not a thing this path carries out.
    ///
    /// Reached only by an outcome no decision on Chrome's page makes today —
    /// a scroll, which its tree cannot say a box takes — and kept as an
    /// answer rather than assumed impossible.
    Uncarried {
        /// What was decided.
        outcome: Outcome,
    },
}

impl fmt::Display for Unacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refusal) => refusal.fmt(f),
            Self::Unread(why) => why.fmt(f),
            Self::Failed(why) => why.fmt(f),
            Self::Undrawn { node } => write!(f, "Chrome drew {node} nowhere"),
            Self::Covered { node, by: Some(by) } => write!(
                f,
                "the middle of {node} is under Chrome's node {by}, so nothing was pressed"
            ),
            Self::Covered { node, by: None } => write!(
                f,
                "Chrome finds nothing at the middle of {node}, so nothing was pressed"
            ),
            Self::Moved { to: Some(to) } => write!(
                f,
                "the page went to {to} while the verb was decided, so nothing was done"
            ),
            Self::Moved { to: None } => f.write_str(
                "the page went somewhere else while the verb was decided, so nothing was done",
            ),
            Self::Untypable { node, why } => {
                write!(f, "{why}, so nothing was typed into {node}")
            }
            Self::Uncarried { outcome } => {
                write!(
                    f,
                    "decided — {outcome} — and not carried out through Chrome"
                )
            }
        }
    }
}

impl std::error::Error for Unacted {}

impl From<Failed> for Unacted {
    fn from(why: Failed) -> Self {
        Self::Failed(why)
    }
}

impl From<Unread> for Unacted {
    fn from(why: Unread) -> Self {
        Self::Unread(why)
    }
}

impl From<chrome_protocol::Malformed> for Unacted {
    fn from(why: chrome_protocol::Malformed) -> Self {
        Self::Failed(Failed::Malformed(why))
    }
}

impl Tab<'_> {
    /// Do `verb` to what `target` names on the page, as it is now.
    ///
    /// # Errors
    ///
    /// [`Unacted`]: a [`Refusal`], decided before Chrome is asked to do
    /// anything; or why it was not carried out.
    pub fn act(&mut self, target: &Target, verb: &Verb) -> Result<Outcome, Unacted> {
        self.act_and_hear(target, verb).map(|(outcome, _)| outcome)
    }

    /// [`Tab::act`], and what the page's script said while the verb ran, as
    /// an answer's `issues` ([`crate::chrome_said`], queue item 639).
    ///
    /// # Errors
    ///
    /// As [`Tab::act`].
    pub fn act_and_hear(
        &mut self,
        target: &Target,
        verb: &Verb,
    ) -> Result<(Outcome, Vec<String>), Unacted> {
        self.act_as_action(target, verb).1
    }

    /// [`Tab::act_and_hear`], and the action the verb was accepted as.
    pub(crate) fn act_as_action(
        &mut self,
        target: &Target,
        verb: &Verb,
    ) -> (ActionId, Result<(Outcome, Vec<String>), Unacted>) {
        // Accepted, and so an action, whether it is refused or carried out
        // (ADR 0012 § 4): what the page asks for until it answers is the
        // agent's, and so is a navigation it asked for (`chrome_record`).
        let action = self.acting();
        let acted = self.decide_and_carry_out(target, verb);
        // Heard to the end whatever came of it, so that Chrome is not left
        // telling of the page's script.
        let heard = self.heard();
        self.done_acting();
        let acted = match (acted, heard) {
            (Ok(outcome), Ok(issues)) => Ok((outcome, issues)),
            (Ok(_), Err(why)) => Err(Unacted::Failed(why)),
            (Err(why), _) => Err(why),
        };
        (action, acted)
    }

    /// [`Tab::act`], once the verb is an action.
    fn decide_and_carry_out(&mut self, target: &Target, verb: &Verb) -> Result<Outcome, Unacted> {
        let document = self.document();
        let (snapshot, nodes) = self.read_whole()?;
        let reading = ChromeReading::of(&snapshot);
        let aimed = find(&reading, target).map_err(Unacted::Refused)?;
        let dom = dom_of(aimed.id);
        let described = chrome_protocol::described(&self.ask(&Command::Describe { node: dom })?)?;
        let reading = reading.with_element(aimed.id, &described);
        let decided = perform(&reading, target, verb).map_err(Unacted::Refused)?;
        // Decided, and so heard from here: what the page's script says as
        // the verb is carried out is the answer's `issues`.
        self.hear(about(&decided))?;
        // The last word from Chrome before anything is carried out: every
        // message that arrived while the page was read has been taken.
        if self.document() != document {
            return Err(Unacted::Moved {
                to: self.address().map(ToString::to_string),
            });
        }
        match decided {
            Outcome::Activated { node, name } => {
                let pressed = Pressed {
                    name,
                    to: None,
                    submits: reading.submits(node),
                };
                self.press(&nodes, node, pressed)
            }
            Outcome::Followed { node, to } => {
                let pressed = Pressed {
                    name: aimed.name,
                    to: Some(to),
                    submits: false,
                };
                self.press(&nodes, node, pressed)
            }
            Outcome::TextPut { node, text } if reading.is_a_date_field(node) => {
                self.date_in(&nodes, node, text)
            }
            Outcome::TextPut { node, text } => self.type_in(node, text),
            // Never decided here yet: Chrome's reading names no option's
            // list until item 660 (`ChromeReading::list_of`).
            outcome @ (Outcome::Scrolled { .. }
            | Outcome::TextCanceled { .. }
            | Outcome::Chosen { .. }
            | Outcome::Submitted { .. }
            | Outcome::NotSubmitted { .. }) => Err(Unacted::Uncarried { outcome }),
        }
    }

    /// Click the middle of `node`, read in `nodes` before the press, and
    /// say what came of it.
    fn press(
        &mut self,
        nodes: &[AxNode],
        node: BoxId,
        pressed: Pressed,
    ) -> Result<Outcome, Unacted> {
        let Pressed { name, to, submits } = pressed;
        let dom = dom_of(node);
        // What the page asked for before this press is not what it did.
        drop(self.take_requested());
        drop(self.take_within());
        let before = self.address().cloned();
        self.ask(&Command::ScrollIntoView { node: dom })?;
        let rect = self
            .drawn(dom)?
            .into_iter()
            .next()
            .ok_or(Unacted::Undrawn { node })?;
        let (x, y) = middle(rect.left(), rect.top(), rect.size.width, rect.size.height)
            .ok_or(Unacted::Undrawn { node })?;
        let hit = match self.ask(&Command::NodeAt { x, y }) {
            Ok(result) => Some(chrome_protocol::node_at(&result)?),
            Err(Failed::Declined(_)) => None,
            Err(why) => return Err(why.into()),
        };
        if !hit.is_some_and(|hit| within(nodes, dom).contains(&hit)) {
            return Err(Unacted::Covered { node, by: hit });
        }
        let (x, y) = (pixels(x), pixels(y));
        // A press that may be refused is logged: Chrome tells of a failing
        // control it cannot focus only there. What it logged before is
        // forgotten first, or it would be told of again.
        if submits {
            drop(self.take_unfocusable());
            self.ask(&Command::ForgetTheLog)?;
            self.ask(&Command::TellOfTheLog)?;
        }
        let clicked = self.click(x, y);
        // No longer, whatever came of the click.
        if submits {
            let off = self.ask(&Command::StopTellingOfTheLog);
            clicked?;
            off?;
        } else {
            clicked?;
        }
        let asked = self
            .take_requested()
            .into_iter()
            .find(|requested| requested.disposition == "currentTab");
        let moves = self.take_within();
        Ok(match (asked, to) {
            (Some(asked), Some(to)) if asked.reason == "anchorClick" => {
                Outcome::Followed { node, to }
            }
            // A link to a part of the page asks for nothing: Chrome moves to
            // it in the click's task and says so after the fact.
            (None, Some(to)) if went_to_a_part(&moves, before.as_ref(), &to) => {
                Outcome::Followed { node, to }
            }
            (Some(asked), _) if asked.reason.starts_with("formSubmission") => Outcome::Submitted {
                node,
                method: if asked.reason == "formSubmissionPost" {
                    Method::Post
                } else {
                    Method::Get
                },
                to: asked.url,
            },
            (None, None) if submits => {
                let mut invalid: Vec<Invalid> = self
                    .bubble(nodes)?
                    .into_iter()
                    .map(|bubble| Invalid {
                        control: Some(box_of(bubble.control)),
                        constraint: None,
                        message: bubble.message,
                    })
                    .collect();
                invalid.extend(self.unfocusable(nodes)?);
                if invalid.is_empty() {
                    Outcome::Activated { node, name }
                } else {
                    Outcome::NotSubmitted { node, invalid }
                }
            }
            _ => Outcome::Activated { node, name },
        })
    }

    /// A press of the left button at `x`, `y`, answered once the renderer
    /// has answered a question asked after it — and so after anything the
    /// click asked for, or logged, was told of on the same channel.
    fn click(&mut self, x: f64, y: f64) -> Result<(), Unacted> {
        for kind in [Mouse::Moved, Mouse::Pressed, Mouse::Released] {
            self.ask(&Command::Mouse { kind, x, y })?;
        }
        self.ask(&Command::Window)?;
        Ok(())
    }

    /// Each control Chrome logged that it could not give the keyboard as it
    /// refused the press ([`crate::chrome_unfocusable`]), in the order it
    /// logged them: Chrome's words about it, and no constraint. Its box when
    /// the tree read before the press, `nodes`, holds it, and [`None`] — a
    /// control an agent cannot name — when it does not, as for a `hidden`
    /// one.
    fn unfocusable(&mut self, nodes: &[AxNode]) -> Result<Vec<Invalid>, Unacted> {
        let mut invalid = Vec::new();
        for Unfocusable { object, words } in self.take_unfocusable() {
            let described = self.ask(&Command::DescribeValue { object })?;
            let dom = chrome_protocol::backend_node(&described)?;
            let read = nodes
                .iter()
                .any(|read| read.node == Some(dom) && !read.ignored);
            invalid.push(Invalid {
                control: read.then(|| box_of(dom)),
                constraint: None,
                message: words,
            });
        }
        Ok(invalid)
    }

    /// The bubble Chrome shows now over a form it would not send, that was
    /// not in `before`.
    fn bubble(&mut self, before: &[AxNode]) -> Result<Option<chrome_bubble::Bubble>, Unacted> {
        let after = chrome_protocol::ax_nodes(&self.ask(&Command::ReadTree)?)?;
        Ok(chrome_bubble::shown(before, &after))
    }

    /// Replace what the field `node` holds with `text`, as a person typing
    /// does, and say whether the page took it.
    fn type_in(&mut self, node: BoxId, text: String) -> Result<Outcome, Unacted> {
        let dom = dom_of(node);
        let before = self.holds(dom)?;
        self.ask(&Command::Focus { node: dom })?;
        self.chord(Chord::SelectAll)?;
        if text.is_empty() {
            self.chord(Chord::DeleteBackward)?;
        } else {
            self.ask(&Command::InsertText { text: text.clone() })?;
        }
        let after = self.holds(dom)?;
        Ok(taken(node, text, &before, &after))
    }

    /// Put the date `text` into the date field `node` — or empty it, for
    /// the empty text — as a person does: each of its segments, found in
    /// `nodes` in the order Chrome lays them out, given the keyboard and
    /// typed into; and say whether the page took it.
    fn date_in(&mut self, nodes: &[AxNode], node: BoxId, text: String) -> Result<Outcome, Unacted> {
        let dom = dom_of(node);
        let untypable = |why| Unacted::Untypable { node, why };
        let digits = if text.is_empty() {
            None
        } else {
            Some(Digits::of(&text).map_err(untypable)?)
        };
        let mut segments = Vec::new();
        for segment in chrome_date::segments(nodes, dom) {
            let described =
                chrome_protocol::described(&self.ask(&Command::Describe { node: segment })?)?;
            let part = described
                .attributes
                .iter()
                .find(|(name, _)| name == "pseudo")
                .and_then(|(_, pseudo)| Part::of_pseudo(pseudo));
            segments.push((segment, part));
        }
        let order = chrome_date::laid_out(&segments).map_err(untypable)?;
        let before = self.holds(dom)?;
        for (segment, part) in order {
            // A segment given the keyboard starts again at the first digit
            // typed into it, so what it held is replaced.
            self.ask(&Command::Focus { node: segment })?;
            match &digits {
                Some(digits) => {
                    for digit in digits.of_part(part).chars().filter_map(Digit::of) {
                        self.chord(Chord::Digit(digit))?;
                    }
                }
                None => self.chord(Chord::DeleteBackward)?,
            }
        }
        let after = self.holds(dom)?;
        Ok(taken(node, text, &before, &after))
    }

    /// A key down and up again.
    fn chord(&mut self, chord: Chord) -> Result<(), Failed> {
        self.ask(&Command::Key { chord, down: true })?;
        self.ask(&Command::Key { chord, down: false })?;
        Ok(())
    }

    /// What the field `dom` holds now, as Chrome's tree says: nothing, when
    /// it says nothing.
    fn holds(&mut self, dom: u64) -> Result<String, Unacted> {
        let nodes = chrome_protocol::ax_nodes(&self.ask(&Command::ReadNode { node: dom })?)?;
        Ok(nodes
            .into_iter()
            .find(|node| node.node == Some(dom))
            .and_then(|node| node.value)
            .unwrap_or_default())
    }
}

/// What came of putting `text` into the field `node`, which held `before`
/// and holds `after`: the page did not take it when the field holds what
/// it held, and that was not already `text`.
/// What the lines a page's script says while `decided` is carried out are
/// about, in the engine's words for its own verbs (`alo-renderer`'s `press`
/// and `put`).
fn about(decided: &Outcome) -> &'static str {
    match decided {
        Outcome::TextPut { .. } | Outcome::TextCanceled { .. } => "the text",
        Outcome::Scrolled { .. } => "the scroll",
        Outcome::Activated { .. }
        | Outcome::Followed { .. }
        | Outcome::Chosen { .. }
        | Outcome::Submitted { .. }
        | Outcome::NotSubmitted { .. } => "the click",
    }
}

fn taken(node: BoxId, text: String, before: &str, after: &str) -> Outcome {
    if after == before && before != text {
        Outcome::TextCanceled { node, text }
    } else {
        Outcome::TextPut { node, text }
    }
}

/// What was pressed, beside the node: what it is called, where it goes
/// when it is a link, and whether it is a submit button.
struct Pressed {
    name: Option<String>,
    to: Option<String>,
    submits: bool,
}

/// The DOM node a node of the tree describes, by Chrome's `backendNodeId`:
/// the id [`crate::chrome_tree`] gave it.
fn dom_of(node: BoxId) -> u64 {
    u64::try_from(node.as_usize()).unwrap_or(u64::MAX)
}

/// The node of the tree that describes the DOM node `dom`, by the id
/// [`crate::chrome_tree`] gives it.
fn box_of(dom: u64) -> BoxId {
    BoxId::from_wire(usize::try_from(dom).unwrap_or(usize::MAX))
}

/// The whole CSS pixel at the middle of a rectangle — where a person aims —
/// or [`None`] for one with no area, or too far away to be a point.
fn middle(left: f32, top: f32, width: f32, height: f32) -> Option<(i64, i64)> {
    if !(width > 0.0 && height > 0.0) {
        return None;
    }
    let at = |start: f32, length: f32| {
        let point = (f64::from(start) + f64::from(length) / 2.0).floor();
        // Within ±2⁵³, where an `f64` holds every whole number exactly.
        (point.is_finite() && point.abs() < 9_007_199_254_740_992.0).then_some(point)
    };
    Some((whole(at(left, width)?), whole(at(top, height)?)))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "`middle` passes only whole numbers inside ±2⁵³, which an i64 holds exactly"
)]
fn whole(point: f64) -> i64 {
    point as i64
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a point is a pixel of a window, far inside the ±2⁵³ an f64 holds exactly"
)]
fn pixels(point: i64) -> f64 {
    point as f64
}

/// Whether the page's own frame, at `before` when it was pressed, moved in
/// `moves` to the part of itself the link `href` goes to: a `fragment` move
/// to the address `href` names, resolved against `before`. A move a script
/// made instead — `pushState`, or `location.hash` set somewhere else — is
/// not the link followed.
fn went_to_a_part(moves: &[Within], before: Option<&Url>, href: &str) -> bool {
    let Some(goes) = before.and_then(|before| alo_url::join(before, href).ok()) else {
        return false;
    };
    moves.iter().any(|moved| {
        moved.how == "fragment"
            && alo_url::parse(&moved.url).is_ok_and(|url| url.to_string() == goes.to_string())
    })
}

/// The DOM nodes Chrome's tree holds at or under the DOM node `dom`,
/// including those it ignores: what a click meant for `dom` may land on.
fn within(nodes: &[AxNode], dom: u64) -> HashSet<u64> {
    let by_id: std::collections::HashMap<&str, &AxNode> =
        nodes.iter().map(|node| (node.id.as_str(), node)).collect();
    let mut found = HashSet::new();
    let mut seen = HashSet::new();
    let mut waiting: Vec<&AxNode> = nodes.iter().filter(|node| node.node == Some(dom)).collect();
    // A tree read already ([`crate::chrome_tree`]) holds no loop; `seen`
    // keeps this from following one regardless.
    while let Some(node) = waiting.pop() {
        if !seen.insert(node.id.as_str()) {
            continue;
        }
        found.extend(node.node);
        waiting.extend(
            node.children
                .iter()
                .filter_map(|child| by_id.get(child.as_str()).copied()),
        );
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ax(id: &str, dom: Option<u64>, children: &[&str]) -> AxNode {
        AxNode {
            id: id.to_owned(),
            node: dom,
            children: children.iter().map(|child| (*child).to_owned()).collect(),
            ..AxNode::default()
        }
    }

    #[test]
    fn a_click_meant_for_a_node_may_land_on_anything_inside_it() {
        // A label holding a radio and its words; the words' text, ignored
        // by Chrome, is still inside.
        let nodes = [
            ax("1", Some(10), &["2", "5"]),
            ax("2", Some(20), &["3", "4"]),
            ax("3", Some(30), &[]),
            ax("4", None, &["6"]),
            ax("5", Some(50), &[]),
            ax("6", Some(60), &[]),
        ];
        assert_eq!(within(&nodes, 20), HashSet::from([20, 30, 60]));
        assert_eq!(within(&nodes, 50), HashSet::from([50]));
        assert!(within(&nodes, 99).is_empty(), "a node not in the tree");
        // A loop, which a tree that was read cannot hold, ends regardless.
        let looped = [ax("1", Some(1), &["2"]), ax("2", Some(2), &["1"])];
        assert_eq!(within(&looped, 1), HashSet::from([1, 2]));
    }

    #[test]
    fn a_click_is_aimed_at_the_whole_pixel_in_the_middle() {
        assert_eq!(middle(29.0, 242.59375, 13.0, 13.0), Some((35, 249)));
        assert_eq!(middle(0.0, 0.0, 1.0, 1.0), Some((0, 0)));
        assert_eq!(middle(-10.5, 3.0, 4.0, 2.0), Some((-9, 4)));
        assert_eq!(middle(0.0, 0.0, 0.0, 10.0), None, "no width");
        assert_eq!(middle(0.0, 0.0, 10.0, -1.0), None, "no height");
        assert_eq!(middle(f32::NAN, 0.0, 1.0, 1.0), None);
        assert_eq!(middle(0.0, 0.0, f32::INFINITY, 1.0), None);
        assert_eq!(middle(f32::MAX, 0.0, f32::MAX, 1.0), None, "past 2^53");
    }

    #[test]
    fn a_link_to_a_part_is_followed_only_to_where_it_goes() {
        let moved = |url: &str, how: &str| Within {
            frame: "F".to_owned(),
            url: url.to_owned(),
            how: how.to_owned(),
        };
        let before = alo_url::parse("http://a.test/p.html").unwrap();
        let at = Some(&before);
        let part = [moved("http://a.test/p.html#second", "fragment")];
        assert!(went_to_a_part(&part, at, "#second"));
        assert!(went_to_a_part(&part, at, "p.html#second"));
        assert!(!went_to_a_part(&part, at, "#third"), "somewhere else");
        assert!(!went_to_a_part(&part, None, "#second"), "no address before");
        assert!(!went_to_a_part(&[], at, "#second"), "nothing moved");
        assert!(
            !went_to_a_part(
                &[moved("http://a.test/p.html#second", "historyApi")],
                at,
                "#second"
            ),
            "a script's move, not the link's"
        );
        assert!(went_to_a_part(
            &[moved("http://a.test/p.html#", "fragment")],
            at,
            "#"
        ));
    }

    #[test]
    fn text_is_cancelled_only_when_the_field_still_holds_what_it_held() {
        let node = BoxId::from_wire(7);
        let put = |text: &str| Outcome::TextPut {
            node,
            text: text.to_owned(),
        };
        assert_eq!(
            taken(node, "2026-10-12".to_owned(), "", "2026-10-12"),
            put("2026-10-12")
        );
        assert_eq!(
            taken(node, "2026-10-12".to_owned(), "2024-02-29", "2024-02-29"),
            Outcome::TextCanceled {
                node,
                text: "2026-10-12".to_owned()
            }
        );
        assert_eq!(
            taken(node, "2026-10-12".to_owned(), "2026-10-12", "2026-10-12"),
            put("2026-10-12"),
            "what it held already"
        );
        assert_eq!(taken(node, String::new(), "", ""), put(""));
    }

    #[test]
    fn a_node_is_its_dom_node_by_the_id_the_tree_gave_it() {
        assert_eq!(dom_of(BoxId::from_wire(76)), 76);
        assert_eq!(box_of(76), BoxId::from_wire(76));
        assert_eq!(dom_of(box_of(3)), 3);
    }

    #[test]
    fn why_a_verb_was_not_carried_out_reads_as_a_sentence() {
        let node = BoxId::from_wire(7);
        assert_eq!(
            Unacted::Covered { node, by: Some(9) }.to_string(),
            "the middle of box#7 is under Chrome's node 9, so nothing was pressed"
        );
        assert_eq!(
            Unacted::Covered { node, by: None }.to_string(),
            "Chrome finds nothing at the middle of box#7, so nothing was pressed"
        );
        assert_eq!(
            Unacted::Undrawn { node }.to_string(),
            "Chrome drew box#7 nowhere"
        );
        assert_eq!(
            Unacted::Untypable {
                node,
                why: chrome_date::Untypable::LeadingZeros {
                    text: "02026-01-02".to_owned()
                }
            }
            .to_string(),
            "Chrome's date field would hold the year of \"02026-01-02\" without its leading zeros, so nothing was typed into box#7"
        );
        assert_eq!(
            Unacted::Refused(Refusal::Disabled { node }).to_string(),
            "box#7 says it is disabled"
        );
    }
}
