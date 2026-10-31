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
//! decoration, hidden from the tree, and so are the icons (queue item 712):
//! *Go*'s arrow and *Add a site*'s plus. A group with nothing in it is not
//! drawn, except *Your sites*, which always offers *Add a site*.
//!
//! # What is typed, and what is offered for it
//!
//! Queue item 701. What a person typed is the field's `value`, set as an
//! attribute's value with `alo-dom`'s operations — so the field shows it
//! in place of its hint, and an agent reads it as the field's value — and
//! never parsed. While the box has the keyboard the screen's `main` is
//! `focused`, which its stylesheet draws as the box's darker border.
//! What is offered for it ([`crate::typed`]) stands between the box and
//! the groups: a `listbox` named *Suggestions*, each offer an `option`
//! named by its words and what they lead to — *Go to the site* and the
//! address; *Search for "…"* or *Ask alo about "…"* and [`NOT_SET_UP`] —
//! the first `aria-selected`, being what Return chooses, and each that is
//! not set up `aria-disabled`. What was typed enters an offer's words as a
//! text node's data, as every stranger's string here does. Nothing typed,
//! nothing offered, and no list.
//!
//! # The icons are ours too
//!
//! Each is an inline `<svg>` in [`TEMPLATE`]. `alo-dom`'s `create_element`
//! makes HTML elements only, so *Add a site*'s plus is not made here: it is
//! written in the template's `<template id="add-icon">`, parsed with the rest
//! of the template, and moved from there into its tile. Nothing a person or a
//! site said reaches an icon.
//!
//! It runs no script: the template has none and nothing here makes one.

use crate::start::{KeptSite, Start, Waiting};
use crate::typed::Offer;
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

/// What the list of offers under the box is called.
pub const SUGGESTIONS: &str = "Suggestions";

/// What the offer to go to an address says.
pub const GO_TO_THE_SITE: &str = "Go to the site";

/// What an offer that cannot yet do what it says says under its words.
pub const NOT_SET_UP: &str = "Not set up yet";

/// The words of the offer to search for `said`.
pub fn search_for(said: &str) -> String {
    format!("Search for \u{201c}{said}\u{201d}")
}

/// The words of the offer to ask alo about `said`.
pub fn ask_about(said: &str) -> String {
    format!("Ask alo about \u{201c}{said}\u{201d}")
}

/// The id of the template's heading, which holds the greeting.
const GREETING: &str = "greeting";

/// The id of the template's place for the groups.
const BLOCKS: &str = "blocks";

/// The id of the template's `main`, the whole screen.
const SCREEN: &str = "start";

/// The id of the template's field.
const FIELD: &str = "ask";

/// The id of the template's one box, around the field and *Go*.
const BOX: &str = "the-box";

/// The id of the template's `<template>` holding *Add a site*'s icon.
const ADD_ICON: &str = "add-icon";

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
    let field = by_id(&document, FIELD)?;
    if !start.typed.is_empty() {
        // A stranger's string, entering as an attribute's value.
        attribute(&mut document, field, "value", &start.typed)?;
    }
    let screen = by_id(&document, SCREEN)?;
    if start.focused {
        attribute(&mut document, screen, "class", "start focused")?;
    }
    let blocks = by_id(&document, BLOCKS)?;
    if !start.offers.is_empty() {
        let offers = make_offers(&mut document, &start.offers)?;
        insert_before(&mut document, screen, offers, blocks)?;
    }
    if let Some(card) = &start.card {
        // A conversation rather than a greeting (queue item 717).
        let class = if start.focused {
            "start conversing focused"
        } else {
            "start conversing"
        };
        attribute(&mut document, screen, "class", class)?;
        let the_box = by_id(&document, BOX)?;
        return crate::card_document::put(&mut document, card, screen, the_box, blocks)
            .map(|()| document);
    }
    let icon = take_icon(&document, ADD_ICON)?;
    let sites = make_sites(&mut document, &start.sites, icon)?;
    append(&mut document, blocks, sites)?;
    if !start.waiting.is_empty() {
        let waiting = make_waiting(&mut document, &start.waiting)?;
        append(&mut document, blocks, waiting)?;
    }
    Ok(document)
}

/// The template's element with `id`.
pub(crate) fn by_id(document: &Document, id: &str) -> Result<NodeId, Unbuilt> {
    document
        .descendants(document.root())
        .find(|&node| document.attribute_by_name(node, "id") == Some(id))
        .ok_or_else(|| Unbuilt {
            why: format!("the first screen's template has no element with the id {id:?}"),
        })
}

/// The icon the template's `<template>` with `id` holds: its one element,
/// SVG as the template's parser made it, to be moved where it is drawn.
fn take_icon(document: &Document, id: &str) -> Result<NodeId, Unbuilt> {
    let holder = by_id(document, id)?;
    document
        .element(holder)
        .and_then(|holder| holder.template_contents)
        .and_then(|contents| {
            document
                .children(contents)
                .find(|&child| document.element(child).is_some())
        })
        .ok_or_else(|| Unbuilt {
            why: format!("the first screen's template {id:?} holds no icon"),
        })
}

/// *Your sites*: a navigation named by its label, a list of tiles, and *Add
/// a site* last, with `icon`.
fn make_sites(
    document: &mut Document,
    sites: &[KeptSite],
    icon: NodeId,
) -> Result<NodeId, Unbuilt> {
    let group = element(document, "nav")?;
    attribute(document, group, "aria-labelledby", "sites-label")?;
    let label = make_label(document, "sites-label", YOUR_SITES)?;
    append(document, group, label)?;
    let list = element(document, "ul")?;
    attribute(document, list, "class", "sites")?;
    for site in sites {
        let mark = site.letter().map(Mark::Letter);
        let tile = make_tile(document, mark, site.label(), false)?;
        append(document, list, tile)?;
    }
    let add = make_tile(document, Some(Mark::Icon(icon)), ADD_A_SITE, true)?;
    append(document, list, add)?;
    append(document, group, list)?;
    Ok(group)
}

/// What stands above a tile's name: a site's letter, or an icon of ours.
enum Mark {
    Letter(String),
    Icon(NodeId),
}

/// One tile, in a list item: a link holding its mark, if it has one, and
/// its name as text.
fn make_tile(
    document: &mut Document,
    mark: Option<Mark>,
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
    match mark {
        Some(Mark::Letter(letter)) => {
            let mark = element(document, "span")?;
            attribute(document, mark, "class", "letter")?;
            attribute(document, mark, "aria-hidden", "true")?;
            let text = document.create_text_node(&letter);
            append(document, mark, text)?;
            append(document, tile, mark)?;
        }
        Some(Mark::Icon(icon)) => append(document, tile, icon)?,
        None => {}
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

/// The offers for what was typed: a list named [`SUGGESTIONS`], each offer
/// an option holding its words and, under them, what it leads to.
fn make_offers(document: &mut Document, offers: &[Offer]) -> Result<NodeId, Unbuilt> {
    let list = element(document, "div")?;
    attribute(document, list, "class", "offers")?;
    attribute(document, list, "role", "listbox")?;
    attribute(document, list, "aria-label", SUGGESTIONS)?;
    for (place, offer) in offers.iter().enumerate() {
        let chosen = place == 0;
        let (words, leads_to) = match offer {
            Offer::Go(url) => (GO_TO_THE_SITE.to_owned(), url.serialised.clone()),
            Offer::Search(said) => (search_for(said), NOT_SET_UP.to_owned()),
            Offer::Ask(said) => (ask_about(said), NOT_SET_UP.to_owned()),
        };
        let option = element(document, "div")?;
        let class = match (chosen, offer.is_set_up()) {
            (true, true) => "offer chosen",
            (true, false) => "offer chosen unset",
            (false, true) => "offer",
            (false, false) => "offer unset",
        };
        attribute(document, option, "class", class)?;
        attribute(document, option, "role", "option")?;
        attribute(
            document,
            option,
            "aria-selected",
            if chosen { "true" } else { "false" },
        )?;
        if !offer.is_set_up() {
            attribute(document, option, "aria-disabled", "true")?;
        }
        for (class, said) in [("what", words), ("where", leads_to)] {
            let part = element(document, "span")?;
            attribute(document, part, "class", class)?;
            // What was typed, and the address it was read as, entering as
            // a text node's data.
            let text = document.create_text_node(&said);
            append(document, part, text)?;
            append(document, option, part)?;
        }
        append(document, list, option)?;
    }
    Ok(list)
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

pub(crate) fn element(document: &mut Document, local: &str) -> Result<NodeId, Unbuilt> {
    document.create_element(local).map_err(|refused| Unbuilt {
        why: format!("a <{local}> could not be made: {refused}"),
    })
}

pub(crate) fn attribute(
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

pub(crate) fn insert_before(
    document: &mut Document,
    parent: NodeId,
    child: NodeId,
    before: NodeId,
) -> Result<(), Unbuilt> {
    document
        .insert_before(parent, child, Some(before))
        .map(|_| ())
        .map_err(|refused| Unbuilt {
            why: format!("a node could not be put into the first screen: {refused}"),
        })
}

pub(crate) fn append(
    document: &mut Document,
    parent: NodeId,
    child: NodeId,
) -> Result<(), Unbuilt> {
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
    fn add_a_site_holds_the_templates_plus_drawn_as_svg_and_hidden() {
        let document = built(&Start::default());
        let tile = document
            .descendants(document.root())
            .find(|&node| document.attribute_by_name(node, "class") == Some("tile add"));
        let icon = tile.and_then(|tile| document.children(tile).next());
        let name = icon
            .and_then(|icon| document.element(icon))
            .map(|icon| &icon.name);
        assert_eq!(
            name.map(|name| (&name.ns, name.local.as_ref())),
            Some((&alo_dom::Namespace::Svg, "svg"))
        );
        assert_eq!(
            icon.and_then(|icon| document.attribute_by_name(icon, "aria-hidden")),
            Some("true")
        );
        // Moved, not copied: the template it was written in is left empty,
        // and the tile's words are still only its name.
        let holder = by_id(&document, ADD_ICON).ok();
        let contents = holder
            .and_then(|holder| document.element(holder))
            .and_then(|holder| holder.template_contents);
        assert_eq!(
            contents.map(|contents| document.children(contents).count()),
            Some(0)
        );
        assert_eq!(of_class(&document, "tile add"), vec![ADD_A_SITE]);
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

    /// What `typed` makes of the first screen: what the browser process
    /// sends for it while the box has the keyboard.
    fn typing(typed: &str) -> Start {
        Start {
            typed: typed.to_owned(),
            focused: true,
            offers: crate::typed::offers(typed),
            ..Start::default()
        }
    }

    #[test]
    fn what_is_typed_is_the_fields_value_and_the_offers_stand_under_the_box() {
        let document = built(&typing("example.com"));
        let field = by_id(&document, FIELD).ok();
        assert_eq!(
            field.and_then(|field| document.attribute_by_name(field, "value")),
            Some("example.com")
        );
        let screen = by_id(&document, SCREEN).ok();
        assert_eq!(
            screen.and_then(|screen| document.attribute_by_name(screen, "class")),
            Some("start focused")
        );
        assert_eq!(
            of_class(&document, "offer chosen"),
            vec!["Go to the sitehttps://example.com/"]
        );
        assert_eq!(
            of_class(&document, "offer unset"),
            vec![
                format!("{}{NOT_SET_UP}", search_for("example.com")),
                format!("{}{NOT_SET_UP}", ask_about("example.com")),
            ]
        );
        // Between the box and the groups.
        let offers = document
            .descendants(document.root())
            .find(|&node| document.attribute_by_name(node, "class") == Some("offers"));
        let next = offers.and_then(|offers| document.next_sibling(offers));
        assert_eq!(next, by_id(&document, BLOCKS).ok());
    }

    #[test]
    fn nothing_typed_is_no_value_and_no_list() {
        let document = built(&Start {
            focused: true,
            ..Start::default()
        });
        let field = by_id(&document, FIELD).ok();
        assert_eq!(
            field.and_then(|field| document.attribute_by_name(field, "value")),
            None
        );
        assert!(of_class(&document, "offers").is_empty());
        let unfocused = built(&Start::default());
        let screen = by_id(&unfocused, SCREEN).ok();
        assert_eq!(
            screen.and_then(|screen| unfocused.attribute_by_name(screen, "class")),
            Some("start")
        );
    }

    #[test]
    fn what_is_typed_that_looks_like_markup_stays_a_value_and_text() {
        for hostile in [
            "<b>bold</b>",
            "</script><script>x()</script>",
            "a\u{0}b",
            "\" onfocus=\"x()",
            &"<i>".repeat(crate::typed::LONGEST_TYPED / 3),
        ] {
            let document = built(&typing(hostile));
            let elements: Vec<String> = document
                .descendants(document.root())
                .filter_map(|node| document.element(node))
                .map(|element| element.name.local.to_string())
                .collect();
            for made in ["script", "b", "i"] {
                assert!(!elements.iter().any(|name| name == made), "{made}");
            }
            let field = by_id(&document, FIELD).ok();
            assert_eq!(
                field.and_then(|field| document.attribute_by_name(field, "value")),
                Some(hostile)
            );
            assert_eq!(
                document
                    .element(field.unwrap_or(document.root()))
                    .map(|field| field.attrs.len()),
                Some(6),
                "no attribute added but the value"
            );
            let said = hostile.trim();
            let texts = document
                .descendants(document.root())
                .filter(|&node| {
                    matches!(document.kind(node), Some(NodeKind::Text(data))
                        if *data == search_for(said) || *data == ask_about(said))
                })
                .count();
            assert_eq!(texts, 2, "{hostile:?}");
        }
    }
}
