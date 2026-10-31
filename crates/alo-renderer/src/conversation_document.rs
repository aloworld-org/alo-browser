/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The conversation, built into the first screen's document from its turns
//! (ADR 0074, ADR 0085, queue item 703), the way the rest of the first
//! screen is built ([`crate::start_document`]): every element made with
//! `alo-dom`'s operations, every stranger's string a text node.
//!
//! # What changes on the screen
//!
//! With a turn, the screen is a conversation rather than a greeting: the
//! greeting, the line under it and the groups are not built, and above the
//! one box stands the conversation, clipped to the room above the box
//! (queue item 750). In it, **the newest turn** stands with its top at the
//! room's top, moved up by how far the browser process scrolled
//! ([`crate::start::Start::down`]), and **the earlier turns** stand above
//! it, oldest first, [`crate::conversation_scroll::BETWEEN_TURNS`] apart — so that where the newest
//! turn is, and the live card in it, never depends on how tall the turns
//! above it came out ([`crate::conversation_scroll`]).
//!
//! Each turn is what the person said, as their bubble at its right, then
//! what came back ([`crate::turn::Turn`]): a search's results
//! ([`crate::results_document`]), a live card ([`crate::card_document`]),
//! the address of a page shown earlier, or why what was chosen could not be
//! done.
//!
//! # What an agent reads
//!
//! ADR 0002: a `region` named [`CONVERSATION`] holding each turn, oldest
//! first, as a `region` named by what the person said — their words as a
//! paragraph, then the results' region, the card's region, a link named by
//! a page's address (*host* then *path*), or a paragraph saying why not.
//! Every turn is read wherever it is scrolled: what is clipped is still
//! the conversation, and an agent asks the tree, never the picture.
//!
//! # A page shown earlier is a link
//!
//! Of the class `title`, as a site's title is, so a press on it is claimed
//! by its place among the titles drawn ([`crate::conversation_claim`]) —
//! the browser process holds which page each place opens, from what it
//! sent ([`crate::turn::Turn::links`]) — and its `href` is a name of ours,
//! [`crate::results_document::TITLE_HREF`], never the page's address.

use crate::card::Card;
use crate::conversation_scroll::{EARLIER, NEWEST};
use crate::results_document::TITLE_HREF;
use crate::start::Start;
use crate::start_document::{Unbuilt, append, attribute, by_id, element, insert_before};
use crate::turn::Turn;
use alo_dom::{Document, NodeId};
use alo_url::Url;

/// What the conversation is named, to an agent.
pub const CONVERSATION: &str = "Conversation";

/// Put `start`'s turns into `document`, the first screen's, above `before`
/// — the one box — inside `screen`, its `main`, taking the screen's
/// greeting, its line and `blocks` out.
///
/// # Errors
///
/// [`Unbuilt`] if the template lacks a place it names, or `alo-dom`
/// refused an operation, which no turn can cause.
pub fn put(
    document: &mut Document,
    start: &Start,
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
    let thread = element(document, "section")?;
    attribute(document, thread, "class", "thread")?;
    attribute(document, thread, "aria-label", CONVERSATION)?;
    let turns = element(document, "div")?;
    attribute(document, turns, "class", "turns")?;
    if start.down != 0.0 && start.down.is_finite() {
        // A number of the browser process's, never a string of anybody's.
        attribute(document, turns, "style", &format!("top: {}px", -start.down))?;
    }
    if let Some((newest, earlier)) = start.turns.split_last() {
        if !earlier.is_empty() {
            let above = element(document, "div")?;
            attribute(document, above, "class", "earlier")?;
            attribute(document, above, "id", EARLIER)?;
            for (place, turn) in earlier.iter().enumerate() {
                let made = make_turn(document, turn, place, false)?;
                append(document, above, made)?;
            }
            append(document, turns, above)?;
        }
        let made = make_turn(document, newest, earlier.len(), true)?;
        attribute(document, made, "id", NEWEST)?;
        append(document, turns, made)?;
    }
    append(document, thread, turns)?;
    insert_before(document, screen, thread, before)
}

/// One turn, the `place`th of the conversation: a region named by what the
/// person said, holding their bubble and what came back. Only the `newest`
/// may hold a live card; a card anywhere else, which the wire never lets
/// through, is drawn as the address it was at.
fn make_turn(
    document: &mut Document,
    turn: &Turn,
    place: usize,
    newest: bool,
) -> Result<NodeId, Unbuilt> {
    let made = element(document, "section")?;
    let class = if matches!(turn, Turn::Searched(_)) {
        "turn searched"
    } else {
        "turn"
    };
    attribute(document, made, "class", class)?;
    let said_id = format!("said-{place}");
    attribute(document, made, "aria-labelledby", &said_id)?;
    let said = element(document, "p")?;
    attribute(document, said, "class", "you")?;
    attribute(document, said, "id", &said_id)?;
    // What the person said, a stranger's string, entering as a text node's
    // data.
    let words = document.create_text_node(turn.said());
    append(document, said, words)?;
    append(document, made, said)?;
    let reply = match turn {
        Turn::Searched(results) => crate::results_document::make(document, results)?,
        Turn::Page(card) if newest => crate::card_document::make(document, card)?,
        Turn::Page(Card { url, .. }) | Turn::Visited { url, .. } => make_visited(document, url)?,
        Turn::Failed { why, .. } => {
            let line = element(document, "p")?;
            attribute(document, line, "class", "failed")?;
            // Why, in this browser's words, naming an address: a text
            // node's data.
            let text = document.create_text_node(why);
            append(document, line, text)?;
            line
        }
    };
    append(document, made, reply)?;
    Ok(made)
}

/// A page shown earlier, as its address: a line holding a link, the host
/// in bold and then the rest, as the card's bar writes it.
fn make_visited(document: &mut Document, url: &Url) -> Result<NodeId, Unbuilt> {
    let visit = element(document, "p")?;
    attribute(document, visit, "class", "visit")?;
    let link = element(document, "a")?;
    attribute(document, link, "class", "title")?;
    attribute(document, link, "href", TITLE_HREF)?;
    let (host, rest) = Card::new("", url.clone()).address();
    for (class, words) in [("host", host), ("path", rest)] {
        if words.is_empty() {
            continue;
        }
        let part = element(document, "span")?;
        attribute(document, part, "class", class)?;
        // The page's address, entering as a text node's data.
        let text = document.create_text_node(&words);
        append(document, part, text)?;
        append(document, link, part)?;
    }
    append(document, visit, link)?;
    Ok(visit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::{Found, Results, Showing};
    use crate::start_document::build;
    use alo_dom::NodeKind;

    fn url(address: &str) -> Url {
        alo_url::parse(address).unwrap_or_else(|_| Url::about_blank())
    }

    fn built(turns: Vec<Turn>, down: f32) -> Document {
        build(&Start {
            greeting: "Hello.".to_owned(),
            turns,
            down,
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

    fn three() -> Vec<Turn> {
        vec![
            Turn::Searched(Results {
                said: "ev chargers".to_owned(),
                showing: Showing::Found(vec![Found::new(
                    "Chargers",
                    url("https://a.example/"),
                    "About them.",
                )]),
            }),
            Turn::Visited {
                said: "Chargers".to_owned(),
                url: url("https://a.example/b?c"),
            },
            Turn::Failed {
                said: "b.example".to_owned(),
                why: "https://b.example/ could not be opened: no Chrome".to_owned(),
            },
        ]
    }

    #[test]
    fn every_turn_is_kept_oldest_first_the_newest_under_the_earlier_ones() {
        let document = built(three(), 0.0);
        assert!(of_class(&document, "hello").is_empty());
        assert_eq!(
            of_class(&document, "you"),
            vec!["ev chargers", "Chargers", "b.example"]
        );
        let earlier = by_id(&document, EARLIER).ok();
        let newest = by_id(&document, NEWEST).ok();
        assert!(earlier.is_some() && newest.is_some());
        assert_eq!(
            earlier.map(|earlier| document.children(earlier).count()),
            Some(2)
        );
        assert_eq!(
            earlier.and_then(|earlier| document.next_sibling(earlier)),
            newest
        );
        assert_eq!(
            of_class(&document, "failed"),
            vec!["https://b.example/ could not be opened: no Chrome"]
        );
        assert_eq!(of_class(&document, "visit"), vec!["a.example/b?c"]);
        // The site's title and the page shown earlier, in the order drawn.
        assert_eq!(
            of_class(&document, "title"),
            vec!["Chargers", "a.example/b?c"]
        );
        // Each turn named by what was said.
        let turn = newest.and_then(|newest| document.attribute_by_name(newest, "aria-labelledby"));
        assert_eq!(turn, Some("said-2"));
        // The thread stands above the box.
        let thread = document
            .descendants(document.root())
            .find(|&node| document.attribute_by_name(node, "class") == Some("thread"));
        assert_eq!(
            thread
                .and_then(|thread| document.next_sibling(thread))
                .and_then(|next| document.attribute_by_name(next, "class")),
            Some("box")
        );
    }

    #[test]
    fn one_turn_has_no_earlier_ones_and_scrolled_its_turns_are_moved_up() {
        let one = vec![Turn::Page(Card::new(
            "example.com",
            url("https://example.com/"),
        ))];
        let document = built(one.clone(), 0.0);
        assert!(by_id(&document, EARLIER).is_err());
        assert_eq!(of_class(&document, "ibtn close").len(), 1, "a live card");
        let turns = |document: &Document| {
            document
                .descendants(document.root())
                .find(|&node| document.attribute_by_name(node, "class") == Some("turns"))
                .and_then(|turns| document.attribute_by_name(turns, "style"))
                .map(str::to_owned)
        };
        assert_eq!(turns(&document), None, "not scrolled, no style");
        assert_eq!(
            turns(&built(one.clone(), 120.5)),
            Some("top: -120.5px".to_owned())
        );
        assert_eq!(turns(&built(one, -300.0)), Some("top: 300px".to_owned()));
    }

    #[test]
    fn a_card_is_live_only_as_the_newest_turn() {
        let card = Turn::Page(Card::new("one", url("https://one.example/")));
        let document = built(vec![card, Turn::Searched(Results::searching("two"))], 0.0);
        assert!(of_class(&document, "ibtn close").is_empty());
        assert_eq!(of_class(&document, "visit"), vec!["one.example/"]);
    }

    #[test]
    fn every_turns_words_stay_text() {
        let hostile = "</p><script>x()</script><b>b</b>";
        let document = built(
            vec![
                Turn::Searched(Results::searching(hostile)),
                Turn::Visited {
                    said: hostile.to_owned(),
                    url: url("https://a.example/%3Cscript%3E"),
                },
                Turn::Failed {
                    said: hostile.to_owned(),
                    why: hostile.to_owned(),
                },
                Turn::Page(Card::new(hostile, url("https://b.example/"))),
            ],
            0.0,
        );
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
        // Four bubbles and one failure.
        assert_eq!(texts, 5);
    }
}
