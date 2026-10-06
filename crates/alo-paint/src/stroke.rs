/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How a path's outline is drawn: its width, its ends, its corners, its
//! dashes.
//!
//! CSS never strokes a path — a border is a filled ring — and SVG does (ADR
//! 0022 § 2). A stroke is not a second kind of drawing: [`crate::raster`]
//! turns a path and one of these into **the outline the stroke covers**, and
//! that outline is filled like any other shape. So paint keeps one thing it
//! draws, and the offset curves, joins and caps are the rented rasteriser's
//! physics rather than ours.
//!
//! The words are SVG's and the meanings are the ones every stroker shares: the
//! miter limit is a ratio of the miter's length to the stroke's width, and a
//! dash pattern starts again at the start of every subpath.

use core::fmt;

/// What the open ends of a stroke look like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineCap {
    /// Cut square at the end point.
    #[default]
    Butt,
    /// A half disc past the end point.
    Round,
    /// Half a width of square past the end point.
    Square,
}

/// What the corners of a stroke look like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineJoin {
    /// A sharp point, cut back to a bevel past the miter limit.
    #[default]
    Miter,
    /// A disc at the corner.
    Round,
    /// The two outer edges joined by a straight line.
    Bevel,
}

/// A dash pattern.
///
/// The lengths alternate dash and gap, starting with a dash. They are held
/// here exactly as given; making them an even count and refusing a pattern
/// that comes to nothing is the maker's job, because what *nothing* means (a
/// solid stroke, in SVG) is the maker's language and not paint's.
#[derive(Debug, Clone, PartialEq)]
pub struct Dashes {
    /// Dash, gap, dash, gap…
    pub lengths: Vec<f32>,
    /// How far into the pattern each subpath starts.
    pub offset: f32,
}

/// Everything about a stroke except what colour it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    /// How wide, across the path, in the path's own units.
    pub width: f32,
    /// The ends.
    pub cap: LineCap,
    /// The corners.
    pub join: LineJoin,
    /// How long a miter may be, as a multiple of the width, before it is
    /// beveled instead.
    pub miter_limit: f32,
    /// The dashes, or [`None`] for a stroke drawn whole.
    pub dashes: Option<Dashes>,
}

impl Default for Stroke {
    /// SVG's initial values: one unit wide, butt ends, mitered corners up to
    /// four widths, no dashes.
    fn default() -> Self {
        Self {
            width: 1.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 4.0,
            dashes: None,
        }
    }
}

impl LineCap {
    /// The cap `stroke-linecap` names, or [`None`] for a word that is not one.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        [Self::Butt, Self::Round, Self::Square]
            .into_iter()
            .find(|cap| text.eq_ignore_ascii_case(&cap.to_string()))
    }
}

impl LineJoin {
    /// The join `stroke-linejoin` names, or [`None`] for a word that is not
    /// one.
    ///
    /// SVG 2 also writes `miter-clip` and `arcs`. No browser draws either, so
    /// a page that writes one gets what it gets everywhere else: the word is
    /// not understood.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        [Self::Miter, Self::Round, Self::Bevel]
            .into_iter()
            .find(|join| text.eq_ignore_ascii_case(&join.to_string()))
    }
}

impl fmt::Display for LineCap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Butt => "butt",
            Self::Round => "round",
            Self::Square => "square",
        })
    }
}

impl fmt::Display for LineJoin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Miter => "miter",
            Self::Round => "round",
            Self::Bevel => "bevel",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_and_joins_are_read_by_name_whatever_their_case() {
        assert_eq!(LineCap::parse("butt"), Some(LineCap::Butt));
        assert_eq!(LineCap::parse(" Round "), Some(LineCap::Round));
        assert_eq!(LineCap::parse("SQUARE"), Some(LineCap::Square));
        assert_eq!(LineCap::parse("flat"), None);
        assert_eq!(LineJoin::parse("miter"), Some(LineJoin::Miter));
        assert_eq!(LineJoin::parse("round"), Some(LineJoin::Round));
        assert_eq!(LineJoin::parse("Bevel"), Some(LineJoin::Bevel));
        assert_eq!(LineJoin::parse("miter-clip"), None);
        assert_eq!(LineJoin::parse("arcs"), None);
        assert_eq!(LineJoin::parse(""), None);
    }

    #[test]
    fn nothing_written_is_svgs_initial_stroke() {
        let stroke = Stroke::default();
        assert_eq!(
            (Some(stroke.width), Some(stroke.miter_limit)),
            (Some(1.0), Some(4.0))
        );
        assert_eq!((stroke.cap, stroke.join), (LineCap::Butt, LineJoin::Miter));
        assert_eq!(stroke.dashes, None);
    }
}
