/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What caused each document's load, and the walk back along it.
//!
//! ADR 0012 § 3: *"**Which page, and which agent action** is two questions, and
//! the second one is usually answered indirectly. An agent activates a link; a
//! document loads; that document fetches a script. The script's cause is the
//! document. The document's cause is the agent action. The action's cause is
//! the person who asked for it."*
//!
//! [`crate::cause`] carries one link. This file is the rest of the chain: a
//! [`Documents`] holds what caused each document's load, and [`Documents::chain`]
//! walks from a request's [`Cause`] to the person at the far end of it,
//! naming every action it passes through on the way.
//!
//! # Walked, never assembled
//!
//! The ADR says the chain is *"walked rather than assembled: a document already
//! has an identity, and it records what caused its load, so nothing has to keep
//! a side table of which activity belongs to which action."* That is a rule
//! about **where the truth is**, not a performance note. A side table of *this
//! action's requests* would be a second answer to a question the causes already
//! answer, and the day the two disagree the record is worse than nothing —
//! because it still reads like evidence.
//!
//! So the only thing written down here is *what caused this document's load*,
//! by the browser process, at the moment it loaded it. Everything else is
//! derived.
//!
//! # The walk does not trust the shape it is walking
//!
//! A cycle cannot be *created*: [`Documents::opened`] mints the document and
//! records its cause in one act, so a document's cause can only ever name a
//! document that already existed, and ADR 0003's ids never come round again.
//!
//! The walk still carries the documents it has been through and stops if one
//! comes back, because a walk that trusted an invariant is a walk that hangs on
//! the first bug in whatever maintains it — and this one runs in the **browser
//! process**, where a hang is the single thing ADR 0005 says must never happen.
//! [`Documents`]'s own tests reach past its constructor to build a cycle by
//! hand for exactly that reason: what is asserted is that the walk survives a
//! state nothing can put it in.
//!
//! # Bounded, because a session is long
//!
//! ADR 0012 § 6: *"Everything, for the session, in memory, bounded."* Every
//! page load remembers one more cause, so [`MOST_DOCUMENTS`] is the ceiling and
//! the oldest go first. A chain that reaches one of those says
//! [`End::Forgotten`] rather than stopping quietly, because *we knew and no
//! longer do* and *nobody ever said* are different answers and a record that
//! ran them together would be guessing in the one place that exists not to.

use crate::cause::{ActionId, Cause, DocumentId, Identities, TabId};
use core::fmt;
use std::collections::{HashMap, HashSet, VecDeque};

/// How many documents' causes are remembered at once.
///
/// **A choice rather than a measurement**, in the shape `alo-net`'s other
/// bounds are made: a limit somebody else chooses is not a limit. It is a
/// bound on *memory* rather than on a page — nothing a stranger sends can add
/// an entry here, since only a load the browser process performed makes one —
/// so it is set where a person's whole session of ordinary browsing fits inside
/// it and a program that opened documents in a loop for a week does not.
pub const MOST_DOCUMENTS: usize = 4096;

/// What caused each document's load.
///
/// One of these per browser process, beside the [`Identities`] that mints for
/// it. A renderer holds neither (ADR 0005, ADR 0012 § 4).
#[derive(Debug, Default)]
pub struct Documents {
    /// What caused each remembered document's load.
    ///
    /// **The first answer wins and there is no way to write a second**, which
    /// is [`Documents::opened`] minting and recording in one act rather than a
    /// rule anybody follows: a cause that could be rewritten afterwards is a
    /// sentence somebody composed rather than a thing that happened.
    caused_by: HashMap<DocumentId, Cause>,
    /// The order they were loaded in, so the oldest goes first at the ceiling.
    order: VecDeque<DocumentId>,
    /// The most recent document dropped under [`MOST_DOCUMENTS`], if any.
    ///
    /// Ids count up and the oldest is dropped first, so anything at or below
    /// this was once remembered here. That is what lets a walk tell
    /// [`End::Forgotten`] from [`End::Unrecorded`] instead of offering one
    /// answer for two different situations.
    forgotten_to: Option<DocumentId>,
}

impl Documents {
    /// A document that has just loaded, and what caused it to.
    ///
    /// Minting and recording are **one act**, which is ADR 0012 § 3 made
    /// structural: there is no moment at which a document exists here without a
    /// cause, and no second call that could give it a different one.
    ///
    /// The cause is the one the load's own request carried — a person
    /// navigating, or the agent action whose verb led here. Which is why a
    /// document's cause can only name things that already existed, and why the
    /// walk below terminates.
    pub fn opened(&mut self, minting: &mut Identities, cause: Cause) -> DocumentId {
        let document = minting.a_document();
        self.caused_by.insert(document, cause);
        self.order.push_back(document);
        while self.order.len() > MOST_DOCUMENTS {
            if let Some(oldest) = self.order.pop_front() {
                self.caused_by.remove(&oldest);
                self.forgotten_to = Some(oldest);
            }
        }
        document
    }

    /// What caused a document's load, while it is still remembered.
    pub fn cause_of(&self, document: DocumentId) -> Option<&Cause> {
        self.caused_by.get(&document)
    }

    /// How many documents' causes are held.
    pub fn remembered(&self) -> usize {
        self.order.len()
    }

    /// Whether none are.
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Walk from a cause to what caused *that*, and so on.
    ///
    /// The answer to both of ADR 0012 § 3's questions at once: the first link
    /// is the page, and an [`ActionId`] anywhere along the way is the agent
    /// action. Neither is chosen over the other — the chain keeps them in
    /// order, which is the whole reason a cause is a link rather than a label.
    pub fn chain(&self, from: &Cause) -> Chain {
        let mut links = vec![from.clone()];
        let mut through: HashSet<DocumentId> = HashSet::new();
        let mut cause = from.clone();
        loop {
            let document = match cause {
                // The far end: a navigation happens in a tab rather than in a
                // page, so there is nothing above it to ask about.
                Cause::Person { tab } => return Chain::ending(links, End::Person(tab)),
                Cause::Document { document } | Cause::Agent { document, .. } => document,
            };
            if !through.insert(document) {
                return Chain::ending(links, End::CameRound(document));
            }
            let Some(next) = self.caused_by.get(&document) else {
                return Chain::ending(links, self.why_not(document));
            };
            links.push(next.clone());
            cause = next.clone();
        }
    }

    /// Why a document's cause is not here: dropped under the ceiling, or never
    /// written down at all.
    fn why_not(&self, document: DocumentId) -> End {
        match self.forgotten_to {
            Some(last) if document <= last => End::Forgotten(document),
            _ => End::Unrecorded(document),
        }
    }
}

/// Where a walk stopped.
///
/// Part of the answer rather than an aside: a chain that reached a person is
/// evidence, and one that ran out of record is a chain with a piece missing.
/// Saying which is the difference between a record and a story.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// A person's own navigation, in this tab. Where a whole chain ends.
    Person(TabId),
    /// This document's cause was remembered and has since been dropped under
    /// [`MOST_DOCUMENTS`].
    Forgotten(DocumentId),
    /// Nothing here ever recorded what caused this document's load.
    ///
    /// Not something a page can cause: it is a document opened by something
    /// that did not go through [`Documents::opened`], which is a fault in the
    /// browser process rather than in anybody's page.
    Unrecorded(DocumentId),
    /// The walk arrived at a document it had already been through.
    ///
    /// Impossible to create through [`Documents::opened`] and reported anyway
    /// — see this module's own documentation for why the walk does not trust
    /// that.
    CameRound(DocumentId),
}

impl fmt::Display for End {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            End::Person(tab) => write!(f, "the person, in {tab}"),
            End::Forgotten(document) => {
                write!(f, "what caused {document} is no longer remembered")
            }
            End::Unrecorded(document) => write!(f, "nothing recorded what caused {document}"),
            End::CameRound(document) => write!(f, "{document} came round again"),
        }
    }
}

/// A cause, and everything that caused it, in order.
///
/// The first link is the request's own cause; the last is whatever the walk
/// could reach. [`Chain::end`] says why it stopped there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chain {
    links: Vec<Cause>,
    end: End,
}

impl Chain {
    fn ending(links: Vec<Cause>, end: End) -> Self {
        Self { links, end }
    }

    /// Every cause along the way, nearest first.
    ///
    /// Never empty: the cause the walk started from is the first link.
    pub fn links(&self) -> &[Cause] {
        &self.links
    }

    /// Where the walk stopped, and why.
    pub fn end(&self) -> End {
        self.end
    }

    /// The agent action nearest this request, if the chain passes through one.
    ///
    /// [`None`] is a real answer and the common one: most requests are a
    /// person's browsing, and this is what makes *the agent did that* a claim
    /// rather than a default. **Nearest** because a chain may hold several — an
    /// agent that activated a link on a page it had already loaded — and the
    /// rest are in [`Chain::links`] rather than summarised away.
    pub fn action(&self) -> Option<ActionId> {
        self.links.iter().find_map(|link| match link {
            Cause::Agent { action, .. } => Some(*action),
            Cause::Person { .. } | Cause::Document { .. } => None,
        })
    }

    /// Whether this action is anywhere in the chain.
    ///
    /// The question queue item 133 asks of a record — *did this follow from
    /// what the agent did* — and it is deliberately not the same as
    /// [`Chain::action`] being equal to it: an agent that acted twice caused
    /// the second action's requests through the first action's document.
    pub fn followed_from(&self, action: ActionId) -> bool {
        self.links
            .iter()
            .any(|link| matches!(link, Cause::Agent { action: named, .. } if *named == action))
    }

    /// The tab a person's navigation started this in, when the walk reached
    /// one.
    pub fn person(&self) -> Option<TabId> {
        match self.end {
            End::Person(tab) => Some(tab),
            End::Forgotten(_) | End::Unrecorded(_) | End::CameRound(_) => None,
        }
    }

    /// Whether the walk reached a person rather than running out of record.
    pub fn is_whole(&self) -> bool {
        matches!(self.end, End::Person(_))
    }
}

impl fmt::Display for Chain {
    /// Nearest first, and why it stopped where it did — the sentence a person
    /// reads in queue item 127, rather than a list of numbers.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (behind, link) in self.links.iter().enumerate() {
            if behind > 0 {
                f.write_str(", caused by ")?;
            }
            write!(f, "{link}")?;
        }
        match self.end {
            // The last link already said it: a chain that reached a person
            // ends in that person, and saying so twice would read as two
            // navigations rather than one.
            End::Person(_) => Ok(()),
            end => write!(f, " — {end}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A browser process: the identities it mints and what it remembers.
    fn browser() -> (Identities, Documents) {
        (Identities::default(), Documents::default())
    }

    // --- The chain ADR 0012 § 3 describes, walked ---------------------------

    /// The ADR's own example: an agent activates a link, a document loads, that
    /// document fetches a script. The script's cause is the document, the
    /// document's cause is the action, and only the walk says the script was
    /// the agent's doing.
    #[test]
    fn a_script_a_page_fetched_leads_back_to_the_agent_action_that_opened_it() {
        let (mut minting, mut documents) = browser();
        let tab = minting.a_tab();
        let read = documents.opened(&mut minting, Cause::Person { tab });
        let action = minting.an_action();
        let opened = documents.opened(
            &mut minting,
            Cause::Agent {
                action,
                document: read,
            },
        );

        let script = documents.chain(&Cause::Document { document: opened });

        assert_eq!(script.action(), Some(action), "{script}");
        assert!(script.followed_from(action));
        assert_eq!(script.person(), Some(tab), "the far end is a person");
        assert!(script.is_whole());
        assert_eq!(
            script.links().len(),
            3,
            "the script, the document it is in, and the action that opened it: {script}",
        );
    }

    /// The other direction, and the one that makes the first worth anything: a
    /// page a person opened themselves is nobody's action. A chain that
    /// answered *some action* for ordinary browsing would make the record
    /// useless in exactly the case somebody consults it.
    #[test]
    fn a_page_a_person_opened_leads_back_to_the_person_and_to_no_action() {
        let (mut minting, mut documents) = browser();
        let tab = minting.a_tab();
        let opened = documents.opened(&mut minting, Cause::Person { tab });

        let fetched = documents.chain(&Cause::Document { document: opened });

        assert_eq!(fetched.action(), None);
        assert_eq!(fetched.person(), Some(tab));
        assert!(fetched.is_whole());
    }

    /// An agent that acted twice: the second action's document was loaded from
    /// a page the first action opened. Both are in the chain, nearest first,
    /// because ADR 0012 keeps both answers rather than choosing.
    #[test]
    fn two_actions_are_both_in_the_chain_nearest_first() {
        let (mut minting, mut documents) = browser();
        let tab = minting.a_tab();
        let first_page = documents.opened(&mut minting, Cause::Person { tab });
        let first = minting.an_action();
        let second_page = documents.opened(
            &mut minting,
            Cause::Agent {
                action: first,
                document: first_page,
            },
        );
        let second = minting.an_action();
        let third_page = documents.opened(
            &mut minting,
            Cause::Agent {
                action: second,
                document: second_page,
            },
        );

        let chain = documents.chain(&Cause::Document {
            document: third_page,
        });

        assert_eq!(chain.action(), Some(second), "the nearest one: {chain}");
        assert!(chain.followed_from(first), "the earlier one is still there");
        assert!(chain.followed_from(second));
        assert_eq!(chain.person(), Some(tab));
    }

    /// A person's own navigation is one link and no walk: it happens in a tab
    /// rather than in a page, so there is nothing above it to ask about.
    #[test]
    fn a_persons_navigation_is_where_a_chain_ends() {
        let (mut minting, documents) = browser();
        let tab = minting.a_tab();

        let chain = documents.chain(&Cause::Person { tab });

        assert_eq!(chain.links().len(), 1);
        assert_eq!(chain.end(), End::Person(tab));
        assert_eq!(chain.to_string(), "the person, in tab#0");
    }

    // --- What the walk does not trust ---------------------------------------

    /// The closing condition of queue item 199, and the reason it is written
    /// down: ADR 0003's ids make a cycle impossible to **create**, and a walk
    /// that trusted that is a walk that hangs on the first bug in whatever
    /// maintains it.
    ///
    /// So this reaches past [`Documents::opened`] to build one by hand. There
    /// is no public way to do it, which is the point: what is asserted is that
    /// the walk survives a state nothing can put it in.
    #[test]
    fn a_cycle_stops_the_walk_rather_than_looping_it() {
        let (mut minting, mut documents) = browser();
        let first = minting.a_document();
        let second = minting.a_document();
        documents
            .caused_by
            .insert(first, Cause::Document { document: second });
        documents
            .caused_by
            .insert(second, Cause::Document { document: first });

        let chain = documents.chain(&Cause::Document { document: first });

        assert_eq!(chain.end(), End::CameRound(first));
        assert!(!chain.is_whole(), "a cycle is not a chain that ended");
        assert_eq!(chain.person(), None);
        assert_eq!(chain.links().len(), 3, "first, second, and back: {chain}");
    }

    /// A document that names itself, which is the shortest cycle there is.
    #[test]
    fn a_document_that_caused_itself_stops_at_once() {
        let (mut minting, mut documents) = browser();
        let only = minting.a_document();
        documents
            .caused_by
            .insert(only, Cause::Document { document: only });

        let chain = documents.chain(&Cause::Document { document: only });

        assert_eq!(chain.end(), End::CameRound(only));
    }

    // --- The bound ----------------------------------------------------------

    /// ADR 0012 § 6's *bounded*, and the honesty that has to come with it: a
    /// chain reaching a document we have dropped says so rather than reading
    /// like one that ended.
    #[test]
    fn the_oldest_go_first_and_a_chain_that_reaches_one_says_so() {
        let (mut minting, mut documents) = browser();
        let tab = minting.a_tab();
        let first = documents.opened(&mut minting, Cause::Person { tab });
        let mut latest = first;
        for _ in 0..MOST_DOCUMENTS {
            latest = documents.opened(&mut minting, Cause::Document { document: latest });
        }

        assert_eq!(documents.remembered(), MOST_DOCUMENTS);
        assert_eq!(documents.cause_of(first), None, "the oldest went");

        let chain = documents.chain(&Cause::Document { document: latest });
        assert_eq!(chain.end(), End::Forgotten(first));
        assert!(!chain.is_whole(), "a piece of it is missing");
        assert_eq!(chain.person(), None, "and it must not claim otherwise");
    }

    /// *We knew and no longer do* and *nobody ever said* are different answers.
    /// A document nothing recorded is a fault in the browser process; one that
    /// was dropped is this file doing what it said it would.
    #[test]
    fn a_document_nobody_recorded_is_not_reported_as_forgotten() {
        let (mut minting, documents) = browser();
        let stray = minting.a_document();

        let chain = documents.chain(&Cause::Document { document: stray });

        assert_eq!(chain.end(), End::Unrecorded(stray));
        assert_eq!(
            chain.to_string(),
            "document#0 — nothing recorded what caused document#0",
        );
    }

    // --- What is written down, and what cannot be ---------------------------

    /// A document's cause is written once. The second call is a different
    /// document, because [`Documents::opened`] mints — so there is no call that
    /// could rewrite the first one's.
    #[test]
    fn a_causes_record_is_not_something_a_later_call_can_change() {
        let (mut minting, mut documents) = browser();
        let tab = minting.a_tab();
        let one = documents.opened(&mut minting, Cause::Person { tab });
        let two = documents.opened(&mut minting, Cause::Person { tab });

        assert_ne!(one, two, "opening records a new document every time");
        assert_eq!(documents.cause_of(one), Some(&Cause::Person { tab }));
        assert_eq!(documents.remembered(), 2);
        assert!(!documents.is_empty());
    }

    /// The sentence a person reads. Nearest first, and the end says how it
    /// finished rather than repeating the last link.
    #[test]
    fn a_chain_says_what_led_to_what_in_words() {
        let (mut minting, mut documents) = browser();
        let tab = minting.a_tab();
        let page = documents.opened(&mut minting, Cause::Person { tab });
        let action = minting.an_action();
        let opened = documents.opened(
            &mut minting,
            Cause::Agent {
                action,
                document: page,
            },
        );

        let chain = documents.chain(&Cause::Document { document: opened });

        assert_eq!(
            chain.to_string(),
            "document#1, caused by action#0, in document#0, caused by the person, in tab#0",
        );
    }
}
