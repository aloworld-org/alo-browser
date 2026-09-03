//! One request down one stream, and the answer back.
//!
//! **One exchange, then closed.** Connection pooling and keep-alive are queue
//! item 54, and doing them later costs nothing here: a pool hands out a stream
//! and this function does not care where the stream came from. Doing them
//! *now* would have meant writing the framing and the reuse together, and the
//! framing is the part where being wrong is a security bug.
//!
//! # It runs in the browser process
//!
//! ADR 0005: a renderer has no network. This opens sockets, so it is the
//! browser process's, and a renderer receives what came back.

use crate::body::{self, Framing};
use crate::http::{self, Malformed};
use crate::request::Request;
use crate::response::Response;
use crate::tls::{self, Trust};
use std::io::{BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// How long to wait for a server that has stopped answering.
///
/// A browser that waits for ever on a socket is one a page can hold a tab
/// hostage with.
const PATIENCE: Duration = Duration::from_secs(30);

/// Send a request down a stream and read the answer.
///
/// The stream is somebody else's to open and to close — which is what will let
/// queue item 54 hand one from a pool without changing this.
///
/// # Errors
///
/// [`Malformed`] when what comes back is not HTTP this engine will read, or
/// when it stops before it said it would.
pub fn exchange(
    stream: &mut (impl Read + Write),
    request: &Request,
) -> Result<Response, Malformed> {
    stream
        .write_all(&http::write_request(request))
        .map_err(|why| Malformed {
            why: format!("could not send the request: {why}"),
        })?;
    stream.flush().map_err(|why| Malformed {
        why: format!("could not send the request: {why}"),
    })?;

    let mut reader = BufReader::new(stream);
    let head = http::read_head(&mut reader)?;
    let framing = Framing::of(head.status, &head.headers)?;
    let body = body::read(&mut reader, framing)?;

    Ok(Response {
        url: request.url.clone(),
        status: head.status,
        headers: head.headers,
        body,
    })
}

/// Fetch one thing over HTTP, opening and closing a socket for it.
///
/// # Errors
///
/// A sentence, for anything from "there is no such host" to "the certificate
/// was refused" — the last of which carries what to tell a person, and loses
/// none of it here: see [`crate::certificate`].
pub fn fetch(request: &Request, trust: &Trust) -> Result<Response, String> {
    let url = &request.url;
    let host = url
        .host
        .as_ref()
        .ok_or_else(|| "an http URL with no host".to_owned())?
        .to_string();
    let port = url
        .effective_port()
        .ok_or_else(|| "an http URL with no port".to_owned())?;
    let secure = url.scheme == "https";

    let socket = TcpStream::connect((host.as_str(), port))
        .map_err(|why| format!("could not reach {host}: {why}"))?;
    socket
        .set_read_timeout(Some(PATIENCE))
        .and_then(|()| socket.set_write_timeout(Some(PATIENCE)))
        .map_err(|why| format!("could not set a timeout: {why}"))?;

    if secure {
        // The host in the certificate is the host in the URL. It is checked
        // here and not later, because later is after a password has been sent.
        let mut secured = tls::secure(trust, &host, socket).map_err(|why| why.to_string())?;
        exchange(&mut secured, request).map_err(|why| why.to_string())
    } else {
        let mut socket = socket;
        exchange(&mut socket, request).map_err(|why| why.to_string())
    }
}
