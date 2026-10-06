/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Putting composed pixels in the window: the one file that may name
//! `softbuffer` (ADR 0024 § 1).
//!
//! It copies and nothing else. What the pixels are is [`crate::compose`]'s,
//! and is tested there by reference render; this turns each into the 32-bit
//! word `softbuffer` takes and hands the buffer to the window server.
//!
//! # No `unsafe`, and why that is checked rather than hoped
//!
//! `softbuffer` reaches the window server through `unsafe` of its own (on
//! macOS, through the `objc2` crates), which ADR 0010 calls the crate's, not
//! ours. Every function called here — `Context::new`, `Surface::new`,
//! `Surface::resize`, `Surface::buffer_mut`, `Buffer::present`, and the
//! buffer's `DerefMut` to `[u32]` — is a safe function in `softbuffer` 0.4.8's
//! source, which the commit adding this file checked (ADR 0024 § 1's stop
//! rule). The workspace forbids `unsafe` here at the compiler regardless.
//!
//! `raw-window-handle` is named for its two traits only: they are the bound
//! `softbuffer` puts on a window, and naming them is what keeps `winit` out of
//! this file.

use alo_paint::Canvas;
use alo_value::Rgba;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;

/// A window's surface, ready to be given pixels.
pub struct Presenter<W> {
    surface: Surface<W, W>,
}

impl<W> std::fmt::Debug for Presenter<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Presenter")
    }
}

impl<W: HasDisplayHandle + HasWindowHandle + Clone> Presenter<W> {
    /// A surface over `window`.
    ///
    /// # Errors
    ///
    /// What `softbuffer` said, in words, when the window server would not give
    /// it a surface.
    pub fn over(window: W) -> Result<Self, String> {
        let context = Context::new(window.clone())
            .map_err(|why| format!("no drawing context for the window: {why}"))?;
        let surface = Surface::new(&context, window)
            .map_err(|why| format!("no surface on the window: {why}"))?;
        Ok(Self { surface })
    }

    /// Show `canvas` in the window.
    ///
    /// A canvas of no size shows nothing and is not an error: a window can be
    /// that small for a moment.
    ///
    /// # Errors
    ///
    /// What `softbuffer` said when the buffer could not be had or shown.
    pub fn show(&mut self, canvas: &Canvas) -> Result<(), String> {
        let (Some(width), Some(height)) = (
            NonZeroU32::new(canvas.width()),
            NonZeroU32::new(canvas.height()),
        ) else {
            return Ok(());
        };
        self.surface
            .resize(width, height)
            .map_err(|why| format!("the surface could not be resized: {why}"))?;
        let mut buffer = self
            .surface
            .buffer_mut()
            .map_err(|why| format!("no buffer to draw into: {why}"))?;
        for (word, pixel) in buffer.iter_mut().zip(canvas.pixels()) {
            *word = packed(*pixel);
        }
        buffer
            .present()
            .map_err(|why| format!("the buffer could not be shown: {why}"))
    }
}

/// A pixel as the word `softbuffer` takes: `0x00RRGGBB`.
///
/// Alpha is dropped because a composed window is opaque — it starts as the
/// window's background, and everything is blended over that.
pub fn packed(pixel: Rgba) -> u32 {
    let (red, green, blue, _) = pixel.to_rgba8();
    (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pixel_is_packed_red_green_blue_from_the_top() {
        assert_eq!(
            packed(Rgba::from_rgba8(0x12, 0x34, 0x56, 0xff)),
            0x0012_3456
        );
        assert_eq!(packed(Rgba::WHITE), 0x00ff_ffff);
        assert_eq!(packed(Rgba::BLACK), 0);
    }
}
