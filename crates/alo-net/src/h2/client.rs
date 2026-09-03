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
    begin(wire, speaking)?;
    let stream = speaking.session.open()?;
    let block = hpack::encode(&fields_for(request), &mut speaking.writing);
    write_all(
        wire,
        &frame::write(&Frame::Headers {
            stream,
            block,
            // No body yet: a `POST` with one is queue item 163, and sending
            // `END_STREAM` here is what says truthfully that there is none.
            end_stream: true,
            end_headers: true,
            priority: None,
        }),
    )?;

    let mut headers = Headers::new();
    let mut status = None;
    let mut body = Vec::new();
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
                for field in hpack::decode(&block, &mut speaking.reading)? {
                    read_one(&field, &mut status, &mut headers)?;
                }
                if end_stream {
                    break;
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
                body.extend_from_slice(&data);
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

    let Some(status) = status else {
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
            headers,
            body,
        },
        short,
    })
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

/// One header out of a decoded block.
fn read_one(
    field: &Field,
    status: &mut Option<Status>,
    headers: &mut Headers,
) -> Result<(), Broken> {
    if let Some(name) = field.name.strip_prefix(':') {
        if name != "status" {
            return Err(malformed(&format!(
                "a response carrying :{name}, which only a request may have"
            )));
        }
        if status.is_some() {
            return Err(malformed("a response with two :status headers"));
        }
        *status = Some(Status(field.value.parse::<u16>().map_err(|_| {
            malformed(&format!(
                "a :status of {:?}, which is not a number",
                field.value
            ))
        })?));
        return Ok(());
    }
    // A pseudo-header after an ordinary one is how a message is smuggled past
    // something that only reads the first few headers.
    if status.is_none() {
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
    Ok(())
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
    for header in request.headers.iter() {
        let name = header.name.to_ascii_lowercase();
        // `Host` is `:authority` here, and the rest describe a hop that HTTP/2
        // has its own way of saying. Sending one makes the message malformed.
        if ABOUT_THE_HOP.contains(&name.as_str()) {
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
