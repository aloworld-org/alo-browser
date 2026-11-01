/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A card unseen, and seen again (queue item 771, ADR 0076 B4 as amended;
//! queue items 781 and 739): what `AppKit` is told so that a parked card is
//! nowhere a person looks — its `NSWindow` fully transparent and letting a
//! person's pointer through to whatever is under it, and its application
//! kept out of the Dock, ⌘-Tab and Stage Manager's strip — and told again
//! once alo has put it where a person is to see it. One reason to change:
//! what keeps a card from a person until *Expand*.
//!
//! # The window
//!
//! Measured through this host on macOS 26.5 (CEF 156.0.14): a window at
//! `alphaValue` 0 goes on painting and streaming its page at sixty frames a
//! second, its page `visible`, and input sent over the pipe lands on it;
//! the window server lists it with alpha 0 and nothing of it is in a
//! screenshot. Transparent alone, a person's click on the strip of it the
//! window server keeps on a display still reached the page — so it also
//! ignores the pointer, and the click goes to what is under it.
//!
//! And it floats (`NSFloatingWindowLevel`, the window server's layer 3)
//! over every ordinary window while it is unseen. Chromium stops laying out
//! and drawing a page whose window another window covers — macOS says it is
//! occluded, transparent or not — and a card's window is shown where the
//! window server chooses, under the conversation's own window once the host
//! is no longer made active by showing it: measured in the application, the
//! card stayed blank, its window never laid out at the card's size (*still
//! moving*) and so never parked; with alo's conversation in front of it,
//! the page said `hidden` once resized. Floating, it said `visible` and was
//! laid out, fitted and parked; and the strip of it the window server
//! keeps on a display is above any ordinary window there (reasoned from
//! the level, not measured). A floating window is also one Stage Manager
//! leaves out of its strip (measured, an ordinary application's too).
//! Seen, it is an ordinary window again.
//!
//! CEF's views give no window a transparency, so the window is `AppKit`'s,
//! found among the application's windows by its content view, which is
//! what CEF gives as a views window's handle on macOS (measured).
//!
//! # The application
//!
//! A transparent window of an ordinary application is still that
//! application's: Stage Manager put the parked window on the conversation's
//! own display into its strip as a blank tile with the host's icon, the
//! Dock showed the host and ⌘-Tab listed it, bringing nothing visible to the
//! front (measured, queue items 772 and 739). An **accessory** application
//! (`NSApplicationActivationPolicyAccessory`) is in none of them: measured
//! through this host, a card's window then streams as before (38–41 frames
//! a second of a noisy page at two, `requestAnimationFrame` at sixty, a
//! press over the pipe landing), is moved and parked where alo asks, and
//! with another application brought in front on its display is listed
//! where it was parked, never shrunk into a tile; macOS says the process is
//! a `UIElement`, and it is not in the Dock. Made accessory before CEF
//! starts, the host is never made the active application by showing its
//! window — the conversation keeps the keyboard — where an ordinary one
//! was. Once seen it is an ordinary application again — in the Dock and
//! ⌘-Tab, its menu bar shown — as a window a person works in is.
//!
//! Each call here is a safe one of `objc2-app-kit`'s; a pointer is compared
//! by its address and never followed. No `unsafe` (ADR 0080's boundary is
//! `application_mac.rs` alone).

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApp, NSApplicationActivationPolicy, NSFloatingWindowLevel, NSNormalWindowLevel,
};
use std::ffi::c_void;

/// Make the window whose content view is `handle` unseen — fully
/// transparent, letting the pointer through, floating over every ordinary
/// window — or, not `unseen`, seen, taking the pointer, and an ordinary
/// window. Nothing when not on the main thread, where `AppKit`
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
            window.setLevel(if unseen {
                NSFloatingWindowLevel
            } else {
                NSNormalWindowLevel
            });
        }
    }
}

/// Make the application `listed` among a person's applications — the Dock,
/// ⌘-Tab, Stage Manager — as an ordinary one is, or, not `listed`, an
/// accessory application, which is in none of them and is not made active
/// by showing a window. Said on the error output when macOS refused it;
/// nothing when not on the main thread, where the application object is.
pub fn listed(listed: bool) {
    let Some(main) = MainThreadMarker::new() else {
        return;
    };
    let policy = if listed {
        NSApplicationActivationPolicy::Regular
    } else {
        NSApplicationActivationPolicy::Accessory
    };
    if !NSApp(main).setActivationPolicy(policy) {
        eprintln!("alo's CEF host: macOS kept the application's activation policy");
    }
}
