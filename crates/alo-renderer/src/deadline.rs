/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's script stopped at a deadline (ADR 0039 § 3, queue item 373).
//!
//! The renderer has one thread, and while a page's listener runs that
//! thread is inside it. So the only way to stop a listener that will not
//! end is from **another** thread, through the engine's [`Stop`] — which
//! the engine reads at every call and every backward jump. A [`Deadline`]
//! is that thread: armed, it waits; disarmed before its time, it asks
//! nothing; reaching its time, it asks the page to stop.
//!
//! **Its own ask is taken back when it is disarmed**, so a switch it threw
//! is left as it was found. One somebody else threw stays thrown.
//!
//! # Why one second for leaving
//!
//! [`LONGEST_LEAVING`]. An honest `pagehide` listener does a little
//! arithmetic, builds a string and asks for one thing: microseconds of
//! script. One that runs for seconds is broken, or is keeping a process the
//! person closed alive, and for a `Load` on the same site the next page
//! waits for it. One second is far beyond the first and short enough that
//! nobody waits on the second. **A policy, not a speed claim**: it changes
//! when a frozen page's honest listener is found to need more, and that page
//! is the evidence.

use core::time::Duration;
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::thread::JoinHandle;

use alo_js::interpret::Stop;

/// The longest a page's leaving steps are given (ADR 0039 § 3).
pub const LONGEST_LEAVING: Duration = Duration::from_secs(1);

/// A thread that stops a page's script if it is still running at a
/// deadline.
#[derive(Debug)]
pub(crate) struct Deadline {
    /// The switch it throws.
    stop: Stop,
    /// Dropped to disarm it.
    done: Option<Sender<()>>,
    /// The thread, which answers whether it threw the switch.
    timer: Option<JoinHandle<bool>>,
}

impl Deadline {
    /// Throw `stop` once `after` has passed, unless disarmed first.
    ///
    /// # Errors
    ///
    /// Why no thread could be made to keep the time.
    pub(crate) fn arm(stop: Stop, after: Duration) -> std::io::Result<Self> {
        let (done, waiting) = channel::<()>();
        let asking = stop.clone();
        let timer = std::thread::Builder::new()
            .name("deadline".to_owned())
            .spawn(move || match waiting.recv_timeout(after) {
                Err(RecvTimeoutError::Timeout) => {
                    asking.ask();
                    true
                }
                // Disarmed: its sender went.
                Ok(()) | Err(RecvTimeoutError::Disconnected) => false,
            })?;
        Ok(Self {
            stop,
            done: Some(done),
            timer: Some(timer),
        })
    }

    /// Disarm it, waiting for its thread to end: whether it reached its time
    /// and threw the switch, which it then takes back.
    pub(crate) fn disarm(mut self) -> bool {
        self.end()
    }

    fn end(&mut self) -> bool {
        drop(self.done.take());
        let fired = self
            .timer
            .take()
            .is_some_and(|timer| timer.join().unwrap_or(false));
        if fired {
            self.stop.clear();
        }
        fired
    }
}

impl Drop for Deadline {
    fn drop(&mut self) {
        self.end();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn disarmed_in_time_it_asks_nothing() {
        let stop = Stop::new();
        let Ok(deadline) = Deadline::arm(stop.clone(), Duration::from_secs(60)) else {
            panic!("a thread is made");
        };
        let began = Instant::now();
        assert!(!deadline.disarm());
        assert!(!stop.asked());
        assert!(began.elapsed() < Duration::from_secs(5), "it did not wait");
    }

    #[test]
    fn at_its_time_it_asks_and_disarmed_it_takes_that_back() {
        let stop = Stop::new();
        let Ok(deadline) = Deadline::arm(stop.clone(), Duration::from_millis(10)) else {
            panic!("a thread is made");
        };
        while !stop.asked() {
            std::thread::yield_now();
        }
        assert!(deadline.disarm());
        assert!(!stop.asked(), "its own ask is taken back");
    }

    #[test]
    fn a_switch_somebody_else_threw_stays_thrown() {
        let stop = Stop::new();
        let Ok(deadline) = Deadline::arm(stop.clone(), Duration::from_secs(60)) else {
            panic!("a thread is made");
        };
        stop.ask();
        assert!(!deadline.disarm());
        assert!(stop.asked());
    }
}
