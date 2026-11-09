/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo-present`: a frame of known colours shown in a window of its own,
//! from pixels kept on the GPU or copied by `softbuffer` (queue item 873,
//! ADR 0105), for a test or a person to read the screen back.
//!
//! `alo-present kept|copied [WIDTH HEIGHT [LEFT TOP]]` — the window's size
//! and place in points, 400 × 300 at 120, 120 unless named. It says
//! `shown kept …` or `shown copied …` once the frame is shown
//! ([`alo_window::present_shown::show_until_told`]) and closes when its
//! standard input ends. `kept` falls back to `copied` where the GPU cannot
//! be had, and says why on standard error.

use alo_window::present_chosen::How;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let how = match args.first().map(String::as_str) {
        Some("kept") => How::Kept,
        Some("copied") => How::Copied,
        _ => {
            eprintln!("alo-present: say kept or copied, then a width, height, left and top");
            return ExitCode::from(2);
        }
    };
    let number = |at: usize, otherwise: f64| {
        args.get(at)
            .map_or(Some(otherwise), |given| given.parse::<f64>().ok())
    };
    let (Some(width), Some(height), Some(left), Some(top)) = (
        number(1, 400.0),
        number(2, 300.0),
        number(3, 120.0),
        number(4, 120.0),
    ) else {
        eprintln!("alo-present: the size and place are numbers of points");
        return ExitCode::from(2);
    };
    match alo_window::present_shown::show_until_told(how, (width, height), (left, top)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            eprintln!("alo-present: {why}");
            ExitCode::FAILURE
        }
    }
}
