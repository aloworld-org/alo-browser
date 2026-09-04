/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What caused a request, and the identities a cause names.
//!
//! ADR 0012: *"Every request carries a **cause** — a person, a document, or a
//! named agent action — which the **browser process** assigns and which cannot
//! be omitted."* [`Request`](crate::Request) holds one and there is no
//! constructor that does not take it, so a request that cannot say what caused
//! it does not compile.
//!
//! # Three causes, and there is no fourth
//!
//! There is deliberately no `Unknown`, no `Internal` and no `Other`. A request
//! this engine makes on its own behalf is attributed to whatever caused the
//! thing it is *about*: a violation report to the load that violated the policy
//! ([`crate::csp_report`]), a preflight to the request it is asking about
//! ([`crate::cors::asking_first`]), a redirect and a range request to whatever
//! asked for the first hop — each of which is a clone of a cause rather than a
//! new one. A category for the awkward cases does not stay empty, and the one
//! line in the record nobody can account for is the line somebody will need.
//!
//! # Why the identities live here rather than beside the things they name
//!
//! A tab is a browser-process thing and so is a document, and neither of them
//! is a network concept — but a **cause** is a field on a request, and a field
//! cannot name a type from a crate that depends on this one.
//!
//! The alternative was a second tab identity, minted here and mapped onto
//! `alo_renderer`'s. That is refused for ADR 0003's reason: two identity spaces
//! for one thing eventually disagree, and an id that means one tab in the
//! record and another in the browser would join two unrelated pieces of
//! somebody's history into one story. So there is one [`TabId`], it is this
//! one, and `alo_renderer::tab` re-exports it.
//!
//! # Allocated once and never reused
//!
//! ADR 0003's rule, taken unchanged, because ADR 0012 § 3 relies on it: the
//! chain a record is walked along terminates and cannot lie about which
//! document it reached only while an id that came back round is impossible.
//! [`Identities`] is the only thing that can mint one, there is one of it per
//! browser process, and it counts up.

use core::fmt;

/// Which tab. Allocated by [`Identities::a_tab`] and never reused.
///
/// The browser process's own tab identity: `alo_renderer::tab::TabId` is this
/// type rather than a second one, for the reason this module's own
/// documentation gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TabId(u64);

impl TabId {
    /// The id as a number, for diagnostics and for a caller keeping its own
    /// record beside this one.
    ///
    /// Not an index into anything: it is the order the tab was opened in.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tab#{}", self.0)
    }
}

/// Which document. Allocated by [`Identities::a_document`] and never reused.
///
/// A document rather than a page: the same address loaded twice is two
/// documents, and a record that called them one would be answering *what did
/// this fetch* with two visits joined together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentId(u64);

impl DocumentId {
    /// The id as a number, for diagnostics.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Display for DocumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "document#{}", self.0)
    }
}

/// Which agent action. Allocated by [`Identities::an_action`] and never reused.
///
/// Minted when a verb is **accepted** rather than when it succeeds, because a
/// verb that was refused is still something the agent did and still something
/// the person is entitled to see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActionId(u64);

impl ActionId {
    /// The id as a number, for diagnostics.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "action#{}", self.0)
    }
}

/// Who wanted this.
///
/// Not [`Purpose`](crate::Purpose), which says *what kind of thing* is wanted
/// and which a renderer is the only side that knows. This says *who wanted it*,
/// which a renderer may never state: it parsed a stranger's page (ADR 0005), so
/// a cause it could state is a cause it could forge into *the person did that*.
///
/// **There is no `Default`, and there will not be one.** ADR 0012 § 1 makes the
/// guarantee structural rather than a discipline, for the reason ADR 0002 gives
/// about verbs and coordinates: the call site added in a hurry is exactly the
/// one that would have omitted it, and exactly the one somebody will later need
/// to account for.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cause {
    /// A navigation somebody made: typed, a bookmark, a link they clicked,
    /// back.
    Person {
        /// The tab they made it in.
        tab: TabId,
    },
    /// A page loading what it needs, or a script it ran.
    Document {
        /// The document that wanted it. Its origin and its tab are reachable
        /// through the document rather than repeated here — ADR 0012 § 3 makes
        /// a cause a link in a chain, and a copy of a fact is a copy that can
        /// disagree with it.
        document: DocumentId,
    },
    /// A verb this engine performed on an agent's behalf.
    Agent {
        /// The action, by the identity minted when the verb was accepted.
        action: ActionId,
        /// The document it acted in.
        document: DocumentId,
    },
}

impl Cause {
    /// The document this happened in, when there is one.
    ///
    /// [`None`] for a person's own navigation, which is the one cause that
    /// happens in a tab rather than in a page — the first load of a window is
    /// the request that creates the document rather than one made by it.
    pub fn in_document(&self) -> Option<DocumentId> {
        match self {
            Cause::Person { .. } => None,
            Cause::Document { document } | Cause::Agent { document, .. } => Some(*document),
        }
    }
}

impl fmt::Display for Cause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Cause::Person { tab } => write!(f, "the person, in {tab}"),
            Cause::Document { document } => write!(f, "{document}"),
            Cause::Agent { action, document } => write!(f, "{action}, in {document}"),
        }
    }
}

/// The browser process's supply of identities.
///
/// One of these per browser process, and nothing else can mint an id — which is
/// what makes ADR 0003's *allocated once and never reused* a property of the
/// type rather than a rule somebody remembers. It is also ADR 0012 § 4 in the
/// only place it can be made structural: **a renderer cannot hold one**, since
/// ADR 0005 gives it no access to the browser process's state at all, so a
/// renderer has nothing it could name a cause with.
///
/// Counting up rather than recycling costs one `u64` per kind and nothing else.
/// A browser that opened a tab every microsecond would take half a million
/// years to exhaust one, so the saturating add below is a statement that the
/// number cannot wrap rather than a case anybody reaches.
#[derive(Debug, Default)]
pub struct Identities {
    tabs: u64,
    documents: u64,
    actions: u64,
}

impl Identities {
    /// A tab nothing has ever been.
    pub fn a_tab(&mut self) -> TabId {
        let id = TabId(self.tabs);
        self.tabs = self.tabs.saturating_add(1);
        id
    }

    /// A document nothing has ever been.
    pub fn a_document(&mut self) -> DocumentId {
        let id = DocumentId(self.documents);
        self.documents = self.documents.saturating_add(1);
        id
    }

    /// An action nothing has ever been.
    pub fn an_action(&mut self) -> ActionId {
        let id = ActionId(self.actions);
        self.actions = self.actions.saturating_add(1);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR 0003, taken unchanged: an id that came back round would join two
    /// unrelated pieces of somebody's history into one story.
    #[test]
    fn an_identity_is_never_handed_out_twice() {
        let mut minting = Identities::default();
        let tabs: Vec<TabId> = (0..64).map(|_| minting.a_tab()).collect();
        let documents: Vec<DocumentId> = (0..64).map(|_| minting.a_document()).collect();
        let actions: Vec<ActionId> = (0..64).map(|_| minting.an_action()).collect();

        for (earlier, later) in tabs.iter().zip(tabs.iter().skip(1)) {
            assert!(earlier < later, "{earlier} came again as {later}");
        }
        for (earlier, later) in documents.iter().zip(documents.iter().skip(1)) {
            assert!(earlier < later, "{earlier} came again as {later}");
        }
        for (earlier, later) in actions.iter().zip(actions.iter().skip(1)) {
            assert!(earlier < later, "{earlier} came again as {later}");
        }
    }

    /// The three kinds count separately, because they are three kinds. A tab
    /// and a document that shared a counter would still be distinct types —
    /// and a person reading `tab#4` beside `document#5` would be reading two
    /// numbers that mean nothing next to each other.
    #[test]
    fn the_three_kinds_are_counted_apart() {
        let mut minting = Identities::default();
        assert_eq!(minting.a_tab().as_u64(), 0);
        assert_eq!(minting.a_document().as_u64(), 0);
        assert_eq!(minting.an_action().as_u64(), 0);
        assert_eq!(minting.a_tab().as_u64(), 1);
    }

    /// ADR 0012 § 2. This test is the enforcement rather than a description of
    /// it: a fourth variant makes this match non-exhaustive, and adding a
    /// wildcard arm to fix it is a diff somebody has to argue for.
    #[test]
    fn three_causes_and_there_is_no_fourth() {
        let mut minting = Identities::default();
        let tab = minting.a_tab();
        let document = minting.a_document();
        let action = minting.an_action();

        for cause in [
            Cause::Person { tab },
            Cause::Document { document },
            Cause::Agent { action, document },
        ] {
            let said = match cause {
                Cause::Person { .. } => "a person",
                Cause::Document { .. } => "a document",
                Cause::Agent { .. } => "an agent",
            };
            assert!(!said.is_empty());
        }
    }

    #[test]
    fn a_person_navigates_in_a_tab_rather_than_in_a_document() {
        let mut minting = Identities::default();
        let person = Cause::Person {
            tab: minting.a_tab(),
        };
        assert_eq!(person.in_document(), None);

        let document = minting.a_document();
        assert_eq!(
            Cause::Document { document }.in_document(),
            Some(document),
            "a page's own fetch happens in that page",
        );
        assert_eq!(
            Cause::Agent {
                action: minting.an_action(),
                document,
            }
            .in_document(),
            Some(document),
            "a verb acts in the document it was aimed at",
        );
    }

    /// A cause is read by a person in item 127's interface, so it says
    /// something rather than printing a number.
    #[test]
    fn a_cause_says_who_in_words() {
        let mut minting = Identities::default();
        let tab = minting.a_tab();
        let document = minting.a_document();
        let action = minting.an_action();

        assert_eq!(
            Cause::Person { tab }.to_string(),
            "the person, in tab#0",
            "a person's navigation",
        );
        assert_eq!(Cause::Document { document }.to_string(), "document#0");
        assert_eq!(
            Cause::Agent { action, document }.to_string(),
            "action#0, in document#0",
        );
    }
}
