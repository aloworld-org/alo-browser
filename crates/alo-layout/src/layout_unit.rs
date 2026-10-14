/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The grid layout is placed on: 1/64 of a device pixel (ADR 0054, ADR 0057).
//!
//! Chrome lays out in a fixed-point unit of a sixty-fourth of a pixel, and
//! the measure is Chrome's picture, so this engine places what it lays out on
//! the same grid. **The pixel is the screen's**: at a scale of two the grid
//! is 1/128 of a CSS pixel, and every rule here rounds `v·s` and divides back
//! (ADR 0057 § 2), so every function takes the scale and every answer is
//! still in CSS pixels. At a scale of one nothing differs from ADR 0054. How a number reaches the grid depends on what it is, and
//! each way is one function here, so that the constant and the rules live in
//! one file rather than in every reader:
//!
//! - **A length, a percentage and a `calc()`** are [`truncated`] toward
//!   zero, each on its own (ADR 0054 §§ 1 and 2).
//! - **A run of text's width** is [`rounded_up`], so a box that shrinks to
//!   its text always holds it (§ 4).
//! - **What layout shares out** puts each edge on the grid: [`edges`] (§ 3).
//! - **A set line height** keeps ADR 0050's rules, [`rounded`] and
//!   [`floored`], which Chrome applies there and which disagree with
//!   truncation; `crate::line_height` reads them from here.
//!
//! A border width is floored to whole device pixels where it is read
//! (`crate::line_width`), and a whole device pixel is 64 units, so it is on
//! the grid already.
//!
//! **A length or a text width within four of a float's own steps of a grid
//! line is on it.** This engine computes a length in
//! `f32`, where Chrome computes it in double before it becomes a float:
//! `0.53em` at 100 px is 52.999996 here and 53 there, and truncated those
//! are 3,391 units and 3,392. Without the slack a length meant to be whole
//! would lose a unit to the order of two multiplications. The slack is
//! relative, 2⁻²¹ of the number, because a float's error is: the nearest to
//! a line that Chrome was measured to keep off it is a percentage of a
//! percentage at 710.9993 units, which it truncates to 710, and four steps
//! of a float there are 0.0003 of a unit. **An edge gets no slack**: Chrome
//! shares free space in float, as `taffy` does, and three `1fr` tracks in
//! 184 px end at 12,287.999 units in both and are floored to 12,287, a unit
//! short of the container, which ADR 0054 § 3 records. ADR 0050's rules for
//! a line height, [`rounded`] and [`floored`], are kept exactly as that
//! decision measured them.
//!
//! **At a scale that is not a power of two, an edge and a number's line
//! height are on the grid line they are within a sixteenth of a unit of.**
//! At one and at two a float holds every grid line, so a sum that misses
//! one misses it in Chrome too. At 1.5 the grid is 1/96 of a CSS pixel,
//! which no float holds: a box's edge Chrome puts at 4,832 units, counting
//! in whole sixty-fourths of a device pixel, comes out of this engine's
//! float at 4,831.99, and floored that is a unit short. Measured in Chrome
//! 154 at `--force-device-scale-factor=1.5` over the corpus (queue item
//! 593): of 985 boxes exact at one, 183 were off at 1.5 before, 80 with
//! this for an edge, and none of the nine left is for it; a line height
//! written as a number was a unit short in 2 of 121 probes. A length and a
//! text width keep their relative slack alone: given this one as well, a
//! length moved 3 boxes the corpus had right and a text width 7 more
//! ([`alo_value::device_pixels`]).
//!
//! **Past the grid's range a number saturates**, as Chrome's does: it holds
//! 2³¹ − 1 units, about 33,554,432 px, and reports the largest of them as
//! the largest 32-bit float below 2³¹ (ADR 0054 § 5). This engine lays out
//! in `f32`, which holds every sixty-fourth exactly only up to 262,144 px;
//! beyond that the grid is as fine as the float allows. A number that is not
//! a number is left alone: nothing here invents one.

/// How many layout units make a device pixel.
pub(crate) const UNITS_PER_PX: f32 = 64.0;

/// The largest number of units Chrome reports, as `getBoundingClientRect`
/// gives it: the largest `f32` below 2³¹.
const LIMIT: f32 = 2_147_483_392.0;

/// How near a grid line a number is taken to be on it, as a fraction of the
/// number: four steps of an `f32`, whose own step is 2⁻²³ of it.
const SLACK: f32 = 1.0 / 2_097_152.0;

/// Pixels truncated toward zero onto the grid of a screen at `scale`, as a
/// length, a percentage and a `calc()` are.
///
/// `0.3px` is 19 units, `-0.3px` is −19, and `19.92px` is 1,274; at a scale
/// of two `0.31px` is 39/128, where at one it is 38/128.
pub fn truncated(pixels: f32, scale: f32) -> f32 {
    from_units(on_a_line(saturated(pixels * per_px(scale))).trunc(), scale)
}

/// Pixels rounded up onto the grid of a screen at `scale`, as a run of
/// text's width is.
///
/// `W` at 13 px measures 785.28 units and its box is 786; a width already on
/// the grid stays where it is.
pub fn rounded_up(pixels: f32, scale: f32) -> f32 {
    from_units(on_a_line(saturated(pixels * per_px(scale))).ceil(), scale)
}

/// Pixels rounded to the nearest unit of a screen at `scale`, a half going
/// up: a line height written as a length or a percentage (ADR 0050 § 3).
pub fn rounded(pixels: f32, scale: f32) -> f32 {
    from_units((saturated(pixels * per_px(scale)) + 0.5).floor(), scale)
}

/// Pixels floored onto the grid of a screen at `scale`: a line height
/// written as a number, once it is multiplied out (ADR 0050 § 3).
pub fn floored(pixels: f32, scale: f32) -> f32 {
    from_units(saturated(on_the_grid(pixels, scale)).floor(), scale)
}

/// Where a box's near edge goes on the grid, and how big it then is, from
/// where an algorithm put it relative to its parent and how big it said.
///
/// **The near edge is truncated toward zero**, which is how Chrome places a
/// share of free space: a 10 px box centred in 100.3 px is 2,889 units in,
/// where half is 2,889.5, and a 20 px box centred in 10.3 px is −310, not
/// −311 (ADR 0054 § 3).
///
/// **The size is the distance between the two edges, each floored**, so
/// that flexible grid tracks end at their ideal edges floored, as Chrome's
/// do: seven `1fr` in 201 px are 1,837, 1,838, 1,838, 1,837 and so on, each
/// edge the ideal one floored. For a box on the near side of its parent this
/// is the same as truncating both edges, which is how ADR 0054 § 6 says it.
/// Below zero the two differ, and flooring is the one that leaves a size
/// already on the grid as it is: the overflowing 20 px box above stays
/// 1,280 units wide, as it is in Chrome, where truncating its far edge too
/// would make it 1,279.
///
/// Both on the grid of a screen at `scale`.
pub fn edges(near: f32, size: f32, scale: f32) -> (f32, f32) {
    let start = saturated(on_the_grid(near, scale));
    let end = saturated(on_the_grid(near + size, scale));
    let placed = from_units(start.trunc(), scale);
    let sized = from_units(end.floor() - start.floor(), scale);
    (placed, sized)
}

/// A number of units, moved onto the grid line it is within [`SLACK`] of.
fn on_a_line(units: f32) -> f32 {
    let nearest = units.round();
    if (units - nearest).abs() <= units.abs() * SLACK {
        nearest
    } else {
        units
    }
}

/// Pixels as units of a screen at `scale`, on the grid line they are within
/// a float's error of where a float cannot hold the grid
/// ([`alo_value::device_pixels`]).
fn on_the_grid(pixels: f32, scale: f32) -> f32 {
    alo_value::device_pixels(pixels, scale) * UNITS_PER_PX
}

/// A number of units held inside Chrome's range.
fn saturated(units: f32) -> f32 {
    units.clamp(-LIMIT, LIMIT)
}

/// How many units make a CSS pixel on a screen at `scale`: 64 of a device
/// pixel, and one unless the scale is a scale (ADR 0057 § 3).
fn per_px(scale: f32) -> f32 {
    UNITS_PER_PX * alo_value::device_scale(scale)
}

fn from_units(units: f32, scale: f32) -> f32 {
    units / per_px(scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `units` sixty-fourths of a pixel, as measured in Chrome 154.
    fn sixty_fourths(units: f32) -> f32 {
        units / UNITS_PER_PX
    }

    /// The same number, to the bit: every result here is a whole number of
    /// sixty-fourths, which an `f32` holds exactly in this range.
    fn same(left: f32, right: f32) -> bool {
        left.to_bits() == right.to_bits()
    }

    #[test]
    fn a_length_is_truncated_toward_zero() {
        // ADR 0054 § 1's table, each as Chrome 154 measured it.
        for (pixels, units) in [
            (0.3, 19.0),
            (0.6, 38.0),
            (9.6, 614.0),
            (19.92, 1274.0),
            (10.015, 640.0),
            (10.007_812_5, 640.0),
            (0.011_718_75, 0.0),
            (-0.3, -19.0),
            (-19.92, -1274.0),
            (-0.1, -6.0),
        ] {
            assert!(
                same(truncated(pixels, 1.0), sixty_fourths(units)),
                "{pixels} px is {units} units, not {}",
                truncated(pixels, 1.0) * UNITS_PER_PX
            );
        }
    }

    #[test]
    fn a_length_a_float_holds_just_short_of_the_grid_is_on_it() {
        // `0.53em` at 100 px, in `f32`: 52.999996, where a double gives 53
        // and 3,392 units.
        let whole = 0.53_f32 * 100.0;
        assert!(whole < 53.0, "the float this guards against: {whole}");
        assert!(same(truncated(whole, 1.0), 53.0));
        assert!(same(rounded_up(7.000_000_5, 1.0), 7.0));
        // A quarter of a unit is not a float's error, and is truncated; nor
        // is the seven ten-thousandths a percentage of a percentage leaves
        // short of 711, which Chrome truncates to 710 (ADR 0054 § 2).
        assert!(same(
            truncated(sixty_fourths(447.75), 1.0),
            sixty_fourths(447.0)
        ));
        assert!(same(
            truncated(sixty_fourths(710.9993), 1.0),
            sixty_fourths(710.0)
        ));
    }

    #[test]
    fn a_length_on_the_grid_stays_where_it_is() {
        for pixels in [0.0, 16.0, 19.5, 28.718_75, -4.0, 0.015_625] {
            assert!(same(truncated(pixels, 1.0), pixels), "{pixels}");
        }
    }

    #[test]
    fn a_text_width_is_rounded_up() {
        // ADR 0054 § 4: `W` at 13 px, `iii` at 10.5 px, `October 2026` at
        // 13 px, and `Hello` at 16 px, which is on the grid already.
        for (units, placed) in [
            (785.28, 786.0),
            (560.11, 561.0),
            (4551.63, 4552.0),
            (2275.0, 2275.0),
        ] {
            assert!(
                same(rounded_up(sixty_fourths(units), 1.0), sixty_fourths(placed)),
                "{units}"
            );
        }
    }

    #[test]
    fn a_line_height_is_rounded_or_floored_as_adr_0050_says() {
        assert!(same(
            rounded(sixty_fourths(1109.76), 1.0),
            sixty_fourths(1110.0)
        ));
        assert!(same(
            rounded(sixty_fourths(1109.5), 1.0),
            sixty_fourths(1110.0)
        ));
        assert!(same(
            floored(sixty_fourths(1259.52), 1.0),
            sixty_fourths(1259.0)
        ));
        assert!(same(floored(-0.3, 1.0), sixty_fourths(-20.0)));
    }

    #[test]
    fn a_share_of_free_space_is_truncated_toward_zero() {
        // A 10 px box centred in 100.3 px: 5,779 units free, half of it
        // 2,889.5, and Chrome puts it at 2,889.
        let (near, size) = edges(sixty_fourths(2889.5), 10.0, 1.0);
        assert!(same(near, sixty_fourths(2889.0)));
        assert!(same(size, 10.0));
        // A 20 px box centred in 10.3 px overflows by 621 units: −310.5 in
        // front, and Chrome puts it at −310 and keeps it 20 px wide.
        let (near, size) = edges(sixty_fourths(-310.5), 20.0, 1.0);
        assert!(same(near, sixty_fourths(-310.0)));
        assert!(same(size, 20.0));
    }

    #[test]
    fn an_edge_a_float_holds_just_short_of_the_grid_is_floored() {
        // Three `1fr` tracks of 184 px with 6 px gaps: the last ends at
        // 12,287.999 units in `taffy` and in Chrome, which makes that track
        // 3,669 wide and not 3,670.
        let (near, size) = edges(sixty_fourths(8618.666), sixty_fourths(3669.333), 1.0);
        assert!(same(near, sixty_fourths(8618.0)));
        assert!(same(size, sixty_fourths(3669.0)));
    }

    #[test]
    fn flexible_tracks_end_at_their_ideal_edges_floored() {
        // Seven `1fr` in 201 px (ADR 0054 § 3): Chrome's edges are 0, 1,837,
        // 3,675, 5,513, 7,350, 9,188, 11,026 and 12,863 units. The last
        // edge is the ideal one only to within how the float holds 201, so
        // only the first six tracks are asked about.
        let share = 201.0_f32 / 7.0;
        let mut sizes = Vec::new();
        for track in 0_u8..6 {
            let near = share * f32::from(track);
            sizes.push(edges(near, share, 1.0).1 * UNITS_PER_PX);
        }
        let expected = [1837.0, 1838.0, 1838.0, 1837.0, 1838.0, 1838.0];
        for (size, wanted) in sizes.iter().zip(expected) {
            assert!(same(*size, wanted), "{sizes:?}");
        }
    }

    #[test]
    fn at_a_scale_of_two_the_grid_is_a_hundred_and_twenty_eighth() {
        // ADR 0057, Evidence 2, as Chrome 154 measured it under
        // `--force-device-scale-factor=2`: `0.31px` is 38/128 at one and
        // 39/128 at two, `7.2px` 921/128 at two, and `1.01px` 129/128.
        let hundred_twenty_eighths = |units: f32| units / 128.0;
        for (pixels, at_one, at_two) in [
            (0.31, 38.0, 39.0),
            (7.2, 920.0, 921.0),
            (19.92, 2548.0, 2549.0),
            (1.01, 128.0, 129.0),
            (9.6, 1228.0, 1228.0),
        ] {
            assert!(same(truncated(pixels, 1.0), hundred_twenty_eighths(at_one)));
            assert!(
                same(truncated(pixels, 2.0), hundred_twenty_eighths(at_two)),
                "{pixels}: {}",
                truncated(pixels, 2.0) * 128.0
            );
        }
        // A run of 2,793.2 sixty-fourths is 2,794 at one and 2,793.5 at two.
        assert!(same(
            rounded_up(sixty_fourths(2793.2), 1.0),
            sixty_fourths(2794.0)
        ));
        assert!(same(
            rounded_up(sixty_fourths(2793.2), 2.0),
            sixty_fourths(2793.5)
        ));
        // A set `line-height: 17.6px` is 1,126 at one and 1,126.5 at two.
        assert!(same(rounded(17.6, 2.0), sixty_fourths(1126.5)));
        assert!(same(
            floored(sixty_fourths(1259.52), 2.0),
            sixty_fourths(1259.5)
        ));
        // A share of free space: a 10 px box centred in 100.3 px is 2,889.5
        // units in, which is on the grid at two.
        let (near, size) = edges(sixty_fourths(2889.5), 10.0, 2.0);
        assert!(same(near, sixty_fourths(2889.5)));
        assert!(same(size, 10.0));
    }

    #[test]
    fn at_one_and_a_half_an_edge_a_float_short_of_the_grid_is_on_it() {
        // 4,832 units at 1.5 is 50.333… CSS pixels, which a float holds a
        // step short; Chrome, counting in whole units, has the line.
        let short = f32::from_bits((4832.0_f32 / 96.0).to_bits() - 1);
        assert!(short * 96.0 < 4832.0, "the float this guards against");
        let (near, size) = edges(short, 10.0, 1.5);
        assert!((near * 96.0 - 4832.0).abs() < 1e-3, "{}", near * 96.0);
        assert!((size * 96.0 - 960.0).abs() < 1e-3, "{}", size * 96.0);
        // `line-height: 1.3` at 13.3333 px: 1,663.996 units, which Chrome
        // has as 1,664.
        let line = floored(13.3333 * 1.3, 1.5);
        assert!((line * 96.0 - 1664.0).abs() < 1e-3, "{}", line * 96.0);
        // A fifth of a unit short is short, and floored.
        let fifth = floored(4831.8 / 96.0, 1.5);
        assert!((fifth * 96.0 - 4831.0).abs() < 1e-3, "{}", fifth * 96.0);
    }

    #[test]
    fn what_is_not_a_scale_lays_out_at_one() {
        for scale in [0.0, -2.0, f32::NAN, f32::INFINITY] {
            assert!(
                same(truncated(0.31, scale), truncated(0.31, 1.0)),
                "{scale}"
            );
            assert!(
                same(rounded_up(0.31, scale), rounded_up(0.31, 1.0)),
                "{scale}"
            );
        }
    }

    #[test]
    fn past_the_grids_range_a_number_saturates() {
        for pixels in [33_554_431.99, 1e9, f32::INFINITY] {
            assert!(
                same(truncated(pixels, 1.0), sixty_fourths(LIMIT)),
                "{pixels}"
            );
            assert!(
                same(rounded_up(pixels, 1.0), sixty_fourths(LIMIT)),
                "{pixels}"
            );
        }
        assert!(same(
            truncated(f32::NEG_INFINITY, 1.0),
            sixty_fourths(-LIMIT)
        ));
        assert!(truncated(f32::NAN, 1.0).is_nan(), "nothing is invented");
    }
}
