/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search's images, built into the first screen's document (queue item
//! 743, ADR 0086): the All tab's row of five under the sites, with *See all
//! images*, and the Images tab's grid — the owner's canvas, `Links.dc.html`'s
//! `.imgs` and `Images.dc.html`'s `.grid` — and the pictures they name.
//!
//! # The pictures
//!
//! Each image with a thumbnail is an `<img>` whose `src` is a name of ours,
//! [`src`] — the turn's place and the image's — never the thumbnail's
//! address, and the thumbnail's bytes are handed to the drawing as that
//! picture ([`pictures`]), as the card's frame is: decoded in this confined
//! process by the picture readers every `<img>` goes through. A thumbnail
//! that is not there — never sent, left out by the browser process, or past
//! the first screen's budget — leaves the tile its colour with the image's
//! title in it; one that does not decode is drawn as the tile's colour.
//!
//! # A picture is not stretched
//!
//! The engine does not read `object-fit` (it is not one of alo's
//! properties yet). So a picture is laid out the full width of its tile at
//! its own proportions, centred up and down, and the tile clips what is
//! taller: a photograph wider than tall fills a tile of the row; a picture
//! wider than the tile's own proportions leaves the tile's colour above and
//! below it. Neither is ever drawn out of its proportions.
//!
//! # What an agent reads
//!
//! ADR 0002: the row as a `region` named *Images*, its *See all images* a
//! button, and each image a link named by its title — on the grid, its
//! title and its site. A picture is decoration, hidden from the tree: what
//! it shows is the title's to say, never read off its pixels. Every string
//! is a stranger's and enters as a text node.
//!
//! # A press
//!
//! An image, in the row or the grid, is of the class `im`, `im held` or
//! `pic`, and claimed by its place among every image drawn
//! ([`crate::conversation_claim`]); its `href` is [`IMAGE_HREF`], a name of
//! ours. *See all images* is of the class `seeall`, claimed by its place
//! among those drawn, and opens that search's Images tab.

use crate::resource::Resource;
use crate::results::{Pictured, Results};
use crate::start::Start;
use crate::start_document::{Unbuilt, append, attribute, element};
use crate::turn::Turn;
use alo_dom::{Document, NodeId};

/// What an image links to in this document: a name of ours.
pub const IMAGE_HREF: &str = "#image";

/// What the row of images is named, to an agent, and says over itself.
pub const IMAGES: &str = "Images";

/// What *See all images* says.
pub const SEE_ALL: &str = "See all images";

/// The name the picture of the `image`th image of the `turn`th turn is
/// drawn by.
pub fn src(turn: usize, image: usize) -> String {
    format!("alo:thumbnail-{turn}-{image}")
}

/// Every picture `start`'s turns draw, by the name each `<img>` gives it:
/// what the drawing is handed beside the document.
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
                .drawn_images()
                .iter()
                .enumerate()
                .filter_map(move |(image, pictured)| {
                    pictured
                        .picture
                        .as_ref()
                        .map(|bytes| Resource::new(src(turn, image), None, bytes.clone()))
                })
        })
        .collect()
}

/// All's row of images, for the `place`th turn, under `made`'s sites: its
/// heading and *See all images*, then a tile for each image All draws.
/// Nothing while there are none — still being asked for, none found, or
/// not answered: the Images tab says which.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no images can
/// cause.
pub fn row(
    document: &mut Document,
    made: NodeId,
    results: &Results,
    place: usize,
) -> Result<(), Unbuilt> {
    if !results.draws_see_all() {
        return Ok(());
    }
    let drawn = results.drawn_images();
    let section = element(document, "section")?;
    attribute(document, section, "class", "imrow")?;
    attribute(document, section, "aria-label", IMAGES)?;
    let heading = element(document, "div")?;
    attribute(document, heading, "class", "imh")?;
    let named = element(document, "span")?;
    attribute(document, named, "class", "imn")?;
    let words = document.create_text_node(IMAGES);
    append(document, named, words)?;
    append(document, heading, named)?;
    let all = element(document, "button")?;
    attribute(document, all, "class", "seeall")?;
    attribute(document, all, "type", "button")?;
    let words = document.create_text_node(SEE_ALL);
    append(document, all, words)?;
    append(document, heading, all)?;
    append(document, section, heading)?;
    let tiles = element(document, "div")?;
    attribute(document, tiles, "class", "imgs")?;
    for (image, pictured) in drawn.iter().enumerate() {
        let tile = element(document, "a")?;
        let held = pictured.picture.is_some();
        attribute(document, tile, "class", if held { "im held" } else { "im" })?;
        attribute(document, tile, "href", IMAGE_HREF)?;
        // The image's named, a stranger's string: the tile's words when
        // there is no picture, and its name to an agent when there is.
        let named = element(document, "span")?;
        attribute(
            document,
            named,
            "class",
            if held { "imt sr" } else { "imt" },
        )?;
        let words = document.create_text_node(&pictured.title);
        append(document, named, words)?;
        append(document, tile, named)?;
        if held {
            let picture = thumbnail(document, place, image)?;
            append(document, tile, picture)?;
        }
        append(document, tiles, tile)?;
    }
    append(document, section, tiles)?;
    append(document, made, section)
}

/// The Images tab's grid, for the `place`th turn, under `made`'s tab row:
/// each image its tile, then its title and site; or one sentence where the
/// images would be.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no images can
/// cause.
pub fn grid(
    document: &mut Document,
    made: NodeId,
    results: &Results,
    place: usize,
) -> Result<(), Unbuilt> {
    if let Some(note) = results.images_note() {
        let said = element(document, "p")?;
        attribute(document, said, "class", "note")?;
        // The provider's refusal, or what was typed, as a text node's data.
        let text = document.create_text_node(&note);
        append(document, said, text)?;
        return append(document, made, said);
    }
    let grid = element(document, "div")?;
    attribute(document, grid, "class", "grid")?;
    for (image, pictured) in results.drawn_images().iter().enumerate() {
        let made = cell(document, pictured, place, image)?;
        append(document, grid, made)?;
    }
    append(document, made, grid)
}

/// One image of the grid: its tile, holding its picture if there is one,
/// and under it its title and its site, with the site's mark.
fn cell(
    document: &mut Document,
    pictured: &Pictured,
    place: usize,
    image: usize,
) -> Result<NodeId, Unbuilt> {
    let link = element(document, "a")?;
    attribute(document, link, "class", "pic")?;
    attribute(document, link, "href", IMAGE_HREF)?;
    let tile = element(document, "div")?;
    attribute(document, tile, "class", "img")?;
    if pictured.picture.is_some() {
        let picture = thumbnail(document, place, image)?;
        append(document, tile, picture)?;
    }
    append(document, link, tile)?;
    let caption = element(document, "div")?;
    attribute(document, caption, "class", "cap")?;
    let named = element(document, "span")?;
    attribute(document, named, "class", "ct")?;
    // The image's title and its site, strangers' strings, as text.
    let words = document.create_text_node(&pictured.title);
    append(document, named, words)?;
    append(document, caption, named)?;
    let site = element(document, "span")?;
    attribute(document, site, "class", "cs")?;
    if let Some(letter) = pictured.letter() {
        let mark = element(document, "span")?;
        attribute(
            document,
            mark,
            "class",
            &format!("fav small tint{}", pictured.tint()),
        )?;
        attribute(document, mark, "aria-hidden", "true")?;
        let text = document.create_text_node(&letter);
        append(document, mark, text)?;
        append(document, site, mark)?;
    }
    let words = document.create_text_node(&pictured.site());
    append(document, site, words)?;
    append(document, caption, site)?;
    append(document, link, caption)?;
    Ok(link)
}

/// The `<img>` drawing the `image`th image of the `place`th turn: its
/// picture by a name of ours, hidden from the tree.
fn thumbnail(document: &mut Document, place: usize, image: usize) -> Result<NodeId, Unbuilt> {
    let picture = element(document, "img")?;
    attribute(document, picture, "class", "thumb")?;
    attribute(document, picture, "src", &src(place, image))?;
    attribute(document, picture, "alt", "")?;
    attribute(document, picture, "aria-hidden", "true")?;
    Ok(picture)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::{Found, Images, Showing, Tab};
    use crate::start_document::build;

    fn url(address: &str) -> alo_url::Url {
        alo_url::parse(address).unwrap_or_else(|_| alo_url::Url::about_blank())
    }

    fn eight() -> Vec<Pictured> {
        (0..8)
            .map(|n| {
                let image = Pictured::new(
                    format!("Image {n}"),
                    url(&format!("https://p{n}.example/")),
                    "",
                );
                if n % 2 == 0 {
                    image.holding(vec![0x89, b'P', b'N', b'G', n])
                } else {
                    image
                }
            })
            .collect()
    }

    fn results(tab: Tab) -> Results {
        Results::of(
            "x",
            Showing::Found(vec![Found::new("A", url("https://a.example/"), "")]),
        )
        .with_images(Images::Found(eight()))
        .on(tab)
    }

    fn start(turns: Vec<Turn>) -> Start {
        Start {
            greeting: "Hello.".to_owned(),
            turns,
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

    #[test]
    fn all_draws_five_tiles_and_names_each_picture_by_its_place() {
        let start = start(vec![
            Turn::Failed {
                said: "a".to_owned(),
                why: "b".to_owned(),
            },
            Turn::Searched(results(Tab::All)),
        ]);
        let document = build(&start).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        assert_eq!(of_class(&document, "im held").len(), 3);
        assert_eq!(of_class(&document, "im"), vec!["Image 1", "Image 3"]);
        assert_eq!(of_class(&document, "seeall"), vec![SEE_ALL]);
        let named: Vec<String> = pictures(&start).into_iter().map(|each| each.src).collect();
        assert_eq!(
            named,
            vec![
                "alo:thumbnail-1-0",
                "alo:thumbnail-1-2",
                "alo:thumbnail-1-4"
            ]
        );
        let srcs: Vec<&str> = document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some("thumb"))
            .filter_map(|node| document.attribute_by_name(node, "src"))
            .collect();
        assert_eq!(srcs, named);
    }

    #[test]
    fn the_images_tab_draws_every_image_and_sites_none() {
        let images = start(vec![Turn::Searched(results(Tab::Images))]);
        let document = build(&images).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        assert_eq!(of_class(&document, "pic").len(), 8);
        assert_eq!(
            of_class(&document, "ct").first().map(String::as_str),
            Some("Image 0")
        );
        assert_eq!(
            of_class(&document, "cs").first().map(String::as_str),
            Some("Pp0.example")
        );
        assert_eq!(pictures(&images).len(), 4);
        assert!(of_class(&document, "found").is_empty());
        let sites = start(vec![Turn::Searched(results(Tab::Sites))]);
        let document = build(&sites).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        assert!(of_class(&document, "imrow").is_empty());
        assert!(of_class(&document, "more").is_empty());
        assert_eq!(of_class(&document, "found").len(), 1);
        assert!(pictures(&sites).is_empty());
    }

    #[test]
    fn with_no_images_all_draws_no_row_and_the_images_tab_says_why() {
        let asking = Results::of(
            "ev",
            Showing::Found(vec![Found::new("A", url("https://a.example/"), "")]),
        )
        .with_images(Images::Asking);
        let document = build(&start(vec![Turn::Searched(asking.clone())]))
            .unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        assert!(of_class(&document, "imrow").is_empty());
        let document = build(&start(vec![Turn::Searched(asking.on(Tab::Images))]))
            .unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        assert_eq!(
            of_class(&document, "note"),
            vec!["Searching Brave Search for images of \u{201c}ev\u{201d}\u{2026}"]
        );
    }

    #[test]
    fn a_title_holding_markup_stays_its_characters() {
        let hostile = "<img src=x onerror=y()></a>";
        let image = Pictured::new(hostile, url("https://a.example/"), hostile);
        let held = Results::of(
            "x",
            Showing::Found(vec![Found::new("A", url("https://a.example/"), "")]),
        )
        .with_images(Images::Found(vec![image]));
        for tab in [Tab::All, Tab::Images] {
            let document = build(&start(vec![Turn::Searched(held.clone().on(tab))]))
                .unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
            let images = document
                .descendants(document.root())
                .filter_map(|node| document.element(node))
                .filter(|element| element.name.local.as_ref() == "img")
                .count();
            assert_eq!(images, 0, "{tab:?}");
        }
    }
}
