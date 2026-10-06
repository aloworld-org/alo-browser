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
//! # Events are interfaces too
//!
//! `EventTarget` is at the top of a node's chain (ADR 0018 § 1), and `Event`
//! and `CustomEvent` are the interfaces of the event objects a script makes
//! and dispatches. Their prototypes are the document cell's like every
//! other, so the browser's own dispatch (queue item 255) finds them where a
//! node's native finds `Element.prototype`.
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
pub mod element;
pub mod event;
pub mod event_target;
pub mod node;

use alo_dom::{NodeId, NodeKind};
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
    pub const ALL: [Self; 13] = [
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
        }
    }

    /// What its prototype inherits from.
    pub const fn inherits(self) -> Inherits {
        match self {
            Self::EventTarget | Self::Event => Inherits::Object,
            Self::Node => Inherits::Interface(Self::EventTarget),
            Self::CustomEvent => Inherits::Interface(Self::Event),
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
            Self::Node => node::furnish(objects, prototype, function_prototype),
            Self::Element => element::furnish(objects, prototype, function_prototype),
            Self::Document => document::furnish(objects, prototype, function_prototype),
            Self::DomException => dom_exception::furnish(objects, prototype, function_prototype),
            // `ChildNode.remove()` is the one member these have, as a mixin.
            Self::CharacterData | Self::DocumentType => {
                child_node::furnish(objects, prototype, function_prototype)
            }
            // In the chain, with none of their members built: `Text`'s
            // `splitText` and `wholeText`, `CharacterData`'s `data`, a
            // fragment's queries. Each is added here when something needs it.
            Self::Text | Self::Comment | Self::ProcessingInstruction | Self::DocumentFragment => {
                Ok(())
            }
        }
    }
}

/// The prototype of every interface, as the document cell holds them.
#[derive(Debug, Default)]
pub struct Interfaces([Field; Interface::ALL.len()]);

impl Interfaces {
    /// The prototype of `interface`, once it has been made.
    pub fn prototype(&self, interface: Interface) -> Option<Ref> {
        self.0.get(interface.index()).and_then(Field::get)
    }

    /// Record `prototype` as `interface`'s, through the barrier every store
    /// of a reference passes (ADR 0014 § 5).
    pub(crate) fn set(&mut self, barrier: &mut Barrier, interface: Interface, prototype: Ref) {
        if let Some(field) = self.0.get_mut(interface.index()) {
            field.set(barrier, Some(prototype));
        }
    }

    /// Every prototype, as a strong edge of the cell that holds them.
    pub(crate) fn trace(&self, tracer: &mut Tracer) {
        for field in &self.0 {
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
