/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Making a fetch the browser process decided a page may make (ADR 0032 § 3,
//! queue item 338).
//!
//! [`crate::fetch_decide`] says whether; this says what is sent, hop by hop,
//! and what is kept. One function, [`make`], from a decided [`Fetch`] and the
//! session's [`Network`] to the [`Fetched`] the renderer is sent — every
//! request through the same [`Pool`] every other request uses, so each one
//! is a line in the session's record with the cause the decision assigned.
//!
//! # Each hop, in this order
//!
//! 1. **Its own headers, decided again**: `Origin` where Fetch sends one —
//!    the document's, or `null` once a redirect has gone from one origin to a
//!    third (Fetch's *tainted origin*); `Referer` from this process's copy of
//!    the document's URL under the page's policy, for where this hop goes;
//!    and `Cookie` from the jar under the document's top-level site, only
//!    when the credentials mode lets this hop carry who the person is.
//! 2. **Asked about first**, when the hop is a `cors` request to another
//!    origin that a form could not have sent and the preflight cache, under
//!    the same top-level site, does not already cover it: the `OPTIONS` is
//!    made, and recorded, before the request it asks about, and a refusal
//!    ends the fetch with nothing else sent.
//! 3. **Made**, through [`Pool::hop`] — one exchange, from the cache where the
//!    cache can answer it, and a redirect handed back rather than followed.
//! 4. **Its `Set-Cookie` kept**, when this hop carried credentials: the same
//!    answer decides whether the person's cookies go and whether a server may
//!    set one ([`Jar::keep_what_was_set`]).
//! 5. **Read only if it may be**: once a `cors` fetch has gone to another
//!    origin, every answer on the way must pass the CORS check, a redirect
//!    included, as Fetch's *HTTP fetch* has it.
//! 6. **A redirect**, when it is one: stopped at under `manual` (an opaque
//!    redirect), a failure under `error`, and under `follow` taken to the
//!    next hop by [`redirect::next`] — which drops a body a `303` loses and
//!    the credentials that belong to the origin being left — within
//!    [`redirect::MOST_HOPS`] and never in a circle. Before the next hop is
//!    sent it is decided again: a `same-origin` fetch may not leave, a `cors`
//!    fetch may not be sent to credentials in a URL at another origin, a
//!    secure page's fetch may not be redirected to an insecure one, and the
//!    document's `connect-src` must allow where it goes. Each of those is a
//!    line in the record naming its rule.
//!
//! Then the answer is filtered ([`crate::fetch_filter`]) for what the page
//! may read, knowing whether a redirect was followed and whether any hop left
//! the document's origin.
//!
//! # `connect-src` on a hop
//!
//! The first hop's URL is judged by [`crate::fetch_decide`]; every later one
//! here, against the copy of the document's policies the decision carries
//! ([`Fetch::policies`]). A hop is a request [`redirect::next`] made, which
//! says it was redirected, and CSP judges such a request with each source's
//! path ignored (CSP3, *does url match expression in origin with redirect
//! count*): `connect-src https://a.example/api/` follows `a.example` to
//! `/other` and refuses it to `b.example`.
//!
//! # And what is said
//!
//! A failure crosses to the page with no reason (ADR 0032 § 4). The reason
//! comes back beside it in [`Made::said`], with every cookie a server set
//! that was not kept, for whoever shows the person.

use std::borrow::Cow;
use std::time::SystemTime;

use alo_net::cors::{self, Credentials, Mode};
use alo_net::jar::{How, Jar};
use alo_net::mixed::{self, Verdict};
use alo_net::pool::Pool;
use alo_net::preflight::Preflights;
use alo_net::redirect::{self, Next, Trail};
use alo_net::referrer;
use alo_net::request::Request;
use alo_url::{Opaque, Origin};

use crate::fetch::Fetched;
use crate::fetch_decide::{Fetch, same_origin};
use crate::fetch_filter::{self, Filtered, Route};

/// What a browser process makes a page's fetches with: one of each for the
/// session.
///
/// The pool is every request's — a page's fetch goes over the same
/// connections, the same cache and into the same record as everything else,
/// which is ADR 0032's *over the same stack*. The jar and the preflight cache
/// are partitioned by top-level site inside themselves (ADR 0007), so one of
/// each serves every tab.
pub struct Network {
    /// Connections, the cache, and the session's record.
    pub pool: Pool,
    /// The person's cookies.
    pub jar: Jar,
    /// The answers to `OPTIONS` already asked.
    pub preflights: Preflights,
}

impl Network {
    /// A session's network over `pool`, with no cookies and nothing asked.
    pub fn over(pool: Pool) -> Self {
        Self {
            pool,
            jar: Jar::new(),
            preflights: Preflights::new(),
        }
    }
}

/// What making a fetch came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    /// What the renderer is sent.
    pub fetched: Fetched,
    /// What to tell the person: why it failed, when it did, and each cookie a
    /// server set that was not kept. Never for the page.
    pub said: Vec<String>,
}

/// Where a fetch has got to: the request for the hop about to be sent.
struct Hop {
    /// What to send, before this hop's own headers are decided.
    request: Request,
    /// Whether any hop so far went to an origin other than the document's.
    left_the_origin: bool,
    /// Whether a redirect went from one origin to a third, after which the
    /// page's origin is said as `null` (Fetch's *tainted origin*).
    tainted: bool,
    /// Whether this hop was reached by a redirect.
    redirected: bool,
}

impl Hop {
    /// The decided request, as its first hop.
    fn first(fetch: &Fetch) -> Self {
        Self {
            request: fetch.request.clone(),
            left_the_origin: fetch.is_cross_origin(),
            tainted: false,
            redirected: false,
        }
    }

    /// Whether this hop carries who the person is: always for `include`, for
    /// `same-origin` only while no hop has left the document's origin, never
    /// for `omit`. The same answer says whether a `Set-Cookie` in its answer
    /// is kept.
    fn includes_credentials(&self, fetch: &Fetch) -> bool {
        match fetch.credentials {
            Credentials::Include => true,
            Credentials::SameOrigin => !self.left_the_origin,
            Credentials::Omit => false,
        }
    }

    /// Whether this hop is a CORS request: a `cors` fetch that has gone to
    /// another origin, whose every answer from here on must agree to be read.
    fn is_cors(&self, fetch: &Fetch) -> bool {
        fetch.mode == Mode::Cors && self.left_the_origin
    }

    /// How this hop was reached, as the filter and the CORS check need it.
    fn route(&self) -> Route {
        Route {
            redirected: self.redirected,
            left_the_origin: self.left_the_origin,
            tainted: self.tainted,
        }
    }

    /// This hop's request with its own `Origin`, `Referer` and `Cookie`.
    fn as_sent(&self, fetch: &Fetch, jar: &Jar, now: SystemTime) -> Request {
        let mut request = self.request.clone();
        for name in ["Origin", "Referer", "Cookie"] {
            request.headers.remove(name);
        }
        if self.is_cors(fetch) || !matches!(request.method.as_str(), "GET" | "HEAD") {
            request.headers.add("Origin", self.route().asker(fetch));
        }
        if let Some(referrer) = referrer::for_request(fetch.referrer, &fetch.document, &request.url)
        {
            request.headers.add("Referer", referrer);
        }
        if self.includes_credentials(fetch)
            && let Some(cookies) =
                jar.header_for(&request.url, &fetch.partition, How::Embedded, now)
        {
            request.headers.add("Cookie", cookies);
        }
        request
    }

    /// The hop a redirect leads to, decided before it is sent; or the rule
    /// that refuses it.
    fn then(&self, fetch: &Fetch, next: Request) -> Result<Self, String> {
        let elsewhere = !fetch
            .request
            .initiator
            .as_ref()
            .is_some_and(|asker| same_origin(asker, &next.url));
        if fetch.mode == Mode::SameOrigin && elsewhere {
            return Err(format!(
                "it asked for the same origin only, and was redirected to {}",
                Origin::of(&next.url)
            ));
        }
        if fetch.mode == Mode::Cors
            && (self.left_the_origin || elsewhere)
            && alo_url::includes_credentials(&next.url)
        {
            return Err(
                "it was redirected to a URL with a name and password in it, at another origin"
                    .to_owned(),
            );
        }
        if matches!(mixed::what_to_do(&next), Verdict::Refused { .. }) {
            return Err(
                "a page reached securely may not be redirected to an insecure connection, which \
                 anybody in between could read and change"
                    .to_owned(),
            );
        }
        if let Err(refusal) = fetch.policies.allows(&next, None) {
            return Err(format!("it was redirected, and {refusal}"));
        }
        let from = Origin::of(&self.request.url);
        let to = Origin::of(&next.url);
        let page_is_from = fetch
            .request
            .initiator
            .as_ref()
            .is_some_and(|asker| !asker.is_opaque() && *asker == from);
        Ok(Self {
            request: next,
            left_the_origin: self.left_the_origin || elsewhere,
            tainted: self.tainted || (from != to && !page_is_from),
            redirected: true,
        })
    }
}

/// Make `fetch` through `network`: each hop decided, sent and recorded, and
/// the answer filtered for the page that asked.
pub fn make(fetch: &Fetch, network: &mut Network) -> Made {
    let mut said = Vec::new();
    let filtered = making(fetch, network, &mut said);
    if let Some(why) = &filtered.why {
        said.push(format!(
            "the page's fetch of {} failed: {why}",
            fetch.request.url
        ));
    }
    Made {
        fetched: filtered.fetched,
        said,
    }
}

/// The hops, until one is the answer or something refuses.
fn making(fetch: &Fetch, network: &mut Network, said: &mut Vec<String>) -> Filtered {
    let mut hop = Hop::first(fetch);
    let mut trail = Trail::from(&hop.request.url);
    loop {
        let sending = hop.as_sent(fetch, &network.jar, SystemTime::now());
        if hop.is_cors(fetch)
            && let Err(why) = asked_first(fetch, &sending, hop.tainted, network)
        {
            return fetch_filter::failed(fetch, why);
        }
        let response = match network.pool.hop(&sending, &fetch.partition) {
            Ok(response) => response,
            Err(why) => return fetch_filter::failed(fetch, why),
        };
        if hop.includes_credentials(fetch) {
            for refused in network.jar.keep_what_was_set(
                &response,
                &fetch.partition,
                How::Embedded,
                SystemTime::now(),
            ) {
                said.push(format!(
                    "a cookie from {} was not kept: {refused}",
                    response.url
                ));
            }
        }
        let route = hop.route();
        if hop.is_cors(fetch)
            && let Err(refusal) =
                cors::agreed_to_be_read(&route.asker(fetch), fetch.credentials, &response)
        {
            return fetch_filter::failed(fetch, refusal.to_string());
        }
        if fetch.redirect != redirect::Mode::Follow {
            // `manual` and `error` are the filter's to answer: an opaque
            // redirect, or a failure.
            return fetch_filter::filter(fetch, &response, route);
        }
        let next = match redirect::next(&sending, &response) {
            Ok(Next::Keep) => return fetch_filter::filter(fetch, &response, route),
            Ok(Next::Follow(next)) => *next,
            Err(refusal) => return fetch_filter::failed(fetch, refusal.to_string()),
        };
        if let Err(refusal) = trail.and_then(&next.url) {
            network.pool.refused(&next, refusal.to_string());
            return fetch_filter::failed(fetch, refusal.to_string());
        }
        hop = match hop.then(fetch, next.clone()) {
            Ok(then) => then,
            Err(rule) => {
                network.pool.refused(&next, rule.clone());
                return fetch_filter::failed(fetch, rule);
            }
        };
    }
}

/// Send the `OPTIONS` a CORS hop needs first, when it needs one, and check
/// its answer; or why the request may not be sent.
///
/// The preflight is the page's origin asking — `null` once `tainted`, as the
/// request itself says, and then it is `null` the server must agree to — and
/// carries no cookies whatever the credentials mode, as Fetch's
/// *CORS-preflight fetch* has it. It is never redirected.
fn asked_first(
    fetch: &Fetch,
    sending: &Request,
    tainted: bool,
    network: &mut Network,
) -> Result<(), String> {
    let now = SystemTime::now();
    // Judged as the page said it was: an origin of its own once tainted, which
    // agrees with `null` and with nothing else, and which no answer remembered
    // for the page's real origin covers.
    let judged = if tainted {
        let mut judged = sending.clone();
        judged.initiator = Some(Origin::Opaque(Opaque::new()));
        Cow::Owned(judged)
    } else {
        Cow::Borrowed(sending)
    };
    if !network
        .preflights
        .must_ask(&judged, fetch.credentials, &fetch.partition, now)
    {
        return Ok(());
    }
    let mut asking = cors::asking_first(sending);
    if let Some(origin) = sending.headers.get("Origin") {
        asking.headers.replace("Origin", origin);
    }
    if let Some(referrer) = sending.headers.get("Referer") {
        asking.headers.add("Referer", referrer);
    }
    let answer = network.pool.fetch(&asking)?;
    if !answer.status.is_ok() {
        return Err(format!(
            "it was asked about first, and {} answered {} rather than agreeing",
            Origin::of(&sending.url),
            answer.status
        ));
    }
    network
        .preflights
        .allowed(
            &judged,
            fetch.credentials,
            &fetch.partition,
            &answer,
            SystemTime::now(),
        )
        .map_err(|refusal| refusal.to_string())
}
