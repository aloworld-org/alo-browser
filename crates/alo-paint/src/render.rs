/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Drawing a display list onto a canvas.
//!
//! Every item becomes coverage and coverage becomes pixels. There is nothing
//! clever here on purpose: the decisions were made when the display list was
//! built, and this file only carries them out — which is what makes a picture
//! that came out wrong diagnosable from the list rather than from the pixels.
//!
//! # At a scale
//!
//! A list is in CSS pixels and the canvas is in device pixels: on a screen
//! of `s` device pixels to the CSS pixel the caller hands a canvas `s` times
//! the page's size, and the whole list is drawn under a growth of `s` that
//! every transform in it composes with (ADR 0057 § 4). Nothing is drawn
//! small and enlarged: a shape is grown before it is filled, a letter is
//! outlined at the size its face is set at on that screen and shaped as
//! layout shaped it there, and a blur grows with the transform in force as
//! it always has. At a scale of one the growth is the identity, so nothing
//! drawn at one moves.

use crate::blur::blurred;
use crate::canvas::Canvas;
use crate::clip::{Clip, mask};
use crate::clipped_picture::draw_clipped_picture;
use crate::coverage::Coverage;
use crate::display::{DisplayItem, DisplayList};
use crate::drawn_picture::draw_faded_picture;
use crate::drawn_text::draw_text;
use crate::extent::{Reach, reach_of};
use crate::fill_rule::FillRule;
use crate::glyph::outline_at;
use crate::kept::{Kept, Key, Made};
use crate::paint::Paint;
use crate::path::{Path, Point};
use crate::picture_filter::Uploads;
use crate::raster::{fill, fill_on_page};
use crate::region::Region;
use alo_text::{Direction, Font, shape_at};
use alo_value::{Matrix, Rgba};
use std::sync::Arc;

/// Draw a display list onto a canvas, at the scale the list was built for
/// ([`DisplayList::scale`]): a canvas of device pixels, the page's size in
/// CSS pixels times the scale.
pub fn render(list: &DisplayList, canvas: &mut Canvas) {
    draw(list, canvas, None);
}

/// The same, taking every shape's coverage, shadow and clip it can from
/// what the drawing before it kept, and keeping what this one makes for
/// the next (queue item 871, [`crate::kept`]): the same pixels, since a
/// kept coverage is the very coverage that would have been made.
pub fn render_keeping(list: &DisplayList, canvas: &mut Canvas, kept: &mut Kept) {
    kept.begin();
    draw(list, canvas, Some(kept));
    kept.end();
}

/// A clip in force, and what it is kept as when coverage is kept.
struct Held {
    clip: Arc<Clip>,
    key: Option<Key>,
}

/// The clip in force, innermost of those open.
fn innermost(clips: &[Held]) -> Option<&Clip> {
    clips.last().map(|held| held.clip.as_ref())
}

/// [`render`], keeping coverage in `kept` when there is one. The caller
/// begins and ends the drawing in it.
pub(crate) fn draw(list: &DisplayList, canvas: &mut Canvas, mut kept: Option<&mut Kept>) {
    // The screen's growth is the transform outside every other, so a box's
    // own transform happens in CSS pixels and lands in device pixels.
    let screen = Matrix::scaling(list.scale());
    // Three stacks, because three things nest. A clip applies to a subtree; a
    // transform applies to a subtree and composes with the ones outside it; a
    // group is a subtree drawn somewhere else and composited back.
    let mut clips: Vec<Held> = Vec::new();
    let mut transforms: Vec<Matrix> = Vec::new();
    let mut groups: Vec<(f32, Canvas)> = Vec::new();
    // What Chrome's decode cache holds by the time each picture is drawn:
    // whether a copy has mipmap levels is decided by its first draw (queue
    // item 179).
    let mut uploads = Uploads::default();
    let (width, height) = (canvas.width(), canvas.height());
    let confined = canvas.confinement();
    let mut seen = Seen::of(confined, (width, height));

    for item in list.items() {
        let current = transforms.last().copied().unwrap_or(screen);
        if seen.passes_over(item, current, list.scale()) {
            // Not drawn, but still this list's: what it was made as is
            // kept for the next drawing.
            touch(kept.as_deref_mut(), item, current, (width, height));
            continue;
        }
        match item {
            // Where a pointer finds a box, which is nothing to draw.
            DisplayItem::Area { .. } => {}
            DisplayItem::Picture {
                rect,
                picture,
                fade,
                ..
            } => {
                let target = groups.last_mut().map_or(&mut *canvas, |(_, under)| under);
                let placed = (*rect, current, *fade);
                draw_picture_item(target, picture, placed, innermost(&clips), &mut uploads);
            }
            DisplayItem::Beneath { rect, .. } => {
                let covered = filled(
                    kept.as_deref_mut(),
                    &moved(&rectangle(*rect), current),
                    FillRule::NonZero,
                    (width, height),
                );
                clear_beneath(canvas, &mut groups, &covered, innermost(&clips));
            }
            DisplayItem::PaintedGlyph { .. } | DisplayItem::LayeredGlyph { .. } => {
                let target = groups.last_mut().map_or(&mut *canvas, |(_, under)| under);
                draw_colour_glyph(target, item, current, innermost(&clips));
            }
            DisplayItem::PushTransform { matrix, .. } => {
                // The inner transform happens first, in the coordinates the
                // outer one has already established.
                transforms.push(matrix.then(current));
            }
            DisplayItem::PopTransform => {
                transforms.pop();
            }
            DisplayItem::PushGroup { opacity, .. } => {
                // A surface of its own, transparent, the size of the page: the
                // group is drawn whole and faded once.
                let mut group = Canvas::new(width, height, Rgba::TRANSPARENT);
                group.confine(confined);
                groups.push((*opacity, group));
            }
            DisplayItem::PopGroup => {
                if let Some((opacity, group)) = groups.pop() {
                    let target = groups.last_mut().map_or(&mut *canvas, |(_, under)| under);
                    target.draw_over(&group, opacity);
                }
            }
            DisplayItem::PushClip { path, .. } => {
                let shape = moved(path, current);
                let unseen = seen.push(&shape);
                clips.push(clipped(clips.last(), &shape, unseen, kept.as_deref_mut()));
            }
            DisplayItem::PopClip => {
                clips.pop();
                seen.pop();
            }
            DisplayItem::Fill { .. } | DisplayItem::Backdrop { .. } => {
                let target = groups.last_mut().map_or(&mut *canvas, |(_, group)| group);
                let (clip, kept) = (innermost(&clips), kept.as_deref_mut());
                if matches!(item, DisplayItem::Backdrop { .. }) {
                    draw_backdrop(target, item, current, clip, kept);
                } else {
                    draw_fill(target, item, current, clip, kept);
                }
            }
            DisplayItem::Shadow { .. }
            | DisplayItem::LettersShadow { .. }
            | DisplayItem::GlyphShadow { .. } => {
                let target = groups.last_mut().map_or(&mut *canvas, |(_, group)| group);
                draw_shadow_item(
                    target,
                    item,
                    current,
                    innermost(&clips),
                    kept.as_deref_mut(),
                );
            }
            DisplayItem::Text { .. } => {
                let target = groups.last_mut().map_or(&mut *canvas, |(_, group)| group);
                let misses = |reach: &dyn Fn() -> Reach| seen.misses_by(reach);
                draw_text(
                    target,
                    item,
                    (current, list.scale()),
                    innermost(&clips),
                    &misses,
                    kept.as_deref_mut(),
                );
            }
        }
    }
}

/// The clip in force inside `outer` once `shape`, already under the
/// transform in force, is pushed. A clip the confined region cannot see
/// (`unseen`) lets nothing through there, which a clip of no pixels says
/// without making its mask. With coverage kept, its mask and the clips it
/// is combined with are taken as kept where they can be.
fn clipped(outer: Option<&Held>, shape: &Path, unseen: bool, kept: Option<&mut Kept>) -> Held {
    let Some(kept) = kept else {
        let mask = if unseen {
            Clip::from(&Coverage::empty())
        } else {
            Clip::from(&mask(shape))
        };
        let clip = match outer {
            Some(outer) => outer.clip.intersected(&mask),
            None => mask,
        };
        return Held {
            clip: Arc::new(clip),
            key: None,
        };
    };
    let own = Key::of(Made::Mask, &[], shape);
    let key = Key::within(outer.and_then(|outer| outer.key.as_ref()), &own);
    if unseen {
        kept.touch(&own);
        kept.touch(&key);
        return Held {
            clip: Arc::new(Clip::from(&Coverage::empty())),
            key: Some(key),
        };
    }
    let made = kept.coverage(&own, Made::Mask, || mask(shape));
    let clip = kept.clip(&key, || {
        let mask = Clip::from(&made);
        match outer {
            Some(outer) => outer.clip.intersected(&mask),
            None => mask,
        }
    });
    Held {
        clip,
        key: Some(key),
    }
}

/// A shape filled on a page by a rule, as kept when it was, and kept when
/// coverage is.
fn filled(
    kept: Option<&mut Kept>,
    shape: &Path,
    rule: FillRule,
    page: (u32, u32),
) -> Arc<Coverage> {
    match kept {
        Some(kept) => kept.coverage(&filled_key(shape, rule, page), Made::Filled, || {
            fill_on_page(shape, rule, page)
        }),
        None => Arc::new(fill_on_page(shape, rule, page)),
    }
}

fn filled_key(shape: &Path, rule: FillRule, page: (u32, u32)) -> Key {
    let rule = match rule {
        FillRule::NonZero => 0,
        FillRule::EvenOdd => 1,
    };
    Key::of(Made::Filled, &[rule, page.0, page.1], shape)
}

/// A box's shadow: its shape filled and blurred by `radius` device pixels,
/// as kept when it was, and kept when coverage is.
fn shadowed(kept: Option<&mut Kept>, shape: &Path, radius: f32) -> Arc<Coverage> {
    match kept {
        Some(kept) => kept.coverage(&shadow_key(shape, radius), Made::Blurred, || {
            blurred(&fill(shape), radius)
        }),
        None => Arc::new(blurred(&fill(shape), radius)),
    }
}

fn shadow_key(shape: &Path, radius: f32) -> Key {
    Key::of(Made::Blurred, &[radius.to_bits()], shape)
}

/// A colour glyph: its program run on a surface of its own (queue item
/// 785), or its layers over each other and then faded once (queue item
/// 830), inside the clip in force. Anything else draws nothing.
fn draw_colour_glyph(
    target: &mut Canvas,
    item: &DisplayItem,
    current: Matrix,
    clip: Option<&Clip>,
) {
    let inside = |x, y| clip.map_or(u8::MAX, |clip| clip.at(x, y));
    match item {
        DisplayItem::PaintedGlyph {
            steps,
            pen,
            to_css,
            fade,
            ..
        } => {
            let placement = font_units(*pen, *to_css).then(current);
            crate::painted_glyph::draw_painted(target, steps, placement, *fade, &inside);
        }
        DisplayItem::LayeredGlyph { layers, fade, .. } => {
            crate::glyph_fade::draw_faded_layers(target, layers, *fade, current, &inside);
        }
        _ => {}
    }
}

/// A shape filled, under the transform in force, inside the clip in force.
/// Only what lands on the page is made, so a shape a stranger wrote a
/// million pixels wide costs a page of mask, not more. Anything else draws
/// nothing.
fn draw_fill(
    target: &mut Canvas,
    item: &DisplayItem,
    current: Matrix,
    clip: Option<&Clip>,
    kept: Option<&mut Kept>,
) {
    let DisplayItem::Fill {
        path, paint, rule, ..
    } = item
    else {
        return;
    };
    let page = (target.width(), target.height());
    let covered = filled(kept, &moved(path, current), *rule, page);
    draw_coverage(target, &covered, paint, current, clip);
}

/// A box's backdrop (queue item 885, ADR 0104): its crop read from
/// `target` — the canvas, or the group drawn into — blurred by its
/// standard deviation grown with the transform in force, or taken as the
/// last drawing kept it while nothing under it changed, and drawn where its
/// shape covers inside the clip in force. Anything else draws nothing.
fn draw_backdrop(
    target: &mut Canvas,
    item: &DisplayItem,
    current: Matrix,
    clip: Option<&Clip>,
    mut kept: Option<&mut Kept>,
) {
    let DisplayItem::Backdrop {
        rect, path, blur, ..
    } = item
    else {
        return;
    };
    let size = (target.width(), target.height());
    let Some(crop) = crate::extent::crop_under(*rect, current, size) else {
        return;
    };
    let covered = filled(
        kept.as_deref_mut(),
        &moved(path, current),
        FillRule::NonZero,
        size,
    );
    let sigma = blur * current.scale_factor();
    let make = || crate::backdrop::blurred(target, crop, sigma);
    let blurred = match kept {
        Some(kept) => kept.backdrop(&crate::backdrop::key(crop, sigma), make),
        None => Arc::new(make()),
    };
    crate::backdrop::draw(target, crop, &blurred, &covered, clip);
}

/// A rectangle as a shape.
fn rectangle(rect: alo_layout::Rect) -> Path {
    Path::rectangle(rect.left(), rect.top(), rect.size.width, rect.size.height)
}

/// An item passed over on a confined canvas, kept for the next drawing as
/// what it was made as, when coverage is kept.
fn touch(kept: Option<&mut Kept>, item: &DisplayItem, current: Matrix, page: (u32, u32)) {
    if let Some(kept) = kept {
        for key in keys_of(item, current, page) {
            kept.touch(&key);
        }
    }
}

/// What an item passed over would have been kept as: its fill, its shadow
/// or the place it leaves clear.
fn keys_of(item: &DisplayItem, current: Matrix, page: (u32, u32)) -> Vec<Key> {
    match item {
        DisplayItem::Fill { path, rule, .. } => {
            vec![filled_key(&moved(path, current), *rule, page)]
        }
        DisplayItem::Shadow { path, blur, .. } => vec![shadow_key(
            &moved(path, current),
            blur * current.scale_factor(),
        )],
        DisplayItem::Beneath { rect, .. } => vec![filled_key(
            &moved(&rectangle(*rect), current),
            FillRule::NonZero,
            page,
        )],
        DisplayItem::Backdrop {
            rect, path, blur, ..
        } => {
            let mut keys = vec![filled_key(&moved(path, current), FillRule::NonZero, page)];
            if let Some(crop) = crate::extent::crop_under(*rect, current, page) {
                keys.push(crate::backdrop::key(crop, blur * current.scale_factor()));
            }
            keys
        }
        _ => Vec::new(),
    }
}

/// What a confined canvas's clips let through of its region.
enum Open {
    /// Every pixel: the canvas is not confined.
    Everywhere,
    /// The pixels of this region.
    Within(Region),
    /// No pixel.
    Nowhere,
}

/// On a canvas confined to a region (queue item 870, [`crate::repaint`]),
/// what of the region each clip in force still lets through: an item that
/// cannot reach that is passed over, and a clip that cannot reach it is
/// never made. Nothing is passed over on a canvas that is not confined.
struct Seen {
    region: Option<Region>,
    size: (u32, u32),
    /// What each clip pushed lets through of the region, [`None`] nothing.
    through: Vec<Option<Region>>,
}

impl Seen {
    fn of(region: Option<Region>, size: (u32, u32)) -> Self {
        Self {
            region,
            size,
            through: Vec::new(),
        }
    }

    /// What of the region the clips in force let through.
    fn open(&self) -> Open {
        match (self.region, self.through.last()) {
            (None, _) => Open::Everywhere,
            (Some(region), None) => Open::Within(region),
            (Some(_), Some(Some(through))) => Open::Within(*through),
            (Some(_), Some(None)) => Open::Nowhere,
        }
    }

    /// Whether `item`, drawn under `current` on a screen of `scale`, can be
    /// passed over: it changes no pixel the clips let through of the
    /// region. Only a shape, a shadow and a place left clear are asked —
    /// what is costly to make and plain to bound ([`crate::extent`]); a run
    /// of text asks once it is outlined ([`crate::drawn_text`], queue item
    /// 888); anything else is drawn, and its pixels outside the region are
    /// passed over by the canvas itself.
    fn passes_over(&self, item: &DisplayItem, current: Matrix, scale: f32) -> bool {
        matches!(
            item,
            DisplayItem::Fill { .. }
                | DisplayItem::Shadow { .. }
                | DisplayItem::LettersShadow { .. }
                | DisplayItem::Beneath { .. }
                | DisplayItem::Backdrop { .. }
        ) && self.misses_by(|| reach_of(item, current, scale, self.size))
    }

    /// Whether what `reach` says can be changed misses every pixel the
    /// clips let through of the region — never on a canvas not confined —
    /// the reach asked only when the region is open somewhere.
    fn misses_by(&self, reach: impl FnOnce() -> Reach) -> bool {
        match self.open() {
            Open::Everywhere => false,
            Open::Nowhere => true,
            Open::Within(open) => reach()
                .region(self.size)
                .and_then(|reach| reach.meeting(open))
                .is_none(),
        }
    }

    /// A clip of `shape`, already under the transform in force, pushed:
    /// whether the region cannot see it at all.
    fn push(&mut self, shape: &Path) -> bool {
        let open = match self.open() {
            Open::Everywhere => return false,
            Open::Nowhere => None,
            Open::Within(open) => Some(open),
        };
        let reach = shape
            .bounds()
            .and_then(|bounds| Region::around(bounds, self.size));
        let through = open
            .zip(reach)
            .and_then(|(open, reach)| open.meeting(reach));
        self.through.push(through);
        through.is_none()
    }

    /// The last clip popped.
    fn pop(&mut self) {
        if self.region.is_some() {
            self.through.pop();
        }
    }
}

/// A picture drawn beneath the canvas (queue item 869, ADR 0100): its
/// rectangle under the transform in force, inside the clip in force, made
/// clear on the canvas and on every group drawn through, so what is beneath
/// the canvas shows through all of them.
fn clear_beneath(
    canvas: &mut Canvas,
    groups: &mut [(f32, Canvas)],
    covered: &Coverage,
    clip: Option<&Clip>,
) {
    let inside = |x, y| clip.map_or(u8::MAX, |clip| clip.at(x, y));
    crate::beneath::clear(canvas, covered, &inside);
    for (_, group) in groups {
        crate::beneath::clear(group, covered, &inside);
    }
}

/// A picture item: drawn into its rectangle under the transform in force at
/// its fade, and inside a clip only what the clip lets through kept (queue
/// item 717).
fn draw_picture_item(
    target: &mut Canvas,
    picture: &std::sync::Arc<Canvas>,
    (rect, current, fade): (alo_layout::Rect, Matrix, u8),
    clip: Option<&Clip>,
    uploads: &mut Uploads,
) {
    match clip {
        None => draw_faded_picture(target, picture, rect, current, uploads, fade),
        Some(clip) => draw_clipped_picture(
            target,
            picture,
            rect,
            current,
            uploads,
            &|x, y| clip.at(x, y),
            fade,
        ),
    }
}

/// A shadow item: a box's or a line's shape, a run's letters glyph by
/// glyph, or a colour glyph as it is drawn — each softened, coverage rather
/// than colour so that what is behind it is not blurred along with it, the
/// radius grown with the transform, because a shadow is blurred in the
/// box's own coordinates and then moved with it. Anything else draws
/// nothing.
fn draw_shadow_item(
    target: &mut Canvas,
    item: &DisplayItem,
    current: Matrix,
    clip: Option<&Clip>,
    kept: Option<&mut Kept>,
) {
    let (soft, color) = match item {
        DisplayItem::Shadow {
            path, blur, color, ..
        } => (
            shadowed(kept, &moved(path, current), blur * current.scale_factor()),
            *color,
        ),
        DisplayItem::LettersShadow {
            glyphs,
            blur,
            color,
            ..
        } => {
            draw_letters_shadow(target, glyphs, (*blur, *color), current, clip);
            return;
        }
        // One shape — layers and pictures as they are drawn — softened once
        // (queue item 786).
        DisplayItem::GlyphShadow {
            cast, blur, color, ..
        } => (
            Arc::new(blurred(
                &crate::glyph_shadow::coverage(cast, current),
                blur * current.scale_factor(),
            )),
            *color,
        ),
        _ => return,
    };
    draw_coverage(target, &soft, &Paint::Solid(color), Matrix::IDENTITY, clip);
}

/// A run's shadow, glyph by glyph (queue item 831): each glyph's outline
/// under the transform in force, softened by `blur` grown with it, and drawn
/// in the shadow's colour over the glyphs before it — as Chrome 157 draws
/// one, so two letters' soft edges add where they meet and a translucent
/// shadow is darker where two letters overlap.
pub(crate) fn draw_letters_shadow(
    target: &mut Canvas,
    glyphs: &[Path],
    (blur, color): (f32, Rgba),
    current: Matrix,
    clip: Option<&Clip>,
) {
    for glyph in glyphs {
        let soft = blurred(&fill(&moved(glyph, current)), blur * current.scale_factor());
        draw_coverage(target, &soft, &Paint::Solid(color), Matrix::IDENTITY, clip);
    }
}

/// A glyph's font units, `y` up, where its pen is in CSS pixels: grown by
/// `to_css` and turned over.
pub fn font_units(pen: (f32, f32), to_css: f32) -> Matrix {
    Matrix {
        a: to_css,
        b: 0.0,
        c: 0.0,
        d: -to_css,
        e: pen.0,
        f: pen.1,
    }
}

/// A shape where the transform in force puts it.
///
/// A path is built in the page's coordinates, so a box with no transform over
/// it is left exactly alone — which is almost every box, and is why this asks
/// before it copies.
pub(crate) fn moved(path: &Path, transform: Matrix) -> Path {
    if transform.is_identity() {
        return path.clone();
    }
    path.transformed(transform)
}

/// Every glyph of a run, as one shape, with the pen where the run starts.
///
/// A shadow is not cast from it: Chrome softens each glyph's shadow on its
/// own and puts them over each other, darker where two letters overlap
/// (queue item 831), so a shadow is cast from [`outlined_glyphs_at`].
///
/// Public because the browser's own words are drawn with it too: `alo-window`
/// says what happened to a tab in a sentence that belongs to no document, and
/// a second copy of how a run becomes a shape would be a second place for the
/// letters to land somewhere else.
pub fn outlined_run(
    text: &str,
    font: &Font,
    size: f32,
    letter_spacing: f32,
    origin: (f32, f32),
) -> Path {
    outlined_run_at(text, font, size, 1.0, letter_spacing, origin)
}

/// The same run as set on a screen of `scale` device pixels to the CSS
/// pixel, still in CSS pixels: shaped and outlined at the
/// size its face is set at there ([`alo_text::face_size_at`]), as layout
/// measured it, or a line laid out at two is drawn with one's advances.
pub fn outlined_run_at(
    text: &str,
    font: &Font,
    size: f32,
    scale: f32,
    letter_spacing: f32,
    origin: (f32, f32),
) -> Path {
    let mut whole = Path::new();
    for glyph in outlined_glyphs_at(text, font, size, scale, letter_spacing, origin) {
        whole.extend(&glyph);
    }
    whole
}

/// The same run, one shape per glyph, each where it is drawn: what a line
/// under the text steps aside for, glyph by glyph ([`crate::skip_ink`]).
/// A glyph with nothing to draw is an empty shape, so there is one per
/// glyph the run was shaped to.
pub fn outlined_glyphs_at(
    text: &str,
    font: &Font,
    size: f32,
    scale: f32,
    letter_spacing: f32,
    origin: (f32, f32),
) -> Vec<Path> {
    // The same spacing the line was measured with, or the letters land where
    // the line did not put them.
    let run = alo_text::spaced(
        shape_at(text, font, size, scale, Direction::LeftToRight),
        letter_spacing,
    );
    let mut glyphs = Vec::with_capacity(run.glyphs.len());
    let mut pen = origin;
    for glyph in &run.glyphs {
        // A glyph drawn in layers of colour is drawn as its layers, never as
        // its own plain outline (`glyph_layers.rs`, queue item 766).
        if crate::glyph::layers_at(font, glyph.glyph_id, size, scale).is_some()
            || crate::glyph::painted(font, glyph.glyph_id, (size, scale), Rgba::BLACK).is_some()
        {
            glyphs.push(Path::new());
            pen.0 += glyph.advance;
            continue;
        }
        glyphs.push(outline_at(font, glyph.glyph_id, size, scale).map_or_else(
            Path::new,
            |shaped| {
                shaped
                    .path
                    .translated(Point::new(pen.0 + glyph.offset.0, pen.1 + glyph.offset.1))
            },
        ));
        pen.0 += glyph.advance;
    }
    glyphs
}

/// How far the pen moves over a run [`outlined_glyphs_at`] draws: the sum of
/// the advances it puts each glyph at, so the next run drawn after it starts
/// where this one ended ([`crate::ink_runs`]).
pub fn run_width_at(text: &str, font: &Font, size: f32, scale: f32, letter_spacing: f32) -> f32 {
    alo_text::spaced(
        shape_at(text, font, size, scale, Direction::LeftToRight),
        letter_spacing,
    )
    .glyphs
    .iter()
    .map(|glyph| glyph.advance)
    .sum()
}

/// Put a coverage mask onto the canvas, filled, inside whatever clip is in
/// force.
pub(crate) fn draw_coverage(
    canvas: &mut Canvas,
    coverage: &Coverage,
    paint: &Paint,
    transform: Matrix,
    clip: Option<&Clip>,
) {
    if coverage.is_empty() || paint.is_invisible() {
        return;
    }
    // A gradient is measured in the box's own coordinates, so a transformed
    // box asks where the pixel *came from* rather than where it is. A
    // transform that flattens the box onto a line covers nothing, so there is
    // nothing to ask.
    let flat = paint.solid();
    let back = if flat.is_some() || transform.is_identity() {
        Some(Matrix::IDENTITY)
    } else {
        transform.inverted()
    };
    let Some(back) = back else {
        return;
    };
    let (left, top) = coverage.origin();
    // Only the rows and columns a confined canvas would keep (queue item
    // 870): the others would be passed over pixel by pixel all the same.
    let (rows, columns) = match canvas.confinement() {
        Some(region) => (
            span(top, coverage.height(), (region.top, region.bottom)),
            span(left, coverage.width(), (region.left, region.right)),
        ),
        None => (0..coverage.height(), 0..coverage.width()),
    };
    // A flat colour, which is most boxes, a row at a time (queue item 892):
    // the same blend at every pixel, without asking where each one is.
    if let Some(color) = flat {
        // The first column that lands on the page, left of which nothing is.
        let on_page = u32::try_from(-i64::from(left)).unwrap_or(0);
        let first = columns.start.max(on_page);
        let width = coverage.width() as usize;
        for row in rows {
            let Some(y) = place(top, row) else {
                continue;
            };
            let (Some(x), Some(line)) = (
                place(left, first),
                coverage.data().get(
                    row as usize * width + first as usize
                        ..row as usize * width + columns.end as usize,
                ),
            ) else {
                continue;
            };
            canvas.blend_run((x, y), line, color, |x| {
                clip.map_or(255, |clip| clip.at(x, y))
            });
        }
        return;
    }
    for row in rows {
        for column in columns.clone() {
            let value = coverage.at(column, row);
            if value == 0 {
                continue;
            }
            let (Some(x), Some(y)) = (place(left, column), place(top, row)) else {
                continue;
            };
            // A gradient is asked at the middle of the pixel; a flat fill is
            // not asked at all, which is most boxes.
            let color = flat.unwrap_or_else(|| {
                let (local_x, local_y) = back.apply(pixel_centre(x), pixel_centre(y));
                paint.at(local_x, local_y)
            });
            // A clip multiplies coverage rather than switching it on and off,
            // so the edge of a rounded clip is as smooth as the shape it came
            // from.
            let inside = clip.map_or(255, |clip| clip.at(x, y));
            if inside == 0 {
                continue;
            }
            let combined = crate::clip::multiply(value, inside);
            canvas.blend(x, y, color, combined);
        }
    }
}

/// The middle of a pixel, which is where a gradient is sampled: sampling at
/// the corner would shift the whole gradient half a pixel.
pub(crate) fn pixel_centre(position: u32) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a canvas is at most a few thousand pixels across"
    )]
    let middle = position as f32 + 0.5;
    middle
}

/// The steps of a mask `length` long from `origin` that land on the page
/// between `from` and `to`.
pub(crate) fn span(origin: i32, length: u32, (from, to): (u32, u32)) -> core::ops::Range<u32> {
    let clamp = |at: u32| {
        let step = (i64::from(at) - i64::from(origin)).clamp(0, i64::from(length));
        u32::try_from(step).unwrap_or(0)
    };
    clamp(from)..clamp(to).max(clamp(from))
}

/// Where a pixel of the mask lands on the canvas, or [`None`] when it is off
/// the left or top edge.
pub(crate) fn place(base: i32, step: u32) -> Option<u32> {
    let step = i32::try_from(step).ok()?;
    u32::try_from(base.checked_add(step)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{PaintContext, build};
    use crate::fill_rule::FillRule;
    use crate::path::Path;
    use alo_box::BoxId;
    use alo_value::Rgba;

    /// A display list holding one filled rectangle.
    fn one_rectangle(x: f32, y: f32, width: f32, height: f32, color: Rgba) -> DisplayList {
        // The list is built from a document everywhere else; this reaches in
        // so that the renderer can be tested without one.
        let mut list = DisplayList::default();
        list.push(DisplayItem::Fill {
            rule: FillRule::NonZero,
            box_id: BoxId::from_index_for_tests(0),
            path: Path::rectangle(x, y, width, height),
            paint: Paint::Solid(color),
        });
        list
    }

    #[test]
    fn an_empty_list_leaves_the_canvas_alone() {
        let mut canvas = Canvas::new(2, 2, Rgba::WHITE);
        render(&DisplayList::default(), &mut canvas);
        assert_eq!(canvas.at(0, 0), Some(Rgba::WHITE));
    }

    /// `coverage` blended pixel by pixel through `clip` in `color`: what
    /// every flat fill was before queue item 892.
    fn pixel_by_pixel(canvas: &mut Canvas, coverage: &Coverage, color: Rgba, clip: Option<&Clip>) {
        let (left, top) = coverage.origin();
        for row in 0..coverage.height() {
            for column in 0..coverage.width() {
                let value = coverage.at(column, row);
                let (Some(x), Some(y)) = (place(left, column), place(top, row)) else {
                    continue;
                };
                let inside = clip.map_or(255, |clip| clip.at(x, y));
                canvas.blend(x, y, color, crate::clip::multiply(value, inside));
            }
        }
    }

    #[test]
    fn a_flat_fill_a_row_at_a_time_is_blended_pixel_by_pixel() {
        // Queue item 892: coverage of every value, hanging off each edge of
        // the canvas, through a clip and none, onto a canvas confined and
        // not, opaque and translucent — every pixel as blended one by one.
        let mut state = 892_u32;
        let mut next = move || {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            u8::try_from((state >> 16) & 0xff).unwrap_or(0)
        };
        let values: Vec<u8> = (0..30 * 20)
            .map(|k| match k % 4 {
                0 => 0,
                1 => u8::MAX,
                _ => next(),
            })
            .collect();
        let clip = Clip::from(&Coverage::new(
            16,
            12,
            (4, 3),
            (0..16 * 12).map(|k| [0, 90, u8::MAX][k % 3]).collect(),
        ));
        let ground = Rgba::from_rgba8(30, 160, 90, 200);
        for origin in [(-6, -4), (3, 2), (10, 9), (-40, 0)] {
            let coverage = Coverage::new(30, 20, origin, values.clone());
            for color in [
                Rgba::from_rgba8(200, 30, 30, 255),
                Rgba::from_rgba8(20, 30, 200, 120),
            ] {
                for through in [None, Some(&clip)] {
                    for confined in [None, Region::new(2, 5, 17, 14)] {
                        let mut rows = Canvas::new(24, 18, ground);
                        rows.confine(confined);
                        let mut pixels = rows.clone();
                        draw_coverage(
                            &mut rows,
                            &coverage,
                            &Paint::Solid(color),
                            Matrix::IDENTITY,
                            through,
                        );
                        pixel_by_pixel(&mut pixels, &coverage, color, through);
                        assert_eq!(rows, pixels, "{origin:?} {color} {confined:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_rectangle_is_drawn_where_it_says() {
        let mut canvas = Canvas::new(4, 4, Rgba::WHITE);
        render(&one_rectangle(1.0, 1.0, 2.0, 2.0, Rgba::BLACK), &mut canvas);
        assert_eq!(canvas.at(0, 0), Some(Rgba::WHITE));
        assert_eq!(canvas.at(1, 1), Some(Rgba::BLACK));
        assert_eq!(canvas.at(2, 2), Some(Rgba::BLACK));
        assert_eq!(canvas.at(3, 3), Some(Rgba::WHITE));
    }

    #[test]
    fn a_rectangle_off_the_canvas_draws_the_part_that_is_on_it() {
        let mut canvas = Canvas::new(2, 2, Rgba::WHITE);
        render(
            &one_rectangle(-1.0, -1.0, 2.0, 2.0, Rgba::BLACK),
            &mut canvas,
        );
        assert_eq!(canvas.at(0, 0), Some(Rgba::BLACK));
        assert_eq!(canvas.at(1, 1), Some(Rgba::WHITE));
    }

    #[test]
    fn an_invisible_colour_draws_nothing() {
        let mut canvas = Canvas::new(2, 2, Rgba::WHITE);
        render(
            &one_rectangle(0.0, 0.0, 2.0, 2.0, Rgba::TRANSPARENT),
            &mut canvas,
        );
        assert_eq!(canvas.at(0, 0), Some(Rgba::WHITE));
    }

    #[test]
    fn a_half_covered_edge_is_a_blend_rather_than_on_or_off() {
        let mut canvas = Canvas::new(2, 2, Rgba::WHITE);
        render(&one_rectangle(0.0, 0.0, 2.0, 0.5, Rgba::BLACK), &mut canvas);
        let top = canvas.at(0, 0).expect("a pixel");
        assert!(
            top.red > 0.1 && top.red < 0.9,
            "the half-covered row is grey, not black or white: {top}",
        );
        assert_eq!(canvas.at(0, 1), Some(Rgba::WHITE));
    }

    /// One run of text as a display list built for a screen at `scale`.
    fn one_run(text: &str, font: &Font, size: f32, scale: f32) -> DisplayList {
        let item = DisplayItem::Text {
            box_id: BoxId::from_index_for_tests(0),
            text: text.to_owned(),
            origin: (2.0, 20.0),
            font: font.clone(),
            oblique: 0.0,
            size,
            letter_spacing: 0.0,
            color: Rgba::BLACK,
            shadows: Vec::new(),
        };
        DisplayList::from_parts(vec![item], Vec::new(), scale)
    }

    #[test]
    fn at_two_a_run_is_set_and_drawn_as_it_is_on_that_screen() {
        let Some(font) = Font::load(
            "DejaVu Sans",
            alo_text::Weight::NORMAL,
            alo_text::Slant::Normal,
            dejavu::sans::regular().to_vec(),
        ) else {
            panic!("the font this crate is tested with");
        };
        // `13.337px` is set at 13.335 on a screen at two and 13.33 at one
        // (ADR 0057, *Measured since*), so a hundred `x` end 0.3 CSS pixels
        // apart. Drawn at two, the letters are the run as set at two, grown
        // by two: device pixel for device pixel, the same coverage.
        let text = "x".repeat(100);
        let mut drawn = Canvas::new(1700, 60, Rgba::WHITE);
        render(&one_run(&text, &font, 13.337, 2.0), &mut drawn);
        let screen = Matrix::scaling(2.0);
        let mut expected = Canvas::new(1700, 60, Rgba::WHITE);
        let set_at_two = outlined_run_at(&text, &font, 13.337, 2.0, 0.0, (2.0, 20.0));
        let paint = Paint::Solid(Rgba::BLACK);
        let ink = fill(&moved(&set_at_two, screen));
        draw_coverage(&mut expected, &ink, &paint, screen, None);
        let mut differing = 0;
        for y in 0..60 {
            for x in 0..1700 {
                if drawn.at(x, y) != expected.at(x, y) {
                    differing += 1;
                }
            }
        }
        assert_eq!(differing, 0);
        // And set at one, the run would end a little over half a device
        // pixel short of where it does.
        let set_at_one = outlined_run_at(&text, &font, 13.337, 1.0, 0.0, (2.0, 20.0));
        let (Some(two), Some(one)) = (set_at_two.bounds(), set_at_one.bounds()) else {
            panic!("both runs have ink");
        };
        let short = (two.2 - one.2) * 2.0;
        assert!((0.5..0.7).contains(&short), "{short}");
        // Its letters are outlined at that size too: as tall as one `x`
        // set at 13.335.
        let x = shape_at("x", &font, 13.337, 2.0, Direction::LeftToRight);
        let Some(letter) = x
            .glyphs
            .first()
            .and_then(|glyph| outline_at(&font, glyph.glyph_id, 13.337, 2.0))
            .and_then(|glyph| glyph.path.bounds())
        else {
            panic!("an x has a shape");
        };
        assert!(
            (two.1 - (20.0 + letter.1)).abs() < 1.0e-4,
            "{two:?} {letter:?}"
        );
    }

    #[test]
    fn a_document_with_nothing_in_it_draws_nothing() {
        let fonts = alo_text::FontDatabase::new();
        let list = build(
            &alo_box::BoxTree::empty_for_tests(),
            &alo_layout::LayoutTree::default(),
            &alo_style::StyleTree::default(),
            PaintContext {
                fonts: &fonts,
                pictures: &std::collections::BTreeMap::new(),
                drawings: &std::collections::BTreeMap::new(),
                beneath: &std::collections::BTreeSet::new(),
            },
        );
        assert!(list.is_empty());
    }
}
