/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `navigator.sendBeacon(url, data)` (ADR 0040 § 5, queue item 369).
//!
//! A beacon is an **ask**, as a fetch is ([`crate::fetching`]): it is
//! recorded in the document cell and carried out in the answer to the
//! message whose task made it. Nothing is sent from inside the call, and
//! nothing here could send it — a renderer has no network (ADR 0005). What
//! makes it a beacon is the claim it carries, [`Keepalive::Beacon`]: the
//! browser process may make it after the page has gone, decides it by every
//! rule a fetch is decided by, and records it as a beacon.
//!
//! # Beacon's steps, as this engine takes them
//!
//! 1. Web IDL converts the arguments: `url` with `ToString`; `data`, an
//!    optional `BodyInit?`, is no body when it is `null` or omitted and
//!    otherwise a string. None of `BodyInit`'s object types — `Blob`,
//!    `FormData`, `URLSearchParams`, a buffer source — exists in this engine
//!    yet, so `ToString` is the conversion every other value gets, correctly
//!    and not as a substitute: the item that builds one adds it here.
//! 2. `url` is parsed against the document's base URL — the first
//!    `<base href>`, as a fetch's is ([`crate::navigating::base`]). One that
//!    does not parse, or whose scheme is not `http` or `https`, is a
//!    `TypeError`.
//! 3. A string body is its UTF-8 bytes and brings `Content-Type:
//!    text/plain;charset=UTF-8`, which is CORS-safelisted, so the mode is
//!    `no-cors`. A body whose type is not safelisted, which only the object
//!    types above can have, would make it `cors`.
//! 4. If the body would take the document's keep-alive bytes in flight past
//!    [`crate::fetching::MOST_KEPT_ALIVE`], the answer is `false` and nothing is asked
//!    (§ 4). So it is when the asks waiting to be taken have no room for it
//!    ([`crate::fetching::MOST_ASKED_BYTES`]): a beacon that cannot be
//!    queued is one the page must be told was not.
//! 5. Otherwise the ask is recorded — a `POST`, credentials `include`,
//!    redirect `follow`, the document's referrer policy — and the answer is
//!    `true`. **It never says what became of the request**: Beacon does
//!    not, and ADR 0032 § 4 gives a page no reason for a failure anyway.
//!
//! A `url` and a `data` that are both objects are refused by name after the
//! first's `toString` has run, as every member with two string arguments
//! is ([`Missing::ASecondArgumentBehindACall`], queue item 221).

use alo_js::abrupt::{Internal, Missing};
use alo_js::convert::Primitive;
use alo_js::heap::Ref;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Fault, Value};
use alo_net::cors::{Credentials, Mode};
use alo_net::redirect;
use alo_url::Url;

use crate::document_cell::DocumentCell;
use crate::fetching::{Asked, Keepalive};
use crate::idl::{self, Converted};
use crate::navigating;
use crate::navigator::Navigator;

/// The member's name, for its messages.
const MEMBER: &str = "sendBeacon";

/// Come back here with `url` converted.
const URL_CONVERTED: u32 = 1;
/// Come back here with `data` converted.
const DATA_CONVERTED: u32 = 2;

/// `sendBeacon(url, data)`.
pub(crate) fn send(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call)?;
    idl::needs(call, 1, MEMBER)?;
    let (url, data) = match arguments(call)? {
        Ok(both) => both,
        Err(asked) => return Ok(asked),
    };
    let owner = call
        .host_defined()
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    let url = resolved(call, owner, &url)?;
    let body = data.map(String::into_bytes);
    let held = call
        .seen()
        .embedded::<DocumentCell>(owner)
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    let headers = if body.is_some() {
        vec![(
            "Content-Type".to_owned(),
            "text/plain;charset=UTF-8".to_owned(),
        )]
    } else {
        Vec::new()
    };
    let asked = Asked {
        number: held.fetches.next_number(),
        url,
        method: "POST".to_owned(),
        headers,
        body: body.unwrap_or_default(),
        mode: Mode::NoCors,
        credentials: Credentials::Include,
        redirect: redirect::Mode::Follow,
        referrer: None,
        keepalive: Keepalive::Beacon,
    };
    if !held.fetches.may_keep_alive(asked.body.len()) || !held.fetches.has_room_for(asked.bytes()) {
        return Ok(Answer::Value(Value::Bool(false)));
    }
    call.objects()
        .write_embedded::<DocumentCell, _>(owner, |held, _| held.fetches.ask(asked))
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    Ok(Answer::Value(Value::Bool(true)))
}

/// Web IDL's brand check: `this` must be a `Navigator`.
fn this(call: &Call<'_>) -> Result<(), Escape> {
    let is_one = match call.this() {
        Value::Object(held) => call.seen().embedded::<Navigator>(held).is_some(),
        _ => false,
    };
    if is_one {
        Ok(())
    } else {
        Err(Escape::type_error(
            format!("'{MEMBER}' was used on something that is not a Navigator"),
            call.at(),
        ))
    }
}

/// `data` as nothing — `null`, `undefined` or omitted — or a value to
/// convert.
fn has_data(call: &Call<'_>) -> bool {
    !matches!(call.argument(1), Value::Null | Value::Undefined)
}

/// `url` and `data`, converted, or what to ask for first.
///
/// Step 0 converts what is primitive and asks for the first object's
/// `toString`; step 1 has `url`'s and converts `data`, which must then be
/// primitive; step 2 has `data`'s, `url` having been primitive.
fn arguments(call: &Call<'_>) -> Result<Result<(String, Option<String>), Answer>, Escape> {
    match call.step() {
        0 => {
            let url = match idl::string(call, call.argument(0), URL_CONVERTED)? {
                Converted::Ready(url) => url,
                Converted::Asked(asked) => return Ok(Err(asked)),
            };
            if !has_data(call) {
                return Ok(Ok((url, None)));
            }
            match idl::string(call, call.argument(1), DATA_CONVERTED)? {
                Converted::Ready(data) => Ok(Ok((url, Some(data)))),
                Converted::Asked(asked) => Ok(Err(asked)),
            }
        }
        URL_CONVERTED => {
            let url = idl::answered_string(call)?;
            if !has_data(call) {
                return Ok(Ok((url, None)));
            }
            if Primitive::of(call.argument(1)).is_none() {
                return Err(Escape::NotBuiltYet(Missing::ASecondArgumentBehindACall));
            }
            match idl::string(call, call.argument(1), DATA_CONVERTED)? {
                Converted::Ready(data) => Ok(Ok((url, Some(data)))),
                Converted::Asked(_) => Err(Escape::Broken(Internal::BuiltinIsWrong)),
            }
        }
        DATA_CONVERTED => {
            let data = idl::answered_string(call)?;
            match idl::string(call, call.argument(0), URL_CONVERTED)? {
                Converted::Ready(url) => Ok(Ok((url, Some(data)))),
                // Step 2 is reached only with a primitive `url`.
                Converted::Asked(_) => Err(Escape::Broken(Internal::BuiltinIsWrong)),
            }
        }
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// `url`, parsed against the base URL of the document `owner` holds, if a
/// beacon may be sent to it.
fn resolved(call: &Call<'_>, owner: Ref, url: &str) -> Result<Url, Escape> {
    let held = call
        .seen()
        .embedded::<DocumentCell>(owner)
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    let base = navigating::base(held.document(), held.url());
    let parsed = alo_url::join(&base, url).map_err(|why| {
        Escape::type_error(
            format!("{url:?} is not a URL a beacon can be sent to: {}", why.why),
            call.at(),
        )
    })?;
    if !matches!(parsed.scheme.as_str(), "http" | "https") {
        return Err(Escape::type_error(
            format!(
                "a beacon is sent over HTTP or HTTPS, and {url:?} is a {}: URL",
                parsed.scheme
            ),
            call.at(),
        ));
    }
    Ok(parsed)
}
