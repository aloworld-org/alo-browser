/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! SVG's `<paint>`: what `fill` and `stroke` are written as.
//!
//! The two properties take the same values and differ only in what nothing
//! written means — a fill is black and a stroke is nothing — so they are read
//! here once. `currentColor` is the element's computed `color`, which inherits
//! from the HTML around the `<svg>`: an icon drawn in `currentColor` is the
//! colour of the text beside it, which is the point of it.
//!
//! A reference to a paint server — `url(#teal)` — is read here into a
//! [`Server`], the name it refers to and what to draw if it cannot be used;
//! what the name refers to is found by the walk, which alone has the
//! document and the shape's geometry ([`crate::server`]).

use alo_style::ComputedStyle;
use alo_value::Rgba;

/// What a paint property asks a shape to be painted with.
#[derive(Debug, Clone, PartialEq)]
pub enum Brush {
    /// One colour, its opacity property already in its alpha.
    Colour(Rgba),
    /// A paint server, found by its name.
    Server(Server),
}

/// A reference to a paint server, as written: `url(#name)`, and what is
/// drawn in its place when it cannot be used.
#[derive(Debug, Clone, PartialEq)]
pub struct Server {
    /// The property, for what is recorded about it.
    pub property: &'static str,
    /// What was written, for what is recorded about it.
    pub written: String,
    /// The `id` it names, or [`None`] for a reference outside this
    /// document, which is never followed.
    pub id: Option<String>,
    /// What is drawn when it cannot be used: the colour written after it,
    /// opacity applied, or [`None`] for none written and for `none`.
    pub fallback: Option<Rgba>,
    /// The property's opacity — `fill-opacity` or `stroke-opacity` — which
    /// fades every colour the server has.
    pub opacity: f32,
}

/// What a paint property asks for, or [`None`] for `none` and for anything
/// this engine cannot draw.
///
/// `initial` is the property's value when nothing sets it. A context keyword
/// belongs to `<use>` and markers and draws nothing here, and is recorded.
pub fn paint_of(
    style: &ComputedStyle,
    property: &'static str,
    initial: Option<Rgba>,
    issues: &mut Vec<String>,
) -> Option<Brush> {
    match style.get(property).map(str::trim) {
        None => initial.map(Brush::Colour),
        Some(text) if text.eq_ignore_ascii_case("none") => None,
        Some(text) if starts_with_ignoring_case(text, "url(") => {
            let close = text.find(')')?;
            let inside = text.get(4..close)?.trim().trim_matches(['"', '\'']);
            let id = inside.strip_prefix('#').map(str::to_owned);
            let rest = text.get(close + 1..).map(str::trim).unwrap_or_default();
            let fallback = if rest.is_empty() || rest.eq_ignore_ascii_case("none") {
                None
            } else {
                alo_value::parse_color(rest).map(|color| color.resolve(style.current_color()))
            };
            Some(Brush::Server(Server {
                property,
                written: text.to_owned(),
                id,
                fallback,
                opacity: 1.0,
            }))
        }
        Some(text) if starts_with_ignoring_case(text, "context-") => {
            issues.push(format!(
                "{property}: {text}: context paint belongs to <use> and markers, not drawn"
            ));
            None
        }
        Some(_) => style.color(property).map(Brush::Colour),
    }
}

/// A brush faded by its opacity property's value, or [`None`] when a colour
/// is left that cannot be seen. A server keeps the opacity for its colours,
/// and its fallback is faded now.
pub fn faded_brush(brush: Brush, opacity: Option<&str>) -> Option<Brush> {
    match brush {
        Brush::Colour(color) => faded(color, opacity).map(Brush::Colour),
        Brush::Server(server) => Some(Brush::Server(Server {
            fallback: server.fallback.and_then(|color| faded(color, opacity)),
            opacity: alpha(opacity).unwrap_or(1.0),
            ..server
        })),
    }
}

/// An opacity: a number or a percentage, held to between nothing and one.
pub fn alpha(text: Option<&str>) -> Option<f32> {
    let text = text?.trim();
    let value = match alo_value::parse_length_percentage(text) {
        Some(alo_value::LengthPercentage::Percentage(percent)) => percent / 100.0,
        _ => alo_value::parse_number(text)?,
    };
    value.is_finite().then(|| value.clamp(0.0, 1.0))
}

/// A colour faded by an opacity property's value, or [`None`] when what is
/// left cannot be seen.
pub fn faded(color: Rgba, opacity: Option<&str>) -> Option<Rgba> {
    let color = Rgba {
        alpha: color.alpha * alpha(opacity).unwrap_or(1.0),
        ..color
    };
    (!color.is_invisible()).then_some(color)
}

fn starts_with_ignoring_case(text: &str, start: &str) -> bool {
    text.get(..start.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(start))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fallback_is_faded_with_the_server_and_the_server_keeps_the_opacity() {
        let server = Brush::Server(Server {
            property: "fill",
            written: "url(#g) red".to_owned(),
            id: Some("g".to_owned()),
            fallback: Some(Rgba::BLACK),
            opacity: 1.0,
        });
        let Some(Brush::Server(faded)) = faded_brush(server, Some("0.25")) else {
            panic!("still a server");
        };
        assert!((faded.opacity - 0.25).abs() < 1.0e-6);
        assert_eq!(faded.fallback.map(|color| color.alpha), Some(0.25));
    }

    #[test]
    fn an_alpha_is_a_number_or_a_percentage_held_to_one() {
        assert_eq!(alpha(Some("0.25")), Some(0.25));
        assert_eq!(alpha(Some("25%")), Some(0.25));
        assert_eq!(alpha(Some("7")), Some(1.0));
        assert_eq!(alpha(Some("-1")), Some(0.0));
        assert_eq!(alpha(Some("half")), None);
        assert_eq!(alpha(None), None);
    }

    #[test]
    fn a_colour_faded_to_nothing_is_nothing() {
        assert_eq!(faded(Rgba::BLACK, Some("0")), None);
        assert_eq!(faded(Rgba::TRANSPARENT, None), None);
        assert_eq!(
            faded(Rgba::BLACK, Some("50%")).map(|color| color.alpha),
            Some(0.5)
        );
        assert_eq!(faded(Rgba::BLACK, Some("bogus")), Some(Rgba::BLACK));
    }
}
