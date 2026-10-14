/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a checkbox and a radio look like: Chrome's own picture of each.
//!
//! An `<input type=checkbox>` was a square `#767676` border over the page's
//! background, and a radio the same border rounded by the user-agent sheet,
//! until queue item 454. Chrome 154 draws neither with CSS. It computes no
//! border and no background for either, and draws the control itself:
//!
//! - **unchecked**, a white inside within a one-pixel `#767676` edge: a
//!   square whose outer corners are rounded 2.5 pixels, or a circle;
//! - **checked**, a checkbox filled with the accent colour and a tick in
//!   white across it, and a radio whose edge is the accent colour around
//!   a white ring and an accent dot six tenths of it across;
//! - **mixed**, a checkbox filled like a checked one with a bar across it;
//! - **disabled**, an inside of translucent grey a fifth of a pixel in,
//!   and over it `#767676` at 77/255 as the edge, the checked fill or the
//!   dot, whatever the page's `accent-color` says.
//!
//! The shape is the largest square that fits in the box, centred
//! ([`crate::control`]'s field): a checkbox thirty pixels wide is a
//! thirteen-pixel square in its middle, as in Chrome.
//!
//! # Measured, not read
//!
//! Every number here was measured from Chrome 154's screenshots, not read
//! from its source: shapes at eight device pixels to one, fitted by
//! coverage at 13 and 20 pixels square, and a disabled control's at one and
//! two as well; colours at one to one, but for a disabled edge, which at
//! one to one is never alone, and was read at eight. The disabled
//! colours were taken over black, white and grey, which is what separates a
//! colour from its opacity: over black a translucent grey reads as its
//! colour times its alpha, over white as that plus the white it lets
//! through. Each is written below as the two readings it came from.
//!
//! # The edge is not the author's border
//!
//! The edge is part of the picture, so it is drawn whether or not the page
//! gave the box a border, and inside the box rather than around it. What
//! Chrome does with a border a page *does* set on a checkbox — ignores it —
//! is queue item 453.

use crate::control::{self, Mark};
use crate::corner::{Corners, ring, rounded_rectangle};
use crate::path::Path;
use alo_box::{Checked, Toggle};
use alo_layout::{Edges, Rect};
use alo_value::Rgba;

/// The edge of a live control that is not checked: `#767676`.
pub const EDGE: Rgba = Rgba {
    red: 118.0 / 255.0,
    green: 118.0 / 255.0,
    blue: 118.0 / 255.0,
    alpha: 1.0,
};

/// The inside of a live control, and the ring between a checked radio's
/// edge and its dot.
pub const INSIDE: Rgba = Rgba::WHITE;

/// A disabled control's edge, and what it draws its state in: `#767676` at
/// 77/255, the `rgba(118, 118, 118, 0.3)` Chrome computes for a disabled
/// control's border. It reads 36 over black and 214 over white, and 152
/// and 209 over [`DISABLED_INSIDE`]; the two readings alone, taken as a
/// grey, would make that 153, which Chrome does not draw. It is a disabled
/// button's and field's edge too ([`crate::face`]). Measured at eight device
/// pixels to one, where a radio's outermost pixel is this colour over the
/// page alone.
pub const DISABLED_EDGE: Rgba = Rgba {
    red: 118.0 / 255.0,
    green: 118.0 / 255.0,
    blue: 118.0 / 255.0,
    alpha: 77.0 / 255.0,
};

/// A disabled control's inside: 167 over black, 248 over white.
pub const DISABLED_INSIDE: Rgba = grey(167.0, 248.0);

/// How far in from the edge's outside a disabled control's inside begins,
/// in pixels, at any size: a fifth. Fitted by coverage at one, two and
/// eight device pixels to one, at 13 and 20 pixels square (queue item 578);
/// at eight to one the outermost device pixel is the edge over the page and
/// the next the edge over two fifths of the inside.
const DISABLED_INSET: f32 = 0.2;

/// A disabled checkbox's tick, drawn over [`DISABLED_EDGE`] over
/// [`DISABLED_INSIDE`], which reads 152 over black and 209 over white:
/// white at the opacity that turns that 152 into the tick's 214 — 62/103.
/// Over white the two make 237, which is what Chrome draws there.
pub const DISABLED_MARK: Rgba = Rgba {
    red: 1.0,
    green: 1.0,
    blue: 1.0,
    alpha: 62.0 / 103.0,
};

/// A translucent grey from what it reads as over black and over white.
///
/// Over white, a grey at alpha `a` lets `255 × (1 − a)` of the white through,
/// so the difference between the two readings is that; over black it is the
/// grey times `a` alone.
pub(crate) const fn grey(over_black: f32, over_white: f32) -> Rgba {
    let alpha = (255.0 - (over_white - over_black)) / 255.0;
    let level = over_black / (255.0 - (over_white - over_black));
    Rgba {
        red: level,
        green: level,
        blue: level,
        alpha,
    }
}

/// How round a checkbox's corners are, in pixels, at any size: 2. Chrome
/// fills a checked one to that, and strokes an unchecked one's edge along
/// it half a pixel in, so the edge's outside is rounded 2.5 and its inside
/// 1.5 — fitted at two, four and eight device pixels to one.
const CORNER: f32 = 2.0;

/// How wide the edge is, in pixels, at any size.
const EDGE_WIDTH: f32 = 1.0;

/// The state a toggle is drawn in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct State {
    /// Whether it is checked, not, or mixed.
    pub checked: Option<Checked>,
    /// Whether it cannot be operated.
    pub disabled: bool,
    /// The page's `accent-color`, if it set one.
    pub accent: Option<Rgba>,
}

/// One shape of the picture, in the order it is drawn.
#[derive(Debug, Clone)]
pub struct Layer {
    /// The shape, to be filled by the non-zero rule.
    pub path: Path,
    /// What it is filled with.
    pub color: Rgba,
}

/// The picture a toggle draws of itself in `border_box`, back to front.
///
/// # A disabled control
///
/// Chrome draws a disabled control's inside, then its edge over it in one
/// translucent grey, [`DISABLED_EDGE`], which is also what a checked one
/// draws its fill or its dot in. Each shape is composited on its own,
/// source over: where the edge crosses the inside the two hide more of the
/// page than either alone (0.79 against 0.68), and the inside stops a fifth
/// of a pixel short of the edge's outside, so the outermost fifth is the
/// edge over the page alone. Measured at one, two and eight device pixels to
/// one over black and over white (queue item 578): drawn by exact
/// coverage, every disabled radio's pixel is that within 9 of 255 at one
/// and at two device pixels to one, and 0.17 on average at eight, where
/// pieces drawn side by side, or one picture faded once, are 24 or more
/// from Chrome's at one to one.
pub fn artwork(toggle: Toggle, state: State, border_box: Rect) -> Vec<Layer> {
    let field = control::field(border_box);
    let corners = match toggle {
        Toggle::CheckBox => Corners::all(CORNER + EDGE_WIDTH / 2.0),
        // A circle: a rounded square whose radii are half its side, fitted to
        // whatever is left inside the edge too.
        Toggle::Radio => Corners::all(field.size.width / 2.0),
    };
    let edge_widths = Edges::all(EDGE_WIDTH.min(field.size.width / 2.0));
    let whole = || rounded_rectangle(field, corners);
    let edge = || ring(field, corners, edge_widths);
    let mark = match toggle {
        Toggle::CheckBox => match state.checked {
            Some(Checked::Yes) => Some(Mark::Tick),
            Some(Checked::Mixed) => Some(Mark::Dash),
            _ => None,
        },
        Toggle::Radio => (state.checked == Some(Checked::Yes)).then_some(Mark::Dot),
    };
    let layer = |path: Path, color: Rgba| Layer { path, color };
    if state.disabled {
        let inset = DISABLED_INSET.min(field.size.width / 2.0);
        let inside = rounded_rectangle(
            field.shrunk_by(Edges::all(inset)),
            match toggle {
                Toggle::CheckBox => Corners::all(CORNER),
                Toggle::Radio => Corners::all(field.size.width / 2.0 - inset),
            },
        );
        let mut layers = vec![layer(inside, DISABLED_INSIDE)];
        layers.extend(match (toggle, mark) {
            (_, None) => vec![layer(edge(), DISABLED_EDGE)],
            (Toggle::Radio, Some(mark)) => vec![
                layer(edge(), DISABLED_EDGE),
                layer(control::mark(mark, field), DISABLED_EDGE),
            ],
            (Toggle::CheckBox, Some(mark)) => vec![
                layer(
                    rounded_rectangle(field, Corners::all(CORNER)),
                    DISABLED_EDGE,
                ),
                layer(control::mark(mark, field), DISABLED_MARK),
            ],
        });
        return layers;
    }
    // A live control's shapes are opaque, so each is drawn over the last:
    // the inside reaches under the edge, where it cannot be seen, and the
    // pixels the two share are the edge over the inside, with no seam.
    let accent = state.accent.unwrap_or(control::DEFAULT_ACCENT);
    match (toggle, mark) {
        // Unchecked, or a radio in the mixed state, which ARIA says is to be
        // treated as unchecked.
        (_, None) => vec![layer(whole(), INSIDE), layer(edge(), EDGE)],
        // A checked radio keeps its inside: the accent is its edge and its
        // dot, with a ring of white between.
        (Toggle::Radio, Some(mark)) => vec![
            layer(whole(), INSIDE),
            layer(edge(), accent),
            layer(control::mark(mark, field), accent),
        ],
        // A checked or mixed checkbox is filled edge to edge, rounded to
        // [`CORNER`], and the mark drawn across it.
        (Toggle::CheckBox, Some(mark)) => vec![
            layer(rounded_rectangle(field, Corners::all(CORNER)), accent),
            layer(control::mark(mark, field), control::mark_color(accent)),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    fn live(checked: Option<Checked>) -> State {
        State {
            checked,
            disabled: false,
            accent: None,
        }
    }

    fn bounds(layer: &Layer) -> (f32, f32, f32, f32) {
        layer.path.bounds().unwrap_or((0.0, 0.0, 0.0, 0.0))
    }

    fn colors(artwork: &[Layer]) -> Vec<Rgba> {
        artwork.iter().map(|layer| layer.color).collect()
    }

    fn disabled(checked: Option<Checked>) -> State {
        State {
            checked,
            disabled: true,
            accent: Some(Rgba::new(0.0, 0.5, 0.0, 1.0)),
        }
    }

    #[test]
    fn an_unchecked_checkbox_is_white_inside_a_grey_edge() {
        let live = artwork(
            Toggle::CheckBox,
            live(Some(Checked::No)),
            Rect::new(4.0, 3.0, 13.0, 13.0),
        );
        assert_eq!(colors(&live), vec![Rgba::WHITE, EDGE]);
        // The edge is the whole box, and the opaque edge hides an inside
        // that reaches under it.
        let inside = live.first().map(bounds).unwrap_or_default();
        assert!(close(inside.0, 4.0) && close(inside.1, 3.0));
        assert!(close(inside.2, 17.0) && close(inside.3, 16.0));
        let edge = live.get(1).map(bounds).unwrap_or_default();
        assert!(close(edge.0, 4.0) && close(edge.2, 17.0));
        // A disabled one's inside stops a fifth of a pixel short of the
        // box, and its translucent edge is drawn over it.
        let dead = artwork(
            Toggle::CheckBox,
            disabled(Some(Checked::No)),
            Rect::new(4.0, 3.0, 13.0, 13.0),
        );
        assert_eq!(colors(&dead), vec![DISABLED_INSIDE, DISABLED_EDGE]);
        let inside = dead.first().map(bounds).unwrap_or_default();
        assert!(close(inside.0, 4.2) && close(inside.1, 3.2), "{inside:?}");
        assert!(close(inside.2, 16.8) && close(inside.3, 15.8), "{inside:?}");
        let edge = dead.get(1).map(bounds).unwrap_or_default();
        assert!(close(edge.0, 4.0) && close(edge.2, 17.0));
    }

    #[test]
    fn a_checked_checkbox_is_filled_with_the_accent_and_ticked_in_white() {
        let checked = artwork(
            Toggle::CheckBox,
            live(Some(Checked::Yes)),
            Rect::new(0.0, 0.0, 13.0, 13.0),
        );
        assert_eq!(colors(&checked), vec![control::DEFAULT_ACCENT, Rgba::WHITE]);
    }

    #[test]
    fn a_checked_radio_is_an_accent_edge_a_white_ring_and_an_accent_dot() {
        let radio = artwork(
            Toggle::Radio,
            live(Some(Checked::Yes)),
            Rect::new(0.0, 0.0, 20.0, 20.0),
        );
        let accent = control::DEFAULT_ACCENT;
        assert_eq!(colors(&radio), vec![Rgba::WHITE, accent, accent]);
        // Chrome's dot is 12 across at 20: from 4 to 16.
        let dot = radio.get(2).map(bounds).unwrap_or_default();
        assert!(close(dot.0, 4.0) && close(dot.2, 16.0), "{dot:?}");
    }

    #[test]
    fn a_radio_in_the_mixed_state_is_drawn_unchecked() {
        let mixed = artwork(
            Toggle::Radio,
            live(Some(Checked::Mixed)),
            Rect::new(0.0, 0.0, 13.0, 13.0),
        );
        assert_eq!(colors(&mixed), vec![Rgba::WHITE, EDGE]);
    }

    #[test]
    fn a_disabled_toggle_draws_its_state_in_its_edge_grey_whatever_the_accent() {
        // A checked checkbox: the inside, then its fill in the edge's grey
        // over it, rounded as a live one's is, then the tick.
        let checkbox = artwork(
            Toggle::CheckBox,
            disabled(Some(Checked::Yes)),
            Rect::new(0.0, 0.0, 13.0, 13.0),
        );
        assert_eq!(
            colors(&checkbox),
            vec![DISABLED_INSIDE, DISABLED_EDGE, DISABLED_MARK]
        );
        let fill = checkbox.get(1).map(bounds).unwrap_or_default();
        assert!(close(fill.0, 0.0) && close(fill.2, 13.0), "{fill:?}");
        // A radio: the inside, the edge, and the dot in the edge's grey.
        let radio = artwork(
            Toggle::Radio,
            disabled(Some(Checked::Yes)),
            Rect::new(0.0, 0.0, 20.0, 20.0),
        );
        assert_eq!(
            colors(&radio),
            vec![DISABLED_INSIDE, DISABLED_EDGE, DISABLED_EDGE]
        );
        let inside = radio.first().map(bounds).unwrap_or_default();
        assert!(close(inside.0, 0.2) && close(inside.2, 19.8), "{inside:?}");
        let dot = radio.get(2).map(bounds).unwrap_or_default();
        assert!(close(dot.0, 4.0) && close(dot.2, 16.0), "{dot:?}");
    }

    #[test]
    fn a_disabled_edge_over_its_inside_reads_as_chromes_fill() {
        // Where the edge crosses the inside, and wherever a checked control
        // is filled, Chrome reads 152 over black and 209 over white: the
        // edge's grey composited over the inside's, source over.
        let both = DISABLED_EDGE.over(DISABLED_INSIDE);
        for (page, want) in [(Rgba::BLACK, 152.0), (Rgba::WHITE, 209.0)] {
            let got = both.over(page).red * 255.0;
            assert!((got - want).abs() < 0.8, "{got} against {want}");
        }
        // And the tick over that reads 214 over black and 237 over white.
        for (page, want) in [(Rgba::BLACK, 214.0), (Rgba::WHITE, 237.0)] {
            let got = DISABLED_MARK.over(both).over(page).red * 255.0;
            assert!((got - want).abs() < 0.8, "{got} against {want}");
        }
    }

    #[test]
    fn a_disabled_grey_reads_as_measured_over_black_and_over_white() {
        // The readings each constant was made from, recovered to the unit
        // Chrome rounds them to: what a grey makes over black is its colour
        // times its alpha, and over white that plus the white it lets
        // through.
        for (color, black, white) in [
            (DISABLED_EDGE, 36.0, 214.0),
            (DISABLED_INSIDE, 167.0, 248.0),
        ] {
            let over_black = color.red * color.alpha * 255.0;
            let over_white = over_black + 255.0 * (1.0 - color.alpha);
            assert!((over_black - black).abs() < 0.5, "{color:?}");
            assert!((over_white - white).abs() < 0.5, "{color:?}");
        }
    }

    #[test]
    fn a_wide_toggle_is_a_square_in_its_middle() {
        // Chrome draws a checkbox 30 by 13 as a 13-pixel square from 8.5.
        let wide = artwork(
            Toggle::CheckBox,
            live(Some(Checked::No)),
            Rect::new(0.0, 0.0, 30.0, 13.0),
        );
        let edge = wide.get(1).map(bounds).unwrap_or_default();
        assert!(close(edge.0, 8.5) && close(edge.2, 21.5), "{edge:?}");
    }

    #[test]
    fn a_page_accent_colours_a_live_checked_control() {
        let green = Rgba::new(0.0, 0.5, 0.0, 1.0);
        let state = State {
            checked: Some(Checked::Yes),
            disabled: false,
            accent: Some(green),
        };
        let checked = artwork(Toggle::CheckBox, state, Rect::new(0.0, 0.0, 13.0, 13.0));
        assert_eq!(checked.first().map(|layer| layer.color), Some(green));
    }
}
