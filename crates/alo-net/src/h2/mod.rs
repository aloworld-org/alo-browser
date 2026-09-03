//! HTTP/2.
//!
//! # What is here, and what is queued
//!
//! [`frame`] is the wire format: nine bytes of header and a payload, and every
//! rule about what makes one unreadable. It is the whole of this item; HPACK,
//! streams and flow control, and negotiating the protocol at all are queue items
//! 160, 161 and 162.
//!
//! Framing first because everything else is carried inside it, and because it is
//! where a peer that misbehaves gets to choose how much memory we allocate.
//! Every length in this module is checked against a bound before anything is
//! reserved.
//!
//! # Why this is worth building rather than renting
//!
//! ADR 0001 rents the physics — text shaping, codecs, TLS. A protocol is not
//! physics. It is a state machine whose mistakes are our security bugs, and
//! `alo-net`'s whole shape (a request, a response, a framing that is refused
//! when it says two things) is ours. A rented HTTP/2 would arrive with its own
//! opinions about connections, bodies and errors, and the seam between those
//! opinions and ours is where the bugs would live.

pub mod flow;
pub mod frame;
pub mod hpack;
pub mod huffman;
pub mod session;
pub mod stream;

/// Why a connection or a stream was ended.
///
/// The numbers are on the wire, and an unknown one is carried rather than
/// flattened: a peer that sends an error code we have not heard of is telling us
/// something, and "an error" is less than it said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// Finished, with nothing wrong.
    NoError,
    /// The peer broke the protocol.
    ProtocolError,
    /// Something went wrong on the sender's side.
    InternalError,
    /// More data than the window allowed.
    FlowControlError,
    /// A `SETTINGS` that was never acknowledged.
    SettingsTimeout,
    /// A frame on a stream that was already finished.
    StreamClosed,
    /// A frame whose length is not one that frame may have.
    FrameSizeError,
    /// A stream refused before anything happened on it.
    RefusedStream,
    /// No longer wanted.
    Cancel,
    /// The header block could not be decoded.
    ///
    /// Fatal to the **connection** rather than the stream, always: HPACK carries
    /// state from one block to the next, so a block that could not be decoded
    /// leaves the table in a condition nobody can reason about.
    CompressionError,
    /// A `CONNECT` that failed.
    ConnectError,
    /// Slow down.
    EnhanceYourCalm,
    /// The TLS underneath is not good enough for HTTP/2.
    InadequateSecurity,
    /// This has to be done over HTTP/1.1.
    Http11Required,
    /// Something this engine has not heard of, kept as sent.
    Unknown(u32),
}

impl ErrorCode {
    /// The number on the wire.
    pub fn number(self) -> u32 {
        match self {
            ErrorCode::NoError => 0,
            ErrorCode::ProtocolError => 1,
            ErrorCode::InternalError => 2,
            ErrorCode::FlowControlError => 3,
            ErrorCode::SettingsTimeout => 4,
            ErrorCode::StreamClosed => 5,
            ErrorCode::FrameSizeError => 6,
            ErrorCode::RefusedStream => 7,
            ErrorCode::Cancel => 8,
            ErrorCode::CompressionError => 9,
            ErrorCode::ConnectError => 10,
            ErrorCode::EnhanceYourCalm => 11,
            ErrorCode::InadequateSecurity => 12,
            ErrorCode::Http11Required => 13,
            ErrorCode::Unknown(number) => number,
        }
    }

    /// What a number on the wire means.
    pub fn of(number: u32) -> Self {
        match number {
            0 => ErrorCode::NoError,
            1 => ErrorCode::ProtocolError,
            2 => ErrorCode::InternalError,
            3 => ErrorCode::FlowControlError,
            4 => ErrorCode::SettingsTimeout,
            5 => ErrorCode::StreamClosed,
            6 => ErrorCode::FrameSizeError,
            7 => ErrorCode::RefusedStream,
            8 => ErrorCode::Cancel,
            9 => ErrorCode::CompressionError,
            10 => ErrorCode::ConnectError,
            11 => ErrorCode::EnhanceYourCalm,
            12 => ErrorCode::InadequateSecurity,
            13 => ErrorCode::Http11Required,
            other => ErrorCode::Unknown(other),
        }
    }
}

impl core::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ErrorCode::NoError => f.write_str("no error"),
            ErrorCode::ProtocolError => f.write_str("the protocol was broken"),
            ErrorCode::InternalError => f.write_str("something went wrong at the other end"),
            ErrorCode::FlowControlError => f.write_str("more data than the window allowed"),
            ErrorCode::SettingsTimeout => f.write_str("settings were never acknowledged"),
            ErrorCode::StreamClosed => f.write_str("a frame arrived on a finished stream"),
            ErrorCode::FrameSizeError => f.write_str("a frame of a length it may not have"),
            ErrorCode::RefusedStream => f.write_str("the stream was refused"),
            ErrorCode::Cancel => f.write_str("cancelled"),
            ErrorCode::CompressionError => f.write_str("the header block could not be decoded"),
            ErrorCode::ConnectError => f.write_str("the CONNECT failed"),
            ErrorCode::EnhanceYourCalm => f.write_str("too many requests"),
            ErrorCode::InadequateSecurity => {
                f.write_str("the connection is not secure enough for HTTP/2")
            }
            ErrorCode::Http11Required => f.write_str("this has to be done over HTTP/1.1"),
            ErrorCode::Unknown(number) => {
                write!(f, "error {number}, which this engine cannot name")
            }
        }
    }
}
