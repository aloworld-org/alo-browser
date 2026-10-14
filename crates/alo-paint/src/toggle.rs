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

/// A toggle's picture: the shapes drawn as one, then the shapes over them.
#[derive(Debug, Clone)]
pub struct Artwork {
    /// The edge and what it holds, back to front, drawn on a surface of
    /// their own and composited onto the page once, at [`Self::opacity`].
    pub joined: Vec<Layer>,
    /// How much of [`Self::joined`] reaches the page: one for a live
    /// control, whose shapes are opaque and need no surface of their own.
    pub opacity: f32,
    /// What is drawn over that, each shape on its own: a tick, a bar, or a
    /// disabled radio's dot.
    pub over: Vec<Layer>,
}

impl Artwork {
    /// Every shape, back to front, whatever surface it is drawn on.
    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        self.joined.iter().chain(&self.over)
    }
}

/// The picture a toggle draws of itself in `border_box`.
///
/// # Where two pieces meet
///
/// A disabled control's pieces are translucent, and Chrome's seam between
/// two of them is neither piece and not the page: across it the picture
/// moves from one piece's reading to the other's in proportion to how much
/// of the pixel each covers. Measured at eight device pixels to one over
/// black and over white (queue item 463), the seam's opacity lies between
/// the edge's and the inside's everywhere and dips below neither. Two
/// translucent shapes drawn side by side, each smoothed on its own, would
/// each let the page through the half pixel the other covers, and a seam
/// over black showed up to 44 of 255 of it.
///
/// So the outer piece is drawn under the whole shape and the inner one over
/// it, opaque, on a surface faded once to the inner piece's opacity: where
/// the inner piece covers part of a pixel the result is the two readings
/// mixed by that part, and at the outer edge it is the outer piece alone.
/// The outer piece's own colour there is its reading divided by that
/// opacity, which is a colour only while the inner piece is the more
/// opaque — true of a disabled edge (169/255) under a disabled inside
/// (174/255) or fill (198/255).
///
/// A disabled radio's dot is the more opaque of it and its inside, so it
/// cannot join them. It is drawn over the inside in the colour that, over
/// the inside, reads as Chrome's dot does ([`over_to_read`]), which mixes
/// across its seam the same way.
pub fn artwork(toggle: Toggle, state: State, border_box: Rect) -> Artwork {
    let field = control::field(border_box);
    let corners = match toggle {
        Toggle::CheckBox => Corners::all(CORNER + EDGE_WIDTH / 2.0),
        // A circle: a rounded square whose radii are half its side, fitted to
        // whatever is left inside the edge too.
        Toggle::Radio => Corners::all(field.size.width / 2.0),
    };
    let edge_widths = Edges::all(EDGE_WIDTH.min(field.size.width / 2.0));
    let whole = || rounded_rectangle(field, corners);
    let within_edge =
        || rounded_rectangle(field.shrunk_by(edge_widths), corners.inside(edge_widths));
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
        // The piece inside the edge: a checked checkbox's fill, anything
        // else's inside, which a checked radio's dot is drawn over.
        let inner = match (toggle, mark) {
            (Toggle::CheckBox, Some(_)) => DISABLED_FILL,
            _ => DISABLED_INSIDE,
        };
        let opacity = inner.alpha;
        let over = match (toggle, mark) {
            (_, None) => Vec::new(),
            (Toggle::Radio, Some(mark)) => over_to_read(DISABLED_FILL, DISABLED_INSIDE)
                .map(|dot| vec![layer(control::mark(mark, field), dot)])
                .unwrap_or_default(),
            (Toggle::CheckBox, Some(mark)) => {
                vec![layer(control::mark(mark, field), DISABLED_MARK)]
            }
        };
        return Artwork {
            joined: vec![
                layer(whole(), within(DISABLED_EDGE, opacity)),
                layer(within_edge(), within(inner, opacity)),
            ],
            opacity,
            over,
        };
    }
    // A live control's shapes are opaque, so each is drawn over the last:
    // the inside reaches under the edge, where it cannot be seen, and the
    // pixels the two share are the edge over the inside, with no seam.
    let accent = state.accent.unwrap_or(control::DEFAULT_ACCENT);
    let edge = ring(field, corners, edge_widths);
    let joined = match (toggle, mark) {
        // Unchecked, or a radio in the mixed state, which ARIA says is to be
        // treated as unchecked.
        (_, None) => vec![layer(whole(), INSIDE), layer(edge, EDGE)],
        // A checked radio keeps its inside: the accent is its edge and its
        // dot, with a ring of white between.
        (Toggle::Radio, Some(mark)) => vec![
            layer(whole(), INSIDE),
            layer(edge, accent),
            layer(control::mark(mark, field), accent),
        ],
        // A checked or mixed checkbox is filled edge to edge, rounded to
        // [`CORNER`], and the mark drawn across it.
        (Toggle::CheckBox, Some(mark)) => vec![
            layer(rounded_rectangle(field, Corners::all(CORNER)), accent),
            layer(control::mark(mark, field), control::mark_color(accent)),
        ],
    };
    Artwork {
        joined,
        opacity: 1.0,
        over: Vec::new(),
    }
}

/// A translucent colour as it is drawn on a surface that is then faded to
/// `opacity`, so that it reaches the page as `color` does.
fn within(color: Rgba, opacity: f32) -> Rgba {
    Rgba {
        alpha: (color.alpha / opacity).min(1.0),
        ..color
    }
}

/// The colour that, drawn over `under`, reads as `target` over any page —
/// or [`None`] when no colour does, because `target` lets through more of
/// the page than `under` already does, or is a grey `under` cannot be
/// turned into.
///
/// Over a page `p`, `target` reads `t + (1 − a)·p` with `t` its colour
/// times its alpha `a`. Drawn over `under`, a colour of alpha `x` lets
/// through `(1 − x)(1 − under's alpha)` of the page, which fixes `x`; what
/// `under` still shows then fixes the colour.
pub fn over_to_read(target: Rgba, under: Rgba) -> Option<Rgba> {
    let through = 1.0 - under.alpha;
    if through <= 0.0 {
        return None;
    }
    let alpha = 1.0 - (1.0 - target.alpha) / through;
    if alpha <= 0.0 || alpha > 1.0 {
        return None;
    }
    let channel = |want: f32, below: f32| {
        let level = (want * target.alpha - (1.0 - alpha) * below * under.alpha) / alpha;
        (0.0..=1.0).contains(&level).then_some(level)
    };
    Some(Rgba {
        red: channel(target.red, under.red)?,
        green: channel(target.green, under.green)?,
        blue: channel(target.blue, under.blue)?,
        alpha,
    })
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

    fn colors(artwork: &Artwork) -> Vec<Rgba> {
        artwork.layers().map(|layer| layer.color).collect()
    }

    #[test]
    fn an_unchecked_checkbox_is_white_inside_a_grey_edge() {
        let live = artwork(
            Toggle::CheckBox,
            live(Some(Checked::No)),
            Rect::new(4.0, 3.0, 13.0, 13.0),
        );
        assert_eq!(colors(&live), vec![Rgba::WHITE, EDGE]);
        assert!(close(live.opacity, 1.0) && live.over.is_empty());
        // The edge is the whole box, and the opaque edge hides an inside
        // that reaches under it.
        let inside = live.joined.first().map(bounds).unwrap_or_default();
        assert!(close(inside.0, 4.0) && close(inside.1, 3.0));
        assert!(close(inside.2, 17.0) && close(inside.3, 16.0));
        let edge = live.joined.get(1).map(bounds).unwrap_or_default();
        assert!(close(edge.0, 4.0) && close(edge.2, 17.0));
        // A disabled one turns that over: the edge's grey is the whole box,
        // and the inside, opaque on a surface faded to its own opacity,
        // stops one pixel in.
        let disabled = State {
            checked: Some(Checked::No),
            disabled: true,
            accent: None,
        };
        let dead = artwork(Toggle::CheckBox, disabled, Rect::new(4.0, 3.0, 13.0, 13.0));
        assert!(close(dead.opacity, DISABLED_INSIDE.alpha));
        let edge = dead.joined.first().map(bounds).unwrap_or_default();
        assert!(close(edge.0, 4.0) && close(edge.2, 17.0));
        let inside = dead.joined.get(1).map(bounds).unwrap_or_default();
        assert!(close(inside.0, 5.0) && close(inside.1, 4.0));
        assert!(close(inside.2, 16.0) && close(inside.3, 15.0));
        assert_eq!(dead.joined.get(1).map(|layer| layer.color.alpha), Some(1.0));
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
        let dot = radio.joined.get(2).map(bounds).unwrap_or_default();
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
    fn a_disabled_toggle_is_translucent_grey_whatever_the_accent() {
        let state = State {
            checked: Some(Checked::Yes),
            disabled: true,
            accent: Some(Rgba::new(0.0, 0.5, 0.0, 1.0)),
        };
        let checkbox = artwork(Toggle::CheckBox, state, Rect::new(0.0, 0.0, 13.0, 13.0));
        // Its grey edge stays, under the fill, and both reach the page faded
        // to the fill's opacity; the tick is drawn over them.
        assert!(close(checkbox.opacity, DISABLED_FILL.alpha));
        let joined: Vec<Rgba> = checkbox.joined.iter().map(|layer| layer.color).collect();
        assert_eq!(
            joined,
            vec![
                within(DISABLED_EDGE, DISABLED_FILL.alpha),
                within(DISABLED_FILL, DISABLED_FILL.alpha)
            ]
        );
        let fill = checkbox.joined.get(1).map(bounds).unwrap_or_default();
        assert!(close(fill.0, 1.0) && close(fill.2, 12.0), "{fill:?}");
        let over: Vec<Rgba> = checkbox.over.iter().map(|layer| layer.color).collect();
        assert_eq!(over, vec![DISABLED_MARK]);
        // A radio's inside is whole, and its dot is drawn over it in the
        // colour that reads there as Chrome's dot does.
        let radio = artwork(Toggle::Radio, state, Rect::new(0.0, 0.0, 13.0, 13.0));
        assert!(close(radio.opacity, DISABLED_INSIDE.alpha));
        assert_eq!(radio.joined.len(), 2);
        let dot = radio.over.first().map(|layer| layer.color);
        assert_eq!(dot, over_to_read(DISABLED_FILL, DISABLED_INSIDE));
        assert!(dot.is_some());
    }

    #[test]
    fn a_joined_picture_reaches_the_page_as_each_piece_reads() {
        // The edge and the inside, each drawn on a transparent surface faded
        // to the joined opacity, over black and over white: what each was
        // measured as.
        for (inner, opacity) in [
            (DISABLED_INSIDE, DISABLED_INSIDE.alpha),
            (DISABLED_FILL, DISABLED_FILL.alpha),
        ] {
            for (piece, measured) in [(DISABLED_EDGE, DISABLED_EDGE), (inner, inner)] {
                let drawn = within(piece, opacity);
                assert!(drawn.alpha <= 1.0);
                for page in [Rgba::BLACK, Rgba::WHITE] {
                    let faded = Rgba {
                        alpha: drawn.alpha * opacity,
                        ..drawn
                    };
                    let got = faded.over(page);
                    let want = measured.over(page);
                    assert!(close(got.red, want.red), "{piece:?} over {page:?}");
                }
            }
        }
    }

    #[test]
    fn a_colour_drawn_over_another_reads_as_asked() {
        let Some(dot) = over_to_read(DISABLED_FILL, DISABLED_INSIDE) else {
            panic!("a disabled dot can be drawn over a disabled inside");
        };
        for page in [Rgba::BLACK, Rgba::WHITE, Rgba::new(0.5, 0.5, 0.5, 1.0)] {
            let got = dot.over(DISABLED_INSIDE.over(page));
            let want = DISABLED_FILL.over(page);
            assert!(close(got.red, want.red), "over {page:?}: {got:?} {want:?}");
        }
        // Nothing drawn over a colour lets more of the page through it.
        assert_eq!(over_to_read(DISABLED_EDGE, DISABLED_INSIDE), None);
        // And the inside cannot be drawn over a whole edge, which is why the
        // two are joined: the grey it would take is brighter than white.
        assert_eq!(over_to_read(DISABLED_INSIDE, DISABLED_EDGE), None);
        assert_eq!(over_to_read(Rgba::WHITE, Rgba::WHITE), None);
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
        let wide = artwork(
            Toggle::CheckBox,
            live(Some(Checked::No)),
            Rect::new(0.0, 0.0, 30.0, 13.0),
        );
        let edge = wide.joined.get(1).map(bounds).unwrap_or_default();
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
        assert_eq!(checked.joined.first().map(|layer| layer.color), Some(green));
    }
}
