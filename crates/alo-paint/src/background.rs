/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a box's background is: a colour, and the layers painted over it.
//!
//! Read once per box from its style, and separately from [`crate::build`]
//! because that file decides *where* things are drawn and in what order, and
//! this one decides what a background property says. A new background
//! property changes this file and not that one.
//!
//! # Which property wins
//!
//! The cascade does not expand shorthands, so `background` arrives whole
//! beside whatever longhands were written. `background-color` beats the
//! shorthand's colour and `background-image` beats its pictures, as each did
//! before there were layers; a longhand this engine cannot read gives way to
//! the shorthand rather than to nothing, and is recorded.
//!
//! # What is recorded
//!
//! A list that cannot be read at all, and a layer that was read and could not
//! be drawn — a `url()`, until queue item 311. The layers around one of those
//! are still drawn, because they are what the author wrote and drawing them is
//! not a guess.

use alo_css::{IssueKind, Location, StyleIssue};
use alo_style::ComputedStyle;
use alo_value::{Background, Gradient, Image, Rgba};

/// A box's background, ready to paint.
#[derive(Debug, Clone, Default)]
pub(crate) struct Backdrop {
    /// The colour beneath every layer.
    pub(crate) color: Option<Rgba>,
    /// The gradients to paint over it, **bottom first** — the order to draw
    /// them in, which is the reverse of the order they were written.
    pub(crate) gradients: Vec<Gradient>,
    /// What was written and not drawn, and why.
    pub(crate) refused: Vec<StyleIssue>,
}

/// Read a box's background from its style.
pub(crate) fn read(style: &ComputedStyle) -> Backdrop {
    let mut backdrop = Backdrop::default();
    let shorthand = match style.get("background") {
        Some(text) => {
            let read = alo_value::parse_background(text);
            if read.is_none() {
                backdrop.refused.push(refusal(
                    IssueKind::UnsupportedValue,
                    format!("background: {text}"),
                ));
            }
            read
        }
        None => None,
    };

    backdrop.color = match style.color("background-color") {
        Some(color) => Some(color),
        None => shorthand
            .as_ref()
            .and_then(|background| background.color)
            .map(|color| color.resolve(style.current_color())),
    };

    let longhand = style.get("background-image").and_then(|text| {
        let read = alo_value::parse_background_image(text);
        if read.is_none() {
            backdrop.refused.push(refusal(
                IssueKind::UnsupportedValue,
                format!("background-image: {text}"),
            ));
        }
        read.map(|images| ("background-image", images))
    });
    let images =
        longhand.or_else(|| shorthand.map(|Background { images, .. }| ("background", images)));
    if let Some((property, images)) = images {
        for image in images.into_iter().rev() {
            match image {
                Image::None => {}
                Image::Gradient(gradient) => backdrop.gradients.push(gradient),
                Image::Url(address) => backdrop.refused.push(refusal(
                    IssueKind::UndrawnLayer,
                    format!("{property}: url({address})"),
                )),
            }
        }
    }
    backdrop
}

fn refusal(kind: IssueKind, source: String) -> StyleIssue {
    StyleIssue {
        kind,
        source,
        at: Location { line: 0, column: 0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_css::{MediaContext, parse_stylesheet};
    use alo_dom::parse_document;
    use alo_style::{Origin, SourcedSheet, resolve};

    /// The background of the one `<div>` in a page styled by `css`.
    fn backdrop(css: &str) -> Backdrop {
        let document = parse_document("<!DOCTYPE html><div></div>");
        let author = parse_stylesheet(css);
        let styles = resolve(
            &document,
            &[SourcedSheet::new(Origin::Author, &author)],
            &MediaContext::default(),
        );
        let root = document.root();
        let div = document.descendants(root).find(|id| {
            document
                .element(*id)
                .is_some_and(|element| element.name.is_html("div"))
        });
        div.and_then(|id| styles.get(id))
            .map(read)
            .unwrap_or_default()
    }

    fn red() -> Rgba {
        Rgba::from_rgba8(255, 0, 0, 255)
    }

    #[test]
    fn a_colour_alone_is_a_colour_and_no_layers() {
        let found = backdrop("div { background: red }");
        assert_eq!(found.color, Some(red()));
        assert!(found.gradients.is_empty() && found.refused.is_empty());
    }

    #[test]
    fn layers_are_painted_from_the_last_written_to_the_first() {
        let found = backdrop(
            "div { background: linear-gradient(red, blue), \
             radial-gradient(white, black), green }",
        );
        assert_eq!(found.color, Some(Rgba::from_rgba8(0, 128, 0, 255)));
        assert!(matches!(
            found.gradients.as_slice(),
            [Gradient::Radial { .. }, Gradient::Linear { .. }]
        ));
        assert!(found.refused.is_empty());
    }

    #[test]
    fn a_longhand_beats_the_shorthand() {
        let found = backdrop(
            "div { background: linear-gradient(red, blue), green; \
             background-color: red; background-image: none }",
        );
        assert_eq!(found.color, Some(red()));
        assert!(found.gradients.is_empty());
    }

    #[test]
    fn a_picture_is_recorded_and_the_rest_still_drawn() {
        let found = backdrop("div { background: url(hand.png), linear-gradient(red, blue), red }");
        assert_eq!(found.color, Some(red()));
        assert_eq!(found.gradients.len(), 1);
        let said: Vec<String> = found.refused.iter().map(ToString::to_string).collect();
        assert_eq!(said.len(), 1, "{said:?}");
        assert!(
            said.iter().all(|line| line.contains("url(hand.png)")),
            "{said:?}"
        );
    }

    #[test]
    fn a_list_that_cannot_be_read_is_recorded_and_draws_nothing() {
        for css in [
            "div { background: linear-gradient(red, blue) no-repeat, red }",
            "div { background: red, linear-gradient(red, blue) }",
            "div { background: conic-gradient(red, blue) }",
            "div { background: radial-gradient(circle 10px, red, blue) }",
            "div { background: linear-gradient(red, blue),, red }",
        ] {
            let found = backdrop(css);
            assert_eq!(found.color, None, "{css}");
            assert!(found.gradients.is_empty(), "{css}");
            assert_eq!(found.refused.len(), 1, "{css}");
        }
    }

    #[test]
    fn an_unreadable_longhand_gives_way_to_the_shorthand_and_says_so() {
        let found = backdrop(
            "div { background: linear-gradient(red, blue); \
             background-image: image-set(a.png 1x) }",
        );
        assert_eq!(found.gradients.len(), 1);
        assert_eq!(found.refused.len(), 1);
    }
}
