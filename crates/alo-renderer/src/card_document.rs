/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page opened from the conversation, built into the first screen's
//! document as a card (ADR 0076 B, queue item 717), the way the rest of the
//! first screen is built ([`crate::start_document`]): the card's markup is
//! the template's `<template id="card">`, ours and fixed, and what the
//! person said and the page's address enter it as text nodes.
//!
//! # Where it stands
//!
//! In the newest turn of the conversation, under what the person said
//! ([`crate::conversation_document`]): only the newest turn is ever a live
//! card, so there is one card, and one `card-address`, in a document.
//!
//! # What an agent reads
//!
//! ADR 0002: a `region` named by
//! the page's address — the bar's own words, which it is labelled by —
//! holding *Back*, *Forward*, *Expand to the whole window* and *Close* as
//! buttons, *Back* and *Forward* disabled when there is nowhere to go; then
//! the page, as an image named [`PICTURED`] once a frame has arrived; a
//! card whose page a Chromium judges holds its shield after the address
//! (queue item 807): a toggle button named [`shield_named`], pressed while
//! the lists alo carries are asked for the site — `aria-pressed`, not a
//! `switch`, whose `aria-checked` this engine draws a tick for — holding
//! how many requests they refused, or *Off*; and,
//! while the page is still loading, the bar's [`LOADING`] after the address
//! (queue item 751), a paragraph a person and an agent read alike; and,
//! once a person asked alo to work on the page, the agent's column under
//! it ([`crate::agent_document`], queue item 704) — or, in the split view,
//! beside it ([`crate::split_document`], queue item 819). The
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

use crate::agent_document;
use crate::card::{Card, FRAME_SRC};
use crate::resource::Resource;
use crate::start_document::{Unbuilt, append, attribute, by_id, element};
use alo_dom::{Document, NodeId};

/// What the picture of the page is named, to an agent.
pub const PICTURED: &str = "The page, as Chrome last drew it";

/// What the bar says after the address while the page is still loading
/// (queue item 751).
pub const LOADING: &str = "Loading";

/// What a card's shield is named, to an agent, for the site `site`.
pub fn shield_named(site: &str) -> String {
    format!("Block trackers and ads on {site}")
}

/// What a card's shield says while blocking is off for its site.
pub const OFF: &str = "Off";

/// The id of the template's `<template>` holding the card.
const CARD: &str = "card";

/// The id of the card's address, which names it.
const ADDRESS: &str = "card-address";

/// The card for `card`, made from the template's `<template id="card">`
/// and filled — its address, its buttons' states and its picture — for the
/// conversation to put in its turn ([`crate::conversation_document`]).
///
/// # Errors
///
/// [`Unbuilt`] if the template lacks the card or a place in it, or
/// `alo-dom` refused an operation, which no card can cause.
pub fn make(document: &mut Document, card: &Card) -> Result<NodeId, Unbuilt> {
    let made = take_card(document)?;
    fill(document, made, card, Column::Under)?;
    Ok(made)
}

/// The card for `card` as the split view draws it (queue item 819,
/// [`crate::split_document`]): as [`make`] makes it, outlined while the
/// agent works, but without the agent's column under its page, which the
/// split view stands beside it.
///
/// # Errors
///
/// As [`make`].
pub fn make_beside(document: &mut Document, card: &Card) -> Result<NodeId, Unbuilt> {
    let made = take_card(document)?;
    fill(document, made, card, Column::Beside)?;
    Ok(made)
}

/// Where the agent's column goes for a card it works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Column {
    /// Under the page, inside the card (queue item 704).
    Under,
    /// Beside the card, in the split view's column: not in the card.
    Beside,
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
fn fill(document: &mut Document, made: NodeId, card: &Card, column: Column) -> Result<(), Unbuilt> {
    let address = document
        .descendants(made)
        .find(|&node| document.attribute_by_name(node, "id") == Some(ADDRESS))
        .ok_or_else(|| Unbuilt {
            why: format!("the card's template has no element with the id {ADDRESS:?}"),
        })?;
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
    shield(document, made, card)?;
    for (class, open) in [("ibtn back", card.back), ("ibtn forward", card.forward)] {
        if open {
            continue;
        }
        let button = of_class(document, made, class)?;
        attribute(document, button, "class", &format!("{class} off"))?;
        attribute(document, button, "disabled", "")?;
    }
    if card.loading {
        // After the address, before *Expand*.
        let bar = of_class(document, made, "bar")?;
        let expand = of_class(document, made, "ibtn expand")?;
        let loading = element(document, "p")?;
        attribute(document, loading, "class", "loading")?;
        let words = document.create_text_node(LOADING);
        append(document, loading, words)?;
        document
            .insert_before(bar, loading, Some(expand))
            .map_err(|refused| Unbuilt {
                why: format!("the card's loading could not be put in its bar: {refused}"),
            })?;
    }
    if card.frame.is_some() {
        let page = of_class(document, made, "page")?;
        let picture = element(document, "img")?;
        attribute(document, picture, "class", "shot")?;
        attribute(document, picture, "src", FRAME_SRC)?;
        attribute(document, picture, "alt", PICTURED)?;
        append(document, page, picture)?;
    }
    if let Some(menu) = &card.menu {
        // alo's menu over the page, where it was pressed (queue item 798).
        let page = of_class(document, made, "page")?;
        let drawn = crate::card_menu::make(document, menu)?;
        append(document, page, drawn)?;
    }
    if let Some(work) = &card.agent {
        // The agent at work (queue item 704): its column under the page,
        // and, while it works, the page outlined in the acting colour.
        if work.is_working() {
            attribute(document, made, "class", "card acting")?;
            let page = of_class(document, made, "page")?;
            let ring = agent_document::ring(document)?;
            append(document, page, ring)?;
        }
        if column == Column::Under {
            let under = agent_document::make(document, work)?;
            append(document, made, under)?;
        }
    }
    Ok(())
}

/// The card's shield, filled from `card`'s and named for its page's site —
/// what the switch switches, its registrable domain — or taken out of the
/// bar when the card has none.
fn shield(document: &mut Document, made: NodeId, card: &Card) -> Result<(), Unbuilt> {
    let button = of_class(document, made, "shield")?;
    let Some(shield) = card.shield else {
        let _ = document.remove(button);
        return Ok(());
    };
    let site = card
        .url
        .host
        .as_ref()
        .map(alo_url::site::of)
        .unwrap_or_default();
    // The page's site, entering as an attribute's value.
    attribute(document, button, "aria-label", &shield_named(&site))?;
    if !shield.on {
        attribute(document, button, "class", "shield off")?;
        attribute(document, button, "aria-pressed", "false")?;
    }
    let count = element(document, "span")?;
    attribute(document, count, "class", "count")?;
    let words = if shield.on {
        shield.blocked.to_string()
    } else {
        OFF.to_owned()
    };
    let text = document.create_text_node(&words);
    append(document, count, text)?;
    append(document, button, count)?;
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
            turns: vec![crate::turn::Turn::Page(card)],
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
    fn a_page_loading_is_said_after_the_address_until_it_has_loaded() {
        let loading = built(card("a", "https://example.com/"));
        assert_eq!(of_class(&loading, "loading"), vec![super::LOADING]);
        let said = loading
            .descendants(loading.root())
            .find(|&node| loading.attribute_by_name(node, "class") == Some("loading"));
        let class_of = |node: Option<alo_dom::NodeId>| {
            node.and_then(|node| loading.attribute_by_name(node, "class"))
        };
        assert_eq!(
            class_of(said.and_then(|said| loading.previous_sibling(said))),
            Some("url")
        );
        assert_eq!(
            class_of(said.and_then(|said| loading.next_sibling(said))),
            Some("ibtn expand")
        );
        let loaded = built(card("a", "https://example.com/").holding(vec![1]).loaded());
        assert!(of_class(&loaded, "loading").is_empty());
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
