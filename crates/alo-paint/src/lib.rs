//! Paint: shapes into coverage, and coverage into pixels.
//!
//! Everything this engine draws is a path — a rectangle is four lines, a
//! rounded corner is an arc, a letter is a few dozen curves — so there is one
//! shape type and one rasteriser. A glyph and the box behind it come out of the
//! same code with the same anti-aliasing, which is what stops them disagreeing
//! along their shared edge.
//!
//! # What is rented and where
//!
//! - **Reading a font's outlines** is `ttf-parser`, and [`glyph`] is the only
//!   file that names it.
//! - **Filling a path with anti-aliasing** is `tiny-skia`, and [`raster`] is
//!   the only file that names it.
//!
//! `scripts/gate.sh` checks both on every run.
//!
//! # Coverage is not colour
//!
//! A rasterised shape says **how much of each pixel it covers**, from zero to
//! 255. Colour arrives when the coverage is composited, which is why the same
//! glyph mask serves black text on white and white text on black, and why a
//! mask can be reused for a shadow rather than rasterised twice.

pub mod glyph;
pub mod path;
pub mod raster;

pub use glyph::{Glyph, outline};
pub use path::{Path, Point, Segment};
pub use raster::{Coverage, fill};
