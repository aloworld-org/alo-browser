/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page opened from the conversation, built into the first screen's
//! document as a card (ADR 0076 B, queue item 717), the way the rest of the
//! first screen is built ([`crate::start_document`]): the card's markup is
//! the template's `<template id="card">`, ours and fixed, and what the
//! person said and the page's address enter it as text nodes.
//!
//! # What changes on the screen
//!
//! With a card, the screen is a conversation rather than a greeting: the
//! greeting, the line under it and the groups of sites and waiting chats
//! are not built, and a column 880 wide stands above the one box, holding
//! what the person said, then the card. The screen's `main` is of class
//! `start conversing`, which its sheet draws closer to the top.
//!
//! # What an agent reads
//!
//! ADR 0002: what the person said, as a paragraph; then a `region` named by
//! the page's address — the bar's own words, which it is labelled by —
//! holding *Back*, *Forward*, *Expand to the whole window* and *Close* as
//! buttons, *Back* and *Forward* disabled when there is nowhere to go; then
//! the page, as an image named [`PICTURED`] once a frame has arrived. The
//! icons are decoration and hidden from the tree. What the page itself
//! holds is Chrome's to say, through the agent's socket (ADR 0058), never
//! read off its picture.
//!
//! # The picture
//!
//! Chrome's frame is drawn by an `<img>` whose `src` is
//! [`crate::card::FRAME_SRC`], and the frame's bytes are handed to the
//! drawing as that picture ([`frame`]) — the one way every `<img>` is
//! drawn, decoded in this confined process. No frame yet, no `<img>`: the
//! page box is the card's white.

use crate::card::{Card, FRAME_SRC};
use crate::resource::Resource;
use crate::start_document::{Unbuilt, append, attribute, by_id, element, insert_before};
use alo_dom::{Document, NodeId};

/// What the picture of the page is named, to an agent.
pub const PICTURED: &str = "The page, as Chrome last drew it";

/// The id of the template's `<template>` holding the card.
const CARD: &str = "card";

/// The id of the card's address, which names it.
const ADDRESS: &str = "card-address";

/// Put `card` into `document`, the first screen's, above `before` — the one
/// box — inside `screen`, its `main`, taking the screen's greeting, its line
/// and `blocks` out.
///
/// # Errors
///
/// [`Unbuilt`] if the template lacks the card or a place in it, or
/// `alo-dom` refused an operation, which no card can cause.
pub fn put(
    document: &mut Document,
    card: &Card,
    screen: NodeId,
    before: NodeId,
    blocks: NodeId,
) -> Result<(), Unbuilt> {
    let sub = document
        .children(screen)
        .find(|&child| document.attribute_by_name(child, "class") == Some("sub"));
    for gone in [Some(by_id(document, "greeting")?), sub, Some(blocks)]
        .into_iter()
        .flatten()
    {
        document
            .remove_child(screen, gone)
            .map_err(|refused| Unbuilt {
                why: format!("a node could not be taken out of the first screen: {refused}"),
            })?;
    }
    let thread = element(document, "div")?;
    attribute(document, thread, "class", "thread")?;
    let said = element(document, "p")?;
    attribute(document, said, "class", "you")?;
    // A stranger's string, entering as a text node's data.
    let words = document.create_text_node(&card.said);
    append(document, said, words)?;
    append(document, thread, said)?;
    let made = take_card(document)?;
    append(document, thread, made)?;
    insert_before(document, screen, thread, before)?;
    fill(document, made, card)
}

/// The picture the card's `<img>` names, if it holds a frame: what the
/// drawing is handed beside the document.
pub fn frame(card: &Card) -> Option<Resource> {
    card.frame
        .as_ref()
        .map(|png| Resource::new(FRAME_SRC, None, png.clone()))
}

/// The card the template's `<template id="card">` holds, moved out of it.
fn take_card(document: &Document) -> Result<NodeId, Unbuilt> {
    let holder = by_id(document, CARD)?;
    document
        .element(holder)
        .and_then(|holder| holder.template_contents)
        .and_then(|contents| {
            document
                .children(contents)
                .find(|&child| document.element(child).is_some())
        })
        .ok_or_else(|| Unbuilt {
            why: "the first screen's template holds no card".to_owned(),
        })
}

/// The card's address, its buttons' states and its picture.
fn fill(document: &mut Document, made: NodeId, card: &Card) -> Result<(), Unbuilt> {
    let address = by_id(document, ADDRESS)?;
    let (host, rest) = card.address();
    for (class, words) in [("host", host), ("path", rest)] {
        if words.is_empty() {
            continue;
        }
        let part = element(document, "span")?;
        attribute(document, part, "class", class)?;
        // The page's address, entering as a text node's data.
        let text = document.create_text_node(&words);
        append(document, part, text)?;
        append(document, address, part)?;
    }
    for (class, open) in [("ibtn back", card.back), ("ibtn forward", card.forward)] {
        if open {
            continue;
        }
        let button = of_class(document, made, class)?;
        attribute(document, button, "class", &format!("{class} off"))?;
        attribute(document, button, "disabled", "")?;
    }
    if card.frame.is_some() {
        let page = of_class(document, made, "page")?;
        let picture = element(document, "img")?;
        attribute(document, picture, "class", "shot")?;
        attribute(document, picture, "src", FRAME_SRC)?;
        attribute(document, picture, "alt", PICTURED)?;
        append(document, page, picture)?;
    }
    Ok(())
}

/// The element under `inside` whose whole class is `class`.
fn of_class(document: &Document, inside: NodeId, class: &str) -> Result<NodeId, Unbuilt> {
    document
        .descendants(inside)
        .find(|&node| document.attribute_by_name(node, "class") == Some(class))
        .ok_or_else(|| Unbuilt {
            why: format!("the card's template has no element of the class {class:?}"),
        })
}

#[cfg(test)]
mod tests {
    use crate::card::Card;
    use crate::start::Start;
    use crate::start_document::build;
    use alo_dom::{Document, NodeKind};

    fn card(said: &str, address: &str) -> Card {
        Card::new(said, alo_url::parse(address).unwrap())
    }

    fn built(card: Card) -> Document {
        build(&Start {
            greeting: "Hello.".to_owned(),
            card: Some(card),
            ..Start::default()
        })
        .unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why))
    }

    fn of_class(document: &Document, class: &str) -> Vec<String> {
        document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some(class))
            .map(|node| document.text_content(node))
            .collect()
    }

    #[test]
    fn a_card_takes_the_greeting_and_groups_and_stands_above_the_box() {
        let document = built(card("example.com", "https://example.com/a?b"));
        assert!(of_class(&document, "hello").is_empty());
        assert!(of_class(&document, "sub").is_empty());
        assert!(of_class(&document, "blocks").is_empty());
        assert_eq!(of_class(&document, "you"), vec!["example.com"]);
        assert_eq!(of_class(&document, "host"), vec!["example.com"]);
        assert_eq!(of_class(&document, "path"), vec!["/a?b"]);
        assert_eq!(of_class(&document, "ibtn back off").len(), 1);
        assert_eq!(of_class(&document, "ibtn forward off").len(), 1);
        assert_eq!(of_class(&document, "ibtn expand").len(), 1);
        assert_eq!(of_class(&document, "ibtn close").len(), 1);
        let thread = document
            .descendants(document.root())
            .find(|&node| document.attribute_by_name(node, "class") == Some("thread"));
        let next = thread.and_then(|thread| document.next_sibling(thread));
        assert_eq!(
            next.and_then(|next| document.attribute_by_name(next, "class")),
            Some("box")
        );
        assert!(
            of_class(&document, "shot").is_empty(),
            "no frame, no picture"
        );
    }

    #[test]
    fn a_frame_is_a_picture_and_somewhere_to_go_is_not_greyed() {
        let mut open = card("a", "https://example.com/").holding(vec![1, 2, 3]);
        open.back = true;
        open.forward = true;
        let document = built(open.clone());
        assert_eq!(of_class(&document, "ibtn back").len(), 1);
        assert_eq!(of_class(&document, "ibtn forward").len(), 1);
        let picture = document
            .descendants(document.root())
            .find(|&node| document.attribute_by_name(node, "class") == Some("shot"));
        assert_eq!(
            picture.and_then(|picture| document.attribute_by_name(picture, "src")),
            Some(crate::card::FRAME_SRC)
        );
        assert_eq!(
            super::frame(&open).map(|frame| frame.bytes),
            Some(vec![1, 2, 3])
        );
    }

    #[test]
    fn what_the_person_said_and_the_address_stay_text() {
        let hostile = "</p><script>x()</script>";
        let document = built(card(
            hostile,
            "https://example.com/%3C/p%3E%3Cscript%3Ex()%3C/script%3E",
        ));
        let scripts = document
            .descendants(document.root())
            .filter_map(|node| document.element(node))
            .filter(|element| element.name.local.as_ref() == "script")
            .count();
        assert_eq!(scripts, 0);
        let texts = document
            .descendants(document.root())
            .filter(|&node| matches!(document.kind(node), Some(NodeKind::Text(data)) if data == hostile))
            .count();
        assert_eq!(texts, 1);
    }
}
