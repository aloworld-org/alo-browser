/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! HTML's *the body element*: what `document.body` reads, and what assigning
//! to it does (queue item 253).
//!
//! **The body element** is the first child of *the html element* — the
//! document element, when it is an HTML `html` — that is a `body` or a
//! `frameset`, or nothing. Assigning one replaces it, or appends the new one
//! to the document element when there is none, and refuses anything else
//! with `HierarchyRequestError`.
//!
//! # `frameset` counts, though nothing renders it
//!
//! Law 1 refuses to *render* frames, and nothing here does. What it does not
//! license is a `document.body` that answers differently from the standard on
//! a page that has one: a getter that skipped `<frameset>` would read `null`
//! where every other engine reads an element, and a setter given one would
//! append beside it instead of replacing it — the approximate member ADR 0013
//! § 3 refuses. Recognising the name costs one comparison; the page still
//! fails for want of frames, as law 1 says it should.
//!
//! The rules live here rather than in the bindings for ADR 0017 § 5's
//! reason: the replacement and the append are this crate's own operations,
//! under the same validity rules an agent's change meets.

use crate::document::Document;
use crate::node::NodeId;
use crate::validity::Refusal;

impl Document {
    /// The body element: the first `body` or `frameset` child of the html
    /// element, or [`None`].
    pub fn body(&self) -> Option<NodeId> {
        self.body_and_its_parent().map(|(body, _)| body)
    }

    /// Make `new` the body element — `document.body = new`, where [`None`]
    /// is `null`.
    ///
    /// The same element as the body element changes nothing. Otherwise the
    /// body element is replaced by `new` in its parent, or, when there is
    /// none, `new` is appended to the document element. Either way it is one
    /// of this crate's own operations, so `new` is taken from wherever it
    /// was and the change is counted once.
    ///
    /// # Errors
    ///
    /// [`Refusal::HierarchyRequest`] when `new` is not an HTML `body` or
    /// `frameset` — `null` included — and when there is no body element and
    /// no document element to append to; and whatever the replacement or the
    /// append refuses. The tree is left exactly as it was.
    pub fn set_body(&mut self, new: Option<NodeId>) -> Result<(), Refusal> {
        let new =
            new.filter(|new| self.is_body_or_frameset(*new))
                .ok_or(Refusal::HierarchyRequest(
                    "the body must be a body or a frameset element",
                ))?;
        match self.body_and_its_parent() {
            Some((old, _)) if old == new => Ok(()),
            Some((old, html)) => self.replace_child(html, new, old).map(drop),
            None => {
                let root = self.document_element().ok_or(Refusal::HierarchyRequest(
                    "there is no document element to put a body in",
                ))?;
                self.append_child(root, new).map(drop)
            }
        }
    }

    /// The body element, and the html element it is a child of.
    fn body_and_its_parent(&self) -> Option<(NodeId, NodeId)> {
        let html = self.html_element()?;
        let body = self
            .children(html)
            .find(|child| self.is_body_or_frameset(*child))?;
        Some((body, html))
    }

    /// The document element: the document's one element child.
    pub fn document_element(&self) -> Option<NodeId> {
        self.children(self.root())
            .find(|child| self.element(*child).is_some())
    }

    /// The html element: the document element, when it is an HTML `html`.
    fn html_element(&self) -> Option<NodeId> {
        self.document_element()
            .filter(|root| self.element(*root).is_some_and(|e| e.name.is_html("html")))
    }

    /// Whether `node` is an HTML `body` or `frameset`.
    fn is_body_or_frameset(&self, node: NodeId) -> bool {
        self.element(node)
            .is_some_and(|e| e.name.is_html("body") || e.name.is_html("frameset"))
    }
}
