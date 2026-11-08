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

use crate::fill_rule::FillRule;
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
        /// What counts as inside a shape that crosses itself. Non-zero for
        /// everything CSS draws; an SVG shape says (ADR 0022 § 3).
        rule: FillRule,
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
    /// A glyph drawn by a paint program of `COLR` version 1 (queue item
    /// 785): run on a surface of its own and drawn at the text colour's
    /// alpha, as a layered glyph is ([`crate::painted_glyph`]).
    PaintedGlyph {
        /// The box it belongs to.
        box_id: BoxId,
        /// Its program, in font units with `y` up.
        steps: std::sync::Arc<Vec<crate::colour_paint::Step>>,
        /// Where its pen is, on the baseline, in page coordinates.
        pen: (f32, f32),
        /// CSS pixels to a font unit.
        to_css: f32,
        /// How strongly it is drawn, of 255: the text colour's alpha.
        fade: u8,
    },
    /// The shadow a run of letters casts (queue item 831): each glyph's
    /// outline softened on its own and drawn over the others in one colour,
    /// as Chrome 157 draws a text shadow glyph by glyph — two letters 8 px
    /// apart blurred 16 meet darker than one shape would, and where two
    /// letters overlap a translucent shadow is darker still. Listed as a
    /// [`DisplayItem::Shadow`] of their outlines together is.
    LettersShadow {
        /// The box that cast it.
        box_id: BoxId,
        /// Each glyph's outline, already moved by the shadow's offset.
        glyphs: Vec<Path>,
        /// How far each edge fades over.
        blur: f32,
        /// What colour.
        color: Rgba,
    },
    /// The shadow one colour glyph casts (queue item 786): the glyph as it
    /// is drawn — a picture's alpha, its layers at their colours' opacity
    /// put over each other — as one shape, blurred on its own and drawn in
    /// one colour ([`crate::glyph_shadow`]). The letters beside it cast a
    /// [`DisplayItem::Shadow`] of their outlines.
    GlyphShadow {
        /// The box that cast it.
        box_id: BoxId,
        /// The glyph, already moved by the shadow's offset.
        cast: crate::glyph_shadow::Cast,
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
    /// Where a person's pointer finds a box: drawn as nothing.
    ///
    /// A box with no background is still somewhere a person can click, so
    /// what a box *occupies* cannot be read from what it fills. It goes in
    /// the list rather than beside it because which box is on top at a point
    /// is paint order, under the clips and transforms around it, and the list
    /// is where that is decided once ([`crate::hit`]). One per piece of a box
    /// broken across lines, as its background is.
    Area {
        /// The box it is.
        box_id: BoxId,
        /// The shape it occupies, in the coordinates of the transforms around
        /// it.
        path: Path,
    },
    /// A decoded picture, drawn into a rectangle.
    Picture {
        /// The box it belongs to.
        box_id: BoxId,
        /// Where it goes, in page coordinates.
        rect: Rect,
        /// The pixels, at their own size — scaled to `rect` when drawn.
        picture: std::sync::Arc<crate::canvas::Canvas>,
        /// How strongly it is drawn, of 255: a colour glyph's picture under
        /// text of a translucent colour is faded by that colour's alpha
        /// (queue item 830); every other picture is drawn whole, at 255.
        fade: u8,
    },
    /// Where a picture drawn beneath the canvas shows through (queue item
    /// 869, ADR 0100): every pixel of the rectangle under the transform in
    /// force, inside the clip in force, made clear by as much as a picture
    /// there would have covered it ([`crate::beneath`]).
    Beneath {
        /// The box whose picture is drawn beneath.
        box_id: BoxId,
        /// Where the picture stands, in page coordinates.
        rect: Rect,
    },
    /// What the page drew behind a box before it, blurred, and drawn where
    /// the box's background goes (`backdrop-filter: blur()`, queue item
    /// 885, ADR 0104): read from `rect`'s crop under the transform in force
    /// ([`crate::backdrop::crop_of`]) and nowhere else, blurred by a
    /// standard deviation of `blur` CSS pixels grown with that transform
    /// ([`crate::backdrop`]), and drawn where `path` covers inside the clip
    /// in force. Before the box's own shadows and background, which are
    /// drawn over it.
    Backdrop {
        /// The box whose backdrop it is.
        box_id: BoxId,
        /// Its border box, as laid out, in page coordinates: what is read.
        rect: Rect,
        /// Its border box with its radii, as its background is filled: where
        /// it is drawn.
        path: Path,
        /// The blur's standard deviation, in CSS pixels.
        blur: f32,
    },
    /// A glyph drawn in layers of colour (`COLR` version 0, queue item 766):
    /// its layers put over each other on a surface of their own, as
    /// Chrome's rasteriser makes a glyph's picture, and that drawn at the
    /// text colour's alpha — faded once under a translucent colour, whole
    /// under an opaque one (queue item 830, [`crate::glyph_fade`]).
    LayeredGlyph {
        /// The box it belongs to.
        box_id: BoxId,
        /// Each layer's outline, in page coordinates, and its colour —
        /// the text's own, at its own opacity, for a layer asking for it —
        /// bottom first.
        layers: Vec<(Path, Rgba)>,
        /// How strongly the glyph is drawn, of 255: the text colour's alpha.
        fade: u8,
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
        /// How far the font's letters are leaned, in degrees, because the
        /// page asked for a slant the font has no face for
        /// ([`crate::synthesis`]). Nothing for a face drawn as it is.
        oblique: f32,
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
            | DisplayItem::LettersShadow { box_id, .. }
            | DisplayItem::GlyphShadow { box_id, .. }
            | DisplayItem::Picture { box_id, .. }
            | DisplayItem::Beneath { box_id, .. }
            | DisplayItem::Backdrop { box_id, .. }
            | DisplayItem::LayeredGlyph { box_id, .. }
            | DisplayItem::PaintedGlyph { box_id, .. }
            | DisplayItem::Text { box_id, .. }
            | DisplayItem::Area { box_id, .. }
            | DisplayItem::PushClip { box_id, .. }
            | DisplayItem::PushTransform { box_id, .. }
            | DisplayItem::PushGroup { box_id, .. } => Some(*box_id),
            DisplayItem::PopClip | DisplayItem::PopTransform | DisplayItem::PopGroup => None,
        }
    }
}

/// Everything to draw, in the order to draw it.
///
/// In CSS pixels at every scale (ADR 0057 § 4), and **built for one**: what
/// it snapped onto whole pixels it snapped onto the device pixels of a
/// screen at [`DisplayList::scale`], and [`crate::render`] draws it at that
/// scale, so a list cannot be drawn at a scale it was not built for.
#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    items: Vec<DisplayItem>,
    /// What the styles asked to be drawn and this engine did not draw.
    issues: Vec<alo_css::StyleIssue>,
    /// Device pixels per CSS pixel on the screen it was built for. Read it
    /// with [`DisplayList::scale`], which makes the zero of a default list
    /// one.
    scale: f32,
}

impl DisplayList {
    /// The items, in paint order.
    pub fn items(&self) -> &[DisplayItem] {
        &self.items
    }

    /// What the styles asked to be drawn and was not, box by box: a
    /// background that could not be read, or one layer of it that could not
    /// be drawn.
    pub fn issues(&self) -> &[alo_css::StyleIssue] {
        &self.issues
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

    /// Device pixels per CSS pixel on the screen this list was built for,
    /// and is drawn at: 1 for a list built for no screen.
    pub fn scale(&self) -> f32 {
        alo_value::device_scale(self.scale)
    }

    /// Take a list's items, put in order, what was refused while building
    /// it, and the scale it was built for.
    pub(crate) fn from_parts(
        items: Vec<DisplayItem>,
        issues: Vec<alo_css::StyleIssue>,
        scale: f32,
    ) -> Self {
        Self {
            items,
            issues,
            scale: alo_value::device_scale(scale),
        }
    }

    /// The list as one line per item, for a test to compare.
    ///
    /// This is what makes a picture that changed *diagnosable*: the line says
    /// which box, what colour and where, so a failure reads "the row's
    /// background moved four pixels" rather than "the image differs".
    ///
    /// An [`DisplayItem::Area`] has no line: it draws nothing, and the
    /// outline is of what is drawn. Where a point falls is asserted through
    /// [`crate::hit`] instead.
    pub fn to_outline(&self) -> String {
        let mut out = String::new();
        for item in &self.items {
            // Writing to a `String` cannot fail.
            let _ = match item {
                DisplayItem::Area { .. } => Ok(()),
                DisplayItem::Picture { .. }
                | DisplayItem::LayeredGlyph { .. }
                | DisplayItem::PaintedGlyph { .. } => writeln!(out, "{}", drawn_line(item)),
                DisplayItem::Beneath { box_id, rect } => writeln!(
                    out,
                    "beneath {box_id} at ({}, {}) {}×{}",
                    rect.left(),
                    rect.top(),
                    rect.right() - rect.left(),
                    rect.bottom() - rect.top(),
                ),
                DisplayItem::Backdrop {
                    box_id, rect, blur, ..
                } => writeln!(
                    out,
                    "backdrop {box_id} blur {blur} at ({}, {}) {}×{}",
                    rect.left(),
                    rect.top(),
                    rect.right() - rect.left(),
                    rect.bottom() - rect.top(),
                ),
                DisplayItem::Fill {
                    box_id,
                    path,
                    paint,
                    rule,
                } => {
                    let (left, top, right, bottom) = path.bounds().unwrap_or((0.0, 0.0, 0.0, 0.0));
                    // The rule is named only when it is not the one everything
                    // CSS fills uses, so a list of boxes reads as it always has.
                    let rule = match rule {
                        FillRule::NonZero => "",
                        FillRule::EvenOdd => " evenodd",
                    };
                    writeln!(
                        out,
                        "fill {box_id} {paint}{rule} at ({left}, {top}) {}×{}",
                        right - left,
                        bottom - top,
                    )
                }
                DisplayItem::LettersShadow {
                    box_id,
                    glyphs,
                    blur,
                    color,
                } => writeln!(
                    out,
                    "{}",
                    shadow_line(*box_id, *color, *blur, bounds_of(glyphs.iter()))
                ),
                DisplayItem::Shadow {
                    box_id,
                    path,
                    blur,
                    color,
                } => writeln!(
                    out,
                    "{}",
                    shadow_line(*box_id, *color, *blur, bounds_of([path].into_iter()))
                ),
                DisplayItem::GlyphShadow {
                    box_id,
                    cast,
                    blur,
                    color,
                } => {
                    let (left, top, right, bottom) = cast.bounds().unwrap_or((0.0, 0.0, 0.0, 0.0));
                    writeln!(
                        out,
                        "shadow {box_id} {color} blur {blur} at ({left}, {top}) {}×{} of {}",
                        right - left,
                        bottom - top,
                        cast.kind(),
                    )
                }
                DisplayItem::Text { .. } => writeln!(out, "{}", text_line(item)),
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

/// A text item's line in [`DisplayList::to_outline`]; empty for anything
/// else.
/// How strongly a picture or a glyph is drawn, in words — nothing when it
/// is drawn whole, so every list of a page's pictures reads as it did.
fn faded(fade: u8) -> String {
    if fade == u8::MAX {
        String::new()
    } else {
        format!(" faded to {fade}/255")
    }
}

/// A picture's or a colour glyph's line: what it is, where, and how
/// strongly it is drawn.
fn drawn_line(item: &DisplayItem) -> String {
    match item {
        DisplayItem::Picture {
            box_id,
            rect,
            picture,
            fade,
        } => format!(
            "picture {box_id} {}×{} at ({}, {}) {}×{}{}",
            picture.width(),
            picture.height(),
            rect.left(),
            rect.top(),
            rect.right() - rect.left(),
            rect.bottom() - rect.top(),
            faded(*fade),
        ),
        DisplayItem::LayeredGlyph {
            box_id,
            layers,
            fade,
        } => glyph_line(*box_id, layers, *fade),
        DisplayItem::PaintedGlyph {
            box_id,
            steps,
            pen,
            fade,
            ..
        } => format!(
            "glyph {box_id} painted in {} steps at ({}, {}){}",
            steps.len(),
            pen.0,
            pen.1,
            faded(*fade)
        ),
        _ => String::new(),
    }
}

/// Where some shapes reach together, as `(left, top, right, bottom)`; all
/// noughts for none.
fn bounds_of<'a>(paths: impl Iterator<Item = &'a Path>) -> (f32, f32, f32, f32) {
    paths
        .filter_map(Path::bounds)
        .reduce(|one, other| {
            (
                one.0.min(other.0),
                one.1.min(other.1),
                one.2.max(other.2),
                one.3.max(other.3),
            )
        })
        .unwrap_or((0.0, 0.0, 0.0, 0.0))
}

/// A shadow's line: its colour, its blur, and where its shape reaches.
fn shadow_line(
    box_id: BoxId,
    color: Rgba,
    blur: f32,
    (left, top, right, bottom): (f32, f32, f32, f32),
) -> String {
    format!(
        "shadow {box_id} {color} blur {blur} at ({left}, {top}) {}×{}",
        right - left,
        bottom - top,
    )
}

/// A layered glyph's line: its layers, where they reach and how strongly.
fn glyph_line(box_id: BoxId, layers: &[(Path, Rgba)], fade: u8) -> String {
    let (left, top, right, bottom) = bounds_of(layers.iter().map(|(path, _)| path));
    format!(
        "glyph {box_id} {} layers at ({left}, {top}) {}×{}{}",
        layers.len(),
        right - left,
        bottom - top,
        faded(fade),
    )
}

fn text_line(item: &DisplayItem) -> String {
    let DisplayItem::Text {
        box_id,
        text,
        origin,
        size,
        color,
        shadows,
        oblique,
        ..
    } = item
    else {
        return String::new();
    };
    let mut line = format!(
        "text {box_id} {text:?} {color} {size}px at ({}, {})",
        origin.0, origin.1,
    );
    // Said only when it is there, so a list of upright text reads as it
    // always has.
    if *oblique != 0.0 {
        let _ = write!(line, " leaned {oblique}deg");
    }
    for shadow in shadows {
        let _ = write!(
            line,
            " + shadow {} blur {} at ({}, {})",
            shadow.color, shadow.blur, shadow.offset.0, shadow.offset.1,
        );
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_built_for_no_screen_is_drawn_at_one() {
        assert_eq!(DisplayList::default().scale().to_bits(), 1.0_f32.to_bits());
        let at_two = DisplayList::from_parts(Vec::new(), Vec::new(), 2.0);
        assert_eq!(at_two.scale().to_bits(), 2.0_f32.to_bits());
        for scale in [0.0, -2.0, f32::NAN, f32::INFINITY] {
            let list = DisplayList::from_parts(Vec::new(), Vec::new(), scale);
            assert_eq!(list.scale().to_bits(), 1.0_f32.to_bits(), "{scale}");
        }
    }
}
