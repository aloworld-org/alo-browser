/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `fetch` on the global object (ADR 0032 §§ 1, 2 and 4, queue item 335).
//!
//! A script's `fetch` is an **ask**: it is recorded in the document cell
//! ([`crate::fetching`]) and answers a pending promise. Nothing here reaches
//! a network, and nothing here could — the renderer has none (ADR 0005).
//!
//! # Two functions, so that every failure rejects
//!
//! Fetch never throws: an argument Web IDL refuses, a URL that does not
//! parse, a forbidden method, a getter in `init` that throws — each
//! **rejects** the promise `fetch` answered. So `fetch` itself does one
//! thing: it makes the promise, and asks for a call of the **request steps**
//! — a second native, held by `fetch` and reachable from nowhere else — with
//! [`Want::Catch`]. Whatever the request steps throw is caught and rejects
//! the promise; what they return is the ask's number, and the promise waits
//! under it. What the engine does not catch — a stop, a full heap, a refusal
//! by name of something not built — still ends the run, as it would anywhere
//! else.
//!
//! # The request steps
//!
//! With every argument converted ([`crate::fetch_init`]), Fetch's `Request`
//! constructor, as far as a fetch's request needs it:
//!
//! 1. the input resolves against the document's base URL — the first
//!    `<base href>`, as a link's does ([`crate::navigating::base`]) — and a
//!    URL that does not, or that includes credentials, is a `TypeError`;
//! 2. `data:` and `blob:` are refused by name: Fetch answers both without a
//!    network, so they are the renderer's to answer, and that is not built
//!    (ADR 0032 § 3). Every other scheme is asked for, and the browser
//!    process answers anything but `http` and `https` as a network error;
//! 3. `mode` defaults to `cors`, and `navigate` is a `TypeError`;
//!    `credentials` defaults to `same-origin` and `redirect` to `follow`;
//! 4. `method` defaults to `GET`; one that is not a method, or is `CONNECT`,
//!    `TRACE` or `TRACK`, is a `TypeError`, and a known one is normalised;
//!    `no-cors` allows only `GET`, `HEAD` and `POST`;
//! 5. each header is normalised and checked — a name or value that is not
//!    one is a `TypeError` — and then **dropped, silently, if it is
//!    forbidden**, as the `Headers` guard drops it, or under `no-cors` if a
//!    form could not have sent it. The same lists as the browser process's
//!    (`alo-net`'s `forbidden.rs` and `cors.rs`), so the two cannot
//!    disagree;
//! 6. a body on a `GET` or `HEAD` is a `TypeError`; a string body is its
//!    UTF-8 bytes, and brings `Content-Type: text/plain;charset=UTF-8` when
//!    the page set none.
//!
//! An ask that would take what is waiting to be taken past
//! [`crate::fetching::MOST_ASKED_BYTES`] fails as a network error does.
//!
//! **Absent**: a `Request` as the input (no page can make one), and the
//! `init` members [`crate::fetch_init`] refuses by name.

use alo_js::abrupt::{Internal, Missing};
use alo_js::heap::Ref;
use alo_js::interpret::Engine;
use alo_js::object::native::{Answer, Call, Native, Want};
use alo_js::object::{Property, Value};
use alo_js::{Escape, Fault};
use alo_net::cors::{self, Credentials, Mode};
use alo_net::forbidden;
use alo_net::redirect;
use alo_net::referrer::Policy;
use alo_url::Url;

use crate::delivering;
use crate::document_cell::DocumentCell;
use crate::fetch_init::{
    self, BODY, CREDENTIALS, HEADERS, INPUT, METHOD, MODE, REDIRECT, REFERRER_POLICY, Reading,
};
use crate::fetching::Asked;
use crate::navigating;

/// The one message every fetch that failed rejects with, whatever happened
/// (ADR 0032 § 4): the page is not told why, and the person is.
pub const FAILED: &str = "the fetch failed, and a page is not told why";

/// `fetch`, keeping the promise it answers.
const FETCH: Native = Native::new("fetch", fetch).keeping(1);
/// The request steps, keeping every converted argument.
const REQUEST: Native = Native::new("fetch", request).keeping(fetch_init::SLOTS);

/// `fetch`: where it keeps its promise.
const PROMISE: usize = 0;
/// `fetch`: come back here once the request steps have answered or thrown.
const REQUESTED: u32 = 1;
/// `fetch`: come back here once the promise is rejected.
const REJECTED: u32 = 2;

/// Put `fetch` on `engine`'s global object, and make the function the
/// renderer delivers each answer with ([`crate::delivering`]).
///
/// Called after [`crate::install`], whose document cell holds the
/// interfaces a `Response` is made with.
///
/// **A safepoint.** `cell` must be rooted by the caller.
///
/// # Errors
///
/// [`Escape::Full`] when the heap cannot hold them; a fault when `cell` is not
/// a document cell; a `TypeError` when the global object already has a
/// `fetch` — an embedder offering twice.
pub fn offer(engine: &mut Engine, cell: Ref) -> Result<(), Escape> {
    if engine.objects().embedded::<DocumentCell>(cell).is_none() {
        return Err(Escape::fault(Fault::NotAnObject));
    }
    let global = engine.global()?;
    let name: Vec<u16> = "fetch".encode_utf16().collect();
    if let Some(key) = engine.objects().existing_key(&name)
        && engine.objects().own_property(global, key)?.is_some()
    {
        return Err(Escape::type_error("this realm already has a fetch", 0));
    }
    let settle = engine.function(delivering::SETTLE)?;
    engine
        .objects()
        .write_embedded::<DocumentCell, _>(cell, |held, barrier| {
            held.fetches.set_settle(barrier, settle);
        })
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    let request = engine.function(REQUEST)?;
    let (intrinsics, objects) = engine.intrinsics();
    let function_prototype = intrinsics.function_prototype(objects)?;
    let objects = engine.objects();
    let scope = objects.heap_mut().open();
    objects.heap_mut().hold(request);
    let made = objects
        .native_holding(FETCH, Some(function_prototype), Value::Object(request))
        .map_err(|why| Escape::refused(why, 0))
        .and_then(|fetch| {
            objects.heap_mut().hold(fetch);
            let property = Property::data(Value::Object(fetch), true, true, true);
            objects
                .define_named(global, &name, property)
                .map_err(|named| Escape::named(named, 0))
        });
    objects.heap_mut().close(scope);
    if made? {
        Ok(())
    } else {
        // A fresh global object refusing a new property is a reference to
        // something else: this crate's bug, not a page's.
        Err(Escape::fault(Fault::NotAnObject))
    }
}

/// `fetch(input, init)`.
fn fetch(call: &mut Call<'_>) -> Result<Answer, Escape> {
    match call.step() {
        0 => {
            let at = call.at();
            let prototype = call.intrinsics()?.promise_prototype(call.seen())?;
            let promise = call
                .objects()
                .promise(Some(prototype))
                .map_err(|why| Escape::refused(why, at))?;
            call.keep(PROMISE, Value::Object(promise))?;
            Ok(Answer::want(
                Want::Catch {
                    callee: call.held(),
                    receiver: Value::Undefined,
                    arguments: vec![call.argument(0), call.argument(1)],
                },
                REQUESTED,
            ))
        }
        REQUESTED if call.threw() => Ok(Answer::want(
            Want::Settle {
                promise: call.kept(PROMISE)?,
                fulfilled: false,
                value: call.answer()?,
            },
            REJECTED,
        )),
        REQUESTED => {
            let Value::Object(promise) = call.kept(PROMISE)? else {
                return Err(Escape::Broken(Internal::BuiltinIsWrong));
            };
            let number = delivering::number(call.answer()?)
                .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
            let owner = call
                .host_defined()
                .ok_or(Escape::fault(Fault::NotAnObject))?;
            call.objects()
                .write_embedded::<DocumentCell, _>(owner, |held, barrier| {
                    held.fetches.wait(barrier, number, promise);
                })
                .ok_or(Escape::fault(Fault::NotAnObject))?;
            Ok(Answer::Value(Value::Object(promise)))
        }
        REJECTED => Ok(Answer::Value(call.kept(PROMISE)?)),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// The request steps: convert the arguments, then make and record the ask,
/// answering its number.
fn request(call: &mut Call<'_>) -> Result<Answer, Escape> {
    match fetch_init::read(call)? {
        Reading::Asked(answer) => Ok(answer),
        Reading::Done => {
            let number = record(call)?;
            // Numbers are counted from nothing, one per ask: far inside the
            // integers an `f64` holds exactly.
            #[expect(
                clippy::cast_precision_loss,
                reason = "an ask's number is a count, far below 2⁵³"
            )]
            let number = number as f64;
            Ok(Answer::Value(Value::Number(number)))
        }
    }
}

/// A refusal by name of `what`, which this engine has not built.
fn not_built(what: &'static str) -> Escape {
    Escape::NotBuiltYet(Missing::InTheEmbedder(what))
}

/// A `TypeError` saying `why`, where the call was.
fn refused(call: &Call<'_>, why: impl Into<String>) -> Escape {
    Escape::type_error(why.into(), call.at())
}

/// The string kept in slot `which`, or [`None`] for nothing kept.
fn kept_text(call: &Call<'_>, which: usize) -> Result<Option<String>, Escape> {
    match call.kept(which)? {
        Value::Undefined => Ok(None),
        Value::Text(held) => call
            .seen()
            .units(held)
            .map(|units| Some(String::from_utf16_lossy(units)))
            .ok_or(Escape::Broken(Internal::BuiltinIsWrong)),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// The page's wishes, from the converted members and their defaults.
struct Wishes {
    mode: Mode,
    credentials: Credentials,
    redirect: redirect::Mode,
    referrer: Option<Policy>,
}

/// Fetch's steps from the converted arguments to the ask, recorded in the
/// document cell: its number.
fn record(call: &mut Call<'_>) -> Result<u64, Escape> {
    let owner = call
        .host_defined()
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    let url = resolved(call, owner)?;
    let wishes = wishes(call)?;
    let method = method(call, wishes.mode)?;
    let body = kept_text(call, BODY)?;
    if body.is_some() && matches!(method.as_str(), "GET" | "HEAD") {
        return Err(refused(call, format!("a {method} cannot have a body")));
    }
    let headers = headers(call, wishes.mode, body.is_some())?;
    let fetches = &held_cell(call, owner)?.fetches;
    let asked = Asked {
        number: fetches.next_number(),
        url,
        method,
        headers,
        body: body.map(String::into_bytes).unwrap_or_default(),
        mode: wishes.mode,
        credentials: wishes.credentials,
        redirect: wishes.redirect,
        referrer: wishes.referrer,
    };
    if !fetches.has_room_for(asked.bytes()) {
        return Err(refused(call, FAILED));
    }
    let number = asked.number;
    call.objects()
        .write_embedded::<DocumentCell, _>(owner, |held, _| held.fetches.ask(asked))
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    Ok(number)
}

/// The input, resolved against the base URL of the document `owner` holds,
/// if a fetch may ask for it.
fn resolved(call: &Call<'_>, owner: Ref) -> Result<Url, Escape> {
    let input = kept_text(call, INPUT)?.ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    let held = held_cell(call, owner)?;
    let base = navigating::base(held.document(), held.url());
    let url = alo_url::join(&base, &input).map_err(|why| {
        refused(
            call,
            format!("{input:?} is not a URL a fetch can ask for: {}", why.why),
        )
    })?;
    if alo_url::includes_credentials(&url) {
        return Err(refused(
            call,
            format!("{input:?} names a user or a password, which a fetch may not"),
        ));
    }
    match url.scheme.as_str() {
        "data" => Err(not_built(
            "a fetch of a data: URL is not built (ADR 0032 § 3)",
        )),
        "blob" => Err(not_built(
            "a fetch of a blob: URL is not built (ADR 0032 § 3)",
        )),
        _ => Ok(url),
    }
}

/// `mode`, `credentials`, `redirect` and `referrerPolicy`, or their
/// defaults.
fn wishes(call: &Call<'_>) -> Result<Wishes, Escape> {
    let mode = match kept_text(call, MODE)?.as_deref() {
        None | Some("cors") => Mode::Cors,
        Some("no-cors") => Mode::NoCors,
        Some("same-origin") => Mode::SameOrigin,
        Some("navigate") => {
            return Err(refused(call, "a fetch cannot be made in navigate mode"));
        }
        Some(_) => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    let credentials = match kept_text(call, CREDENTIALS)?.as_deref() {
        None | Some("same-origin") => Credentials::SameOrigin,
        Some("omit") => Credentials::Omit,
        Some("include") => Credentials::Include,
        Some(_) => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    let redirect = match kept_text(call, REDIRECT)?.as_deref() {
        None | Some("follow") => redirect::Mode::Follow,
        Some("error") => redirect::Mode::Error,
        Some("manual") => redirect::Mode::Manual,
        Some(_) => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    // `""` is no policy, and leaves the document's.
    let referrer = kept_text(call, REFERRER_POLICY)?
        .as_deref()
        .and_then(Policy::named);
    Ok(Wishes {
        mode,
        credentials,
        redirect,
        referrer,
    })
}

/// `method`, checked and normalised, or `GET`.
fn method(call: &Call<'_>, mode: Mode) -> Result<String, Escape> {
    let method = match kept_text(call, METHOD)? {
        None => "GET".to_owned(),
        Some(method) => {
            if !forbidden::is_a_method(&method) {
                return Err(refused(call, format!("{method:?} is not a method")));
            }
            if forbidden::is_forbidden_method(&method) {
                return Err(refused(
                    call,
                    format!("{method} is a method no page may use"),
                ));
            }
            forbidden::normalised_method(&method)
        }
    };
    if mode == Mode::NoCors && !matches!(method.as_str(), "GET" | "HEAD" | "POST") {
        return Err(refused(
            call,
            format!("a no-cors fetch can only GET, HEAD or POST, not {method}"),
        ));
    }
    Ok(method)
}

/// The headers, normalised and checked, with what the guard drops dropped,
/// and the `Content-Type` a string body brings when the page set none.
fn headers(call: &Call<'_>, mode: Mode, has_body: bool) -> Result<Vec<(String, String)>, Escape> {
    let encoded = kept_text(call, HEADERS)?.unwrap_or_default();
    let given = fetch_init::decode(&encoded).ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    let mut headers = Vec::new();
    for (name, value) in given {
        let value = value.trim_matches(['\t', '\n', '\r', ' ']).to_owned();
        if !forbidden::is_a_header_name(&name) || !forbidden::is_a_header_value(&value) {
            return Err(refused(
                call,
                format!("{name:?}: {value:?} is not a header"),
            ));
        }
        if !value.is_ascii() {
            return Err(not_built(
                "a fetch header value with a byte past 0x7F is not built: its bytes are held \
                 as text",
            ));
        }
        let dropped = forbidden::is_forbidden_request_header(&name, &value)
            || (mode == Mode::NoCors
                && !cors::a_form_could_have_sent(&name.to_ascii_lowercase(), &value));
        if !dropped {
            headers.push((name, value));
        }
    }
    if has_body
        && !headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
    {
        headers.push((
            "Content-Type".to_owned(),
            "text/plain;charset=UTF-8".to_owned(),
        ));
    }
    Ok(headers)
}

/// The document cell `owner` names.
fn held_cell<'a>(call: &'a Call<'_>, owner: Ref) -> Result<&'a DocumentCell, Escape> {
    call.seen()
        .embedded::<DocumentCell>(owner)
        .ok_or(Escape::fault(Fault::NotAnObject))
}
