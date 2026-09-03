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

use crate::cache::{self, Answer, Cache};
use crate::connection::{Connection, Exchanged, PATIENCE, Protocol, exchange};
use crate::freshness;
use crate::redirect::{self, Next, Trail};
use crate::request::Request;
use crate::response::Response;
use crate::tls::Trust;
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};

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
    /// What HTTP/2 needs remembered between exchanges — the two HPACK tables
    /// and the stream bookkeeping.
    ///
    /// It belongs to the *connection*, not to the exchange. Losing it between
    /// requests would mean the second request on a connection could not be
    /// decoded at all, which is the same class of mistake as throwing away a
    /// read-ahead buffer (queue item 54) and has the same shape of symptom:
    /// everything works once.
    speaking: Option<Box<crate::h2::client::Speaking>>,
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
    /// Names turned into addresses, remembered briefly. Here rather than
    /// global because a pool is what a session holds, and forgetting every name
    /// is what changing network has to mean.
    resolver: crate::resolve::Resolver,
    /// What has been kept, so a second load of the same thing need not ask.
    ///
    /// The pool owns it because the pool is what a caller holds for the life of
    /// a session, and a cache that did not outlive one request would be a cache
    /// that never hit.
    cache: Cache,
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
            cache: Cache::new(),
            resolver: crate::resolve::Resolver::new(),
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

        if let Some((mut kept, mut speaking)) = self.take(&server) {
            self.reused += 1;
            match speak(&mut kept, &mut speaking, request) {
                Ok(done) => {
                    if done.reusable {
                        self.put(&server, kept, speaking);
                    }
                    return Ok(done.response);
                }
                Err(why) => {
                    // The bet, lost. All three conditions, or it is a failure.
                    if kept.anything_arrived() || !is_safe_to_repeat(&request.method) {
                        return Err(why);
                    }
                    // Fall through and try once on a new connection.
                }
            }
        }

        // Resolve before connecting, so `crate::resolve`'s rebinding rule is
        // applied by us rather than by the standard library, which has no rule.
        let where_to = self
            .resolver
            .resolve(
                &server.host,
                server.port,
                crate::resolve::reach_for(request.initiator.as_ref()),
            )
            .map_err(|why| why.to_string())?;
        let mut fresh =
            Connection::open(&server.host, secure, &self.trust, self.patience, &where_to)?;
        let mut speaking = None;
        let done = speak(&mut fresh, &mut speaking, request)?;
        if done.reusable {
            self.put(&server, fresh, speaking);
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
            let response = self.fetch_perhaps_from_the_cache(&asking)?;
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

    /// One exchange, answered from the cache where the cache can answer it.
    ///
    /// Between [`Pool::follow`] and [`Pool::fetch`] rather than inside either:
    /// `fetch` is one exchange over a socket and stays that way, and a redirect
    /// is cacheable like anything else, so this has to sit on the inside of the
    /// redirect loop rather than around it.
    fn fetch_perhaps_from_the_cache(&mut self, request: &Request) -> Result<Response, String> {
        let sent_at = SystemTime::now();
        let asking = match self.cache.answer(request, sent_at) {
            Answer::Stored(response) => return Ok(*response),
            Answer::Fetch => request.clone(),
            Answer::Revalidate { conditions } => {
                cache::asking_whether_it_changed(request, &conditions)
            }
        };

        let mut response = self.fetch(&asking)?;
        let arrived_at = SystemTime::now();
        // A response with no `Date` is treated as having arrived now. Without
        // this every age calculation on such a response starts at zero, which
        // reads as "brand new" — the optimistic direction to be wrong in.
        cache::dated(&mut response, arrived_at);

        if freshness::is_not_modified(response.status) {
            // The server said what is stored is still good. If nothing is
            // stored, nobody could have asked — so the `304` is unusable and
            // saying so beats handing up an empty body as though it were a page.
            return self
                .cache
                .refresh(request, &response, arrived_at)
                .ok_or_else(|| {
                    "the server said nothing had changed about something we do not have".to_owned()
                });
        }

        // A write makes what is stored a lie, whatever the response says.
        if !matches!(
            request.method.to_ascii_uppercase().as_str(),
            "GET" | "HEAD" | "OPTIONS" | "TRACE"
        ) {
            self.cache.forget(request);
        } else if !cache::nobody_wants_this_kept(request, &response) {
            self.cache.keep(request, &response, sent_at, arrived_at);
        }
        Ok(response)
    }

    /// What is kept, for a caller that wants to look.
    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    /// A kept connection to this server, if there is one worth having.
    fn take(
        &mut self,
        server: &Server,
    ) -> Option<(Connection, Option<Box<crate::h2::client::Speaking>>)> {
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
            return Some((kept.connection, kept.speaking));
        }
        None
    }

    /// Keep a connection, within the bounds.
    fn put(
        &mut self,
        server: &Server,
        connection: Connection,
        speaking: Option<Box<crate::h2::client::Speaking>>,
    ) {
        if self.idle() >= MOST_IN_ALL {
            return;
        }
        let waiting = self.idle.entry(server.clone()).or_default();
        if waiting.len() >= MOST_PER_HOST {
            return;
        }
        waiting.push(Idle {
            connection,
            speaking,
            since: Instant::now(),
        });
    }
}

/// One exchange, in whichever protocol the handshake settled on.
///
/// The choice was made during TLS, before a byte of the request went out, so
/// nothing here can discover it late and have to send the request again.
fn speak(
    connection: &mut Connection,
    speaking: &mut Option<Box<crate::h2::client::Speaking>>,
    request: &Request,
) -> Result<Exchanged, String> {
    match connection.protocol() {
        Protocol::Http11 => exchange(connection, request).map_err(|why| why.to_string()),
        Protocol::Http2 => {
            let held = speaking.get_or_insert_with(|| Box::new(crate::h2::client::Speaking::new()));
            let response = crate::h2::client::exchange(connection, held, request)
                .map_err(|why| why.why.clone())?;
            Ok(Exchanged {
                response,
                // HTTP/2 connections are meant to be kept. A `GOAWAY` is the
                // only thing that says otherwise, and the session remembers it.
                reusable: !held.session.is_going_away(),
            })
        }
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
