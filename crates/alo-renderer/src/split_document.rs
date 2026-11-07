/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The split view, built into the first screen's document while alo's
//! agent works on the live card (queue item 819, `docs/design/shell.md`,
//! *The agent at work*), the way the rest of the first screen is built
//! ([`crate::start_document`]): every element made with `alo-dom`'s
//! operations, every stranger's string a text node.
//!
//! # What is drawn
//!
//! In place of the conversation, two things side by side
//! ([`crate::split_place`] has their numbers, `interface/start.css` *The
//! split view* lays them out):
//!
//! - **The page**, large, on the left: the card as the conversation draws
//!   it — its bar, its page outlined in the acting colour — without the
//!   agent's column under it ([`crate::card_document::make_beside`]).
//! - **The agent's column**, on the right: the agent at work as
//!   [`crate::agent_document`] makes it — the working line with *Stop*,
//!   what was asked, the steps, the question with *Allow* and *Not now* —
//!   then, at its foot, what is offered for what was typed and the one box.
//!
//! The conversation's earlier turns are not drawn while the agent works;
//! they are kept, and the conversation is drawn again as soon as the run
//! ends.
//!
//! # What an agent reads
//!
//! ADR 0002: the card's `region` named by the page's address, as in the
//! conversation, and a `complementary` named [`BESIDE`] holding the region
//! [`crate::agent_document::AT_WORK`], the offers and the box. What was
//! said earlier is not read while it is not drawn.

use crate::agent_document;
use crate::card_document;
use crate::conversation_document::take_the_greeting;
use crate::start::Start;
use crate::start_document::{Unbuilt, append, attribute, by_id, element, insert_before};
use alo_dom::{Document, NodeId};

/// What the agent's column is named, to an agent reading the screen.
pub const BESIDE: &str = "alo beside the page";

/// The box's hint in the agent's column (queue item 827): the first
/// screen's *Ask anything, search, or type a website…* is 292.72 wide at
/// 16 px in Instrument Sans and the column's field 263, so it was cut
/// mid-word (*…type a webs*); without *anything* it says the same three
/// things in 223.98 — and in 239.31 in Inter, the widest of the five faces
/// *Settings* offers (queue items 853, 854), so whole in each. The field's
/// name, read by an agent, is unchanged.
pub const COLUMN_HINT: &str = "Ask, search, or type a website\u{2026}";

/// Put the split view for `start` into `document`, the first screen's,
/// inside `screen`, its `main`: the greeting, its line and `blocks` taken
/// out, the card's page on the left, and the agent's column on the right
/// holding `offers`, if any, and `the_box` at its foot.
///
/// # Errors
///
/// [`Unbuilt`] if `start` holds no live card, the template lacks a place it
/// names, or `alo-dom` refused an operation, which no card can cause.
pub fn put(
    document: &mut Document,
    start: &Start,
    screen: NodeId,
    (the_box, blocks): (NodeId, NodeId),
    offers: Option<NodeId>,
) -> Result<(), Unbuilt> {
    let card = start.card().ok_or_else(|| Unbuilt {
        why: "the split view was asked for with no live card".to_owned(),
    })?;
    take_the_greeting(document, screen, blocks)?;
    let page = element(document, "div")?;
    attribute(document, page, "class", "beside-page")?;
    let made = card_document::make_beside(document, card)?;
    append(document, page, made)?;
    insert_before(document, screen, page, the_box)?;
    let column = element(document, "aside")?;
    attribute(document, column, "class", "beside")?;
    attribute(document, column, "aria-label", BESIDE)?;
    if let Some(work) = &card.agent {
        let agent = agent_document::make(document, work)?;
        append(document, column, agent)?;
    }
    document
        .remove_child(screen, the_box)
        .map_err(|refused| Unbuilt {
            why: format!("the box could not be taken out of the first screen: {refused}"),
        })?;
    if let Some(offers) = offers {
        append(document, column, offers)?;
    }
    append(document, column, the_box)?;
    append(document, screen, column)?;
    let field = by_id(document, crate::start_document::FIELD)?;
    attribute(document, field, "placeholder", COLUMN_HINT)
}

#[cfg(test)]
mod tests {
    use crate::agent_work::{AtWork, Now, Step};
    use crate::card::Card;
    use crate::start::Start;
    use crate::start_document::build;
    use crate::turn::Turn;
    use alo_dom::Document;
    use alo_layout::Size;

    fn at_work(now: Now, width: f32) -> Start {
        let mut card = Card::new("a page", alo_url::parse("https://example.com/").unwrap());
        card.agent = Some(AtWork {
            asked: "send my name".to_owned(),
            steps: vec![Step {
                line: "Typing \u{201c}Ada\u{201d} into Name".to_owned(),
                why: String::new(),
            }],
            now,
            choosing: None,
        });
        Start {
            greeting: "Hello.".to_owned(),
            viewport: Size::new(width, 820.0),
            turns: vec![
                Turn::Visited {
                    said: "earlier".to_owned(),
                    url: alo_url::parse("https://earlier.example/").unwrap(),
                    ran: Vec::new(),
                },
                Turn::Page(card),
            ],
            ..Start::default()
        }
    }

    fn built(start: &Start) -> Document {
        build(start).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why))
    }

    fn of_class(document: &Document, class: &str) -> Vec<alo_dom::NodeId> {
        document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some(class))
            .collect()
    }

    fn class_of(document: &Document, node: Option<alo_dom::NodeId>) -> Option<&str> {
        node.and_then(|node| document.attribute_by_name(node, "class"))
    }

    #[test]
    fn while_it_acts_the_page_and_the_column_stand_side_by_side_and_the_box_at_its_foot() {
        let document = built(&at_work(Now::Acting, 1216.0));
        let screen = document
            .descendants(document.root())
            .find(|&node| document.attribute_by_name(node, "id") == Some("start"));
        assert_eq!(class_of(&document, screen), Some("start split"));
        // Its elements with a class, the template's own `<template>`s
        // and the text between them aside.
        let children: Vec<_> = screen
            .map(|screen| {
                document
                    .children(screen)
                    .filter(|&child| class_of(&document, Some(child)).is_some())
                    .collect()
            })
            .unwrap_or_default();
        let classes: Vec<_> = children
            .iter()
            .map(|&child| class_of(&document, Some(child)))
            .collect();
        assert_eq!(classes, vec![Some("beside-page"), Some("beside")]);
        // The card on the left, outlined, with no column of its own.
        assert_eq!(of_class(&document, "card acting").len(), 1);
        assert_eq!(of_class(&document, "ring").len(), 1);
        assert_eq!(of_class(&document, "agent").len(), 1);
        let agent = of_class(&document, "agent")[0];
        assert_eq!(
            class_of(&document, document.parent(agent)),
            Some("beside"),
            "the column beside the page, not in the card"
        );
        // The box the column's last.
        let column = children[1];
        let last = document
            .children(column)
            .filter(|&child| class_of(&document, Some(child)).is_some())
            .last();
        assert_eq!(class_of(&document, last), Some("box"));
        // Nothing of the conversation: no thread, no earlier turn.
        assert!(of_class(&document, "thread").is_empty());
        assert!(of_class(&document, "visit").is_empty());
        assert!(of_class(&document, "hello").is_empty());
    }

    #[test]
    fn asking_splits_too_and_the_question_is_in_the_column() {
        let document = built(&at_work(
            Now::Asking {
                what: "Press \u{201c}Send\u{201d}?".to_owned(),
            },
            1216.0,
        ));
        assert_eq!(of_class(&document, "abtn allow").len(), 1);
        assert_eq!(of_class(&document, "beside").len(), 1);
    }

    #[test]
    fn narrower_than_the_least_or_once_it_ends_the_conversation_is_drawn() {
        for start in [
            at_work(Now::Acting, 1137.0),
            at_work(
                Now::Done {
                    said: "Sent.".to_owned(),
                },
                1216.0,
            ),
        ] {
            let document = built(&start);
            assert!(of_class(&document, "beside").is_empty());
            assert_eq!(of_class(&document, "thread").len(), 1);
            assert_eq!(of_class(&document, "visit").len(), 1);
            let agent = of_class(&document, "agent");
            assert_eq!(agent.len(), 1);
            let card = document.parent(agent[0]);
            assert!(
                class_of(&document, card).is_some_and(|class| class.starts_with("card")),
                "the column under the page, in the card"
            );
        }
    }
}
