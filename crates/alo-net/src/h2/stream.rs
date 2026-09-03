/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One stream, and which frames are legal while it is in which state.
//!
//! # Why a state machine rather than a flag
//!
//! A stream is not open or closed. It is open in each direction separately, and
//! the two ends stop at different times — a request that has been fully sent
//! while its response is still arriving is *half-closed locally*, and it is the
//! normal state of every request a browser makes.
//!
//! Collapsing that into a boolean is how a `DATA` frame arriving after a
//! response finished becomes a body silently appended to a page rather than a
//! `STREAM_CLOSED`. The states are named here so that the illegal transitions
//! are things a reader can see are missing.

use super::ErrorCode;
use super::flow::Window;
use super::frame::Broken;

/// Where a stream is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Nothing has happened on it. Every stream number is in this state until
    /// something uses it — which is why a frame arriving on a number *lower*
    /// than one already used is a protocol error rather than a new stream.
    Idle,
    /// A server promised to use this number. Nothing may be sent on it by us.
    ///
    /// This engine sends `SETTINGS_ENABLE_PUSH: 0`, so a server that reaches
    /// this state has ignored what it was told — but the state exists because
    /// refusing it correctly means knowing what it is.
    Reserved,
    /// Both ends may send.
    Open,
    /// We have finished sending; the other end has not.
    ///
    /// The normal state of every request a browser makes, from the moment the
    /// request is out until the response ends.
    HalfClosedLocal,
    /// The other end has finished; we have not.
    HalfClosedRemote,
    /// Over.
    Closed,
}

/// What is happening on one stream.
#[derive(Debug, Clone)]
pub struct Stream {
    /// Its number.
    pub id: u32,
    /// Where it is.
    pub state: State,
    /// How much more may be sent on it.
    pub sending: Window,
    /// How much more may arrive on it.
    pub receiving: Window,
    /// Why it ended, if it ended badly.
    pub ended_by: Option<ErrorCode>,
    /// Whether a header block has already arrived on it.
    ///
    /// The difference between a response's headers and its **trailers**, which
    /// look identical on the wire and are told apart only by which came first.
    already_had_headers: bool,
}

impl Stream {
    /// A stream that nothing has happened on.
    pub fn new(id: u32, sending: i64, receiving: i64) -> Self {
        Self {
            id,
            state: State::Idle,
            sending: Window::of(sending),
            receiving: Window::of(receiving),
            ended_by: None,
            already_had_headers: false,
        }
    }

    /// Whether anything more may arrive on it.
    pub fn may_still_receive(&self) -> bool {
        matches!(self.state, State::Open | State::HalfClosedLocal)
    }

    /// Whether anything more may be sent on it.
    pub fn may_still_send(&self) -> bool {
        matches!(self.state, State::Open | State::HalfClosedRemote)
    }

    /// A `HEADERS` arrived.
    ///
    /// # Errors
    ///
    /// [`Broken`] when it arrived somewhere headers may not.
    pub fn headers_arrived(&mut self, end_stream: bool) -> Result<(), Broken> {
        match self.state {
            State::Idle => self.state = State::Open,
            // Trailers, which are legal and end the stream by definition — a
            // second header block that did not end the stream would be a third
            // one waiting to arrive, and there is no such thing.
            State::Open | State::HalfClosedLocal => {
                if !end_stream && self.already_had_headers {
                    return Err(self.broken("a second header block that does not end the stream"));
                }
            }
            State::Reserved => self.state = State::HalfClosedLocal,
            State::HalfClosedRemote | State::Closed => {
                return Err(self.closed("HEADERS"));
            }
        }
        self.already_had_headers = true;
        if end_stream {
            self.ended_receiving();
        }
        Ok(())
    }

    /// A `DATA` frame arrived, of this many bytes **including its padding**.
    ///
    /// Padding counts: it is bytes on the wire that the sender chose to send,
    /// and a window that did not count it would be a window a peer could evade
    /// by padding everything.
    ///
    /// # Errors
    ///
    /// [`Broken`] when the stream is not one that may receive, or when more
    /// arrived than its window allowed.
    pub fn data_arrived(&mut self, on_the_wire: usize, end_stream: bool) -> Result<(), Broken> {
        if !self.may_still_receive() {
            return Err(self.closed("DATA"));
        }
        // A stream's overrun ends the stream; the connection's ends everything,
        // and that is the caller's window rather than this one.
        self.receiving.arrived(on_the_wire, false)?;
        if end_stream {
            self.ended_receiving();
        }
        Ok(())
    }

    /// This end has finished sending.
    pub fn finished_sending(&mut self) {
        self.state = match self.state {
            State::Open => State::HalfClosedLocal,
            State::HalfClosedRemote => State::Closed,
            other => other,
        };
    }

    /// The stream was reset, by either end.
    pub fn reset(&mut self, why: ErrorCode) {
        self.state = State::Closed;
        self.ended_by = Some(why);
    }

    fn ended_receiving(&mut self) {
        self.state = match self.state {
            State::Open => State::HalfClosedRemote,
            State::HalfClosedLocal => State::Closed,
            other => other,
        };
    }

    fn closed(&self, what: &str) -> Broken {
        Broken {
            why: format!(
                "a {what} frame on stream {}, which is {:?}",
                self.id, self.state
            ),
            error: ErrorCode::StreamClosed,
            fatal: false,
        }
    }

    fn broken(&self, why: &str) -> Broken {
        Broken {
            why: format!("{why}, on stream {}", self.id),
            error: ErrorCode::ProtocolError,
            fatal: false,
        }
    }
}
