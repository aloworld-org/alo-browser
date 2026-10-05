/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `queueMicrotask`, which is HTML's rather than the language's.
//!
//! ADR 0016 § 1: the job queue is the engine's, and the embedder supplies
//! `queueMicrotask` by **asking the engine to queue one** — one entry point,
//! the same queue a promise reaction will wait in, and no host hook that runs
//! code. A builtin is handed no engine, so it asks the way it asks for a call:
//! [`Want::Job`]. A callee that is not a function is the `TypeError` HTML's
//! `queueMicrotask` throws, and the engine throws it where the call was made.

use alo_js::object::Property;
use alo_js::object::native::{Answer, Call, Native, Want};
use alo_js::{Engine, Escape, Fault, Value};

/// The name a page calls it by.
const NAME: &str = "queueMicrotask";

/// `queueMicrotask(callback)`.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the signature is `Body`, which every builtin shares"
)]
fn queue_microtask(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if call.step() == 0 {
        return Ok(Answer::want(
            Want::Job {
                callee: call.argument(0),
                arguments: Vec::new(),
            },
            1,
        ));
    }
    Ok(Answer::Value(Value::Undefined))
}

/// Put `queueMicrotask` on the global object: writable, not enumerable,
/// configurable, as Web IDL defines an operation.
///
/// # Errors
///
/// [`Escape::Full`] if the heap cannot hold the function or its name, and
/// [`Escape::Broken`] if the engine cannot define it on its own global object,
/// which is its bug.
pub(super) fn install(engine: &mut Engine) -> Result<(), Escape> {
    let global = engine.global()?;
    let function = engine.function(Native::new(NAME, queue_microtask))?;
    // Held in a scope until the global object holds it: defining a property
    // interns its name, and interning may allocate.
    let name: Vec<u16> = NAME.encode_utf16().collect();
    let scope = engine.objects().heap_mut().open();
    engine.objects().heap_mut().hold(function);
    let defined = engine.objects().define_named(
        global,
        &name,
        Property::data(Value::Object(function), true, false, true),
    );
    engine.objects().heap_mut().close(scope);
    match defined {
        Ok(true) => Ok(()),
        // A fresh global object refusing a configurable property is the
        // engine's bug, not a page's.
        Ok(false) => Err(Escape::fault(Fault::Gone)),
        Err(named) => Err(Escape::named(named, 0)),
    }
}
