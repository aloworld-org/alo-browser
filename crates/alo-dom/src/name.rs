/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Namespaces and qualified names.
//!
//! These are ours rather than the parser's. `html5ever` has perfectly good
//! interned names, but a tree that stores them is a tree that has to be
//! rewritten the day the parser changes — and ADR 0001 is explicit that
//! `html5ever` is rented, not adopted. Conversion happens once, at the parse
//! boundary in [`crate::parse`], which is the only module in this crate that
//! names `html5ever` at all.

use core::fmt;

/// A namespace an element or attribute name can live in.
///
/// The named variants are the ones the HTML parser can actually produce. Any
/// other URI is kept verbatim in [`Namespace::Other`] rather than discarded —
/// the same rule the style engine applies to unknown properties, for the same
/// reason: a later stage should be able to implement it without re-parsing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Namespace {
    /// No namespace. The ordinary case for an attribute such as `class`.
    None,
    /// `http://www.w3.org/1999/xhtml`
    Html,
    /// `http://www.w3.org/1998/Math/MathML`
    MathMl,
    /// `http://www.w3.org/2000/svg`
    Svg,
    /// `http://www.w3.org/1999/xlink`
    XLink,
    /// `http://www.w3.org/XML/1998/namespace`
    Xml,
    /// `http://www.w3.org/2000/xmlns/`
    XmlNs,
    /// Any other namespace URI, kept as written.
    Other(Box<str>),
}

/// The XHTML namespace URI.
pub const HTML_NS: &str = "http://www.w3.org/1999/xhtml";
/// The MathML namespace URI.
pub const MATHML_NS: &str = "http://www.w3.org/1998/Math/MathML";
/// The SVG namespace URI.
pub const SVG_NS: &str = "http://www.w3.org/2000/svg";
/// The XLink namespace URI.
pub const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
/// The XML namespace URI.
pub const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
/// The XMLNS namespace URI.
pub const XMLNS_NS: &str = "http://www.w3.org/2000/xmlns/";

impl Namespace {
    /// The namespace URI, or the empty string for [`Namespace::None`].
    pub fn as_str(&self) -> &str {
        match self {
            Namespace::None => "",
            Namespace::Html => HTML_NS,
            Namespace::MathMl => MATHML_NS,
            Namespace::Svg => SVG_NS,
            Namespace::XLink => XLINK_NS,
            Namespace::Xml => XML_NS,
            Namespace::XmlNs => XMLNS_NS,
            Namespace::Other(uri) => uri,
        }
    }

    /// The namespace a URI names. An unrecognised URI becomes
    /// [`Namespace::Other`]; it is never dropped.
    pub fn from_uri(uri: &str) -> Self {
        match uri {
            "" => Namespace::None,
            HTML_NS => Namespace::Html,
            MATHML_NS => Namespace::MathMl,
            SVG_NS => Namespace::Svg,
            XLINK_NS => Namespace::XLink,
            XML_NS => Namespace::Xml,
            XMLNS_NS => Namespace::XmlNs,
            other => Namespace::Other(other.into()),
        }
    }
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A qualified name: a namespace, a local name, and the prefix it was written
/// with.
///
/// The prefix is kept because it is what the author wrote, and because a
/// diagnostic that says `xlink:href` is more use than one that says `href` in
/// a namespace nobody can see. Nothing matches on it — matching is by
/// namespace and local name, which is what the specification says identity is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedName {
    /// The namespace the name lives in.
    pub ns: Namespace,
    /// The local part of the name, lowercased for HTML by the parser.
    pub local: Box<str>,
    /// The prefix the name was written with, if it had one.
    pub prefix: Option<Box<str>>,
}

impl QualifiedName {
    /// A name in a namespace, with no prefix.
    pub fn new(ns: Namespace, local: &str) -> Self {
        Self {
            ns,
            local: local.into(),
            prefix: None,
        }
    }

    /// A name in the HTML namespace, with no prefix. The common case.
    pub fn html(local: &str) -> Self {
        Self::new(Namespace::Html, local)
    }

    /// Whether this is `local` in the HTML namespace.
    pub fn is_html(&self, local: &str) -> bool {
        self.ns == Namespace::Html && &*self.local == local
    }

    /// Whether this is `local` with no namespace — the ordinary attribute case.
    pub fn is_plain(&self, local: &str) -> bool {
        self.ns == Namespace::None && &*self.local == local
    }
}

impl fmt::Display for QualifiedName {
    /// Writes `prefix:local`, or just `local` when there is no prefix. This is
    /// for diagnostics; serialisation has its own rules and does not use this.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.prefix {
            Some(prefix) => write!(f, "{prefix}:{}", self.local),
            None => f.write_str(&self.local),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_known_uri_round_trips() {
        for ns in [
            Namespace::None,
            Namespace::Html,
            Namespace::MathMl,
            Namespace::Svg,
            Namespace::XLink,
            Namespace::Xml,
            Namespace::XmlNs,
        ] {
            assert_eq!(Namespace::from_uri(ns.as_str()), ns);
        }
    }

    #[test]
    fn an_unknown_uri_is_kept_rather_than_dropped() {
        let ns = Namespace::from_uri("https://alo.example/ns");
        assert_eq!(ns, Namespace::Other("https://alo.example/ns".into()));
        assert_eq!(ns.as_str(), "https://alo.example/ns");
    }

    #[test]
    fn identity_is_namespace_and_local_name_not_prefix() {
        let plain = QualifiedName::html("div");
        let prefixed = QualifiedName {
            prefix: Some("h".into()),
            ..QualifiedName::html("div")
        };
        assert!(plain.is_html("div"));
        assert!(prefixed.is_html("div"));
        assert!(!plain.is_html("span"));
        assert!(!plain.is_plain("div"));
        assert_eq!(prefixed.to_string(), "h:div");
        assert_eq!(plain.to_string(), "div");
    }
}
