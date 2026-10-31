/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where one message from Chrome ends and the next begins (ADR 0058 § 3).
//!
//! Over `--remote-debugging-pipe` every DevTools message is one JSON text
//! followed by a NUL byte, in both directions. A pipe has no boundaries of its
//! own, so this finds them — and the far side chooses how long a message is,
//! so the longest one is ours: [`MOST_MESSAGE`]. What a message *means* is
//! [`crate::chrome_protocol`]'s.
//!
//! # A message too long is skipped, not the end of the pipe
//!
//! A page decides how long some of Chrome's messages are: what its script
//! writes to its console arrives whole, and a string of twenty million
//! characters is a message of twenty megabytes (measured against Chrome 155,
//! queue item 639). A JSON text holds no NUL of its own — a NUL in a string
//! is written escaped — so the next message begins after the next NUL
//! whatever came before it. So a message past the bound is **skipped**:
//! its first [`HEAD`] bytes are kept, to say what it was, and every byte
//! after them is read and dropped as it arrives, never held. The pipe goes
//! on, and a page cannot end the browser's hold on its tab by saying
//! something long.

use core::fmt;
use std::io::{ErrorKind, Read, Write};

/// The longest message read from Chrome, in bytes, its NUL not counted.
///
/// Every reply this browser asks for today is a few hundred bytes. Sixteen
/// mebibytes is room for an accessibility tree of a very large page (item
/// 620), and is still a bound a hostile far side cannot talk past.
pub const MOST_MESSAGE: usize = 16 * 1024 * 1024;

/// How much is read from the pipe at a time.
const CHUNK: usize = 64 * 1024;

/// How many bytes of a message too long to read are kept, to say what it
/// was: Chrome writes an answer's `id`, or an event's `method`, first.
pub const HEAD: usize = 128;

/// One message taken off the pipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// A whole message, without its NUL.
    Whole(Vec<u8>),
    /// A message longer than the bound, skipped to its NUL: its first
    /// [`HEAD`] bytes, or fewer if the bound is smaller.
    Skipped {
        /// What it began with.
        head: Vec<u8>,
    },
}

/// Why no message could be taken off the pipe. Each one ends the stream:
/// past any of them, where the next message starts is not known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unframed {
    /// The pipe closed part of the way through a message.
    Truncated,
    /// The pipe could not be read.
    Unreadable(ErrorKind),
}

impl fmt::Display for Unframed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("Chrome's pipe closed in the middle of a message"),
            Self::Unreadable(kind) => write!(f, "Chrome's pipe could not be read: {kind}"),
        }
    }
}

/// Messages read off one pipe, one after another.
pub struct Frames<R> {
    from: R,
    held: Vec<u8>,
    chunk: Vec<u8>,
    most: usize,
}

impl<R: Read> Frames<R> {
    /// Messages read from `from`, each at most [`MOST_MESSAGE`] bytes.
    pub fn from(from: R) -> Self {
        Self::bounded(from, MOST_MESSAGE)
    }

    /// Messages read from `from`, each at most `most` bytes — for a test that
    /// cannot send sixteen mebibytes to watch one refused.
    pub fn bounded(from: R, most: usize) -> Self {
        Self {
            from,
            held: Vec::new(),
            chunk: vec![0; CHUNK],
            most,
        }
    }

    /// The next message, or `None` once the pipe has closed between two of
    /// them. A message longer than the bound is [`Frame::Skipped`], and the
    /// one after it is read as usual.
    ///
    /// # Errors
    ///
    /// [`Unframed`]: a pipe closed part of the way through a message, or a
    /// pipe that could not be read.
    pub fn next_message(&mut self) -> Result<Option<Frame>, Unframed> {
        let mut looked = 0;
        // Once the message is known to be too long: what it began with. From
        // then on nothing of it is held.
        let mut skipping: Option<Vec<u8>> = None;
        loop {
            if let Some(at) = self.held.get(looked..).and_then(nul_in) {
                let end = looked + at;
                // What follows the message, starting at its NUL, and then
                // what follows the NUL.
                let mut after = self.held.split_off(end);
                let rest = after.split_off(after.len().min(1));
                let message = core::mem::replace(&mut self.held, rest);
                return Ok(Some(match skipping {
                    Some(head) => Frame::Skipped { head },
                    None if end > self.most => Frame::Skipped {
                        head: self.head_of(&message),
                    },
                    None => Frame::Whole(message),
                }));
            }
            if skipping.is_none() && self.held.len() > self.most {
                skipping = Some(self.head_of(&self.held));
            }
            if skipping.is_some() {
                self.held.clear();
                self.held.shrink_to(CHUNK);
            }
            looked = self.held.len();
            let read = match self.from.read(&mut self.chunk) {
                Ok(read) => read,
                Err(why) if why.kind() == ErrorKind::Interrupted => continue,
                Err(why) => return Err(Unframed::Unreadable(why.kind())),
            };
            let Some(arrived) = self.chunk.get(..read) else {
                return Err(Unframed::Unreadable(ErrorKind::InvalidData));
            };
            if arrived.is_empty() {
                return if self.held.is_empty() && skipping.is_none() {
                    Ok(None)
                } else {
                    Err(Unframed::Truncated)
                };
            }
            self.held.extend_from_slice(arrived);
        }
    }

    /// The first [`HEAD`] bytes of `message`, and never more than the bound.
    fn head_of(&self, message: &[u8]) -> Vec<u8> {
        let kept = HEAD.min(self.most).min(message.len());
        message.get(..kept).unwrap_or_default().to_vec()
    }
}

/// Where the first NUL in `bytes` is — found by the standard library's own
/// search for one, which is built optimised even in a debug build, where a
/// byte-by-byte search of a streamed frame's half megabyte was the reading
/// thread's largest cost (queue item 717).
fn nul_in(bytes: &[u8]) -> Option<usize> {
    core::ffi::CStr::from_bytes_until_nul(bytes)
        .ok()
        .map(|before| before.to_bytes().len())
}

/// Write one message and the NUL that ends it.
///
/// # Errors
///
/// A message holding a NUL of its own, which would end early on the far side
/// and leave the rest to be read as the next one; or the write's own error, a
/// pipe that will not take bytes being a Chrome that has gone.
pub fn write(to: &mut impl Write, message: &[u8]) -> std::io::Result<()> {
    if nul_in(message).is_some() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "a message to Chrome may not hold a NUL",
        ));
    }
    to.write_all(message)?;
    to.write_all(&[0])?;
    to.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A reader that hands over one byte at a time, as a slow pipe may.
    struct Trickle(Cursor<Vec<u8>>);

    impl Read for Trickle {
        fn read(&mut self, into: &mut [u8]) -> std::io::Result<usize> {
            match into.first_mut() {
                Some(first) => self.0.read(core::slice::from_mut(first)),
                None => Ok(0),
            }
        }
    }

    fn all(bytes: &[u8], most: usize) -> Vec<Result<Option<Frame>, Unframed>> {
        let mut frames = Frames::bounded(Cursor::new(bytes.to_vec()), most);
        let mut seen = Vec::new();
        loop {
            let next = frames.next_message();
            let done = !matches!(next, Ok(Some(_)));
            seen.push(next);
            if done {
                return seen;
            }
        }
    }

    fn whole(bytes: &[u8]) -> Frame {
        Frame::Whole(bytes.to_vec())
    }

    fn skipped(head: &[u8]) -> Frame {
        Frame::Skipped {
            head: head.to_vec(),
        }
    }

    #[test]
    fn messages_end_at_each_nul() {
        assert_eq!(
            all(b"{\"id\":1}\0{}\0", 64),
            [
                Ok(Some(whole(b"{\"id\":1}"))),
                Ok(Some(whole(b"{}"))),
                Ok(None)
            ]
        );
        assert_eq!(
            all(b"", 64),
            [Ok(None)],
            "a pipe closed at once is no message"
        );
        assert_eq!(
            all(b"\0", 64),
            [Ok(Some(whole(b""))), Ok(None)],
            "an empty message is a message, for the protocol to refuse"
        );
    }

    #[test]
    fn a_message_split_across_reads_is_one_message() {
        let mut frames = Frames::bounded(Trickle(Cursor::new(b"{\"a\":1}\0{}\0".to_vec())), 64);
        assert_eq!(frames.next_message(), Ok(Some(whole(b"{\"a\":1}"))));
        assert_eq!(frames.next_message(), Ok(Some(whole(b"{}"))));
        assert_eq!(frames.next_message(), Ok(None));
    }

    #[test]
    fn a_message_cut_off_by_the_pipe_closing_is_truncated() {
        assert_eq!(
            all(b"{}\0{\"id\":", 64),
            [Ok(Some(whole(b"{}"))), Err(Unframed::Truncated)]
        );
    }

    #[test]
    fn a_message_longer_than_the_bound_is_skipped_and_the_next_is_read() {
        let mut long = vec![b'a'; 9];
        long.push(0);
        long.extend_from_slice(b"{}\0");
        assert_eq!(
            all(&long, 8),
            [
                Ok(Some(skipped(&[b'a'; 8]))),
                Ok(Some(whole(b"{}"))),
                Ok(None)
            ],
            "skipped, its head kept, and the pipe goes on"
        );
        let mut exact = vec![b'a'; 8];
        exact.push(0);
        assert_eq!(all(&exact, 8), [Ok(Some(whole(&[b'a'; 8]))), Ok(None)]);
        // Cut off by the pipe closing while it is skipped: truncated.
        assert_eq!(all(&[b'a'; 9], 8), [Err(Unframed::Truncated)]);
    }

    #[test]
    fn a_skipped_message_keeps_only_its_head_however_it_arrives() {
        // Twenty megabytes, as Chrome sends a page's long console line,
        // between two messages, a byte at a time for the first part.
        let mut bytes = b"{\"id\":1}\0{\"method\":\"Runtime.consoleAPICalled\",".to_vec();
        bytes.resize(20 * 1024 * 1024, b'x');
        bytes.extend_from_slice(b"\0{\"id\":2}\0");
        let mut frames = Frames::bounded(Cursor::new(bytes), 16 * 1024 * 1024);
        assert_eq!(frames.next_message(), Ok(Some(whole(b"{\"id\":1}"))));
        let Ok(Some(Frame::Skipped { head })) = frames.next_message() else {
            panic!("not skipped");
        };
        assert_eq!(head.len(), HEAD);
        assert!(head.starts_with(b"{\"method\":\"Runtime.consoleAPICalled\","));
        assert!(
            frames.held.capacity() <= 2 * CHUNK,
            "what was skipped is not held after it"
        );
        assert_eq!(frames.next_message(), Ok(Some(whole(b"{\"id\":2}"))));
        assert_eq!(frames.next_message(), Ok(None));

        let mut trickled = vec![b'b'; 40];
        trickled.extend_from_slice(b"\0{}\0");
        let mut frames = Frames::bounded(Trickle(Cursor::new(trickled)), 16);
        assert_eq!(frames.next_message(), Ok(Some(skipped(&[b'b'; 16]))));
        assert_eq!(frames.next_message(), Ok(Some(whole(b"{}"))));
    }

    #[test]
    fn hostile_bytes_are_framed_without_a_panic() {
        for bytes in [
            &b"\xff\xfe\0"[..],
            b"\0\0\0",
            b"\0\xff",
            &[0u8; 300],
            b"{\"id\":1}\0\x80",
        ] {
            let _ = all(bytes, 16);
        }
    }

    #[test]
    fn a_message_is_written_with_its_nul_and_never_holding_one() {
        let mut sent = Vec::new();
        write(&mut sent, b"{\"id\":1}").unwrap();
        assert_eq!(sent, b"{\"id\":1}\0");
        let refused = write(&mut sent, b"{\0}").unwrap_err();
        assert_eq!(refused.kind(), ErrorKind::InvalidInput);
        assert_eq!(sent, b"{\"id\":1}\0", "nothing of it was written");
    }
}
