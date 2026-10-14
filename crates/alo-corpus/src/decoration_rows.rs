/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A picture's decoration lines, read as rows of long runs of ink, and two
//! such readings compared piece by piece.
//!
//! A decoration is the only thing on its row that runs unbroken for twelve
//! CSS pixels or more in a case written for it, so a row with such a run is
//! a line's row, and each such run is a piece of a line. An underline steps
//! aside where a letter's ink crosses it (queue item 579), so a line is
//! several pieces. What a letter inks inside a gap is never that long.

use alo_paint::Canvas;
use std::collections::BTreeMap;

/// The pieces of the lines in a picture: each row a line is on, and where
/// each of its pieces starts and ends, in device pixels.
pub type Lines = BTreeMap<u32, Vec<(u32, u32)>>;

/// Whether a pixel is ink: black or a strong colour, not the page's white
/// and not a letter's soft edge.
fn ink(canvas: &Canvas, x: u32, y: u32) -> bool {
    canvas.at(x, y).is_some_and(|pixel| {
        let (red, green, blue, _) = pixel.to_rgba8();
        red.min(green).min(blue) < 100
    })
}

/// Each row holding a run of ink at least `run` device pixels long, and
/// every such run on it: where it starts and ends.
pub fn lines(canvas: &Canvas, run: u32) -> Lines {
    let mut rows: Lines = BTreeMap::new();
    for y in 0..canvas.height() {
        let mut x = 0;
        while x < canvas.width() {
            if !ink(canvas, x, y) {
                x += 1;
                continue;
            }
            let start = x;
            while x < canvas.width() && ink(canvas, x, y) {
                x += 1;
            }
            if x - start >= run {
                rows.entry(y).or_default().push((start, x));
            }
        }
    }
    rows
}

/// Whether `ours` has a line on every row `theirs` has one and on no other,
/// in as many pieces, each piece's ends within a device pixel of theirs.
///
/// # Errors
///
/// The first difference, said, when it does not.
pub fn compare(theirs: &Lines, ours: &Lines) -> Result<(), String> {
    let (their_rows, our_rows): (Vec<_>, Vec<_>) = (theirs.keys().collect(), ours.keys().collect());
    if their_rows != our_rows {
        return Err(format!(
            "lines on rows {our_rows:?}, where Chrome's are on {their_rows:?}"
        ));
    }
    for ((row, their_pieces), our_pieces) in theirs.iter().zip(ours.values()) {
        if their_pieces.len() != our_pieces.len() {
            return Err(format!(
                "row {row}: Chrome's pieces {their_pieces:?}, ours {our_pieces:?}"
            ));
        }
        for ((start, end), (our_start, our_end)) in their_pieces.iter().zip(our_pieces) {
            if start.abs_diff(*our_start) > 1 || end.abs_diff(*our_end) > 1 {
                return Err(format!(
                    "row {row}: Chrome's piece runs {start}..{end}, ours {our_start}..{our_end}"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_value::Rgba;

    fn canvas_with(runs: &[(i32, i32, u32)]) -> Canvas {
        let mut canvas = Canvas::new(40, 4, Rgba::WHITE);
        for &(row, start, end) in runs {
            canvas.fill_rect(start, row, end, 1, Rgba::BLACK);
        }
        canvas
    }

    #[test]
    fn a_long_run_is_a_line_and_a_short_one_is_not() {
        // Row, start and length.
        let canvas = canvas_with(&[(1, 2, 18), (2, 4, 5), (2, 12, 18)]);
        let found = lines(&canvas, 12);
        assert_eq!(
            found,
            BTreeMap::from([(1, vec![(2, 20)]), (2, vec![(12, 30)])])
        );
    }

    #[test]
    fn a_piece_a_pixel_off_agrees_and_two_off_does_not() {
        let theirs = BTreeMap::from([(1, vec![(2, 20)])]);
        assert!(compare(&theirs, &BTreeMap::from([(1, vec![(3, 19)])])).is_ok());
        assert!(compare(&theirs, &BTreeMap::from([(1, vec![(2, 22)])])).is_err());
        assert!(compare(&theirs, &BTreeMap::from([(2, vec![(2, 20)])])).is_err());
        assert!(compare(&theirs, &BTreeMap::from([(1, vec![(2, 9), (11, 20)])])).is_err());
    }
}
