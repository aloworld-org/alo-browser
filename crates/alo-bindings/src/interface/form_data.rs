/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `FormData` (ADR 0051 § 7, queue item 472): a form's entries as an object
//! a page holds, opened by the contact form's script, which sends
//! `new URLSearchParams(new FormData(form))` by `fetch`.
//!
//! # The constructor
//!
//! `new FormData(form, submitter)`, as Chrome 154 answers it (measured):
//!
//! - With no `form` it is empty, and `submitter` is not looked at.
//! - A `form` that is not a `<form>` — anything but an HTML element called
//!   `form` — is a `TypeError`, `null` included, since the argument is
//!   optional but not nullable.
//! - A `submitter` that is not an HTML element is a `TypeError`; one that is
//!   not a submit button is a `TypeError` too; and one whose form owner is
//!   not `form` is a `NotFoundError` `DOMException`.
//! - Otherwise its list is HTML's *constructing the entry list* for `form`
//!   and `submitter`, by the function a submission builds its list with
//!   ([`alo_dom::submission::entries`]).
//!
//! HTML fires `formdata` at the form while it constructs that list, and the
//! event is queue item 491. Firing an event nobody hears and not firing it
//! are the same thing, so the constructor answers as it should **unless a
//! `formdata` listener is on the event's path** — the form, its ancestors,
//! and the window — and then it is refused by name rather than leaving that
//! listener unheard. The same list is refused by name when it holds what
//! this engine cannot hand over: a file field's entry is a `File` (ADR 0051
//! § 8), an image button's is where it was clicked, and a field whose value
//! is not read yet has none ([`alo_dom::submission::Unsubmittable`]).
//!
//! # Iterating it
//!
//! `FormData` is iterable over pairs, so Web IDL gives its prototype
//! `entries` and makes `[Symbol.iterator]` **the same function**: each
//! answers a new iterator over the list ([`crate::FormDataIterator`]),
//! whose `next` is [`super::form_data_iterator`]'s.
//!
//! **Absent** until a page needs them: `append`, `delete`, `get`, `getAll`,
//! `has`, `set`, `keys`, `values` and `forEach`. With no `append` or `set` a
//! page cannot change a list once made, so the iterator never meets one
//! that changed.

use alo_dom::submission::{self, Entry, Unsubmittable, Value as Sent};
use alo_dom::{Document, NodeId, kind, owner};
use alo_js::abrupt::{Internal, Missing};
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};
use alo_js::{Escape, Fault, Value};

use super::{Interface, dom_exception};
use crate::define;
use crate::document_cell::DocumentCell;
use crate::embed;
use crate::form_data::FormData;
use crate::form_data_iterator::FormDataIterator;
use crate::listeners;

/// The name a page constructs it by, for its messages.
const NAME: &str = "FormData";

/// The refusal while `formdata` is not fired.
const LISTENED: &str = "a 'formdata' listener would hear this FormData being made, and the \
                        formdata event is queue item 491";

/// The refusal for a file field's entry.
const A_FILE: &str = "the form holds a file field, whose entry is a File, and the File API is \
                      not built (ADR 0051 § 8)";

/// `FormData.prototype`'s member. `[Symbol.iterator]` is put beside it,
/// as the same function, by [`crate::install::furnish`].
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::operation(objects, prototype, function_prototype, "entries", entries)
}

/// `new FormData(form, submitter)`.
pub(crate) fn construct(call: &mut Call<'_>) -> Result<Answer, Escape> {
    if !call.constructing() {
        return Err(Escape::type_error(
            format!("'{NAME}' is a constructor, and needs 'new'"),
            call.at(),
        ));
    }
    let Value::Object(instance) = call.this() else {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    };
    if call.seen().embedded::<FormData>(instance).is_none() {
        return Err(Escape::Broken(Internal::BuiltinIsWrong));
    }
    if call.argument(0) == Value::Undefined {
        return Ok(Answer::Value(Value::Object(instance)));
    }
    let (owner, form) = html_element(call, 0)
        .filter(|(owner, node)| {
            embed::document(call.seen(), *owner)
                .and_then(|document| document.element(*node))
                .is_some_and(|element| element.name.is_html("form"))
        })
        .ok_or_else(|| {
            Escape::type_error(format!("argument 1 to '{NAME}' is not a <form>"), call.at())
        })?;
    let submitter = match call.argument(1) {
        Value::Undefined | Value::Null => None,
        _ => Some(
            html_element(call, 1)
                .filter(|(of, _)| *of == owner)
                .ok_or_else(|| {
                    Escape::type_error(
                        format!(
                            "argument 2 to '{NAME}' is not an HTMLElement of the form's document"
                        ),
                        call.at(),
                    )
                })?
                .1,
        ),
    };
    let document = embed::document(call.seen(), owner).ok_or(Escape::fault(Fault::NotAnObject))?;
    if let Some(button) = submitter {
        if !document.element(button).is_some_and(kind::is_submit_button) {
            return Err(Escape::type_error(
                "the submitter given to 'FormData' is not a submit button",
                call.at(),
            ));
        }
        if owner::form_owner(document, button) != Some(form) {
            return Err(dom_exception::thrown_named(
                call,
                owner,
                "NotFoundError",
                "the submitter given to 'FormData' is not owned by its form",
            ));
        }
    }
    if listened(call.seen(), owner, form) {
        return Err(not_built(LISTENED));
    }
    let list = listed(document, form, submitter)?;
    call.objects()
        .write_embedded::<FormData, _>(instance, |held, _| held.fill(list))
        .ok_or(Escape::Broken(Internal::BuiltinIsWrong))?;
    Ok(Answer::Value(Value::Object(instance)))
}

/// Argument `which` as an HTML element: its document cell and its node.
fn html_element(call: &Call<'_>, which: usize) -> Option<(Ref, NodeId)> {
    let Value::Object(held) = call.argument(which) else {
        return None;
    };
    let (owner, node) = embed::node_of(call.seen(), held)?;
    embed::document(call.seen(), owner)?
        .element(node)
        .filter(|element| element.name.ns == alo_dom::Namespace::Html)
        .map(|_| (owner, node))
}

/// Whether a `formdata` event fired at `form` would reach a listener: one
/// on the form, on an ancestor, or — for a form in its document — on the
/// window.
fn listened(objects: &Objects, owner: Ref, form: NodeId) -> bool {
    let Some(held) = objects.embedded::<DocumentCell>(owner) else {
        return false;
    };
    let kind: Vec<u16> = "formdata".encode_utf16().collect();
    let hears = |target: Option<Ref>| {
        target
            .and_then(|target| listeners::of(objects, target))
            .is_some_and(|list| !list.matching(&kind).is_empty())
    };
    let document = held.document();
    let mut at = Some(form);
    let mut root = form;
    while let Some(node) = at {
        if hears(held.wrapper(node)) {
            return true;
        }
        root = node;
        at = document.parent(node);
    }
    root == document.root() && hears(held.window())
}

/// HTML's *constructing the entry list* for `form` and `submitter`, as
/// text, or refused by name.
fn listed(
    document: &Document,
    form: NodeId,
    submitter: Option<NodeId>,
) -> Result<Vec<(String, String)>, Escape> {
    let entries = submission::entries(document, form, submitter).map_err(|why| match why {
        // Each says itself, and each is a `'static` text in the refusal.
        Unsubmittable::ImageButton => not_built(
            "an image button sends where it was clicked, and an agent clicks nowhere (ADR \
             0051 § 4)",
        ),
        Unsubmittable::Unheld { .. } => not_built(
            "the form holds a field whose value this engine does not read yet (ADR 0051 § 3)",
        ),
        Unsubmittable::Charset { .. }
        | Unsubmittable::Dialog
        | Unsubmittable::AnotherWindow { .. } => Escape::Broken(Internal::BuiltinIsWrong),
    })?;
    entries
        .into_iter()
        .map(|Entry { name, value }| match value {
            Sent::Text(text) => Ok((name, text)),
            Sent::NoFile => Err(not_built(A_FILE)),
        })
        .collect()
}

/// A refusal by name of `what`, which this engine has not built.
fn not_built(what: &'static str) -> Escape {
    Escape::NotBuiltYet(Missing::InTheEmbedder(what))
}

/// `entries()`, and `[Symbol.iterator]()`: a new iterator over the list.
fn entries(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let Value::Object(target) = call.this() else {
        return Err(not_a_form_data(call));
    };
    if call.seen().embedded::<FormData>(target).is_none() {
        return Err(not_a_form_data(call));
    }
    let at = call.at();
    let prototype = call
        .host_defined()
        .and_then(|page| call.seen().embedded::<DocumentCell>(page))
        .and_then(|held| held.interfaces().prototype(Interface::FormDataIterator))
        .ok_or(Escape::fault(Fault::Gone))?;
    // `target` is `this`, on the interpreter's stack, and the prototype is
    // held by the document cell, which the realm roots.
    let iterator = call
        .objects()
        .foreign(Box::new(FormDataIterator::new(prototype, target)))
        .map_err(|why| Escape::refused(why, at))?;
    Ok(Answer::Value(Value::Object(iterator)))
}

/// The `TypeError` for a member used on anything but a `FormData`.
fn not_a_form_data(call: &Call<'_>) -> Escape {
    Escape::type_error(
        "'entries' was used on something that is not a FormData",
        call.at(),
    )
}
