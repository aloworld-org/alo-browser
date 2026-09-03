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
//! # A picture, end to end
//!
//! [`build::build`] turns a laid-out document into a list of things to draw;
//! [`render`] draws them onto a [`Canvas`]; [`to_png`] writes it out. The list
//! in the middle is what makes a picture that came out wrong *diagnosable*: it
//! says "the row's background moved four pixels", where an image says only
//! that some bytes differ.
//!
//! # Coverage is not colour
//!
//! A rasterised shape says **how much of each pixel it covers**, from zero to
//! 255. Colour arrives when the coverage is composited, which is why the same
//! glyph mask serves black text on white and white text on black, and why a
//! mask can be reused for a shadow rather than rasterised twice.

pub mod blur;
pub mod build;
pub mod canvas;
pub mod control;
pub mod corner;
pub mod coverage;
pub mod display;
pub mod encode;
pub mod glyph;
pub mod paint;
pub mod path;
pub mod picture;
pub mod raster;
pub mod render;

pub use blur::blurred;
pub use build::{PaintContext, build};
pub use canvas::Canvas;
pub use control::{Mark, mark};
pub use corner::{Corners, between, ring, rounded_rectangle};
pub use coverage::Coverage;
pub use display::{DisplayItem, DisplayList, TextShadow};
pub use encode::{PictureError, from_png, to_png};
pub use glyph::{Glyph, outline};
pub use paint::Paint;
pub use path::{Path, Point, Segment};
pub use raster::fill;
pub use render::render;
