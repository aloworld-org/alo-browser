/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `CSSStyleDeclaration` (ADR 0033 §§ 3–6, queue item 342): an element's
//! `style` attribute, as a block of declarations a script reads and edits
//! through `el.style` ([`crate::style_declaration`]).
//!
//! Every member parses the element's `style` attribute now
//! ([`InlineStyle::parse`]), and every change is written back as CSSOM's
//! *update style attribute*: the block serialised and set as the attribute
//! through `alo-dom` ([`InlineStyle::serialize`]), a counted change (ADR
//! 0017 § 5). An edit that changes nothing — the same value and priority
//! already held, a property not there to remove, a name this engine does not
//! act on, a value a style sheet would not keep — writes nothing and counts
//! nothing. What is kept, refused and read back is `alo-css`'s
//! ([`alo_css::inline`]), so this file decides none of it. An attribute the
//! page's policies refuse is read as empty, and a write records its text as
//! the declaration's own (ADR 0034 §§ 1, 3).
//!
//! - `cssText`, read as the block serialised; set, it replaces the whole
//!   attribute with the serialisation of what it parses to — written even
//!   when that is the same, and set to `""` rather than removed when nothing
//!   is left, as CSSOM's update steps do.
//! - `length` and `item(index)`, the declarations as written: `margin: 0`
//!   is one, where other engines count its four sides (ADR 0033 § 5).
//! - `getPropertyValue`, `getPropertyPriority`, `setProperty` and
//!   `removeProperty`, which answers the value it removed.
//! - `parentRule`, always `null`: no inline block has a rule.
//! - For every property this engine acts on, an accessor under each name
//!   CSSOM gives it ([`crate::style_names`]): the getter is
//!   `getPropertyValue` and the setter `setProperty` with no priority. Each
//!   accessor's natives are made around the name's place in
//!   [`ATTRIBUTES`], which is how one body serves every property.
//!
//! A value is `[LegacyNullToEmptyString]` where CSSOM says so — a named
//! setter's, and `setProperty`'s value and priority — so `el.style.color =
//! null` removes the colour. `setProperty` takes three `DOMString`s, and one
//! object among them runs the page's `toString`; a second is refused by
//! name, as `classList.add` refuses one ([`idl::strings`]).
//!
//! **Not here:** the indexed getter, `el.style[0]` (queue item 345). A
//! declaration's indices are the attribute's, live, and an embedder cell
//! answers own properties only from what it stores, which is what `classList`
//! is waiting on too (queue item 328). `item()` answers the same names.
//! `cssFloat`, because `float` is not acted on (law 1).

use std::sync::LazyLock;

use alo_css::{Edit, InlineStyle};
use alo_js::abrupt::Internal;
use alo_js::convert;
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Fault, Value};

use crate::define;
use crate::document_cell::DocumentCell;
use crate::embed;
use crate::idl::{self, Converted, Spelled, This};
use crate::style_declaration::StyleDeclaration;
use crate::style_names::{ATTRIBUTES, Named};
use crate::style_policy;

/// `CSSStyleDeclaration.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "cssText",
        css_text,
        Some(set_css_text),
    )?;
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "length",
        length,
        None,
    )?;
    let operations = [
        ("item", item as fn(&mut Call<'_>) -> _),
        ("getPropertyValue", get_property_value),
        ("getPropertyPriority", get_property_priority),
        ("setProperty", set_property),
        ("removeProperty", remove_property),
    ];
    for (name, body) in operations {
        define::operation(objects, prototype, function_prototype, name, body)?;
    }
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "parentRule",
        parent_rule,
        None,
    )?;
    let named: &'static [Named] = LazyLock::force(&ATTRIBUTES);
    for (at, attribute) in named.iter().enumerate() {
        // The list is a few hundred names; one past `u32` is this crate's bug.
        let at = u32::try_from(at).map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))?;
        define::attribute_holding(
            objects,
            prototype,
            function_prototype,
            attribute.attribute.as_str(),
            (named_get, named_set),
            Value::Number(f64::from(at)),
        )?;
    }
    Ok(())
}

/// Web IDL's brand check: `this` must be a `CSSStyleDeclaration`, or the
/// member's `TypeError`. Answers its element.
fn this(call: &Call<'_>, member: &str) -> Result<This, Escape> {
    let held = match call.this() {
        Value::Object(held) => call
            .seen()
            .embedded::<StyleDeclaration>(held)
            .and_then(StyleDeclaration::element)
            .and_then(|wrapper| embed::node_of(call.seen(), wrapper)),
        _ => None,
    };
    held.map(|(owner, node)| This { owner, node })
        .ok_or_else(|| {
            Escape::type_error(
                format!("'{member}' was used on something that is not a CSSStyleDeclaration"),
                call.at(),
            )
        })
}

/// The block the element's `style` attribute holds now: empty when it has
/// none, and **empty when the page's policies refuse it** — the same answer
/// the renderer's draw gets ([`style_policy::applied`]), so that a write
/// starts from nothing rather than adopting injected text as its own (ADR
/// 0034 § 3).
fn block(call: &Call<'_>, this: This) -> Result<InlineStyle, Escape> {
    let document_cell = call
        .seen()
        .embedded::<DocumentCell>(this.owner)
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    let attribute = document_cell
        .document()
        .element(this.node)
        .filter(|element| style_policy::applied(element, document_cell.policies()).is_ok())
        .and_then(|element| element.attr("style"))
        .unwrap_or_default();
    Ok(InlineStyle::parse(attribute))
}

/// CSSOM's *update style attribute*: `block` serialised and set as the
/// element's `style` attribute, through `alo-dom`, as the text the
/// declaration wrote ([`alo_dom::declared`]).
fn update(call: &mut Call<'_>, this: This, block: &InlineStyle) -> Result<(), Escape> {
    let text = block.serialize();
    idl::change(call, this.owner, |document| {
        document.set_declared_style(this.node, &text)
    })?
    // The brand check found the element a moment ago, and nothing has run
    // since that could have taken it away.
    .ok_or(Escape::Broken(Internal::BuiltinIsWrong))
}

/// `get cssText`.
fn css_text(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = this(call, "cssText")?;
    let text = block(call, this)?.serialize();
    idl::answer_text(call, Some(text))
}

/// `set cssText`: the attribute replaced by what the value parses to.
fn set_css_text(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = this(call, "cssText")?;
    idl::needs(call, 1, "cssText")?;
    let value = match idl::only_string(call)? {
        Converted::Ready(value) => value,
        Converted::Asked(asked) => return Ok(asked),
    };
    update(call, this, &InlineStyle::parse(&value))?;
    Ok(Answer::Value(Value::Undefined))
}

/// `get length`.
fn length(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = this(call, "length")?;
    let count = block(call, this)?.len();
    // An `unsigned long`; a block parsed from a string the engine could make
    // holds far fewer declarations than that.
    let count = u32::try_from(count).map_err(|_| Escape::Broken(Internal::BuiltinIsWrong))?;
    Ok(Answer::Value(Value::Number(f64::from(count))))
}

/// `item(index)`: the name of the declaration at `index`, or `""`.
fn item(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "item")?;
    idl::needs(call, 1, "item")?;
    let index = match idl::only_unsigned_long(call)? {
        Ok(index) => index,
        Err(asked) => return Ok(asked),
    };
    // Read after the conversion, which may have run the page's `valueOf`
    // and changed the attribute.
    let this = this(call, "item")?;
    let block = block(call, this)?;
    let name = usize::try_from(index)
        .ok()
        .and_then(|index| block.item(index))
        .unwrap_or_default()
        .to_owned();
    idl::answer_text(call, Some(name))
}

/// The one `DOMString` argument a property's name is given in.
fn property(call: &Call<'_>, member: &'static str) -> Result<Converted, Escape> {
    idl::needs(call, 1, member)?;
    idl::only_string(call)
}

/// `getPropertyValue(property)`.
fn get_property_value(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "getPropertyValue")?;
    let name = match property(call, "getPropertyValue")? {
        Converted::Ready(name) => name,
        Converted::Asked(asked) => return Ok(asked),
    };
    let this = this(call, "getPropertyValue")?;
    let value = block(call, this)?.value(&name);
    idl::answer_text(call, Some(value))
}

/// `getPropertyPriority(property)`.
fn get_property_priority(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "getPropertyPriority")?;
    let name = match property(call, "getPropertyPriority")? {
        Converted::Ready(name) => name,
        Converted::Asked(asked) => return Ok(asked),
    };
    let this = this(call, "getPropertyPriority")?;
    let priority = block(call, this)?.priority(&name).to_owned();
    idl::answer_text(call, Some(priority))
}

/// `setProperty(property, value, priority)`.
fn set_property(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "setProperty")?;
    idl::needs(call, 2, "setProperty")?;
    let spelled = [
        Spelled::AsGiven,
        Spelled::NullIsEmpty,
        Spelled::NullOrAbsentIsEmpty,
    ];
    let given = match idl::strings(call, &spelled)? {
        Ok(given) => given,
        Err(asked) => return Ok(asked),
    };
    let [name, value, priority] = given.as_slice() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    let this = this(call, "setProperty")?;
    set(call, this, name, value, priority)?;
    Ok(Answer::Value(Value::Undefined))
}

/// Set `name` in the block, and write the attribute back if that changed
/// it.
fn set(
    call: &mut Call<'_>,
    this: This,
    name: &str,
    value: &str,
    priority: &str,
) -> Result<(), Escape> {
    let mut block = block(call, this)?;
    match block.set(name, value, priority) {
        Edit::Changed => update(call, this, &block),
        Edit::Unchanged | Edit::Ignored => Ok(()),
    }
}

/// `removeProperty(property)`: the value it had, and the attribute written
/// back if anything was removed.
fn remove_property(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "removeProperty")?;
    let name = match property(call, "removeProperty")? {
        Converted::Ready(name) => name,
        Converted::Asked(asked) => return Ok(asked),
    };
    let this = this(call, "removeProperty")?;
    let mut block = block(call, this)?;
    let value = block.value(&name);
    match block.remove(&name) {
        Edit::Changed => update(call, this, &block)?,
        Edit::Unchanged | Edit::Ignored => {}
    }
    idl::answer_text(call, Some(value))
}

/// `get parentRule`: `null`, since an inline block belongs to no rule.
fn parent_rule(call: &mut Call<'_>) -> Result<Answer, Escape> {
    this(call, "parentRule")?;
    Ok(Answer::Value(Value::Null))
}

/// The accessor a named attribute's native was made around.
fn named(call: &Call<'_>) -> Result<&'static Named, Escape> {
    let Value::Number(at) = call.held() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    usize::try_from(convert::to_uint32(at))
        .ok()
        .and_then(|at| LazyLock::force(&ATTRIBUTES).get(at))
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))
}

/// A named attribute's getter: `getPropertyValue` of its property.
fn named_get(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let named = named(call)?;
    let this = this(call, &named.attribute)?;
    let value = block(call, this)?.value(named.property);
    idl::answer_text(call, Some(value))
}

/// A named attribute's setter: `setProperty` of its property, with the value
/// given and no priority.
fn named_set(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let named = named(call)?;
    this(call, &named.attribute)?;
    let given = match idl::strings(call, &[Spelled::NullIsEmpty])? {
        Ok(given) => given,
        Err(asked) => return Ok(asked),
    };
    let [value] = given.as_slice() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    let this = this(call, &named.attribute)?;
    set(call, this, named.property, value, "")?;
    Ok(Answer::Value(Value::Undefined))
}
