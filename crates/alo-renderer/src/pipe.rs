/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Messages over a pipe, one after another.
//!
//! [`crate::wire`] says what a message *is*. This says where one ends, which is
//! a separate question and the one a stream gets wrong: a pipe has no message
//! boundaries of its own, so every message says how long it is — and every
//! length is a number the other end chose.
//!
//! # Why the length is checked before the read rather than after
//!
//! Because a read is where the memory goes. Eight bytes saying "a gigabyte
//! follows" cost eight bytes to send and a gigabyte to believe.

use crate::wire::{LARGEST_MESSAGE, Unreadable};
use std::io::{Read, Write};

/// Write one message, with its length in front.
///
/// # Errors
///
/// The underlying write error, unchanged: a pipe that will not take bytes is a
/// process that has gone, and the caller is the one that knows what that means.
pub fn write(to: &mut impl Write, message: &[u8]) -> std::io::Result<()> {
    if message.len() > LARGEST_MESSAGE {
        return Err(std::io::Error::other(
            "a message larger than this engine will send",
        ));
    }
    to.write_all(&(message.len() as u64).to_be_bytes())?;
    to.write_all(message)?;
    to.flush()
}

/// What came off a pipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arrived {
    /// A whole message.
    Message(Vec<u8>),
    /// The other end closed, cleanly, between messages.
    ///
    /// Distinct from an error on purpose: a renderer that finished and exited
    /// is not a renderer that crashed, and a browser process that could not
    /// tell them apart would report a bug every time a tab closed.
    Ended,
}

/// Read one message.
///
/// # Errors
///
/// [`Unreadable`] for a length this engine will not honour, and for a stream
/// that stops **inside** a message — which is different from one that stops
/// between messages, and the difference is [`Arrived::Ended`].
pub fn read(from: &mut impl Read) -> Result<Arrived, Unreadable> {
    let mut header = [0u8; 8];
    match fill(from, &mut header) {
        Filled::Whole => {}
        Filled::NothingAtAll => return Ok(Arrived::Ended),
        Filled::StoppedPartWay => {
            return Err(Unreadable {
                why: "the other end stopped in the middle of a message header".to_owned(),
            });
        }
        Filled::Broke(why) => return Err(Unreadable { why }),
    }
    let how_long = u64::from_be_bytes(header);
    let how_long = usize::try_from(how_long).unwrap_or(usize::MAX);
    if how_long > LARGEST_MESSAGE {
        // Before the read, because the read is where the memory goes.
        return Err(Unreadable {
            why: format!("a message of {how_long} bytes, which is more than this engine reads"),
        });
    }
    let mut message = vec![0u8; how_long];
    match fill(from, &mut message) {
        Filled::Whole => Ok(Arrived::Message(message)),
        Filled::NothingAtAll | Filled::StoppedPartWay => Err(Unreadable {
            why: format!("the other end stopped {how_long} bytes into a message"),
        }),
        Filled::Broke(why) => Err(Unreadable { why }),
    }
}

/// How a fill went, with "nothing at all" told apart from "stopped part way".
enum Filled {
    Whole,
    NothingAtAll,
    StoppedPartWay,
    Broke(String),
}

fn fill(from: &mut impl Read, into: &mut [u8]) -> Filled {
    let mut got = 0;
    while got < into.len() {
        let room = into.get_mut(got..).unwrap_or_default();
        match from.read(room) {
            Ok(0) => {
                return if got == 0 {
                    Filled::NothingAtAll
                } else {
                    Filled::StoppedPartWay
                };
            }
            Ok(more) => got += more,
            Err(why) if why.kind() == std::io::ErrorKind::Interrupted => {}
            Err(why) => return Filled::Broke(why.to_string()),
        }
    }
    Filled::Whole
}
