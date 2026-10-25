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
//! and text arriving as typing's does (`Input.insertText`) does not reach
//! it — measured against Chrome 155, a date put in that way leaves the
//! field empty. A date is decided here as the engine decides it, a valid
//! date string or nothing, and carried in a segment at a time
//! ([`crate::chrome_date`], item 632).
//!
//! A `<select>` takes no text at all: an option is chosen by activating it,
//! and text on a list is refused by a refusal of its own,
//! `Refusal::Chooses`, by the engine's own rule asked of Chrome's element
//! (`alo_dom::field::is_chosen_from`, ADR 0064 § 4) — never answered as
//! text the page would not take, which would blame a page that cancelled
//! nothing.
//!
//! # An option's list
//!
//! Chrome's tree holds a list's options under it — a `listbox`'s as its
//! children, a closed list's `combobox` under a `MenuListPopup` — and an
//! `<optgroup>` as a `group` between (ADR 0064, *What was measured*). So the
//! list an option is chosen in is the nearest `listbox` or `combobox` above
//! it in the tree ([`ChromeReading::list_above`]), and it is one only when
//! both elements are described and are HTML's: an `<option>`, in a
//! `<select>`. A list a page builds of its own — `role=listbox` and
//! `role=option` on `div`s — is not one, and its option is activated as any
//! other control is, by a click whose meaning is the page's script.

use crate::chrome_protocol::Described;
use alo_agent::{Candidate, ListOf, Reading};
use alo_box::{BoxId, KnownRole, Role};
use alo_dom::node::{Attribute, Element};
use alo_renderer::{Snapshot, SnapshotNode};
use std::collections::HashMap;

/// A page in Chrome, as a verb is decided on it.
#[derive(Debug, Clone, Default)]
pub struct ChromeReading {
    candidates: Vec<Candidate>,
    /// Each node's parent in the tree, by id.
    parents: HashMap<BoxId, BoxId>,
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
            parents: snapshot.root.as_ref().map(parents_in).unwrap_or_default(),
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

    /// The nearest `listbox` or `combobox` above the node `id` in the tree:
    /// the list it is chosen in when it is an option of a `<select>`, which
    /// [`Reading::list_of`] answers once both are described.
    pub(crate) fn list_above(&self, id: BoxId) -> Option<BoxId> {
        let roles: HashMap<BoxId, &Role> = self
            .candidates
            .iter()
            .map(|candidate| (candidate.id, &candidate.role))
            .collect();
        let mut current = self.parents.get(&id).copied();
        // A tree read already holds no loop; the bound keeps this from
        // following one regardless.
        for _ in 0..self.parents.len() {
            let above = current?;
            if matches!(
                roles.get(&above),
                Some(Role::Known(KnownRole::ListBox | KnownRole::ComboBox))
            ) {
                return Some(above);
            }
            current = self.parents.get(&above).copied();
        }
        None
    }

    /// Whether the node `list` was described, and is a `<select>` that
    /// drops down (`alo_dom::choice::drops_down`): a closed list, whose
    /// options Chrome does not draw, so a click cannot be aimed at one.
    pub(crate) fn drops_down(&self, list: BoxId) -> bool {
        self.element(list).is_some_and(|element| {
            alo_dom::field::is_chosen_from(element) && alo_dom::choice::drops_down(element)
        })
    }

    /// Whether the node `id` was described, and is a submit button by this
    /// engine's own rule (`alo_dom::kind::is_submit_button`): a press on it
    /// is what sends its form, and so what Chrome may refuse to send it.
    pub(crate) fn submits(&self, id: BoxId) -> bool {
        self.element(id)
            .is_some_and(alo_dom::kind::is_submit_button)
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
        self.element(id).and_then(alo_dom::field::unbuilt)
    }

    fn max_length(&self, id: BoxId) -> Option<usize> {
        self.element(id).and_then(alo_dom::constraint::max_length)
    }

    fn is_chosen_from(&self, id: BoxId) -> bool {
        self.element(id).is_some_and(alo_dom::field::is_chosen_from)
    }

    /// The `<select>` above the `<option>` `id`, both described (ADR 0064
    /// § 6, queue item 660).
    fn list_of(&self, id: BoxId) -> Option<ListOf> {
        if !self
            .element(id)
            .is_some_and(|option| option.name.is_html("option"))
        {
            return None;
        }
        let list = self.list_above(id)?;
        let element = self
            .element(list)
            .filter(|element| alo_dom::field::is_chosen_from(element))?;
        Some(ListOf {
            list,
            several: alo_dom::choice::takes_several(element),
        })
    }
}

/// Each node's parent under `root`, by id.
fn parents_in(root: &SnapshotNode) -> HashMap<BoxId, BoxId> {
    let mut parents = HashMap::new();
    let mut waiting = vec![root];
    while let Some(node) = waiting.pop() {
        for child in &node.children {
            parents.insert(child.id, node.id);
            waiting.push(child);
        }
    }
    parents
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
        // A date that is one is put, a segment at a time (item 632).
        assert_eq!(
            perform(
                &reading,
                &named("Day"),
                &Verb::PutText("2026-10-12".to_owned())
            ),
            Ok(Outcome::TextPut {
                node: id(4),
                text: "2026-10-12".to_owned(),
            })
        );
        assert_eq!(reading.unbuilt_field(id(4)), None);
        let select =
            ChromeReading::of(&page()).with_element(id(3), &described("SELECT", "select", &[]));
        assert_eq!(select.unbuilt_field(id(3)), None);
        assert!(select.is_chosen_from(id(3)));
        assert_eq!(
            perform(
                &select,
                &Target::Node(id(3)),
                &Verb::PutText("x".to_owned())
            ),
            Err(Refusal::Chooses { node: id(3) })
        );
        assert!(!reading.is_chosen_from(id(4)), "a date field is typed into");
    }

    #[test]
    fn an_option_is_chosen_in_the_select_above_it() {
        let id = BoxId::from_wire;
        let option =
            |at: usize, name: &str| node(at, Role::Known(KnownRole::Option), name, Vec::new());
        // As Chrome 155 reads them: a `listbox` with an `<optgroup>`, a
        // closed list's `combobox` with its `MenuListPopup`, and a list a
        // page builds of `div`s.
        let page = Snapshot {
            root: Some(node(
                1,
                Role::Known(KnownRole::Document),
                "Lists",
                vec![
                    node(
                        2,
                        Role::Known(KnownRole::ListBox),
                        "Toppings",
                        vec![
                            option(3, "Bacon"),
                            node(
                                4,
                                Role::Known(KnownRole::Group),
                                "Greens",
                                vec![option(5, "Onion")],
                            ),
                        ],
                    ),
                    node(
                        6,
                        Role::Known(KnownRole::ComboBox),
                        "Crust",
                        vec![node(
                            7,
                            Role::Declared("MenuListPopup".into()),
                            "",
                            vec![option(8, "Deep")],
                        )],
                    ),
                    node(
                        9,
                        Role::Known(KnownRole::ListBox),
                        "Own",
                        vec![option(10, "Picked")],
                    ),
                ],
            )),
        };
        let reading = ChromeReading::of(&page)
            .with_element(id(2), &described("SELECT", "select", &[("multiple", "")]))
            .with_element(id(5), &described("OPTION", "option", &[]))
            .with_element(id(6), &described("SELECT", "select", &[]))
            .with_element(id(8), &described("OPTION", "option", &[]))
            .with_element(id(9), &described("DIV", "div", &[("role", "listbox")]))
            .with_element(id(10), &described("DIV", "div", &[("role", "option")]));
        assert_eq!(reading.list_above(id(5)), Some(id(2)), "through its group");
        assert_eq!(reading.list_above(id(8)), Some(id(6)), "through the popup");
        assert_eq!(reading.list_above(id(2)), None);
        assert_eq!(
            reading.list_of(id(5)),
            Some(ListOf {
                list: id(2),
                several: true
            })
        );
        assert_eq!(
            reading.list_of(id(8)),
            Some(ListOf {
                list: id(6),
                several: false
            })
        );
        assert!(reading.drops_down(id(6)));
        assert!(!reading.drops_down(id(2)));
        assert!(!reading.drops_down(id(9)), "not a <select>");
        assert_eq!(reading.list_of(id(10)), None, "a page's own list");
        assert_eq!(reading.list_of(id(3)), None, "an option not described");
        assert_eq!(
            perform(&reading, &named("Deep"), &Verb::Activate),
            Ok(Outcome::Chosen {
                node: id(8),
                list: id(6),
                chosen: true
            })
        );
        assert_eq!(
            perform(&reading, &named("Picked"), &Verb::Activate),
            Ok(Outcome::Activated {
                node: id(10),
                name: Some("Picked".to_owned())
            })
        );
    }

    #[test]
    fn a_submit_button_is_one_by_this_engines_rule() {
        let id = BoxId::from_wire;
        let button = |attributes: &[(&str, &str)]| {
            ChromeReading::of(&page())
                .with_element(id(2), &described("BUTTON", "button", attributes))
                .submits(id(2))
        };
        assert!(button(&[]), "a button with no type submits");
        assert!(button(&[("type", "SUBMIT")]));
        assert!(button(&[("type", "nonsense")]), "an unknown type submits");
        assert!(!button(&[("type", "button")]));
        assert!(!button(&[("type", "reset")]));
        let input = ChromeReading::of(&page())
            .with_element(id(3), &described("INPUT", "input", &[("type", "submit")]));
        assert!(input.submits(id(3)));
        assert!(
            !ChromeReading::of(&page()).submits(id(2)),
            "a node never described is not known to submit"
        );
        let svg =
            ChromeReading::of(&page()).with_element(id(2), &described("button", "button", &[]));
        assert!(!svg.submits(id(2)), "not HTML's button");
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
