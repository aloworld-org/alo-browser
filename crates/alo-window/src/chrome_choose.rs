/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ★ An option chosen through Chrome (ADR 0064 § 6, queue item 660).
//!
//! `alo-agent` decides that `Activate` on an option of a `<select>` chooses
//! it in its list ([`Outcome::Chosen`]); this carries that out on a page in
//! Chrome as a person does, and reads back what came of it.
//!
//! - **A list Chrome draws** — one with a `size`, or `multiple` — is chosen
//!   in by a click at the option's middle, aimed and checked as every press
//!   is ([`Tab::aim`]). In a list of several the click holds the platform's
//!   toggle, ⌘ on macOS: measured against Chrome 155, a plain click there
//!   chooses that option **alone** and drops every other choice, and a
//!   ⌘-click toggles it, as `alo_dom::choice` does on this engine's page.
//!   The page hears what it hears from a person: one `input` and one
//!   `change` at the list when the click changed what it holds.
//! - **A closed list** draws no option, so there is no box to press; and on
//!   a focused closed list every key that reaches an option — type-ahead's
//!   letters — passes through options nobody chose, each heard as `input`
//!   and `change`. So nothing is sent to Chrome at all, and the answer is
//!   [`Unacted::Closed`], until a way through a shown Chrome's menu is
//!   measured (item 658).
//! - **Never script**, and never `DOM.setAttributeValue`: setting the
//!   attribute sends the page no `input` and no `change`, so its listeners
//!   and checks would not run.
//!
//! Whether the option is chosen afterwards is **read from Chrome's tree**
//! once the click has been answered, not assumed from what was asked: a
//! page's `change` listener may put the old choice back, and the answer then
//! says the option is not chosen.

use crate::chrome_act::{Unacted, dom_of};
use crate::chrome_protocol::{self, AxNode, Command};
use crate::chrome_tab::Tab;
use alo_agent::Outcome;
use alo_box::BoxId;

/// An option to choose, in its list, as the reading of the page says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Choosing {
    /// The option.
    pub(crate) option: BoxId,
    /// Its list.
    pub(crate) list: BoxId,
    /// Whether the list drops down, and so draws no option.
    pub(crate) closed: bool,
    /// Whether the list takes several choices, in which the click toggles.
    pub(crate) toggles: bool,
}

impl Tab<'_> {
    /// Choose `choosing.option` in its list, with `nodes` the tree read
    /// before the click, and say whether it is chosen afterwards.
    pub(crate) fn choose(
        &mut self,
        nodes: &[AxNode],
        choosing: Choosing,
    ) -> Result<Outcome, Unacted> {
        let Choosing {
            option,
            list,
            closed,
            toggles,
        } = choosing;
        if closed {
            return Err(Unacted::Closed { node: option, list });
        }
        let (x, y) = self.aim(nodes, option)?;
        self.click(x, y, toggles)?;
        let read = chrome_protocol::ax_nodes(&self.ask(&Command::ReadNode {
            node: dom_of(option),
        })?)?;
        Ok(Outcome::Chosen {
            node: option,
            list,
            chosen: is_selected(&read, dom_of(option)),
        })
    }
}

/// Whether Chrome's tree, `read`, says the DOM node `dom` is selected: an
/// option nobody's tree says is chosen is not.
fn is_selected(read: &[AxNode], dom: u64) -> bool {
    read.iter()
        .find(|node| node.node == Some(dom))
        .is_some_and(|node| {
            node.properties
                .iter()
                .any(|(name, value)| name == "selected" && value == "true")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option(dom: u64, properties: &[(&str, &str)]) -> AxNode {
        AxNode {
            id: dom.to_string(),
            node: Some(dom),
            role: "option".to_owned(),
            properties: properties
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            ..AxNode::default()
        }
    }

    #[test]
    fn an_option_is_chosen_only_when_chromes_tree_says_it_is_selected() {
        let read = [
            option(4, &[("selected", "false")]),
            option(5, &[("focusable", "true"), ("selected", "true")]),
            option(6, &[]),
        ];
        assert!(!is_selected(&read, 4));
        assert!(is_selected(&read, 5));
        assert!(!is_selected(&read, 6), "saying nothing is not chosen");
        assert!(!is_selected(&read, 7), "a node not read");
    }
}
