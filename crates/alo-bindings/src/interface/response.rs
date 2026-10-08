/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Response` (ADR 0032 § 4, queue item 335): what a fetch is settled with,
//! read-only.
//!
//! - `type`, `url`, `redirected`, `status`, `ok`, `statusText` and
//!   `headers`: what the browser process let through, which for an opaque
//!   response is `0`, `""`, `false` and an empty `Headers`.
//! - `bodyUsed`, and `text()`: the body read once, whole, decoded as UTF-8,
//!   in a promise fulfilled before `text()` answers — the body is already in
//!   this process, so there is nothing to wait for but the job. A second
//!   read is a promise rejected with a `TypeError`, as Fetch's *unusable*
//!   body is.
//!
//! **Absent**: `json()` (it comes with `JSON`, item 73), `arrayBuffer()`,
//! `blob()`, `formData()` and `bytes()`, `body` as a stream, `clone()`, the
//! constructor and its statics — each opened by a page (ADR 0032, *What
//! this does not decide*).

use alo_js::abrupt::Internal;
use alo_js::builtin::error::Family;
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call, Native, Want};
use alo_js::{Escape, Value};

use crate::define;
use crate::response::Response;

/// `text()`, keeping the promise it answers while it is settled.
const TEXT: Native = Native::new("text", text).keeping(1);

/// `text()`: where it keeps its promise.
const PROMISE: usize = 0;
/// `text()`: come back here once the promise is settled.
const SETTLED: u32 = 1;

/// `Response.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    let read_only = [
        ("type", kind as fn(&mut Call<'_>) -> _),
        ("url", url),
        ("redirected", redirected),
        ("status", status),
        ("ok", ok),
        ("statusText", status_text),
        ("headers", headers),
        ("bodyUsed", body_used),
    ];
    for (name, get) in read_only {
        define::attribute(objects, prototype, function_prototype, name, get, None)?;
    }
    define::native_operation(objects, prototype, function_prototype, TEXT)
}

/// Web IDL's brand check: `this` must be a `Response`, or the member's
/// `TypeError`. Answers it.
fn this<'c>(call: &'c Call<'_>, member: &'static str) -> Result<&'c Response, Escape> {
    match call.this() {
        Value::Object(held) => call.seen().embedded::<Response>(held),
        _ => None,
    }
    .ok_or_else(|| {
        Escape::type_error(
            format!("'{member}' was used on something that is not a Response"),
            call.at(),
        )
    })
}

/// `text` as a string the page can hold.
fn string(call: &mut Call<'_>, text: &str) -> Result<Answer, Escape> {
    let at = call.at();
    let made = call
        .objects()
        .text(text.encode_utf16().collect())
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Answer::Value(Value::Text(made)))
}

/// `get type`.
fn kind(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let name = this(call, "type")?.kind().name();
    string(call, name)
}

/// `get url`.
fn url(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let url = this(call, "url")?.url().to_owned();
    string(call, &url)
}

/// `get redirected`.
fn redirected(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Bool(
        this(call, "redirected")?.redirected(),
    )))
}

/// `get status`.
fn status(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Number(f64::from(
        this(call, "status")?.status(),
    ))))
}

/// `get ok`.
fn ok(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Bool(this(call, "ok")?.ok())))
}

/// `get statusText`.
fn status_text(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let text = this(call, "statusText")?.status_text().to_owned();
    string(call, &text)
}

/// `get headers`: the one `Headers`, made with the response.
fn headers(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "headers")?
        .headers()
        .map(|headers| Answer::Value(Value::Object(headers)))
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))
}

/// `get bodyUsed`.
fn body_used(call: &mut Call<'_>) -> Result<Answer, Escape> {
    Ok(Answer::Value(Value::Bool(
        this(call, "bodyUsed")?.body_used(),
    )))
}

/// `text()`: a promise for the body as text, fulfilled — or, for a body
/// already read, rejected — before this answers.
fn text(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if call.step() == SETTLED {
        return Ok(Answer::Value(call.kept(PROMISE)?));
    }
    this(call, "text")?;
    let Value::Object(response) = call.this() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    let at = call.at();
    let intrinsics = call.intrinsics()?;
    let prototype = intrinsics.promise_prototype(call.seen())?;
    let promise = call
        .objects()
        .promise(Some(prototype))
        .map_err(|why| Escape::refused(why, at))?;
    call.keep(PROMISE, Value::Object(promise))?;
    let read = call
        .objects()
        .write_embedded::<Response, _>(response, |held, _| held.read_text())
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    let (fulfilled, value) = if let Some(units) = read {
        let made = call
            .objects()
            .text(units)
            .map_err(|why| Escape::refused(why, at))?;
        (true, Value::Text(made))
    } else {
        let error = intrinsics.error(
            call.objects(),
            Family::TypeError,
            "this response's body has already been read",
            at,
        )?;
        (false, Value::Object(error))
    };
    Ok(Answer::want(
        Want::Settle {
            promise: Value::Object(promise),
            fulfilled,
            value,
        },
        SETTLED,
    ))
}
