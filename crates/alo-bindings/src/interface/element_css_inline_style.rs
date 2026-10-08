/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `ElementCSSInlineStyle` (ADR 0033 § 3, queue item 342): `style`, a mixin
//! on `HTMLElement` and `SVGElement`.
//!
//! `[SameObject, PutForwards=cssText] readonly attribute CSSStyleDeclaration
//! style`:
//!
//! - **Reading** it answers the element's one `CSSStyleDeclaration`, made
//!   the first time and kept by the wrapper ([`crate::style_declaration`]).
//! - **Assigning** to it is Web IDL's `[PutForwards]`: the declaration is
//!   read, and the value is put into its `cssText` with an ordinary
//!   `[[Set]]` — so `el.style = "color: red"` runs `cssText`'s setter, or
//!   whatever a page has put in its place, with the value unconverted.
//!
//! Each interface the mixin is on has **its own** getter and setter, as Web
//! IDL makes a mixin's member once per interface it is included in, so
//! `HTMLElement.prototype`'s refuses an SVG element and the other way round.
//! A MathML element has none: this engine builds no MathML (law 1).

use alo_js::abrupt::Internal;
use alo_js::heap::Ref;
use alo_js::object::access::Set;
use alo_js::object::native::{Answer, Call, Want};
use alo_js::object::{Objects, Value};
use alo_js::{Escape, Fault};

use crate::define;
use crate::idl::{self, Brand};
use crate::style_declaration;

/// `style` on `HTMLElement.prototype`.
pub(super) fn furnish_html(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "style",
        html_style,
        Some(set_html_style),
    )
}

/// `style` on `SVGElement.prototype`.
pub(super) fn furnish_svg(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "style",
        svg_style,
        Some(set_svg_style),
    )
}

/// `HTMLElement`'s `get style`.
fn html_style(call: &mut Call<'_>) -> Result<Answer, Escape> {
    style(call, Brand::HtmlElement)
}

/// `HTMLElement`'s `set style`.
fn set_html_style(call: &mut Call<'_>) -> Result<Answer, Escape> {
    set_style(call, Brand::HtmlElement)
}

/// `SVGElement`'s `get style`.
fn svg_style(call: &mut Call<'_>) -> Result<Answer, Escape> {
    style(call, Brand::SvgElement)
}

/// `SVGElement`'s `set style`.
fn set_svg_style(call: &mut Call<'_>) -> Result<Answer, Escape> {
    set_style(call, Brand::SvgElement)
}

/// The element's one declaration, after `brand`'s check.
fn declaration(call: &mut Call<'_>, brand: Brand) -> Result<Ref, Escape> {
    let this = idl::this(call, brand, "style")?;
    let Value::Object(wrapper) = call.this() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    let at = call.at();
    style_declaration::made(call.objects(), this.owner, wrapper, at)
}

/// `get style`.
fn style(call: &mut Call<'_>, brand: Brand) -> Result<Answer, Escape> {
    let declaration = declaration(call, brand)?;
    Ok(Answer::Value(Value::Object(declaration)))
}

/// `set style`, which `[PutForwards=cssText]` makes `Set(style, "cssText",
/// value)`: a setter found along the declaration's chain is called with the
/// value as given, and anything else is the ordinary store a non-strict
/// `[[Set]]` makes, refused silently.
fn set_style(call: &mut Call<'_>, brand: Brand) -> Result<Answer, Escape> {
    if call.step() != 0 {
        // Step 1: the setter has run, and a setter's answer is not used.
        return Ok(Answer::Value(Value::Undefined));
    }
    let declaration = declaration(call, brand)?;
    let value = call.argument(0);
    let units: Vec<u16> = "cssText".encode_utf16().collect();
    // Interned when `CSSStyleDeclaration.prototype` was furnished.
    let key = call
        .seen()
        .existing_key(&units)
        .ok_or(Escape::fault(Fault::Gone))?;
    let set = call
        .objects()
        .set(declaration, key, value)
        .map_err(Escape::fault)?;
    match set {
        Set::Setter(setter @ Value::Object(_)) => Ok(Answer::want(
            Want::Call {
                callee: setter,
                receiver: Value::Object(declaration),
                arguments: vec![value],
            },
            1,
        )),
        Set::Done | Set::Refused | Set::Setter(_) => Ok(Answer::Value(Value::Undefined)),
    }
}
