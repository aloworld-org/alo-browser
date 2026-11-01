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
//!
//! # Never over a card
//!
//! A page shown as a card in the conversation (ADR 0076 B) is drawn by alo
//! from the frames Chrome streams of it, and Chrome's overlay is in those
//! frames: on a card, `Overlay.highlightNode` drew Chrome's inspector
//! tooltip — the element's tag and size, *Accessibility*, *Name*, *Role*,
//! *Keyboard-focusable* — over the page, and it stayed after alo's own
//! agent had finished, until the next verb (queue item 822, found by hand
//! in 704). So **while its page is a card, Chrome is asked to draw nothing
//! over it** ([`Marker::over_a_card`]): a verb takes away any mark there is
//! instead, and the card's run says, in its lines, what each step did. A
//! card expanded into Chrome's own window ([`Marker::expanded`]) is
//! Chrome's window again, and marked as this module's first sections say.
//! Whatever is drawn is taken away when a run of alo's agent ends or is
//! stopped ([`Marker::clear`], `CardOrder::Unmark`), so nothing of Chrome's
//! is left over a page nobody is acting on. alo's own ring on a card,
//! drawn by the first screen against the frame it was measured on, is
//! queue item 823 (ADR 0066's amendment of 1 November 2026).

use crate::asking::Answer;
use crate::chrome::Failed;
use crate::chrome_act::dom_of;
use crate::chrome_protocol::Command;
use crate::chrome_tab::Tab;
use crate::colours;
use crate::message::CardOrder;
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

/// What a [`Marker`] asks of Chrome for a [`Marking`]: the decision, before
/// anything is sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Ask for nothing.
    Nothing,
    /// Take any mark away: `Overlay.hideHighlight`, sent only when Chrome
    /// has been let draw one.
    Unmark,
    /// Mark the first of these Chrome drew, on the page the verb was done
    /// to.
    Nodes(Vec<BoxId>),
}

/// The mark over one tab's page in Chrome: whether Chrome has been let draw
/// one yet, and whether the page is a card, over which nothing is drawn.
#[derive(Debug, Default)]
pub struct Marker {
    shown: bool,
    card: bool,
}

impl Marker {
    /// The mark over a page shown as a card in the conversation: never
    /// drawn while it is one (queue item 822).
    pub fn over_a_card() -> Self {
        Self::over(true)
    }

    /// The mark over a page shown as a card when `card`, or in Chrome's own
    /// window.
    pub fn over(card: bool) -> Self {
        Self { shown: false, card }
    }

    /// What a card order asks of the mark: its card expanded into Chrome's
    /// own window ([`Marker::expanded`]), or any mark taken away as a run
    /// of alo's agent ends ([`Marker::clear`], `CardOrder::Unmark`).
    pub fn heed(&mut self, tab: &mut Tab<'_>, order: &CardOrder) {
        match order {
            CardOrder::Expand { .. } => self.expanded(),
            // A mark Chrome would not take away is said by nothing: the
            // next verb's mark replaces it.
            CardOrder::Unmark => {
                let _ = self.clear(tab);
            }
            CardOrder::Stream { .. }
            | CardOrder::Unstream
            | CardOrder::Input(_)
            | CardOrder::Travel(_) => {}
        }
    }

    /// Its card expanded into Chrome's own window, which a person now
    /// looks at: marked from now on, as ADR 0066 has it.
    pub fn expanded(&mut self) {
        self.card = false;
    }

    /// What `marking` comes to here: on a card, never a node — a verb that
    /// ran takes any mark away, as one refused does.
    pub fn plan(&self, marking: &Marking) -> Plan {
        match marking {
            Marking::Keep => Plan::Nothing,
            Marking::Unmark => Plan::Unmark,
            Marking::Nodes(_) if self.card => Plan::Unmark,
            Marking::Nodes(nodes) => Plan::Nodes(nodes.clone()),
        }
    }

    /// Take any mark away, as a run of alo's agent ends or is stopped
    /// (queue item 822): nothing is asked of Chrome when it has never been
    /// let draw one.
    ///
    /// # Errors
    ///
    /// [`Failed`], when Chrome did not answer or declined.
    pub fn clear(&mut self, tab: &mut Tab<'_>) -> Result<Marked, Failed> {
        self.unmark(tab)
    }

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
        let nodes = match self.plan(marking) {
            Plan::Nothing => return Ok(Marked::Kept),
            Plan::Unmark => return self.unmark(tab),
            Plan::Nodes(nodes) => nodes,
        };
        if before.is_none() || tab.document() != before {
            return self.unmark(tab);
        }
        for node in nodes {
            let dom = dom_of(node);
            if tab.drawn(dom)?.is_empty() {
                continue;
            }
            self.show(tab)?;
            let (red, green, blue, _) = colours::mark().to_rgba8();
            tab.ask(&Command::Mark {
                node: dom,
                border: (red, green, blue),
            })?;
            return Ok(Marked::Node(node));
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
    fn over_a_card_nothing_is_marked_and_a_verb_takes_any_mark_away() {
        let node = BoxId::from_wire(7);
        let ran = Marking::of(true, &done(Outcome::Activated { node, name: None }));
        let mut card = Marker::over_a_card();
        assert_eq!(card.plan(&ran), Plan::Unmark);
        assert_eq!(card.plan(&Marking::Unmark), Plan::Unmark);
        assert_eq!(card.plan(&Marking::Keep), Plan::Nothing);
        // Expanded into Chrome's own window, it is marked as ADR 0066 has it.
        card.expanded();
        assert_eq!(card.plan(&ran), Plan::Nodes(vec![node]));
        let window = Marker::default();
        assert_eq!(window.plan(&ran), Plan::Nodes(vec![node]));
        assert_eq!(window.plan(&Marking::Keep), Plan::Nothing);
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
