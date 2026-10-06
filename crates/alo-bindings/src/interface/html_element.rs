/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `HTMLElement`: `click()` (ADR 0018 § 6, queue item 261).
//!
//! HTML's `click()`, step by step:
//!
//! 1. a **form control that is disabled** — a `button`, `input`, `select`
//!    or `textarea` that says so, or sits in a disabled `<fieldset>` outside
//!    its first `<legend>` — does nothing;
//! 2. nor does an element whose **click is already in progress**: a second
//!    `click()` from inside the first's listeners;
//! 3. otherwise the flag is set, and a **synthetic pointer event** named
//!    `click` is fired at the element — a `PointerEvent` like an agent's
//!    (ADR 0018 § 5), bubbling, cancelable and composed, every coordinate
//!    `0`, `pointerId` `-1` — but **not trusted**, since a script made it;
//! 4. and the flag is unset.
//!
//! The DOM standard's dispatch runs a click's **activation behaviour** around
//! it, and that is `alo-dom`'s ([`alo_dom::activation`]), run here exactly as
//! the renderer runs it for an agent's `Activate`: *before* (a checkbox
//! turned over, a radio checked) ahead of the listeners, then *cancelled* to
//! put it back if one called `preventDefault`, or *after* — the `input` and
//! `change` a box still in its document fires, **inside this call**, as HTML
//! has them.
//!
//! # Driven from script
//!
//! Every dispatch here is driven the way `dispatchEvent` drives one
//! (`scripted.rs`): the native suspends once per listener, a throw is
//! reported and the next listener runs, and **no microtask runs between
//! listeners**, because the script that called `click()` is still running.
//! The click, `input` and `change` are three dispatches, each with its own
//! base step (`CLICK`, `INPUT`, `CHANGE`), and what they make and
//! must keep across a listener is held by the element's wrapper
//! (`clicking.rs`) — which is also the click in progress flag.
//!
//! # A link is not followed from script yet
//!
//! A click nobody cancelled on a link follows it, which is the page
//! navigating itself: a request for the browser process that no script can
//! make yet. So that one case is refused **by name, after its listeners
//! have run** — they see the click and may cancel it, and a cancelled one
//! needs nothing more — rather than answered as if the page had gone
//! somewhere (queue item 263).
//!
//! # What is not here
//!
//! `HTMLElement`'s other members — `hidden`, `title`, `lang`, `dir`,
//! `innerText`, `focus()`, `blur()`, the dataset — each wait for a page or
//! an item (ADR 0017 § 8), and focus for item 258. There is no `HTMLElement`
//! on the global, as there is no `Element` there.

use alo_dom::activation::{self, Follows};
use alo_dom::{Document, NodeId};
use alo_js::abrupt::{Internal, Missing};
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Fault, Value};

use crate::clicking::Clicking;
use crate::define;
use crate::dispatch;
use crate::embed::{self, Wrapping};
use crate::event::{self, Firing};
use crate::idl::{self, Brand, This};
use crate::interface;
use crate::scripted::{self, Driven, STEPS};
use crate::wrapper::Wrapper;

/// `HTMLElement.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::operation(objects, prototype, function_prototype, "click", click)
}

/// The base step of the click's dispatch.
const CLICK: u32 = 0;

/// The base step of the `input` a toggled box fires.
const INPUT: u32 = STEPS;

/// The base step of the `change` after it.
const CHANGE: u32 = 2 * STEPS;

/// What a link's activation from script is refused as.
const FOLLOWING: &str =
    "a script's click() following a link, which navigates the page, is queue item 263";

/// `click()`.
fn click(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::HtmlElement, "click")?;
    let Value::Object(wrapper) = call.this() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    let (base, driven) = if call.step() == 0 {
        match start(call, this, wrapper)? {
            Some(event) => (CLICK, scripted::drive(call, event, CLICK)?),
            None => return Ok(Answer::Value(Value::Undefined)),
        }
    } else {
        let base = call.step() / STEPS * STEPS;
        let event = held(call, wrapper)?
            .event()
            .ok_or(Escape::fault(Fault::Gone))?;
        (base, scripted::resume(call, event, base)?)
    };
    match driven {
        Driven::Asked(asked) => Ok(asked),
        Driven::Done { canceled } => finished(call, this, wrapper, base, canceled),
    }
}

/// Steps 1 to 3 as far as the dispatch: nothing for a disabled form control
/// or a click in progress, and otherwise the click made and held, the
/// pre-activation run, and the dispatch begun. Answers the click to drive.
fn start(call: &mut Call<'_>, this: This, wrapper: Ref) -> Result<Option<Ref>, Escape> {
    if is_disabled_form_control(idl::read(call, this.owner)?, this.node) {
        return Ok(None);
    }
    if held_by(call.seen(), wrapper).is_some() {
        return Ok(None);
    }
    // The wrapper is `this`, and the document cell is held by it.
    let made = event::create(call.objects(), this.owner, &Firing::CLICK)?;
    // Held before anything else allocates.
    call.objects()
        .write_embedded::<Wrapper, _>(wrapper, |held, barrier| {
            held.begin_click(Clicking::new(barrier, made));
        })
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    let done = idl::change(call, this.owner, |document| {
        activation::before(document, this.node)
    })?;
    call.objects()
        .write_embedded::<Wrapper, _>(wrapper, |held, _| {
            held.clicking_mut().map(|clicking| clicking.activated(done))
        })
        .flatten()
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    // A script cannot have reached an event made a moment ago.
    dispatch::begin(call.objects(), made, wrapper, false)
        .map_err(|_refused| Escape::Broken(Internal::BuiltinIsWrong))?;
    Ok(Some(made))
}

/// One of the three dispatches has ended: run what follows it.
fn finished(
    call: &mut Call<'_>,
    this: This,
    wrapper: Ref,
    base: u32,
    canceled: bool,
) -> Result<Answer, Escape> {
    match base {
        CLICK => {
            let done = held(call, wrapper)?.activation().clone();
            if canceled {
                idl::change(call, this.owner, |document| {
                    activation::cancelled(document, &done);
                })?;
                return end(call, wrapper);
            }
            match activation::after(idl::read(call, this.owner)?, &done) {
                Follows::Nothing => end(call, wrapper),
                Follows::InputAndChange(at) => {
                    let target = wrap(call, this.owner, at)?;
                    fire(call, this, wrapper, target, INPUT)
                }
                Follows::Link { .. } => {
                    end(call, wrapper)?;
                    Err(Escape::NotBuiltYet(Missing::InTheEmbedder(FOLLOWING)))
                }
            }
        }
        INPUT => {
            let target = held(call, wrapper)?
                .target()
                .ok_or(Escape::fault(Fault::Gone))?;
            fire(call, this, wrapper, target, CHANGE)
        }
        CHANGE => end(call, wrapper),
        _ => Err(Escape::Broken(Internal::BuiltinIsWrong)),
    }
}

/// Fire the `input` (at [`INPUT`]) or `change` (at [`CHANGE`]) a toggled
/// box fires at `target`, its wrapper, which the caller holds — by the
/// document's tree, or by the click in progress.
fn fire(
    call: &mut Call<'_>,
    this: This,
    wrapper: Ref,
    target: Ref,
    base: u32,
) -> Result<Answer, Escape> {
    let firing = if base == INPUT {
        Firing::INPUT
    } else {
        Firing::CHANGE
    };
    let made = event::create(call.objects(), this.owner, &firing)?;
    call.objects()
        .write_embedded::<Wrapper, _>(wrapper, |held, barrier| {
            held.clicking_mut()
                .map(|clicking| clicking.firing(barrier, target, made))
        })
        .flatten()
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    dispatch::begin(call.objects(), made, target, false)
        .map_err(|_refused| Escape::Broken(Internal::BuiltinIsWrong))?;
    match scripted::drive(call, made, base)? {
        Driven::Asked(asked) => Ok(asked),
        Driven::Done { canceled } => finished(call, this, wrapper, base, canceled),
    }
}

/// Step 4: the click in progress flag unset, and nothing answered.
fn end(call: &mut Call<'_>, wrapper: Ref) -> Result<Answer, Escape> {
    call.objects()
        .write_embedded::<Wrapper, _>(wrapper, Wrapper::end_click)
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    Ok(Answer::Value(Value::Undefined))
}

/// The click in progress at `wrapper`, which the native began.
fn held<'a>(call: &'a Call<'_>, wrapper: Ref) -> Result<&'a Clicking, Escape> {
    held_by(call.seen(), wrapper).ok_or(Escape::Broken(Internal::BuiltinIsWrong))
}

/// The click in progress at `wrapper`, if there is one.
fn held_by(objects: &Objects, wrapper: Ref) -> Option<&Clicking> {
    objects.embedded::<Wrapper>(wrapper)?.clicking()
}

/// `node`'s one wrapper, made if it has none.
///
/// **A safepoint** when it is new: `node` is in the document, whose tree
/// holds its wrapper from the moment it is made.
fn wrap(call: &mut Call<'_>, owner: Ref, node: NodeId) -> Result<Ref, Escape> {
    let at = call.at();
    let prototype = interface::prototype_of(call.seen(), owner, node);
    embed::wrap(call.objects(), owner, node, prototype).map_err(|why| match why {
        Wrapping::Refused(refused) => Escape::refused(refused, at),
        Wrapping::NotADocument | Wrapping::NoSuchNode(_) => Escape::fault(Fault::Gone),
    })
}

/// HTML's *form control that is disabled*: a `button`, `input`, `select` or
/// `textarea` that is disabled, as `:disabled` says it is.
///
/// Not an `option`, an `optgroup` or a `fieldset`, which HTML calls disabled
/// by rules of their own and `click()` does not ask about.
fn is_disabled_form_control(document: &Document, node: NodeId) -> bool {
    let Some(element) = document.element(node) else {
        return false;
    };
    ["button", "input", "select", "textarea"]
        .iter()
        .any(|name| element.name.is_html(name))
        && alo_css::state::is_disabled(document, node, element)
}

#[cfg(test)]
mod tests {
    use alo_dom::parse_document;

    use super::*;

    fn first(document: &Document, local: &str) -> NodeId {
        document
            .descendants(document.root())
            .find(|node| {
                document
                    .element(*node)
                    .is_some_and(|element| element.name.is_html(local))
            })
            .unwrap()
    }

    #[test]
    fn only_the_four_controls_are_disabled_form_controls() {
        let document = parse_document(
            "<fieldset disabled><legend><input id=a></legend>\
             <button>b</button><select></select><textarea></textarea>\
             <optgroup disabled><option>o</option></optgroup></fieldset>",
        );
        assert!(!is_disabled_form_control(
            &document,
            first(&document, "input")
        ));
        assert!(is_disabled_form_control(
            &document,
            first(&document, "button")
        ));
        assert!(is_disabled_form_control(
            &document,
            first(&document, "select")
        ));
        assert!(is_disabled_form_control(
            &document,
            first(&document, "textarea")
        ));
        assert!(!is_disabled_form_control(
            &document,
            first(&document, "fieldset")
        ));
        assert!(!is_disabled_form_control(
            &document,
            first(&document, "option")
        ));
        assert!(!is_disabled_form_control(
            &document,
            first(&document, "optgroup")
        ));
    }
}
