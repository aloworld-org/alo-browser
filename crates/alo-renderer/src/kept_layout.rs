/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A layout kept from the drawing before, when nothing it is made from
//! changed (queue item 890).
//!
//! Every drawing of the first screen is a new document, styled, boxed and
//! laid out again ([`crate::pipeline`]): a Tab in *Settings* over a card at
//! two spent about 19 of its 38 ms laying out a document whose boxes did
//! not move — only a ring's class changed, and a ring is a `box-shadow`,
//! which paint draws and layout never reads.
//!
//! Layout is a function of nothing but its inputs ([`alo_layout::compute`]):
//! the boxes, the styles, the viewport and the faces text is measured in.
//! So the last drawing's layout is this drawing's when
//!
//! - the box tree is equal, every box, its text and every side map;
//! - every element's and pseudo-element's style is alike but for the
//!   properties only stages after layout read — paint, SVG, the renderer's
//!   choice of fonts — as `alo-css`' list of what each crate reads says
//!   ([`alo_css::properties::read_only_by`]), which a test in every reading
//!   crate holds to its source; a property layout or the cascade reads, or
//!   one not listed, must be equal;
//! - the viewport is the same size, bit for bit, and the faces are the
//!   very same faces ([`FontDatabase::is`]).
//!
//! Otherwise the document is laid out again, as before. Laid out alike is
//! held by tests that lay each kept drawing out again and compare every
//! number.

use alo_box::BoxTree;
use alo_css::properties::{Reader, read_only_by};
use alo_layout::{LayoutTree, Size};
use alo_style::StyleTree;
use alo_text::{FontDatabase, TextMeasurer};

use crate::pipeline::Drawing;

/// The stages that run after layout, each of which is run again for every
/// drawing: what only they read cannot move a box.
const AFTER_LAYOUT: [Reader; 3] = [Reader::Paint, Reader::Svg, Reader::Renderer];

/// What a layout was made from, as the drawing that holds it keeps it.
pub struct LaidOutFrom<'a> {
    /// The boxes.
    pub boxes: &'a BoxTree,
    /// Their styles.
    pub styles: &'a StyleTree,
    /// The viewport, in CSS pixels.
    pub viewport: Size,
    /// The faces text was measured in.
    pub fonts: &'a FontDatabase,
}

/// Whether a layout made from `before` is the layout `now` would make.
pub fn lays_out_alike(before: &LaidOutFrom<'_>, now: &LaidOutFrom<'_>) -> bool {
    before.viewport.width.to_bits() == now.viewport.width.to_bits()
        && before.viewport.height.to_bits() == now.viewport.height.to_bits()
        && before.fonts.is(now.fonts)
        && before.boxes == now.boxes
        && before
            .styles
            .alike_but_for(now.styles, &|name| read_only_by(name, &AFTER_LAYOUT))
}

/// The layout of `boxes` and `styles` for `viewport` in `fonts`, and
/// whether it is `before`'s, kept: made afresh unless `before` was made from
/// what lays out alike.
pub fn laid_out(
    before: Option<&Drawing>,
    (boxes, styles): (&BoxTree, &StyleTree),
    viewport: Size,
    fonts: &FontDatabase,
) -> (LayoutTree, bool) {
    let now = LaidOutFrom {
        boxes,
        styles,
        viewport,
        fonts,
    };
    let from = before.map(|drawing| {
        (
            LaidOutFrom {
                boxes: &drawing.boxes,
                styles: &drawing.styles,
                viewport: drawing.layout.viewport(),
                fonts: &drawing.fonts,
            },
            &drawing.layout,
        )
    });
    match kept(from, &now) {
        Some(layout) => (layout, true),
        None => (
            alo_layout::compute(boxes, styles, viewport, &TextMeasurer::new(fonts)),
            false,
        ),
    }
}

/// The layout for `now`: `before`'s own, taken, when it lays out alike, or
/// [`None`] when it has to be made.
pub fn kept(
    before: Option<(LaidOutFrom<'_>, &LayoutTree)>,
    now: &LaidOutFrom<'_>,
) -> Option<LayoutTree> {
    let (from, layout) = before?;
    lays_out_alike(&from, now).then(|| layout.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};

    /// A page's boxes and styles under `css`.
    fn styled(html: &str, css: &str) -> (BoxTree, StyleTree) {
        let document = alo_dom::parse_document(html);
        let agent = alo_css::parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let author = alo_css::parse_stylesheet(css);
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &agent),
            SourcedSheet::new(Origin::Author, &author),
        ];
        let device = alo_css::MediaContext::sized(200.0, 100.0, alo_css::ColorScheme::Light);
        let styles = resolve(&document, &sheets, &device);
        let boxes = alo_box::build(&document, &styles);
        (boxes, styles)
    }

    fn alike(css: &str, other: &str) -> bool {
        alike_in(
            "<!DOCTYPE html><html><body><div class=a>one</div></body></html>",
            css,
            other,
        )
    }

    fn alike_in(html: &str, css: &str, other: &str) -> bool {
        let fonts = FontDatabase::new();
        let (one, one_styles) = styled(html, css);
        let (two, two_styles) = styled(html, other);
        let from = |boxes, styles| LaidOutFrom {
            boxes,
            styles,
            viewport: Size::new(200.0, 100.0),
            fonts: &fonts,
        };
        lays_out_alike(&from(&one, &one_styles), &from(&two, &two_styles))
    }

    #[test]
    fn what_only_paint_reads_lays_out_alike() {
        assert!(alike(".a { width: 20px }", ".a { width: 20px }"));
        assert!(alike(
            ".a { width: 20px }",
            ".a { width: 20px; box-shadow: 0 0 0 3px red; opacity: 0.5 }"
        ));
        assert!(alike(".a { background: red }", ".a { background: blue }"));
    }

    #[test]
    fn what_layout_or_the_cascade_reads_does_not() {
        assert!(!alike(".a { width: 20px }", ".a { width: 21px }"));
        assert!(!alike(".a { }", ".a { padding: 1px }"));
        assert!(
            !alike(".a { }", ".a { font-weight: 700 }"),
            "a face is chosen by it"
        );
        assert!(
            !alike(".a { }", ".a { color: red }"),
            "the cascade reads it"
        );
        assert!(!alike(".a { }", ".a { display: none }"), "other boxes");
        assert!(
            !alike(".a { }", ".a { --gap: 3px }"),
            "custom properties are compared"
        );
        let list = "<!DOCTYPE html><html><body><ul><li>one</li></ul></body></html>";
        assert!(alike_in(list, "", ""));
        assert!(
            !alike_in(list, "", "::marker { font-size: 30px }"),
            "a pseudo-element's style is compared"
        );
    }

    #[test]
    fn another_viewport_or_other_faces_do_not() {
        let html = "<!DOCTYPE html><html><body><p>one</p></body></html>";
        let (boxes, styles) = styled(html, "");
        let fonts = FontDatabase::new();
        let mut more = FontDatabase::new();
        more.map_generic("serif", "Nothing");
        let at = |viewport, fonts| LaidOutFrom {
            boxes: &boxes,
            styles: &styles,
            viewport,
            fonts,
        };
        let here = at(Size::new(200.0, 100.0), &fonts);
        assert!(lays_out_alike(&here, &at(Size::new(200.0, 100.0), &fonts)));
        assert!(!lays_out_alike(&here, &at(Size::new(200.0, 100.5), &fonts)));
        assert!(!lays_out_alike(&here, &at(Size::new(199.5, 100.0), &fonts)));
        assert!(!lays_out_alike(&here, &at(Size::new(200.0, 100.0), &more)));
    }
}
