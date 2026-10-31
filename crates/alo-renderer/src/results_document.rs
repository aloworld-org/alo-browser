/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search's results, built into the first screen's document as the
//! design's All tab (queue item 708, ADR 0083), the way a card is
//! ([`crate::card_document`]): the tab row is the template's `<template
//! id="results">`, ours and fixed, and what the person said and every
//! site's name, address, title and description enter it as text nodes.
//!
//! # What changes on the screen
//!
//! As with a card, the screen is a conversation: the greeting, its line and
//! the groups are not built, and a column 880 wide stands above the one
//! box, holding what the person said, then the results.
//!
//! # What an agent reads
//!
//! ADR 0002: what the person said, as a paragraph; then a `region` named
//! *Search results*, holding a `tablist` named *Kinds of result* — *All*
//! selected, the rest disabled — then each site as an `article`: its name
//! and address as text, its title as a heading holding a link, and its
//! description as a paragraph; then *More sites ›*, a disabled button. Or,
//! where the sites would be, one sentence: searching, nothing found, or why
//! the search was not answered. A site's letter is decoration, hidden from
//! the tree.
//!
//! # A title is a link to its page
//!
//! A press on a site's title is claimed by its place among the titles
//! ([`crate::conversation_claim`]), and the browser process opens the page
//! it holds for that place in the card, as an address chosen from the box
//! opens. The link's `href` is a name of ours, [`TITLE_HREF`], never the
//! site's address: nothing a stranger wrote is an attribute here.

use crate::results::{Found, Results};
use crate::start_document::{Unbuilt, append, attribute, by_id, element, insert_before};
use alo_dom::{Document, NodeId};

/// What a site's title links to in this document: a name of ours.
pub const TITLE_HREF: &str = "#found";

/// What *More sites ›* says.
pub const MORE_SITES: &str = "More sites \u{203a}";

/// The id of the template's `<template>` holding the results.
const RESULTS: &str = "results";

/// Put `results` into `document`, the first screen's, above `before` — the
/// one box — inside `screen`, its `main`, taking the screen's greeting, its
/// line and `blocks` out.
///
/// # Errors
///
/// [`Unbuilt`] if the template lacks the results or a place in them, or
/// `alo-dom` refused an operation, which no results can cause.
pub fn put(
    document: &mut Document,
    results: &Results,
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
    attribute(document, thread, "class", "thread searched")?;
    let said = element(document, "p")?;
    attribute(document, said, "class", "you")?;
    // A stranger's string, entering as a text node's data.
    let words = document.create_text_node(&results.said);
    append(document, said, words)?;
    append(document, thread, said)?;
    let made = take_results(document)?;
    append(document, thread, made)?;
    insert_before(document, screen, thread, before)?;
    fill(document, made, results)
}

/// The results the template's `<template id="results">` holds, moved out
/// of it.
fn take_results(document: &Document) -> Result<NodeId, Unbuilt> {
    let holder = by_id(document, RESULTS)?;
    document
        .element(holder)
        .and_then(|holder| holder.template_contents)
        .and_then(|contents| {
            document
                .children(contents)
                .find(|&child| document.element(child).is_some())
        })
        .ok_or_else(|| Unbuilt {
            why: "the first screen's template holds no results".to_owned(),
        })
}

/// The sites, or the sentence said in their place, then *More sites ›*.
fn fill(document: &mut Document, made: NodeId, results: &Results) -> Result<(), Unbuilt> {
    let list = document
        .descendants(made)
        .find(|&node| document.attribute_by_name(node, "class") == Some("res"))
        .ok_or_else(|| Unbuilt {
            why: "the results' template has no list of sites".to_owned(),
        })?;
    if let Some(note) = results.note() {
        let said = element(document, "p")?;
        attribute(document, said, "class", "note")?;
        // The provider's refusal, or what was typed, as a text node's data.
        let text = document.create_text_node(&note);
        append(document, said, text)?;
        append(document, list, said)?;
        return Ok(());
    }
    for found in results.on_all() {
        let site = make_site(document, found)?;
        append(document, list, site)?;
    }
    let more = element(document, "button")?;
    attribute(document, more, "class", "more")?;
    attribute(document, more, "type", "button")?;
    attribute(document, more, "disabled", "")?;
    let words = document.create_text_node(MORE_SITES);
    append(document, more, words)?;
    append(document, made, more)?;
    Ok(())
}

/// One site: its mark, name and address on a line; its title as a link;
/// its description.
fn make_site(document: &mut Document, found: &Found) -> Result<NodeId, Unbuilt> {
    let article = element(document, "article")?;
    attribute(document, article, "class", "found")?;
    let source = element(document, "div")?;
    attribute(document, source, "class", "src")?;
    if let Some(letter) = found.letter() {
        let mark = element(document, "span")?;
        attribute(
            document,
            mark,
            "class",
            &format!("fav tint{}", found.tint()),
        )?;
        attribute(document, mark, "aria-hidden", "true")?;
        let text = document.create_text_node(&letter);
        append(document, mark, text)?;
        append(document, source, mark)?;
    }
    for (class, words) in [("name", found.site()), ("addr", found.address())] {
        let part = element(document, "span")?;
        attribute(document, part, "class", class)?;
        // The provider's name for the site and its address, as text.
        let text = document.create_text_node(&words);
        append(document, part, text)?;
        append(document, source, part)?;
    }
    append(document, article, source)?;
    let heading = element(document, "h3")?;
    attribute(document, heading, "class", "t")?;
    let link = element(document, "a")?;
    attribute(document, link, "class", "title")?;
    attribute(document, link, "href", TITLE_HREF)?;
    // The page's title, a stranger's string, as a text node's data.
    let title = document.create_text_node(&found.title);
    append(document, link, title)?;
    append(document, heading, link)?;
    append(document, article, heading)?;
    if !found.description.is_empty() {
        let said = element(document, "p")?;
        attribute(document, said, "class", "d")?;
        let text = document.create_text_node(&found.description);
        append(document, said, text)?;
        append(document, article, said)?;
    }
    Ok(article)
}

#[cfg(test)]
mod tests {
    use crate::results::{Found, Results, Showing};
    use crate::start::Start;
    use crate::start_document::build;
    use alo_dom::{Document, NodeKind};

    fn found(title: &str, address: &str) -> Found {
        Found::new(title, alo_url::parse(address).unwrap(), "Two lines of it.")
    }

    fn built(results: Results) -> Document {
        build(&Start {
            greeting: "Hello.".to_owned(),
            results: Some(results),
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
    fn found_sites_take_the_greeting_and_stand_above_the_box() {
        let document = built(Results {
            said: "ev charger".to_owned(),
            showing: Showing::Found(vec![
                found("One", "https://one.example/a/b").named("One Guide"),
                found("Two", "https://two.example/"),
                found("Three", "https://three.example/"),
                found("Four", "https://four.example/"),
            ]),
        });
        assert!(of_class(&document, "hello").is_empty());
        assert!(of_class(&document, "blocks").is_empty());
        assert_eq!(of_class(&document, "you"), vec!["ev charger"]);
        assert_eq!(of_class(&document, "title"), vec!["One", "Two", "Three"]);
        assert_eq!(
            of_class(&document, "name"),
            vec!["One Guide", "two.example", "three.example"]
        );
        assert_eq!(
            of_class(&document, "addr").first().map(String::as_str),
            Some("one.example \u{203a} a \u{203a} b")
        );
        assert_eq!(of_class(&document, "more"), vec![super::MORE_SITES]);
        assert_eq!(of_class(&document, "rt on"), vec!["All"]);
        assert_eq!(of_class(&document, "rt").len(), 5);
        assert!(of_class(&document, "note").is_empty());
    }

    #[test]
    fn with_no_sites_a_sentence_stands_where_they_would() {
        let document = built(Results {
            said: "x".to_owned(),
            showing: Showing::Unanswered("Brave Search could not answer just now (500).".into()),
        });
        assert_eq!(
            of_class(&document, "note"),
            vec!["Brave Search could not answer just now (500)."]
        );
        assert!(of_class(&document, "found").is_empty());
        assert!(of_class(&document, "more").is_empty());
    }

    #[test]
    fn a_title_and_a_description_stay_text() {
        let hostile = "<b>Fish &amp; chips</b></h3><script>x()</script>";
        let mut site = found(hostile, "https://example.com/");
        site.description = hostile.to_owned();
        site.name = hostile.to_owned();
        let document = built(Results {
            said: hostile.to_owned(),
            showing: Showing::Found(vec![site]),
        });
        let elements = |name: &str| {
            document
                .descendants(document.root())
                .filter_map(|node| document.element(node))
                .filter(|element| element.name.local.as_ref() == name)
                .count()
        };
        assert_eq!(elements("script"), 0);
        assert_eq!(elements("b"), 0);
        let texts = document
            .descendants(document.root())
            .filter(|&node| matches!(document.kind(node), Some(NodeKind::Text(data)) if data == hostile))
            .count();
        assert_eq!(
            texts, 4,
            "said, name, title and description, each one text node"
        );
    }
}
