/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The first screen's document, built from its state (ADR 0074, queue item
//! 700), the way the chats panel's is ([`crate::panel_document`]): the only
//! markup is [`TEMPLATE`], ours and fixed, and the greeting, every site and
//! every waiting chat are put into it with `alo-dom`'s operations.
//!
//! # What an agent reads
//!
//! ADR 0002: a `main` holding the greeting as a heading, a `search` holding
//! the one field — named *Ask anything, search, or type a website* — and its
//! *Go* button; then *Your sites*, a navigation of links, each named by its
//! site, with *Add a site* last; then *Pick up where you left off*, a list of
//! links each named by its chat and what is waiting. A site's letter is
//! decoration, hidden from the tree. A group with nothing in it is not drawn,
//! except *Your sites*, which always offers *Add a site*.
//!
//! It runs no script: the template has none and nothing here makes one.

use crate::start::{KeptSite, Start, Waiting};
use alo_dom::{Document, NodeId, parse_document};

/// The first screen's markup, compiled in.
pub const TEMPLATE: &str = include_str!("../interface/start.html");

/// The first screen's stylesheet, after the owner's design, compiled in.
pub const SHEET: &str = include_str!("../interface/start.css");

/// What the sites' group is called.
pub const YOUR_SITES: &str = "Your sites";

/// What the last tile, which adds one, says.
pub const ADD_A_SITE: &str = "Add a site";

/// What the waiting chats' group is called.
pub const WAITING: &str = "Pick up where you left off";

/// The id of the template's heading, which holds the greeting.
const GREETING: &str = "greeting";

/// The id of the template's place for the groups.
const BLOCKS: &str = "blocks";

/// Why the first screen could not be built: an `alo-dom` operation refused,
/// or the template lacked a place it names. Either is a defect in this file
/// or its template, never in what the browser process sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unbuilt {
    /// In words.
    pub why: String,
}

/// The first screen's document for `start`.
///
/// # Errors
///
/// [`Unbuilt`] if the template lacks the greeting or the place for the
/// groups, or `alo-dom` refused an operation, which no state can cause.
pub fn build(start: &Start) -> Result<Document, Unbuilt> {
    let mut document = parse_document(TEMPLATE);
    let greeting = by_id(&document, GREETING)?;
    let words = document.create_text_node(&start.greeting);
    append(&mut document, greeting, words)?;
    let blocks = by_id(&document, BLOCKS)?;
    let sites = make_sites(&mut document, &start.sites)?;
    append(&mut document, blocks, sites)?;
    if !start.waiting.is_empty() {
        let waiting = make_waiting(&mut document, &start.waiting)?;
        append(&mut document, blocks, waiting)?;
    }
    Ok(document)
}

/// The template's element with `id`.
fn by_id(document: &Document, id: &str) -> Result<NodeId, Unbuilt> {
    document
        .descendants(document.root())
        .find(|&node| document.attribute_by_name(node, "id") == Some(id))
        .ok_or_else(|| Unbuilt {
            why: format!("the first screen's template has no element with the id {id:?}"),
        })
}

/// *Your sites*: a navigation named by its label, a list of tiles, and *Add
/// a site* last.
fn make_sites(document: &mut Document, sites: &[KeptSite]) -> Result<NodeId, Unbuilt> {
    let group = element(document, "nav")?;
    attribute(document, group, "aria-labelledby", "sites-label")?;
    let label = make_label(document, "sites-label", YOUR_SITES)?;
    append(document, group, label)?;
    let list = element(document, "ul")?;
    attribute(document, list, "class", "sites")?;
    for site in sites {
        let tile = make_tile(document, site.letter().as_deref(), site.label(), false)?;
        append(document, list, tile)?;
    }
    let add = make_tile(document, None, ADD_A_SITE, true)?;
    append(document, list, add)?;
    append(document, group, list)?;
    Ok(group)
}

/// One tile, in a list item: a link holding its letter, if it has one, and
/// its name as text.
fn make_tile(
    document: &mut Document,
    letter: Option<&str>,
    name: &str,
    adds: bool,
) -> Result<NodeId, Unbuilt> {
    let item = element(document, "li")?;
    let tile = element(document, "a")?;
    attribute(
        document,
        tile,
        "class",
        if adds { "tile add" } else { "tile" },
    )?;
    attribute(
        document,
        tile,
        "href",
        if adds { "#add-a-site" } else { "#site" },
    )?;
    if let Some(letter) = letter {
        let mark = element(document, "span")?;
        attribute(document, mark, "class", "letter")?;
        attribute(document, mark, "aria-hidden", "true")?;
        let text = document.create_text_node(letter);
        append(document, mark, text)?;
        append(document, tile, mark)?;
    }
    let said = element(document, "span")?;
    attribute(document, said, "class", "name")?;
    // A stranger's string, entering as a text node's data.
    let text = document.create_text_node(name);
    append(document, said, text)?;
    append(document, tile, said)?;
    append(document, item, tile)?;
    Ok(item)
}

/// *Pick up where you left off*: a section named by its label, holding a
/// list of the chats waiting.
fn make_waiting(document: &mut Document, waiting: &[Waiting]) -> Result<NodeId, Unbuilt> {
    let group = element(document, "section")?;
    attribute(document, group, "aria-labelledby", "waiting-label")?;
    let label = make_label(document, "waiting-label", WAITING)?;
    append(document, group, label)?;
    let list = element(document, "ul")?;
    attribute(document, list, "class", "waiting")?;
    for chat in waiting {
        let item = element(document, "li")?;
        let link = element(document, "a")?;
        attribute(document, link, "class", "wait")?;
        attribute(document, link, "href", "#chat")?;
        for (class, words) in [
            ("name", crate::strip::longest(&chat.name)),
            ("said", crate::strip::longest(&chat.said)),
        ] {
            let part = element(document, "span")?;
            attribute(document, part, "class", class)?;
            let text = document.create_text_node(words);
            append(document, part, text)?;
            append(document, link, part)?;
        }
        append(document, item, link)?;
        append(document, list, item)?;
    }
    append(document, group, list)?;
    Ok(group)
}

/// A group's label: a heading of the second level, with `id`.
fn make_label(document: &mut Document, id: &str, words: &str) -> Result<NodeId, Unbuilt> {
    let label = element(document, "h2")?;
    attribute(document, label, "class", "label")?;
    attribute(document, label, "id", id)?;
    let text = document.create_text_node(words);
    append(document, label, text)?;
    Ok(label)
}

fn element(document: &mut Document, local: &str) -> Result<NodeId, Unbuilt> {
    document.create_element(local).map_err(|refused| Unbuilt {
        why: format!("a <{local}> could not be made: {refused}"),
    })
}

fn attribute(
    document: &mut Document,
    node: NodeId,
    local: &str,
    value: &str,
) -> Result<(), Unbuilt> {
    document
        .set_attribute(node, local, value)
        .ok_or_else(|| Unbuilt {
            why: format!("{local} could not be set on a node that is not an element"),
        })
}

fn append(document: &mut Document, parent: NodeId, child: NodeId) -> Result<(), Unbuilt> {
    document
        .append_child(parent, child)
        .map(|_| ())
        .map_err(|refused| Unbuilt {
            why: format!("a node could not be put into the first screen: {refused}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::NodeKind;

    fn built(start: &Start) -> Document {
        build(start).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why))
    }

    fn of_class(document: &Document, class: &str) -> Vec<String> {
        document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some(class))
            .map(|node| document.text_content(node))
            .collect()
    }

    #[test]
    fn an_empty_first_screen_greets_and_offers_only_to_add_a_site() {
        let document = built(&Start {
            greeting: "Good morning.".to_owned(),
            ..Start::default()
        });
        assert_eq!(of_class(&document, "hello"), vec!["Good morning."]);
        assert_eq!(of_class(&document, "label"), vec![YOUR_SITES]);
        assert_eq!(of_class(&document, "tile add"), vec![ADD_A_SITE]);
        assert!(of_class(&document, "wait").is_empty());
        let names: Vec<String> = document
            .descendants(document.root())
            .filter_map(|node| document.element(node))
            .map(|element| element.name.local.to_string())
            .collect();
        assert!(!names.iter().any(|name| name == "script"));
    }

    #[test]
    fn sites_and_waiting_chats_are_put_in_order() {
        let document = built(&Start {
            greeting: "Good morning.".to_owned(),
            sites: vec![
                KeptSite::new("Mail", "https://mail.example/"),
                KeptSite::new("", "evhomeguide.org"),
            ],
            waiting: vec![
                Waiting::new("Weekend in Lisbon", "found 3 hotels"),
                Waiting::new("Invoice for March", "draft ready to check"),
            ],
            ..Start::default()
        });
        assert_eq!(
            of_class(&document, "tile"),
            vec!["MMail", "Eevhomeguide.org"]
        );
        assert_eq!(
            of_class(&document, "wait"),
            vec![
                "Weekend in Lisbonfound 3 hotels",
                "Invoice for Marchdraft ready to check"
            ]
        );
        assert_eq!(of_class(&document, "label"), vec![YOUR_SITES, WAITING]);
    }

    #[test]
    fn a_name_that_looks_like_markup_is_one_text_node() {
        let hostile = "</h1><script>x()</script>";
        let document = built(&Start {
            greeting: hostile.to_owned(),
            sites: vec![KeptSite::new(hostile, "a")],
            waiting: vec![Waiting::new(hostile, hostile)],
            ..Start::default()
        });
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
        assert_eq!(texts, 4);
    }
}
