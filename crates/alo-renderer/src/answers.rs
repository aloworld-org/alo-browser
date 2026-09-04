/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Waiting for a renderer's answer, with a bound on the silence.
//!
//! [`crate::pipe`] says where a message ends; this says how long a browser
//! process is willing to wait for one. They are separate questions and this is
//! the one that was not asked: `pipe::read` blocks until bytes arrive, so a
//! renderer that is **alive and never answers** — wedged on a page, or on
//! something a hostile page arranged — hung the browser process, which is the
//! one thing ADR 0005 says must never happen. A renderer that *dies* was always
//! survivable; one that simply stops talking was not.
//!
//! # Why a thread, and why it is the only way here
//!
//! A pipe read cannot be given a deadline in safe Rust: the platform calls that
//! would do it are FFI, and law 4 forbids `unsafe` outside an ADR'd boundary —
//! ADR 0010 refused FFI for the sandbox itself on exactly that ground. So the
//! read happens on a thread of its own and the browser process waits on a
//! channel, which *does* take a bound.
//!
//! # Why the channel is bounded
//!
//! Because a thread that read ahead as fast as a renderer would write is a
//! renderer that can fill the browser process's memory by talking. The old
//! blocking read had that backpressure for free: nothing was read until
//! somebody asked. [`sync_channel`] with room for one keeps it — the reader
//! stops at one message in hand, and everything after that stays in the pipe
//! where the operating system already bounds it.
//!
//! # A bound without a kill would be worse than no bound
//!
//! The protocol is one answer per request, so an answer that arrives after we
//! stopped waiting for it would be handed back as the answer to the *next*
//! request — a picture of the wrong page, or a tree the agent then acts on.
//! That is why [`Nothing::Silent`] is fatal to the renderer at
//! [`crate::host::Renderers::ask`] rather than something to retry: once we have
//! stopped listening, the two ends can no longer agree about which answer is
//! whose.

use crate::pipe::{self, Arrived};
use crate::wire::Unreadable;
use std::io::{BufReader, Read};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::time::Duration;

/// How long a renderer may say nothing before it is given up on.
///
/// **It is a choice rather than a measurement**, and it is written down as one:
/// no page here takes anything near it — the corpus renders in milliseconds —
/// and `LOOP.md` says a claim about speed is measured on hardware or not made,
/// so this is not one. What it is is the point past which waiting is worse than
/// losing the page, and both directions cost something real:
///
/// - Too short and a renderer that would have answered is killed, which loses a
///   page a person was reading and hides nothing, since the renderer was fine.
/// - Too long and the **browser** process sits in a read while every other tab,
///   and everything a person could click, waits with it. Ten seconds of that is
///   bad; a minute of it is a browser somebody force-quits.
///
/// The honest answer is not a number at all — it is asking the person whether
/// to keep waiting, which is what other browsers do and what this cannot do
/// until there is an interface to ask in (the block on queue items 157 and
/// 158). Until then a rule stands in for the question, and it is set far above
/// any legitimate answer this engine gives today.
pub const LONGEST_SILENCE: Duration = Duration::from_secs(10);

/// Why nothing usable came back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Nothing {
    /// Bytes arrived and were not a message this engine will read.
    Unreadable(Unreadable),
    /// The bound passed and the renderer had said nothing at all.
    ///
    /// The one this file exists for: the process is alive, so nothing has
    /// crashed and nothing will arrive by waiting a little longer either.
    Silent(Duration),
    /// The pipe had already ended, and whoever was told about it was told then.
    ///
    /// A renderer is dropped the first time it fails, so this is a caller
    /// asking a second time rather than an ending nobody has heard about.
    AlreadyEnded,
}

impl core::fmt::Display for Nothing {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Nothing::Unreadable(why) => write!(f, "it said something unreadable: {why}"),
            Nothing::Silent(bound) => {
                write!(f, "it said nothing for {bound:?}, so it was stopped")
            }
            Nothing::AlreadyEnded => write!(f, "it had already stopped answering"),
        }
    }
}

impl std::error::Error for Nothing {}

/// What a renderer has said, read on a thread of its own.
///
/// One of these per renderer, made when the process is spawned and dropped when
/// it is stopped.
#[derive(Debug)]
pub struct Answers {
    said: Receiver<Result<Arrived, Unreadable>>,
}

impl Answers {
    /// Start reading messages off this stream.
    ///
    /// The stream is a renderer's standard output in every real use; it is a
    /// parameter rather than a [`std::process::ChildStdout`] so that the rules
    /// below can be asserted against a reader a test controls the timing of,
    /// which is the half a spawned process cannot make precise.
    pub fn read_from(from: impl Read + Send + 'static) -> Self {
        let (sender, said) = sync_channel(1);
        // Detached, deliberately: nothing here ever joins it. A join is an
        // unbounded wait on a renderer, which is the exact bug this file is
        // for, and it would be taken at the worst moment — while stopping a
        // renderer that had already proved it does not answer. The thread ends
        // on its own when the pipe closes, which [`crate::host::Renderers::
        // stop`] guarantees by killing the process and waiting for it.
        std::thread::spawn(move || read_ahead(BufReader::new(from), &sender));
        Self { said }
    }

    /// The next thing it said, or nothing within this long.
    ///
    /// # Errors
    ///
    /// [`Nothing`], in its three kinds. A [`Nothing::Silent`] must be fatal to
    /// the renderer — see this module's last section for why a caller may not
    /// simply ask again.
    pub fn within(&self, bound: Duration) -> Result<Arrived, Nothing> {
        match self.said.recv_timeout(bound) {
            Ok(Ok(arrived)) => Ok(arrived),
            Ok(Err(unreadable)) => Err(Nothing::Unreadable(unreadable)),
            Err(RecvTimeoutError::Timeout) => Err(Nothing::Silent(bound)),
            Err(RecvTimeoutError::Disconnected) => Err(Nothing::AlreadyEnded),
        }
    }
}

/// Read messages until there are no more, handing each over as it arrives.
///
/// It stops on anything that is not a message, because both of the other
/// answers are final: a stream that ended has nothing further to say, and one
/// that produced bytes we could not read has lost its place in the message
/// boundaries and everything after it would be rubbish too.
fn read_ahead(mut from: BufReader<impl Read>, to: &SyncSender<Result<Arrived, Unreadable>>) {
    loop {
        let said = pipe::read(&mut from);
        let more_to_come = matches!(said, Ok(Arrived::Message(_)));
        // A send that fails is the browser process having dropped this
        // renderer, which is the other way this thread ends.
        if to.send(said).is_err() || !more_to_come {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::mpsc::{Sender, channel};
    use std::time::Instant;

    /// A reader that says nothing until somebody tells it to.
    ///
    /// The whole point of it: a renderer that is silent is a renderer that has
    /// *not* closed its pipe, so a test of silence needs a reader that blocks
    /// rather than one that ends. Dropping the [`Sender`] is how a test says
    /// the stream is over, so nothing is left blocked when a test finishes.
    struct WhenTold {
        told: Receiver<Vec<u8>>,
        saying: VecDeque<u8>,
    }

    impl WhenTold {
        fn new() -> (Sender<Vec<u8>>, Self) {
            let (sender, told) = channel();
            (
                sender,
                Self {
                    told,
                    saying: VecDeque::new(),
                },
            )
        }
    }

    impl Read for WhenTold {
        fn read(&mut self, into: &mut [u8]) -> std::io::Result<usize> {
            while self.saying.is_empty() {
                match self.told.recv() {
                    Ok(more) => self.saying.extend(more),
                    // Nobody will say anything else: the stream is over, which
                    // is a clean end rather than an error.
                    Err(_) => return Ok(0),
                }
            }
            let mut given = 0;
            for room in into.iter_mut() {
                match self.saying.pop_front() {
                    Some(byte) => *room = byte,
                    None => break,
                }
                given += 1;
            }
            Ok(given)
        }
    }

    /// One message, framed the way [`pipe::write`] frames it.
    fn framed(payload: &[u8]) -> Vec<u8> {
        let mut bytes = (payload.len() as u64).to_be_bytes().to_vec();
        bytes.extend_from_slice(payload);
        bytes
    }

    #[test]
    fn a_renderer_that_says_nothing_is_given_up_on_after_the_bound() {
        let (_sender, reader) = WhenTold::new();
        let answers = Answers::read_from(reader);

        let bound = Duration::from_millis(150);
        let began = Instant::now();
        let waited_for = answers.within(bound);
        let waited = began.elapsed();

        assert_eq!(waited_for, Err(Nothing::Silent(bound)));
        assert!(
            waited < bound * 4,
            "it waited {waited:?} for a bound of {bound:?}",
        );
        assert!(
            waited >= bound,
            "it gave up after {waited:?}, before the bound of {bound:?} had passed",
        );
    }

    /// The half that decides what the bound may be: a renderer that is merely
    /// slow is answered from, not killed.
    #[test]
    fn a_renderer_that_is_slow_is_waited_for() {
        let (sender, reader) = WhenTold::new();
        let answers = Answers::read_from(reader);
        let slowly = Duration::from_millis(120);
        std::thread::spawn(move || {
            std::thread::sleep(slowly);
            let _ = sender.send(framed(b"an answer"));
        });

        let began = Instant::now();
        let said = answers.within(Duration::from_secs(30));
        let waited = began.elapsed();

        assert_eq!(said, Ok(Arrived::Message(b"an answer".to_vec())));
        assert!(
            waited >= slowly,
            "it answered in {waited:?}, before the answer was sent",
        );
    }

    /// A pipe that closes cleanly is an ending rather than a silence, and the
    /// difference is what tells a renderer that exited from one that is wedged.
    #[test]
    fn a_pipe_that_ends_is_an_ending_rather_than_a_silence() {
        let (sender, reader) = WhenTold::new();
        let answers = Answers::read_from(reader);
        drop(sender);

        assert_eq!(answers.within(Duration::from_secs(30)), Ok(Arrived::Ended));
        // And the reader is finished, so a caller that asks again is told that
        // rather than waiting the whole bound for a thread that has gone.
        assert_eq!(
            answers.within(Duration::from_secs(30)),
            Err(Nothing::AlreadyEnded),
        );
    }

    /// Messages keep their order and their boundaries across the thread, which
    /// is the thing a hand-off could quietly get wrong.
    #[test]
    fn several_messages_arrive_whole_and_in_order() {
        let (sender, reader) = WhenTold::new();
        let answers = Answers::read_from(reader);
        // Split across two writes on purpose: a message that arrives in pieces
        // is the ordinary case on a pipe.
        let _ = sender.send(framed(b"first"));
        let mut second = framed(b"second");
        let rest = second.split_off(3);
        let _ = sender.send(second);
        let _ = sender.send(rest);
        let _ = sender.send(framed(b"third"));
        drop(sender);

        let bound = Duration::from_secs(30);
        assert_eq!(
            answers.within(bound),
            Ok(Arrived::Message(b"first".to_vec()))
        );
        assert_eq!(
            answers.within(bound),
            Ok(Arrived::Message(b"second".to_vec())),
        );
        assert_eq!(
            answers.within(bound),
            Ok(Arrived::Message(b"third".to_vec()))
        );
        assert_eq!(answers.within(bound), Ok(Arrived::Ended));
    }

    /// Bytes that are not a message are reported as such and end the reading,
    /// because a stream that lost its place in the message boundaries has
    /// nothing further worth reading.
    #[test]
    fn bytes_that_are_not_a_message_are_said_to_be_unreadable_and_end_it() {
        let (sender, reader) = WhenTold::new();
        let answers = Answers::read_from(reader);
        // A length no message may have, which is refused before the read
        // rather than believed.
        let _ = sender.send(u64::MAX.to_be_bytes().to_vec());

        let bound = Duration::from_secs(30);
        assert!(
            matches!(answers.within(bound), Err(Nothing::Unreadable(_))),
            "a length nobody can honour was not refused",
        );
        assert_eq!(answers.within(bound), Err(Nothing::AlreadyEnded));
    }

    /// The reading stops when the browser process stops listening, so a
    /// renderer that talks for ever does not leave a thread reading for ever.
    #[test]
    fn dropping_the_answers_ends_the_reading() {
        let (sender, reader) = WhenTold::new();
        let answers = Answers::read_from(reader);
        // Two messages: the first is taken by the reader, the second fills the
        // channel's one place, and a third send is what fails once nobody is
        // listening.
        let _ = sender.send(framed(b"one"));
        let _ = sender.send(framed(b"two"));
        assert_eq!(
            answers.within(Duration::from_secs(30)),
            Ok(Arrived::Message(b"one".to_vec())),
        );
        drop(answers);

        // Whatever it is told now goes nowhere, and the thread that would have
        // read it has stopped. Sending is how a test can tell: the reader holds
        // the other end of this channel, so a send fails once it has gone.
        let mut sent_after = 0;
        for _ in 0..100 {
            if sender.send(framed(b"three")).is_err() {
                break;
            }
            sent_after += 1;
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            sent_after < 100,
            "the reading thread was still there after the answers were dropped",
        );
    }

    #[test]
    fn what_a_person_is_told_names_what_happened() {
        assert_eq!(
            Nothing::Silent(Duration::from_secs(10)).to_string(),
            "it said nothing for 10s, so it was stopped",
        );
        assert_eq!(
            Nothing::AlreadyEnded.to_string(),
            "it had already stopped answering",
        );
        assert_eq!(
            Nothing::Unreadable(Unreadable {
                why: "a message of 9 bytes".to_owned(),
            })
            .to_string(),
            "it said something unreadable: a message of 9 bytes",
        );
    }
}
