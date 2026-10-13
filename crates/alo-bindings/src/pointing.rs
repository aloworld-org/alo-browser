/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a pointer was and what it pressed, as a `MouseEvent` or a
//! `PointerEvent` reports it (ADR 0018 § 5, ADR 0024 § 5, queue item 535).
//!
//! Two things make one of these events: a click keyboard activation fires,
//! which is an agent's `Activate` and a script's `el.click()`, and a
//! person's mouse. The first points at nothing, so its numbers are
//! [`Pointing::KEYBOARD`]'s — every coordinate `0`, no button, no device —
//! and are not a position somebody pressed. The second carries the
//! numbers the renderer worked out from the point the browser process sent
//! (`alo-renderer`'s `event_loop/pointed.rs` holds which event gets which,
//! as Chrome 154 was measured to give them).
//!
//! # Held as given
//!
//! A coordinate is held as the event reports it: a `PointerEvent`'s as the
//! point arrived, a `MouseEvent`'s and a `click`'s already floored to whole
//! pixels. This file decides nothing about which; it carries the number.
//! A point crosses the boundary as an `f32`, and is reported as that
//! `f32`'s exact value as a double — which is what Chrome reports
//! (`70.5999984741211` for a point at 70.6).
//!
//! # Not the screen
//!
//! `screenX` and `screenY` report the same numbers as `clientX` and
//! `clientY`: the window as though it were at the screen's origin. Where
//! the window is on the person's screen is beyond the window, which ADR
//! 0038 § 1 keeps from every page, and this is the least informative value
//! that is still a position on the same scale.

/// What did the pointing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    /// Nothing: a click keyboard activation fires.
    None,
    /// A mouse.
    Mouse,
}

impl Device {
    /// `pointerId`: `-1` for a click no pointing device caused, which is
    /// what Pointer Events gives it, and `1` for the mouse, which is the id
    /// Chrome 154 gives its one mouse.
    pub const fn id(self) -> i32 {
        match self {
            Self::None => -1,
            Self::Mouse => 1,
        }
    }

    /// `pointerType`: the empty string for no device, and `"mouse"`.
    pub const fn kind(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Mouse => "mouse",
        }
    }
}

/// What a `MouseEvent` or a `PointerEvent` reports about its pointer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointing {
    /// `clientX`, and `screenX`, in CSS pixels from the viewport's left.
    pub x: f32,
    /// `clientY`, and `screenY`, in CSS pixels from the viewport's top.
    pub y: f32,
    /// `button`: which button changed — `0` the main one — or `-1` for
    /// none.
    pub button: i16,
    /// `buttons`: which are held down, as UI Events' bits — `1` the main
    /// one.
    pub buttons: u16,
    /// `detail`: how many times in quick succession the button was
    /// pressed, for a `mousedown`, `mouseup` or `click`; `0` for a
    /// `PointerEvent` that is not a click, and for a click nothing pressed.
    pub detail: u32,
    /// What pointed.
    pub device: Device,
}

impl Pointing {
    /// A click keyboard activation fires: nothing pointed, so every number
    /// is `0` and there is no device (ADR 0018 § 5).
    pub const KEYBOARD: Self = Self {
        x: 0.0,
        y: 0.0,
        button: 0,
        buttons: 0,
        detail: 0,
        device: Device::None,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_keyboards_click_points_at_nothing() {
        let keyboard = Pointing::KEYBOARD;
        assert_eq!(
            (keyboard.button, keyboard.buttons, keyboard.detail),
            (0, 0, 0)
        );
        assert_eq!(keyboard.x.to_bits(), 0);
        assert_eq!(keyboard.y.to_bits(), 0);
        assert_eq!(keyboard.device.id(), -1);
        assert_eq!(keyboard.device.kind(), "");
    }

    #[test]
    fn a_mouse_is_the_pointer_chrome_numbers_one() {
        assert_eq!(Device::Mouse.id(), 1);
        assert_eq!(Device::Mouse.kind(), "mouse");
    }
}
