//! Connections, kept and handed out again.
//!
//! Opening a socket costs a round trip; opening a TLS one costs two or three.
//! A page asks for thirty things from the same host, so a browser that opened
//! thirty connections would spend most of a page load saying hello.
//!
//! # The race this file exists to get right
//!
//! **A kept connection can be closed by the server at any moment**, and there
//! is no way to be told. So every reuse is a bet, and the bet is sometimes
//! lost — which is fine, as long as losing it is a *retry* and not a failure.
//!
//! The rule, and it is narrow on purpose: a request is tried again only when
//!
//! 1. the connection was **reused** rather than freshly opened,
//! 2. **not one byte** of an answer arrived, and
//! 3. the method is one where doing it twice is the same as doing it once.
//!
//! Miss any of the three and repeating is guessing. A `POST` that failed after
//! the server received it is a payment that has happened; sending it again is
//! a payment that has happened twice. That is why the third condition is about
//! the *method* and not about how likely it seems.
//!
//! # Bounds, because a pool with none is a file-descriptor leak
//!
//! A cap per host, a cap overall, and an age past which an idle connection is
//! closed rather than gambled on.

use crate::connection::{Connection, PATIENCE, exchange};
use crate::redirect::{self, Next, Trail};
use crate::request::Request;
use crate::response::Response;
use crate::tls::Trust;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How many idle connections to keep to one host.
///
/// Six, which is what browsers settled on for HTTP/1.1 — enough to load a page
/// in parallel, few enough that a hundred tabs do not exhaust a server.
const MOST_PER_HOST: usize = 6;

/// How many idle connections to keep in total.
const MOST_IN_ALL: usize = 64;

/// How long an idle connection is worth gambling on.
///
/// Past this it is closed rather than reused. Servers commonly close at five
/// seconds; keeping one for minutes means losing the bet nearly every time,
/// and every lost bet costs a round trip more than opening one would have.
const KEEP_IDLE_FOR: Duration = Duration::from_secs(20);

/// Which server a connection goes to.
///
/// The **scheme is part of it**: an `https` connection to a host is not an
/// `http` connection to the same host, and handing one out for the other would
/// send a page's cookies in the clear.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Server {
    scheme: String,
    host: String,
    port: u16,
}

struct Idle {
    connection: Connection,
    since: Instant,
}

/// Connections, kept between requests.
pub struct Pool {
    idle: HashMap<Server, Vec<Idle>>,
    trust: Trust,
    /// How long to wait for a server that has stopped answering.
    patience: Duration,
    /// How many requests have been served on a connection that was already
    /// open. For a test, and for anybody wondering whether the pool is doing
    /// anything.
    reused: usize,
}

impl Pool {
    /// A pool that trusts what this machine trusts.
    ///
    /// # Errors
    ///
    /// A sentence, when the certificate store cannot be read.
    pub fn from_this_machine() -> Result<Self, String> {
        Ok(Self::with_trust(Trust::from_this_machine()?))
    }

    /// A pool that trusts exactly this.
    pub fn with_trust(trust: Trust) -> Self {
        Self {
            idle: HashMap::new(),
            trust,
            patience: PATIENCE,
            reused: 0,
        }
    }

    /// The same pool, waiting this long on a server that has gone quiet.
    ///
    /// A browser wants tens of seconds; a test wants a fraction of one, and a
    /// test that waited the browser's timeout would make the whole suite wait
    /// with it.
    #[must_use]
    pub fn patient_for(mut self, patience: Duration) -> Self {
        self.patience = patience;
        self
    }

    /// How many requests have been served on an already-open connection.
    pub fn reused(&self) -> usize {
        self.reused
    }

    /// How many connections are being kept.
    pub fn idle(&self) -> usize {
        self.idle.values().map(Vec::len).sum()
    }

    /// Fetch one thing, over a kept connection where there is one.
    ///
    /// # Errors
    ///
    /// A sentence, for anything from "there is no such host" to "the
    /// certificate was refused".
    pub fn fetch(&mut self, request: &Request) -> Result<Response, String> {
        let server = server_of(request)?;
        let secure = server.scheme == "https";

        if let Some(mut kept) = self.take(&server) {
            self.reused += 1;
            match exchange(&mut kept, request) {
                Ok(done) => {
                    if done.reusable {
                        self.put(&server, kept);
                    }
                    return Ok(done.response);
                }
                Err(why) => {
                    // The bet, lost. All three conditions, or it is a failure.
                    if kept.anything_arrived() || !is_safe_to_repeat(&request.method) {
                        return Err(why.to_string());
                    }
                    // Fall through and try once on a new connection.
                }
            }
        }

        let mut fresh = Connection::open(
            &server.host,
            server.port,
            secure,
            &self.trust,
            self.patience,
        )?;
        let done = exchange(&mut fresh, request).map_err(|why| why.to_string())?;
        if done.reusable {
            self.put(&server, fresh);
        }
        Ok(done.response)
    }

    /// Fetch, following redirects to wherever they end.
    ///
    /// This is what a load is. [`Pool::fetch`] is one exchange and stays that
    /// way, because the cache (item 56) and the same-origin policy (item 61)
    /// both need to see each hop rather than only the last one.
    ///
    /// # Errors
    ///
    /// Whatever [`Pool::fetch`] fails with, and a [`crate::redirect::Refusal`]
    /// in words when the chain points somewhere this engine will not go.
    pub fn follow(&mut self, request: &Request) -> Result<Response, String> {
        let mut trail = Trail::from(&request.url);
        let mut asking = request.clone();
        loop {
            let response = self.fetch(&asking)?;
            match redirect::next(&asking, &response).map_err(|refusal| refusal.to_string())? {
                Next::Keep => return Ok(response),
                Next::Follow(hop) => {
                    trail
                        .and_then(&hop.url)
                        .map_err(|refusal| refusal.to_string())?;
                    asking = *hop;
                }
            }
        }
    }

    /// A kept connection to this server, if there is one worth having.
    fn take(&mut self, server: &Server) -> Option<Connection> {
        let waiting = self.idle.get_mut(server)?;
        while let Some(kept) = waiting.pop() {
            if kept.since.elapsed() > KEEP_IDLE_FOR {
                // Too old to be worth gambling on; dropping it closes it.
                continue;
            }
            if !kept.connection.is_quiet() {
                // The server sent something nobody asked for. Whatever that
                // is, it is not the answer to the next request.
                continue;
            }
            return Some(kept.connection);
        }
        None
    }

    /// Keep a connection, within the bounds.
    fn put(&mut self, server: &Server, connection: Connection) {
        if self.idle() >= MOST_IN_ALL {
            return;
        }
        let waiting = self.idle.entry(server.clone()).or_default();
        if waiting.len() >= MOST_PER_HOST {
            return;
        }
        waiting.push(Idle {
            connection,
            since: Instant::now(),
        });
    }
}

/// Whether doing this twice is the same as doing it once.
///
/// The standard's word is *idempotent*, and the list is the standard's. It is
/// deliberately not "anything that looks read-only": a `POST` that failed after
/// the server received it is a payment that has happened, and sending it again
/// is a payment that has happened twice.
fn is_safe_to_repeat(method: &str) -> bool {
    matches!(
        method,
        "GET" | "HEAD" | "OPTIONS" | "TRACE" | "PUT" | "DELETE"
    )
}

/// Which server a request goes to.
fn server_of(request: &Request) -> Result<Server, String> {
    let url = &request.url;
    let host = url
        .host
        .as_ref()
        .ok_or_else(|| "an http URL with no host".to_owned())?
        .to_string();
    let port = url
        .effective_port()
        .ok_or_else(|| "an http URL with no port".to_owned())?;
    Ok(Server {
        scheme: url.scheme.clone(),
        host,
        port,
    })
}
