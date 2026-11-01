/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search's news, built into the first screen's document as the News tab
//! (queue item 746, ADR 0087) — the owner's canvas, `News.dc.html`'s `.n`.
//!
//! Each story is a row, 16 above and below and a line of `#E6E0D6` under
//! it (the first 4 above): its site's mark and name and how old it is, on a
//! line of 13; its headline as a link, 17 on 24, 600; and two lines of what
//! the provider says of it — and, 20 beside all that, its photo.
//!
//! # The photo (queue item 765)
//!
//! A tile 132 × 88, rounded 10, on `#E9E3D8` — the canvas's `.th`. As a
//! video's picture ([`crate::videos_document`]): an `<img>` whose `src` is
//! a name of ours, [`src`], and whose bytes are handed to the drawing beside
//! the document ([`pictures`]), decoded in this confined process by the
//! picture readers every `<img>` goes through; the tile is one grid cell,
//! the photo 132 wide and centred in it, its overflow clipped by the tile's
//! rounded corners. **With no photo kept** the tile holds the site's letter
//! where the canvas holds its placeholder's words (11.5, `#3D3A35`,
//! centred); with no letter either, it is its colour alone. A photo kept
//! that does not decode leaves the tile its colour, as a video's does.
//!
//! # A press
//!
//! A story's headline is of the class `title`, as a site's title is, and is
//! claimed by its place among every title drawn
//! ([`crate::conversation_claim`]); the browser process holds the page of
//! each, and opens the one pressed in the card. Its `href` is a name of
//! ours, [`crate::results_document::TITLE_HREF`].
//!
//! # What an agent reads
//!
//! ADR 0002: each story an `article` holding its site and age as text, its
//! headline as a heading holding a link, and what is said of it as a
//! paragraph. The site's letter is decoration, hidden from the tree. Every
//! string is a stranger's and enters as a text node.

use crate::marks_document::Beside;
use crate::resource::Resource;
use crate::results::Results;
use crate::results_document::TITLE_HREF;
use crate::results_media::Story;
use crate::start::Start;
use crate::start_document::{Unbuilt, append, attribute, element};
use crate::turn::Turn;
use alo_dom::{Document, NodeId};

/// The name the photo of the `story`th story of the `turn`th turn is drawn
/// by.
pub fn src(turn: usize, story: usize) -> String {
    format!("alo:story-{turn}-{story}")
}

/// Every story's photo `start`'s turns draw, by the name each `<img>`
/// gives it: what the drawing is handed beside the document.
pub fn pictures(start: &Start) -> Vec<Resource> {
    start
        .turns
        .iter()
        .enumerate()
        .filter_map(|(turn, each)| match each {
            Turn::Searched(results) => Some((turn, results)),
            _ => None,
        })
        .flat_map(|(turn, results)| {
            results
                .drawn_stories()
                .iter()
                .enumerate()
                .filter_map(move |(story, drawn)| {
                    drawn
                        .picture
                        .as_ref()
                        .map(|bytes| Resource::new(src(turn, story), None, bytes.clone()))
                })
        })
        .collect()
}

/// The News tab, for the `place`th turn, under `made`'s tab row: each
/// story its row, or one sentence where the stories would be.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no stories can
/// cause.
pub fn list(
    document: &mut Document,
    made: NodeId,
    results: &Results,
    place: usize,
) -> Result<(), Unbuilt> {
    if let Some(note) = results.news_note() {
        let said = element(document, "p")?;
        attribute(document, said, "class", "note")?;
        // The provider's refusal, or what was typed, as a text node's data.
        let text = document.create_text_node(&note);
        append(document, said, text)?;
        return append(document, made, said);
    }
    let list = element(document, "div")?;
    attribute(document, list, "class", "news")?;
    for (at, story) in results.drawn_stories().iter().enumerate() {
        let row = row(document, story, place, at)?;
        append(document, list, row)?;
    }
    append(document, made, list)
}

/// One story, the `at`th of the `place`th turn: its site and age, its
/// headline, and what is said of it, then its photo.
fn row(document: &mut Document, story: &Story, place: usize, at: usize) -> Result<NodeId, Unbuilt> {
    let article = element(document, "article")?;
    attribute(document, article, "class", "n")?;
    let body = element(document, "div")?;
    attribute(document, body, "class", "nb")?;
    told(document, body, story, place, at)?;
    append(document, article, body)?;
    let tile = photo(document, story, place, at)?;
    append(document, article, tile)?;
    Ok(article)
}

/// The tile beside the `at`th story of the `place`th turn: its photo, or
/// its site's letter, or nothing on the tile's colour.
fn photo(
    document: &mut Document,
    story: &Story,
    place: usize,
    at: usize,
) -> Result<NodeId, Unbuilt> {
    let tile = element(document, "div")?;
    attribute(document, tile, "class", "th")?;
    attribute(document, tile, "aria-hidden", "true")?;
    if story.picture.is_some() {
        let picture = element(document, "img")?;
        attribute(document, picture, "class", "thimg")?;
        attribute(document, picture, "src", &src(place, at))?;
        attribute(document, picture, "alt", "")?;
        append(document, tile, picture)?;
    } else if let Some(letter) = story.letter() {
        let words = element(document, "span")?;
        attribute(document, words, "class", "thl")?;
        // One letter of the site's name: a stranger's character, as text.
        let text = document.create_text_node(&letter);
        append(document, words, text)?;
        append(document, tile, words)?;
    }
    Ok(tile)
}

/// What is told of the `at`th story of the `place`th turn, put in `into`:
/// its site and age, its headline, and what is said of it.
fn told(
    document: &mut Document,
    into: NodeId,
    story: &Story,
    place: usize,
    at: usize,
) -> Result<(), Unbuilt> {
    let article = into;
    let source = element(document, "div")?;
    attribute(document, source, "class", "src")?;
    crate::marks_document::mark(
        document,
        source,
        story
            .mark
            .as_deref()
            .map(|bytes| (bytes, crate::marks_document::src(place, Beside::Story, at))),
        story.letter(),
        story.tint(),
    )?;
    let name = element(document, "span")?;
    attribute(document, name, "class", "name")?;
    // The provider's name for the site, a stranger's string, as text.
    let words = document.create_text_node(&story.site());
    append(document, name, words)?;
    append(document, source, name)?;
    if !story.age.trim().is_empty() {
        let age = element(document, "span")?;
        attribute(document, age, "class", "age")?;
        let words = document.create_text_node(&format!("\u{b7} {}", story.age.trim()));
        append(document, age, words)?;
        append(document, source, age)?;
    }
    append(document, article, source)?;
    let heading = element(document, "h3")?;
    attribute(document, heading, "class", "nt")?;
    let link = element(document, "a")?;
    attribute(document, link, "class", "title")?;
    attribute(document, link, "href", TITLE_HREF)?;
    let title = document.create_text_node(&story.title);
    append(document, link, title)?;
    append(document, heading, link)?;
    append(document, article, heading)?;
    if !story.description.is_empty() {
        let description = element(document, "p")?;
        attribute(document, description, "class", "vd")?;
        let text = document.create_text_node(&story.description);
        append(document, description, text)?;
        append(document, article, description)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::{Found, Showing, Stories, Tab};
    use crate::start::Start;
    use crate::start_document::build;
    use crate::turn::Turn;

    fn url(address: &str) -> alo_url::Url {
        alo_url::parse(address).unwrap_or_else(|_| alo_url::Url::about_blank())
    }

    fn built(news: Stories) -> Document {
        let results = Results::of(
            "ev",
            Showing::Found(vec![Found::new("A", url("https://a.example/"), "")]),
        )
        .with_news(news)
        .on(Tab::News);
        build(&Start {
            greeting: "Hello.".to_owned(),
            turns: vec![Turn::Searched(results)],
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
    fn each_story_is_its_site_age_and_headline() {
        let document = built(Stories::Found(vec![
            Story::new("More electricians trained", url("https://c.example/a"))
                .from_site("Consumer energy report", "2 days ago")
                .said_of("Waiting times are shorter."),
            Story::new("No site named", url("https://daily.example/b")),
        ]));
        assert_eq!(of_class(&document, "n").len(), 2);
        assert_eq!(
            of_class(&document, "name"),
            vec!["Consumer energy report", "daily.example"]
        );
        assert_eq!(of_class(&document, "age"), vec!["\u{b7} 2 days ago"]);
        assert_eq!(
            of_class(&document, "title"),
            vec!["More electricians trained", "No site named"]
        );
        assert_eq!(
            of_class(&document, "vd"),
            vec!["Waiting times are shorter."]
        );
        assert_eq!(of_class(&document, "rt on"), vec!["News"]);
    }

    #[test]
    fn a_story_shows_its_photo_or_its_sites_letter_or_its_tile_alone() {
        let start = |news: Stories, tab: Tab| Start {
            greeting: "Hello.".to_owned(),
            turns: vec![Turn::Searched(
                Results::of(
                    "ev",
                    Showing::Found(vec![Found::new("A", url("https://a.example/"), "")]),
                )
                .with_news(news)
                .on(tab),
            )],
            ..Start::default()
        };
        let stories = || {
            Stories::Found(vec![
                Story::new("Pictured", url("https://c.example/a"))
                    .from_site("Consumer energy report", "")
                    .holding(vec![0x89, b'P', b'N', b'G']),
                Story::new("Lettered", url("https://daily.example/b")),
                Story::new("Bare", url("https://c.example/c")).from_site("\u{2014}", ""),
                Story::new("Pictured too", url("https://e.example/d")).holding(vec![0xff, 0xd8]),
            ])
        };
        let shown = start(stories(), Tab::News);
        let document = build(&shown).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        assert_eq!(of_class(&document, "th").len(), 4, "a tile beside each");
        let photos: Vec<Option<&str>> = document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some("thimg"))
            .map(|node| document.attribute_by_name(node, "src"))
            .collect();
        assert_eq!(photos, vec![Some("alo:story-0-0"), Some("alo:story-0-3")]);
        // The second's site is its host, `daily.example`; the third's name
        // has no letter in it, so its tile is its colour alone.
        assert_eq!(of_class(&document, "thl"), vec!["D"]);
        let named: Vec<String> = pictures(&shown).into_iter().map(|each| each.src).collect();
        assert_eq!(named, vec!["alo:story-0-0", "alo:story-0-3"]);
        assert!(
            pictures(&start(stories(), Tab::All)).is_empty(),
            "drawn only on News"
        );
    }

    #[test]
    fn with_no_stories_the_tab_says_why() {
        let document = built(Stories::Unanswered(
            "Brave Search could not answer just now (500).".to_owned(),
        ));
        assert_eq!(
            of_class(&document, "note"),
            vec!["Brave Search could not answer just now (500)."]
        );
        assert!(of_class(&document, "n").is_empty());
    }

    #[test]
    fn a_headline_holding_markup_stays_its_characters() {
        let hostile = "<b>Fish &amp; chips</b></h3><script>x()</script>";
        let document = built(Stories::Found(vec![
            Story::new(hostile, url("https://a.example/"))
                .from_site(hostile, hostile)
                .said_of(hostile),
        ]));
        let elements = document
            .descendants(document.root())
            .filter_map(|node| document.element(node))
            .filter(|element| matches!(element.name.local.as_ref(), "b" | "script"))
            .count();
        assert_eq!(elements, 0);
        assert_eq!(of_class(&document, "title"), vec![hostile]);
    }
}
