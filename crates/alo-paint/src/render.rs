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
use alo_value::{Matrix, Rgba};

/// Draw a display list onto a canvas.
pub fn render(list: &DisplayList, canvas: &mut Canvas) {
    // Three stacks, because three things nest. A clip applies to a subtree; a
    // transform applies to a subtree and composes with the ones outside it; a
    // group is a subtree drawn somewhere else and composited back.
    let mut clips: Vec<Clip> = Vec::new();
    let mut transforms: Vec<Matrix> = Vec::new();
    let mut groups: Vec<(f32, Canvas)> = Vec::new();
    let (width, height) = (canvas.width(), canvas.height());

    for item in list.items() {
        let current = transforms.last().copied().unwrap_or(Matrix::IDENTITY);
        match item {
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
            DisplayItem::Fill { path, paint, .. } => {
                let target = groups.last_mut().map_or(&mut *canvas, |(_, group)| group);
                draw_coverage(
                    target,
                    &fill(&moved(path, current)),
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
                let target = groups.last_mut().map_or(&mut *canvas, |(_, group)| group);
                // Furthest back first, so the first shadow written ends up on
                // top — which is the order CSS draws them in.
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
                        clips.last(),
                    );
                }
                draw_coverage(
                    target,
                    &fill(&moved(&outlined, current)),
                    &Paint::Solid(*color),
                    current,
                    clips.last(),
                );
            }
        }
    }
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
