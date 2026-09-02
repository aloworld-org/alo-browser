//! Colours, as channels.
//!
//! A colour arrives from the cascade as text — `#101014`, `rgb(1 2 3)`,
//! `oklch(70% 0.1 200)` — and paint needs four numbers. This is where the one
//! becomes the other, in the crate that already turns text into numbers, so
//! that paint does not grow a second value parser beside it.
//!
//! # Two decisions worth knowing
//!
//! **`currentColor` is a value, not a colour.** It is the initial value of
//! `border-color`, and it means "whatever `color` is on this element" — which
//! is not knowable until there *is* an element. So it is carried as itself and
//! resolved by whoever has one, and an engine that folded it into black at
//! parse time would draw every default border the wrong colour.
//!
//! **Channels are floats from zero to one**, not bytes. Compositing multiplies
//! and adds them, and doing that in eight bits loses a little every time —
//! which shows up as banding in exactly the gradients a design system uses.
//! Bytes come back at the very end, in [`Rgba::to_rgba8`].
//!
//! # What is refused
//!
//! `oklch`, `lab`, `color()` and `color-mix()` are not implemented and are
//! refused rather than approximated. They are a different colour space, and a
//! colour converted by guesswork is a wrong pixel that looks nearly right —
//! which law 3 calls a bug.

use core::fmt;

/// A colour, resolved: four channels from zero to one, in sRGB.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    /// Red.
    pub red: f32,
    /// Green.
    pub green: f32,
    /// Blue.
    pub blue: f32,
    /// How opaque: zero is invisible, one is solid.
    pub alpha: f32,
}

impl Rgba {
    /// Fully transparent, which is what `transparent` means and what an
    /// unpainted background is.
    pub const TRANSPARENT: Rgba = Rgba {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 0.0,
    };

    /// Solid black.
    pub const BLACK: Rgba = Rgba {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 1.0,
    };

    /// Solid white.
    pub const WHITE: Rgba = Rgba {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
        alpha: 1.0,
    };

    /// A colour from four channels, each clamped to the range it has.
    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red: clamp(red),
            green: clamp(green),
            blue: clamp(blue),
            alpha: clamp(alpha),
        }
    }

    /// A colour from four bytes.
    pub fn from_rgba8(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red: f32::from(red) / 255.0,
            green: f32::from(green) / 255.0,
            blue: f32::from(blue) / 255.0,
            alpha: f32::from(alpha) / 255.0,
        }
    }

    /// The colour as four bytes, rounded rather than truncated.
    ///
    /// Truncating loses half a level on every channel, which turns a
    /// mid-grey ramp into a ramp that is consistently one darker.
    pub fn to_rgba8(self) -> (u8, u8, u8, u8) {
        (
            to_byte(self.red),
            to_byte(self.green),
            to_byte(self.blue),
            to_byte(self.alpha),
        )
    }

    /// Whether this colour would draw nothing at all.
    pub fn is_invisible(self) -> bool {
        self.alpha <= 0.0
    }

    /// This colour drawn over another one.
    ///
    /// Source-over, the compositing every background and every shadow uses.
    /// It is here rather than in paint because it is arithmetic on channels
    /// and belongs with the channels.
    #[must_use]
    pub fn over(self, under: Rgba) -> Rgba {
        let alpha = self.alpha + under.alpha * (1.0 - self.alpha);
        if alpha <= 0.0 {
            return Rgba::TRANSPARENT;
        }
        let blend = |top: f32, bottom: f32| {
            (top * self.alpha + bottom * under.alpha * (1.0 - self.alpha)) / alpha
        };
        Rgba {
            red: blend(self.red, under.red),
            green: blend(self.green, under.green),
            blue: blend(self.blue, under.blue),
            alpha,
        }
    }
}

impl Default for Rgba {
    fn default() -> Self {
        Rgba::TRANSPARENT
    }
}

impl fmt::Display for Rgba {
    /// The form CSS would serialise it as: `rgb(r g b)`, with the alpha only
    /// when it is not solid.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (red, green, blue, _) = self.to_rgba8();
        if (self.alpha - 1.0).abs() < f32::EPSILON {
            write!(f, "rgb({red} {green} {blue})")
        } else {
            write!(f, "rgb({red} {green} {blue} / {})", self.alpha)
        }
    }
}

/// A colour as a style sheet wrote it, which may not be a colour yet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    /// A colour.
    Rgba(Rgba),
    /// `currentColor` — whatever `color` is on the element this is used on.
    CurrentColor,
}

impl Color {
    /// The colour, given what `color` is on the element.
    pub fn resolve(self, current: Rgba) -> Rgba {
        match self {
            Color::Rgba(rgba) => rgba,
            Color::CurrentColor => current,
        }
    }

    /// Whether this is `currentColor` and still needs an element.
    pub fn is_current_color(self) -> bool {
        self == Color::CurrentColor
    }
}

impl From<Rgba> for Color {
    fn from(rgba: Rgba) -> Self {
        Color::Rgba(rgba)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Color::Rgba(rgba) => write!(f, "{rgba}"),
            Color::CurrentColor => f.write_str("currentcolor"),
        }
    }
}

/// Turn a hue, a saturation and a lightness into channels.
///
/// The algorithm is the specification's. It is here rather than in the parser
/// because it is arithmetic about colour rather than about text.
pub fn from_hsl(hue_degrees: f32, saturation: f32, lightness: f32, alpha: f32) -> Rgba {
    let saturation = clamp(saturation);
    let lightness = clamp(lightness);
    // A hue is an angle, so 400 degrees is 40 and -30 is 330.
    let hue = hue_degrees.rem_euclid(360.0) / 60.0;
    let chroma = (1.0 - (2.0f32.mul_add(lightness, -1.0)).abs()) * saturation;
    let second = chroma * (1.0 - ((hue % 2.0) - 1.0).abs());
    // The wheel in sixths: which pair of channels the hue falls between.
    let sixth = if hue < 1.0 {
        0
    } else if hue < 2.0 {
        1
    } else if hue < 3.0 {
        2
    } else if hue < 4.0 {
        3
    } else if hue < 5.0 {
        4
    } else {
        5
    };
    let (red, green, blue) = match sixth {
        0 => (chroma, second, 0.0),
        1 => (second, chroma, 0.0),
        2 => (0.0, chroma, second),
        3 => (0.0, second, chroma),
        4 => (second, 0.0, chroma),
        _ => (chroma, 0.0, second),
    };
    let lift = chroma.mul_add(-0.5, lightness);
    Rgba::new(red + lift, green + lift, blue + lift, alpha)
}

fn clamp(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn to_byte(value: f32) -> u8 {
    let scaled = (clamp(value) * 255.0).round();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to zero..one and scaled, so it is a whole number in 0..=255"
    )]
    let byte = scaled as u8;
    byte
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.002
    }

    fn same(left: Rgba, right: Rgba) -> bool {
        close(left.red, right.red)
            && close(left.green, right.green)
            && close(left.blue, right.blue)
            && close(left.alpha, right.alpha)
    }

    #[test]
    fn a_colour_round_trips_through_bytes() {
        for bytes in [(0, 0, 0, 255), (255, 255, 255, 255), (16, 16, 20, 128)] {
            let colour = Rgba::from_rgba8(bytes.0, bytes.1, bytes.2, bytes.3);
            assert_eq!(colour.to_rgba8(), bytes);
        }
    }

    #[test]
    fn bytes_are_rounded_rather_than_truncated() {
        // A half-way channel is 128 rather than 127: truncating loses half a
        // level on every channel of every pixel.
        let half = Rgba::new(0.5, 0.5, 0.5, 1.0);
        assert_eq!(half.to_rgba8(), (128, 128, 128, 255));
    }

    #[test]
    fn a_channel_outside_its_range_is_brought_back_into_it() {
        let wild = Rgba::new(2.0, -1.0, f32::NAN, 5.0);
        assert!(same(wild, Rgba::new(1.0, 0.0, 0.0, 1.0)));
    }

    #[test]
    fn transparent_draws_nothing_and_says_so() {
        assert!(Rgba::TRANSPARENT.is_invisible());
        assert!(!Rgba::BLACK.is_invisible());
        assert_eq!(Rgba::default(), Rgba::TRANSPARENT);
    }

    #[test]
    fn a_solid_colour_over_anything_is_itself() {
        assert!(same(Rgba::WHITE.over(Rgba::BLACK), Rgba::WHITE));
        assert!(same(Rgba::BLACK.over(Rgba::WHITE), Rgba::BLACK));
    }

    #[test]
    fn nothing_over_something_leaves_it_alone() {
        assert!(same(Rgba::TRANSPARENT.over(Rgba::WHITE), Rgba::WHITE));
        assert!(same(
            Rgba::TRANSPARENT.over(Rgba::TRANSPARENT),
            Rgba::TRANSPARENT,
        ));
    }

    #[test]
    fn half_of_white_over_black_is_half_way_between_them() {
        let half_white = Rgba::new(1.0, 1.0, 1.0, 0.5);
        let blended = half_white.over(Rgba::BLACK);
        assert!(same(blended, Rgba::new(0.5, 0.5, 0.5, 1.0)));
    }

    #[test]
    fn the_hue_wheel_gives_the_colours_it_should() {
        assert!(same(
            from_hsl(0.0, 1.0, 0.5, 1.0),
            Rgba::new(1.0, 0.0, 0.0, 1.0)
        ));
        assert!(same(
            from_hsl(120.0, 1.0, 0.5, 1.0),
            Rgba::new(0.0, 1.0, 0.0, 1.0)
        ));
        assert!(same(
            from_hsl(240.0, 1.0, 0.5, 1.0),
            Rgba::new(0.0, 0.0, 1.0, 1.0)
        ));
        assert!(same(
            from_hsl(60.0, 1.0, 0.5, 1.0),
            Rgba::new(1.0, 1.0, 0.0, 1.0)
        ));
    }

    #[test]
    fn a_hue_is_an_angle_so_it_wraps_both_ways() {
        assert!(same(
            from_hsl(360.0, 1.0, 0.5, 1.0),
            from_hsl(0.0, 1.0, 0.5, 1.0)
        ));
        assert!(same(
            from_hsl(400.0, 1.0, 0.5, 1.0),
            from_hsl(40.0, 1.0, 0.5, 1.0)
        ));
        assert!(same(
            from_hsl(-120.0, 1.0, 0.5, 1.0),
            from_hsl(240.0, 1.0, 0.5, 1.0)
        ));
    }

    #[test]
    fn no_saturation_is_grey_and_the_extremes_of_lightness_are_black_and_white() {
        assert!(same(
            from_hsl(200.0, 0.0, 0.5, 1.0),
            Rgba::new(0.5, 0.5, 0.5, 1.0)
        ));
        assert!(same(from_hsl(200.0, 1.0, 0.0, 1.0), Rgba::BLACK));
        assert!(same(from_hsl(200.0, 1.0, 1.0, 1.0), Rgba::WHITE));
    }

    #[test]
    fn current_colour_is_carried_until_there_is_an_element_to_ask() {
        assert!(Color::CurrentColor.is_current_color());
        assert_eq!(Color::CurrentColor.resolve(Rgba::WHITE), Rgba::WHITE);

        let fixed = Color::Rgba(Rgba::BLACK);
        assert!(!fixed.is_current_color());
        assert_eq!(
            fixed.resolve(Rgba::WHITE),
            Rgba::BLACK,
            "a colour that is a colour ignores what it is asked",
        );
    }

    #[test]
    fn a_colour_writes_itself_back_out() {
        assert_eq!(Rgba::BLACK.to_string(), "rgb(0 0 0)");
        assert_eq!(
            Rgba::new(1.0, 0.0, 0.0, 0.5).to_string(),
            "rgb(255 0 0 / 0.5)",
        );
        assert_eq!(Color::CurrentColor.to_string(), "currentcolor");
        assert_eq!(Color::from(Rgba::WHITE).to_string(), "rgb(255 255 255)");
    }
}
