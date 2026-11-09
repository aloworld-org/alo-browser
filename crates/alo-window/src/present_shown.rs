/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A frame of known colours shown in a window of its own, one way or the
//! other ([`How`]), for the screen to be read back (queue item 873, ADR
//! 0105): what `alo-present` runs, and what
//! `the_window_on_screen_kept_and_copied.rs` captures, so that the window
//! put on screen from pixels kept on the GPU is compared with `softbuffer`'s
//! in the same run.
//!
//! The second file that may name `winit`: a window that is not the
//! browser's, opened, shown and closed, and nothing else. Every `winit`
//! function called here is a safe one also called by [`crate::window`].

use crate::present_chosen::{How, Screen};
use std::io::Read;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

/// The flat colours across the top half of the frame, left to right, each
/// a sixteenth of its width: primaries, greys, alo's own `#e62828` and
/// accents.
pub const STRIPES: [u32; 16] = [
    0x00e6_2828,
    0x00ff_0000,
    0x0000_ff00,
    0x0000_00ff,
    0x0080_8080,
    0x0040_4040,
    0x00c0_c0c0,
    0x00ff_ffff,
    0x0000_0000,
    0x0074_efcd,
    0x00ff_ff00,
    0x0000_ffff,
    0x00ff_00ff,
    0x0076_7676,
    0x00e7_6f51,
    0x0029_335c,
];

/// The frame of `width` by `height` pixels, as words row by row: the
/// [`STRIPES`] over the top half, and below it cells of eight by eight
/// pixels, each another colour, about four thousand of them.
pub fn pattern(width: u32, height: u32) -> Vec<u32> {
    let mut words = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            words.push(if y < height / 2 {
                let stripe = u64::from(x) * 16 / u64::from(width.max(1));
                usize::try_from(stripe)
                    .ok()
                    .and_then(|stripe| STRIPES.get(stripe))
                    .copied()
                    .unwrap_or(0)
            } else {
                let cell = (y / 8) * width.div_ceil(8) + x / 8;
                let red = (cell * 7) % 256;
                let green = (cell * 13 + cell / 256 * 3) % 256;
                let blue = (cell * 29 + cell / 65) % 256;
                (red << 16) | (green << 8) | blue
            });
        }
    }
    words
}

/// Show [`pattern`] in a window of `points` at `at` on the screen, put on
/// it as `how` says, until this process's standard input ends.
///
/// Once the first frame is shown it writes one line to standard output:
/// `shown kept` or `shown copied`, the bytes handed on, and the frame's
/// size in pixels — `shown kept 7840000 2800x700`.
///
/// # Errors
///
/// No event loop, no window, or neither way onto the screen.
pub fn show_until_told(how: How, points: (f64, f64), at: (f64, f64)) -> Result<(), String> {
    let event_loop = EventLoop::<()>::with_user_event()
        .build()
        .map_err(|why| format!("no event loop: {why}"))?;
    let proxy = event_loop.create_proxy();
    std::thread::spawn(move || {
        let mut rest = Vec::new();
        let _ = std::io::stdin().read_to_end(&mut rest);
        let _ = proxy.send_event(());
    });
    let mut shown = Shown {
        how,
        points,
        at,
        window: None,
        screen: None,
        said: false,
        failed: None,
    };
    event_loop
        .run_app(&mut shown)
        .map_err(|why| format!("the event loop stopped: {why}"))?;
    shown.failed.map_or(Ok(()), Err)
}

struct Shown {
    how: How,
    points: (f64, f64),
    at: (f64, f64),
    window: Option<Arc<Window>>,
    screen: Option<Screen<Arc<Window>>>,
    said: bool,
    failed: Option<String>,
}

impl ApplicationHandler for Shown {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = WindowAttributes::default()
            .with_title("alo-present")
            .with_resizable(false)
            .with_inner_size(LogicalSize::new(self.points.0, self.points.1))
            .with_position(LogicalPosition::new(self.at.0, self.at.1));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(why) => {
                self.failed = Some(format!("no window: {why}"));
                event_loop.exit();
                return;
            }
        };
        match Screen::over(Arc::clone(&window), self.how) {
            Ok((screen, missed)) => {
                if let Some(why) = missed {
                    eprintln!("alo-present: the pixels are copied: {why}");
                }
                self.screen = Some(screen);
            }
            Err(why) => {
                self.failed = Some(why);
                event_loop.exit();
                return;
            }
        }
        window.focus_window();
        window.request_redraw();
        self.window = Some(window);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, (): ()) {
        event_loop.exit();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => {
                let (Some(window), Some(screen)) = (&self.window, self.screen.as_mut()) else {
                    return;
                };
                let size = window.inner_size();
                let words = pattern(size.width, size.height);
                let whole = crate::conversation_composed::Recomposed::Whole;
                match screen.show((size.width, size.height), &words, &whole) {
                    Ok(written) if !self.said => {
                        self.said = true;
                        let way = if screen.is_kept() { "kept" } else { "copied" };
                        println!("shown {way} {written} {}x{}", size.width, size.height);
                    }
                    Ok(_) => {}
                    Err(why) => {
                        self.failed = Some(why);
                        event_loop.exit();
                    }
                }
            }
            WindowEvent::Resized(_) | WindowEvent::Occluded(false) => {
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pattern_is_the_stripes_over_cells_of_their_own() {
        let words = pattern(160, 40);
        assert_eq!(words.len(), 160 * 40);
        for (stripe, colour) in STRIPES.iter().enumerate() {
            assert_eq!(words[stripe * 10], *colour, "stripe {stripe}");
            assert_eq!(words[19 * 160 + stripe * 10 + 9], *colour);
        }
        assert_ne!(words[24 * 160], words[24 * 160 + 8], "cells differ");
        assert_eq!(words[24 * 160], words[31 * 160 + 7], "a cell is one colour");
    }
}
