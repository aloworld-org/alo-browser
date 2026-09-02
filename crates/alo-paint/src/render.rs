//! Drawing a display list onto a canvas.
//!
//! Every item becomes coverage and coverage becomes pixels. There is nothing
//! clever here on purpose: the decisions were made when the display list was
//! built, and this file only carries them out — which is what makes a picture
//! that came out wrong diagnosable from the list rather than from the pixels.

use crate::blur::blurred;
use crate::canvas::Canvas;
use crate::coverage::Coverage;
use crate::display::{DisplayItem, DisplayList};
use crate::glyph::outline;
use crate::paint::Paint;
use crate::path::{Path, Point};
use crate::raster::fill;
use alo_text::{Direction, Font, shape};

/// Draw a display list onto a canvas.
pub fn render(list: &DisplayList, canvas: &mut Canvas) {
    // A stack rather than one mask, because clips nest: a rounded card inside
    // a scrolling panel is clipped by both, and the answer is where they
    // overlap.
    let mut clips: Vec<Clip> = Vec::new();
    for item in list.items() {
        match item {
            DisplayItem::PushClip { path, .. } => {
                let mask = Clip::from(&fill(path));
                clips.push(match clips.last() {
                    Some(outer) => outer.intersected(&mask),
                    None => mask,
                });
            }
            DisplayItem::PopClip => {
                clips.pop();
            }
            DisplayItem::Fill { path, paint, .. } => {
                draw_coverage(canvas, &fill(path), paint, clips.last());
            }
            DisplayItem::Shadow {
                path, blur, color, ..
            } => {
                // The shape, softened. Coverage rather than colour, so what is
                // behind the shadow is not blurred along with it.
                let soft = blurred(&fill(path), *blur);
                draw_coverage(canvas, &soft, &Paint::Solid(*color), clips.last());
            }
            DisplayItem::Text {
                text,
                origin,
                font,
                size,
                color,
                shadows,
                ..
            } => {
                // The run is outlined once and drawn once per shadow plus
                // once for itself: shaping and outlining are the expensive
                // part, and a shadow is the same letters somewhere else.
                let outlined = outlined_run(text, font, *size, *origin);
                // Furthest back first, so the first shadow written ends up on
                // top — which is the order CSS draws them in.
                for shadow in shadows.iter().rev() {
                    let moved = outlined.translated(Point::new(shadow.offset.0, shadow.offset.1));
                    let soft = blurred(&fill(&moved), shadow.blur);
                    draw_coverage(canvas, &soft, &Paint::Solid(shadow.color), clips.last());
                }
                draw_coverage(
                    canvas,
                    &fill(&outlined),
                    &Paint::Solid(*color),
                    clips.last(),
                );
            }
        }
    }
}

/// Every glyph of a run, as one shape, with the pen where the run starts.
///
/// One path rather than one per glyph because a shadow is blurred from it: two
/// letters that touch would otherwise be blurred separately and composited on
/// top of one another, which is darker where they overlap.
fn outlined_run(text: &str, font: &Font, size: f32, origin: (f32, f32)) -> Path {
    let run = shape(text, font, size, Direction::LeftToRight);
    let mut whole = Path::new();
    let mut pen = origin;
    for glyph in &run.glyphs {
        if let Some(shaped) = outline(font, glyph.glyph_id, size) {
            let placed = shaped
                .path
                .translated(Point::new(pen.0 + glyph.offset.0, pen.1 + glyph.offset.1));
            whole.extend(&placed);
        }
        pen.0 += glyph.advance;
    }
    whole
}

/// Put a coverage mask onto the canvas, filled, inside whatever clip is in
/// force.
fn draw_coverage(canvas: &mut Canvas, coverage: &Coverage, paint: &Paint, clip: Option<&Clip>) {
    if coverage.is_empty() || paint.is_invisible() {
        return;
    }
    let flat = paint.solid();
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
            let color = flat.unwrap_or_else(|| paint.at(pixel_centre(x), pixel_centre(y)));
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
fn pixel_centre(position: u32) -> f32 {
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
fn place(base: i32, step: u32) -> Option<u32> {
    let step = i32::try_from(step).ok()?;
    u32::try_from(base.checked_add(step)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{PaintContext, build};
    use crate::path::Path;
    use alo_box::BoxId;
    use alo_value::Rgba;

    /// A display list holding one filled rectangle.
    fn one_rectangle(x: f32, y: f32, width: f32, height: f32, color: Rgba) -> DisplayList {
        // The list is built from a document everywhere else; this reaches in
        // so that the renderer can be tested without one.
        let mut list = DisplayList::default();
        list.push(DisplayItem::Fill {
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

    #[test]
    fn a_document_with_nothing_in_it_draws_nothing() {
        let fonts = alo_text::FontDatabase::new();
        let list = build(
            &alo_box::BoxTree::empty_for_tests(),
            &alo_layout::LayoutTree::default(),
            &alo_style::StyleTree::default(),
            PaintContext { fonts: &fonts },
        );
        assert!(list.is_empty());
    }
}
