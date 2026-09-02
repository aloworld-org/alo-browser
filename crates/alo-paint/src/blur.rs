//! Blurring coverage.
//!
//! A shadow is the shape it comes from, softened. CSS says the softening is a
//! Gaussian whose standard deviation is half the blur radius the author wrote
//! — `box-shadow: 0 0 10px` fades over about ten pixels — and it says in the
//! same breath that three passes of a **box blur** are a close enough
//! approximation, which is what every engine actually does. Three passes of a
//! moving average converge on a Gaussian quickly, and each pass is one add and
//! one subtract per pixel rather than a kernel's worth of multiplies.
//!
//! # Why this works on coverage rather than on pixels
//!
//! Blurring colour means blurring four channels, and blurring them after they
//! have been composited blurs whatever was behind the shape too. Coverage is
//! one channel and it is the shape alone, so the shadow is blurred once and
//! then drawn in its colour — which is also why the same code softens a
//! rounded card and a letter.

use crate::coverage::Coverage;

/// Coverage softened by a blur radius, in pixels.
///
/// The result is **larger** than what went in: a blurred edge reaches past
/// where the sharp one was, and a mask that kept its old size would have cut
/// the shadow off square. A radius of zero comes back unchanged rather than
/// going three times round a loop that would do nothing.
pub fn blurred(coverage: &Coverage, radius: f32) -> Coverage {
    let sigma = standard_deviation(radius);
    let box_size = box_size(sigma);
    if coverage.is_empty() || box_size <= 1 {
        return coverage.clone();
    }
    // Three passes each reach `box_size / 2` outwards, and the mask has to be
    // big enough to hold all of it or the shadow ends at a straight edge.
    let margin = (box_size / 2).saturating_mul(3).saturating_add(1);
    let Some(grown) = grown(coverage, margin) else {
        return coverage.clone();
    };

    let width = grown.width();
    let height = grown.height();
    let mut data = grown.data().to_vec();
    // Horizontally three times, then vertically three times. A box blur is
    // separable, so six one-dimensional passes cost what one two-dimensional
    // pass of the same size would have cost squared.
    for _ in 0..3 {
        data = across(&data, width, height, box_size);
    }
    let mut turned = transpose(&data, width, height);
    for _ in 0..3 {
        turned = across(&turned, height, width, box_size);
    }
    data = transpose(&turned, height, width);
    Coverage::new(width, height, grown.origin(), data)
}

/// How far a blur reaches beyond the shape that cast it, in pixels.
///
/// The renderer needs this before it has the blurred mask, to know how much
/// room to leave; it is the same number this file grows the mask by.
pub fn reach(radius: f32) -> f32 {
    let box_size = box_size(standard_deviation(radius));
    if box_size <= 1 {
        return 0.0;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "a box size is at most a few hundred pixels"
    )]
    let reach = ((box_size / 2) * 3 + 1) as f32;
    reach
}

/// The Gaussian a CSS blur radius asks for.
///
/// Half the radius, which is the number the specification gives and the reason
/// a ten-pixel blur fades over about ten pixels rather than about twenty.
fn standard_deviation(radius: f32) -> f32 {
    if !radius.is_finite() || radius <= 0.0 {
        return 0.0;
    }
    // A blur wider than this is a broken value rather than a picture, and the
    // mask it would ask for is enormous.
    (radius / 2.0).min(256.0)
}

/// How wide each of the three box passes is.
///
/// The specification's own formula: three boxes of this width have the same
/// variance as the Gaussian being approximated. Odd, so that each pass is
/// centred on the pixel it is writing and the shadow does not creep sideways.
fn box_size(sigma: f32) -> u32 {
    if sigma <= 0.0 {
        return 0;
    }
    let ideal = sigma * 3.0 * (2.0 * core::f32::consts::PI).sqrt() / 4.0 + 0.5;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "sigma is clamped to at most 256, so this is a small positive number"
    )]
    let size = ideal as u32;
    if size % 2 == 0 { size + 1 } else { size }
}

/// The same coverage with blank pixels all round it, so a blur has room.
fn grown(coverage: &Coverage, margin: u32) -> Option<Coverage> {
    let width = coverage.width().checked_add(margin.checked_mul(2)?)?;
    let height = coverage.height().checked_add(margin.checked_mul(2)?)?;
    let count = (width as usize).checked_mul(height as usize)?;
    if count > 16 * 1024 * 1024 {
        return None;
    }
    let mut data = vec![0u8; count];
    for row in 0..coverage.height() {
        for column in 0..coverage.width() {
            let index = ((row + margin) as usize) * (width as usize) + ((column + margin) as usize);
            if let Some(pixel) = data.get_mut(index) {
                *pixel = coverage.at(column, row);
            }
        }
    }
    let margin = i32::try_from(margin).ok()?;
    let origin = (
        coverage.origin().0.checked_sub(margin)?,
        coverage.origin().1.checked_sub(margin)?,
    );
    Some(Coverage::new(width, height, origin, data))
}

/// One horizontal box-blur pass: every pixel becomes the average of the
/// `size` pixels centred on it.
///
/// A running sum rather than a sum per pixel, which is what makes the cost of
/// a blur independent of how wide it is.
fn across(data: &[u8], width: u32, height: u32, size: u32) -> Vec<u8> {
    let mut out = vec![0u8; data.len()];
    let width = width as usize;
    let half = (size / 2) as usize;
    let size = size as usize;
    for row in 0..height as usize {
        let start = row * width;
        let mut sum: u32 = 0;
        // Prime the window with everything that overlaps the first pixel; what
        // is off the left edge is blank, and blank is what it contributes.
        for column in 0..half.min(width) {
            sum += u32::from(data.get(start + column).copied().unwrap_or(0));
        }
        for column in 0..width {
            let entering = column + half;
            if entering < width {
                sum += u32::from(data.get(start + entering).copied().unwrap_or(0));
            }
            if let Some(pixel) = out.get_mut(start + column) {
                #[expect(clippy::cast_possible_truncation, reason = "a mean of bytes is a byte")]
                let mean = (sum / (size as u32)) as u8;
                *pixel = mean;
            }
            if column >= half {
                let leaving = column - half;
                sum -= u32::from(data.get(start + leaving).copied().unwrap_or(0));
            }
        }
    }
    out
}

/// The same values with rows and columns swapped, so that a horizontal pass
/// blurs vertically.
///
/// One pass written once rather than two nearly-identical ones: the second
/// would have been the first with the indices exchanged, and that is a bug
/// waiting for someone to fix only one of them.
fn transpose(data: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mut out = vec![0u8; data.len()];
    let (width, height) = (width as usize, height as usize);
    for row in 0..height {
        for column in 0..width {
            if let (Some(value), Some(pixel)) = (
                data.get(row * width + column),
                out.get_mut(column * height + row),
            ) {
                *pixel = *value;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::Path;
    use crate::raster::fill;

    fn square() -> Coverage {
        fill(&Path::rectangle(0.0, 0.0, 20.0, 20.0))
    }

    /// Everything the coverage covers, added up.
    fn total(coverage: &Coverage) -> u64 {
        coverage.data().iter().map(|value| u64::from(*value)).sum()
    }

    #[test]
    fn no_blur_leaves_the_shape_exactly_as_it_was() {
        let sharp = square();
        for radius in [0.0, -4.0, f32::NAN] {
            assert!(blurred(&sharp, radius) == sharp, "radius {radius}");
        }
        assert!((reach(0.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn blurring_nothing_is_nothing() {
        assert!(blurred(&Coverage::empty(), 10.0).is_empty());
    }

    #[test]
    fn a_blurred_shape_is_larger_than_the_one_it_came_from() {
        let sharp = square();
        let soft = blurred(&sharp, 8.0);
        assert!(soft.width() > sharp.width(), "it spread sideways");
        assert!(soft.height() > sharp.height());
        assert!(soft.origin().0 < sharp.origin().0, "and upwards and left");
        assert!(reach(8.0) > 0.0);
    }

    #[test]
    fn the_middle_stays_solid_and_the_edge_becomes_a_ramp() {
        let soft = blurred(&square(), 6.0);
        let (left, top) = soft.origin();
        // The middle of a shape much wider than the blur is untouched.
        let middle = soft.at(
            u32::try_from(10 - left).expect("inside"),
            u32::try_from(10 - top).expect("inside"),
        );
        assert!(middle > 250, "the middle is still solid, got {middle}");

        // Across the left edge, coverage rises and never falls.
        let row = u32::try_from(10 - top).expect("inside");
        let mut previous = 0;
        for x in 0..u32::try_from(10 - left).expect("inside") {
            let value = soft.at(x, row);
            assert!(
                value >= previous,
                "coverage fell at {x}: {previous} to {value}"
            );
            previous = value;
        }
        assert!(previous > 250);
    }

    #[test]
    fn blurring_softens_without_inventing_or_losing_much_coverage() {
        let sharp = square();
        let soft = blurred(&sharp, 6.0);
        let (before, after) = (total(&sharp), total(&soft));
        let difference = before.abs_diff(after);
        assert!(
            difference * 20 < before,
            "a blur moves coverage about rather than making it: {before} became {after}",
        );
    }

    #[test]
    fn a_blur_is_symmetrical() {
        let soft = blurred(&square(), 6.0);
        let (width, height) = (soft.width(), soft.height());
        for x in 0..width {
            let left = soft.at(x, height / 2);
            let right = soft.at(width - 1 - x, height / 2);
            assert!(
                left.abs_diff(right) <= 2,
                "the two sides differ at {x}: {left} against {right}",
            );
        }
    }

    #[test]
    fn a_blur_nobody_could_draw_is_refused_rather_than_asked_for() {
        let sharp = fill(&Path::rectangle(0.0, 0.0, 4000.0, 4000.0));
        let soft = blurred(&sharp, 512.0);
        assert!(
            soft.width() == sharp.width() && soft.height() == sharp.height(),
            "a mask that large comes back unblurred rather than asking for a gigabyte",
        );
    }
}
