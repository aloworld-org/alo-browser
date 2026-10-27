/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The box an agent's verb acted on, marked over a page in Chrome (ADR 0066,
//! queue item 680).
//!
//! On this engine's path the box is ringed by the window, on the frame
//! painted after the verb (`mark.rs`, item 624). Through Chrome the page is
//! Chrome's window, and the box is marked **by Chrome**: `Overlay.highlightNode`
//! on the node the verb's outcome names, its border in alo's accent
//! ([`colours::mark`]), its other regions clear, and Chrome's label naming
//! it by its name and role. Chrome draws it in its own compositor against
//! the node, so it follows the page as it scrolls and moves the box, takes
//! no pointer, and tells the page nothing (ADR 0066 § 1). It does not look
//! like alo's ring, and ADR 0066 § 3 says so.
//!
//! # Asked after the verb, never before
//!
//! As the ring is: the verb is decided, carried out, recorded and answered
//! first, and only then is Chrome asked to mark anything — so the mark
//! changes nothing about what the verb did, and no verb gains a coordinate
//! (ADR 0002). The node is the renderer's own answer, by Chrome's
//! `backendNodeId`, never a position.
//!
//! # Which, and when nothing
//!
//! [`Marking::of`] decides from the answer, by the rule the conductor
//! answers by: a verb that ran marks the node its outcome names — an option
//! chosen, and then its list, since a closed list's option is drawn nowhere
//! (ADR 0064 § 5) — the first of them Chrome says it drew. A verb refused,
//! by the page or by this process, takes away any mark there is, as a
//! refused verb's repaint does on this engine's path; so does a verb that
//! took the tab to another document, whose box is not in the page now
//! shown, and one whose node Chrome drew nowhere. A read marks nothing and
//! takes nothing away. The next verb's mark replaces the last, because
//! Chrome draws one highlight at a time.
//!
//! # Only where a person looks
//!
//! Only a shown Chrome is marked: a headless one has nobody to see it, and
//! the commands would be sent for nothing.

use crate::asking::Answer;
use crate::chrome::Failed;
use crate::chrome_act::dom_of;
use crate::chrome_protocol::Command;
use crate::chrome_tab::Tab;
use crate::colours;
use alo_agent::Outcome;
use alo_box::BoxId;
use alo_net::cause::DocumentId;

/// What an answer asks of the mark over Chrome's page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Marking {
    /// Mark the first of these Chrome drew, in this order.
    Nodes(Vec<BoxId>),
    /// Take away any mark there is.
    Unmark,
    /// Leave the mark as it is.
    Keep,
}

impl Marking {
    /// What an answer to an ask — a verb when `a_verb` — asks of the mark.
    pub fn of(a_verb: bool, answer: &Answer) -> Self {
        match answer {
            Answer::Done {
                outcome: Outcome::Chosen { node, list, .. },
                ..
            } => Self::Nodes(vec![*node, *list]),
            Answer::Done { outcome, .. } => Self::Nodes(vec![outcome.node()]),
            Answer::Refused(_) if a_verb => Self::Unmark,
            _ => Self::Keep,
        }
    }
}

/// What was done to the mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marked {
    /// This node is marked.
    Node(BoxId),
    /// No mark is drawn.
    Unmarked,
    /// Nothing was asked of Chrome.
    Kept,
}

/// The mark over one tab's page in Chrome: whether Chrome has been let draw
/// one yet.
#[derive(Debug, Default)]
pub struct Marker {
    shown: bool,
}

impl Marker {
    /// Do what `marking` asks over `tab`'s page, which held the document
    /// `before` when the verb was asked.
    ///
    /// # Errors
    ///
    /// [`Failed`], when Chrome did not answer or declined. Nothing about
    /// the verb depends on it: the agent has been answered already.
    pub fn mark(
        &mut self,
        tab: &mut Tab<'_>,
        marking: &Marking,
        before: Option<DocumentId>,
    ) -> Result<Marked, Failed> {
        let nodes = match marking {
            Marking::Keep => return Ok(Marked::Kept),
            Marking::Unmark => return self.unmark(tab),
            Marking::Nodes(nodes) => nodes,
        };
        if before.is_none() || tab.document() != before {
            return self.unmark(tab);
        }
        for node in nodes {
            let dom = dom_of(*node);
            if tab.drawn(dom)?.is_empty() {
                continue;
            }
            self.show(tab)?;
            let (red, green, blue, _) = colours::mark().to_rgba8();
            tab.ask(&Command::Mark {
                node: dom,
                border: (red, green, blue),
            })?;
            return Ok(Marked::Node(*node));
        }
        self.unmark(tab)
    }

    /// Take any mark away — and when Chrome has never been let draw one,
    /// there is none, and nothing is asked.
    fn unmark(&mut self, tab: &mut Tab<'_>) -> Result<Marked, Failed> {
        if self.shown {
            tab.ask(&Command::Unmark)?;
        }
        Ok(Marked::Unmarked)
    }

    /// Let Chrome draw a mark, once: its DOM domain on first, which its
    /// overlay needs.
    fn show(&mut self, tab: &mut Tab<'_>) -> Result<(), Failed> {
        if !self.shown {
            tab.ask(&Command::ReadDocument)?;
            tab.ask(&Command::ShowMarks)?;
            self.shown = true;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asking::Refused;

    fn done(outcome: Outcome) -> Answer {
        Answer::Done {
            tab: "https://example.com/".to_owned(),
            outcome,
            issues: Vec::new(),
        }
    }

    #[test]
    fn a_verb_that_ran_marks_the_node_its_outcome_names() {
        let node = BoxId::from_wire(7);
        assert_eq!(
            Marking::of(true, &done(Outcome::Activated { node, name: None })),
            Marking::Nodes(vec![node])
        );
    }

    #[test]
    fn an_option_chosen_is_marked_or_else_its_list() {
        let (node, list) = (BoxId::from_wire(7), BoxId::from_wire(3));
        assert_eq!(
            Marking::of(
                true,
                &done(Outcome::Chosen {
                    node,
                    list,
                    chosen: true
                })
            ),
            Marking::Nodes(vec![node, list])
        );
    }

    #[test]
    fn a_refused_verb_takes_the_mark_away_and_a_refused_read_does_not() {
        let refused = Answer::Refused(Refused::NoSuchTab {
            tab: "https://example.com/".to_owned(),
        });
        assert_eq!(Marking::of(true, &refused), Marking::Unmark);
        assert_eq!(Marking::of(false, &refused), Marking::Keep);
        assert_eq!(Marking::of(false, &Answer::Tabs(Vec::new())), Marking::Keep);
    }
}
