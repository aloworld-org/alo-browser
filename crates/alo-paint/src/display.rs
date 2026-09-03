/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The display list: what to draw, in what order.
//!
//! A step between layout and pixels, and it earns its place twice over. It is
//! **what a reference render diffs against when a picture differs** — a list
//! says "the background moved four pixels" where an image says only "these
//! bytes differ" — and it is where paint order is decided, once, rather than
//! being implied by the order some loop happens to visit boxes in.
//!
//! This file is the list and what is in it. Turning a laid-out document into
//! one is [`crate::build`], which is a different reason to change: a new CSS
//! property changes the builder, a new kind of drawing changes this.

use crate::paint::Paint;
use crate::path::Path;
use alo_box::BoxId;
use alo_layout::Rect;
use alo_value::{Matrix, Rgba};
use core::fmt::Write as _;

/// A shadow cast by a run of text.
///
/// Not a [`DisplayItem`] of its own: a text shadow is the same glyphs, and the
/// renderer outlines a run once and draws it several times. Splitting them
/// would have meant shaping and outlining the text once per shadow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextShadow {
    /// How far right and down from the text it copies.
    pub offset: (f32, f32),
    /// How far its edge fades over.
    pub blur: f32,
    /// What colour.
    pub color: Rgba,
}

/// A line drawn under, over or through text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationLine {
    /// Below the baseline, where the face says.
    Underline,
    /// Along the top of the letters.
    Overline,
    /// Half way up an `x`.
    LineThrough,
}

/// The decoration lines named in a value.
///
/// `text-decoration` is a shorthand that may also carry a style, a colour and a
/// thickness; this takes the line keywords and leaves the rest, because a value
/// this engine only half understands should still produce the part it does.
/// `none` produces nothing, which is what it means.
pub fn lines_in(value: &str) -> Vec<DecorationLine> {
    let mut found = Vec::new();
    for word in value.split_ascii_whitespace() {
        let line = match word.to_ascii_lowercase().as_str() {
            "underline" => DecorationLine::Underline,
            "overline" => DecorationLine::Overline,
            "line-through" => DecorationLine::LineThrough,
            _ => continue,
        };
        if !found.contains(&line) {
            found.push(line);
        }
    }
    found
}

/// One thing to draw.
///
/// A picture is one of these rather than a `Fill` with an image paint, because
/// it is scaled to a box rather than tiled into a shape — and because the thing
/// being drawn is somebody else's bytes, which is worth being able to find in a
/// display list.
#[derive(Debug, Clone)]
pub enum DisplayItem {
    /// A shape, filled.
    Fill {
        /// The box it belongs to, so that a difference can be named.
        box_id: BoxId,
        /// What to fill.
        path: Path,
        /// What with: one colour, or a colour that changes across the shape.
        paint: Paint,
    },
    /// A shape, blurred, drawn behind or inside something.
    ///
    /// The path is already offset and spread, so an outset shadow and an inset
    /// one differ only in the shape they carry — an inset shadow is the box
    /// with a hole in it, clipped to the box. That keeps the blur in one
    /// place instead of two nearly-identical ones.
    Shadow {
        /// The box that cast it.
        box_id: BoxId,
        /// The shape to blur.
        path: Path,
        /// How far its edge fades over.
        blur: f32,
        /// What colour.
        color: Rgba,
    },
    /// Everything until the matching [`DisplayItem::PopClip`] is drawn only
    /// where this shape covers.
    ///
    /// A pair rather than a field on every item: a clip applies to a whole
    /// subtree, and repeating it on each item would be the same shape stored a
    /// hundred times and rasterised a hundred times.
    PushClip {
        /// The box that asked for it.
        box_id: BoxId,
        /// What to clip to.
        path: Path,
    },
    /// The end of the innermost clip.
    PopClip,
    /// Everything until the matching [`DisplayItem::PopTransform`] is drawn
    /// with its points put through this matrix.
    ///
    /// A pair, like a clip, and for the same reason: a transform applies to a
    /// whole subtree, and a box inside a transformed one is transformed by
    /// both.
    PushTransform {
        /// The box that asked for it.
        box_id: BoxId,
        /// Where its points go.
        matrix: Matrix,
    },
    /// The end of the innermost transform.
    PopTransform,
    /// Everything until the matching [`DisplayItem::PopGroup`] is drawn on a
    /// surface of its own and composited back at this opacity.
    ///
    /// This is what `opacity` *is*. Fading each box separately would show
    /// every box in a group through every other one; fading them once, as a
    /// picture, is the answer — and it is why a group needs its own canvas.
    PushGroup {
        /// The box that asked for it.
        box_id: BoxId,
        /// How much of the group reaches the page, from nothing to one.
        opacity: f32,
    },
    /// The end of the innermost group.
    PopGroup,
    /// A run of text.
    /// A decoded picture, drawn into a rectangle.
    Picture {
        /// The box it belongs to.
        box_id: BoxId,
        /// Where it goes, in page coordinates.
        rect: Rect,
        /// The pixels, at their own size — scaled to `rect` when drawn.
        picture: std::sync::Arc<crate::canvas::Canvas>,
    },
    /// A run of text, on its baseline.
    Text {
        /// The box it belongs to.
        box_id: BoxId,
        /// What it says.
        text: String,
        /// Where the pen starts: the left end of the baseline.
        origin: (f32, f32),
        /// What font.
        font: alo_text::Font,
        /// How big.
        size: f32,
        /// Extra room after every character, which the pen has to know about
        /// or the letters land where the line did not put them.
        letter_spacing: f32,
        /// What colour.
        color: Rgba,
        /// What it casts behind it, furthest back last — which is the order
        /// `text-shadow` is written in, and the order it is drawn in reversed.
        shadows: Vec<TextShadow>,
    },
}

impl DisplayItem {
    /// The box this came from, or [`None`] for the end of a clip, which came
    /// from wherever the clip did.
    pub fn box_id(&self) -> Option<BoxId> {
        match self {
            DisplayItem::Fill { box_id, .. }
            | DisplayItem::Shadow { box_id, .. }
            | DisplayItem::Picture { box_id, .. }
            | DisplayItem::Text { box_id, .. }
            | DisplayItem::PushClip { box_id, .. }
            | DisplayItem::PushTransform { box_id, .. }
            | DisplayItem::PushGroup { box_id, .. } => Some(*box_id),
            DisplayItem::PopClip | DisplayItem::PopTransform | DisplayItem::PopGroup => None,
        }
    }
}

/// Everything to draw, in the order to draw it.
#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    items: Vec<DisplayItem>,
}

impl DisplayList {
    /// The items, in paint order.
    pub fn items(&self) -> &[DisplayItem] {
        &self.items
    }

    /// How many there are.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there is nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Add an item.
    ///
    /// Used by [`crate::build`], and by a test that needs a list without a
    /// document: a test about the renderer alone needs a list anyway, and
    /// there is no other way to make one — which is what keeps a display list
    /// in step with a laid-out document everywhere else.
    pub fn push(&mut self, item: DisplayItem) {
        self.items.push(item);
    }

    /// Take a list's items, to put them in order.
    pub(crate) fn from_items(items: Vec<DisplayItem>) -> Self {
        Self { items }
    }

    /// The list as one line per item, for a test to compare.
    ///
    /// This is what makes a picture that changed *diagnosable*: the line says
    /// which box, what colour and where, so a failure reads "the row's
    /// background moved four pixels" rather than "the image differs".
    pub fn to_outline(&self) -> String {
        let mut out = String::new();
        for item in &self.items {
            // Writing to a `String` cannot fail.
            let _ = match item {
                DisplayItem::Picture {
                    box_id,
                    rect,
                    picture,
                } => writeln!(
                    out,
                    "picture {box_id} {}×{} at ({}, {}) {}×{}",
                    picture.width(),
                    picture.height(),
                    rect.left(),
                    rect.top(),
                    rect.right() - rect.left(),
                    rect.bottom() - rect.top(),
                ),
                DisplayItem::Fill {
                    box_id,
                    path,
                    paint,
                } => {
                    let (left, top, right, bottom) = path.bounds().unwrap_or((0.0, 0.0, 0.0, 0.0));
                    writeln!(
                        out,
                        "fill {box_id} {paint} at ({left}, {top}) {}×{}",
                        right - left,
                        bottom - top,
                    )
                }
                DisplayItem::Shadow {
                    box_id,
                    path,
                    blur,
                    color,
                } => {
                    let (left, top, right, bottom) = path.bounds().unwrap_or((0.0, 0.0, 0.0, 0.0));
                    writeln!(
                        out,
                        "shadow {box_id} {color} blur {blur} at ({left}, {top}) {}×{}",
                        right - left,
                        bottom - top,
                    )
                }
                DisplayItem::Text {
                    box_id,
                    text,
                    origin,
                    size,
                    color,
                    shadows,
                    ..
                } => {
                    let mut line = format!(
                        "text {box_id} {text:?} {color} {size}px at ({}, {})",
                        origin.0, origin.1,
                    );
                    for shadow in shadows {
                        let _ = write!(
                            line,
                            " + shadow {} blur {} at ({}, {})",
                            shadow.color, shadow.blur, shadow.offset.0, shadow.offset.1,
                        );
                    }
                    writeln!(out, "{line}")
                }
                DisplayItem::PushClip { box_id, path } => {
                    let (left, top, right, bottom) = path.bounds().unwrap_or((0.0, 0.0, 0.0, 0.0));
                    writeln!(
                        out,
                        "clip {box_id} to ({left}, {top}) {}×{}",
                        right - left,
                        bottom - top,
                    )
                }
                DisplayItem::PushTransform { box_id, matrix } => {
                    writeln!(out, "transform {box_id} by {matrix}")
                }
                DisplayItem::PushGroup { box_id, opacity } => {
                    writeln!(out, "group {box_id} at opacity {opacity}")
                }
                DisplayItem::PopClip => writeln!(out, "unclip"),
                DisplayItem::PopTransform => writeln!(out, "untransform"),
                DisplayItem::PopGroup => writeln!(out, "ungroup"),
            };
        }
        out
    }
}
