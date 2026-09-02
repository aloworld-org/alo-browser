//! Shadows: a copy of a shape, moved, blurred, and drawn behind it.
//!
//! `box-shadow` and `text-shadow` are the same idea written two ways. A box
//! shadow may be spread — grown or shrunk before it is blurred — and may be
//! `inset`, drawn inside the box rather than behind it; a text shadow has
//! neither, because there is no sensible way to grow a letter. Everything else
//! is shared, so it is one type with two ways in.
//!
//! # Lengths, not pixels
//!
//! The offsets and the blur are [`Length`]s, kept as the author wrote them,
//! because `0 0.125em` is a shadow that changes size with the text it is under
//! and resolving it at parse time would have frozen it at whatever the root
//! font size happened to be. [`Shadow::drawn`] turns one into pixels when
//! something is about to draw it.

use crate::color::{Color, Rgba};
use crate::length::{FontMetrics, Length};
use core::fmt;

/// A shadow, as the author wrote it.
#[derive(Debug, Clone, PartialEq)]
pub struct Shadow {
    /// How far to the right, and how far down.
    pub offset: (Length, Length),
    /// How far the edge fades over. Never negative.
    pub blur: Length,
    /// How much larger the shape is before it is blurred; negative shrinks it.
    /// Always zero for a text shadow.
    pub spread: Length,
    /// What colour, or [`None`] when the author left it to the text colour.
    pub color: Option<Color>,
    /// Whether it is drawn inside the box rather than behind it. Always false
    /// for a text shadow.
    pub inset: bool,
}

/// A shadow in pixels, ready to draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawnShadow {
    /// How far right and down, in pixels.
    pub offset: (f32, f32),
    /// How far the edge fades over, in pixels.
    pub blur: f32,
    /// How much larger the shape is before it is blurred, in pixels.
    pub spread: f32,
    /// What colour.
    pub color: Rgba,
    /// Whether it is drawn inside the box.
    pub inset: bool,
}

impl Shadow {
    /// This shadow in pixels, with `currentColor` answered.
    ///
    /// A shadow with no colour of its own is the colour of the text it is
    /// under, which is what CSS says and what makes `text-shadow: 0 1px 2px`
    /// legible on a dark page and on a light one.
    pub fn drawn(&self, metrics: FontMetrics, current: Rgba) -> DrawnShadow {
        DrawnShadow {
            offset: (self.offset.0.to_px(metrics), self.offset.1.to_px(metrics)),
            // A negative blur is not a sharper shadow, it is nonsense; CSS
            // makes it invalid, and a value that got this far is clamped.
            blur: self.blur.to_px(metrics).max(0.0),
            spread: self.spread.to_px(metrics),
            color: self.color.unwrap_or(Color::CurrentColor).resolve(current),
            inset: self.inset,
        }
    }
}

impl DrawnShadow {
    /// Whether drawing this would change nothing.
    pub fn is_invisible(self) -> bool {
        self.color.is_invisible()
    }
}

impl fmt::Display for DrawnShadow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.inset {
            write!(f, "inset ")?;
        }
        write!(
            f,
            "{} {} blur {} spread {} {}",
            self.offset.0, self.offset.1, self.blur, self.spread, self.color,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Unit;

    fn metrics() -> FontMetrics {
        FontMetrics::estimated(16.0, 16.0)
    }

    fn shadow() -> Shadow {
        Shadow {
            offset: (
                Length::px(0.0),
                Length {
                    value: 0.5,
                    unit: Unit::Em,
                },
            ),
            blur: Length {
                value: 1.0,
                unit: Unit::Em,
            },
            spread: Length::px(-2.0),
            color: None,
            inset: false,
        }
    }

    #[test]
    fn a_shadow_keeps_its_lengths_until_something_draws_it() {
        let drawn = shadow().drawn(metrics(), Rgba::BLACK);
        assert!((drawn.offset.1 - 8.0).abs() < f32::EPSILON, "half of 16px");
        assert!((drawn.blur - 16.0).abs() < f32::EPSILON);
        assert!((drawn.spread + 2.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_shadow_with_no_colour_takes_the_colour_of_the_text() {
        let drawn = shadow().drawn(metrics(), Rgba::new(1.0, 0.0, 0.0, 1.0));
        assert_eq!(drawn.color, Rgba::new(1.0, 0.0, 0.0, 1.0));
        assert!(!drawn.is_invisible());
    }

    #[test]
    fn a_negative_blur_is_no_blur_rather_than_a_sharper_one() {
        let drawn = Shadow {
            blur: Length::px(-4.0),
            ..shadow()
        }
        .drawn(metrics(), Rgba::BLACK);
        assert!((drawn.blur).abs() < f32::EPSILON);
    }

    #[test]
    fn a_transparent_shadow_is_worth_nothing_and_says_so() {
        let drawn = Shadow {
            color: Some(Color::Rgba(Rgba::TRANSPARENT)),
            ..shadow()
        }
        .drawn(metrics(), Rgba::BLACK);
        assert!(drawn.is_invisible());
    }

    #[test]
    fn a_shadow_reads_back_as_what_it_is() {
        let drawn = Shadow {
            inset: true,
            color: Some(Color::Rgba(Rgba::BLACK)),
            ..shadow()
        }
        .drawn(metrics(), Rgba::WHITE);
        assert!(
            drawn
                .to_string()
                .starts_with("inset 0 8 blur 16 spread -2 ")
        );
    }
}
