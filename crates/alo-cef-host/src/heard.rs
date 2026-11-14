/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What alo tells a windowless page's host (queue item 979, ADR 0113): the
//! view's size and scale, whether it is hidden, and whether it has the
//! focus — what CEF's `WasResized`, `NotifyScreenInfoChanged`, `WasHidden`
//! and `SetFocus` carry, which nothing on the DevTools pipe can, since a
//! windowless page has no window for Chromium to read them from.
//!
//! # The channel
//!
//! The pipe carries only Chromium's protocol, and what the host tells alo
//! goes out on descriptor 5 (`told.rs`). For a windowless host alo makes
//! descriptor 5 a socket rather than a pipe (`alo-window`'s `chrome.rs`), so
//! the same descriptor carries alo's lines the other way, and the host
//! reads them on a thread of its own ([`listen`]). A host whose descriptor
//! 5 is a pipe — a windowed card's, the machine's Chrome's alo never tells,
//! a host started by hand — cannot open it for reading, and hears nothing.
//!
//! # What may be told, and nothing else
//!
//! A line is a word, then its one field after a tab, then a line's end:
//!
//! - `view\tWIDTHxHEIGHT@SCALE` — the view's size in CSS pixels and its
//!   scale, as `--alo-windowless=` is written ([`Viewport::parse`], so each
//!   side 1 to 16,384 and the scale above 0 and at most 4);
//! - `hidden\t1` or `hidden\t0` — the card off the screen, or on it again;
//! - `focus\t1` or `focus\t0` — the page given the focus, or it taken away.
//!
//! Anything else is **refused**: said on the host's error output, and
//! nothing done — a line longer than [`LONGEST_LINE`] bytes (passed over to
//! its end), one not UTF-8, another word, a field not as written, a field
//! more, and any line past [`MOST_A_SECOND`] in one second. alo is the only
//! writer, but the host reads it as a boundary all the same: nothing a line
//! says is ever run, and the most it can do is size, scale, hide or focus
//! the one page.

use crate::windowless::Viewport;
use std::io::{BufRead, Read};
use std::time::{Duration, Instant};

/// The longest line heard, its end included: `view\t16384x16384@1.25` is
/// 22 bytes, so a scale may be written with a few digits more and nothing
/// else fits.
pub const LONGEST_LINE: usize = 64;

/// The most lines heard in any one second: a window dragged on a display
/// of sixty frames a second asks for one size a frame, and alo carries out
/// only the newest of those waiting (`chrome_card.rs`). More is refused.
pub const MOST_A_SECOND: usize = 60;

/// What alo told the host.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Heard {
    /// The view's size and scale: `WasResized`, and
    /// `NotifyScreenInfoChanged` when the scale is another.
    View(Viewport),
    /// Off the screen, or on it again: `WasHidden`.
    Hidden(bool),
    /// The focus given, or taken away: `SetFocus`.
    Focused(bool),
}

/// `line`, without its end, read — or nothing, when it is not one alo may
/// tell.
pub fn read(line: &str) -> Option<Heard> {
    if line.len() >= LONGEST_LINE {
        return None;
    }
    let (word, field) = line.split_once('\t')?;
    let flag = |field: &str| match field {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    };
    match word {
        // A second tab is in no field alo writes, and `parse` refuses it.
        "view" => Viewport::parse(field).map(Heard::View),
        "hidden" => flag(field).map(Heard::Hidden),
        "focus" => flag(field).map(Heard::Focused),
        _ => None,
    }
}

/// The lines heard in the last second, to refuse those past
/// [`MOST_A_SECOND`].
#[derive(Debug, Default)]
pub struct Rate {
    /// When each line taken in the last second was heard, oldest first.
    taken: std::collections::VecDeque<Instant>,
}

impl Rate {
    /// Whether a line heard at `now` is taken: fewer than [`MOST_A_SECOND`]
    /// were taken in the second before it.
    pub fn take(&mut self, now: Instant) -> bool {
        while self
            .taken
            .front()
            .is_some_and(|at| now.saturating_duration_since(*at) >= Duration::from_secs(1))
        {
            self.taken.pop_front();
        }
        if self.taken.len() >= MOST_A_SECOND {
            return false;
        }
        self.taken.push_back(now);
        true
    }
}

/// What carrying out `after` changes of a view that was `before`: whether
/// the screen's information is told again (the scale is another), and
/// whether the view is resized (anything is another) — or neither, for the
/// same view told again.
pub fn changes(before: Viewport, after: Viewport) -> (bool, bool) {
    let rescaled = (before.scale - after.scale).abs() > f32::EPSILON;
    let resized = rescaled || before.width != after.width || before.height != after.height;
    (rescaled, resized)
}

/// Read `from` line by line until it ends, handing `heard` each line alo
/// may tell, and saying each refused on the error output.
pub fn listen(from: impl Read, mut heard: impl FnMut(Heard)) {
    let mut reading = std::io::BufReader::new(from);
    let mut rate = Rate::default();
    let mut line = Vec::with_capacity(LONGEST_LINE);
    loop {
        line.clear();
        let most = u64::try_from(LONGEST_LINE).unwrap_or(u64::MAX);
        match (&mut reading).take(most).read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if line.last() != Some(&b'\n') {
            refused("a line past the longest");
            if pass_over(&mut reading).is_err() {
                return;
            }
            continue;
        }
        line.pop();
        let read = std::str::from_utf8(&line).ok().and_then(read);
        match read {
            None => refused("a line not as alo writes it"),
            Some(_) if !rate.take(Instant::now()) => refused("a line past the most a second"),
            Some(read) => heard(read),
        }
    }
}

/// Read `reading` to the end of the line it is in.
fn pass_over(reading: &mut impl BufRead) -> std::io::Result<()> {
    let mut passed = Vec::new();
    loop {
        passed.clear();
        let read = reading.take(4096).read_until(b'\n', &mut passed)?;
        if read == 0 || passed.last() == Some(&b'\n') {
            return Ok(());
        }
    }
}

/// Say on the host's error output that a line was refused, and why.
fn refused(why: &str) {
    eprintln!("alo's CEF host: windowless: refused {why}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(width: i32, height: i32, scale: f32) -> Heard {
        Heard::View(Viewport {
            width,
            height,
            scale,
        })
    }

    #[test]
    fn a_view_hiding_and_focus_are_heard() {
        assert_eq!(read("view\t800x500@2"), Some(view(800, 500, 2.0)));
        assert_eq!(read("view\t1x1@0.5"), Some(view(1, 1, 0.5)));
        assert_eq!(read("hidden\t1"), Some(Heard::Hidden(true)));
        assert_eq!(read("hidden\t0"), Some(Heard::Hidden(false)));
        assert_eq!(read("focus\t1"), Some(Heard::Focused(true)));
        assert_eq!(read("focus\t0"), Some(Heard::Focused(false)));
    }

    #[test]
    fn anything_else_is_refused() {
        for refused in [
            "",
            "view",
            "view\t",
            "view\t800x500",
            "view\t0x500@2",
            "view\t16385x500@2",
            "view\t800x500@5",
            "view\t800x500@2\t1",
            "View\t800x500@2",
            " view\t800x500@2",
            "hidden",
            "hidden\t2",
            "hidden\ttrue",
            "hidden\t1\t",
            "focus\t",
            "focus\t01",
            "resize\t800x500",
            "script\talert(1)",
            "navigate\thttps://example.com",
        ] {
            assert_eq!(read(refused), None, "{refused:?}");
        }
        let long = format!("view\t800x500@{}", "1".repeat(LONGEST_LINE));
        assert_eq!(read(&long), None, "a line past the longest");
    }

    #[test]
    fn only_the_most_a_second_is_taken() {
        let start = Instant::now();
        let mut rate = Rate::default();
        let taken = (0..MOST_A_SECOND * 2).filter(|_| rate.take(start)).count();
        assert_eq!(taken, MOST_A_SECOND);
        let almost = start + Duration::from_millis(999);
        assert!(!rate.take(almost), "still the same second");
        assert!(rate.take(start + Duration::from_secs(1)), "a second on");
    }

    #[test]
    fn a_scale_told_again_is_the_screen_and_a_size_only_a_resize() {
        let at = |width, height, scale| Viewport {
            width,
            height,
            scale,
        };
        assert_eq!(changes(at(600, 400, 1.0), at(800, 500, 1.0)), (false, true));
        assert_eq!(changes(at(800, 500, 1.0), at(800, 500, 2.0)), (true, true));
        assert_eq!(
            changes(at(800, 500, 2.0), at(800, 500, 2.0)),
            (false, false)
        );
    }

    #[test]
    fn lines_are_heard_in_order_and_the_refused_passed_over() {
        let long = "x".repeat(LONGEST_LINE * 3);
        let mut written = format!("focus\t0\n{long}\nview\t800x500@2\nscript\tx\n").into_bytes();
        written.extend_from_slice(b"focus\t\xff\n");
        written.extend_from_slice(b"hidden\t1\nview\t1x1@9\n");
        let mut heard = Vec::new();
        listen(written.as_slice(), |told| heard.push(told));
        assert_eq!(
            heard,
            vec![
                Heard::Focused(false),
                view(800, 500, 2.0),
                Heard::Hidden(true)
            ]
        );
    }

    #[test]
    fn a_burst_is_heard_only_to_the_most_a_second() {
        let written = "focus\t1\n".repeat(MOST_A_SECOND * 3);
        let mut heard = 0;
        listen(written.as_bytes(), |_| heard += 1);
        assert_eq!(heard, MOST_A_SECOND);
    }
}
