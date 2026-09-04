/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The same-origin policy, and the one way through it.
//!
//! # What the policy actually is
//!
//! A page may **send** a request almost anywhere. What it may not do is **read
//! the answer**. That distinction is the whole of the same-origin policy and it
//! is the thing most explanations get backwards: an image from another site
//! loads, a form posts to another site, a script runs — and none of those let
//! the page see what came back. Only when the page wants to *read* does the
//! other site have to agree, and agreeing is what CORS is.
//!
//! Getting the distinction wrong in either direction is a bug with a name.
//! Refusing to *send* breaks the web. Allowing a *read* without agreement is
//! how one site reads your bank statement.
//!
//! # The two halves, and why preflight exists
//!
//! A request a page could already have made with a plain HTML form is sent
//! first and checked afterwards — there is nothing to protect, because the form
//! could have done it anyway. Anything else is asked about **first**, with an
//! `OPTIONS`, because a `DELETE` that arrived and was then refused is a
//! `DELETE` that happened.
//!
//! That is the whole reason preflight exists, and it is why the safelist is a
//! list of *what a form can already do* rather than a list of what seems
//! harmless.
//!
//! # What is not here
//!
//! Asking first costs a round trip, and a server may say how long its answer
//! holds. Remembering one is [`crate::preflight`] — a separate file because
//! this one answers *may this be done* and that one answers *have we already
//! asked*, which are different questions with different ways of being wrong.

use crate::headers::Headers;
use crate::media_type::MediaType;
use crate::request::Request;
use crate::response::Response;
use alo_url::Origin;
use core::fmt;

/// What a page is trying to do with a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Read the answer, and ask the other origin's permission if it is one.
    #[default]
    Cors,
    /// Fetch it without reading it — an image, a stylesheet, a script tag.
    ///
    /// What comes back is **opaque**: it can be shown or run, and nothing about
    /// it can be inspected. That is not a lesser kind of CORS; it is the
    /// same-origin policy working, and it is how the web worked before CORS
    /// existed.
    NoCors,
    /// Refuse outright if it is not the same origin.
    SameOrigin,
    /// A person going somewhere, which is not a page reading anything.
    Navigate,
}

/// Whether a request carries who you are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Credentials {
    /// Never.
    Omit,
    /// Only when it is the same origin.
    #[default]
    SameOrigin,
    /// Always — which is what makes the wildcard illegal below.
    Include,
}

/// Why a read was refused.
///
/// Named cases rather than one string, because a person looking at a blocked
/// request needs to know **which** rule stopped it and what the server would
/// have to send. Browsers are notorious for answering that badly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The response said nothing about who may read it.
    NoPermissionGiven {
        /// Who was asking.
        asker: String,
    },
    /// The response named an origin, and it was not this one.
    ForSomebodyElse {
        /// Who was asking.
        asker: String,
        /// Who the server said may read it.
        allowed: String,
    },
    /// The response said "anyone", on a request that carried credentials.
    ///
    /// `*` means "anyone may read this, and it contains nothing personal". A
    /// request carrying cookies contradicts that by existing, so the two
    /// together are refused — otherwise every server that ever wrote `*` for a
    /// public file would be handing over its logged-in pages too.
    AnyoneIsNotEnoughForCredentials {
        /// Who was asking.
        asker: String,
    },
    /// Credentials were sent and the server did not say they were allowed.
    CredentialsNotAllowed {
        /// Who was asking.
        asker: String,
    },
    /// A preflight did not allow the method.
    MethodNotAllowed {
        /// The method.
        method: String,
    },
    /// A preflight did not allow a header.
    HeaderNotAllowed {
        /// The header.
        header: String,
    },
    /// The request was same-origin-only and this was not.
    NotTheSameOrigin {
        /// Who was asking.
        asker: String,
        /// What was asked for.
        target: String,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::NoPermissionGiven { asker } => write!(
                f,
                "this page at {asker} may not read that response: the server did not send an \
                 Access-Control-Allow-Origin header, so it has not agreed to be read by anybody"
            ),
            Refusal::ForSomebodyElse { asker, allowed } => write!(
                f,
                "this page at {asker} may not read that response: the server allowed {allowed} \
                 to read it, and that is a different origin"
            ),
            Refusal::AnyoneIsNotEnoughForCredentials { asker } => write!(
                f,
                "this page at {asker} may not read that response: the request carried credentials, \
                 and a server that answers `*` is saying the response contains nothing personal — \
                 which a request carrying cookies contradicts. The server would have to name this \
                 origin exactly"
            ),
            Refusal::CredentialsNotAllowed { asker } => write!(
                f,
                "this page at {asker} may not read that response: the request carried credentials \
                 and the server did not send Access-Control-Allow-Credentials"
            ),
            Refusal::MethodNotAllowed { method } => write!(
                f,
                "the server was asked in advance whether {method} was allowed and did not say it was"
            ),
            Refusal::HeaderNotAllowed { header } => write!(
                f,
                "the server was asked in advance whether the {header} header was allowed and did \
                 not say it was"
            ),
            Refusal::NotTheSameOrigin { asker, target } => write!(
                f,
                "this request from {asker} to {target} asked for the same origin only"
            ),
        }
    }
}

impl std::error::Error for Refusal {}

/// The request headers a page could already have set with a plain HTML form.
///
/// This is the safelist, and it is a list of *what a form can already do*
/// rather than of what seems harmless. Anything outside it is asked about
/// first, because a request that arrives and is then refused has already
/// happened.
const A_FORM_COULD_HAVE_SENT: [&str; 5] = [
    "accept",
    "accept-language",
    "content-language",
    "content-type",
    "range",
];

/// The response headers a page may read without the server naming them.
///
/// The same idea from the other side: these are the ones a page could already
/// have learned from an HTML form submission, so keeping them secret would
/// protect nothing.
const ALREADY_VISIBLE: [&str; 7] = [
    "cache-control",
    "content-language",
    "content-length",
    "content-type",
    "expires",
    "last-modified",
    "pragma",
];

/// Headers the **browser** sets and a page never can.
///
/// They are not part of any CORS question, and treating them as author headers
/// gets two things wrong at once: every request carrying a `Cookie` would be
/// preflighted, and the preflight would name `cookie` in
/// `Access-Control-Request-Headers` — telling the server the page had asked for
/// something it cannot ask for, and inviting it to allow something it cannot
/// grant. A test caught both.
const THE_BROWSER_SETS_THESE: [&str; 10] = [
    "cookie",
    "cookie2",
    "host",
    "connection",
    "keep-alive",
    "origin",
    "referer",
    "te",
    "trailer",
    "upgrade",
];

/// The `Content-Type` values a form can produce.
const A_FORM_COULD_HAVE_MEANT: [&str; 3] = [
    "application/x-www-form-urlencoded",
    "multipart/form-data",
    "text/plain",
];

/// Whether these two are the same origin.
pub fn is_same_origin(asker: Option<&Origin>, target: &Response) -> bool {
    let Some(asker) = asker else {
        // Nobody asked, which means the person did. A typed address is not a
        // page reading anything.
        return true;
    };
    // An opaque origin is the same as itself and nothing else — including
    // another opaque one, which is why this cannot be an equality on a string.
    !asker.is_opaque() && *asker == Origin::of(&target.url)
}

/// Whether one header, with this value, is one a plain HTML form could have
/// sent.
///
/// **The value matters and not only the name.** `content-type` is on the
/// safelist, and `Content-Type: application/json` is not on it: a form can
/// produce three media types and that is not one of them. Deciding by name
/// alone reads a JSON post as something a form could already have done, which
/// is the permissive direction to be wrong in.
fn a_form_could_have_sent(name: &str, value: &str) -> bool {
    if !A_FORM_COULD_HAVE_SENT.contains(&name) {
        return false;
    }
    if name != "content-type" {
        return true;
    }
    let kind = MediaType::parse(value)
        .map(|kind| kind.essence())
        .unwrap_or_default();
    A_FORM_COULD_HAVE_MEANT.contains(&kind.as_str())
}

/// The names on this request that a plain HTML form could not have sent.
///
/// Lowercased, sorted and deduplicated. This is the **shape** of a request as
/// far as CORS is concerned, and three things ask for it: whether to preflight
/// at all, what the `OPTIONS` names, and — since queue item 164 — whether a
/// remembered answer covers a later request. One list, because three spellings
/// of it would be a cache answering a question the preflight never asked.
///
/// Headers the browser sets are not here. They are nobody's to ask about: see
/// [`THE_BROWSER_SETS_THESE`].
pub fn names_a_form_could_not_have_sent(request: &Request) -> Vec<String> {
    let mut names: Vec<String> = request
        .headers
        .iter()
        .map(|header| (header.name.to_ascii_lowercase(), &header.value))
        .filter(|(name, value)| {
            !THE_BROWSER_SETS_THESE.contains(&name.as_str()) && !a_form_could_have_sent(name, value)
        })
        .map(|(name, _)| name)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Whether this request has to be asked about before it is sent.
///
/// The rule is not "is it dangerous". It is **could a plain HTML form have done
/// this already** — if it could, there is nothing to protect and asking first
/// would only make the web slower.
pub fn needs_asking_first(request: &Request) -> bool {
    if !matches!(
        request.method.to_ascii_uppercase().as_str(),
        "GET" | "HEAD" | "POST"
    ) {
        return true;
    }
    !names_a_form_could_not_have_sent(request).is_empty()
}

/// The `OPTIONS` request that asks whether the real one is allowed.
///
/// It carries no credentials, ever: the question "may I do this" must not
/// itself be a request that does anything on somebody's behalf.
///
/// Its **cause is the cause of the request it is asking about** (ADR 0012 § 2).
/// An engine-made request is attributed to whatever caused the thing it is
/// about, and this one exists for exactly one other request.
pub fn asking_first(request: &Request) -> Request {
    let mut asking = Request::get(request.url.clone(), request.cause.clone());
    "OPTIONS".clone_into(&mut asking.method);
    asking.purpose = request.purpose.clone();
    asking.initiator.clone_from(&request.initiator);
    asking.headers.add(
        "Access-Control-Request-Method",
        request.method.to_ascii_uppercase(),
    );
    let names = names_a_form_could_not_have_sent(request);
    if !names.is_empty() {
        asking
            .headers
            .add("Access-Control-Request-Headers", names.join(", "));
    }
    if let Some(asker) = &request.initiator {
        asking.headers.add("Origin", asker.to_string());
    }
    asking
}

/// Whether the answer to an `OPTIONS` allows the real request.
///
/// # Errors
///
/// [`Refusal`] naming the method or header the server did not allow.
pub fn asking_first_allowed(
    request: &Request,
    credentials: Credentials,
    answer: &Response,
) -> Result<(), Refusal> {
    may_read(request, credentials, answer)?;

    let method = request.method.to_ascii_uppercase();
    let allowed: Vec<String> = list(&answer.headers, "Access-Control-Allow-Methods");
    // A server may answer `*`, and it means what it says — except for
    // credentials, where the check above has already refused the wildcard.
    let any_method = allowed.iter().any(|one| one == "*");
    if !any_method
        && !allowed.iter().any(|one| one.eq_ignore_ascii_case(&method))
        // These three are always allowed once a preflight has succeeded at all,
        // because they are what a form could have sent.
        && !matches!(method.as_str(), "GET" | "HEAD" | "POST")
    {
        return Err(Refusal::MethodNotAllowed { method });
    }

    let permitted: Vec<String> = list(&answer.headers, "Access-Control-Allow-Headers");
    let any_header = permitted.iter().any(|one| one == "*");
    for name in names_a_form_could_not_have_sent(request) {
        // A wildcard never covers `Authorization`. It is the one header a
        // server has to name, because `*` is written by people who mean "my
        // public API" and a credential is never that.
        if name == "authorization" {
            if !permitted.iter().any(|one| one.eq_ignore_ascii_case(&name)) {
                return Err(Refusal::HeaderNotAllowed { header: name });
            }
            continue;
        }
        if !any_header && !permitted.iter().any(|one| one.eq_ignore_ascii_case(&name)) {
            return Err(Refusal::HeaderNotAllowed { header: name });
        }
    }
    Ok(())
}

/// Whether the page that asked may read this response.
///
/// # Errors
///
/// [`Refusal`], in words that say what the server would have to send.
pub fn may_read(
    request: &Request,
    credentials: Credentials,
    response: &Response,
) -> Result<(), Refusal> {
    if is_same_origin(request.initiator.as_ref(), response) {
        return Ok(());
    }
    let asker = request
        .initiator
        .as_ref()
        .map_or_else(|| "null".to_owned(), ToString::to_string);

    let Some(allowed) = response.headers.get("Access-Control-Allow-Origin") else {
        return Err(Refusal::NoPermissionGiven { asker });
    };
    let allowed = allowed.trim();
    let with_credentials = credentials == Credentials::Include;

    if allowed == "*" {
        if with_credentials {
            return Err(Refusal::AnyoneIsNotEnoughForCredentials { asker });
        }
        return Ok(());
    }
    if allowed != asker {
        return Err(Refusal::ForSomebodyElse {
            asker,
            allowed: allowed.to_owned(),
        });
    }
    if with_credentials
        && !response
            .headers
            .get("Access-Control-Allow-Credentials")
            .is_some_and(|said| said.trim().eq_ignore_ascii_case("true"))
    {
        return Err(Refusal::CredentialsNotAllowed { asker });
    }
    Ok(())
}

/// Which of a response's headers the page may actually see.
///
/// Everything else is there — this engine has it — and is not handed up. That
/// is the other half of the policy: `Set-Cookie` on a cross-origin response is
/// honoured by the browser and invisible to the page, which is what stops a
/// page reading a session token it was never given.
pub fn readable(request: &Request, response: &Response) -> Headers {
    if is_same_origin(request.initiator.as_ref(), response) {
        return response.headers.clone();
    }
    let exposed: Vec<String> = list(&response.headers, "Access-Control-Expose-Headers");
    let anything = exposed.iter().any(|one| one == "*");
    let mut visible = Headers::new();
    for header in response.headers.iter() {
        let name = header.name.to_ascii_lowercase();
        let allowed = ALREADY_VISIBLE.contains(&name.as_str())
            || exposed.iter().any(|one| one.eq_ignore_ascii_case(&name))
            // A wildcard exposes ordinary headers and never `Set-Cookie`,
            // which is not the page's to read under any arrangement.
            || (anything && name != "set-cookie");
        if allowed {
            visible.add(header.name.clone(), header.value.clone());
        }
    }
    visible
}

/// A response a page may hold but not look at.
///
/// This is the same-origin policy working rather than failing: an image from
/// another site draws, a script runs, and neither hands the page anything it
/// could read. Status zero and no headers, so that a page cannot use *whether*
/// something loaded as a way to read it.
pub fn made_opaque(response: &Response) -> Response {
    Response {
        url: response.url.clone(),
        status: crate::response::Status(0),
        headers: Headers::new(),
        body: Vec::new(),
    }
}

/// A comma-separated header, as a list of lowercase entries.
///
/// `pub(crate)` for [`crate::preflight`], which reads the same two headers and
/// must read them the same way: a cache that split `Access-Control-Allow-Methods`
/// differently from the check that allowed the request would remember a
/// permission that was never granted.
pub(crate) fn list(headers: &Headers, name: &str) -> Vec<String> {
    headers
        .all(name)
        .flat_map(|value| value.split(','))
        .map(|one| one.trim().to_ascii_lowercase())
        .filter(|one| !one.is_empty())
        .collect()
}
