/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One request and one response, over HTTP/2.
//!
//! # What is different from HTTP/1.1, beyond the framing
//!
//! **There is no request line and there are no status lines.** The method, the
//! scheme, the path and the authority are *headers* whose names begin with a
//! colon, and a colon is a character no ordinary header name may contain — which
//! is what makes them impossible to forge from an ordinary one. They must come
//! before every other header, and this is where that is enforced on the way out
//! and checked on the way in.
//!
//! **Some HTTP/1.1 headers are forbidden.** `Connection`, `Keep-Alive`,
//! `Transfer-Encoding` and their friends describe a hop, and HTTP/2 has its own
//! way of saying all of it. A server that receives one must treat the message as
//! malformed, so sending one is not a compatibility gesture — it is a broken
//! request.
//!
//! **Header names are lowercase.** Not conventionally: a name with a capital in
//! it is malformed, and a peer is entitled to reject the whole message.

use super::frame::{self, Broken, Frame, Setting};
use super::hpack::{self, Field, Table};
use super::session::{Delivered, Session};
use super::{ErrorCode, flow};
use crate::headers::Headers;
use crate::request::Request;
use crate::response::{Response, Status};
use std::io::{Read, Write};

/// Headers that describe one hop of an HTTP/1.1 connection, and which HTTP/2
/// has its own way of saying.
///
/// Sending one is not a compatibility gesture; it makes the message malformed.
const ABOUT_THE_HOP: [&str; 6] = [
    "connection",
    "keep-alive",
    "proxy-connection",
    "transfer-encoding",
    "upgrade",
    "host",
];

/// Everything one connection needs to remember between exchanges.
#[derive(Debug)]
pub struct Speaking {
    /// Which streams exist, and what may arrive on them.
    pub session: Session,
    /// The table this end encodes against.
    pub writing: Table,
    /// The table this end decodes against.
    pub reading: Table,
    /// Whether the preface and first `SETTINGS` have gone out.
    started: bool,
}

impl Default for Speaking {
    fn default() -> Self {
        Self::new()
    }
}

impl Speaking {
    /// A connection that has not said hello yet.
    pub fn new() -> Self {
        Self {
            session: Session::new(),
            writing: Table::new(4096),
            reading: Table::new(4096),
            started: false,
        }
    }
}

/// Say hello: the preface, and what this end will accept.
///
/// # Errors
///
/// [`Broken`] when the connection could not be written to.
pub fn begin(wire: &mut impl Write, speaking: &mut Speaking) -> Result<(), Broken> {
    if speaking.started {
        return Ok(());
    }
    let mut out = frame::PREFACE.to_vec();
    out.extend_from_slice(&frame::write(&Frame::Settings {
        ack: false,
        values: vec![
            // Zero, and meant: a pushed response is a response to a request
            // nobody made, and this engine has nowhere to put one. `session`
            // refuses a `PUSH_PROMISE` outright, so saying so here is what
            // makes that refusal fair rather than a surprise.
            (Setting::ENABLE_PUSH, 0),
            (
                Setting::MAX_CONCURRENT_STREAMS,
                u32::try_from(super::session::MOST_OPEN).unwrap_or(100),
            ),
            (
                Setting::MAX_HEADER_LIST_SIZE,
                u32::try_from(super::session::LONGEST_HEADER_BLOCK).unwrap_or(65_536),
            ),
        ],
    }));
    write_all(wire, &out)?;
    speaking.started = true;
    Ok(())
}

/// A response, and why its stream ended before its body did — when it did.
///
/// The same shape [`crate::connection::Exchanged`] has, for the same reason:
/// half a page is not a page, and half a file is the first half of a file.
#[derive(Debug)]
pub struct Ended {
    /// The response, carrying whatever body arrived.
    pub response: Response,
    /// Why the stream ended without `END_STREAM`, when it did.
    pub short: Option<Broken>,
}

/// Do one request and read its response.
///
/// # Errors
///
/// [`Broken`], carrying whether the connection survives it — including a
/// stream that ended before its body did, which is what makes this the strict
/// half of [`exchange_however_it_ends`].
pub fn exchange(
    wire: &mut (impl Read + Write),
    speaking: &mut Speaking,
    request: &Request,
) -> Result<Response, Broken> {
    let ended = exchange_however_it_ends(wire, speaking, request)?;
    match ended.short {
        Some(why) => Err(why),
        None => Ok(ended.response),
    }
}

/// The same exchange, handing up a stream that ended early rather than
/// refusing it.
///
/// **Nothing but a download should call this**, for the reason
/// [`crate::connection::exchange_however_it_ends`] gives on the HTTP/1.1 side:
/// a body that stopped is a wrong page everywhere else. A download does not
/// show the bytes — it asks for the rest of them and checks, byte position by
/// byte position, that what comes back belongs where it is put.
///
/// **Two ways a stream ends early, and only two.** The connection ends, and the
/// server resets the stream. Everything else — a header block that will not
/// decode, a window overrun, a frame where none may be — stays an error and
/// takes the bytes with it, because bytes delivered by a peer that is breaking
/// the protocol are not bytes to build a file out of.
///
/// # Errors
///
/// [`Broken`], for a peer that misbehaved rather than a stream that stopped —
/// and for a stream that stopped **before any response headers arrived**, since
/// there is then no response to hand up and no byte to resume from.
pub fn exchange_however_it_ends(
    wire: &mut (impl Read + Write),
    speaking: &mut Speaking,
    request: &Request,
) -> Result<Ended, Broken> {
    let (stream, mut unsent) = send_request(wire, speaking, request)?;
    let mut answer = Assembling::new();
    let mut short = None;

    loop {
        let frame = match frame::read_however_it_ends(wire, frame::LARGEST_BY_DEFAULT)? {
            frame::Arrived::Frame(frame) => frame,
            frame::Arrived::Ended(why) => {
                short = Some(why);
                break;
            }
        };
        // Every write from here down is an answer to something already read, so
        // a connection that will not take one is a connection nothing more will
        // arrive on. That ends the response rather than failing it: the frames
        // that did arrive were read whole and checked, and throwing them away
        // because the socket closed while we were being polite is exactly what
        // queue item 185 is about.
        if let Err(why) = answer_what_must_be_answered(wire, &frame) {
            short = Some(why);
            break;
        }

        // Read before the session takes the frame, and acted on after: the
        // session's bookkeeping for a reset stream still has to happen, and it
        // is the session that owns what a closed stream means.
        let reset = match &frame {
            Frame::ResetStream { stream: on, error } if *on == stream => Some(*error),
            _ => None,
        };
        let delivered = speaking.session.arrived(frame)?;
        if let Some(error) = reset {
            short = Some(Broken {
                why: format!("the server gave up on the stream: {error}"),
                error,
                // One stream ending is not the connection ending. Whether this
                // one is kept is decided above, by whoever holds the pool.
                fatal: false,
            });
            break;
        }

        // A window that has just widened is body that may now go. This is the
        // whole of "waited on rather than overrun": nothing is sent that the
        // peer did not make room for, and what could not go stays here until it
        // says so.
        if !unsent.is_empty() {
            push_body(wire, speaking, stream, &mut unsent)?;
        }

        let Some(delivered) = delivered else {
            continue;
        };
        match delivered {
            Delivered::Headers {
                stream: on,
                block,
                end_stream,
            } => {
                if on != stream {
                    continue;
                }
                let fields = hpack::decode(&block, &mut speaking.reading)?;
                match answer.took_a_header_block(&fields, end_stream)? {
                    // The session has to be told, because nothing below the
                    // decoder can tell a `103` from a `200` — and it is the
                    // session that decides whether the *next* block is legal.
                    Block::Interim => speaking.session.headers_were_interim(stream),
                    Block::More => {}
                    Block::Whole => break,
                }
            }
            Delivered::Data {
                stream: on,
                data,
                end_stream,
            } => {
                if on != stream {
                    continue;
                }
                answer.took_body(&data)?;
                // Room made back as the body is taken, or the window closes and
                // the server stops sending half way through a large page.
                let widen = u32::try_from(data.len()).unwrap_or(0);
                let room = make_room_back(wire, stream, widen);
                speaking.session.receiving_widened(widen);
                if end_stream {
                    // The response is whole, so a window that could not be
                    // widened cannot make it less so. What it does mean is a
                    // connection that is probably dead — which is the race
                    // `crate::pool` is written around rather than something
                    // this exchange can do anything about.
                    break;
                }
                if let Err(why) = room {
                    short = Some(why);
                    break;
                }
            }
        }
    }

    if !unsent.is_empty() {
        // The answer came before the request finished — a `413`, a redirect, a
        // server that had made its mind up. The rest of the body is bytes
        // nobody wants, and a stream we simply stopped writing to would stay
        // open until the connection ended, counting against how many this
        // engine may have. The write itself is allowed to fail without
        // spoiling the response: the response is whole, and a connection that
        // will not take a reset is one the pool finds out about on its next
        // use rather than one this exchange can mend.
        let _ = write_all(
            wire,
            &frame::write(&Frame::ResetStream {
                stream,
                error: ErrorCode::Cancel,
            }),
        );
        speaking.session.gave_up_on(stream);
    }

    let Some(status) = answer.status else {
        // A stream that stopped before its headers is not a short response, it
        // is no response: there is nothing to say what arrived, and no byte to
        // ask for the rest from. It is also what the pool's retry is for —
        // nothing arrived, so asking again is not guessing.
        return Err(short
            .unwrap_or_else(|| malformed("a response with no :status, which is not a response")));
    };
    Ok(Ended {
        response: Response {
            url: request.url.clone(),
            status,
            headers: answer.headers,
            body: answer.body,
        },
        short,
    })
}

/// What a header block turned out to be.
///
/// Three things arrive looking identical on the wire, and only the decoded
/// `:status` and what came before tell them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    /// Something said before the answer, which is not the answer.
    Interim,
    /// Part of the message, which is not finished.
    More,
    /// The message, finished.
    Whole,
}

/// A response being put together out of what arrives on its stream.
///
/// Separate from the exchange because it is a different question: the exchange
/// asks what is on the wire, and this asks what the message *is* — which block
/// is the answer, which is something said before it, and which is the trailers.
#[derive(Debug)]
struct Assembling {
    status: Option<Status>,
    headers: Headers,
    body: Vec<u8>,
    /// How many interim responses have arrived, so that a server cannot spend
    /// this tab's afternoon saying nothing.
    interim_so_far: usize,
}

impl Assembling {
    fn new() -> Self {
        Self {
            status: None,
            headers: Headers::new(),
            body: Vec::new(),
            interim_so_far: 0,
        }
    }

    /// Take one decoded header block, and say what it turned out to be.
    ///
    /// # Errors
    ///
    /// [`Broken`] for a block that is not one this message may have here.
    fn took_a_header_block(&mut self, fields: &[Field], end_stream: bool) -> Result<Block, Broken> {
        // A block before the answer has to carry a `:status`; one after it is
        // trailers, and must carry no pseudo-header at all.
        let (arrived, said) = read_block(fields, self.status.is_none())?;
        match arrived {
            Some(before) if before.is_interim() => {
                if end_stream {
                    return Err(malformed(
                        "an interim response that ends the stream, when it is not the response",
                    ));
                }
                self.interim_so_far += 1;
                if self.interim_so_far > Status::MOST_INTERIM {
                    return Err(malformed(&format!(
                        "more than {} interim responses before the answer",
                        Status::MOST_INTERIM
                    )));
                }
                // Its headers are read and dropped: `103 Early Hints` is a list
                // of things to fetch early, and fetching early is queue item
                // 113's business rather than this file's. Dropping them is a
                // decision, not an oversight.
                return Ok(Block::Interim);
            }
            // The answer, or a trailer block, which carries no status.
            Some(answer) => self.status = Some(answer),
            None => {}
        }
        for header in said.iter() {
            self.headers.add(header.name.clone(), header.value.clone());
        }
        Ok(if end_stream {
            Block::Whole
        } else {
            Block::More
        })
    }

    /// Take body bytes.
    ///
    /// # Errors
    ///
    /// [`Broken`] when they arrived before anything said what message they
    /// belong to.
    fn took_body(&mut self, data: &[u8]) -> Result<(), Broken> {
        if self.status.is_none() {
            return Err(malformed(
                "a DATA frame before the response's headers, which is a body belonging to no \
                 message",
            ));
        }
        self.body.extend_from_slice(data);
        Ok(())
    }
}

/// Say hello if this is the first exchange, open a stream, and send the
/// request — its head, and as much of its body as the peer has made room for.
///
/// Returns the stream it went out on, and whatever of the body has not gone
/// yet, which is empty in the ordinary case: both windows start at sixty-four
/// kilobytes, which is more than any form.
///
/// # Errors
///
/// [`Broken`] when the connection could not be written to, when no stream may
/// be opened, or when the request asks for something this engine will not
/// promise — see [`Request::unmet_expectation`].
fn send_request<'a>(
    wire: &mut impl Write,
    speaking: &mut Speaking,
    request: &'a Request,
) -> Result<(u32, &'a [u8]), Broken> {
    // Refused before the connection is touched, so nothing has been written
    // and the connection is exactly as it was.
    if let Some(why) = request.unmet_expectation() {
        return Err(Broken {
            why,
            error: ErrorCode::InternalError,
            fatal: false,
        });
    }
    begin(wire, speaking)?;
    let stream = speaking.session.open()?;
    let block = hpack::encode(&fields_for(request), &mut speaking.writing);
    let mut unsent: &[u8] = &request.body;
    write_all(
        wire,
        &frame::write(&Frame::Headers {
            stream,
            block,
            // `END_STREAM` here says truthfully that there is nothing more,
            // which is the whole of what a request without a body is. With one,
            // the flag goes on the last `DATA` frame instead.
            end_stream: unsent.is_empty(),
            end_headers: true,
            priority: None,
        }),
    )?;
    if unsent.is_empty() {
        speaking.session.finished_sending(stream);
    } else {
        push_body(wire, speaking, stream, &mut unsent)?;
    }
    Ok((stream, unsent))
}

/// Send as much of the body as the peer has made room for, and no more.
///
/// **This is where a body meets flow control.** Three numbers bound every frame
/// that goes out and all three are the peer's: the stream's window, the
/// connection's window, and the largest frame it said it would read. When they
/// leave no room this returns having sent what it could, and the caller reads
/// until a `WINDOW_UPDATE` arrives and calls it again — which is the difference
/// between waiting on a window and overrunning it.
///
/// # Errors
///
/// [`Broken`] when the connection could not be written to.
fn push_body(
    wire: &mut impl Write,
    speaking: &mut Speaking,
    stream: u32,
    unsent: &mut &[u8],
) -> Result<(), Broken> {
    let most = speaking.session.most_in_one_frame();
    while !unsent.is_empty() {
        let wanted = unsent.len().min(most);
        let going = speaking.session.room_to_send(stream, wanted);
        if going == 0 {
            // The window is shut. Nothing to do but read until it opens.
            return Ok(());
        }
        let (Some(now), Some(rest)) = (unsent.get(..going), unsent.get(going..)) else {
            // `room_to_send` never hands back more than it was asked for, so
            // this is arithmetic that cannot happen rather than a case; leaving
            // the bytes unsent is the answer that cannot make anything worse.
            return Ok(());
        };
        let last = rest.is_empty();
        write_all(
            wire,
            &frame::write(&Frame::Data {
                stream,
                data: now.to_vec(),
                end_stream: last,
            }),
        )?;
        *unsent = rest;
        if last {
            speaking.session.finished_sending(stream);
        }
    }
    Ok(())
}

/// Reply to the frames a peer waits for before it will send anything more.
///
/// A peer waiting for a `SETTINGS` acknowledgement or a `PING` answer stops
/// sending, so a client that only replied once it had a whole response would
/// deadlock waiting for a response the server is waiting to be allowed to send.
fn answer_what_must_be_answered(wire: &mut impl Write, frame: &Frame) -> Result<(), Broken> {
    match frame {
        Frame::Settings { ack: false, .. } => write_all(
            wire,
            &frame::write(&Frame::Settings {
                ack: true,
                values: Vec::new(),
            }),
        ),
        Frame::Ping { ack: false, data } => write_all(
            wire,
            &frame::write(&Frame::Ping {
                ack: true,
                data: *data,
            }),
        ),
        _ => Ok(()),
    }
}

/// Make room back for what has been taken out of the stream, on both windows.
fn make_room_back(wire: &mut impl Write, stream: u32, by: u32) -> Result<(), Broken> {
    if by == 0 {
        return Ok(());
    }
    write_all(
        wire,
        &frame::write(&Frame::WindowUpdate {
            stream: 0,
            increase: by,
        }),
    )?;
    write_all(
        wire,
        &frame::write(&Frame::WindowUpdate {
            stream,
            increase: by,
        }),
    )
}

/// One decoded header block: its status, when it is a block that has one, and
/// its ordinary headers.
///
/// `carries_status` is what tells a **response** from its **trailers**, which
/// are the same thing on the wire and are told apart only by which came first.
/// A trailer block carrying a `:status` would be a second response smuggled
/// into the end of the first one.
fn read_block(fields: &[Field], carries_status: bool) -> Result<(Option<Status>, Headers), Broken> {
    let mut status = None;
    let mut headers = Headers::new();
    for field in fields {
        if let Some(name) = field.name.strip_prefix(':') {
            if !carries_status {
                return Err(malformed(&format!(
                    "a trailer block carrying :{name}, where no pseudo-header may be"
                )));
            }
            if name != "status" {
                return Err(malformed(&format!(
                    "a response carrying :{name}, which only a request may have"
                )));
            }
            if status.is_some() {
                return Err(malformed("a response with two :status headers"));
            }
            status = Some(Status(field.value.parse::<u16>().map_err(|_| {
                malformed(&format!(
                    "a :status of {:?}, which is not a number",
                    field.value
                ))
            })?));
            continue;
        }
        // A pseudo-header after an ordinary one is how a message is smuggled
        // past something that only reads the first few headers.
        if carries_status && status.is_none() {
            return Err(malformed("a header before :status"));
        }
        if ABOUT_THE_HOP
            .iter()
            .any(|forbidden| field.name.eq_ignore_ascii_case(forbidden))
        {
            return Err(malformed(&format!(
                "a response carrying {}, which HTTP/2 forbids",
                field.name
            )));
        }
        if field.name.chars().any(|letter| letter.is_ascii_uppercase()) {
            return Err(malformed(&format!(
                "a header named {:?}, when HTTP/2 names are lowercase",
                field.name
            )));
        }
        headers.add(field.name.clone(), field.value.clone());
    }
    if carries_status && status.is_none() {
        return Err(malformed(
            "a header block with no :status, which is not a response",
        ));
    }
    Ok((status, headers))
}

/// A request as HTTP/2 carries it: the four pseudo-headers first, then the
/// ordinary ones, lowercased, with the hop-by-hop ones dropped.
fn fields_for(request: &Request) -> Vec<Field> {
    let url = &request.url;
    let mut target = if url.path.is_empty() {
        "/".to_owned()
    } else {
        url.path.clone()
    };
    if let Some(query) = &url.query {
        target.push('?');
        target.push_str(query);
    }
    let authority = match (&url.host, url.port) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_string(),
        (None, _) => String::new(),
    };

    let mut fields = vec![
        Field::new(":method", request.method.to_ascii_uppercase()),
        Field::new(":scheme", url.scheme.clone()),
        Field::new(":authority", authority),
        Field::new(":path", target),
    ];
    // Optional in HTTP/2, where `END_STREAM` is what actually frames a body —
    // and, when it is there, it must agree with the bytes or the message is
    // malformed. So it is written from the body, exactly as HTTP/1.1 writes it,
    // and a caller's is dropped below.
    if let Some(length) = request.declared_length() {
        fields.push(Field::new("content-length", length.to_string()));
    }
    for header in request.headers.iter() {
        let name = header.name.to_ascii_lowercase();
        // `Host` is `:authority` here, and the rest describe a hop that HTTP/2
        // has its own way of saying. Sending one makes the message malformed.
        if ABOUT_THE_HOP.contains(&name.as_str()) || name == "content-length" {
            continue;
        }
        let mut field = Field::new(name, header.value.clone());
        // Anything that is somebody's credential is never put in a table —
        // not ours, and not any relay's. The same rule ADR 0007 applies to
        // cookies, applied to compression.
        if matches!(
            field.name.as_str(),
            "authorization" | "cookie" | "proxy-authorization" | "set-cookie"
        ) {
            field.never_indexed = true;
        }
        fields.push(field);
    }
    fields
}

fn write_all(wire: &mut impl Write, bytes: &[u8]) -> Result<(), Broken> {
    wire.write_all(bytes)
        .and_then(|()| wire.flush())
        .map_err(|why| Broken {
            why: format!("could not write to the connection: {why}"),
            error: ErrorCode::InternalError,
            fatal: true,
        })
}

fn malformed(why: &str) -> Broken {
    Broken {
        why: why.to_owned(),
        error: ErrorCode::ProtocolError,
        fatal: false,
    }
}

/// The starting window, for a caller sizing its own buffers.
pub const AT_FIRST: i64 = flow::AT_FIRST;
