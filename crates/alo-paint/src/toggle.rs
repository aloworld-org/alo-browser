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
//! - **disabled**, the same shapes in translucent greys, whatever the
//!   page's `accent-color` says.
//!
//! The shape is the largest square that fits in the box, centred
//! ([`crate::control`]'s field): a checkbox thirty pixels wide is a
//! thirteen-pixel square in its middle, as in Chrome.
//!
//! # Measured, not read
//!
//! Every number here was measured from Chrome 154's screenshots, not read
//! from its source: shapes at eight device pixels to one, fitted by
//! coverage at 13 and 20 pixels square; colours at one to one. The disabled
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
use crate::corner::{Corners, between, ring, rounded_rectangle};
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

/// A disabled control's edge: 123 over black and 209 over white, so 169/255
/// opaque and 123/169 bright.
pub const DISABLED_EDGE: Rgba = grey(123.0, 209.0);

/// A disabled control's inside: 167 over black, 248 over white.
pub const DISABLED_INSIDE: Rgba = grey(167.0, 248.0);

/// A disabled control's fill when checked, and a disabled radio's dot: 152
/// over black, 209 over white. A page's `accent-color` does not change it.
pub const DISABLED_FILL: Rgba = grey(152.0, 209.0);

/// A disabled checkbox's tick, drawn over [`DISABLED_FILL`]: white at the
/// opacity that turns that fill's 152 over black into the tick's 214 —
/// 62/103. Over white the two make 237, which is what Chrome draws there.
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
pub fn artwork(toggle: Toggle, state: State, border_box: Rect) -> Vec<Layer> {
    let field = control::field(border_box);
    let corners = match toggle {
        Toggle::CheckBox => Corners::all(CORNER + EDGE_WIDTH / 2.0),
        // A circle: a rounded square whose radii are half its side, fitted to
        // whatever is left inside the edge too.
        Toggle::Radio => Corners::all(field.size.width / 2.0),
    };
    let edge_widths = Edges::all(EDGE_WIDTH.min(field.size.width / 2.0));
    let edge = ring(field, corners, edge_widths);
    let palette = Palette::of(state);
    // Each shape is drawn over the page, never over another, wherever it is
    // translucent: Chrome's disabled edge, inside, fill and dot each read the
    // same over black whatever lies beside them, so none of them lies on
    // another. Where the edge is opaque the inside reaches under it, where
    // it cannot be seen, so that the pixels the two share are the edge over
    // the inside: two shapes that meet along a curve, each smoothed on its
    // own, would let the page show through the seam between them.
    let opaque = palette.edge.alpha >= 1.0 && palette.checked_edge.alpha >= 1.0;
    let within_edge =
        || rounded_rectangle(field.shrunk_by(edge_widths), corners.inside(edge_widths));
    let inside = || {
        if opaque {
            rounded_rectangle(field, corners)
        } else {
            within_edge()
        }
    };
    let mark = match toggle {
        Toggle::CheckBox => match state.checked {
            Some(Checked::Yes) => Some(Mark::Tick),
            Some(Checked::Mixed) => Some(Mark::Dash),
            _ => None,
        },
        Toggle::Radio => (state.checked == Some(Checked::Yes)).then_some(Mark::Dot),
    };
    let layer = |path: Path, color: Rgba| Layer { path, color };
    match (toggle, mark) {
        // Unchecked, or a radio in the mixed state, which ARIA says is to be
        // treated as unchecked.
        (_, None) => vec![layer(inside(), palette.inside), layer(edge, palette.edge)],
        // A checked radio keeps its inside: the accent is its edge and its
        // dot, with a ring of white between.
        (Toggle::Radio, Some(mark)) => {
            let dot = control::mark(mark, field);
            let ring_between = if opaque {
                inside()
            } else {
                between(&within_edge(), &dot)
            };
            vec![
                layer(ring_between, palette.inside),
                layer(edge, palette.checked_edge),
                layer(dot, palette.fill),
            ]
        }
        // A live checked or mixed checkbox is filled edge to edge, rounded
        // to [`CORNER`], and the mark drawn across it.
        (Toggle::CheckBox, Some(mark)) if opaque => vec![
            layer(rounded_rectangle(field, Corners::all(CORNER)), palette.fill),
            layer(control::mark(mark, field), palette.mark),
        ],
        // A disabled one keeps its grey edge, and is filled inside it.
        (Toggle::CheckBox, Some(mark)) => vec![
            layer(within_edge(), palette.fill),
            layer(edge, palette.checked_edge),
            layer(control::mark(mark, field), palette.mark),
        ],
    }
}

/// The colours one toggle is drawn in.
struct Palette {
    inside: Rgba,
    edge: Rgba,
    /// A checked radio's edge.
    checked_edge: Rgba,
    /// A checked checkbox's fill, and a checked radio's dot.
    fill: Rgba,
    /// The tick or the bar across a checked checkbox's fill.
    mark: Rgba,
}

impl Palette {
    fn of(state: State) -> Self {
        if state.disabled {
            return Self {
                inside: DISABLED_INSIDE,
                edge: DISABLED_EDGE,
                checked_edge: DISABLED_EDGE,
                fill: DISABLED_FILL,
                mark: DISABLED_MARK,
            };
        }
        let accent = state.accent.unwrap_or(control::DEFAULT_ACCENT);
        Self {
            inside: INSIDE,
            edge: EDGE,
            checked_edge: accent,
            fill: accent,
            mark: control::mark_color(accent),
        }
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

    #[test]
    fn an_unchecked_checkbox_is_white_inside_a_grey_edge() {
        let layers = artwork(
            Toggle::CheckBox,
            live(Some(Checked::No)),
            Rect::new(4.0, 3.0, 13.0, 13.0),
        );
        assert_eq!(layers.len(), 2);
        assert_eq!(layers.first().map(|layer| layer.color), Some(Rgba::WHITE));
        assert_eq!(layers.get(1).map(|layer| layer.color), Some(EDGE));
        // The edge is the whole box, and the opaque edge hides an inside
        // that reaches under it.
        let inside = layers.first().map(bounds).unwrap_or_default();
        assert!(close(inside.0, 4.0) && close(inside.1, 3.0));
        assert!(close(inside.2, 17.0) && close(inside.3, 16.0));
        let edge = layers.get(1).map(bounds).unwrap_or_default();
        assert!(close(edge.0, 4.0) && close(edge.2, 17.0));
        // A translucent edge would show it, so a disabled inside stops one
        // pixel in.
        let disabled = State {
            checked: Some(Checked::No),
            disabled: true,
            accent: None,
        };
        let layers = artwork(Toggle::CheckBox, disabled, Rect::new(4.0, 3.0, 13.0, 13.0));
        let inside = layers.first().map(bounds).unwrap_or_default();
        assert!(close(inside.0, 5.0) && close(inside.1, 4.0));
        assert!(close(inside.2, 16.0) && close(inside.3, 15.0));
    }

    #[test]
    fn a_checked_checkbox_is_filled_with_the_accent_and_ticked_in_white() {
        let layers = artwork(
            Toggle::CheckBox,
            live(Some(Checked::Yes)),
            Rect::new(0.0, 0.0, 13.0, 13.0),
        );
        let colors: Vec<Rgba> = layers.iter().map(|layer| layer.color).collect();
        assert_eq!(colors, vec![control::DEFAULT_ACCENT, Rgba::WHITE]);
    }

    #[test]
    fn a_checked_radio_is_an_accent_edge_a_white_ring_and_an_accent_dot() {
        let layers = artwork(
            Toggle::Radio,
            live(Some(Checked::Yes)),
            Rect::new(0.0, 0.0, 20.0, 20.0),
        );
        let colors: Vec<Rgba> = layers.iter().map(|layer| layer.color).collect();
        let accent = control::DEFAULT_ACCENT;
        assert_eq!(colors, vec![Rgba::WHITE, accent, accent]);
        // Chrome's dot is 12 across at 20: from 4 to 16.
        let dot = layers.get(2).map(bounds).unwrap_or_default();
        assert!(close(dot.0, 4.0) && close(dot.2, 16.0), "{dot:?}");
    }

    #[test]
    fn a_radio_in_the_mixed_state_is_drawn_unchecked() {
        let mixed = artwork(
            Toggle::Radio,
            live(Some(Checked::Mixed)),
            Rect::new(0.0, 0.0, 13.0, 13.0),
        );
        let colors: Vec<Rgba> = mixed.iter().map(|layer| layer.color).collect();
        assert_eq!(colors, vec![Rgba::WHITE, EDGE]);
    }

    #[test]
    fn a_disabled_toggle_is_translucent_grey_whatever_the_accent() {
        let state = State {
            checked: Some(Checked::Yes),
            disabled: true,
            accent: Some(Rgba::new(0.0, 0.5, 0.0, 1.0)),
        };
        let layers = artwork(Toggle::CheckBox, state, Rect::new(0.0, 0.0, 13.0, 13.0));
        let colors: Vec<Rgba> = layers.iter().map(|layer| layer.color).collect();
        // Its grey edge stays, around the fill rather than under it.
        assert_eq!(colors, vec![DISABLED_FILL, DISABLED_EDGE, DISABLED_MARK]);
        let fill = layers.first().map(bounds).unwrap_or_default();
        assert!(close(fill.0, 1.0) && close(fill.2, 12.0), "{fill:?}");
        // A radio's dot is over the page, not over its inside: the inside
        // is a ring around it.
        let radio = artwork(Toggle::Radio, state, Rect::new(0.0, 0.0, 13.0, 13.0));
        let colors: Vec<Rgba> = radio.iter().map(|layer| layer.color).collect();
        assert_eq!(colors, vec![DISABLED_INSIDE, DISABLED_EDGE, DISABLED_FILL]);
        let holes = radio.first().map_or(0, |layer| {
            layer
                .path
                .segments()
                .iter()
                .filter(|segment| matches!(segment, crate::path::Segment::MoveTo(_)))
                .count()
        });
        assert_eq!(holes, 2, "the inside is cut around the dot");
    }

    #[test]
    fn a_disabled_grey_reads_as_measured_over_black_and_over_white() {
        // The readings each constant was made from, recovered: what it
        // makes over black is its colour times its alpha, and over white
        // that plus the white it lets through.
        for (color, black, white) in [
            (DISABLED_EDGE, 123.0, 209.0),
            (DISABLED_INSIDE, 167.0, 248.0),
            (DISABLED_FILL, 152.0, 209.0),
        ] {
            let over_black = color.red * color.alpha * 255.0;
            let over_white = over_black + 255.0 * (1.0 - color.alpha);
            assert!((over_black - black).abs() < 0.01, "{color:?}");
            assert!((over_white - white).abs() < 0.01, "{color:?}");
        }
    }

    #[test]
    fn a_wide_toggle_is_a_square_in_its_middle() {
        // Chrome draws a checkbox 30 by 13 as a 13-pixel square from 8.5.
        let layers = artwork(
            Toggle::CheckBox,
            live(Some(Checked::No)),
            Rect::new(0.0, 0.0, 30.0, 13.0),
        );
        let edge = layers.get(1).map(bounds).unwrap_or_default();
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
        let layers = artwork(Toggle::CheckBox, state, Rect::new(0.0, 0.0, 13.0, 13.0));
        assert_eq!(layers.first().map(|layer| layer.color), Some(green));
    }
}
