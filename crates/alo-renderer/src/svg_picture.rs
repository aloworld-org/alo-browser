/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An SVG file shown by an `<img>`: a document of its own (ADR 0027).
//!
//! # What it is
//!
//! A second [`Document`], read by `alo-dom`'s `read_svg` from the bytes alone
//! and held by the pipeline for the box that shows it. It is never put in the
//! page's tree, never given a JavaScript wrapper and never put in a heap, so
//! no script, the page's or its own, can reach it (§ 4).
//!
//! # What it sees: nothing of the page
//!
//! Its cascade is the user-agent sheet and its own `<style>` elements, and
//! nothing else. The page's sheets do not match inside it, the page's custom
//! properties do not reach it, and `currentColor` is the file's own `color`,
//! or the initial one. Its media queries take the picture's own size as their
//! viewport (§ 4). Every other media feature is the page's, which today means
//! the light scheme every page here is drawn in.
//!
//! # What it can do: nothing
//!
//! Nothing in this file is handed a way to cause a request. [`SvgPicture::read`]
//! takes bytes and [`SvgPicture::draw`] takes a size, and that is all either
//! has. So an `<image>`, a `<use>` naming another file, an `@import`, an
//! `<?xml-stylesheet?>`, a `<link>` and an external DTD are inert by what this
//! code is given rather than by a check somebody could forget. A `<link>`
//! sheet is recorded as not fetched, and the rest are recorded by the reader
//! or the walk as they already are for inline SVG. `<script>` is kept in the
//! tree and run by nothing, because no realm is ever made for this document.
//!
//! # How big it is, and where it is drawn
//!
//! Its natural size is what an inline `<svg>` says of itself
//! (`alo_box::svg::natural_size`): the root's `width` and `height` when they
//! are absolute lengths, and a ratio from those or from the `viewBox` (§ 5).
//! It is drawn as vectors at the size layout gave the `<img>`, never
//! rasterised at its own size and scaled, and the drawing is
//! [`alo_paint::Drawing::confined`] to that box.

use alo_box::NaturalSize;
use alo_css::{ColorScheme, MediaContext, parse_stylesheet};
use alo_dom::sheets::Sheet;
use alo_dom::{Document, NodeId, XmlRefusal};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET};
use alo_svg::Drawn;

/// An SVG file, read and ready to be drawn at whatever size it is given.
#[derive(Debug)]
pub struct SvgPicture {
    document: Document,
    root: NodeId,
    natural: NaturalSize,
    issues: Vec<String>,
}

impl SvgPicture {
    /// Read an SVG file.
    ///
    /// Whether the bytes were meant as SVG is the caller's to have decided,
    /// by their type (ADR 0027 § 1).
    ///
    /// # Errors
    ///
    /// Why the bytes are not an SVG document this engine reads, from
    /// `alo-dom`'s reader. Any input at all returns a picture or a refusal,
    /// never a panic.
    pub fn read(bytes: &[u8]) -> Result<Self, XmlRefusal> {
        let document = alo_dom::read_svg(bytes)?;
        let root = document
            .children(document.root())
            .find(|child| document.element(*child).is_some())
            .ok_or(XmlRefusal::NotOneRoot)?;
        let (natural, issues) = document
            .element(root)
            .map(alo_box::svg::natural_size)
            .unwrap_or_default();
        Ok(Self {
            document,
            root,
            natural,
            issues,
        })
    }

    /// What the file says of its own size and shape.
    pub fn natural_size(&self) -> NaturalSize {
        self.natural
    }

    /// The document read from the file. Nothing in the page can reach it;
    /// this is for a test to look at.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Draw the file into a box of `size` CSS pixels, with its own cascade.
    ///
    /// The issues are everything the file asked for that was refused, left
    /// out or not fetched, beginning with what reading its size refused.
    pub fn draw(&self, size: (f32, f32)) -> Drawn {
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let mut issues = self.issues.clone();
        let own: Vec<_> = alo_dom::sheets::asked_for(&self.document)
            .into_iter()
            .filter_map(|sheet| match sheet {
                Sheet::Written(text) => Some(parse_stylesheet(&text)),
                Sheet::Linked { href } => {
                    issues.push(format!(
                        "a style sheet at {href:?} is not fetched for an SVG picture"
                    ));
                    None
                }
            })
            .collect();
        let mut sheets = vec![SourcedSheet::new(Origin::UserAgent, &agent)];
        sheets.extend(
            own.iter()
                .map(|sheet| SourcedSheet::new(Origin::Author, sheet)),
        );
        issues.extend(
            own.iter()
                .flat_map(alo_css::Stylesheet::issues)
                .map(ToString::to_string),
        );
        let device = MediaContext::sized(size.0, size.1, ColorScheme::Light);
        let styles = alo_style::resolve(&self.document, &sheets, &device);
        issues.extend(styles.issues().iter().map(ToString::to_string));
        let drawn = alo_svg::draw(&self.document, &styles, self.root, size);
        issues.extend(drawn.issues);
        Drawn {
            drawing: drawn.drawing.confined(),
            issues,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_paint::DrawingItem;

    const BLACK: (u8, u8, u8, u8) = (0, 0, 0, 255);
    const GREEN: (u8, u8, u8, u8) = (0, 255, 0, 255);
    const BLUE: (u8, u8, u8, u8) = (0, 0, 255, 255);

    fn read(svg: &str) -> Option<SvgPicture> {
        SvgPicture::read(svg.as_bytes()).ok()
    }

    fn colours(drawn: &Drawn) -> Vec<(u8, u8, u8, u8)> {
        drawn
            .drawing
            .items()
            .iter()
            .filter_map(|item| match item {
                DrawingItem::Fill { color, .. } => Some(color.to_rgba8()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn its_natural_size_is_its_absolute_width_and_height_and_its_ratio() {
        let sized = read(r#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="1in"/>"#)
            .expect("a picture");
        assert_eq!(sized.natural_size().width, Some(48.0));
        assert_eq!(sized.natural_size().height, Some(96.0));
        assert_eq!(sized.natural_size().ratio(), Some(0.5));

        let shaped = read(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 395 385"/>"#)
            .expect("a picture");
        assert_eq!(shaped.natural_size().width, None);
        assert_eq!(shaped.natural_size().height, None);
        assert_eq!(shaped.natural_size().ratio(), Some(395.0 / 385.0));

        let relative = read(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="50%" height="2em" viewBox="0 0 2 1"/>"#,
        )
        .expect("a picture");
        assert_eq!(relative.natural_size().width, None, "a share is no size");
        assert_eq!(relative.natural_size().height, None);
        assert_eq!(relative.natural_size().ratio(), Some(2.0));
    }

    #[test]
    fn a_file_that_is_not_svg_is_refused_by_the_reader() {
        assert!(matches!(
            SvgPicture::read(b"<html/>"),
            Err(XmlRefusal::NotSvg(_))
        ));
        assert!(SvgPicture::read(b"\x89PNG\r\n\x1a\n").is_err());
    }

    #[test]
    fn it_is_drawn_at_the_size_it_is_given_and_confined_there() {
        let picture = read(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10"/></svg>"#,
        )
        .expect("a picture");
        let drawn = picture.draw((40.0, 20.0));
        assert!(drawn.drawing.is_confined());
        let Some(DrawingItem::Fill { path, .. }) = drawn.drawing.items().first() else {
            panic!("a fill: {drawn:?}");
        };
        assert_eq!(
            path.bounds(),
            Some((10.0, 0.0, 30.0, 20.0)),
            "a square met into a 40 × 20 box, centred",
        );
    }

    #[test]
    fn its_own_style_sheet_applies_and_current_color_is_its_own() {
        let picture = read(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 2 1">
                 <style><![CDATA[ .a { fill: #00ff00 } ]]></style>
                 <rect class="a" width="1" height="1" fill="red"/>
                 <rect x="1" width="1" height="1" fill="currentColor"/>
               </svg>"#,
        )
        .expect("a picture");
        let drawn = picture.draw((20.0, 10.0));
        assert_eq!(
            colours(&drawn),
            vec![GREEN, BLACK],
            "its own sheet beats its presentation attribute, and the initial colour is black",
        );
    }

    #[test]
    fn its_media_queries_see_its_own_size() {
        let picture = read(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1 1">
                 <style>rect { fill: #0000ff } @media (max-width: 30px) { rect { fill: #00ff00 } }</style>
                 <rect width="1" height="1"/>
               </svg>"#,
        )
        .expect("a picture");
        assert_eq!(colours(&picture.draw((20.0, 20.0))), vec![GREEN]);
        assert_eq!(colours(&picture.draw((40.0, 40.0))), vec![BLUE]);
    }

    #[test]
    fn a_linked_sheet_is_recorded_and_never_asked_for() {
        let picture = read(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:h="http://www.w3.org/1999/xhtml" viewBox="0 0 1 1">
                 <h:link rel="stylesheet" href="https://elsewhere.example/a.css"/>
                 <rect width="1" height="1"/>
               </svg>"#,
        )
        .expect("a picture");
        let drawn = picture.draw((10.0, 10.0));
        assert_eq!(colours(&drawn), vec![BLACK]);
        assert!(
            drawn
                .issues
                .iter()
                .any(|issue| issue.contains("https://elsewhere.example/a.css")
                    && issue.contains("not fetched")),
            "{:?}",
            drawn.issues,
        );
    }
}
