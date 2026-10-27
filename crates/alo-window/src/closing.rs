/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a program does as it ends, run once however it ends (queue item
//! 688, ADR 0068 § 2).
//!
//! `alo --chrome` ends three ways: Chrome's window closed, the frame of
//! ours closed, or the program quit. The first two return from the frame's
//! event loop, and what follows it in `alo` closes Chrome, waits for it and
//! removes its profile. **Quit does not return.** `AppKit` ends the process
//! as soon as it has told the application it will (ADR 0067 § 1), so what
//! was written after the event loop never ran: no Chrome was left, because
//! Chrome ends when its pipe closes, but its profile was — 67 MB after one
//! page, and whatever the session's sites stored in it.
//!
//! The telling is heard without code of ours. `winit` is the application's
//! delegate, and on `applicationWillTerminate:` it ends the event loop as it
//! would have ended it itself, calling the handler's `exiting` before
//! `AppKit` goes on — measured on this machine, macOS 26.5.2 with `winit`
//! 0.30.13, by `tests/the_application_opens_its_own_page.rs`, which quits
//! the application and finds no profile left. So the shutdown is handed to
//! the frame as a [`Closing`], run in `exiting`: on every way out, before
//! the process ends, and once.
//!
//! A `Closing` that was never run — the frame refused before its event loop
//! began — is run when it is dropped, so no way out forgets it.

/// One shutdown, run at most once: when [`Closing::run`] is first called, or
/// when it is dropped unrun.
pub struct Closing<'a> {
    shutdown: Option<Box<dyn FnOnce() + 'a>>,
}

impl<'a> Closing<'a> {
    /// A closing that runs `shutdown`.
    pub fn of(shutdown: impl FnOnce() + 'a) -> Self {
        Self {
            shutdown: Some(Box::new(shutdown)),
        }
    }

    /// Run the shutdown, unless it has run already.
    pub fn run(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            shutdown();
        }
    }
}

impl Drop for Closing<'_> {
    fn drop(&mut self) {
        self.run();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn a_closing_runs_once_however_often_it_is_asked() {
        let runs = Cell::new(0);
        let mut closing = Closing::of(|| runs.set(runs.get() + 1));
        closing.run();
        assert_eq!(runs.get(), 1);
        closing.run();
        drop(closing);
        assert_eq!(runs.get(), 1);
    }

    #[test]
    fn a_closing_never_run_runs_as_it_is_dropped() {
        let runs = Cell::new(0);
        let closing = Closing::of(|| runs.set(runs.get() + 1));
        assert_eq!(runs.get(), 0);
        drop(closing);
        assert_eq!(runs.get(), 1);
    }
}
