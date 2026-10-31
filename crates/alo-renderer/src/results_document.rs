/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search's results, built into the first screen's document on the tab
//! the browser process chose — the design's **All**, **Sites**, **Images**,
//! **Videos** or **News** (queue items 708, 743–746; ADR 0083, ADR 0086,
//! ADR 0087) — in a turn
//! of the conversation ([`crate::conversation_document`]): the tab row is
//! ours and fixed, made with `alo-dom`'s operations, and every site's name,
//! address, title and description enters it as a text node. The images —
//! All's row and the Images tab's grid — are built by
//! [`crate::images_document`]; the videos by [`crate::videos_document`] and
//! the news by [`crate::news_document`].
//!
//! # What an agent reads
//!
//! ADR 0002: a `region` named *Search results*, holding a `tablist` named
//! *Kinds of result* — the tab shown selected, All always open, Sites,
//! Videos and News once sites were found, Images once images were asked
//! for, and Answer disabled — then, on All, each of the top sites as an `article`:
//! its name and address as text, its title as a heading holding a link, and
//! its description as a paragraph; then the row of images; then *More sites
//! ›*, a button. On Sites every site the same way; on Images the grid. Or,
//! where they would be, one sentence: searching, nothing found, or why the
//! search was not answered. A site's mark — its icon, or its letter where
//! it has none ([`crate::marks_document`], queue item 747) — is decoration,
//! hidden from the tree.
//!
//! # A title is a link to its page
//!
//! A press on a site's title is claimed by its place among the titles
//! ([`crate::conversation_claim`]), and the browser process opens the page
//! it holds for that place in the card, as an address chosen from the box
//! opens. The link's `href` is a name of ours, [`TITLE_HREF`], never the
//! site's address: nothing a stranger wrote is an attribute here.
//!
//! # A tab is claimed by its place among the open ones
//!
//! An open tab is of the class `rt` (`rt on` when shown), a disabled one
//! `rt off`, so a press is claimed by the place of an open tab among every
//! open tab drawn, and the browser process holds which search and which
//! tab each place was. *More sites ›* is claimed by its place among the
//! *More sites ›* drawn, and opens that search's Sites tab.

use crate::results::{Found, Results, TABS, Tab};
use crate::start_document::{Unbuilt, append, attribute, element};
use alo_dom::{Document, NodeId};

/// What a site's title links to in this document: a name of ours.
pub const TITLE_HREF: &str = "#found";

/// What *More sites ›* says.
pub const MORE_SITES: &str = "More sites \u{203a}";

/// What the results are named, to an agent.
pub const REGION: &str = "Search results";

/// What the tab row is named, to an agent.
pub const KINDS: &str = "Kinds of result";

/// The results for `results`, the `place`th turn of the conversation, on
/// the tab they are shown on, for the conversation to put in its turn
/// ([`crate::conversation_document`]).
///
/// Made with `alo-dom`'s operations rather than moved out of a template,
/// since a conversation holds as many as it holds searches and a
/// template's contents can be moved out once: the region named
/// [`REGION`], its tab row — [`TABS`] in order — then what the tab shows.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no results can
/// cause.
pub fn make(document: &mut Document, results: &Results, place: usize) -> Result<NodeId, Unbuilt> {
    let made = element(document, "section")?;
    attribute(document, made, "class", "results")?;
    attribute(document, made, "aria-label", REGION)?;
    let row = element(document, "div")?;
    attribute(document, row, "class", "rtabs")?;
    attribute(document, row, "role", "tablist")?;
    attribute(document, row, "aria-label", KINDS)?;
    let opened = results.open_tabs();
    for (at, name) in TABS.into_iter().enumerate() {
        let built = Tab::at(at);
        let chosen = built == Some(results.tab);
        let open = built.is_some_and(|tab| opened.contains(&tab));
        let tab = element(document, "button")?;
        let class = match (chosen, open) {
            (true, _) => "rt on",
            (false, true) => "rt",
            (false, false) => "rt off",
        };
        attribute(document, tab, "class", class)?;
        attribute(document, tab, "type", "button")?;
        attribute(document, tab, "role", "tab")?;
        attribute(
            document,
            tab,
            "aria-selected",
            if chosen { "true" } else { "false" },
        )?;
        if !open && !chosen {
            attribute(document, tab, "disabled", "")?;
        }
        let words = document.create_text_node(name);
        append(document, tab, words)?;
        append(document, row, tab)?;
    }
    append(document, made, row)?;
    match results.tab {
        Tab::All | Tab::Sites => sites(document, made, results, place)?,
        Tab::Images => crate::images_document::grid(document, made, results, place)?,
        Tab::Videos => crate::videos_document::list(document, made, results, place)?,
        Tab::News => crate::news_document::list(document, made, results, place)?,
    }
    Ok(made)
}

/// The sites the tab shows, or the sentence said in their place; on All,
/// then the row of images and *More sites ›*.
fn sites(
    document: &mut Document,
    made: NodeId,
    results: &Results,
    place: usize,
) -> Result<(), Unbuilt> {
    let list = element(document, "div")?;
    attribute(document, list, "class", "res")?;
    append(document, made, list)?;
    if let Some(note) = results.note() {
        let said = element(document, "p")?;
        attribute(document, said, "class", "note")?;
        // The provider's refusal, or what was typed, as a text node's data.
        let text = document.create_text_node(&note);
        append(document, said, text)?;
        append(document, list, said)?;
        return Ok(());
    }
    for (at, found) in results.drawn_sites().iter().enumerate() {
        let site = make_site(document, found, place, at)?;
        append(document, list, site)?;
    }
    if !results.draws_more_sites() {
        return Ok(());
    }
    crate::images_document::row(document, made, results, place)?;
    let more = element(document, "button")?;
    attribute(document, more, "class", "more")?;
    attribute(document, more, "type", "button")?;
    let words = document.create_text_node(MORE_SITES);
    append(document, more, words)?;
    append(document, made, more)?;
    Ok(())
}

/// One site, the `at`th of the `place`th turn: its mark, name and address
/// on a line; its title as a link; its description.
fn make_site(
    document: &mut Document,
    found: &Found,
    place: usize,
    at: usize,
) -> Result<NodeId, Unbuilt> {
    let article = element(document, "article")?;
    attribute(document, article, "class", "found")?;
    let source = element(document, "div")?;
    attribute(document, source, "class", "src")?;
    crate::marks_document::mark(
        document,
        source,
        found.mark.as_deref().map(|bytes| {
            (
                bytes,
                crate::marks_document::src(place, crate::marks_document::Beside::Site, at),
            )
        }),
        found.letter(),
        found.tint(),
    )?;
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
            turns: vec![crate::turn::Turn::Searched(results)],
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
        let document = built(Results::of(
            "ev charger".to_owned(),
            Showing::Found(vec![
                found("One", "https://one.example/a/b").named("One Guide"),
                found("Two", "https://two.example/"),
                found("Three", "https://three.example/"),
                found("Four", "https://four.example/"),
            ]),
        ));
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
        assert_eq!(
            of_class(&document, "rt"),
            vec!["Sites", "Videos", "News"],
            "Sites, Videos and News are open"
        );
        assert_eq!(
            of_class(&document, "rt off"),
            vec!["Answer", "Images"],
            "no images were asked for, and Answer is not built"
        );
        assert!(of_class(&document, "note").is_empty());
    }

    #[test]
    fn with_no_sites_a_sentence_stands_where_they_would() {
        let document = built(Results::of(
            "x".to_owned(),
            Showing::Unanswered("Brave Search could not answer just now (500).".into()),
        ));
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
        let document = built(Results::of(hostile.to_owned(), Showing::Found(vec![site])));
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
