/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A wheel's lines, glided over the display's frames as Chrome glides them
//! (queue item 756, ADR 0093 § 3).
//!
//! # What Chrome does, measured
//!
//! Google Chrome 156.0.8078.76 on this Mac (macOS, a mouse wheel's notch
//! posted as a one-line `CGEvent`, the page's `scrollY` read at every
//! animation frame), at device scales one and two: a notch is a `wheel`
//! event of 40 CSS pixels, and the page **glides** there rather than
//! jumping. Every sampled frame of one and three lines at once fits exactly,
//! and of five and ten within a CSS pixel, a glide that:
//!
//! - follows CSS's `ease-in-out`, `cubic-bezier(0.42, 0, 0.58, 1)`;
//! - lasts `14 − |Δ| ÷ 60` frames of a sixtieth of a second, `Δ` the
//!   distance in **device** pixels, held between 6 frames and 12: a line,
//!   40 or 80 device pixels, 12 frames (200 ms); three lines at two, 240
//!   device pixels, 10 (167 ms); ten at one, 400, 7⅓ (122 ms); ten at two, 6
//!   (100 ms) — fitted 195.5, 163.5, 121 and 92 ms from frames 16.7 ms
//!   apart;
//! - stands at each frame on a whole device pixel.
//!
//! **A notch while gliding** moves the glide's end on by its lines and
//! starts again from where it stands, **as fast as it was going**: the new
//! glide's first control point is `(0.42, 0.42 × s)`, `s` the speed it had
//! as a share of the new distance over the new duration. Two notches 100
//! ms apart fit that to within 1.5 CSS pixels at every frame at one and at
//! two; started again from rest instead, they miss by up to 8.
//!
//! A trackpad's pixels are not glided: macOS sends them already smooth,
//! momentum and all, and they are followed as they come — as Chrome
//! follows them — which ends a glide under way.

use std::time::{Duration, Instant};

/// How long a frame of Chrome's glide is: a sixtieth of a second.
pub const FRAME: Duration = Duration::from_nanos(16_666_667);

/// The ease-in-out curve's first control point's x, and its second's.
const IN: f64 = 0.42;
const OUT: f64 = 0.58;

/// How long a glide over `distance` device pixels lasts: `14 − |distance|
/// ÷ 60` frames, from 6 to 12.
pub fn duration(distance: f64) -> Duration {
    let frames = (14.0 - distance.abs() / 60.0).clamp(6.0, 12.0);
    FRAME.mul_f64(frames)
}

/// A cubic Bézier timing curve from (0, 0) to (1, 1) through two control
/// points, as CSS's `cubic-bezier` is.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Curve {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

impl Curve {
    /// The point at parameter `t`.
    fn point(self, t: f64) -> (f64, f64) {
        let along = |a: f64, b: f64| {
            let u = 1.0 - t;
            3.0 * u * u * t * a + 3.0 * u * t * t * b + t * t * t
        };
        (along(self.x1, self.x2), along(self.y1, self.y2))
    }

    /// How far along the curve is when `x` of its time has passed.
    fn at(self, x: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        // x rises with t on an ease-in-out curve: halve until it is found.
        let (mut low, mut high) = (0.0_f64, 1.0_f64);
        for _ in 0..48 {
            let middle = f64::midpoint(low, high);
            if self.point(middle).0 < x {
                low = middle;
            } else {
                high = middle;
            }
        }
        self.point(f64::midpoint(low, high)).1
    }

    /// How fast it goes when `x` of its time has passed, in its own units:
    /// distance shares per time share.
    fn slope(self, x: f64) -> f64 {
        let step = 1e-4;
        let (before, after) = ((x - step).max(0.0), (x + step).min(1.0));
        if after <= before {
            return 0.0;
        }
        (self.at(after) - self.at(before)) / (after - before)
    }
}

/// A glide of the conversation's offset from one place to another.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glide {
    /// Where it started, in CSS pixels down.
    from: f64,
    /// Where it ends.
    to: f64,
    /// When it started.
    start: Instant,
    /// How long it lasts.
    lasts: Duration,
    /// Its timing.
    curve: Curve,
}

impl Glide {
    /// A glide from rest at `from` to `to`, starting `now`, on a screen of
    /// `scale` device pixels to the CSS pixel.
    pub fn new(from: f32, to: f32, now: Instant, scale: f32) -> Self {
        Self::moving(f64::from(from), f64::from(to), now, scale, 0.0)
    }

    /// A glide from `from` to `to` already moving at `speed` CSS pixels a
    /// second toward it.
    fn moving(from: f64, to: f64, now: Instant, scale: f32, speed: f64) -> Self {
        let distance = to - from;
        let lasts = duration(distance * f64::from(scale));
        let share = if distance.abs() > f64::EPSILON {
            (speed * lasts.as_secs_f64() / distance).max(0.0)
        } else {
            0.0
        };
        Self {
            from,
            to,
            start: now,
            lasts,
            curve: Curve {
                x1: IN,
                y1: (IN * share).min(1.0),
                x2: OUT,
                y2: 1.0,
            },
        }
    }

    /// The share of its time gone `now`, from 0 to 1.
    fn gone(&self, now: Instant) -> f64 {
        let lasts = self.lasts.as_secs_f64();
        if lasts <= 0.0 {
            return 1.0;
        }
        (now.saturating_duration_since(self.start).as_secs_f64() / lasts).clamp(0.0, 1.0)
    }

    /// Where it stands `now`, in CSS pixels down, on a device pixel of a
    /// screen of `scale`.
    pub fn at(&self, now: Instant, scale: f32) -> f32 {
        let place = self.from + (self.to - self.from) * self.curve.at(self.gone(now));
        let scale = f64::from(alo_value::device_scale(scale));
        #[expect(
            clippy::cast_possible_truncation,
            reason = "an offset inside the conversation's million pixels, on a device pixel"
        )]
        let placed = ((place * scale).round() / scale) as f32;
        // Never −0: the offset a glide starts from is +0.
        placed + 0.0
    }

    /// Where it ends.
    pub fn end(&self) -> f32 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "an offset made from an f32 and lines of 40"
        )]
        let end = self.to as f32;
        end
    }

    /// Whether it has ended by `now`.
    pub fn done(&self, now: Instant) -> bool {
        self.gone(now) >= 1.0
    }

    /// The glide moved on to end at `to` instead, `now`: started again
    /// from where it stands, as fast as it was going.
    #[must_use]
    pub fn to(&self, to: f32, now: Instant, scale: f32) -> Self {
        let gone = self.gone(now);
        let here = self.from + (self.to - self.from) * self.curve.at(gone);
        let lasts = self.lasts.as_secs_f64();
        let speed = if lasts > 0.0 && gone < 1.0 {
            (self.to - self.from) * self.curve.slope(gone) / lasts
        } else {
            0.0
        };
        // Toward the new end, or nothing when it turned back.
        let toward = if (f64::from(to) - here) * speed > 0.0 {
            speed.abs()
        } else {
            0.0
        };
        Self::moving(here, f64::from(to), now, scale, toward)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(milliseconds: f64) -> Duration {
        Duration::from_secs_f64(milliseconds / 1000.0)
    }

    #[test]
    fn a_glide_lasts_as_chromes_by_its_device_pixels() {
        let close = |a: Duration, b: f64| (a.as_secs_f64() * 1000.0 - b).abs() < 0.01;
        assert!(close(duration(40.0), 200.0), "a line at one: 12 frames");
        assert!(close(duration(80.0), 200.0), "a line at two");
        assert!(close(duration(120.0), 200.0), "three at one");
        assert!(close(duration(240.0), 166.67), "three at two: 10 frames");
        assert!(close(duration(200.0), 177.78), "five at one");
        assert!(close(duration(400.0), 122.22), "ten at one");
        assert!(
            close(duration(800.0), 100.0),
            "ten at two: 6 frames, the least"
        );
        assert!(close(duration(-80.0), 200.0), "back as on");
    }

    /// Chrome 156's `scrollY` at each frame of one line down at two, from
    /// 10000, after the glide's start fitted to them (2.5 ms before the
    /// `wheel` event's time): each frame's time from that start, and where
    /// it stood.
    const ONE_LINE_AT_TWO: [(f64, f32); 12] = [
        (16.8, 0.5),
        (33.0, 2.5),
        (49.7, 5.5),
        (66.3, 9.5),
        (83.0, 14.5),
        (99.6, 20.5),
        (116.3, 26.0),
        (133.0, 31.0),
        (149.7, 35.0),
        (166.5, 38.0),
        (183.1, 39.5),
        (199.6, 40.0),
    ];

    #[test]
    fn one_line_stands_where_chrome_stood_at_every_frame() {
        let start = Instant::now();
        let glide = Glide::new(0.0, 40.0, start, 2.0);
        for (at, chrome) in ONE_LINE_AT_TWO {
            let ours = glide.at(start + ms(at), 2.0);
            assert!(
                (ours - chrome).abs() <= 0.5,
                "at {at} ms Chrome stood at {chrome}, and the glide at {ours}"
            );
        }
        assert!(glide.done(start + ms(200.1)));
        assert!(!glide.done(start + ms(199.0)));
        assert_eq!(
            glide.at(start + ms(500.0), 2.0).to_bits(),
            40.0_f32.to_bits()
        );
        assert_eq!(glide.at(start, 2.0).to_bits(), 0.0_f32.to_bits());
        // Half way through its time, half way there.
        assert_eq!(
            glide.at(start + ms(100.0), 2.0).to_bits(),
            20.0_f32.to_bits()
        );
        // On a device pixel: a whole CSS pixel at one.
        let at_one = Glide::new(0.0, -40.0, start, 1.0);
        let place = at_one.at(start + ms(37.0), 1.0);
        assert_eq!(place.to_bits(), place.round().to_bits(), "{place}");
    }

    /// Chrome 156's frames after a second line 100 ms after the first, at
    /// two, from the first glide's start fitted to them (6 ms after the
    /// first `wheel` event): ending at 80. The second was applied 99.5 ms
    /// into the first glide.
    const TWO_LINES_AT_TWO: [(f64, f32); 12] = [
        (119.2, 27.0),
        (135.9, 33.5),
        (152.5, 40.5),
        (169.2, 48.0),
        (185.8, 55.0),
        (202.5, 62.0),
        (219.2, 68.0),
        (235.8, 72.5),
        (252.5, 76.0),
        (269.2, 78.5),
        (285.9, 79.5),
        (302.6, 80.0),
    ];

    #[test]
    fn a_second_line_while_gliding_goes_on_as_fast_as_it_was_going() {
        let start = Instant::now();
        let first = Glide::new(0.0, 40.0, start, 2.0);
        // The second notch, as Chrome applied it: 99.5 ms into the first.
        let second = first.to(80.0, start + ms(99.5), 2.0);
        assert_eq!(second.end().to_bits(), 80.0_f32.to_bits());
        for (at, chrome) in TWO_LINES_AT_TWO {
            let ours = second.at(start + ms(at), 2.0);
            assert!(
                (ours - chrome).abs() <= 1.5,
                "at {at} ms Chrome stood at {chrome}, and the glide at {ours}"
            );
        }
        // Started again from rest, it would lag Chrome by far more.
        let from_rest = Glide::new(first.at(start + ms(99.5), 2.0), 80.0, start + ms(99.5), 2.0);
        let worst = TWO_LINES_AT_TWO
            .iter()
            .map(|&(at, chrome)| (from_rest.at(start + ms(at), 2.0) - chrome).abs())
            .fold(0.0_f32, f32::max);
        assert!(worst > 4.0, "{worst}");
        // Turned back, it starts back from rest where it stands.
        let back = first.to(-40.0, start + ms(100.0), 2.0);
        assert_eq!(
            back.at(start + ms(100.0), 2.0).to_bits(),
            20.0_f32.to_bits()
        );
        assert!(back.at(start + ms(120.0), 2.0) < 20.0);
    }
}
