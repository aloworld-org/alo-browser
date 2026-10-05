/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `Document`: the document's own element, and making nodes (queue item 249,
//! item 80's members only).
//!
//! - `documentElement`, the document's one element child or `null`.
//! - `createElement(localName)`: an HTML element in no tree, its name
//!   lowercased and refused with `InvalidCharacterError` when no element can
//!   have it. Its id is the next from the parser's own counter (ADR 0003), so
//!   the agent can name it like any other. The `options` argument is not
//!   read: `is` is a custom element's, and custom elements are item 87.
//! - `createTextNode(data)`: a text node in no tree.
//!
//! A node made and never inserted is a tree of its own, kept while a script
//! holds it and released at the collection after it does not (ADR 0017 § 3).
//!
//! **Not here** (ADR 0017 § 8): `body`, `head`, `title`, `createComment`,
//! `createDocumentFragment`, `getElementById`, every query and every live
//! collection. Each is absent until a page or an item needs it.

use alo_js::Escape;
use alo_js::heap::Ref;
use alo_js::object::Objects;
use alo_js::object::native::{Answer, Call};

use super::dom_exception;
use crate::define;
use crate::idl::{self, Brand, Converted};

/// `Document.prototype`'s members.
pub(super) fn furnish(
    objects: &mut Objects,
    prototype: Ref,
    function_prototype: Ref,
) -> Result<(), Escape> {
    define::attribute(
        objects,
        prototype,
        function_prototype,
        "documentElement",
        document_element,
        None,
    )?;
    define::operation(
        objects,
        prototype,
        function_prototype,
        "createElement",
        create_element,
    )?;
    define::operation(
        objects,
        prototype,
        function_prototype,
        "createTextNode",
        create_text_node,
    )
}

/// `get documentElement`.
fn document_element(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::Document, "documentElement")?;
    let document = idl::read(call, this.owner)?;
    let element = document
        .children(this.node)
        .find(|child| document.element(*child).is_some());
    idl::answer_node(call, this.owner, element)
}

/// `createElement(localName)`.
fn create_element(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::Document, "createElement")?;
    idl::needs(call, 1, "createElement")?;
    let name = match idl::only_string(call)? {
        Converted::Ready(name) => name,
        Converted::Asked(asked) => return Ok(asked),
    };
    match idl::change(call, this.owner, |document| document.create_element(&name))? {
        Ok(made) => idl::answer_node(call, this.owner, Some(made)),
        Err(refusal) => Err(dom_exception::thrown(call, this.owner, refusal)),
    }
}

/// `createTextNode(data)`.
fn create_text_node(call: &mut Call<'_>) -> Result<Answer, Escape> {
    let this = idl::this(call, Brand::Document, "createTextNode")?;
    idl::needs(call, 1, "createTextNode")?;
    let data = match idl::only_string(call)? {
        Converted::Ready(data) => data,
        Converted::Asked(asked) => return Ok(asked),
    };
    let made = idl::change(call, this.owner, |document| {
        document.create_text_node(&data)
    })?;
    idl::answer_node(call, this.owner, Some(made))
}
