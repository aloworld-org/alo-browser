/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Navigator` (ADR 0030 § 5, queue item 325): HTML's `NavigatorID` in the
//! Gecko compatibility mode, the one whose `vendor` names no company.
//!
//! Every member is read-only, and each answers what ADR 0030 § 5's table
//! says:
//!
//! - `userAgent` and `platform`: what the browser process told the renderer
//!   ([`crate::navigator::Identity`]).
//! - `appVersion`: HTML's Gecko steps over `userAgent`.
//! - `appCodeName` `"Mozilla"`, `appName` `"Netscape"`, `product` `"Gecko"`
//!   and `vendorSub` `""`: HTML's constants, the same in every browser.
//! - `productSub` `"20100101"` and `vendor` `""`: Gecko's.
//! - `oscpu` `""` and `taintEnabled()` `false`: Gecko's partial interface;
//!   HTML allows `oscpu` to be empty, and nothing has asked for more.
//!
//! **Absent** (ADR 0030, *What this does not decide*): `language` and
//! `languages` (item 128), `pdfViewerEnabled`, `plugins` and `mimeTypes`
//! (item 318), `webdriver` (item 133's question), `onLine`,
//! `cookieEnabled`, `hardwareConcurrency` and every other member, each
//! opened by a page. No interface object on the global either, as
//! [`super`] says of every interface but the two events.

use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Value};

use crate::define;
use crate::navigator::Navigator;

/// `Navigator.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    let read_only = [
        ("appCodeName", app_code_name as fn(&mut Call<'_>) -> _),
        ("appName", app_name),
        ("appVersion", app_version),
        ("platform", platform),
        ("product", product),
        ("productSub", product_sub),
        ("userAgent", user_agent),
        ("vendor", vendor),
        ("vendorSub", vendor_sub),
        ("oscpu", oscpu),
    ];
    for (name, get) in read_only {
        define::attribute(objects, prototype, function_prototype, name, get, None)?;
    }
    define::operation(
        objects,
        prototype,
        function_prototype,
        "taintEnabled",
        taint_enabled,
    )
}

/// Web IDL's brand check: `this` must be a `Navigator`, or the member's
/// `TypeError`. Answers it.
fn this<'c>(call: &'c Call<'_>, member: &'static str) -> Result<&'c Navigator, Escape> {
    match call.this() {
        Value::Object(held) => call.seen().embedded::<Navigator>(held),
        _ => None,
    }
    .ok_or_else(|| {
        Escape::type_error(
            format!("'{member}' was used on something that is not a Navigator"),
            call.at(),
        )
    })
}

/// `units` as a string the page can hold.
fn text(call: &mut Call<'_>, units: Vec<u16>) -> Result<Answer, Escape> {
    let at = call.at();
    let made = call
        .objects()
        .text(units)
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Answer::Value(Value::Text(made)))
}

/// A member that answers the same string for every `Navigator`, after the
/// brand check.
fn constant(call: &mut Call<'_>, member: &'static str, answer: &str) -> Result<Answer, Escape> {
    this(call, member)?;
    text(call, answer.encode_utf16().collect())
}

/// `get appCodeName`.
fn app_code_name(call: &mut Call<'_>) -> Result<Answer, Escape> {
    constant(call, "appCodeName", "Mozilla")
}

/// `get appName`.
fn app_name(call: &mut Call<'_>) -> Result<Answer, Escape> {
    constant(call, "appName", "Netscape")
}

/// `get appVersion`.
fn app_version(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let units = this(call, "appVersion")?.app_version();
    text(call, units)
}

/// `get platform`.
fn platform(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let units = this(call, "platform")?.platform().to_vec();
    text(call, units)
}

/// `get product`.
fn product(call: &mut Call<'_>) -> Result<Answer, Escape> {
    constant(call, "product", "Gecko")
}

/// `get productSub`.
fn product_sub(call: &mut Call<'_>) -> Result<Answer, Escape> {
    constant(call, "productSub", "20100101")
}

/// `get userAgent`.
fn user_agent(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let units = this(call, "userAgent")?.user_agent().to_vec();
    text(call, units)
}

/// `get vendor`.
fn vendor(call: &mut Call<'_>) -> Result<Answer, Escape> {
    constant(call, "vendor", "")
}

/// `get vendorSub`.
fn vendor_sub(call: &mut Call<'_>) -> Result<Answer, Escape> {
    constant(call, "vendorSub", "")
}

/// `get oscpu`.
fn oscpu(call: &mut Call<'_>) -> Result<Answer, Escape> {
    constant(call, "oscpu", "")
}

/// `taintEnabled()`: `false`.
fn taint_enabled(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "taintEnabled")?;
    Ok(Answer::Value(Value::Bool(false)))
}
