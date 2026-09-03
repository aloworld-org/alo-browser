//! One connection, and one exchange down it.
//!
//! **The buffer belongs to the connection, not to the exchange.** That is the
//! whole reason this is a type rather than a function taking a socket. Reading
//! a response means reading ahead — a reader that pulls a block at a time will
//! have some of the *next* response in its buffer by the time this one's body
//! ends. Throw that reader away between exchanges and the next one starts in
//! the middle of a sentence.
//!
//! Queue item 53 did throw it away, and correctly: there was nothing to reuse a
//! connection for. Item 54 is where that stops being true, and this is the
//! change it needed.
//!
//! # It runs in the browser process
//!
//! ADR 0005: a renderer has no network. This opens sockets, so it is the
//! browser process's, and a renderer receives what came back.

use crate::body::{self, Framing};
use crate::headers::Headers;
use crate::http::{self, Malformed, Version};
use crate::request::Request;
use crate::response::Response;
use crate::tls::{self, Secured, Trust};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// How long to wait for a server that has stopped answering.
///
/// A browser that waits for ever on a socket is one a page can hold a tab
/// hostage with.
pub const PATIENCE: Duration = Duration::from_secs(30);

/// How much is read from the wire at once.
const BLOCK: usize = 16 * 1024;

/// What a connection is actually made of.
enum Wire {
    /// Plain, for `http:`.
    Plain(TcpStream),
    /// With TLS over it, for `https:`.
    Secure(Box<Secured<TcpStream>>),
}

impl Read for Wire {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Wire::Plain(stream) => stream.read(buffer),
            Wire::Secure(stream) => stream.read(buffer),
        }
    }
}

impl Write for Wire {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        match self {
            Wire::Plain(stream) => stream.write(buffer),
            Wire::Secure(stream) => stream.write(buffer),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Wire::Plain(stream) => stream.flush(),
            Wire::Secure(stream) => stream.flush(),
        }
    }
}

/// An open connection to one server, with whatever has been read ahead on it.
pub struct Connection {
    wire: Wire,
    /// Bytes read from the wire and not yet handed out. **This is the field
    /// the whole file is about.**
    spare: Vec<u8>,
    /// How far into `spare` the reading has got.
    at: usize,
    /// Whether anything at all has arrived since [`Connection::begin`].
    ///
    /// A reused connection that fails **before a single byte arrives** was
    /// almost certainly closed by the server while it sat idle, and that is
    /// safe to try again. One that fails after bytes have arrived is not: the
    /// request was received, and repeating it would be guessing.
    arrived: bool,
}

impl Connection {
    /// Open one, with TLS if the scheme asks for it.
    ///
    /// # Errors
    ///
    /// A sentence — including a refused certificate, which keeps everything
    /// [`crate::certificate`] built for telling a person about it.
    pub fn open(
        host: &str,
        port: u16,
        secure: bool,
        trust: &Trust,
        patience: Duration,
    ) -> Result<Self, String> {
        let socket = TcpStream::connect((host, port))
            .map_err(|why| format!("could not reach {host}: {why}"))?;
        socket
            .set_read_timeout(Some(patience))
            .and_then(|()| socket.set_write_timeout(Some(patience)))
            .map_err(|why| format!("could not set a timeout: {why}"))?;

        let wire = if secure {
            // The host in the certificate is the host in the URL, checked
            // here and not later — later is after a password has been sent.
            Wire::Secure(Box::new(
                tls::secure(trust, host, socket).map_err(|why| why.to_string())?,
            ))
        } else {
            Wire::Plain(socket)
        };
        Ok(Self {
            wire,
            spare: Vec::new(),
            at: 0,
            arrived: false,
        })
    }

    /// Start counting again, before an exchange.
    pub fn begin(&mut self) {
        self.arrived = false;
    }

    /// Whether any byte of an answer arrived since [`Connection::begin`].
    pub fn anything_arrived(&self) -> bool {
        self.arrived
    }

    /// Whether anything is already buffered — which a connection taken from a
    /// pool should not have, and which says the server sent something
    /// unasked-for if it does.
    pub fn is_quiet(&self) -> bool {
        self.at >= self.spare.len()
    }
}

impl Read for Connection {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.at >= self.spare.len() {
            self.spare.resize(BLOCK, 0);
            let read = self.wire.read(&mut self.spare)?;
            self.spare.truncate(read);
            self.at = 0;
            if read == 0 {
                return Ok(0);
            }
            self.arrived = true;
        }
        let available = self.spare.get(self.at..).unwrap_or_default();
        let taking = available.len().min(buffer.len());
        let Some(target) = buffer.get_mut(..taking) else {
            return Ok(0);
        };
        let Some(source) = available.get(..taking) else {
            return Ok(0);
        };
        target.copy_from_slice(source);
        self.at += taking;
        Ok(taking)
    }
}

impl Write for Connection {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.wire.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.wire.flush()
    }
}

/// What an exchange produced, and whether the connection survived it.
pub struct Exchanged {
    /// The response.
    pub response: Response,
    /// Whether this connection may carry another request.
    pub reusable: bool,
}

/// Send a request down a connection and read the answer.
///
/// # Errors
///
/// [`Malformed`] when what comes back is not HTTP this engine will read, or
/// when it stops before it said it would.
pub fn exchange(connection: &mut Connection, request: &Request) -> Result<Exchanged, Malformed> {
    connection.begin();
    connection
        .write_all(&http::write_request(request))
        .map_err(|why| Malformed {
            why: format!("could not send the request: {why}"),
        })?;
    connection.flush().map_err(|why| Malformed {
        why: format!("could not send the request: {why}"),
    })?;

    let head = http::read_head(connection)?;
    let framing = Framing::of(head.status, &head.headers)?;
    let body = body::read(connection, framing)?;
    // Last, and after framing on purpose: `Content-Length` counts the bytes on
    // the wire, which are the compressed ones. A body decompressed before it
    // was framed would be a body framed against a length describing something
    // else.
    let applied = crate::decompress::what_was_applied(&head.headers)?;
    let body = crate::decompress::undo(body, &applied)?;

    Ok(Exchanged {
        reusable: may_reuse(head.version, &head.headers, framing),
        response: Response {
            url: request.url.clone(),
            status: head.status,
            headers: head.headers,
            body,
        },
    })
}

/// Whether a connection may carry another request after this response.
///
/// Three ways it may not, and each of them leaves a socket that the next
/// request would hang on if this got it wrong:
///
/// - The response said `Connection: close`.
/// - The body ended because the connection did, so there is nothing left open.
/// - It is HTTP/1.0, where a connection closes unless the response says to
///   keep it.
fn may_reuse(version: Version, headers: &Headers, framing: Framing) -> bool {
    let says = |word: &str| {
        headers.all("Connection").any(|held| {
            held.split(',')
                .any(|part| part.trim().eq_ignore_ascii_case(word))
        })
    };
    if says("close") || framing == Framing::UntilClose {
        return false;
    }
    match version {
        Version::Http11 => true,
        Version::Http10 => says("keep-alive"),
    }
}
