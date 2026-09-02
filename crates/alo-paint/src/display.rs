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
use alo_value::Rgba;
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

/// One thing to draw.
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
    /// A run of text.
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
            | DisplayItem::Text { box_id, .. }
            | DisplayItem::PushClip { box_id, .. } => Some(*box_id),
            DisplayItem::PopClip => None,
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
                DisplayItem::PopClip => writeln!(out, "unclip"),
            };
        }
        out
    }
}
