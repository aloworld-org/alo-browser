/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! How much one drawing may ask for (ADR 0022 § 5).
//!
//! Every count here is one a stranger chooses inside an attribute or by
//! nesting elements, and each is checked **before** the work it would cause.
//! Past any of them the whole drawing is refused and the box is left empty,
//! as a picture that could not be decoded leaves its box — never truncated
//! silently, because half an icon is a wrong picture nobody can explain.
//!
//! The numbers are generous for anything a person draws and small against
//! what a page could make a renderer spend. Each is pinned by a test at its
//! edge in [`crate::walk`] or [`crate::shape`].

/// Coordinate pairs in one `points` attribute.
///
/// An icon's polygon has a handful and a chart's polyline a few thousand. Past
/// sixty-five thousand it is not a shape somebody drew; it is a way to make a
/// path the size of the attribute.
pub const MOST_POINTS: usize = 65_536;

/// Path segments in one `<path>`'s data, an arc counted as every curve it
/// becomes.
///
/// Checked as the path is made, so data the size of an attribute cannot become
/// a path the size of memory before the drawing's own bound is reached. The
/// same as [`MOST_POINTS`], because a path of lines is a polyline written
/// another way; a detailed illustration's largest path has a few thousand.
pub const MOST_PATH_SEGMENTS: usize = 65_536;

/// Path segments in one drawing, over every shape in it.
///
/// A segment is about thirty bytes, so this holds a drawing to about eight
/// megabytes of path however its shapes are spread. A detailed illustration
/// has some thousands of segments.
pub const MOST_SEGMENTS: usize = 262_144;

/// Elements visited in one drawing.
///
/// Bounds the walk itself, whatever the elements are: a hundred thousand
/// empty `<g>`s draw nothing and would still be visited.
pub const MOST_ELEMENTS: usize = 65_536;

/// How deep elements may nest.
///
/// The walk keeps its own stack rather than recursing, so depth cannot run
/// out the thread's stack; this is what keeps that explicit stack, and the
/// transform each level carries, small.
pub const DEEPEST: usize = 256;

/// How many `opacity` groups may be open inside one another.
///
/// Paint draws a group onto a surface the size of the page and composites it
/// back, so each level open at once costs a page of pixels. Sixteen is far past
/// anything a designer nests and far short of a gigabyte.
pub const MOST_GROUPS_DEEP: usize = 16;

/// Lengths written in one `stroke-dasharray`.
///
/// A dashed line has two and an elaborate one six or eight. A pattern of
/// thousands is not a pattern anybody can see; it is a way to make the dasher
/// walk a list the size of an attribute for every dash it lays.
pub const MOST_DASH_LENGTHS: usize = 256;

/// Dashes one stroked path may be cut into, counted before it is cut.
///
/// ADR 0022 § 5's decompression bomb: a 0.0001-unit dash along a long path is
/// millions of dashes, each a subpath to outline. A dashed rule across a wide
/// page is some hundreds; sixteen thousand, each outlined in a dozen segments,
/// still fits inside [`MOST_SEGMENTS`], so one path at this bound cannot by
/// itself refuse a drawing that would otherwise be drawn.
pub const MOST_DASHES: usize = 16_384;
