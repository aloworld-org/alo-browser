/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! SVG, drawn: what an `<svg>` in a page holds, as a list of filled paths.
//!
//! ADR 0022 decided the shape of this. An outermost `<svg>` is **one replaced
//! box** (`alo-box`'s `svg.rs`), and nothing inside it is a box. What is
//! inside is turned, **after layout** — because a percentage inside an SVG is a
//! share of the viewport, and the viewport is the box — into a
//! [`alo_paint::Drawing`]: paths in the box's own coordinates, the colour and
//! rule each is filled with, and the groups `opacity` fades together. The
//! renderer's pipeline asks [`draw`] for each `<svg>` box's drawing and hands
//! it to paint by box, beside the box's picture, so paint never learns it came
//! from SVG and SVG adds a second drawing model without a second tree.
//!
//! # What is drawn today
//!
//! Item 271: the basic shapes (`rect` with its rounded corners, `circle`,
//! `ellipse`, `polygon`, `polyline`, and `line`, whose fill is nothing), `<g>`,
//! the `transform` attribute, `viewBox` and `preserveAspectRatio`, and `fill`,
//! `fill-rule`, `fill-opacity` and `opacity` — all computed by the one cascade,
//! presentation attributes included (`alo-style`'s `presentation.rs`). Path
//! data is item 272, strokes 273, and a nested `<svg>` 278; each of those, and
//! everything ADR 0022 § 7 refuses, is left out **and recorded**.
//!
//! # The bytes are a stranger's
//!
//! Every count a page can inflate is bounded before the work it causes
//! ([`bounds`]), every number is finite or an error ([`number`]), and the walk
//! keeps its own stack rather than recursing. A drawing past a bound is
//! refused whole and its box left empty, never cut short silently.
//!
//! # Files
//!
//! - [`walk`] — the tree, in paint order, and what each element is;
//! - [`shape`] — a basic shape's path, as SVG 2 defines it;
//! - [`fill`] — the fill a computed style asks for;
//! - [`viewport`] — `viewBox` and `preserveAspectRatio` as one transform;
//! - [`transform`] — the `transform` attribute's grammar;
//! - [`length`] — geometry attributes as user units;
//! - [`number`] — SVG's numbers, in lists;
//! - [`bounds`] — how much one drawing may ask for.

pub mod bounds;
pub mod fill;
pub mod length;
pub mod number;
pub mod shape;
pub mod transform;
pub mod viewport;
pub mod walk;

pub use walk::{Drawn, draw};
