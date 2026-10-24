/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where one message from Chrome ends and the next begins (ADR 0058 § 3).
//!
//! Over `--remote-debugging-pipe` every DevTools message is one JSON text
//! followed by a NUL byte, in both directions. A pipe has no boundaries of its
//! own, so this finds them — and the far side chooses how long a message is,
//! so the longest one is ours: [`MOST_MESSAGE`], refused rather than read on.
//! What a message *means* is [`crate::chrome_protocol`]'s.

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

/// Why no message could be taken off the pipe. Each one ends the stream:
/// past any of them, where the next message starts is not known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unframed {
    /// More than `most` bytes came without a NUL to end them.
    TooLong {
        /// [`MOST_MESSAGE`], or the bound a test asked for.
        most: usize,
    },
    /// The pipe closed part of the way through a message.
    Truncated,
    /// The pipe could not be read.
    Unreadable(ErrorKind),
}

impl fmt::Display for Unframed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong { most } => {
                write!(f, "a message from Chrome ran past {most} bytes")
            }
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

    /// The next whole message, without its NUL, or `None` once the pipe has
    /// closed between two of them.
    ///
    /// # Errors
    ///
    /// [`Unframed`]: a message too long, a pipe closed part of the way through
    /// one, or a pipe that could not be read.
    pub fn next_message(&mut self) -> Result<Option<Vec<u8>>, Unframed> {
        let mut looked = 0;
        loop {
            if let Some(at) = self.held.get(looked..).and_then(nul_in) {
                let end = looked + at;
                if end > self.most {
                    return Err(Unframed::TooLong { most: self.most });
                }
                // What follows the message, starting at its NUL, and then
                // what follows the NUL.
                let mut after = self.held.split_off(end);
                let rest = after.split_off(after.len().min(1));
                let message = core::mem::replace(&mut self.held, rest);
                return Ok(Some(message));
            }
            looked = self.held.len();
            if looked > self.most {
                return Err(Unframed::TooLong { most: self.most });
            }
            let read = match self.from.read(&mut self.chunk) {
                Ok(read) => read,
                Err(why) if why.kind() == ErrorKind::Interrupted => continue,
                Err(why) => return Err(Unframed::Unreadable(why.kind())),
            };
            let Some(arrived) = self.chunk.get(..read) else {
                return Err(Unframed::Unreadable(ErrorKind::InvalidData));
            };
            if arrived.is_empty() {
                return if self.held.is_empty() {
                    Ok(None)
                } else {
                    Err(Unframed::Truncated)
                };
            }
            self.held.extend_from_slice(arrived);
        }
    }
}

/// Where the first NUL in `bytes` is.
fn nul_in(bytes: &[u8]) -> Option<usize> {
    bytes.iter().position(|&byte| byte == 0)
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

    fn all(bytes: &[u8], most: usize) -> Vec<Result<Option<Vec<u8>>, Unframed>> {
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

    #[test]
    fn messages_end_at_each_nul() {
        assert_eq!(
            all(b"{\"id\":1}\0{}\0", 64),
            [
                Ok(Some(b"{\"id\":1}".to_vec())),
                Ok(Some(b"{}".to_vec())),
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
            [Ok(Some(Vec::new())), Ok(None)],
            "an empty message is a message, for the protocol to refuse"
        );
    }

    #[test]
    fn a_message_split_across_reads_is_one_message() {
        let mut frames = Frames::bounded(Trickle(Cursor::new(b"{\"a\":1}\0{}\0".to_vec())), 64);
        assert_eq!(frames.next_message(), Ok(Some(b"{\"a\":1}".to_vec())));
        assert_eq!(frames.next_message(), Ok(Some(b"{}".to_vec())));
        assert_eq!(frames.next_message(), Ok(None));
    }

    #[test]
    fn a_message_cut_off_by_the_pipe_closing_is_truncated() {
        assert_eq!(
            all(b"{}\0{\"id\":", 64),
            [Ok(Some(b"{}".to_vec())), Err(Unframed::Truncated)]
        );
    }

    #[test]
    fn a_message_longer_than_the_bound_is_refused_and_not_read_on() {
        assert_eq!(all(&[b'a'; 9], 8), [Err(Unframed::TooLong { most: 8 })]);
        let mut long = vec![b'a'; 9];
        long.push(0);
        assert_eq!(
            all(&long, 8),
            [Err(Unframed::TooLong { most: 8 })],
            "ended, but too long"
        );
        let mut exact = vec![b'a'; 8];
        exact.push(0);
        assert_eq!(all(&exact, 8), [Ok(Some(vec![b'a'; 8])), Ok(None)]);
        // A far side that never sends a NUL is stopped at the bound, not when
        // it chooses to stop.
        let mut endless = Frames::bounded(std::io::repeat(b'x'), 1000);
        assert_eq!(
            endless.next_message(),
            Err(Unframed::TooLong { most: 1000 })
        );
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
