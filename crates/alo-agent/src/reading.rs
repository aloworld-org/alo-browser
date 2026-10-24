/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a verb is decided on (ADR 0058 § 4, queue item 621).
//!
//! A verb is decided in this browser, by [`crate::verb::perform`], against a
//! page that may have been drawn by either renderer: this engine's own, read
//! through [`AgentTree`], or Chrome, read through its accessibility tree. So
//! the decision is made against a [`Reading`] — the things on the page an
//! agent can name, and the four facts about one of them that decide whether
//! a verb may go ahead — and **every refusal in [`crate::verb`] is reached
//! the same way whichever renderer drew the page**. A second copy of the
//! rules beside the second renderer would be a second structure, and ADR
//! 0002 says what happens to two structures: they disagree, and the agent
//! acts on the one that is wrong.

use crate::name::normalise;
use crate::tree::AgentTree;
use alo_box::{BoxId, Role, States};

/// One thing on a page an agent can name, as a verb weighs it.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// Which node, by the id an agent names it with.
    pub id: BoxId,
    /// What it is.
    pub role: Role,
    /// What it is called, if anything.
    pub name: Option<String>,
    /// What is true of it.
    pub states: States,
    /// Whether it has more content than room for it.
    pub scrolls: bool,
}

/// A page, as a verb is decided on it.
pub trait Reading {
    /// Every node an agent can name, in the order a person meets them.
    fn candidates(&self) -> Vec<Candidate>;
    /// Where the link `id` goes, as its author wrote it.
    fn href_of(&self, id: BoxId) -> Option<String>;
    /// Whether `id` is a date field, which holds a date or nothing.
    fn is_a_date_field(&self, id: BoxId) -> bool;
    /// The type of the field `id` is, when it is one whose text this browser
    /// cannot put in yet (`alo_dom::field::unbuilt`).
    fn unbuilt_field(&self, id: BoxId) -> Option<&'static str>;
    /// The most text the field `id` takes, when it says
    /// (`alo_dom::constraint::max_length`).
    fn max_length(&self, id: BoxId) -> Option<usize>;
}

/// Whether a thing called `held` is the thing called `wanted`: the same
/// words, ignoring case and the space around them, as a person reads a name
/// out.
pub fn is_called(held: Option<&str>, wanted: &str) -> bool {
    match (held, normalise(wanted)) {
        (Some(held), Some(wanted)) => held.eq_ignore_ascii_case(&wanted),
        _ => false,
    }
}

impl Reading for AgentTree<'_> {
    fn candidates(&self) -> Vec<Candidate> {
        self.nodes()
            .into_iter()
            .map(|node| Candidate {
                id: node.id(),
                role: node.role(),
                name: node.name(),
                states: node.states(),
                scrolls: node.scrolls(),
            })
            .collect()
    }

    fn href_of(&self, id: BoxId) -> Option<String> {
        AgentTree::href_of(self, id)
    }

    fn is_a_date_field(&self, id: BoxId) -> bool {
        AgentTree::is_a_date_field(self, id)
    }

    fn unbuilt_field(&self, id: BoxId) -> Option<&'static str> {
        AgentTree::unbuilt_field(self, id)
    }

    fn max_length(&self, id: BoxId) -> Option<usize> {
        AgentTree::max_length(self, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_matched_as_a_person_reads_it_out() {
        assert!(is_called(Some("Save"), "save"));
        assert!(is_called(Some("Save"), "  Save "));
        assert!(!is_called(Some("Save"), "Saved"));
        assert!(!is_called(None, "Save"));
        assert!(!is_called(Some(""), ""), "nothing is not a name");
    }
}
