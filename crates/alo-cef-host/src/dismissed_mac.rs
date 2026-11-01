/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A context menu shown by the host's page, dismissed as a person's Escape
//! dismisses it, before its page is closed (queue item 734,
//! [`crate::after_menus`]).
//!
//! The menu is `AppKit`'s, tracking events in a run loop of its own on the
//! main thread, and neither CEF nor Chromium offers a way to close it from
//! outside. Escape does: measured with a key posted to the host's process
//! from outside (queue item 731), and here the same key is put in the
//! application's own queue of events (`-postEvent:atStart:`), where the
//! menu reads its next event from. No `unsafe` here: each call is one
//! `objc2-app-kit` declares safe.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApp, NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::{NSPoint, NSString};

/// Escape, as macOS numbers its key.
const ESCAPE: u16 = 53;

/// Put Escape — going down, then coming up — at the end of the application's
/// queue of events, for a menu shown to read: whether it was put there.
/// Nothing on any thread but the main one, where there is no menu.
pub fn escape() -> bool {
    let Some(main) = MainThreadMarker::new() else {
        return false;
    };
    let application = NSApp(main);
    let written = NSString::from_str("\u{1b}");
    for kind in [NSEventType::KeyDown, NSEventType::KeyUp] {
        let Some(event) = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            kind,
            NSPoint::new(0.0, 0.0),
            NSEventModifierFlags::empty(),
            0.0,
            0,
            None,
            &written,
            &written,
            false,
            ESCAPE,
        ) else {
            return false;
        };
        application.postEvent_atStart(&event, false);
    }
    true
}
