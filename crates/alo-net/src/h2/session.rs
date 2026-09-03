//! The connection: which streams exist, and what a peer may do to us.
//!
//! [`super::stream`] knows one stream. This knows all of them, and it is the
//! file where a misbehaving peer is stopped — because every way a peer can
//! spend our memory is a *count* rather than a single oversized thing:
//!
//! - **Streams opened and never used.** Bounded by how many may be open at
//!   once, which we tell the peer and then enforce rather than trust.
//! - **A header block spread across `CONTINUATION` frames.** Each frame is
//!   inside the frame-size limit and there is no limit on how many there are.
//!   That is the CONTINUATION flood, and the bound has to be on the **total
//!   across frames**, which is why it is counted here rather than in the frame
//!   reader.
//! - **Streams that are closed but remembered.** A peer that opens and resets a
//!   million streams leaves a million entries unless closed ones are forgotten.
//!
//! Every one of those is a bound written before the code that would need it,
//! which is the order this file was built in.

use super::ErrorCode;
use super::flow::{self, Window};
use super::frame::{Frame, Setting};
use super::stream::{State, Stream};
use std::collections::HashMap;

/// How many streams this end will let a peer have open at once.
pub const MOST_OPEN: usize = 100;

/// The most bytes one header block may be, added up across every
/// `CONTINUATION` frame that carries it.
///
/// The bound the CONTINUATION flood needs. Each frame is individually legal;
/// what is not legal is ten thousand of them.
pub const LONGEST_HEADER_BLOCK: usize = 64 * 1024;

/// The most closed streams remembered, so a peer that opens and resets streams
/// forever does not leave an entry for each.
const REMEMBER_CLOSED: usize = 64;

/// A header block being assembled out of frames.
#[derive(Debug, Clone)]
struct Assembling {
    stream: u32,
    block: Vec<u8>,
    end_stream: bool,
}

/// One HTTP/2 connection's worth of state.
#[derive(Debug)]
pub struct Session {
    streams: HashMap<u32, Stream>,
    /// The highest stream number that has been used, so a lower one can be
    /// refused. Stream numbers only go up; a peer reusing one is either
    /// confused or trying to reach a stream that is gone.
    highest_seen: u32,
    /// The next number this end will open on. Odd, because we are the client.
    next_ours: u32,
    /// A header block part-way through arriving. While this is set, **nothing
    /// else may arrive** — a `CONTINUATION` sequence is uninterruptible, and a
    /// frame for another stream in the middle of one is a protocol error.
    assembling: Option<Assembling>,
    sending: Window,
    receiving: Window,
    /// What the peer said its streams start with.
    their_initial_window: i64,
    /// What we told the peer ours start with.
    our_initial_window: i64,
    /// How many the peer said we may open. Not a bound on *them*; that is
    /// [`MOST_OPEN`], which is ours to enforce.
    they_allow_us: usize,
    closed_recently: Vec<u32>,
    /// Set when a `GOAWAY` has arrived: no new streams, and the ones above the
    /// number it named were never acted on.
    going_away: Option<u32>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    /// A connection with nothing on it yet.
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
            highest_seen: 0,
            next_ours: 1,
            assembling: None,
            sending: Window::new(),
            receiving: Window::new(),
            their_initial_window: flow::AT_FIRST,
            our_initial_window: flow::AT_FIRST,
            they_allow_us: MOST_OPEN,
            closed_recently: Vec::new(),
            going_away: None,
        }
    }

    /// How many streams are open.
    pub fn open_streams(&self) -> usize {
        self.streams
            .values()
            .filter(|stream| stream.state != State::Closed)
            .count()
    }

    /// The connection's own receiving window.
    pub fn receiving(&self) -> Window {
        self.receiving
    }

    /// The connection's own sending window.
    pub fn sending(&self) -> Window {
        self.sending
    }

    /// Take room to send `wanted` bytes on a stream, against both windows.
    ///
    /// Returns how much may actually go, which is the smaller of what was
    /// wanted, what the stream allows, and what the connection allows. Both,
    /// because a `DATA` frame counts against both — a sender that checked only
    /// the stream would overrun the connection on the second stream.
    pub fn room_to_send(&mut self, stream: u32, wanted: usize) -> usize {
        let by_connection = self.sending.room().max(0);
        let can = usize::try_from(by_connection)
            .unwrap_or(usize::MAX)
            .min(wanted);
        let Some(known) = self.streams.get_mut(&stream) else {
            return 0;
        };
        let going = known.sending.take(can);
        self.sending.take(going);
        going
    }

    /// One stream, if it is still remembered.
    pub fn stream(&self, id: u32) -> Option<&Stream> {
        self.streams.get(&id)
    }

    /// Whether the peer has said it is going away.
    pub fn is_going_away(&self) -> bool {
        self.going_away.is_some()
    }

    /// Open a stream for a request of ours.
    ///
    /// # Errors
    ///
    /// [`Broken`] when the peer has said it is going away, or when it has not
    /// left room for another.
    pub fn open(&mut self) -> Result<u32, Broken> {
        if self.going_away.is_some() {
            return Err(Broken {
                why: "a new stream after the server said it was going away".to_owned(),
                error: ErrorCode::RefusedStream,
                fatal: false,
            });
        }
        if self.open_streams() >= self.they_allow_us {
            return Err(Broken {
                why: format!(
                    "a {}th stream, when the server allows {}",
                    self.open_streams() + 1,
                    self.they_allow_us
                ),
                error: ErrorCode::RefusedStream,
                fatal: false,
            });
        }
        let id = self.next_ours;
        self.next_ours += 2;
        let mut stream = Stream::new(id, self.their_initial_window, self.our_initial_window);
        stream.state = State::Open;
        self.streams.insert(id, stream);
        self.highest_seen = self.highest_seen.max(id);
        Ok(id)
    }

    /// Take one frame from the peer.
    ///
    /// Returns a completed header block when one has just finished arriving —
    /// which is the only thing above this that needs to know about
    /// `CONTINUATION` at all.
    ///
    /// # Errors
    ///
    /// [`Broken`], carrying whether the connection survives it.
    pub fn arrived(&mut self, frame: Frame) -> Result<Option<Delivered>, Broken> {
        // A CONTINUATION sequence is uninterruptible. While one is in progress
        // the only legal frame is another CONTINUATION on the same stream, and
        // this check has to come before every other, because a peer that could
        // interleave would be a peer that could make two blocks into one.
        if let Some(assembling) = &self.assembling {
            let wanted = assembling.stream;
            let Frame::Continuation { stream, .. } = &frame else {
                return Err(fatal(
                    "a frame in the middle of a header block, where only CONTINUATION may be",
                    ErrorCode::ProtocolError,
                ));
            };
            if *stream != wanted {
                return Err(fatal(
                    "a CONTINUATION for another stream in the middle of a header block",
                    ErrorCode::ProtocolError,
                ));
            }
        }

        match frame {
            Frame::Headers {
                stream,
                block,
                end_stream,
                end_headers,
                ..
            } => self.headers(stream, block, end_stream, end_headers),
            Frame::Continuation {
                block, end_headers, ..
            } => self.continuation(&block, end_headers),
            Frame::Data {
                stream,
                data,
                end_stream,
            } => self.data(stream, data, end_stream),
            Frame::ResetStream { stream, error } => {
                if let Some(known) = self.streams.get_mut(&stream) {
                    known.reset(error);
                    self.forget_the_oldest_closed(stream);
                }
                Ok(None)
            }
            Frame::Settings { ack, values } => {
                if !ack {
                    self.settings(&values)?;
                }
                Ok(None)
            }
            Frame::WindowUpdate { stream, increase } => {
                self.window_update(stream, increase)?;
                Ok(None)
            }
            Frame::GoAway { last_stream, .. } => {
                self.going_away = Some(last_stream);
                Ok(None)
            }
            // Push is refused rather than handled: this engine sends
            // `ENABLE_PUSH: 0`, so a server reaching here has ignored what it
            // was told, and honouring it anyway would be accepting a response
            // to a request nobody made.
            Frame::PushPromise { .. } => Err(fatal(
                "a PUSH_PROMISE, when this engine said it would not accept one",
                ErrorCode::ProtocolError,
            )),
            // Priority is legal in every state, including on a stream nothing
            // has happened on, and nothing acts on it.
            Frame::Priority { .. } | Frame::Ping { .. } | Frame::Unknown { .. } => Ok(None),
        }
    }

    fn headers(
        &mut self,
        stream: u32,
        block: Vec<u8>,
        end_stream: bool,
        end_headers: bool,
    ) -> Result<Option<Delivered>, Broken> {
        // Stream numbers only go up. A frame on a number below the highest is
        // either confusion or an attempt to reach a stream that is gone.
        if stream < self.highest_seen && !self.streams.contains_key(&stream) {
            return Err(fatal(
                &format!(
                    "a HEADERS on stream {stream}, below the {} already used",
                    self.highest_seen
                ),
                ErrorCode::ProtocolError,
            ));
        }
        if !self.streams.contains_key(&stream) {
            if self.open_streams() >= MOST_OPEN {
                return Err(Broken {
                    why: format!("more than {MOST_OPEN} streams open at once"),
                    error: ErrorCode::RefusedStream,
                    fatal: false,
                });
            }
            self.streams.insert(
                stream,
                Stream::new(stream, self.their_initial_window, self.our_initial_window),
            );
        }
        self.highest_seen = self.highest_seen.max(stream);
        let known = self
            .streams
            .get_mut(&stream)
            .ok_or_else(|| fatal("a stream that vanished", ErrorCode::InternalError))?;
        known.headers_arrived(end_stream)?;

        if block.len() > LONGEST_HEADER_BLOCK {
            return Err(too_much_header());
        }
        if end_headers {
            return Ok(Some(Delivered::Headers {
                stream,
                block,
                end_stream,
            }));
        }
        self.assembling = Some(Assembling {
            stream,
            block,
            end_stream,
        });
        Ok(None)
    }

    fn continuation(
        &mut self,
        block: &[u8],
        end_headers: bool,
    ) -> Result<Option<Delivered>, Broken> {
        let Some(mut assembling) = self.assembling.take() else {
            return Err(fatal(
                "a CONTINUATION with no header block in progress",
                ErrorCode::ProtocolError,
            ));
        };
        // The bound the flood needs: on the total, across frames. Each frame
        // was individually legal; ten thousand of them are not.
        if assembling.block.len() + block.len() > LONGEST_HEADER_BLOCK {
            return Err(too_much_header());
        }
        assembling.block.extend_from_slice(block);
        if end_headers {
            return Ok(Some(Delivered::Headers {
                stream: assembling.stream,
                block: assembling.block,
                end_stream: assembling.end_stream,
            }));
        }
        self.assembling = Some(assembling);
        Ok(None)
    }

    fn data(
        &mut self,
        stream: u32,
        data: Vec<u8>,
        end_stream: bool,
    ) -> Result<Option<Delivered>, Broken> {
        // Against the connection first, and fatally: the connection's window is
        // everybody's, and a peer that overran it has already spent the memory.
        self.receiving.arrived(data.len(), true)?;
        let known = self.streams.get_mut(&stream).ok_or_else(|| Broken {
            why: format!("a DATA frame on stream {stream}, which is not open"),
            error: ErrorCode::StreamClosed,
            fatal: false,
        })?;
        known.data_arrived(data.len(), end_stream)?;
        if end_stream {
            self.forget_the_oldest_closed(stream);
        }
        Ok(Some(Delivered::Data {
            stream,
            data,
            end_stream,
        }))
    }

    fn settings(&mut self, values: &[(Setting, u32)]) -> Result<(), Broken> {
        for (setting, value) in values {
            match *setting {
                Setting::INITIAL_WINDOW_SIZE => {
                    if i64::from(*value) > flow::CEILING {
                        return Err(fatal(
                            "an initial window size past what the protocol allows",
                            ErrorCode::FlowControlError,
                        ));
                    }
                    // Retroactive, on every stream that already exists. This is
                    // where a window legitimately goes negative.
                    let difference = i64::from(*value) - self.their_initial_window;
                    self.their_initial_window = i64::from(*value);
                    for stream in self.streams.values_mut() {
                        stream.sending.resettle(difference)?;
                    }
                }
                Setting::MAX_CONCURRENT_STREAMS => {
                    self.they_allow_us = usize::try_from(*value).unwrap_or(MOST_OPEN);
                }
                Setting::ENABLE_PUSH if *value > 1 => {
                    return Err(fatal(
                        "an ENABLE_PUSH that is neither zero nor one",
                        ErrorCode::ProtocolError,
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn window_update(&mut self, stream: u32, increase: u32) -> Result<(), Broken> {
        if stream == 0 {
            return self.sending.widen(increase, true);
        }
        // A window update for a stream that is gone is not an error: the peer
        // sent it before it knew, and the two crossed. Ignoring it is correct;
        // refusing it would end connections over a race nobody lost.
        if let Some(known) = self.streams.get_mut(&stream) {
            known.sending.widen(increase, false)?;
        }
        Ok(())
    }

    /// Keep only the last few closed streams, so a peer that opens and resets
    /// forever does not leave an entry for each.
    fn forget_the_oldest_closed(&mut self, just_closed: u32) {
        self.closed_recently.retain(|id| id != &just_closed);
        self.closed_recently.push(just_closed);
        while self.closed_recently.len() > REMEMBER_CLOSED {
            if self.closed_recently.is_empty() {
                break;
            }
            let oldest = self.closed_recently.remove(0);
            self.streams.remove(&oldest);
        }
    }
}

/// Something whole, put together out of frames.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delivered {
    /// A complete header block, however many frames it took.
    Headers {
        /// Which stream.
        stream: u32,
        /// The undecoded HPACK block.
        block: Vec<u8>,
        /// Whether the stream ended with it.
        end_stream: bool,
    },
    /// Body bytes.
    Data {
        /// Which stream.
        stream: u32,
        /// The bytes.
        data: Vec<u8>,
        /// Whether the stream ended with them.
        end_stream: bool,
    },
}

use super::frame::Broken;

fn fatal(why: &str, error: ErrorCode) -> Broken {
    Broken {
        why: why.to_owned(),
        error,
        fatal: true,
    }
}

fn too_much_header() -> Broken {
    Broken {
        why: format!(
            "a header block longer than the {LONGEST_HEADER_BLOCK} bytes this engine will assemble"
        ),
        error: ErrorCode::EnhanceYourCalm,
        fatal: true,
    }
}
