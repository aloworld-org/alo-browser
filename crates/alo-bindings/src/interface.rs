/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The interfaces a script sees a node as, and the prototype of each.
//!
//! ADR 0017 § 1: **one file per interface**, each holding that interface's
//! prototype and its members — its attributes as accessor properties whose
//! halves are native functions, its operations as native methods. This file
//! is the list of them, the chain each inherits along, and which one a node
//! of each kind is; the members are in the files below it.
//!
//! # The prototypes are the document cell's
//!
//! A native is handed the heap, its `this` and its arguments, and nothing
//! else (ADR 0017 § 4). `createElement` must make a wrapper that inherits
//! from `Element.prototype`, and a refusal must be a `DOMException` that
//! inherits from `DOMException.prototype`, so the prototypes have to be
//! reachable from the one thing every member has: its `this`, a wrapper,
//! whose document cell holds them. They are strong edges of that cell — the
//! page's prototypes live exactly as long as the page's document.
//!
//! # The chain is the standard's, members or not
//!
//! A node is a `Text`, a `Comment`, a `DocumentType` or a `DocumentFragment`
//! as well as an `Element` or a `Document`, and each inherits as the DOM
//! standard says: `Text` from `CharacterData` from `Node`. Interfaces with
//! no member built yet are in the chain with an empty prototype
//! rather than left out, since a chain with a link missing
//! is the approximate answer ADR 0013 § 3 refuses. A member is added to its
//! interface's file when a page or an item needs it (ADR 0017 § 8).
//!
//! # An HTML element is an `HTMLElement`
//!
//! An element in the HTML namespace inherits from `HTMLElement.prototype`,
//! between `Element` and the interface of its own name (queue item 261,
//! ADR 0018 § 6), and `click()` is the one member it has. An element in
//! another namespace — an `<svg>` the parser put in SVG's — is an `Element`
//! and no more until its own interfaces are built. **The interface of each
//! HTML element's own name** — `HTMLInputElement`, `HTMLButtonElement`,
//! `HTMLUnknownElement` and the rest — is not in the chain yet, so an
//! `<input>`'s prototype is `HTMLElement.prototype` itself: a link short,
//! said here rather than hidden, and queue item 262's to add.
//!
//! # Events are interfaces too
//!
//! `EventTarget` is at the top of a node's chain (ADR 0018 § 1), and `Event`
//! and `CustomEvent` are the interfaces of the event objects a script makes
//! and dispatches. `UIEvent`, `MouseEvent` and `PointerEvent` are a click's
//! chain (§ 5, queue item 256), and `InputEvent`, inheriting `UIEvent`, is
//! what an agent's `PutText` fires (queue item 257): only the browser makes
//! one of these, for an agent's verb, so none has an interface object yet.
//! Their prototypes are the document cell's like every other, so the
//! browser's own dispatch (queue item 255) finds them where a node's native
//! finds `Element.prototype`.
//!
//! # `Navigator` is the browser, not a node
//!
//! `Navigator` (ADR 0030, queue item 325) inherits from `Object.prototype`
//! and is the one interface here whose instance is neither a node nor an
//! event: the page's one [`crate::Navigator`], made by
//! [`crate::introduce`].
//!
//! # `DOMTokenList` is an element's, not a node
//!
//! `DOMTokenList` (queue item 327) inherits from `Object.prototype` too: its
//! one instance per element is that element's `classList`
//! ([`crate::token_list`]), holding the element's wrapper.
//!
//! # `NodeList` is a list, not a node
//!
//! `NodeList` (queue item 329) inherits from `Object.prototype`: a static
//! list `querySelectorAll` answers, which `ParentNode` — a mixin on
//! `Document`, `Element` and `DocumentFragment` — makes
//! ([`crate::node_list`]). It is iterable over its indices, so its
//! `forEach`, `keys`, `values`, `entries` and `[Symbol.iterator]` are
//! `Array.prototype`'s own functions (queue item 331,
//! [`Interface::iterates_as_an_array`]).
//!
//! # An unforgeable member is on the instance
//!
//! Web IDL puts a `[LegacyUnforgeable]` attribute on **every instance**
//! rather than on the prototype, not configurable, its getter one function
//! per realm (ADR 0019 § 3). So beside each prototype the document cell
//! holds that interface's **unforgeables** — Web IDL's `[[Unforgeables]]`, an
//! object with no prototype made once in [`crate::install::furnish`] — and
//! [`crate::unforgeable`] copies them onto each instance as it is made. Only
//! `Event` has one: `isTrusted`. Every other interface's slot is empty rather
//! than an empty object.
//!
//! # What is not here
//!
//! **Only `Event` and `CustomEvent` have an interface object on the global
//! object**, because a page cannot make an event any other way
//! ([`crate::install`]). There is no `Node`, `Element`, `EventTarget` or
//! `DOMException` constructor, so `typeof Node` is `"undefined"`,
//! `instanceof` has nothing to name, and their prototypes have no
//! `constructor`. No `Symbol.toStringTag` either. Each is a member of its
//! own, added when something needs it.

pub mod child_node;
pub mod custom_event;
pub mod document;
pub mod dom_exception;
pub mod dom_token_list;
pub mod element;
pub mod event;
pub mod event_target;
pub mod hidden;
pub mod html_element;
pub mod input_event;
pub mod mouse_event;
pub mod navigator;
pub mod node;
pub mod node_list;
pub mod parent_node;
pub mod pointer_event;
pub mod ui_event;

use alo_dom::{Namespace, NodeId, NodeKind};
use alo_js::Escape;
use alo_js::heap::{Barrier, Field, Ref, Tracer};
use alo_js::object::Objects;

use crate::document_cell::DocumentCell;

/// One interface a script can see an object as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interface {
    /// What a listener is added to and an event dispatched at: every node.
    EventTarget,
    /// Every node: its place in the tree, its text and the four changes.
    Node,
    /// Text, comments and processing instructions.
    CharacterData,
    /// A text node.
    Text,
    /// A comment.
    Comment,
    /// A processing instruction, which an HTML document never parses but
    /// `alo-dom` can hold.
    ProcessingInstruction,
    /// An element.
    Element,
    /// An element in the HTML namespace (queue item 261).
    HtmlElement,
    /// The document node.
    Document,
    /// A doctype.
    DocumentType,
    /// A fragment: a `<template>`'s contents.
    DocumentFragment,
    /// The error a refused change throws (ADR 0017 § 5).
    DomException,
    /// An event (ADR 0018 § 8).
    Event,
    /// An event carrying a page's own `detail`.
    CustomEvent,
    /// An event about the user interface (ADR 0018 § 5).
    UiEvent,
    /// An event a mouse — or a keyboard's click — causes.
    MouseEvent,
    /// What a `click` is: an event a pointer, or nothing pointing, causes.
    PointerEvent,
    /// What text going into a field fires (queue item 257).
    InputEvent,
    /// What the browser says it is (ADR 0030, queue item 325).
    Navigator,
    /// An element's `class` as a set of tokens: `classList` (queue item
    /// 327).
    DomTokenList,
    /// A static list of nodes: what `querySelectorAll` answers (queue item
    /// 329).
    NodeList,
}

/// What an interface's prototype inherits from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inherits {
    /// `Object.prototype`.
    Object,
    /// `Error.prototype`, which `DOMException` inherits from.
    Error,
    /// Another interface's prototype.
    Interface(Interface),
}

impl Interface {
    /// Every interface, each after the one it inherits from — the order
    /// their prototypes are made in.
    pub const ALL: [Self; 21] = [
        Self::EventTarget,
        Self::Node,
        Self::CharacterData,
        Self::Text,
        Self::Comment,
        Self::ProcessingInstruction,
        Self::Element,
        Self::Document,
        Self::DocumentType,
        Self::DocumentFragment,
        Self::DomException,
        Self::Event,
        Self::CustomEvent,
        Self::UiEvent,
        Self::MouseEvent,
        Self::PointerEvent,
        Self::HtmlElement,
        Self::InputEvent,
        Self::Navigator,
        Self::DomTokenList,
        Self::NodeList,
    ];

    /// Its name, as the standard spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::EventTarget => "EventTarget",
            Self::Node => "Node",
            Self::CharacterData => "CharacterData",
            Self::Text => "Text",
            Self::Comment => "Comment",
            Self::ProcessingInstruction => "ProcessingInstruction",
            Self::Element => "Element",
            Self::Document => "Document",
            Self::DocumentType => "DocumentType",
            Self::DocumentFragment => "DocumentFragment",
            Self::DomException => "DOMException",
            Self::Event => "Event",
            Self::CustomEvent => "CustomEvent",
            Self::UiEvent => "UIEvent",
            Self::MouseEvent => "MouseEvent",
            Self::PointerEvent => "PointerEvent",
            Self::HtmlElement => "HTMLElement",
            Self::InputEvent => "InputEvent",
            Self::Navigator => "Navigator",
            Self::DomTokenList => "DOMTokenList",
            Self::NodeList => "NodeList",
        }
    }

    /// What its prototype inherits from.
    pub const fn inherits(self) -> Inherits {
        match self {
            Self::EventTarget
            | Self::Event
            | Self::Navigator
            | Self::DomTokenList
            | Self::NodeList => Inherits::Object,
            Self::Node => Inherits::Interface(Self::EventTarget),
            Self::CustomEvent | Self::UiEvent => Inherits::Interface(Self::Event),
            Self::MouseEvent | Self::InputEvent => Inherits::Interface(Self::UiEvent),
            Self::PointerEvent => Inherits::Interface(Self::MouseEvent),
            Self::HtmlElement => Inherits::Interface(Self::Element),
            Self::DomException => Inherits::Error,
            Self::Text | Self::Comment | Self::ProcessingInstruction => {
                Inherits::Interface(Self::CharacterData)
            }
            Self::CharacterData
            | Self::Element
            | Self::Document
            | Self::DocumentType
            | Self::DocumentFragment => Inherits::Interface(Self::Node),
        }
    }

    /// The interface a node of this kind is.
    pub const fn of(kind: &NodeKind) -> Self {
        match kind {
            NodeKind::Document => Self::Document,
            NodeKind::Fragment => Self::DocumentFragment,
            NodeKind::Doctype { .. } => Self::DocumentType,
            NodeKind::Element(element) if matches!(element.name.ns, Namespace::Html) => {
                Self::HtmlElement
            }
            NodeKind::Element(_) => Self::Element,
            NodeKind::Text(_) => Self::Text,
            NodeKind::Comment(_) => Self::Comment,
            NodeKind::ProcessingInstruction { .. } => Self::ProcessingInstruction,
        }
    }

    /// Where it is in [`Interface::ALL`].
    const fn index(self) -> usize {
        match self {
            Self::EventTarget => 0,
            Self::Node => 1,
            Self::CharacterData => 2,
            Self::Text => 3,
            Self::Comment => 4,
            Self::ProcessingInstruction => 5,
            Self::Element => 6,
            Self::Document => 7,
            Self::DocumentType => 8,
            Self::DocumentFragment => 9,
            Self::DomException => 10,
            Self::Event => 11,
            Self::CustomEvent => 12,
            Self::UiEvent => 13,
            Self::MouseEvent => 14,
            Self::PointerEvent => 15,
            Self::HtmlElement => 16,
            Self::InputEvent => 17,
            Self::Navigator => 18,
            Self::DomTokenList => 19,
            Self::NodeList => 20,
        }
    }

    /// Whether it is iterable over the indices its indexed getter answers,
    /// so that its iteration members are `Array.prototype`'s own functions
    /// rather than its own (Web IDL's *iterable declarations*, queue item
    /// 331).
    ///
    /// `DOMTokenList` is iterable too, and joins this once its indexed
    /// getter is built (queue item 328): without the getter its indices are
    /// not there to walk.
    pub const fn iterates_as_an_array(self) -> bool {
        matches!(self, Self::NodeList)
    }

    /// Whether it has `[LegacyUnforgeable]` members, which go on an
    /// unforgeables object rather than its prototype (ADR 0019 § 3).
    pub const fn has_unforgeables(self) -> bool {
        matches!(self, Self::Event)
    }

    /// Put this interface's `[LegacyUnforgeable]` members on `unforgeables`,
    /// each a native inheriting from `function_prototype` — nothing, for an
    /// interface without them.
    ///
    /// **A safepoint.** `unforgeables` must be held by the caller.
    ///
    /// # Errors
    ///
    /// As [`Interface::furnish`].
    pub(crate) fn furnish_unforgeables(
        self,
        objects: &mut Objects,
        unforgeables: Ref,
        function_prototype: Ref,
    ) -> Result<(), Escape> {
        match self {
            Self::Event => event::unforgeables(objects, unforgeables, function_prototype),
            _ => Ok(()),
        }
    }

    /// Put this interface's members on `prototype`, each a native inheriting
    /// from `function_prototype`.
    ///
    /// **A safepoint.** `prototype` must be held by the caller.
    ///
    /// # Errors
    ///
    /// [`Escape::Full`] when the heap cannot hold a member, and a fault for a
    /// reference this engine has lost.
    pub(crate) fn furnish(
        self,
        objects: &mut Objects,
        prototype: Ref,
        function_prototype: Ref,
    ) -> Result<(), Escape> {
        match self {
            Self::EventTarget => event_target::furnish(objects, prototype, function_prototype),
            Self::Event => event::furnish(objects, prototype, function_prototype),
            Self::CustomEvent => custom_event::furnish(objects, prototype, function_prototype),
            Self::UiEvent => ui_event::furnish(objects, prototype, function_prototype),
            Self::MouseEvent => mouse_event::furnish(objects, prototype, function_prototype),
            Self::PointerEvent => pointer_event::furnish(objects, prototype, function_prototype),
            Self::InputEvent => input_event::furnish(objects, prototype, function_prototype),
            Self::Navigator => navigator::furnish(objects, prototype, function_prototype),
            Self::DomTokenList => dom_token_list::furnish(objects, prototype, function_prototype),
            Self::NodeList => node_list::furnish(objects, prototype, function_prototype),
            Self::Node => node::furnish(objects, prototype, function_prototype),
            // `ParentNode`'s `querySelectorAll` is on these three, as a mixin.
            Self::Element => {
                element::furnish(objects, prototype, function_prototype)?;
                parent_node::furnish(objects, prototype, function_prototype)
            }
            Self::HtmlElement => html_element::furnish(objects, prototype, function_prototype),
            Self::Document => {
                document::furnish(objects, prototype, function_prototype)?;
                parent_node::furnish(objects, prototype, function_prototype)
            }
            Self::DocumentFragment => parent_node::furnish(objects, prototype, function_prototype),
            Self::DomException => dom_exception::furnish(objects, prototype, function_prototype),
            // `ChildNode.remove()` is the one member these have, as a mixin.
            Self::CharacterData | Self::DocumentType => {
                child_node::furnish(objects, prototype, function_prototype)
            }
            // In the chain, with none of their members built: `Text`'s
            // `splitText` and `wholeText`, `CharacterData`'s `data`. Each is
            // added here when something needs it.
            Self::Text | Self::Comment | Self::ProcessingInstruction => Ok(()),
        }
    }
}

/// The prototype of every interface, and the unforgeables of those that have
/// them, as the document cell holds them.
#[derive(Debug, Default)]
pub struct Interfaces {
    prototypes: [Field; Interface::ALL.len()],
    unforgeables: [Field; Interface::ALL.len()],
}

impl Interfaces {
    /// The prototype of `interface`, once it has been made.
    pub fn prototype(&self, interface: Interface) -> Option<Ref> {
        self.prototypes.get(interface.index()).and_then(Field::get)
    }

    /// The unforgeables object of `interface` — Web IDL's `[[Unforgeables]]`
    /// — once it has been made, and [`None`] for an interface with no
    /// `[LegacyUnforgeable]` member.
    pub fn unforgeables(&self, interface: Interface) -> Option<Ref> {
        self.unforgeables
            .get(interface.index())
            .and_then(Field::get)
    }

    /// Record `prototype` as `interface`'s, through the barrier every store
    /// of a reference passes (ADR 0014 § 5).
    pub(crate) fn set(&mut self, barrier: &mut Barrier, interface: Interface, prototype: Ref) {
        if let Some(field) = self.prototypes.get_mut(interface.index()) {
            field.set(barrier, Some(prototype));
        }
    }

    /// Record `unforgeables` as `interface`'s, through the barrier.
    pub(crate) fn set_unforgeables(
        &mut self,
        barrier: &mut Barrier,
        interface: Interface,
        unforgeables: Ref,
    ) {
        if let Some(field) = self.unforgeables.get_mut(interface.index()) {
            field.set(barrier, Some(unforgeables));
        }
    }

    /// Every prototype and unforgeables object, as strong edges of the cell
    /// that holds them.
    pub(crate) fn trace(&self, tracer: &mut Tracer) {
        for field in self.prototypes.iter().chain(&self.unforgeables) {
            field.trace(tracer);
        }
    }
}

/// The prototype a wrapper of `node` inherits from, in the document `cell`
/// holds: its kind's interface's, or [`None`] before the interfaces are made.
pub fn prototype_of(objects: &Objects, cell: Ref, node: NodeId) -> Option<Ref> {
    let held = objects.embedded::<DocumentCell>(cell)?;
    let kind = &held.document().get(node)?.kind;
    held.interfaces.prototype(Interface::of(kind))
}
