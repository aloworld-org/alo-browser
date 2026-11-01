/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A card's window made unseen, and seen again (queue item 771, ADR 0076
//! B4 as amended): its `NSWindow` fully transparent and letting a person's
//! pointer through to whatever is under it, or opaque and taking it.
//!
//! Measured through this host on macOS 26.5 (CEF 156.0.14): a window at
//! `alphaValue` 0 goes on painting and streaming its page at sixty frames a
//! second, its page `visible`, and input sent over the pipe lands on it;
//! the window server lists it with alpha 0 and nothing of it is in a
//! screenshot. Transparent alone, a person's click on the strip of it the
//! window server keeps on a display still reached the page — so it also
//! ignores the pointer, and the click goes to what is under it.
//!
//! CEF's views give no window a transparency, so the window is `AppKit`'s,
//! found among the application's windows by its content view, which is
//! what CEF gives as a views window's handle on macOS (measured). Each call
//! here is a safe one of `objc2-app-kit`'s; a pointer is compared by its
//! address and never followed. No `unsafe` (ADR 0080's boundary is
//! `application_mac.rs` alone).

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSApp;
use std::ffi::c_void;

/// Make the window whose content view is `handle` unseen — fully
/// transparent, letting the pointer through — or, not `unseen`, seen and
/// taking the pointer. Nothing when not on the main thread, where `AppKit`
/// keeps its windows, or when no window of the application has that view.
pub fn unseen(handle: *mut c_void, unseen: bool) {
    let Some(main) = MainThreadMarker::new() else {
        return;
    };
    let named = handle.addr();
    for window in &NSApp(main).windows() {
        let content = window
            .contentView()
            .map(|view| Retained::as_ptr(&view).addr());
        if content == Some(named) {
            window.setAlphaValue(if unseen { 0.0 } else { 1.0 });
            window.setIgnoresMouseEvents(unseen);
        }
    }
}
