/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The host's application object: the one `unsafe` boundary written in
//! alo's repository (ADR 0080, queue item 731).
//!
//! # Why this file holds `unsafe`
//!
//! Chromium on macOS asks the application object `-isHandlingSendEvent`
//! and tells it `-setHandlingSendEvent:` (Chromium's `CrAppProtocol` and
//! `CrAppControlProtocol`, which CEF's `CefAppProtocol` gathers), and
//! `AppKit`'s own `NSApplication` answers neither. Without a class that
//! does, a shown host crashed — `-[NSApplication isHandlingSendEvent]:
//! unrecognized selector`, `SIGTRAP`, a crash report and macOS's dialog —
//! on a page closed from inside, a context menu and a closed list's menu
//! (measured, queue item 727). CEF's own samples answer it with an
//! `NSApplication` subclass (`SimpleApplication`, `ClientApplication`);
//! this is that subclass, defined with `objc2`'s `define_class!`, and
//! defining an Objective-C class is `unsafe` in Rust: the compiler cannot
//! check that a method's Rust signature is the one the Objective-C runtime
//! will call it with, nor that the superclass named is the real one. The
//! owner allowed it here and nowhere else (ADR 0080, 30 October 2026), and
//! `scripts/gate.sh` refuses `unsafe` in any other file of the host or
//! anywhere in the repository's workspace.
//!
//! # What each `unsafe` is
//!
//! - `#[unsafe(super(NSApplication))]`: the class's superclass, `AppKit`'s
//!   `NSApplication`, which `objc2-app-kit` declares.
//! - `#[unsafe(method(…))]`, five: `-sendEvent:` (`AppKit`'s, overridden
//!   to say it is handling an event while its own runs), `-terminate:`
//!   (`AppKit`'s, overridden to close the host's pages and leave the
//!   message loop instead of ending the process at once), and the
//!   protocols' `-isHandlingSendEvent` and `-setHandlingSendEvent:`. Each
//!   takes and returns exactly what its Objective-C declaration does.
//! - `unsafe impl` of four protocols: `NSObjectProtocol`, which
//!   `NSApplication` already conforms to, and the three Chromium asks for,
//!   as the `cef` crate declares them (`cef::application_mac`).
//! - Two messages: `[super sendEvent:event]`, the method overridden, with
//!   the event it was given; and `[AloCefHostApplication
//!   sharedApplication]`, `AppKit`'s class method, which makes this class
//!   the shared application when nothing has made one yet.
//! - One selector given to a menu item ([`item`]): each [`Action`] names a
//!   method `AppKit` or Chromium's views answer with the sender as its one
//!   argument, sent up the responder chain.
//!
//! What is kept here is state for the one application object a process
//! has, so it is in two statics of safe types rather than in instance
//! variables, which `objc2` would require initialised by an `-init` of
//! ours that `+sharedApplication` does not run.
//!
//! # Made before CEF starts
//!
//! [`install`] makes it the shared application after CEF's framework is
//! loaded — so the protocols it conforms to are the framework's — and
//! before CEF starts, as CEF's samples do, and refuses when another object
//! was made first: a host whose application object is not this one is the
//! host that crashed.

use cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, NSObjectProtocol, Sel};
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSApp, NSApplication, NSEvent, NSMenuItem};
use objc2_foundation::NSString;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the application is inside its own `-sendEvent:`, as Chromium
/// asks with `-isHandlingSendEvent`.
static HANDLING_SEND_EVENT: AtomicBool = AtomicBool::new(false);

/// What `-terminate:` does instead of ending the process: given once, by
/// [`install`].
static TERMINATE: OnceLock<fn()> = OnceLock::new();

define_class!(
    /// `NSApplication`, answering Chromium's `CrAppProtocol`,
    /// `CrAppControlProtocol` and CEF's `CefAppProtocol`, and leaving
    /// `-terminate:` to the host.
    #[unsafe(super(NSApplication))]
    #[name = "AloCefHostApplication"]
    struct Application;

    impl Application {
        /// `AppKit`'s own, saying all the while that an event is being
        /// handled — unless one already was, whose handler says when it
        /// ends.
        #[unsafe(method(sendEvent:))]
        fn send_event(&self, event: &NSEvent) {
            let was = HANDLING_SEND_EVENT.swap(true, Ordering::Relaxed);
            // SAFETY: the method this one overrides, on the same object,
            // with the event it was given.
            let () = unsafe { msg_send![super(self), sendEvent: event] };
            HANDLING_SEND_EVENT.store(was, Ordering::Relaxed);
        }

        /// What Quit, a logout and `Browser.close` reach. `AppKit`'s own
        /// ends the process at once, so CEF never shuts down; the host's
        /// closes its pages instead, and leaves the message loop once
        /// they are closed.
        #[unsafe(method(terminate:))]
        fn terminate(&self, _sender: Option<&AnyObject>) {
            if let Some(terminate) = TERMINATE.get() {
                terminate();
            }
        }
    }

    unsafe impl NSObjectProtocol for Application {}

    unsafe impl CrAppProtocol for Application {
        #[unsafe(method(isHandlingSendEvent))]
        fn is_handling_send_event(&self) -> Bool {
            Bool::new(HANDLING_SEND_EVENT.load(Ordering::Relaxed))
        }
    }

    unsafe impl CrAppControlProtocol for Application {
        #[unsafe(method(setHandlingSendEvent:))]
        fn set_handling_send_event(&self, handling: Bool) {
            HANDLING_SEND_EVENT.store(handling.as_bool(), Ordering::Relaxed);
        }
    }

    unsafe impl CefAppProtocol for Application {}
);

/// Why the application object could not be the host's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Uninstalled {
    /// Not on the process's main thread, where `AppKit` lives.
    NotMainThread,
    /// Another application object was made first.
    Another,
}

/// Make the host's application the shared application, `terminate`
/// standing for `AppKit`'s `-terminate:`.
///
/// # Errors
///
/// [`Uninstalled`], with nothing made.
pub fn install(terminate: fn()) -> Result<(), Uninstalled> {
    let main = MainThreadMarker::new().ok_or(Uninstalled::NotMainThread)?;
    let _ = TERMINATE.set(terminate);
    // SAFETY: `AppKit`'s class method, sent to a subclass of the class that
    // declares it; it returns the shared application, made now as one of
    // this class unless one was made before.
    let _: Retained<NSApplication> = unsafe { msg_send![Application::class(), sharedApplication] };
    if NSApp(main).isKindOfClass(Application::class()) {
        Ok(())
    } else {
        Err(Uninstalled::Another)
    }
}

/// What a menu item does, each a method `AppKit` or Chromium's views
/// answer, sent up the responder chain with the item as its argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `-terminate:`, the host's own (above).
    Quit,
    /// `-performClose:`, the key window's, as its close button does.
    CloseWindow,
    /// `-cut:`, the focused page's.
    Cut,
    /// `-copy:`, the focused page's.
    Copy,
    /// `-paste:`, the focused page's.
    Paste,
    /// `-selectAll:`, the focused page's.
    SelectAll,
}

impl Action {
    /// The selector the item sends.
    fn selector(self) -> Sel {
        match self {
            Action::Quit => sel!(terminate:),
            Action::CloseWindow => sel!(performClose:),
            Action::Cut => sel!(cut:),
            Action::Copy => sel!(copy:),
            Action::Paste => sel!(paste:),
            Action::SelectAll => sel!(selectAll:),
        }
    }
}

/// A menu item titled `title`, doing `action`, with ⌘ and `key` as its
/// shortcut.
pub fn item(
    main: MainThreadMarker,
    title: &str,
    action: Action,
    key: &str,
) -> Retained<NSMenuItem> {
    // SAFETY: each selector is one `AppKit` or Chromium's views answer, with
    // the sender as its one argument ([`Action`]).
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(main),
            &NSString::from_str(title),
            Some(action.selector()),
            &NSString::from_str(key),
        )
    }
}
