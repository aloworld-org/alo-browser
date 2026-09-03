/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Nine bytes of header, and a payload whose length the other end chose.
//!
//! # The rule this whole file is built around
//!
//! **A length is checked before anything is reserved.** Every frame announces
//! how long it is, and every announcement comes from somebody who may be
//! hostile. HTTP/1.1 had two such numbers — `Content-Length` and a chunk size —
//! and this has one per frame, several thousand times a page.
//!
//! # The refusals that are not obvious
//!
//! Most of the rules below are "this frame has to be exactly this long". Three
//! are worth knowing before reading:
//!
//! - **Padding is subtracted before it is trusted.** A `DATA` frame's first byte
//!   may say how much of the rest is padding, and a padding length that is not
//!   smaller than what remains is a length that would run the payload backwards.
//!   It is a connection error, and it is the classic HTTP/2 parser bug.
//! - **An unknown frame type is ignored, not refused.** The protocol is
//!   extensible on purpose, and a peer using an extension we do not know is not
//!   misbehaving. Its length is still checked, and its bytes are still consumed
//!   exactly — an "ignore" that lost the stream's place would be worse than a
//!   refusal.
//! - **The top bit of the stream identifier is reserved and ignored**, not
//!   rejected. A sender that sets it is not to be argued with; a reader that
//!   fails to mask it sees stream numbers around two billion.

use super::ErrorCode;
use core::fmt;
use std::io::Read;

/// The nine bytes every frame starts with.
pub const HEADER_BYTES: usize = 9;

/// The largest payload a frame may have unless the peer said otherwise.
///
/// The protocol's own default. A peer may raise it with `SETTINGS`, up to
/// [`LARGEST_ALLOWED`]; nothing may exceed that, whatever anybody says.
pub const LARGEST_BY_DEFAULT: u32 = 16_384;

/// The largest payload any peer may ever ask for — the protocol's ceiling.
pub const LARGEST_ALLOWED: u32 = (1 << 24) - 1;

/// What a client sends before anything else, so a server that is not speaking
/// HTTP/2 fails immediately rather than misreading it as a request.
///
/// It is `PRI * HTTP/2.0` on purpose: an HTTP/1.1 server reading it sees a
/// method it does not know and gives up, instead of half-parsing a frame.
pub const PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// A setting, by the number it has on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setting(pub u16);

impl Setting {
    /// How large the peer's header table may be.
    pub const HEADER_TABLE_SIZE: Setting = Setting(0x1);
    /// Whether the peer may push. This engine always sends zero.
    pub const ENABLE_PUSH: Setting = Setting(0x2);
    /// How many streams may be open at once.
    pub const MAX_CONCURRENT_STREAMS: Setting = Setting(0x3);
    /// How much data may be in flight on one stream.
    pub const INITIAL_WINDOW_SIZE: Setting = Setting(0x4);
    /// The largest frame the sender will accept.
    pub const MAX_FRAME_SIZE: Setting = Setting(0x5);
    /// The largest header block the sender will accept.
    pub const MAX_HEADER_LIST_SIZE: Setting = Setting(0x6);
}

/// How a stream said it wanted to be scheduled.
///
/// Read and carried, and acted on by nothing: priority as specified was
/// complicated enough that browsers and servers disagreed about it, and it has
/// been deprecated in favour of a scheme that is a header rather than a frame.
/// Parsing it is still necessary — the bytes are in the way of the ones that
/// matter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Priority {
    /// The stream this one depends on.
    pub depends_on: u32,
    /// Whether that dependency is exclusive.
    pub exclusive: bool,
    /// One to two hundred and fifty-six, as one less than that.
    pub weight: u8,
}

/// One frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// Part of a body.
    Data {
        /// Which stream.
        stream: u32,
        /// The bytes, with any padding already removed.
        data: Vec<u8>,
        /// Whether the stream ends here.
        end_stream: bool,
    },
    /// A header block, or the start of one.
    Headers {
        /// Which stream.
        stream: u32,
        /// The HPACK block, with any padding already removed. Undecoded: that
        /// is queue item 160, and it needs state this file does not have.
        block: Vec<u8>,
        /// Whether the stream ends here.
        end_stream: bool,
        /// Whether the block is complete, or `CONTINUATION` frames follow.
        end_headers: bool,
        /// What it said about scheduling, if anything.
        priority: Option<Priority>,
    },
    /// Scheduling, on its own.
    Priority {
        /// Which stream.
        stream: u32,
        /// What it asked for.
        priority: Priority,
    },
    /// This stream is over, with a reason.
    ResetStream {
        /// Which stream.
        stream: u32,
        /// Why.
        error: ErrorCode,
    },
    /// What the sender will accept.
    Settings {
        /// Whether this acknowledges the other end's settings rather than
        /// stating any of its own.
        ack: bool,
        /// The settings, in the order sent — order matters, because the same
        /// setting may appear twice and the last one is what it means.
        values: Vec<(Setting, u32)>,
    },
    /// A server saying it intends to send something nobody asked for.
    PushPromise {
        /// Which stream the promise is on.
        stream: u32,
        /// The stream it promises to use.
        promised: u32,
        /// The HPACK block of the request it is answering in advance.
        block: Vec<u8>,
        /// Whether the block is complete.
        end_headers: bool,
    },
    /// Are you there.
    Ping {
        /// Whether this is the answer rather than the question.
        ack: bool,
        /// Eight bytes, echoed exactly.
        data: [u8; 8],
    },
    /// The connection is ending.
    GoAway {
        /// The highest stream the sender may have acted on.
        last_stream: u32,
        /// Why.
        error: ErrorCode,
        /// Anything the sender wanted to say, which is for a person to read and
        /// for nothing to parse.
        debug: Vec<u8>,
    },
    /// Room for more data.
    WindowUpdate {
        /// Which stream, or zero for the connection as a whole.
        stream: u32,
        /// How much more. Never zero — see [`Broken`].
        increase: u32,
    },
    /// The rest of a header block.
    Continuation {
        /// Which stream.
        stream: u32,
        /// The rest of the HPACK block.
        block: Vec<u8>,
        /// Whether the block ends here.
        end_headers: bool,
    },
    /// A frame type this engine has not heard of.
    ///
    /// Kept rather than dropped so that a caller can count them, and so that
    /// "we ignored it" is a thing that happened rather than a thing that did
    /// not.
    Unknown {
        /// Its type byte.
        kind: u8,
        /// Which stream.
        stream: u32,
        /// Its payload, read exactly.
        payload: Vec<u8>,
    },
}

/// Why a frame could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Broken {
    /// In words.
    pub why: String,
    /// What to tell the other end, which decides whether the connection
    /// survives: a `FrameSizeError` on one stream may be that stream's problem,
    /// and one on the connection's own frames is everybody's.
    pub error: ErrorCode,
    /// Whether the whole connection has to end.
    pub fatal: bool,
}

impl fmt::Display for Broken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.why)
    }
}

impl std::error::Error for Broken {}

fn broken(why: impl Into<String>, error: ErrorCode) -> Broken {
    Broken {
        why: why.into(),
        error,
        fatal: true,
    }
}

/// What came off the wire when a frame was asked for.
///
/// The two are different things and collapsing them is what queue item 185 was
/// about: **a connection that ended is not a peer that misbehaved.** A caller
/// waiting for a response has to tell them apart, because bytes already
/// delivered by a connection that then ended are bytes worth keeping, and bytes
/// delivered by a peer breaking the protocol are not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arrived {
    /// A frame, read whole.
    Frame(Frame),
    /// Nothing more is coming, because the connection ended.
    ///
    /// The [`Broken`] says which way it ended — cleanly between frames, or in
    /// the middle of one. Both mean the same to somebody waiting for a response
    /// and only one of them is a server behaving properly, so the distinction is
    /// carried rather than flattened.
    Ended(Broken),
}

/// Read one frame, saying plainly when the connection simply ended.
///
/// [`read`] cannot say it: there, the end of a connection and a peer sending
/// something unreadable are both [`Broken`], and a caller that could not tell
/// them apart would treat a download's connection dropping as a violation and
/// throw away the bytes it already had.
///
/// # Errors
///
/// [`Broken`], for a peer that sent something this engine will not read. The
/// connection ending is [`Arrived::Ended`] rather than an error, which is the
/// whole difference between this and [`read`].
pub fn read_however_it_ends(source: &mut impl Read, largest: u32) -> Result<Arrived, Broken> {
    let mut head = [0u8; HEADER_BYTES];
    match fill(source, &mut head)? {
        Filled::Whole => {}
        Filled::EndedAfter(bytes) => return Ok(Arrived::Ended(ended_after(bytes))),
    }
    let length = u32::from(head[0]) << 16 | u32::from(head[1]) << 8 | u32::from(head[2]);
    let kind = head[3];
    let flags = head[4];
    // The top bit is reserved. A sender that sets it is not to be argued with,
    // and a reader that fails to mask it sees stream numbers near two billion.
    let stream = u32::from_be_bytes([head[5], head[6], head[7], head[8]]) & 0x7fff_ffff;

    if length > largest.min(LARGEST_ALLOWED) {
        return Err(broken(
            format!("a frame of {length} bytes, when {largest} was the most this end accepts"),
            ErrorCode::FrameSizeError,
        ));
    }

    let mut payload = vec![0u8; length as usize];
    match fill(source, &mut payload)? {
        Filled::Whole => {}
        // Past the header, so however few bytes arrived it was mid-frame.
        Filled::EndedAfter(bytes) => {
            return Ok(Arrived::Ended(ended_after(HEADER_BYTES + bytes)));
        }
    }
    interpret(kind, flags, stream, payload).map(Arrived::Frame)
}

/// Read one frame.
///
/// `largest` is what this end has told the peer it will accept — the value of
/// its own `SETTINGS_MAX_FRAME_SIZE`, which is why it is a parameter rather than
/// a constant.
///
/// # Errors
///
/// [`Broken`], carrying the error code to send back and whether the connection
/// can survive it.
pub fn read(source: &mut impl Read, largest: u32) -> Result<Frame, Broken> {
    match read_however_it_ends(source, largest)? {
        Arrived::Frame(frame) => Ok(frame),
        Arrived::Ended(why) => Err(why),
    }
}

/// What a payload means, once it has been read.
///
/// Separate from [`read`] so that every rule below can be tested without a
/// reader — which is most of this file's tests.
///
/// # Errors
///
/// [`Broken`] for a frame that is not a legal one of its type.
pub fn interpret(kind: u8, flags: u8, stream: u32, payload: Vec<u8>) -> Result<Frame, Broken> {
    match kind {
        0x0 => data(flags, stream, payload),
        0x1 => headers(flags, stream, payload),
        0x2 => priority(stream, &payload),
        0x3 => reset_stream(stream, &payload),
        0x4 => settings(flags, stream, &payload),
        0x5 => push_promise(flags, stream, payload),
        0x6 => ping(flags, stream, &payload),
        0x7 => go_away(stream, &payload),
        0x8 => window_update(stream, &payload),
        0x9 => continuation(flags, stream, payload),
        // Extensibility is on purpose, and a peer using an extension we do not
        // know is not misbehaving. Its length was checked and its bytes were
        // read exactly, which is what makes ignoring it safe.
        other => Ok(Frame::Unknown {
            kind: other,
            stream,
            payload,
        }),
    }
}

/// The flag bits, which are per-type and reused across types.
const END_STREAM: u8 = 0x1;
const ACK: u8 = 0x1;
const END_HEADERS: u8 = 0x4;
const PADDED: u8 = 0x8;
const HAS_PRIORITY: u8 = 0x20;

fn data(flags: u8, stream: u32, payload: Vec<u8>) -> Result<Frame, Broken> {
    on_a_stream(stream, "DATA")?;
    Ok(Frame::Data {
        stream,
        data: unpadded(payload, flags & PADDED != 0)?,
        end_stream: flags & END_STREAM != 0,
    })
}

fn headers(flags: u8, stream: u32, payload: Vec<u8>) -> Result<Frame, Broken> {
    on_a_stream(stream, "HEADERS")?;
    let mut rest = unpadded(payload, flags & PADDED != 0)?;
    let priority = if flags & HAS_PRIORITY != 0 {
        if rest.len() < 5 {
            return Err(broken(
                "a HEADERS frame that said it carried a priority and was too short for one",
                ErrorCode::FrameSizeError,
            ));
        }
        let taken: Vec<u8> = rest.drain(..5).collect();
        Some(priority_from(&taken))
    } else {
        None
    };
    Ok(Frame::Headers {
        stream,
        block: rest,
        end_stream: flags & END_STREAM != 0,
        end_headers: flags & END_HEADERS != 0,
        priority,
    })
}

fn priority(stream: u32, payload: &[u8]) -> Result<Frame, Broken> {
    on_a_stream(stream, "PRIORITY")?;
    exactly(payload, 5, "PRIORITY")?;
    Ok(Frame::Priority {
        stream,
        priority: priority_from(payload),
    })
}

fn reset_stream(stream: u32, payload: &[u8]) -> Result<Frame, Broken> {
    on_a_stream(stream, "RST_STREAM")?;
    exactly(payload, 4, "RST_STREAM")?;
    Ok(Frame::ResetStream {
        stream,
        error: ErrorCode::of(number(payload, 0)),
    })
}

fn settings(flags: u8, stream: u32, payload: &[u8]) -> Result<Frame, Broken> {
    on_the_connection(stream, "SETTINGS")?;
    if flags & ACK != 0 {
        if !payload.is_empty() {
            return Err(broken(
                "a SETTINGS acknowledgement carrying settings",
                ErrorCode::FrameSizeError,
            ));
        }
        return Ok(Frame::Settings {
            ack: true,
            values: Vec::new(),
        });
    }
    if payload.len() % 6 != 0 {
        return Err(broken(
            "a SETTINGS frame whose length is not a whole number of settings",
            ErrorCode::FrameSizeError,
        ));
    }
    // In order sent, because the same setting may appear twice and the last one
    // is what it means.
    let values = payload
        .chunks_exact(6)
        .map(|chunk| {
            let name = u16::from_be_bytes([
                chunk.first().copied().unwrap_or(0),
                chunk.get(1).copied().unwrap_or(0),
            ]);
            (Setting(name), number(chunk, 2))
        })
        .collect();
    Ok(Frame::Settings { ack: false, values })
}

fn push_promise(flags: u8, stream: u32, payload: Vec<u8>) -> Result<Frame, Broken> {
    on_a_stream(stream, "PUSH_PROMISE")?;
    let rest = unpadded(payload, flags & PADDED != 0)?;
    if rest.len() < 4 {
        return Err(broken(
            "a PUSH_PROMISE too short to name the stream it promises",
            ErrorCode::FrameSizeError,
        ));
    }
    Ok(Frame::PushPromise {
        stream,
        promised: number(&rest, 0) & 0x7fff_ffff,
        block: rest.get(4..).unwrap_or_default().to_vec(),
        end_headers: flags & END_HEADERS != 0,
    })
}

fn ping(flags: u8, stream: u32, payload: &[u8]) -> Result<Frame, Broken> {
    on_the_connection(stream, "PING")?;
    exactly(payload, 8, "PING")?;
    let mut data = [0u8; 8];
    data.copy_from_slice(payload.get(..8).unwrap_or(&[0; 8]));
    Ok(Frame::Ping {
        ack: flags & ACK != 0,
        data,
    })
}

fn go_away(stream: u32, payload: &[u8]) -> Result<Frame, Broken> {
    on_the_connection(stream, "GOAWAY")?;
    if payload.len() < 8 {
        return Err(broken(
            "a GOAWAY too short to say why",
            ErrorCode::FrameSizeError,
        ));
    }
    Ok(Frame::GoAway {
        last_stream: number(payload, 0) & 0x7fff_ffff,
        error: ErrorCode::of(number(payload, 4)),
        debug: payload.get(8..).unwrap_or_default().to_vec(),
    })
}

fn window_update(stream: u32, payload: &[u8]) -> Result<Frame, Broken> {
    exactly(payload, 4, "WINDOW_UPDATE")?;
    let increase = number(payload, 0) & 0x7fff_ffff;
    if increase == 0 {
        // Room for nothing is not room. Left unchecked it is a peer that can
        // make this end wait for a window that will never open.
        return Err(Broken {
            why: "a WINDOW_UPDATE offering no more room".to_owned(),
            error: ErrorCode::ProtocolError,
            fatal: stream == 0,
        });
    }
    Ok(Frame::WindowUpdate { stream, increase })
}

fn continuation(flags: u8, stream: u32, payload: Vec<u8>) -> Result<Frame, Broken> {
    on_a_stream(stream, "CONTINUATION")?;
    Ok(Frame::Continuation {
        stream,
        block: payload,
        end_headers: flags & END_HEADERS != 0,
    })
}

/// Remove padding, refusing the length that would run the payload backwards.
///
/// The classic HTTP/2 parser bug: the first byte says how much of the rest is
/// padding, and nothing stops it saying more than there is. Subtracting without
/// checking underflows, and in a language where that is not caught it reads
/// whatever was next in memory.
fn unpadded(payload: Vec<u8>, padded: bool) -> Result<Vec<u8>, Broken> {
    if !padded {
        return Ok(payload);
    }
    let Some(&how_much) = payload.first() else {
        return Err(broken(
            "a padded frame with no room for the padding length",
            ErrorCode::ProtocolError,
        ));
    };
    let how_much = usize::from(how_much);
    // Compared against the *whole* payload, the pad-length byte included. So
    // padding that consumes everything after that byte is legal and yields an
    // empty body — a frame that is nothing but padding is a real thing a server
    // sends to disguise a response's size. What is refused is padding that
    // would leave the body a negative length.
    if how_much >= payload.len() {
        return Err(broken(
            format!(
                "a frame claiming {how_much} bytes of padding inside {} bytes",
                payload.len()
            ),
            ErrorCode::ProtocolError,
        ));
    }
    let keep = payload.len() - 1 - how_much;
    Ok(payload.get(1..1 + keep).unwrap_or_default().to_vec())
}

fn priority_from(bytes: &[u8]) -> Priority {
    let word = number(bytes, 0);
    Priority {
        depends_on: word & 0x7fff_ffff,
        exclusive: word & 0x8000_0000 != 0,
        weight: bytes.get(4).copied().unwrap_or(0),
    }
}

fn number(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([
        bytes.get(at).copied().unwrap_or(0),
        bytes.get(at + 1).copied().unwrap_or(0),
        bytes.get(at + 2).copied().unwrap_or(0),
        bytes.get(at + 3).copied().unwrap_or(0),
    ])
}

fn exactly(payload: &[u8], want: usize, what: &str) -> Result<(), Broken> {
    if payload.len() == want {
        return Ok(());
    }
    Err(broken(
        format!(
            "a {what} frame of {} bytes, which must be exactly {want}",
            payload.len()
        ),
        ErrorCode::FrameSizeError,
    ))
}

/// These frames are about one stream, and stream zero is not one.
fn on_a_stream(stream: u32, what: &str) -> Result<(), Broken> {
    if stream == 0 {
        return Err(broken(
            format!("a {what} frame on stream 0, which is the connection rather than a stream"),
            ErrorCode::ProtocolError,
        ));
    }
    Ok(())
}

/// These frames are about the connection, and belong on no stream.
fn on_the_connection(stream: u32, what: &str) -> Result<(), Broken> {
    if stream != 0 {
        return Err(broken(
            format!("a {what} frame on stream {stream}, when it belongs to the connection"),
            ErrorCode::ProtocolError,
        ));
    }
    Ok(())
}

/// How far a read got before it ran out of connection.
enum Filled {
    /// Every byte asked for.
    Whole,
    /// The connection ended after this many of them — zero when it ended
    /// before the read had taken anything at all, which is a server hanging up
    /// tidily between frames.
    EndedAfter(usize),
}

/// Fill a buffer, saying how far it got when the connection ended under it.
///
/// A connection that ends is not a connection that fails, and the difference is
/// worth the four lines: the first is what every server does eventually, and
/// the second is a peer that has gone quiet without going away.
fn fill(source: &mut impl Read, into: &mut [u8]) -> Result<Filled, Broken> {
    let mut filled = 0;
    while filled < into.len() {
        let room = into.get_mut(filled..).unwrap_or_default();
        match source.read(room) {
            Ok(0) => return Ok(Filled::EndedAfter(filled)),
            Ok(got) => filled += got,
            Err(why) if is_the_connection_going(&why) => {
                return Ok(Filled::EndedAfter(filled));
            }
            Err(why) => return Err(broken(why.to_string(), ErrorCode::InternalError)),
        }
    }
    Ok(Filled::Whole)
}

/// Whether a failed read is the peer being gone rather than being slow.
///
/// A reset arrives instead of an orderly end whenever the peer closes with
/// something of ours still unread — which is exactly what a server hanging up
/// part way through a body does, since we are still sending it window updates.
/// So this is the same event as the end of a connection and is read as one.
///
/// A timeout is **not** here, and that is the point of the list being explicit:
/// a peer that has gone quiet may still be there, and treating a stall as an
/// ending would turn every slow server into a half-finished download.
fn is_the_connection_going(why: &std::io::Error) -> bool {
    matches!(
        why.kind(),
        std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::BrokenPipe
            // What a TLS connection closed without a `close_notify` looks
            // like, which is how most of the web ends a connection.
            | std::io::ErrorKind::UnexpectedEof
    )
}

/// The connection ending, said the way the two ways of ending differ.
fn ended_after(bytes: usize) -> Broken {
    if bytes == 0 {
        return broken("the connection ended", ErrorCode::NoError);
    }
    broken(
        "the connection ended in the middle of a frame",
        ErrorCode::ProtocolError,
    )
}

/// Write one frame.
///
/// Nothing here can produce a frame longer than the protocol allows, because
/// nothing here writes a length it was given — it writes the length of what it
/// actually wrote.
pub fn write(frame: &Frame) -> Vec<u8> {
    let (kind, flags, stream, payload) = match frame {
        Frame::Data {
            stream,
            data,
            end_stream,
        } => (0x0, u8::from(*end_stream), *stream, data.clone()),
        Frame::Headers {
            stream,
            block,
            end_stream,
            end_headers,
            priority,
        } => {
            let mut payload = Vec::new();
            let mut flags = u8::from(*end_stream) | (u8::from(*end_headers) << 2);
            if let Some(priority) = priority {
                flags |= 0x20;
                payload.extend_from_slice(&priority_bytes(*priority));
            }
            payload.extend_from_slice(block);
            (0x1, flags, *stream, payload)
        }
        Frame::Priority { stream, priority } => {
            (0x2, 0, *stream, priority_bytes(*priority).to_vec())
        }
        Frame::ResetStream { stream, error } => {
            (0x3, 0, *stream, error.number().to_be_bytes().to_vec())
        }
        Frame::Settings { ack, values } => {
            let mut payload = Vec::new();
            for (Setting(name), value) in values {
                payload.extend_from_slice(&name.to_be_bytes());
                payload.extend_from_slice(&value.to_be_bytes());
            }
            (0x4, u8::from(*ack), 0, payload)
        }
        Frame::PushPromise {
            stream,
            promised,
            block,
            end_headers,
        } => {
            let mut payload = (promised & 0x7fff_ffff).to_be_bytes().to_vec();
            payload.extend_from_slice(block);
            (0x5, u8::from(*end_headers) << 2, *stream, payload)
        }
        Frame::Ping { ack, data } => (0x6, u8::from(*ack), 0, data.to_vec()),
        Frame::GoAway {
            last_stream,
            error,
            debug,
        } => {
            let mut payload = (last_stream & 0x7fff_ffff).to_be_bytes().to_vec();
            payload.extend_from_slice(&error.number().to_be_bytes());
            payload.extend_from_slice(debug);
            (0x7, 0, 0, payload)
        }
        Frame::WindowUpdate { stream, increase } => (
            0x8,
            0,
            *stream,
            (increase & 0x7fff_ffff).to_be_bytes().to_vec(),
        ),
        Frame::Continuation {
            stream,
            block,
            end_headers,
        } => (0x9, u8::from(*end_headers) << 2, *stream, block.clone()),
        Frame::Unknown {
            kind,
            stream,
            payload,
        } => (*kind, 0, *stream, payload.clone()),
    };

    let length = u32::try_from(payload.len()).unwrap_or(LARGEST_ALLOWED);
    let mut out = Vec::with_capacity(HEADER_BYTES + payload.len());
    let bytes = length.to_be_bytes();
    out.extend_from_slice(bytes.get(1..4).unwrap_or(&[0, 0, 0]));
    out.push(kind);
    out.push(flags);
    out.extend_from_slice(&(stream & 0x7fff_ffff).to_be_bytes());
    out.extend_from_slice(&payload);
    out
}

fn priority_bytes(priority: Priority) -> [u8; 5] {
    let mut word = priority.depends_on & 0x7fff_ffff;
    if priority.exclusive {
        word |= 0x8000_0000;
    }
    let [a, b, c, d] = word.to_be_bytes();
    [a, b, c, d, priority.weight]
}
