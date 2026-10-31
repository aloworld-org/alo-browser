/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search's videos, built into the first screen's document as the Videos
//! tab (queue item 745, ADR 0087) — the owner's canvas, `Videos.dc.html`'s
//! `.vr` — and the pictures they name.
//!
//! Each video is a row: its picture, 200 × 112, rounded 10, on `#2A3446`,
//! with its length over the picture's bottom right corner; then, 16 beside
//! it, its title as a link, its site's mark and name and how old it is, and
//! two lines of what the provider says of it.
//!
//! # The picture
//!
//! As an image's ([`crate::images_document`]): an `<img>` whose `src` is a
//! name of ours, [`src`], and whose bytes are handed to the drawing beside
//! the document ([`pictures`]), decoded in this confined process by the
//! picture readers every `<img>` goes through. The tile is a grid of one
//! cell holding the picture and the length both, so the length stands over
//! the picture with nothing positioned (a press on a positioned box in the
//! conversation finds the box behind it, queue item 763). With no picture
//! the tile is its colour.
//!
//! # A press
//!
//! A video's title is of the class `title`, as a site's title is, and is
//! claimed by its place among every title drawn
//! ([`crate::conversation_claim`]); the browser process holds the page of
//! each, and opens the one pressed in the card. Its `href` is a name of
//! ours, [`crate::results_document::TITLE_HREF`].
//!
//! # What an agent reads
//!
//! ADR 0002: each video an `article` holding its length as text, its title
//! as a heading holding a link, its site and age as text and what is said
//! of it as a paragraph. The picture and the site's letter are decoration,
//! hidden from the tree. Every string is a stranger's and enters as a text
//! node.

use crate::marks_document::Beside;
use crate::resource::Resource;
use crate::results::Results;
use crate::results_document::TITLE_HREF;
use crate::results_media::Video;
use crate::start::Start;
use crate::start_document::{Unbuilt, append, attribute, element};
use crate::turn::Turn;
use alo_dom::{Document, NodeId};

/// The name the picture of the `video`th video of the `turn`th turn is
/// drawn by.
pub fn src(turn: usize, video: usize) -> String {
    format!("alo:video-{turn}-{video}")
}

/// Every video picture `start`'s turns draw, by the name each `<img>` gives
/// it: what the drawing is handed beside the document.
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
                .drawn_videos()
                .iter()
                .enumerate()
                .filter_map(move |(video, drawn)| {
                    drawn
                        .picture
                        .as_ref()
                        .map(|bytes| Resource::new(src(turn, video), None, bytes.clone()))
                })
        })
        .collect()
}

/// The Videos tab, for the `place`th turn, under `made`'s tab row: each
/// video its row, or one sentence where the videos would be.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no videos can
/// cause.
pub fn list(
    document: &mut Document,
    made: NodeId,
    results: &Results,
    place: usize,
) -> Result<(), Unbuilt> {
    if let Some(note) = results.videos_note() {
        let said = element(document, "p")?;
        attribute(document, said, "class", "note")?;
        // The provider's refusal, or what was typed, as a text node's data.
        let text = document.create_text_node(&note);
        append(document, said, text)?;
        return append(document, made, said);
    }
    let list = element(document, "div")?;
    attribute(document, list, "class", "vids")?;
    for (at, video) in results.drawn_videos().iter().enumerate() {
        let row = row(document, video, place, at)?;
        append(document, list, row)?;
    }
    append(document, made, list)
}

/// One video: its picture and length, then its title, site and age, and
/// what is said of it.
fn row(document: &mut Document, video: &Video, place: usize, at: usize) -> Result<NodeId, Unbuilt> {
    let article = element(document, "article")?;
    attribute(document, article, "class", "vr")?;
    let tile = element(document, "div")?;
    attribute(document, tile, "class", "vpic")?;
    if video.picture.is_some() {
        let picture = element(document, "img")?;
        attribute(document, picture, "class", "vimg")?;
        attribute(document, picture, "src", &src(place, at))?;
        attribute(document, picture, "alt", "")?;
        attribute(document, picture, "aria-hidden", "true")?;
        append(document, tile, picture)?;
    }
    if !video.length.is_empty() {
        let length = element(document, "span")?;
        attribute(document, length, "class", "dur")?;
        // A length the provider wrote, checked to be digits and colons.
        let words = document.create_text_node(&video.length);
        append(document, length, words)?;
        append(document, tile, length)?;
    }
    append(document, article, tile)?;
    let body = element(document, "div")?;
    attribute(document, body, "class", "vb")?;
    let heading = element(document, "h3")?;
    attribute(document, heading, "class", "nt")?;
    let link = element(document, "a")?;
    attribute(document, link, "class", "title")?;
    attribute(document, link, "href", TITLE_HREF)?;
    // The video's title, a stranger's string, as a text node's data.
    let headline = document.create_text_node(&video.title);
    append(document, link, headline)?;
    append(document, heading, link)?;
    append(document, body, heading)?;
    let meta = element(document, "p")?;
    attribute(document, meta, "class", "m")?;
    crate::marks_document::mark(
        document,
        meta,
        video
            .mark
            .as_deref()
            .map(|bytes| (bytes, crate::marks_document::src(place, Beside::Video, at))),
        video.letter(),
        video.tint(),
    )?;
    let site = element(document, "span")?;
    attribute(document, site, "class", "mn")?;
    let said = if video.age.trim().is_empty() {
        video.site()
    } else {
        format!("{} \u{b7} {}", video.site(), video.age.trim())
    };
    let words = document.create_text_node(&said);
    append(document, site, words)?;
    append(document, meta, site)?;
    append(document, body, meta)?;
    if !video.description.is_empty() {
        let description = element(document, "p")?;
        attribute(document, description, "class", "vd")?;
        let text = document.create_text_node(&video.description);
        append(document, description, text)?;
        append(document, body, description)?;
    }
    append(document, article, body)?;
    Ok(article)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::{Found, Showing, Tab, Videos};
    use crate::start_document::build;

    fn url(address: &str) -> alo_url::Url {
        alo_url::parse(address).unwrap_or_else(|_| alo_url::Url::about_blank())
    }

    fn start(results: Results) -> Start {
        Start {
            greeting: "Hello.".to_owned(),
            turns: vec![
                Turn::Failed {
                    said: "a".to_owned(),
                    why: "b".to_owned(),
                },
                Turn::Searched(results),
            ],
            ..Start::default()
        }
    }

    fn of_class(document: &Document, class: &str) -> Vec<String> {
        document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some(class))
            .map(|node| document.text_content(node))
            .collect()
    }

    fn results(videos: Videos) -> Results {
        Results::of(
            "ev",
            Showing::Found(vec![Found::new("A", url("https://a.example/"), "")]),
        )
        .with_videos(videos)
        .on(Tab::Videos)
    }

    #[test]
    fn each_video_is_its_picture_length_title_site_and_age() {
        let videos = vec![
            Video::new("Fitting a charger", url("https://v.example/watch?v=1"))
                .lasting("4:05")
                .from_site("YouTube", "2 days ago")
                .said_of("Start to finish.")
                .holding(vec![0x89, b'P', b'N', b'G']),
            Video::new("No picture", url("https://w.example/2")),
        ];
        let start = start(results(Videos::Found(videos)));
        let document = build(&start).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        assert_eq!(of_class(&document, "vr").len(), 2);
        assert_eq!(
            of_class(&document, "title"),
            vec!["Fitting a charger", "No picture"]
        );
        assert_eq!(of_class(&document, "dur"), vec!["4:05"]);
        assert_eq!(
            of_class(&document, "mn"),
            vec!["YouTube \u{b7} 2 days ago", "w.example"]
        );
        assert_eq!(of_class(&document, "vd"), vec!["Start to finish."]);
        let named: Vec<String> = pictures(&start).into_iter().map(|each| each.src).collect();
        assert_eq!(named, vec!["alo:video-1-0"]);
        assert!(
            of_class(&document, "found").is_empty(),
            "no sites on Videos"
        );
        assert_eq!(of_class(&document, "rt on"), vec!["Videos"]);
    }

    #[test]
    fn with_no_videos_the_tab_says_why_and_nothing_is_pictured() {
        let start = start(results(Videos::Asking));
        let document = build(&start).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        assert_eq!(
            of_class(&document, "note"),
            vec!["Searching Brave Search for videos of \u{201c}ev\u{201d}\u{2026}"]
        );
        assert!(pictures(&start).is_empty());
        let on_all = Start {
            turns: vec![Turn::Searched(
                results(Videos::Found(vec![
                    Video::new("V", url("https://v.example/")).holding(vec![0x89, b'P']),
                ]))
                .on(Tab::All),
            )],
            ..Start::default()
        };
        assert!(pictures(&on_all).is_empty(), "drawn only on Videos");
    }

    #[test]
    fn a_title_site_and_description_holding_markup_stay_their_characters() {
        let hostile = "<img src=x onerror=y()></a><script>z()</script>";
        let video = Video::new(hostile, url("https://v.example/"))
            .from_site(hostile, hostile)
            .said_of(hostile);
        let document = build(&start(results(Videos::Found(vec![video]))))
            .unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        let elements = |name: &str| {
            document
                .descendants(document.root())
                .filter_map(|node| document.element(node))
                .filter(|element| element.name.local.as_ref() == name)
                .count()
        };
        assert_eq!(elements("img") + elements("script"), 0);
        assert_eq!(of_class(&document, "title"), vec![hostile]);
    }
}
