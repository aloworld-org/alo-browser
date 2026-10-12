/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a button, a text field and a text area look like while the page
//! leaves them alone: Chrome's own picture of each.
//!
//! Chrome 154 computes a `2px outset` border for a button and a `2px inset`
//! one for a field, and the user-agent sheet says the same, so the
//! control takes the room they take. It does not draw them (queue item
//! 505). In their place it draws:
//!
//! - **a fill**, `#efefef` for a button and white for a field, over the
//!   border box half a pixel in, rounded 2 pixels;
//! - **an edge** over it, one pixel of `#767676` around the border box,
//!   its outer corners rounded 2.5 pixels and its inner corners 1.5;
//! - **disabled**, the same shapes in translucent greys: an edge at three
//!   tenths, whose outer half lies over the page and inner half over the
//!   fill.
//!
//! That is the checkbox's edge ([`crate::toggle`]): a one-pixel stroke
//! along a 2-pixel corner, half a pixel in.
//!
//! A `<textarea>` is a field here. It computes a `1px solid` border rather
//! than two inset pixels, and Chrome draws a field's picture in it exactly,
//! every pixel of 60 by 28 at one and at eight device pixels to one, live,
//! disabled and read-only, over black, white and a green (queue item 510).
//!
//! # Measured, not read
//!
//! From Chrome 154's screenshots of a 40 by 22 button and field, on whole
//! pixels, at eight device pixels to one for the shapes and at one to one
//! for the colours, over black and over white. Over black a translucent
//! grey reads as its colour times its alpha, and over white as that plus
//! the white it lets through, which is what separates the two. At eight
//! to one a disabled edge is four device pixels of edge over the page and
//! four of edge over the fill, which is where the fill begins. Chrome's
//! source was not read.
//!
//! # Only while the page has said nothing
//!
//! A page that writes any `border` or `background` property on the
//! control, even one that names the engine's own value, or `appearance:
//! none`, gets its CSS drawn instead: the two-pixel bevel and all. A page
//! that writes its padding, its colour, its size or its font keeps the
//! picture. All of that was measured in the same screenshots, and
//! [`drawn`] says it.

use crate::corner::{Corners, ring, rounded_rectangle};
use crate::toggle::{EDGE, Layer, grey};
use alo_box::Face;
use alo_layout::{Edges, Rect};
use alo_style::ComputedStyle;
use alo_value::Rgba;

/// A live button's fill: `#efefef`, the user-agent sheet's background.
pub const BUTTON_FILL: Rgba = Rgba {
    red: 239.0 / 255.0,
    green: 239.0 / 255.0,
    blue: 239.0 / 255.0,
    alpha: 1.0,
};

/// A live field's fill.
pub const FIELD_FILL: Rgba = Rgba::WHITE;

/// A disabled control's edge: 36 over black and 214 over white, so
/// `#767676` at 77/255, which is the `rgba(118, 118, 118, 0.3)` Chrome
/// computes for its border.
pub const DISABLED_EDGE: Rgba = grey(36.0, 214.0);

/// A disabled button's fill: 96 over black, 238 over white.
pub const DISABLED_BUTTON_FILL: Rgba = grey(96.0, 238.0);

/// A disabled field's fill: 167 over black, 248 over white.
pub const DISABLED_FIELD_FILL: Rgba = grey(167.0, 248.0);

/// The corner the edge is stroked along, in pixels, at any size.
const CORNER: f32 = 2.0;

/// How wide the edge is, in pixels, at any size.
const EDGE_WIDTH: f32 = 1.0;

/// Whether a control with this style draws its picture: only while the
/// page has written no `border` or `background` property on it, and has
/// not written `appearance: none`.
pub fn drawn(style: &ComputedStyle) -> bool {
    let none = style
        .get("appearance")
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("none"));
    !none && !style.authored_names().any(|name| restyles(name.as_str()))
}

/// Whether a page that writes this property has taken over the control's
/// drawing. Every `border` and `background` longhand and shorthand,
/// `border-radius` and `border-image` among them, but not the two `border-`
/// properties that belong to tables.
fn restyles(name: &str) -> bool {
    name.starts_with("background")
        || (name.starts_with("border") && !matches!(name, "border-collapse" | "border-spacing"))
}

/// The picture a button or a field (a text area included) draws of itself
/// in `border_box`, back to front.
pub fn artwork(face: Face, disabled: bool, border_box: Rect) -> Vec<Layer> {
    let (fill, edge) = match (face, disabled) {
        (Face::Button, false) => (BUTTON_FILL, EDGE),
        (Face::Field, false) => (FIELD_FILL, EDGE),
        (Face::Button, true) => (DISABLED_BUTTON_FILL, DISABLED_EDGE),
        (Face::Field, true) => (DISABLED_FIELD_FILL, DISABLED_EDGE),
    };
    let side = border_box.size.width.min(border_box.size.height).max(0.0);
    let half = (EDGE_WIDTH / 2.0).min(side / 2.0);
    let widths = Edges::all(EDGE_WIDTH.min(side / 2.0));
    vec![
        Layer {
            path: rounded_rectangle(border_box.shrunk_by(Edges::all(half)), Corners::all(CORNER)),
            color: fill,
        },
        Layer {
            path: ring(border_box, Corners::all(CORNER + half), widths),
            color: edge,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_css::{MediaContext, parse_stylesheet};
    use alo_dom::parse_document;
    use alo_style::{Origin, SourcedSheet, resolve};

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    fn bounds(layer: Option<&Layer>) -> (f32, f32, f32, f32) {
        layer
            .and_then(|layer| layer.path.bounds())
            .unwrap_or((0.0, 0.0, 0.0, 0.0))
    }

    /// The style of the first `<button>`, with a page's sheet over an
    /// engine sheet that styles it as the real one does.
    fn button_style(author: &str) -> ComputedStyle {
        let document = parse_document("<button style='padding: 4px'>b</button>");
        let agent = parse_stylesheet("button { border: 2px outset black; background: #efefef }");
        let author = parse_stylesheet(author);
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &agent),
            SourcedSheet::new(Origin::Author, &author),
        ];
        let styles = resolve(&document, &sheets, &MediaContext::default());
        document
            .descendants(document.root())
            .find(|id| {
                document
                    .element(*id)
                    .is_some_and(|element| element.name.is_html("button"))
            })
            .and_then(|id| styles.get(id))
            .cloned()
            .unwrap_or_default()
    }

    #[test]
    fn a_button_is_a_grey_fill_inside_a_one_pixel_rounded_edge() {
        let layers = artwork(Face::Button, false, Rect::new(4.0, 4.0, 40.0, 22.0));
        let colors: Vec<Rgba> = layers.iter().map(|layer| layer.color).collect();
        assert_eq!(colors, vec![BUTTON_FILL, EDGE]);
        // The fill is half a pixel in, under the edge; the edge is the
        // whole border box.
        let fill = bounds(layers.first());
        assert!(close(fill.0, 4.5) && close(fill.1, 4.5), "{fill:?}");
        assert!(close(fill.2, 43.5) && close(fill.3, 25.5), "{fill:?}");
        let edge = bounds(layers.get(1));
        assert!(close(edge.0, 4.0) && close(edge.3, 26.0), "{edge:?}");
    }

    #[test]
    fn a_field_is_white_and_a_disabled_control_is_translucent() {
        let field = artwork(Face::Field, false, Rect::new(0.0, 0.0, 40.0, 22.0));
        let colors: Vec<Rgba> = field.iter().map(|layer| layer.color).collect();
        assert_eq!(colors, vec![FIELD_FILL, EDGE]);
        for (face, fill) in [
            (Face::Button, DISABLED_BUTTON_FILL),
            (Face::Field, DISABLED_FIELD_FILL),
        ] {
            let layers = artwork(face, true, Rect::new(0.0, 0.0, 40.0, 22.0));
            let colors: Vec<Rgba> = layers.iter().map(|layer| layer.color).collect();
            assert_eq!(colors, vec![fill, DISABLED_EDGE]);
        }
    }

    #[test]
    fn a_disabled_grey_reads_as_measured_over_black_and_over_white() {
        for (color, black, white) in [
            (DISABLED_EDGE, 36.0, 214.0),
            (DISABLED_BUTTON_FILL, 96.0, 238.0),
            (DISABLED_FIELD_FILL, 167.0, 248.0),
        ] {
            let over_black = color.red * color.alpha * 255.0;
            let over_white = over_black + 255.0 * (1.0 - color.alpha);
            assert!((over_black - black).abs() < 0.01, "{color:?}");
            assert!((over_white - white).abs() < 0.01, "{color:?}");
        }
        // And the edge over the fill is what Chrome draws where they meet:
        // 202 over white for a button, 152 over black for a field.
        let over = |top: Rgba, under: f32| top.red * 255.0 * top.alpha + under * (1.0 - top.alpha);
        assert!((over(DISABLED_EDGE, 238.0) - 202.0).abs() < 1.0);
        assert!((over(DISABLED_EDGE, 167.0) - 152.0).abs() < 1.0);
    }

    #[test]
    fn a_tiny_control_still_draws_inside_itself() {
        let layers = artwork(Face::Field, false, Rect::new(0.0, 0.0, 1.0, 10.0));
        let fill = bounds(layers.first());
        assert!(fill.0 >= 0.0 && fill.2 <= 1.0, "{fill:?}");
        let empty = artwork(Face::Button, false, Rect::new(3.0, 3.0, 0.0, 0.0));
        assert!(empty.iter().all(|layer| {
            layer
                .path
                .bounds()
                .is_none_or(|(l, t, r, b)| close(l, r) || close(t, b))
        }));
    }

    #[test]
    fn a_page_that_writes_a_border_or_a_background_draws_its_own() {
        assert!(drawn(&button_style("")));
        for kept in [
            "button { padding: 9px }",
            "button { color: blue; width: 40px; font-size: 30px }",
            "button { appearance: auto }",
            "button { border-collapse: collapse }",
        ] {
            assert!(drawn(&button_style(kept)), "{kept}");
        }
        for taken in [
            "button { background: #efefef }",
            "button { background-image: none }",
            "button { border-color: red }",
            "button { border-top-color: black }",
            "button { border-radius: 0 }",
            "button { appearance: none }",
        ] {
            assert!(!drawn(&button_style(taken)), "{taken}");
        }
    }
}
