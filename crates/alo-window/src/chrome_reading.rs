/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page in Chrome, as a verb is decided on it (ADR 0058 § 4, queue item
//! 621).
//!
//! `alo-agent` decides every verb against a [`Reading`], whichever renderer
//! drew the page. This is Chrome's: the nodes of the [`Snapshot`] an agent
//! read ([`crate::chrome_tree`]), and for the one node a verb is aimed at,
//! the element behind it as `DOM.describeNode` describes it — so that
//! whether it is a date field, how long a text it takes, and where a link
//! goes are answered by **this engine's own rules** about an element
//! (`alo_dom`), asked of Chrome's, rather than by a second copy of them.
//!
//! Only the node aimed at is described: the rules are asked only of it, and
//! describing every node on a page for each verb would be a round trip to
//! Chrome per node to answer one question.
//!
//! # What a box says, and what its element says
//!
//! Whether text goes into a thing is the role's to say or the box's, in this
//! engine (`alo_agent::verb`). Chrome says it of an editable field and not of
//! the fields it builds of segments — a `time` field is `InputTime`, holding
//! spin buttons, and is not called editable — so for the described element
//! the engine's own rule (`alo_css::state::is_text_entry`) is asked too. A
//! `time` field is then refused as the engine refuses it, by name
//! (`Refusal::Unbuilt`), rather than as a thing text does not go into, which
//! it is not.
//!
//! # Fields typing's text does not reach
//!
//! Chrome builds a `date` field of segments, as it builds a `time` field,
//! and text arriving as typing's does (`Input.insertText`) does not reach it
//! — measured against Chrome 155, a date put in that way leaves the field
//! empty — and a `<select>` takes no text at all. So on Chrome's page both
//! are refused by name as `Refusal::Unbuilt`, beside the kinds the engine
//! refuses, rather than answered as text the page would not take: a page
//! that cancelled nothing would be blamed for a field this browser cannot
//! fill.

use crate::chrome_protocol::Described;
use alo_agent::{Candidate, Reading};
use alo_box::BoxId;
use alo_dom::node::{Attribute, Element};
use alo_renderer::Snapshot;
use std::collections::HashMap;

/// A page in Chrome, as a verb is decided on it.
#[derive(Debug, Clone, Default)]
pub struct ChromeReading {
    candidates: Vec<Candidate>,
    elements: HashMap<BoxId, Element>,
}

impl ChromeReading {
    /// The nodes of `snapshot`, every one an agent can name, with no
    /// element described yet.
    pub fn of(snapshot: &Snapshot) -> Self {
        Self {
            candidates: snapshot
                .nodes()
                .into_iter()
                .map(|node| Candidate {
                    id: node.id,
                    role: node.role.clone(),
                    name: node.name.clone(),
                    states: node.states,
                    scrolls: node.scrolls,
                })
                .collect(),
            elements: HashMap::new(),
        }
    }

    /// The same page, with the node `id`'s element as Chrome `described` it.
    ///
    /// An element that is not HTML's — an SVG `<a>`, a MathML one — has no
    /// facts here: this engine's rules about an element are rules about
    /// HTML's.
    #[must_use]
    pub fn with_element(mut self, id: BoxId, described: &Described) -> Self {
        let Some(element) = html_element(described) else {
            return self;
        };
        if alo_css::state::is_text_entry(&element) {
            for candidate in &mut self.candidates {
                if candidate.id == id {
                    candidate.states.takes_text = true;
                }
            }
        }
        self.elements.insert(id, element);
        self
    }

    fn element(&self, id: BoxId) -> Option<&Element> {
        self.elements.get(&id)
    }
}

/// `described` as an HTML element, or [`None`] for any other: the DOM names
/// an HTML element of an HTML document in upper case, and every other as it
/// was written.
fn html_element(described: &Described) -> Option<Element> {
    if described.local_name.is_empty()
        || described.node_name != described.local_name.to_ascii_uppercase()
    {
        return None;
    }
    let attributes = described
        .attributes
        .iter()
        .map(|(name, value)| Attribute::plain(name, value))
        .collect();
    Some(Element::html(&described.local_name, attributes))
}

impl Reading for ChromeReading {
    fn candidates(&self) -> Vec<Candidate> {
        self.candidates.clone()
    }

    fn href_of(&self, id: BoxId) -> Option<String> {
        self.element(id)?.attr("href").map(str::to_owned)
    }

    fn is_a_date_field(&self, id: BoxId) -> bool {
        self.element(id).is_some_and(alo_dom::date::is_date_field)
    }

    fn unbuilt_field(&self, id: BoxId) -> Option<&'static str> {
        let element = self.element(id)?;
        alo_dom::field::unbuilt(element).or_else(|| {
            if alo_dom::date::is_date_field(element) {
                Some("date")
            } else if element.name.is_html("select") {
                Some("select")
            } else {
                None
            }
        })
    }

    fn max_length(&self, id: BoxId) -> Option<usize> {
        self.element(id).and_then(alo_dom::constraint::max_length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_agent::verb::{Target, Verb, perform};
    use alo_agent::{Outcome, Refusal};
    use alo_box::{KnownRole, Role, States};
    use alo_layout::Rect;
    use alo_renderer::SnapshotNode;

    fn node(id: usize, role: Role, name: &str, children: Vec<SnapshotNode>) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(id),
            role,
            name: Some(name.to_owned()),
            states: States::default(),
            placeholder: None,
            marker: None,
            rect: Rect::ZERO,
            rects: Vec::new(),
            offscreen: false,
            scrolls: false,
            children,
        }
    }

    fn described(node_name: &str, local_name: &str, attributes: &[(&str, &str)]) -> Described {
        Described {
            node_name: node_name.to_owned(),
            local_name: local_name.to_owned(),
            attributes: attributes
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        }
    }

    /// A page as Chrome's tree reads one: a link, a date field, a field that
    /// takes three letters, and Chrome's `time` field, which it does not call
    /// editable.
    fn page() -> Snapshot {
        let mut field = node(3, Role::Known(KnownRole::TextBox), "Short", Vec::new());
        field.states.takes_text = true;
        let mut date = node(4, Role::Known(KnownRole::Date), "Day", Vec::new());
        date.states.takes_text = true;
        Snapshot {
            root: Some(node(
                1,
                Role::Known(KnownRole::Document),
                "A page",
                vec![
                    node(2, Role::Known(KnownRole::Link), "Next", Vec::new()),
                    field,
                    date,
                    node(5, Role::Declared("InputTime".into()), "When", Vec::new()),
                ],
            )),
        }
    }

    fn named(name: &str) -> Target {
        Target::Named(name.to_owned())
    }

    #[test]
    fn a_verb_on_chromes_page_is_decided_by_this_engines_rules() {
        let id = BoxId::from_wire;
        let reading = ChromeReading::of(&page())
            .with_element(id(2), &described("A", "a", &[("href", "/next")]))
            .with_element(id(3), &described("INPUT", "input", &[("maxlength", "3")]))
            .with_element(id(4), &described("INPUT", "input", &[("type", "DATE")]))
            .with_element(id(5), &described("INPUT", "input", &[("type", "time")]));
        assert_eq!(
            perform(&reading, &named("Next"), &Verb::Activate),
            Ok(Outcome::Followed {
                node: id(2),
                to: "/next".to_owned(),
            })
        );
        assert_eq!(
            perform(&reading, &named("Short"), &Verb::PutText("four".to_owned())),
            Err(Refusal::TooLong {
                node: id(3),
                most: 3,
                length: 4,
            })
        );
        assert_eq!(
            perform(&reading, &named("Day"), &Verb::PutText("12/10".to_owned())),
            Err(Refusal::NotADate {
                node: id(4),
                text: "12/10".to_owned(),
            })
        );
        assert_eq!(
            perform(&reading, &named("When"), &Verb::PutText("12:00".to_owned())),
            Err(Refusal::Unbuilt {
                node: id(5),
                kind: "time".to_owned(),
            })
        );
        // A date that is one is refused all the same: Chrome's segments do
        // not take typing's text.
        assert_eq!(
            perform(
                &reading,
                &named("Day"),
                &Verb::PutText("2026-10-12".to_owned())
            ),
            Err(Refusal::Unbuilt {
                node: id(4),
                kind: "date".to_owned(),
            })
        );
        let select =
            ChromeReading::of(&page()).with_element(id(3), &described("SELECT", "select", &[]));
        assert_eq!(select.unbuilt_field(id(3)), Some("select"));
    }

    #[test]
    fn a_node_whose_element_is_not_described_has_no_facts() {
        let reading = ChromeReading::of(&page());
        // A link with no `href` known is a control activated, not followed.
        assert_eq!(
            perform(&reading, &named("Next"), &Verb::Activate),
            Ok(Outcome::Activated {
                node: BoxId::from_wire(2),
                name: Some("Next".to_owned()),
            })
        );
        assert_eq!(
            perform(&reading, &named("When"), &Verb::PutText("12:00".to_owned())),
            Err(Refusal::NotAField {
                node: BoxId::from_wire(5),
                role: Role::Declared("InputTime".into()),
            })
        );
    }

    #[test]
    fn an_element_that_is_not_htmls_says_nothing_of_html() {
        let id = BoxId::from_wire;
        // An SVG `<a>` keeps its name's case; an empty name is no element.
        let reading = ChromeReading::of(&page())
            .with_element(id(2), &described("a", "a", &[("href", "/next")]))
            .with_element(id(3), &described("", "", &[("maxlength", "1")]));
        assert_eq!(reading.href_of(id(2)), None);
        assert_eq!(reading.max_length(id(3)), None);
        assert!(!reading.is_a_date_field(id(4)));
        assert_eq!(reading.unbuilt_field(id(5)), None);
    }
}
