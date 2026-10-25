/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What Chrome has said, held until it is taken — bounded by count **and**
//! by bytes (queue item 653).
//!
//! [`crate::chrome`] reads Chrome's pipe on a thread of its own, so that
//! waiting for Chrome is a wait with a bound, and hands each message across
//! to whoever is waiting for an answer. What has been read and not yet taken
//! is held here. Until item 653 it was held by count alone — 256 messages,
//! each up to [`crate::chrome_wire::MOST_MESSAGE`] — so a page whose script
//! wrote long lines to its console while a verb ran could have the browser
//! process holding some four gibibytes while the tab read them.
//!
//! So a message is held only while there is room for it in both: fewer than
//! the most messages, and its bytes with everything already held no more
//! than the most bytes. Past either, the reader **waits**, and so does
//! Chrome's writing behind it: a far side telling of more than is taken is
//! held back in its own pipe, never stored here.
//!
//! **A message is always held when nothing else is**, whatever its size, so
//! that one longer than the bound cannot leave the reader and the taker
//! waiting on each other for ever. [`crate::chrome`] keeps its bound in
//! bytes above the longest message the pipe reads, so this never lets what
//! is held run past the bound; it is here so that a bound set below that is
//! a slow pipe rather than a stopped one.
//!
//! What a message weighs is its teller's to say ([`Teller::tell`]): the
//! bytes it holds, as near as the teller can count them.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

/// What is held, and whether either end has gone.
struct State<T> {
    /// Each message, oldest first, with what it weighs.
    waiting: VecDeque<(T, usize)>,
    /// What every message waiting weighs, together.
    bytes: usize,
    /// The teller has gone: nothing more will be told.
    told_all: bool,
    /// The taker has gone: nothing told will be taken.
    taken_all: bool,
}

/// The state, and the one signal each end waits on.
struct Shared<T> {
    state: Mutex<State<T>>,
    /// Told whenever a message is held, one is taken, or an end goes.
    changed: Condvar,
    most_messages: usize,
    most_bytes: usize,
}

impl<T> Shared<T> {
    /// The state, locked. Nothing here panics while holding it, so a
    /// poisoned lock is only another thread's panic, and what it guards is
    /// still whole.
    fn lock(&self) -> MutexGuard<'_, State<T>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Whether a message weighing `bytes` can be held beside what is.
    fn has_room(&self, state: &State<T>, bytes: usize) -> bool {
        state.waiting.is_empty()
            || (state.waiting.len() < self.most_messages
                && state.bytes.saturating_add(bytes) <= self.most_bytes)
    }
}

/// The end that tells: the thread reading Chrome's pipe.
pub struct Teller<T> {
    shared: Arc<Shared<T>>,
}

/// The end that takes: whoever waits for Chrome's answer.
pub struct Taker<T> {
    shared: Arc<Shared<T>>,
}

/// Why nothing was taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Untaken {
    /// Nothing was told by the time waited until.
    Waited,
    /// The teller has gone and everything it told has been taken.
    Ended,
}

/// A hold of at most `most_messages` messages and, past the first,
/// `most_bytes` bytes: its two ends.
pub fn held<T>(most_messages: usize, most_bytes: usize) -> (Teller<T>, Taker<T>) {
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            waiting: VecDeque::new(),
            bytes: 0,
            told_all: false,
            taken_all: false,
        }),
        changed: Condvar::new(),
        most_messages,
        most_bytes,
    });
    (
        Teller {
            shared: Arc::clone(&shared),
        },
        Taker { shared },
    )
}

impl<T> Teller<T> {
    /// Hold `message`, weighing `bytes`, waiting for as long as there is no
    /// room for it.
    ///
    /// # Errors
    ///
    /// The message back, when the taker has gone and nothing will take it.
    pub fn tell(&self, message: T, bytes: usize) -> Result<(), T> {
        let mut state = self.shared.lock();
        loop {
            if state.taken_all {
                return Err(message);
            }
            if self.shared.has_room(&state, bytes) {
                break;
            }
            state = self
                .shared
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        state.bytes = state.bytes.saturating_add(bytes);
        state.waiting.push_back((message, bytes));
        drop(state);
        self.shared.changed.notify_all();
        Ok(())
    }
}

impl<T> Drop for Teller<T> {
    fn drop(&mut self) {
        self.shared.lock().told_all = true;
        self.shared.changed.notify_all();
    }
}

impl<T> Taker<T> {
    /// The oldest message held, waiting no later than `until` for one.
    ///
    /// # Errors
    ///
    /// [`Untaken::Waited`] when none was told by then, or
    /// [`Untaken::Ended`] when none ever will be.
    pub fn take(&self, until: Instant) -> Result<T, Untaken> {
        let mut state = self.shared.lock();
        loop {
            if let Some((message, bytes)) = state.waiting.pop_front() {
                state.bytes = state.bytes.saturating_sub(bytes);
                drop(state);
                self.shared.changed.notify_all();
                return Ok(message);
            }
            if state.told_all {
                return Err(Untaken::Ended);
            }
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(Untaken::Waited);
            }
            state = self
                .shared
                .changed
                .wait_timeout(state, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// What everything held and not yet taken weighs, together.
    pub fn bytes(&self) -> usize {
        self.shared.lock().bytes
    }

    /// How many messages are held and not yet taken.
    pub fn messages(&self) -> usize {
        self.shared.lock().waiting.len()
    }
}

impl<T> Drop for Taker<T> {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        state.taken_all = true;
        // Nothing will take them: what they hold is let go now, not when
        // the teller ends.
        state.waiting.clear();
        state.bytes = 0;
        drop(state);
        self.shared.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::{Untaken, held};
    use std::time::{Duration, Instant};

    fn soon() -> Instant {
        Instant::now() + Duration::from_millis(50)
    }

    /// Whether `tell` is still waiting after a while, from another thread.
    fn waits(tell: impl FnOnce() + Send + 'static) -> bool {
        let (done, finished) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            tell();
            let _ = done.send(());
        });
        finished.recv_timeout(Duration::from_millis(200)).is_err()
    }

    #[test]
    fn messages_are_taken_oldest_first_and_weighed_until_taken() {
        let (teller, taker) = held(8, 100);
        teller.tell("a", 10).unwrap();
        teller.tell("b", 20).unwrap();
        assert_eq!((taker.messages(), taker.bytes()), (2, 30));
        assert_eq!(taker.take(soon()), Ok("a"));
        assert_eq!(taker.bytes(), 20);
        assert_eq!(taker.take(soon()), Ok("b"));
        assert_eq!((taker.messages(), taker.bytes()), (0, 0));
        assert_eq!(taker.take(soon()), Err(Untaken::Waited));
    }

    #[test]
    fn a_message_past_the_bytes_waits_until_there_is_room() {
        let (teller, taker) = held(8, 100);
        teller.tell(1, 60).unwrap();
        let teller = std::sync::Arc::new(teller);
        let again = std::sync::Arc::clone(&teller);
        // 60 and 41 is past 100.
        assert!(waits(move || {
            let _ = again.tell(2, 41);
        }));
        assert_eq!(taker.bytes(), 60);
        // Taking the first makes room, and the second is held.
        assert_eq!(taker.take(soon()), Ok(1));
        assert_eq!(taker.take(Instant::now() + Duration::from_secs(5)), Ok(2));
        // Exactly the bound is room.
        teller.tell(3, 60).unwrap();
        teller.tell(4, 40).unwrap();
        assert_eq!(taker.bytes(), 100);
    }

    #[test]
    fn a_message_past_the_count_waits_whatever_it_weighs() {
        let (teller, taker) = held(2, 1000);
        teller.tell(1, 0).unwrap();
        teller.tell(2, 0).unwrap();
        let teller = std::sync::Arc::new(teller);
        let again = std::sync::Arc::clone(&teller);
        assert!(waits(move || {
            let _ = again.tell(3, 0);
        }));
        assert_eq!(taker.messages(), 2);
    }

    #[test]
    fn a_message_heavier_than_the_bound_is_held_alone() {
        let (teller, taker) = held(8, 100);
        teller.tell("heavy", 500).unwrap();
        assert_eq!(taker.bytes(), 500);
        let teller = std::sync::Arc::new(teller);
        let again = std::sync::Arc::clone(&teller);
        assert!(waits(move || {
            let _ = again.tell("light", 1);
        }));
        assert_eq!(taker.take(soon()), Ok("heavy"));
        assert_eq!(
            taker.take(Instant::now() + Duration::from_secs(5)),
            Ok("light")
        );
    }

    #[test]
    fn what_was_told_is_taken_after_the_teller_goes_and_then_it_has_ended() {
        let (teller, taker) = held(8, 100);
        teller.tell(1, 1).unwrap();
        drop(teller);
        assert_eq!(taker.take(soon()), Ok(1));
        assert_eq!(taker.take(soon()), Err(Untaken::Ended));
    }

    #[test]
    fn a_teller_waiting_for_room_is_answered_when_the_taker_goes() {
        let (teller, taker) = held(1, 100);
        teller.tell(1, 1).unwrap();
        let (answer, answered) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = answer.send(teller.tell(2, 1));
        });
        std::thread::sleep(Duration::from_millis(50));
        drop(taker);
        assert_eq!(answered.recv_timeout(Duration::from_secs(5)), Ok(Err(2)));
    }

    #[test]
    fn a_teller_faster_than_its_taker_is_held_to_the_bound() {
        let (teller, taker) = held(1000, 1000);
        std::thread::spawn(move || {
            for each in 0..400_u32 {
                if teller.tell(each, 90).is_err() {
                    return;
                }
            }
        });
        let mut heaviest = 0;
        for expected in 0..400_u32 {
            heaviest = heaviest.max(taker.bytes());
            assert_eq!(
                taker.take(Instant::now() + Duration::from_secs(5)),
                Ok(expected)
            );
            if expected % 50 == 0 {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        // Eleven of 90 is past 1000; eleven of the count is not.
        assert!(heaviest <= 990, "{heaviest}");
        assert!(heaviest >= 900, "the taker never fell behind: {heaviest}");
    }
}
