/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The tree, written back out as HTML.
//!
//! This is ours rather than the parser's serialiser, for the same reason the
//! tree is: it is the half that says what we believe the document *is*. It
//! follows the HTML fragment serialisation algorithm, minus the parts that
//! exist only for markup written before 2015 — `listing` and `xmp` do not
//! appear here, and neither does the legacy doctype form.
//!
//! Round-tripping is what makes this testable: parse, serialise, and the text
//! comes back. When it does not, one of the two halves is wrong, and the diff
//! says which.

use crate::document::Document;
use crate::name::Namespace;
use crate::node::{Attribute, Element, NodeId, NodeKind};

/// Elements with no end tag and no children.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "img", "input",
    "keygen", "link", "meta", "param", "source", "track", "wbr",
];

/// Elements whose text children are markup to nobody: their content is written
/// literally, with no character references.
///
/// The serialisation algorithm also lists `xmp`, `plaintext` and `listing`,
/// which are legacy, and `noscript`, which is raw text only when scripting is
/// enabled — stage 1 has no scripting, so a `<noscript>`'s children are real
/// elements and are written as such.
const RAW_TEXT_ELEMENTS: &[&str] = &["style", "script", "iframe", "noembed", "noframes"];

/// Elements that swallow a leading newline when parsed, and so must be given
/// one back when written.
const NEWLINE_SWALLOWING_ELEMENTS: &[&str] = &["pre", "textarea"];

impl Document {
    /// The subtree under `id`, as HTML, not including `id`'s own tags.
    ///
    /// Serialising the root gives the whole document back.
    pub fn serialize_children(&self, id: NodeId) -> String {
        let mut out = String::new();
        for child in self.children(id) {
            self.write_node(child, &mut out);
        }
        out
    }

    /// One node and everything under it, as HTML.
    pub fn serialize_node(&self, id: NodeId) -> String {
        let mut out = String::new();
        self.write_node(id, &mut out);
        out
    }

    fn write_node(&self, id: NodeId, out: &mut String) {
        let Some(kind) = self.kind(id) else {
            return;
        };
        match kind {
            NodeKind::Document | NodeKind::Fragment => {
                for child in self.children(id) {
                    self.write_node(child, out);
                }
            }
            // The serialisation algorithm writes the name only. The public and
            // system identifiers stay in the tree; see `NodeKind::Doctype`.
            NodeKind::Doctype { name, .. } => {
                out.push_str("<!DOCTYPE ");
                out.push_str(name);
                out.push('>');
            }
            NodeKind::Element(element) => self.write_element(id, element, out),
            NodeKind::Text(text) => {
                if self.parent_is_raw_text(id) {
                    out.push_str(text);
                } else {
                    escape_text(text, out);
                }
            }
            NodeKind::Comment(text) => {
                out.push_str("<!--");
                out.push_str(text);
                out.push_str("-->");
            }
            NodeKind::ProcessingInstruction { target, data } => {
                out.push_str("<?");
                out.push_str(target);
                out.push(' ');
                out.push_str(data);
                out.push('>');
            }
        }
    }

    fn write_element(&self, id: NodeId, element: &Element, out: &mut String) {
        let tag = tag_name(element);
        out.push('<');
        out.push_str(&tag);
        for attr in &element.attrs {
            out.push(' ');
            out.push_str(&attribute_name(attr));
            out.push_str("=\"");
            escape_attribute_value(&attr.value, out);
            out.push('"');
        }
        out.push('>');

        if is_html_element_in(element, VOID_ELEMENTS) {
            return;
        }

        // The parser drops a newline immediately after `<pre>`; writing one
        // back is what makes `<pre>\n\ntext</pre>` survive a round trip.
        if is_html_element_in(element, NEWLINE_SWALLOWING_ELEMENTS)
            && self
                .first_child(id)
                .and_then(|child| self.get(child))
                .and_then(crate::node::Node::text)
                .is_some_and(|text| text.starts_with('\n'))
        {
            out.push('\n');
        }

        // A template's children are its *contents*, which are not its
        // children in the tree — the fragment beside it is the whole point of
        // a template. The serialisation algorithm says the same.
        let content_owner = element.template_contents.unwrap_or(id);
        for child in self.children(content_owner) {
            self.write_node(child, out);
        }
        out.push_str("</");
        out.push_str(&tag);
        out.push('>');
    }

    fn parent_is_raw_text(&self, id: NodeId) -> bool {
        self.parent(id)
            .and_then(|parent| self.element(parent))
            .is_some_and(|element| is_html_element_in(element, RAW_TEXT_ELEMENTS))
    }
}

/// The tag name to write. An HTML, SVG or MathML element is written by its
/// local name; anything else keeps the prefix it was written with, because
/// without it the name would not parse back to the same thing.
fn tag_name(element: &Element) -> String {
    match element.name.ns {
        Namespace::Html | Namespace::Svg | Namespace::MathMl => element.name.local.to_string(),
        _ => element.name.to_string(),
    }
}

/// The attribute name to write, per the serialisation algorithm: the namespaced
/// attributes the HTML parser can produce have fixed spellings.
fn attribute_name(attr: &Attribute) -> String {
    let local = &attr.name.local;
    match attr.name.ns {
        Namespace::None => local.to_string(),
        Namespace::Xml => format!("xml:{local}"),
        Namespace::XLink => format!("xlink:{local}"),
        Namespace::XmlNs if &**local == "xmlns" => "xmlns".to_owned(),
        Namespace::XmlNs => format!("xmlns:{local}"),
        _ => attr.name.to_string(),
    }
}

fn is_html_element_in(element: &Element, names: &[&str]) -> bool {
    element.name.ns == Namespace::Html && names.contains(&&*element.name.local)
}

/// Escape character data. A non-breaking space becomes `&nbsp;` because it is
/// otherwise indistinguishable from a space in the source.
fn escape_text(text: &str, out: &mut String) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\u{00a0}' => out.push_str("&nbsp;"),
            _ => out.push(ch),
        }
    }
}

/// Escape an attribute value. Values are always written in double quotes, so
/// only the quote itself needs escaping — `<` and `>` do not end a value.
fn escape_attribute_value(value: &str, out: &mut String) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\u{00a0}' => out.push_str("&nbsp;"),
            _ => out.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::name::QualifiedName;

    fn escaped(text: &str) -> String {
        let mut out = String::new();
        escape_text(text, &mut out);
        out
    }

    fn escaped_value(value: &str) -> String {
        let mut out = String::new();
        escape_attribute_value(value, &mut out);
        out
    }

    #[test]
    fn text_escapes_the_four_characters_that_need_it() {
        assert_eq!(
            escaped("a & b < c > d\u{00a0}e"),
            "a &amp; b &lt; c &gt; d&nbsp;e"
        );
        assert_eq!(escaped("plain \"quotes\" stay"), "plain \"quotes\" stay");
    }

    #[test]
    fn an_attribute_value_escapes_the_quote_but_not_the_angle_brackets() {
        assert_eq!(escaped_value("a \"b\" & c"), "a &quot;b&quot; &amp; c");
        assert_eq!(escaped_value("1 < 2"), "1 < 2");
    }

    #[test]
    fn namespaced_attributes_have_their_fixed_spellings() {
        let named = |ns: Namespace, local: &str| Attribute {
            name: QualifiedName::new(ns, local),
            value: String::new(),
        };
        assert_eq!(attribute_name(&named(Namespace::None, "class")), "class");
        assert_eq!(attribute_name(&named(Namespace::Xml, "lang")), "xml:lang");
        assert_eq!(
            attribute_name(&named(Namespace::XLink, "href")),
            "xlink:href"
        );
        assert_eq!(attribute_name(&named(Namespace::XmlNs, "xmlns")), "xmlns");
        assert_eq!(attribute_name(&named(Namespace::XmlNs, "svg")), "xmlns:svg");
    }

    #[test]
    fn an_element_in_an_unknown_namespace_keeps_its_prefix() {
        let element = Element {
            name: QualifiedName {
                ns: Namespace::Other("urn:alo".into()),
                local: "widget".into(),
                prefix: Some("alo".into()),
            },
            attrs: Vec::new(),
            template_contents: None,
            mathml_annotation_xml_integration_point: false,
        };
        assert_eq!(tag_name(&element), "alo:widget");
    }

    #[test]
    fn serialising_an_id_from_another_document_gives_nothing() {
        let document = Document::new();
        let stranger = crate::parse::parse_document("<p>x</p>");
        let far = stranger.node_count() - 1;
        assert_eq!(document.serialize_node(crate::node::NodeId(far)), "");
    }
}
