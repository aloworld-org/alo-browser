/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the window server shows on the screen: the one witness of whether a
//! person can see a window (ADR 0062 §§ 2–3, queue item 672).
//!
//! Chrome's word for where its window is does not change when Stage Manager
//! shrinks the window into its strip, and the page inside it still reads
//! `visible`. So nothing over Chrome's pipe says whether a person can see
//! the page, and the window server is asked.
//!
//! # Through `osascript`, and why not the crate
//!
//! ADR 0062 § 3 preferred `objc2-core-graphics` — already in the build,
//! beneath `softbuffer` — on the condition that every call it took was a
//! safe one. Checked in 0.3.2's source, it is not: `CGWindowListCopyWindowInfo`
//! is safe, but it answers an untyped `CFArray`, whose safe `get` is only
//! for an array of a known type and which is made one by `cast_unchecked`,
//! an `unsafe fn`; the keys the dictionaries are read by (`kCGWindowBounds`
//! and the rest) are `extern` statics, which only `unsafe` reads; and
//! `CGRectMakeWithDictionaryRepresentation` takes a raw pointer. Law 4
//! forbids any of that here without an ADR, so — as the ADR's stop rule
//! says — the same function is asked through `osascript`, which is how the
//! tests already asked it. Measured by iteration 382: 80 ms of wall time and
//! 20–30 ms of processor time a reading on this machine, idle.
//!
//! # What is read, and what is not
//!
//! Only the windows of one process, on the screen: each window's number,
//! its layer, how opaque it is drawn and its bounds. Never a window's
//! title: a title is the page's string, a stranger's, and nothing here
//! needs it. The process is passed to the script as an argument, never
//! written into its text. And where every display is ([`displays`]), so
//! that whether a person can see any of a window is said against all of
//! them ([`seen`]) — a card's window is parked by the window server with a
//! strip of it on whichever display it chooses (queue item 771) — and the
//! scale each draws a window at, so that a card's window can be parked by
//! a display at the conversation's (`chrome_park.rs`, queue item 772).
//!
//! What `osascript` prints is read as strictly as anything from outside: a
//! line that is not exactly eight numbers — five for a display — is
//! refused, and so is the whole reading, rather than half of it believed.

use crate::chrome_protocol::Bounds;
use core::fmt;
use std::io::ErrorKind;

/// The most windows of one process a reading holds; more is refused rather
/// than read. A browser's process has a handful.
const MOST_WINDOWS: usize = 256;

/// The script, in JavaScript for Automation: every window on the screen
/// whose owner is the process in its one argument, a line each — number,
/// owner, layer, opacity out of 255, left, top, width, height — in the
/// window server's points.
#[cfg(target_os = "macos")]
const LISTING: &str = "ObjC.import('CoreGraphics');\
    function run(argv) {\
      var owner = Number(argv[0]);\
      var all = ObjC.deepUnwrap(ObjC.castRefToObject(\
        $.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly, 0))) || [];\
      return all.filter(w => w.kCGWindowOwnerPID === owner)\
        .map(w => [w.kCGWindowNumber, w.kCGWindowOwnerPID, w.kCGWindowLayer,\
          Math.round(w.kCGWindowAlpha * 255),\
          w.kCGWindowBounds.X, w.kCGWindowBounds.Y,\
          w.kCGWindowBounds.Width, w.kCGWindowBounds.Height].join(' '))\
        .join('\\n');\
    }";

/// The script listing the displays, a line each — left, top, width, height
/// — in the window server's points, where windows are: `AppKit`'s screens,
/// whose frames count up from the main display's bottom edge, turned to
/// count down from its top as the window server's do; and then the scale a
/// window on it is drawn at, its `backingScaleFactor` (queue item 772).
#[cfg(target_os = "macos")]
const DISPLAYS: &str = "ObjC.import('AppKit');\
    function run() {\
      var screens = $.NSScreen.screens.js;\
      if (screens.length === 0) { return ''; }\
      var main = screens[0].frame.size.height;\
      return screens.map(s => { var f = s.frame;\
          return [f.origin.x, main - f.origin.y - f.size.height,\
            f.size.width, f.size.height, s.backingScaleFactor].join(' '); })\
        .join('\\n');\
    }";

/// A display, as the window server has it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    /// Where it is, in the window server's points, the main display's top
    /// left corner at the origin.
    pub bounds: Bounds,
    /// How many device pixels a point is on it — the scale a window on it
    /// is drawn at: `AppKit`'s `backingScaleFactor`, as it said it.
    pub factor: f64,
}

/// A window the window server shows on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listed {
    /// Its number, the window server's own.
    pub number: u64,
    /// The process that owns it.
    pub owner: u32,
    /// Its layer: 0 for an ordinary window, 3 for a floating one.
    pub layer: i64,
    /// How opaque the window server draws it, from 0 — nothing of it is
    /// drawn — to 255.
    pub alpha: u8,
    /// Where it is, in the screen's points — or [`None`] when the window
    /// server put it at a fraction of a point, which is then nowhere a
    /// window of Chrome's, whose bounds are whole, can be.
    pub bounds: Option<Bounds>,
}

/// Why the window server's listing was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unlisted {
    /// `osascript` could not be run.
    NotRun(ErrorKind),
    /// `osascript` ran and failed, and said this.
    Refused {
        /// What it wrote to its error output, cut short.
        said: String,
    },
    /// What it printed was not a listing.
    Unread(Unread),
    /// This system's window server is not one this browser reads.
    Elsewhere,
}

impl fmt::Display for Unlisted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRun(kind) => write!(f, "the window server could not be asked: {kind}"),
            Self::Refused { said } => write!(f, "the window server's listing failed: {said}"),
            Self::Unread(why) => why.fmt(f),
            Self::Elsewhere => {
                f.write_str("this system's window server is not one this browser reads")
            }
        }
    }
}

impl std::error::Error for Unlisted {}

/// Why a listing printed was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unread {
    /// Not text.
    NotText,
    /// More than [`MOST_WINDOWS`] windows.
    TooMany,
    /// This line, by its number from 1, was not seven numbers.
    BadLine(usize),
}

impl fmt::Display for Unread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotText => f.write_str("the window server's listing was not text"),
            Self::TooMany => write!(
                f,
                "the window server listed more than {MOST_WINDOWS} windows of one process"
            ),
            Self::BadLine(line) => write!(
                f,
                "line {line} of the window server's listing was not a window"
            ),
        }
    }
}

/// The windows the window server shows on the screen for the process
/// `owner`, frontmost first.
///
/// # Errors
///
/// [`Unlisted`], when it could not be asked or what it said is not a
/// listing — or [`Unlisted::Elsewhere`] off macOS.
#[cfg(target_os = "macos")]
pub fn on_the_screen(owner: u32) -> Result<Vec<Listed>, Unlisted> {
    let ran = std::process::Command::new("osascript")
        .args(["-l", "JavaScript", "-e", LISTING, &owner.to_string()])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|why| Unlisted::NotRun(why.kind()))?;
    if !ran.status.success() {
        let said = String::from_utf8_lossy(&ran.stderr);
        return Err(Unlisted::Refused {
            said: said.trim().chars().take(200).collect(),
        });
    }
    read(&ran.stdout).map_err(Unlisted::Unread)
}

/// Nothing is read off macOS: how another system's window server is asked
/// is not built.
///
/// # Errors
///
/// [`Unlisted::Elsewhere`], always.
#[cfg(not(target_os = "macos"))]
pub fn on_the_screen(_owner: u32) -> Result<Vec<Listed>, Unlisted> {
    Err(Unlisted::Elsewhere)
}

/// Every display: where it is, in the window server's points, the main
/// one's top left corner at the origin, and the scale it draws a window at.
///
/// # Errors
///
/// [`Unlisted`], when it could not be asked or what it said is not a
/// listing of displays — or [`Unlisted::Elsewhere`] off macOS.
#[cfg(target_os = "macos")]
pub fn displays() -> Result<Vec<Display>, Unlisted> {
    let ran = std::process::Command::new("osascript")
        .args(["-l", "JavaScript", "-e", DISPLAYS])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|why| Unlisted::NotRun(why.kind()))?;
    if !ran.status.success() {
        let said = String::from_utf8_lossy(&ran.stderr);
        return Err(Unlisted::Refused {
            said: said.trim().chars().take(200).collect(),
        });
    }
    read_displays(&ran.stdout).map_err(Unlisted::Unread)
}

/// Nothing is read off macOS, as [`on_the_screen`].
///
/// # Errors
///
/// [`Unlisted::Elsewhere`], always.
#[cfg(not(target_os = "macos"))]
pub fn displays() -> Result<Vec<Display>, Unlisted> {
    Err(Unlisted::Elsewhere)
}

/// Whether a person may see any of `window` on one of `displays`: drawn at
/// all (its alpha not 0) and meeting one of them by some area — or drawn at
/// a fraction of a point, where where it is cannot be said, and so it is
/// never said to be unseen.
pub fn seen(window: &Listed, displays: &[Bounds]) -> bool {
    if window.alpha == 0 {
        return false;
    }
    let Some(bounds) = window.bounds else {
        return true;
    };
    displays.iter().any(|display| meet(bounds, *display))
}

/// Whether two rectangles share some area: touching edges share none.
fn meet(one: Bounds, other: Bounds) -> bool {
    one.left < other.left.saturating_add(other.width)
        && other.left < one.left.saturating_add(one.width)
        && one.top < other.top.saturating_add(other.height)
        && other.top < one.top.saturating_add(one.height)
}

/// A listing of displays as [`DISPLAYS`] prints it, read.
///
/// # Errors
///
/// [`Unread`], for the first line in it that is not four whole numbers
/// with a width and a height above none, then a scale above none.
pub fn read_displays(printed: &[u8]) -> Result<Vec<Display>, Unread> {
    let text = core::str::from_utf8(printed).map_err(|_| Unread::NotText)?;
    let mut displays = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        if displays.len() == MOST_WINDOWS {
            return Err(Unread::TooMany);
        }
        displays.push(display(line).ok_or(Unread::BadLine(index.saturating_add(1)))?);
    }
    Ok(displays)
}

/// One display's line: its four bounds, each whole, and its scale.
fn display(line: &str) -> Option<Display> {
    let mut fields = line.split(' ');
    let mut sides = [0; 4];
    for side in &mut sides {
        *side = whole(fields.next()?)?;
    }
    let factor = factor(fields.next()?)?;
    if fields.next().is_some() {
        return None;
    }
    let [left, top, width, height] = sides;
    (width > 0 && height > 0).then_some(Display {
        bounds: Bounds {
            left,
            top,
            width,
            height,
        },
        factor,
    })
}

/// A display's scale, written as a number above none: digits, and at most
/// one point with digits after it.
fn factor(field: &str) -> Option<f64> {
    let (before, after) = field.split_once('.').unwrap_or((field, "0"));
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    if !digits(before) || !digits(after) {
        return None;
    }
    let factor: f64 = field.parse().ok()?;
    (factor.is_finite() && factor > 0.0).then_some(factor)
}

/// A listing as the script prints it, read.
///
/// # Errors
///
/// [`Unread`], for the first thing in it that is not a window.
pub fn read(printed: &[u8]) -> Result<Vec<Listed>, Unread> {
    let text = core::str::from_utf8(printed).map_err(|_| Unread::NotText)?;
    let mut listed = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.is_empty() {
            continue;
        }
        if listed.len() == MOST_WINDOWS {
            return Err(Unread::TooMany);
        }
        listed.push(window(line).ok_or(Unread::BadLine(index.saturating_add(1)))?);
    }
    Ok(listed)
}

/// One line: number, owner, layer, opacity, then the four bounds.
fn window(line: &str) -> Option<Listed> {
    let mut fields = line.split(' ');
    let number = fields.next()?.parse().ok()?;
    let owner = fields.next()?.parse().ok()?;
    let layer = whole(fields.next()?)?;
    let alpha = u8::try_from(whole(fields.next()?)?).ok()?;
    let mut sides = [Side::Fraction; 4];
    for side in &mut sides {
        *side = side_of(fields.next()?)?;
    }
    if fields.next().is_some() {
        return None;
    }
    let bounds = match sides {
        [
            Side::Whole(_),
            Side::Whole(_),
            Side::Whole(width),
            Side::Whole(height),
        ] if width < 0 || height < 0 => {
            return None;
        }
        [
            Side::Whole(left),
            Side::Whole(top),
            Side::Whole(width),
            Side::Whole(height),
        ] => Some(Bounds {
            left,
            top,
            width,
            height,
        }),
        _ => None,
    };
    Some(Listed {
        number,
        owner,
        layer,
        alpha,
        bounds,
    })
}

/// One side of a window's bounds, as the window server printed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// A whole number of points.
    Whole(i64),
    /// A number with a fraction of a point.
    Fraction,
}

/// A whole number, written as one: digits after an optional minus.
fn whole(field: &str) -> Option<i64> {
    let digits = field.strip_prefix('-').unwrap_or(field);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    field.parse().ok()
}

/// One side of a window's bounds, or [`None`] when it is not a number.
fn side_of(field: &str) -> Option<Side> {
    if let Some(side) = whole(field) {
        return Some(Side::Whole(side));
    }
    let (before, after) = field.split_once('.')?;
    let fraction = !after.is_empty() && after.bytes().all(|byte| byte.is_ascii_digit());
    (fraction && whole(before).is_some()).then_some(Side::Fraction)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(left: i64, top: i64, width: i64, height: i64) -> Bounds {
        Bounds {
            left,
            top,
            width,
            height,
        }
    }

    #[test]
    fn a_listing_is_read_a_window_a_line() {
        let listed = read(b"4211 64062 0 255 300,240 1000x632\n").err();
        assert_eq!(listed, Some(Unread::BadLine(1)), "commas are not spaces");
        let listed = read(b"4211 64062 0 255 300 240 1000 632\n977 64062 3 0 -1710 -25 1000 40\n")
            .expect("a listing");
        assert_eq!(
            listed,
            [
                Listed {
                    number: 4211,
                    owner: 64062,
                    layer: 0,
                    alpha: 255,
                    bounds: Some(at(300, 240, 1000, 632)),
                },
                Listed {
                    number: 977,
                    owner: 64062,
                    layer: 3,
                    alpha: 0,
                    bounds: Some(at(-1710, -25, 1000, 40)),
                },
            ]
        );
        assert_eq!(read(b""), Ok(Vec::new()), "a process with nothing shown");
        assert_eq!(read(b"\n"), Ok(Vec::new()));
    }

    #[test]
    fn a_window_at_a_fraction_of_a_point_is_listed_nowhere() {
        let listed = read(b"12 7 0 255 300.5 240 1000 632").expect("a listing");
        assert_eq!(listed.first().map(|window| window.bounds), Some(None));
        assert_eq!(listed.first().map(|window| window.number), Some(12));
    }

    #[test]
    fn anything_else_is_refused_whole() {
        for bad in [
            &b"12 7 0 255 300 240 1000"[..],
            b"12 7 0 255 300 240 1000 632 1",
            b"12 7 0 255 300 240 1000 -632",
            b"12 7 0.5 255 300 240 1000 632",
            b"12 -7 0 255 300 240 1000 632",
            b"12 7 0 255 300 240 1e3 632",
            b"12 7 0 255 +300 240 1000 632",
            b"12 7 0 255 300. 240 1000 632",
            b"12 7 0 255 .5 240 1000 632",
            b"12 7 0 255 300 240 1000 632 ",
            b"12  7 0 255 300 240 1000 632",
            b"12 7 0 255 99999999999999999999 240 1000 632",
            b"twelve 7 0 255 300 240 1000 632",
            b"12 7 0 255 NaN 240 1000 632",
            b"12 7 0 256 300 240 1000 632",
            b"12 7 0 -1 300 240 1000 632",
            b"12 7 0 0.5 300 240 1000 632",
            b"12 7 0 300 240 1000 632",
        ] {
            assert_eq!(read(bad), Err(Unread::BadLine(1)), "{bad:?}");
        }
        assert_eq!(
            read(b"1 7 0 255 0 0 1 1\n2 7 0 255 0 0 1 x"),
            Err(Unread::BadLine(2)),
            "the line that failed is named, and the first is not kept"
        );
        assert_eq!(read(&[0xff, 0xfe]), Err(Unread::NotText));
    }

    #[test]
    fn a_process_with_too_many_windows_is_refused() {
        let line = "1 7 0 255 0 0 1 1\n";
        assert_eq!(
            read(line.repeat(MOST_WINDOWS).as_bytes()).map(|all| all.len()),
            Ok(256)
        );
        assert_eq!(
            read(line.repeat(MOST_WINDOWS + 1).as_bytes()),
            Err(Unread::TooMany)
        );
    }

    /// This Mac's three displays as `AppKit` gave them, turned (queue item
    /// 771): the Retina display at the origin, one at one left of it and a
    /// portrait one left of that.
    const THREE: [Bounds; 3] = [
        Bounds {
            left: 0,
            top: 0,
            width: 1710,
            height: 1107,
        },
        Bounds {
            left: -3000,
            top: -80,
            width: 1080,
            height: 1920,
        },
        Bounds {
            left: -1920,
            top: 0,
            width: 1920,
            height: 1080,
        },
    ];

    #[test]
    fn displays_are_read_a_display_a_line() {
        let read = read_displays(b"0 0 1710 1107 2\n-3000 -80 1080 1920 1\n-1920 0 1920 1080 1\n")
            .expect("three displays");
        assert_eq!(
            read.iter()
                .map(|display| display.bounds)
                .collect::<Vec<_>>(),
            THREE.to_vec()
        );
        assert_eq!(
            read.iter()
                .map(|display| display.factor.to_bits())
                .collect::<Vec<_>>(),
            [2.0_f64.to_bits(), 1.0_f64.to_bits(), 1.0_f64.to_bits()]
        );
        let between = read_displays(b"0 0 1280 800 1.5").expect("one display");
        assert_eq!(
            between.first().map(|display| display.factor.to_bits()),
            Some(1.5_f64.to_bits())
        );
        assert_eq!(read_displays(b""), Ok(Vec::new()));
        for bad in [
            &b"0 0 1710"[..],
            b"0 0 1710 1107",
            b"0 0 1710 1107 2 1",
            b"0 0 0 1107 2",
            b"0 0 1710 -1107 2",
            b"0 0 1710.5 1107 2",
            b"0 0 1710 1107 2 ",
            b"0 0 1710 1107 0",
            b"0 0 1710 1107 -2",
            b"0 0 1710 1107 2.",
            b"0 0 1710 1107 .5",
            b"0 0 1710 1107 1e1",
            b"0 0 1710 1107 NaN",
            b"0 0 1710 1107 two",
        ] {
            assert_eq!(read_displays(bad), Err(Unread::BadLine(1)), "{bad:?}");
        }
        assert_eq!(read_displays(&[0xff]), Err(Unread::NotText));
    }

    fn window(alpha: u8, bounds: Option<Bounds>) -> Listed {
        Listed {
            number: 1,
            owner: 7,
            layer: 0,
            alpha,
            bounds,
        }
    }

    #[test]
    fn a_window_is_seen_when_drawn_on_a_display() {
        // Where the window server parked a card's window, measured: 153
        // points of it on the portrait display.
        let parked = Some(at(-3725, 0, 878, 552));
        assert!(seen(&window(255, parked), &THREE));
        // The same, drawn fully transparent.
        assert!(!seen(&window(0, parked), &THREE));
        // Expanded.
        assert!(seen(&window(255, Some(at(120, 80, 900, 600))), &THREE));
        // Barely drawn is drawn.
        assert!(seen(&window(1, Some(at(120, 80, 900, 600))), &THREE));
        // Off every display, and touching one only by an edge.
        assert!(!seen(&window(255, Some(at(-20000, 0, 878, 552))), &THREE));
        assert!(!seen(&window(255, Some(at(-3878, 0, 878, 552))), &THREE));
        assert!(!seen(&window(255, Some(at(1710, 0, 878, 552))), &THREE));
        assert!(!seen(&window(255, Some(at(-1000, 1080, 878, 552))), &THREE));
        // A gap between displays: under the one at one, left of the Retina.
        assert!(!seen(&window(255, Some(at(-1000, 1100, 878, 552))), &THREE));
        // At a fraction of a point, where it is cannot be said.
        assert!(seen(&window(255, None), &THREE));
        assert!(!seen(&window(0, None), &THREE));
    }

    #[test]
    fn a_refusal_says_what_was_refused() {
        assert_eq!(
            Unlisted::Unread(Unread::BadLine(3)).to_string(),
            "line 3 of the window server's listing was not a window"
        );
        assert_eq!(
            Unread::TooMany.to_string(),
            "the window server listed more than 256 windows of one process"
        );
    }
}
