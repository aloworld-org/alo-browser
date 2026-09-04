/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The parser boundary.
//!
//! **This is the only module in the crate that names `html5ever`.** ADR 0001
//! rents the HTML parser and writes the tree; keeping the rented thing behind
//! one file is what makes that true in the source rather than only in prose. A
//! name crosses this boundary once, in each direction, and everything on the
//! other side of it is ours.
//!
//! What stage 1 does not take from the parser, by decision:
//!
//! - **Quirks mode** is recorded and never honoured. Law 1 refuses it, so a
//!   document that would put another engine into quirks mode is still laid out
//!   as standards. See [`QuirksSignal`](crate::QuirksSignal).
//! - **Scripting** does not exist, so a `<script>` is a element with text in it
//!   and nothing runs. `mark_script_already_started` has nothing to mark.
//!
//!   What will run there is decided: ADR 0013 — `alo-js`, ours, a bytecode
//!   compiler and an interpreter in safe Rust, with no JIT until a measurement
//!   says otherwise. Two of its clauses land in *this* file rather than in that
//!   crate. `document.write` stays refused (stage 3, queue item 137), so the
//!   parser is never re-entered by a script it started; and a script that is
//!   fetched is fetched by the browser process, because ADR 0005 gives this one
//!   no network and ADR 0013 § 5 gives `alo-js` no I/O at all.
//! - **Declarative shadow roots** are refused rather than half-built: they are
//!   not in `docs/features.md`, and a parser told "yes" by a sink that cannot
//!   attach one produces a tree that says something happened when it did not.
//! - **Encoding sniffing** is not ours to do. Input is `&str`; whoever read the
//!   bytes decided what they meant.

use crate::document::{Document, QuirksSignal};
use crate::name::{Namespace, QualifiedName};
use crate::node::{Attribute, Element, NodeId, NodeKind};
use core::cell::{Cell, RefCell};
use core::fmt;
use html5ever::interface::tree_builder::{
    ElemName, ElementFlags, NodeOrText, QuirksMode, TreeSink,
};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::{LocalName, ParseOpts, Prefix, QualName};
use std::borrow::Cow;

/// Something the parser objected to, and where.
///
/// Malformed input still produces a usable tree — that is what an HTML parser
/// is for. The issues say what had to be repaired to get one, so a document
/// that renders oddly can be asked why rather than guessed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseIssue {
    /// What the parser objected to.
    pub message: String,
    /// The line it was on, counting from one.
    pub line: u64,
}

impl fmt::Display for ParseIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// Parse a whole HTML document.
///
/// Always succeeds: an HTML parser has a defined repair for every input, and
/// [`Document::issues`] reports what it had to repair.
pub fn parse_document(html: &str) -> Document {
    html5ever::parse_document(Sink::new(), ParseOpts::default()).one(html)
}

/// Parse a fragment as though it appeared inside `context`.
///
/// The children of the returned document's root are the fragment's top-level
/// nodes. The parser wants an `<html>` element to hang a fragment from; that
/// element is this module's business and does not escape it.
pub fn parse_fragment(html: &str, context: &QualifiedName) -> Document {
    let mut document = html5ever::parse_fragment(
        Sink::new(),
        ParseOpts::default(),
        to_qual_name(context),
        Vec::new(),
        false,
    )
    .one(html);
    hoist_fragment_root(&mut document);
    document
}

/// Move the parser's synthesised `<html>` root out of the way, so the caller
/// sees the nodes they asked for and not the scaffolding.
fn hoist_fragment_root(document: &mut Document) {
    let root = document.root();
    let mut children = document.children(root);
    let (Some(scaffold), None) = (children.next(), children.next()) else {
        return;
    };
    if !document
        .get(scaffold)
        .is_some_and(|node| node.is_html_element("html"))
    {
        return;
    }
    document.reparent_children(scaffold, root);
    document.detach(scaffold);
}

// --- name conversion, the only place it happens ----------------------------

fn from_qual_name(name: &QualName) -> QualifiedName {
    QualifiedName {
        ns: Namespace::from_uri(&name.ns),
        local: (*name.local).into(),
        prefix: name.prefix.as_ref().map(|prefix| (**prefix).into()),
    }
}

fn to_qual_name(name: &QualifiedName) -> QualName {
    QualName::new(
        name.prefix.as_ref().map(|prefix| Prefix::from(&**prefix)),
        html5ever::Namespace::from(name.ns.as_str()),
        LocalName::from(&*name.local),
    )
}

fn from_attrs(attrs: Vec<html5ever::Attribute>) -> Vec<Attribute> {
    attrs
        .into_iter()
        .map(|attr| Attribute {
            name: from_qual_name(&attr.name),
            value: attr.value.to_string(),
        })
        .collect()
}

fn from_quirks_mode(mode: QuirksMode) -> QuirksSignal {
    match mode {
        QuirksMode::NoQuirks => QuirksSignal::NoQuirks,
        QuirksMode::LimitedQuirks => QuirksSignal::LimitedQuirks,
        QuirksMode::Quirks => QuirksSignal::Quirks,
    }
}

/// An element name in the form the tree builder asks for it back.
///
/// The tree builder wants borrowed interned names; our nodes hold owned ones,
/// so this hands back an owned copy. Interned names are cheap to build and this
/// happens only while parsing.
#[derive(Debug)]
pub struct SinkName(QualName);

impl ElemName for SinkName {
    fn ns(&self) -> &html5ever::Namespace {
        &self.0.ns
    }

    fn local_name(&self) -> &LocalName {
        &self.0.local
    }
}

/// The tree builder's view of us: it says what happened, we build the tree.
///
/// `TreeSink` takes `&self` throughout, so the document sits behind a
/// `RefCell`. Every borrow here is taken and dropped inside one method and
/// never spans a call back into the parser, which is what keeps that borrow
/// from ever being contended.
struct Sink {
    document: RefCell<Document>,
    line: Cell<u64>,
}

impl Sink {
    fn new() -> Self {
        Self {
            document: RefCell::new(Document::new()),
            line: Cell::new(1),
        }
    }

    fn create(&self, kind: NodeKind) -> NodeId {
        self.document.borrow_mut().create(kind)
    }

    /// Record a violation of something the tree builder promised us. It cannot
    /// happen with a tree builder that keeps its promises; if it ever does, the
    /// document says so rather than the process ending.
    fn record_broken_promise(&self, what: &str) {
        self.document.borrow_mut().record_issue(ParseIssue {
            message: format!("the tree builder called {what} on the wrong kind of node"),
            line: self.line.get(),
        });
    }

    fn append_node_or_text(
        &self,
        child: NodeOrText<NodeId>,
        append_node: impl FnOnce(&mut Document, NodeId),
        append_text: impl FnOnce(&mut Document, &str),
    ) {
        let mut document = self.document.borrow_mut();
        match child {
            NodeOrText::AppendNode(id) => append_node(&mut document, id),
            NodeOrText::AppendText(text) => append_text(&mut document, &text),
        }
    }
}

impl TreeSink for Sink {
    type Handle = NodeId;
    type Output = Document;
    type ElemName<'a>
        = SinkName
    where
        Self: 'a;

    fn finish(self) -> Document {
        self.document.into_inner()
    }

    fn parse_error(&self, msg: Cow<'static, str>) {
        self.document.borrow_mut().record_issue(ParseIssue {
            message: msg.into_owned(),
            line: self.line.get(),
        });
    }

    fn set_current_line(&self, line_number: u64) {
        self.line.set(line_number);
    }

    fn get_document(&self) -> NodeId {
        self.document.borrow().root()
    }

    fn elem_name<'a>(&'a self, target: &'a NodeId) -> SinkName {
        let name = self
            .document
            .borrow()
            .element(*target)
            .map(|element| to_qual_name(&element.name));
        if let Some(name) = name {
            return SinkName(name);
        }
        self.record_broken_promise("elem_name");
        // A name no element can have, so nothing matches it and the parse
        // continues instead of stopping.
        SinkName(QualName::new(
            None,
            html5ever::Namespace::from(""),
            LocalName::from(""),
        ))
    }

    fn create_element(
        &self,
        name: QualName,
        attrs: Vec<html5ever::Attribute>,
        flags: ElementFlags,
    ) -> NodeId {
        let template_contents = flags.template.then(|| self.create(NodeKind::Fragment));
        self.create(NodeKind::Element(Element {
            name: from_qual_name(&name),
            attrs: from_attrs(attrs),
            template_contents,
            mathml_annotation_xml_integration_point: flags.mathml_annotation_xml_integration_point,
        }))
    }

    fn create_comment(&self, text: StrTendril) -> NodeId {
        self.create(NodeKind::Comment(text.to_string()))
    }

    fn create_pi(&self, target: StrTendril, data: StrTendril) -> NodeId {
        self.create(NodeKind::ProcessingInstruction {
            target: target.to_string(),
            data: data.to_string(),
        })
    }

    fn append(&self, parent: &NodeId, child: NodeOrText<NodeId>) {
        let parent = *parent;
        self.append_node_or_text(
            child,
            |document, id| {
                document.append(parent, id);
            },
            |document, text| document.append_text(parent, text),
        );
    }

    fn append_before_sibling(&self, sibling: &NodeId, new_node: NodeOrText<NodeId>) {
        let sibling = *sibling;
        self.append_node_or_text(
            new_node,
            |document, id| {
                document.insert_before(sibling, id);
            },
            |document, text| document.insert_text_before(sibling, text),
        );
    }

    fn append_based_on_parent_node(
        &self,
        element: &NodeId,
        prev_element: &NodeId,
        child: NodeOrText<NodeId>,
    ) {
        let has_parent = self.document.borrow().parent(*element).is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        let doctype = self.create(NodeKind::Doctype {
            name: name.to_string(),
            public_id: public_id.to_string(),
            system_id: system_id.to_string(),
        });
        let mut document = self.document.borrow_mut();
        let root = document.root();
        document.append(root, doctype);
    }

    fn get_template_contents(&self, target: &NodeId) -> NodeId {
        let held = self
            .document
            .borrow()
            .element(*target)
            .and_then(|element| element.template_contents);
        if let Some(contents) = held {
            return contents;
        }
        // Only reachable if the tree builder asked a non-template for its
        // contents. Give it a fragment rather than a node that is already in
        // the tree: a wrong answer here would silently graft the template's
        // children onto the document.
        self.record_broken_promise("get_template_contents");
        self.create(NodeKind::Fragment)
    }

    fn same_node(&self, x: &NodeId, y: &NodeId) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.document
            .borrow_mut()
            .set_quirks_signal(from_quirks_mode(mode));
    }

    fn add_attrs_if_missing(&self, target: &NodeId, attrs: Vec<html5ever::Attribute>) {
        self.document
            .borrow_mut()
            .add_attrs_if_missing(*target, from_attrs(attrs));
    }

    fn remove_from_parent(&self, target: &NodeId) {
        self.document.borrow_mut().detach(*target);
    }

    fn reparent_children(&self, node: &NodeId, new_parent: &NodeId) {
        self.document
            .borrow_mut()
            .reparent_children(*node, *new_parent);
    }

    fn is_mathml_annotation_xml_integration_point(&self, handle: &NodeId) -> bool {
        self.document
            .borrow()
            .element(*handle)
            .is_some_and(|element| element.mathml_annotation_xml_integration_point)
    }

    fn allow_declarative_shadow_roots(&self, _intended_parent: &NodeId) -> bool {
        // Not in `docs/features.md`. Saying no is honest; saying yes and then
        // failing to attach one leaves a tree that claims a shadow root exists.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name::HTML_NS;

    #[test]
    fn a_name_survives_both_conversions() {
        for name in [
            QualifiedName::html("div"),
            QualifiedName::new(Namespace::None, "class"),
            QualifiedName::new(Namespace::Svg, "svg"),
            QualifiedName {
                ns: Namespace::XLink,
                local: "href".into(),
                prefix: Some("xlink".into()),
            },
            QualifiedName::new(Namespace::Other("urn:alo".into()), "thing"),
        ] {
            assert_eq!(from_qual_name(&to_qual_name(&name)), name);
        }
    }

    #[test]
    fn a_converted_name_carries_the_namespace_uri() {
        let converted = to_qual_name(&QualifiedName::html("p"));
        assert_eq!(&*converted.ns, HTML_NS);
        assert_eq!(&*converted.local, "p");
        assert!(converted.prefix.is_none());
    }

    #[test]
    fn every_quirks_mode_maps_to_a_signal() {
        assert_eq!(
            from_quirks_mode(QuirksMode::NoQuirks),
            QuirksSignal::NoQuirks
        );
        assert_eq!(
            from_quirks_mode(QuirksMode::LimitedQuirks),
            QuirksSignal::LimitedQuirks
        );
        assert_eq!(from_quirks_mode(QuirksMode::Quirks), QuirksSignal::Quirks);
    }

    #[test]
    fn an_issue_says_where_it_was() {
        let issue = ParseIssue {
            message: "unexpected token".to_owned(),
            line: 4,
        };
        assert_eq!(issue.to_string(), "line 4: unexpected token");
    }

    #[test]
    fn asking_a_non_element_for_its_name_records_the_broken_promise() {
        let sink = Sink::new();
        let text = sink.create(NodeKind::Text("x".to_owned()));
        let name = sink.elem_name(&text);
        assert_eq!(&**name.local_name(), "");
        assert_eq!(sink.document.borrow().issues().len(), 1);
    }

    #[test]
    fn asking_a_non_template_for_its_contents_gives_a_fresh_fragment() {
        let sink = Sink::new();
        let text = sink.create(NodeKind::Text("x".to_owned()));
        let contents = sink.get_template_contents(&text);
        assert_ne!(contents, text);
        assert_eq!(
            sink.document.borrow().kind(contents),
            Some(&NodeKind::Fragment)
        );
        assert!(!sink.document.borrow().is_attached(contents));
        assert_eq!(sink.document.borrow().issues().len(), 1);
    }
}
