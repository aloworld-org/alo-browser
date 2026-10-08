/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `HTMLElement`'s `hidden` (queue item 327): HTML's reflection of the
//! `hidden` attribute, a member of [`super::html_element`]'s interface kept
//! in a file of its own because `click()` fills that one.
//!
//! **Reading** answers `"until-found"` when the attribute is an ASCII
//! case-insensitive match for it, `true` for any other value, and `false`
//! with no attribute.
//!
//! **Writing** takes Web IDL's `(boolean or unrestricted double or
//! DOMString)?`: a boolean and a number as they are, `null` and `undefined`
//! as null, and anything else — an object, through its `toString` — as a
//! string. Then HTML's setter steps: `"until-found"` in any case sets that
//! state; `false`, `""`, null, `0` and `NaN` remove the attribute; anything
//! else sets it to the empty string.
//!
//! What a hidden element *looks like* is the user agent sheet's
//! (`[hidden] { display: none }`, `alo-style`): the until-found state's
//! `content-visibility: hidden` is not built, so such an element is not
//! drawn at all, as a plain hidden one is not.

use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Value};

use crate::define;
use crate::idl::{self, Brand, Converted, This};

/// The keyword, as HTML spells it.
const UNTIL_FOUND: &str = "until-found";

/// Put `hidden` on `HTMLElement.prototype`.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "hidden",
        hidden,
        Some(set_hidden),
    )
}

/// `get hidden`.
fn hidden(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::HtmlElement, "hidden")?;
    let value = idl::read(call, this.owner)?
        .element(this.node)
        .and_then(|element| element.attr("hidden"))
        .map(|value| value.eq_ignore_ascii_case(UNTIL_FOUND));
    match value {
        None => Ok(Answer::Value(Value::Bool(false))),
        Some(false) => Ok(Answer::Value(Value::Bool(true))),
        Some(true) => idl::answer_text(call, Some(UNTIL_FOUND.to_owned())),
    }
}

/// What the setter does to the attribute.
enum Write {
    /// Take it away.
    Remove,
    /// Set it to this.
    Set(&'static str),
}

/// `set hidden`.
fn set_hidden(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::HtmlElement, "hidden")?;
    idl::needs(call, 1, "hidden")?;
    let write = match call.argument(0) {
        Value::Null | Value::Undefined | Value::Bool(false) => Write::Remove,
        Value::Number(number) if number == 0.0 || number.is_nan() => Write::Remove,
        Value::Bool(true) | Value::Number(_) => Write::Set(""),
        _ => match idl::only_string(call)? {
            Converted::Ready(given) if given.eq_ignore_ascii_case(UNTIL_FOUND) => {
                Write::Set(UNTIL_FOUND)
            }
            Converted::Ready(given) if given.is_empty() => Write::Remove,
            Converted::Ready(_) => Write::Set(""),
            Converted::Asked(asked) => return Ok(asked),
        },
    };
    written(call, this, &write)?;
    Ok(Answer::Value(Value::Undefined))
}

/// Do `write` to the element's `hidden` attribute.
fn written(call: &mut Call<'_>, this: This, write: &Write) -> Result<(), Escape> {
    idl::change(call, this.owner, |document| match write {
        Write::Remove => document.remove_attribute(this.node, "hidden").map(drop),
        Write::Set(value) => document.set_attribute(this.node, "hidden", value),
    })?;
    Ok(())
}
