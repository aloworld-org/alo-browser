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
//! # What it cannot say yet
//!
//! - **A form Chrome would not send** answers [`Outcome::Activated`], not
//!   [`Outcome::NotSubmitted`]: Chrome's tree does not say which control
//!   failed — a `required` field left empty is not `invalid` in it — and its
//!   words for why are drawn in a bubble nothing here can read (item 631).
//! - **Text the page would not take** is answered [`Outcome::TextCanceled`]
//!   when the field holds afterwards what it held before. Whether the page
//!   cancelled the `beforeinput` or put the old text back in its `input`
//!   listener, Chrome does not say, and both leave the field as it was.

use crate::chrome::Failed;
use crate::chrome_protocol::{self, AxNode, Chord, Command, Mouse};
use crate::chrome_reading::ChromeReading;
use crate::chrome_tab::Tab;
use crate::chrome_tree::Unread;
use alo_agent::verb::{Target, Verb, find, perform};
use alo_agent::{Outcome, Refusal};
use alo_box::BoxId;
use alo_dom::submission::Method;
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
        // Accepted, and so an action, whether it is refused or carried out
        // (ADR 0012 § 4): what the page asks for until it answers is the
        // agent's, and so is a navigation it asked for (`chrome_record`).
        self.acting();
        let acted = self.decide_and_carry_out(target, verb);
        self.done_acting();
        acted
    }

    /// [`Tab::act`], once the verb is an action.
    fn decide_and_carry_out(&mut self, target: &Target, verb: &Verb) -> Result<Outcome, Unacted> {
        let (snapshot, nodes) = self.read_whole()?;
        let reading = ChromeReading::of(&snapshot);
        let aimed = find(&reading, target).map_err(Unacted::Refused)?;
        let dom = dom_of(aimed.id);
        let described = chrome_protocol::described(&self.ask(&Command::Describe { node: dom })?)?;
        let reading = reading.with_element(aimed.id, &described);
        let decided = perform(&reading, target, verb).map_err(Unacted::Refused)?;
        match decided {
            Outcome::Activated { node, name } => self.press(&nodes, node, name, None),
            Outcome::Followed { node, to } => self.press(&nodes, node, aimed.name, Some(to)),
            Outcome::TextPut { node, text } => self.type_in(node, text),
            outcome @ (Outcome::Scrolled { .. }
            | Outcome::TextCanceled { .. }
            | Outcome::Submitted { .. }
            | Outcome::NotSubmitted { .. }) => Err(Unacted::Uncarried { outcome }),
        }
    }

    /// Click the middle of `node`, and say what came of it: `to` is where the
    /// link it is goes, when it is one.
    fn press(
        &mut self,
        nodes: &[AxNode],
        node: BoxId,
        name: Option<String>,
        to: Option<String>,
    ) -> Result<Outcome, Unacted> {
        let dom = dom_of(node);
        // What the page asked for before this press is not what it did.
        drop(self.take_requested());
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
        for kind in [Mouse::Moved, Mouse::Pressed, Mouse::Released] {
            self.ask(&Command::Mouse { kind, x, y })?;
        }
        // Answered by the renderer, after anything the click asked for was
        // told of on the same channel.
        self.ask(&Command::Window)?;
        let asked = self
            .take_requested()
            .into_iter()
            .find(|requested| requested.disposition == "currentTab");
        Ok(match (asked, to) {
            (Some(asked), Some(to)) if asked.reason == "anchorClick" => {
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
            _ => Outcome::Activated { node, name },
        })
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
        Ok(if after == before && before != text {
            Outcome::TextCanceled { node, text }
        } else {
            Outcome::TextPut { node, text }
        })
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

/// The DOM node a node of the tree describes, by Chrome's `backendNodeId`:
/// the id [`crate::chrome_tree`] gave it.
fn dom_of(node: BoxId) -> u64 {
    u64::try_from(node.as_usize()).unwrap_or(u64::MAX)
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
    fn a_node_is_its_dom_node_by_the_id_the_tree_gave_it() {
        assert_eq!(dom_of(BoxId::from_wire(76)), 76);
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
            Unacted::Refused(Refusal::Disabled { node }).to_string(),
            "box#7 says it is disabled"
        );
    }
}
