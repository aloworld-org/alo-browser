/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Gradients: a colour that changes across a shape.
//!
//! A flat colour is one number per channel; a gradient is a *function* of
//! position, so it is carried as the stops the author wrote and evaluated
//! wherever a pixel needs it. That is what keeps the same value usable for a
//! background four hundred pixels wide and a chip forty pixels wide without
//! being reparsed.
//!
//! # What is here
//!
//! `linear-gradient` and `radial-gradient`, with colour stops that may or may
//! not say where they sit. Stops without a position are spread evenly between
//! the ones that have one, which is the rule CSS gives and the reason
//! `linear-gradient(red, blue)` works at all.
//!
//! A radial gradient is a `circle` or an `ellipse`, sized by one of the four
//! extent keywords and centred `at` a position of one or two parts, each a
//! keyword, a percentage or a pixel length. That is what alo's own sheets
//! write: Meet's tint is `radial-gradient(circle at 92% 0, …)`.
//!
//! Two colours are mixed with their alpha **premultiplied**, which is what CSS
//! says. `transparent` is transparent *black*, and mixed the other way a fade
//! to it passes through grey.
//!
//! Refused, rather than approximated: `conic-gradient`, the repeating forms,
//! colour interpolation hints, interpolation in any space but sRGB, a radial
//! size written as lengths, and a position of three or four parts. Each is a
//! different curve through colour or a different place, and drawing one as
//! another is a wrong pixel that looks nearly right.

use crate::color::{Color, Rgba};
use core::fmt;

/// One colour, and where it sits along the gradient.
#[derive(Debug, Clone, PartialEq)]
pub struct Stop {
    /// The colour.
    pub color: Color,
    /// Where it sits, from zero to one, or [`None`] when the author left it to
    /// be spread evenly.
    pub position: Option<f32>,
}

/// Which way a linear gradient runs, as an angle in degrees clockwise from
/// upwards — which is how CSS measures it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Angle(pub f32);

impl Angle {
    /// `to bottom`, which is what a gradient does when nobody says otherwise.
    pub const DOWN: Angle = Angle(180.0);

    /// The angle a `to <side>` phrase means.
    pub(crate) fn from_sides(text: &str) -> Option<Self> {
        let mut to_top = false;
        let mut to_bottom = false;
        let mut to_left = false;
        let mut to_right = false;
        let mut words = 0;
        for word in text.split_ascii_whitespace() {
            words += 1;
            match () {
                () if word.eq_ignore_ascii_case("to") && words == 1 => {}
                () if word.eq_ignore_ascii_case("top") => to_top = true,
                () if word.eq_ignore_ascii_case("bottom") => to_bottom = true,
                () if word.eq_ignore_ascii_case("left") => to_left = true,
                () if word.eq_ignore_ascii_case("right") => to_right = true,
                () => return None,
            }
        }
        let degrees = match (to_top, to_right, to_bottom, to_left) {
            (true, false, false, false) => 0.0,
            (true, true, false, false) => 45.0,
            (false, true, false, false) => 90.0,
            (false, true, true, false) => 135.0,
            (false, false, true, false) => 180.0,
            (false, false, true, true) => 225.0,
            (false, false, false, true) => 270.0,
            (true, false, false, true) => 315.0,
            _ => return None,
        };
        Some(Angle(degrees))
    }
}

/// What shape a radial gradient's rings are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// Round, whatever the box.
    Circle,
    /// Stretched to the box's proportions, which is what CSS does when
    /// nobody says.
    Ellipse,
}

/// Where a radial gradient's last ring is: through which side or corner of
/// the box, measured from its centre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extent {
    /// The nearest side.
    ClosestSide,
    /// The nearest corner.
    ClosestCorner,
    /// The farthest side.
    FarthestSide,
    /// The farthest corner, which is what CSS does when nobody says.
    FarthestCorner,
}

impl Extent {
    /// The keyword, as CSS spells it.
    pub fn as_str(self) -> &'static str {
        match self {
            Extent::ClosestSide => "closest-side",
            Extent::ClosestCorner => "closest-corner",
            Extent::FarthestSide => "farthest-side",
            Extent::FarthestCorner => "farthest-corner",
        }
    }

    /// The keyword's meaning, if it is one of the four.
    pub fn parse(word: &str) -> Option<Self> {
        [
            Extent::ClosestSide,
            Extent::ClosestCorner,
            Extent::FarthestSide,
            Extent::FarthestCorner,
        ]
        .into_iter()
        .find(|extent| word.eq_ignore_ascii_case(extent.as_str()))
    }
}

/// How far along one side of a box a point is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Offset {
    /// A share of the box's size, from zero to one; `left` and `top` are
    /// zero, `center` a half, `right` and `bottom` one.
    Fraction(f32),
    /// A distance from the box's left or top edge.
    Pixels(f32),
}

impl Offset {
    /// The distance from the start of a side `size` long.
    pub fn along(self, size: f32) -> f32 {
        match self {
            Offset::Fraction(fraction) => fraction * size,
            Offset::Pixels(pixels) => pixels,
        }
    }
}

impl fmt::Display for Offset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Offset::Fraction(fraction) => write!(f, "{}%", fraction * 100.0),
            Offset::Pixels(pixels) => write!(f, "{pixels}px"),
        }
    }
}

/// Where in a box a radial gradient is centred.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    /// Across, from the left.
    pub x: Offset,
    /// Down, from the top.
    pub y: Offset,
}

impl Position {
    /// The middle of the box, which is where a gradient is centred when
    /// nobody says.
    pub const CENTRE: Position = Position {
        x: Offset::Fraction(0.5),
        y: Offset::Fraction(0.5),
    };
}

/// A colour that changes across a shape.
#[derive(Debug, Clone, PartialEq)]
pub enum Gradient {
    /// Along a line.
    Linear {
        /// Which way it runs.
        angle: Angle,
        /// The colours along it.
        stops: Vec<Stop>,
    },
    /// Outwards from a point.
    Radial {
        /// Round or stretched.
        shape: Shape,
        /// Where the last ring is.
        extent: Extent,
        /// The point.
        centre: Position,
        /// The colours from the middle outwards.
        stops: Vec<Stop>,
    },
}

impl Gradient {
    /// The colours along it.
    pub fn stops(&self) -> &[Stop] {
        match self {
            Gradient::Linear { stops, .. } | Gradient::Radial { stops, .. } => stops,
        }
    }

    /// The colour at a point along the gradient, from zero to one.
    ///
    /// `current` is what `currentColor` means here, which a stop may be.
    pub fn at(&self, along: f32, current: Rgba) -> Rgba {
        let spread = self.spread_stops();
        let Some(first) = spread.first() else {
            return Rgba::TRANSPARENT;
        };
        let along = along.clamp(0.0, 1.0);
        if along <= first.0 {
            return first.1.resolve(current);
        }
        for pair in spread.windows(2) {
            let (Some((from, from_color)), Some((to, to_color))) = (pair.first(), pair.get(1))
            else {
                continue;
            };
            if along > *to {
                continue;
            }
            let span = to - from;
            // Two stops at the same place are a hard edge, which is how a
            // stripe is drawn.
            let fraction = if span <= 0.0 {
                1.0
            } else {
                (along - from) / span
            };
            return mix(
                from_color.resolve(current),
                to_color.resolve(current),
                fraction,
            );
        }
        spread
            .last()
            .map_or(Rgba::TRANSPARENT, |(_, color)| color.resolve(current))
    }

    /// The stops with every position filled in.
    ///
    /// A stop with no position of its own sits evenly between the nearest ones
    /// that have one — which is what makes `linear-gradient(red, blue)` a
    /// gradient rather than an error.
    fn spread_stops(&self) -> Vec<(f32, Color)> {
        let stops = self.stops();
        let mut placed: Vec<(Option<f32>, Color)> = stops
            .iter()
            .map(|stop| (stop.position, stop.color))
            .collect();
        if placed.is_empty() {
            return Vec::new();
        }
        // The ends are pinned first: a gradient starts at zero and ends at one
        // however few of its stops said so.
        if let Some(first) = placed.first_mut()
            && first.0.is_none()
        {
            first.0 = Some(0.0);
        }
        if let Some(last) = placed.last_mut()
            && last.0.is_none()
        {
            last.0 = Some(1.0);
        }

        let mut out: Vec<(f32, Color)> = Vec::with_capacity(placed.len());
        let mut index = 0;
        while index < placed.len() {
            let Some((position, color)) = placed.get(index).copied() else {
                break;
            };
            if let Some(position) = position {
                // A position never goes backwards, which is what CSS says to
                // do with `red 60%, blue 20%`.
                let held = out.last().map_or(0.0, |(held, _)| *held);
                out.push((position.max(held), color));
                index += 1;
                continue;
            }
            // A run with no positions: find where it ends and spread it.
            let start = out.last().map_or(0.0, |(held, _)| *held);
            let mut run = 0;
            while placed
                .get(index + run)
                .is_some_and(|(position, _)| position.is_none())
            {
                run += 1;
            }
            let end = placed
                .get(index + run)
                .and_then(|(position, _)| *position)
                .unwrap_or(1.0);
            #[expect(clippy::cast_precision_loss, reason = "a stop list is short")]
            let count = (run + 1) as f32;
            for step in 0..run {
                #[expect(clippy::cast_precision_loss, reason = "a stop list is short")]
                let at = start + (end - start) * ((step + 1) as f32) / count;
                if let Some((_, color)) = placed.get(index + step) {
                    out.push((at, *color));
                }
            }
            index += run;
        }
        out
    }
}

impl fmt::Display for Gradient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Gradient::Linear { angle, stops } => {
                write!(f, "linear-gradient({}deg", angle.0)?;
                write_stops(f, stops)
            }
            Gradient::Radial {
                shape,
                extent,
                centre,
                stops,
            } => {
                // What CSS does when nobody says is not written, so that a
                // display list of plain radial gradients reads as it always
                // did. Otherwise all of it is, so nothing is left to infer.
                f.write_str("radial-gradient(")?;
                let plain = *shape == Shape::Ellipse
                    && *extent == Extent::FarthestCorner
                    && *centre == Position::CENTRE;
                if !plain {
                    let shape = match shape {
                        Shape::Circle => "circle",
                        Shape::Ellipse => "ellipse",
                    };
                    write!(
                        f,
                        "{shape} {} at {} {}, ",
                        extent.as_str(),
                        centre.x,
                        centre.y
                    )?;
                }
                for (index, stop) in stops.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{}", stop.color)?;
                    if let Some(position) = stop.position {
                        write!(f, " {}%", position * 100.0)?;
                    }
                }
                f.write_str(")")
            }
        }
    }
}

fn write_stops(f: &mut fmt::Formatter<'_>, stops: &[Stop]) -> fmt::Result {
    for stop in stops {
        write!(f, ", {}", stop.color)?;
        if let Some(position) = stop.position {
            write!(f, " {}%", position * 100.0)?;
        }
    }
    f.write_str(")")
}

/// Two colours mixed, in sRGB, with their alpha premultiplied.
///
/// The space CSS uses when nobody says otherwise, and the one every stop here
/// is in: `linear-gradient(in oklab, …)` is refused rather than mixed in the
/// wrong space. Premultiplied because CSS says so, and because otherwise a
/// fade from a colour to `transparent` — which is transparent black — passes
/// through a grey that nobody wrote.
fn mix(from: Rgba, to: Rgba, fraction: f32) -> Rgba {
    let fraction = fraction.clamp(0.0, 1.0);
    let blend = |a: f32, b: f32| a + (b - a) * fraction;
    let alpha = blend(from.alpha, to.alpha);
    if alpha <= 0.0 {
        return Rgba::TRANSPARENT;
    }
    let channel = |a: f32, b: f32| blend(a * from.alpha, b * to.alpha) / alpha;
    Rgba::new(
        channel(from.red, to.red),
        channel(from.green, to.green),
        channel(from.blue, to.blue),
        alpha,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop(color: Rgba, position: Option<f32>) -> Stop {
        Stop {
            color: Color::Rgba(color),
            position,
        }
    }

    fn linear(stops: Vec<Stop>) -> Gradient {
        Gradient::Linear {
            angle: Angle::DOWN,
            stops,
        }
    }

    fn close(left: Rgba, right: Rgba) -> bool {
        (left.red - right.red).abs() < 0.01
            && (left.green - right.green).abs() < 0.01
            && (left.blue - right.blue).abs() < 0.01
    }

    #[test]
    fn two_stops_run_from_one_to_the_other() {
        let gradient = linear(vec![stop(Rgba::BLACK, None), stop(Rgba::WHITE, None)]);
        assert!(close(gradient.at(0.0, Rgba::BLACK), Rgba::BLACK));
        assert!(close(gradient.at(1.0, Rgba::BLACK), Rgba::WHITE));
        assert!(close(
            gradient.at(0.5, Rgba::BLACK),
            Rgba::new(0.5, 0.5, 0.5, 1.0),
        ));
    }

    #[test]
    fn stops_with_no_position_are_spread_evenly() {
        let gradient = linear(vec![
            stop(Rgba::BLACK, None),
            stop(Rgba::new(1.0, 0.0, 0.0, 1.0), None),
            stop(Rgba::WHITE, None),
        ]);
        assert!(
            close(gradient.at(0.5, Rgba::BLACK), Rgba::new(1.0, 0.0, 0.0, 1.0)),
            "the middle stop sits in the middle",
        );
    }

    #[test]
    fn a_stop_that_says_where_it_sits_sits_there() {
        let gradient = linear(vec![
            stop(Rgba::BLACK, Some(0.0)),
            stop(Rgba::WHITE, Some(0.25)),
        ]);
        assert!(close(gradient.at(0.25, Rgba::BLACK), Rgba::WHITE));
        assert!(
            close(gradient.at(0.5, Rgba::BLACK), Rgba::WHITE),
            "and past the last stop it stays that colour",
        );
    }

    #[test]
    fn two_stops_at_the_same_place_are_a_hard_edge() {
        let gradient = linear(vec![
            stop(Rgba::BLACK, Some(0.0)),
            stop(Rgba::BLACK, Some(0.5)),
            stop(Rgba::WHITE, Some(0.5)),
            stop(Rgba::WHITE, Some(1.0)),
        ]);
        assert!(close(gradient.at(0.49, Rgba::BLACK), Rgba::BLACK));
        assert!(close(gradient.at(0.51, Rgba::BLACK), Rgba::WHITE));
    }

    #[test]
    fn a_position_that_goes_backwards_is_pulled_forwards() {
        let gradient = linear(vec![
            stop(Rgba::BLACK, Some(0.6)),
            stop(Rgba::WHITE, Some(0.2)),
        ]);
        // CSS says a stop never precedes the one before it; the second becomes
        // 0.6 as well, which makes a hard edge rather than a reversal.
        assert!(close(gradient.at(0.5, Rgba::BLACK), Rgba::BLACK));
        assert!(close(gradient.at(0.7, Rgba::BLACK), Rgba::WHITE));
    }

    #[test]
    fn a_gradient_of_current_colour_resolves_against_what_it_is_given() {
        let gradient = linear(vec![
            Stop {
                color: Color::CurrentColor,
                position: Some(0.0),
            },
            stop(Rgba::WHITE, Some(1.0)),
        ]);
        let red = Rgba::new(1.0, 0.0, 0.0, 1.0);
        assert!(close(gradient.at(0.0, red), red));
    }

    #[test]
    fn a_fade_to_transparent_keeps_its_colour() {
        let tint = Rgba::from_rgba8(248, 214, 204, 255);
        let gradient = linear(vec![stop(tint, None), stop(Rgba::TRANSPARENT, None)]);
        let half = gradient.at(0.5, Rgba::BLACK);
        assert!(
            close(half, tint),
            "half way, the colour is the tint's and not a grey: {half:?}",
        );
        assert!((half.alpha - 0.5).abs() < 0.001, "and half as opaque");
        assert_eq!(gradient.at(1.0, Rgba::BLACK), Rgba::TRANSPARENT);
    }

    #[test]
    fn two_translucent_colours_mix_by_how_much_of_each_there_is() {
        let red = Rgba::new(1.0, 0.0, 0.0, 0.2);
        let blue = Rgba::new(0.0, 0.0, 1.0, 0.6);
        let half = linear(vec![stop(red, None), stop(blue, None)]).at(0.5, Rgba::BLACK);
        // Premultiplied: 0.1 red and 0.3 blue over 0.4 of alpha.
        assert!((half.alpha - 0.4).abs() < 0.001);
        assert!((half.red - 0.25).abs() < 0.001, "{half:?}");
        assert!((half.blue - 0.75).abs() < 0.001, "{half:?}");
    }

    #[test]
    fn an_extent_is_one_of_four_words() {
        assert_eq!(Extent::parse("closest-side"), Some(Extent::ClosestSide));
        assert_eq!(
            Extent::parse("Farthest-Corner"),
            Some(Extent::FarthestCorner)
        );
        assert_eq!(Extent::parse("cover"), None, "the old spelling is refused");
    }

    #[test]
    fn a_plain_radial_gradient_is_written_as_it_always_was() {
        let plain = Gradient::Radial {
            shape: Shape::Ellipse,
            extent: Extent::FarthestCorner,
            centre: Position::CENTRE,
            stops: vec![stop(Rgba::WHITE, None), stop(Rgba::BLACK, None)],
        };
        assert!(plain.to_string().starts_with("radial-gradient(rgb"));
        let placed = Gradient::Radial {
            shape: Shape::Circle,
            extent: Extent::FarthestCorner,
            centre: Position {
                x: Offset::Fraction(0.92),
                y: Offset::Pixels(0.0),
            },
            stops: vec![stop(Rgba::WHITE, None), stop(Rgba::BLACK, None)],
        };
        assert!(
            placed
                .to_string()
                .starts_with("radial-gradient(circle farthest-corner at 92% 0px, "),
            "{placed}",
        );
    }

    #[test]
    fn a_gradient_with_no_stops_draws_nothing() {
        assert_eq!(linear(Vec::new()).at(0.5, Rgba::BLACK), Rgba::TRANSPARENT);
    }

    #[test]
    fn the_side_phrases_are_the_angles_css_gives_them() {
        assert_eq!(Angle::from_sides("to bottom"), Some(Angle(180.0)));
        assert_eq!(Angle::from_sides("to top"), Some(Angle(0.0)));
        assert_eq!(Angle::from_sides("to right"), Some(Angle(90.0)));
        assert_eq!(Angle::from_sides("to left"), Some(Angle(270.0)));
        assert_eq!(Angle::from_sides("to bottom right"), Some(Angle(135.0)));
        assert_eq!(Angle::from_sides("to top left"), Some(Angle(315.0)));
        assert_eq!(Angle::from_sides("to nowhere"), None);
        assert_eq!(
            Angle::from_sides("to top bottom"),
            None,
            "two opposite sides are not a direction",
        );
    }
}
