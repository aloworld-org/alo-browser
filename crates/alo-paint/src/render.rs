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
use crate::clipped_picture::draw_clipped_picture;
use crate::coverage::Coverage;
use crate::display::{DisplayItem, DisplayList};
use crate::drawn_picture::draw_picture;
use crate::glyph::outline_at;
use crate::paint::Paint;
use crate::path::{Path, Point};
use crate::picture_filter::Uploads;
use crate::raster::{fill, fill_on_page};
use crate::synthesis::leaned;
use alo_text::{Direction, Font, shape_at};
use alo_value::{Matrix, Rgba};

/// Draw a display list onto a canvas, at the scale the list was built for
/// ([`DisplayList::scale`]): a canvas of device pixels, the page's size in
/// CSS pixels times the scale.
pub fn render(list: &DisplayList, canvas: &mut Canvas) {
    // The screen's growth is the transform outside every other, so a box's
    // own transform happens in CSS pixels and lands in device pixels.
    let screen = Matrix::scaling(list.scale());
    // Three stacks, because three things nest. A clip applies to a subtree; a
    // transform applies to a subtree and composes with the ones outside it; a
    // group is a subtree drawn somewhere else and composited back.
    let mut clips: Vec<Clip> = Vec::new();
    let mut transforms: Vec<Matrix> = Vec::new();
    let mut groups: Vec<(f32, Canvas)> = Vec::new();
    // What Chrome's decode cache holds by the time each picture is drawn:
    // whether a copy has mipmap levels is decided by its first draw (queue
    // item 179).
    let mut uploads = Uploads::default();
    let (width, height) = (canvas.width(), canvas.height());

    for item in list.items() {
        let current = transforms.last().copied().unwrap_or(screen);
        match item {
            // Where a pointer finds a box, which is nothing to draw.
            DisplayItem::Area { .. } => {}
            DisplayItem::Picture { rect, picture, .. } => {
                let target = groups.last_mut().map_or(&mut *canvas, |(_, under)| under);
                // Inside a clip, only what the clip lets through is kept
                // (queue item 717).
                match clips.last() {
                    None => draw_picture(target, picture, *rect, current, &mut uploads),
                    Some(clip) => draw_clipped_picture(
                        target,
                        picture,
                        *rect,
                        current,
                        &mut uploads,
                        &|x, y| clip.at(x, y),
                    ),
                }
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
                groups.push((*opacity, Canvas::new(width, height, Rgba::TRANSPARENT)));
            }
            DisplayItem::PopGroup => {
                if let Some((opacity, group)) = groups.pop() {
                    let target = groups.last_mut().map_or(&mut *canvas, |(_, under)| under);
                    target.draw_over(&group, opacity);
                }
            }
            DisplayItem::PushClip { path, .. } => {
                let mask = Clip::from(&fill(&moved(path, current)));
                clips.push(match clips.last() {
                    Some(outer) => outer.intersected(&mask),
                    None => mask,
                });
            }
            DisplayItem::PopClip => {
                clips.pop();
            }
            DisplayItem::Fill {
                path, paint, rule, ..
            } => {
                let target = groups.last_mut().map_or(&mut *canvas, |(_, group)| group);
                // Only what lands on the page is made, so a shape a stranger
                // wrote a million pixels wide costs a page of mask, not more.
                draw_coverage(
                    target,
                    &fill_on_page(&moved(path, current), *rule, (width, height)),
                    paint,
                    current,
                    clips.last(),
                );
            }
            DisplayItem::Shadow {
                path, blur, color, ..
            } => {
                // The shape, softened. Coverage rather than colour, so what is
                // behind the shadow is not blurred along with it. The radius
                // grows with the transform, because a shadow is blurred in the
                // box's own coordinates and then moved with it.
                let soft = blurred(&fill(&moved(path, current)), blur * current.scale_factor());
                let target = groups.last_mut().map_or(&mut *canvas, |(_, group)| group);
                draw_coverage(
                    target,
                    &soft,
                    &Paint::Solid(*color),
                    Matrix::IDENTITY,
                    clips.last(),
                );
            }
            DisplayItem::Text { .. } => {
                let target = groups.last_mut().map_or(&mut *canvas, |(_, group)| group);
                draw_text(target, item, current, clips.last(), list.scale());
            }
        }
    }
}

/// A text item and the shadows it casts, under the transform in force, its
/// letters set as they are on a screen at `scale`. Anything that is not a
/// text item draws nothing.
fn draw_text(
    target: &mut Canvas,
    item: &DisplayItem,
    current: Matrix,
    clip: Option<&Clip>,
    scale: f32,
) {
    let DisplayItem::Text {
        text,
        origin,
        font,
        oblique,
        size,
        letter_spacing,
        color,
        shadows,
        ..
    } = item
    else {
        return;
    };
    // The run is outlined once and drawn once per shadow plus once for
    // itself: shaping and outlining are the expensive part, and a shadow is
    // the same letters somewhere else. A lean is part of the letters, so a
    // shadow leans with them.
    let outlined = letters(text, font, *size, *letter_spacing, *origin, *oblique, scale);
    // Furthest back first, so the first shadow written ends up on top — which
    // is the order CSS draws them in.
    for shadow in shadows.iter().rev() {
        let placed = outlined.translated(Point::new(shadow.offset.0, shadow.offset.1));
        let soft = blurred(
            &fill(&moved(&placed, current)),
            shadow.blur * current.scale_factor(),
        );
        draw_coverage(
            target,
            &soft,
            &Paint::Solid(shadow.color),
            Matrix::IDENTITY,
            clip,
        );
    }
    draw_coverage(
        target,
        &fill(&moved(&outlined, current)),
        &Paint::Solid(*color),
        current,
        clip,
    );
}

/// A shape where the transform in force puts it.
///
/// A path is built in the page's coordinates, so a box with no transform over
/// it is left exactly alone — which is almost every box, and is why this asks
/// before it copies.
fn moved(path: &Path, transform: Matrix) -> Path {
    if transform.is_identity() {
        return path.clone();
    }
    path.transformed(transform)
}

/// A text item's letters as one shape: its run outlined as it is set on a
/// screen at `scale`, then leaned by however far the face had to be
/// ([`crate::synthesis`]).
fn letters(
    text: &str,
    font: &Font,
    size: f32,
    letter_spacing: f32,
    origin: (f32, f32),
    oblique: f32,
    scale: f32,
) -> Path {
    leaned(
        &outlined_run_at(text, font, size, scale, letter_spacing, origin),
        oblique,
        origin.1,
    )
}

/// Every glyph of a run, as one shape, with the pen where the run starts.
///
/// One path rather than one per glyph because a shadow is blurred from it: two
/// letters that touch would otherwise be blurred separately and composited on
/// top of one another, which is darker where they overlap.
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

/// Put a coverage mask onto the canvas, filled, inside whatever clip is in
/// force.
fn draw_coverage(
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
    for row in 0..coverage.height() {
        for column in 0..coverage.width() {
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
            let combined = multiply(value, inside);
            canvas.blend(x, y, color, combined);
        }
    }
}

/// A clip, as coverage over the whole page.
///
/// Held in page coordinates rather than relative to the clipping box, because
/// what it is asked is always "is this pixel inside" — and answering that from
/// a shape with its own origin means an offset at every lookup.
#[derive(Debug, Clone)]
struct Clip {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl Clip {
    fn from(coverage: &Coverage) -> Self {
        let (left, top) = coverage.origin();
        Self {
            left,
            top,
            width: coverage.width(),
            height: coverage.height(),
            data: coverage.data().to_vec(),
        }
    }

    /// How much of a page pixel this clip lets through.
    fn at(&self, x: u32, y: u32) -> u8 {
        let (Some(column), Some(row)) = (back(x, self.left), back(y, self.top)) else {
            return 0;
        };
        if column >= self.width || row >= self.height {
            return 0;
        }
        let index = (row as usize) * (self.width as usize) + (column as usize);
        self.data.get(index).copied().unwrap_or(0)
    }

    /// Where this clip and another overlap.
    fn intersected(&self, inner: &Self) -> Self {
        let mut data = Vec::with_capacity(inner.data.len());
        for row in 0..inner.height {
            for column in 0..inner.width {
                let value = inner
                    .data
                    .get((row as usize) * (inner.width as usize) + (column as usize))
                    .copied()
                    .unwrap_or(0);
                let (Some(x), Some(y)) = (place(inner.left, column), place(inner.top, row)) else {
                    data.push(0);
                    continue;
                };
                data.push(multiply(value, self.at(x, y)));
            }
        }
        Self {
            left: inner.left,
            top: inner.top,
            width: inner.width,
            height: inner.height,
            data,
        }
    }
}

/// Two coverages combined, as a fraction of a fraction.
fn multiply(left: u8, right: u8) -> u8 {
    let product = u32::from(left) * u32::from(right);
    // Rounded rather than truncated, so that full coverage stays full.
    u8::try_from((product + 127) / 255).unwrap_or(255)
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

/// A page position back into a mask's own coordinates.
fn back(position: u32, origin: i32) -> Option<u32> {
    u32::try_from(i64::from(position) - i64::from(origin)).ok()
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
            },
        );
        assert!(list.is_empty());
    }
}
