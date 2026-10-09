/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `encodeURIComponent`, the first function of the global object
//! (ECMA-262 § 19.2.6.5, queue item 359).
//!
//! alo Sites' analytics script, which every page it publishes carries, writes
//! the page's path into what it reports with it. The function is `ToString`
//! of its argument and then [`uri`]'s `Encode`: this file is the first half
//! and the errors, and that one is the encoding.
//!
//! `ToString` of an object runs its `toString` or `valueOf`, which is the
//! page's script, so that conversion is asked for (ADR 0031) and comes back
//! at [`STRING_CONVERTED`] with the primitive it made. Nothing is kept across
//! it: the argument is all the function reads.

use crate::abrupt::{Escape, Internal};
use crate::convert::{self, Hint, Primitive};
use crate::heap::Ref;
use crate::object::native::{Answer, Call, Want};
use crate::object::{Objects, Refused, Value};
use crate::uri::{self, Unencodable};

/// The step the function comes back at with its argument made a primitive.
const STRING_CONVERTED: u32 = 1;

/// Put `encodeURIComponent` on the global object, writable and configurable
/// and not enumerable, as every function property of it is.
///
/// # Errors
///
/// [`Escape::Full`] for a heap at its ceiling, and a fault for a reference
/// this engine has lost.
pub(crate) fn furnish(objects: &mut Objects, global: Ref, functions: Ref) -> Result<(), Escape> {
    super::method(
        objects,
        global,
        functions,
        "encodeURIComponent",
        encode_uri_component,
    )
}

/// `encodeURIComponent(uriComponent)`.
fn encode_uri_component(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let at = call.at();
    let string = match call.step() {
        0 => {
            let given = call.argument(0);
            let Some(primitive) = Primitive::of(given) else {
                return Ok(Answer::want(
                    Want::Primitive {
                        of: given,
                        hint: Hint::String,
                    },
                    STRING_CONVERTED,
                ));
            };
            primitive
        }
        // A conversion answers a primitive or throws, so anything else here
        // is this engine having resumed the wrong builtin.
        STRING_CONVERTED => {
            Primitive::of(call.answer()?).ok_or(Escape::Broken(Internal::BuiltinIsWrong))?
        }
        _ => return Err(Escape::Broken(Internal::BuiltinIsWrong)),
    };
    // A symbol is the `TypeError` `ToString` gives it.
    let units = convert::to_units(call.seen(), string, at)?;
    let encoded = uri::encode_component(&units).map_err(|why| match why {
        Unencodable::LoneSurrogate { unit, index } => Escape::uri_error(
            format!(
                "encodeURIComponent was given a lone surrogate, U+{unit:04X} at index {index}, \
                 which is no character and so has no UTF-8"
            ),
            at,
        ),
        Unencodable::TooLong { units } => Escape::refused(Refused::StringTooLong { units }, at),
    })?;
    let text = call
        .objects()
        .text(encoded)
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Answer::Value(Value::Text(text)))
}
