/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Reading an SVG file: the XML boundary (ADR 0027).
//!
//! **This is the only module in the engine that names `quick-xml`**, as
//! [`crate::parse`] is the only one that names `html5ever`. The crate is a
//! pull reader: it hands over one event at a time and keeps no tree, so the
//! tree is built here from those events, on our own stack of open elements,
//! with node ids allocated as ADR 0003 requires.
//!
//! # Not an XML parser for the engine
//!
//! It has one entry point, [`read_svg`], and one caller, the picture path.
//! Navigation to an XML document, XHTML and `DOMParser`'s XML modes stay
//! stage 3's item 139 (ADR 0027 § 3), and nothing here should be cited as
//! having opened them.
//!
//! # What it accepts, and what it refuses
//!
//! - **UTF-8 only**, with or without a byte order mark; a declaration may
//!   name `UTF-8` or nothing. Any other encoding is refused.
//! - **XML 1.0.** A declaration naming another version is refused, and a
//!   declaration anywhere but at the very start is malformed.
//! - **A document type declaration** is ignored when it has no internal
//!   subset and refused when it has one, because the internal subset is the
//!   only place an entity can be declared. Its external identifier is never
//!   fetched; nothing here can fetch.
//! - **Entities**: the five XML predefines and character references to
//!   characters XML allows. Any other is undeclared, which XML makes fatal,
//!   so there is no expansion and no external entity by construction.
//! - **Processing instructions** are ignored, `<?xml-stylesheet?>` included.
//! - **One root element**, which must be `svg` in the SVG namespace, with
//!   nothing but comments, processing instructions and white space around it.
//! - **`CDATA`** is read as text.
//! - **Every prefix must be declared**, and no element may carry two
//!   attributes with the same namespace and local name, however they were
//!   prefixed.
//!
//! **Any error refuses the whole file** with an [`XmlRefusal`] naming why.
//! HTML recovers from every error and XML is defined not to; a file that
//! every other engine shows as broken must not draw here.
//!
//! # What `quick-xml` is trusted with, and what it is not
//!
//! It is configured strictly: end names checked, no unmatched ends, no bare
//! `&`, comments checked for `--`. Duplicate attributes are checked here, by
//! namespace and local name. It does not
//! check that a character is one XML allows, that a `<` is absent from an
//! attribute value, or that `]]>` is absent from text, so those are checked
//! here. It does not check the `Name` production either; a file that shows
//! that mattering adds the check here, with a test (ADR 0027 § 2).
//!
//! # Bounds, before the work
//!
//! Every bound is checked before the thing it bounds is built or decoded,
//! and passing one refuses the file: the bytes before the reader starts, the
//! elements and depth before an element is made, the attributes before the
//! next is read, and each length on the raw bytes, which decoding only ever
//! shortens.

use crate::document::Document;
use crate::name::{Namespace, QualifiedName};
use crate::node::{Attribute, Element, NodeId, NodeKind};
use core::fmt;
use quick_xml::NsReader;
use quick_xml::errors::Error;
use quick_xml::escape::{EscapeError, ParseCharRefError, resolve_predefined_entity};
use quick_xml::events::{BytesDecl, BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::{XmlVersion, events::attributes::Attribute as XmlAttribute};

/// The most bytes a file may have: one mebibyte. An icon is a few
/// kilobytes — Meet's hand is 13 684 bytes — and an illustration exported
/// from an editor a few hundred; a megabyte of vector markup is not a picture
/// anybody meant to put in an `<img>`.
pub const MOST_BYTES: usize = 1 << 20;

/// The most elements a document may hold: as many as `alo-svg` will walk in
/// one drawing (its `MOST_ELEMENTS`). Reading more than can be drawn is work
/// for nothing.
pub const MOST_ELEMENTS: usize = 65_536;

/// The deepest an element may be nested, the root being one deep: as deep as
/// `alo-svg` will walk (its `DEEPEST`). The open-element stack is ours, so
/// this is checked before anything deeper is made.
pub const DEEPEST: usize = 256;

/// The most attributes one element may carry. A drawn element needs a dozen;
/// an editor's export may carry a few dozen of its own. Duplicate checking is
/// quadratic in this, which is why it is bounded before that work.
pub const MOST_ATTRIBUTES: usize = 256;

/// The longest an element or attribute name may be, in bytes. SVG's longest
/// is `glyph-orientation-horizontal`, 28 bytes.
pub const LONGEST_NAME: usize = 1_024;

/// The longest an attribute value may be, in bytes as written. Path data is
/// the long one: Meet's hand has a `d` of 10 780 bytes, and a quarter of the
/// file bound leaves room for a path twenty-four times that.
pub const LONGEST_VALUE: usize = 256 * 1_024;

/// The longest a run of text — or a comment — may be, in bytes as written.
/// The long text in an SVG file is a `<style>` block, and one a quarter of the
/// file bound is a stylesheet nobody wrote for an icon.
pub const LONGEST_TEXT: usize = 256 * 1_024;

/// Why a file was refused as an SVG picture.
///
/// Every refusal in ADR 0027 § 3 and every bound in § 5 has its own variant,
/// so a page that hits one says which, and a test can pin each at its edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlRefusal {
    /// The file has more than [`MOST_BYTES`].
    TooLarge(usize),
    /// The bytes are not UTF-8; UTF-16 is the likely other.
    NotUtf8,
    /// The declaration names an encoding other than UTF-8.
    NotUtf8Declared(String),
    /// The declaration names a version other than 1.0.
    NotXml10(String),
    /// The document type declaration has an internal subset.
    InternalSubset,
    /// A document type declaration after the root or after another one.
    MisplacedDoctype,
    /// A reference to an entity that is not one of the five predefines.
    UndeclaredEntity(String),
    /// A character XML does not allow, written or referenced.
    IllegalCharacter(u32),
    /// No root element, a second one, or text or `CDATA` outside it.
    NotOneRoot,
    /// The root is not `svg` in the SVG namespace; it is named here.
    NotSvg(String),
    /// A prefix that no `xmlns` declaration in scope binds.
    UnboundPrefix(String),
    /// Two attributes with the same namespace and local name on one element.
    DuplicateAttribute(String),
    /// More than [`MOST_ELEMENTS`].
    TooManyElements,
    /// Deeper than [`DEEPEST`].
    TooDeep,
    /// More than [`MOST_ATTRIBUTES`] on one element.
    TooManyAttributes,
    /// A name longer than [`LONGEST_NAME`].
    NameTooLong,
    /// An attribute value longer than [`LONGEST_VALUE`].
    ValueTooLong,
    /// A run of text or a comment longer than [`LONGEST_TEXT`].
    TextTooLong,
    /// Anything else XML makes fatal, as the reader or this module put it,
    /// and the byte it was found at.
    Malformed {
        /// What was wrong.
        message: String,
        /// The byte offset it was found at.
        at: u64,
    },
}

impl fmt::Display for XmlRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge(bytes) => {
                write!(f, "{bytes} bytes is more than the {MOST_BYTES} allowed")
            }
            Self::NotUtf8 => f.write_str("the file is not UTF-8"),
            Self::NotUtf8Declared(name) => {
                write!(f, "the file declares the encoding {name:?}, not UTF-8")
            }
            Self::NotXml10(version) => write!(f, "the file declares XML {version:?}, not 1.0"),
            Self::InternalSubset => {
                f.write_str("the document type declaration has an internal subset")
            }
            Self::MisplacedDoctype => {
                f.write_str("a document type declaration is not before the root")
            }
            Self::UndeclaredEntity(name) => write!(f, "the entity &{name}; is not declared"),
            Self::IllegalCharacter(code) => write!(f, "U+{code:04X} is not allowed in XML"),
            Self::NotOneRoot => f.write_str("the file does not have exactly one root element"),
            Self::NotSvg(name) => write!(f, "the root is <{name}>, not an SVG <svg>"),
            Self::UnboundPrefix(prefix) => write!(f, "the prefix {prefix:?} is not declared"),
            Self::DuplicateAttribute(name) => write!(f, "the attribute {name} is repeated"),
            Self::TooManyElements => write!(f, "more than {MOST_ELEMENTS} elements"),
            Self::TooDeep => write!(f, "elements nested deeper than {DEEPEST}"),
            Self::TooManyAttributes => {
                write!(f, "more than {MOST_ATTRIBUTES} attributes on one element")
            }
            Self::NameTooLong => write!(f, "a name longer than {LONGEST_NAME} bytes"),
            Self::ValueTooLong => {
                write!(f, "an attribute value longer than {LONGEST_VALUE} bytes")
            }
            Self::TextTooLong => write!(f, "text longer than {LONGEST_TEXT} bytes"),
            Self::Malformed { message, at } => write!(f, "byte {at}: {message}"),
        }
    }
}

impl std::error::Error for XmlRefusal {}

/// Read an SVG file into a document of its own, or say why it is not one.
///
/// The document's root has the `svg` element as its one element child, with
/// any comments around it; processing instructions and the document type
/// declaration leave nothing in the tree. Whether the bytes were meant as SVG
/// at all — the resource's type — is the caller's to have decided
/// (ADR 0027 § 1); this decides only whether they are a well-formed SVG
/// document within the bounds.
///
/// # Errors
///
/// The first thing that makes the file not one, as an [`XmlRefusal`]. Any
/// input at all returns a document or a refusal, never a panic.
pub fn read_svg(bytes: &[u8]) -> Result<Document, XmlRefusal> {
    if bytes.len() > MOST_BYTES {
        return Err(XmlRefusal::TooLarge(bytes.len()));
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let text = core::str::from_utf8(bytes).map_err(|_| XmlRefusal::NotUtf8)?;

    let mut reader = NsReader::from_str(text);
    let config = reader.config_mut();
    config.check_end_names = true;
    config.allow_unmatched_ends = false;
    config.allow_dangling_amp = false;
    config.check_comments = true;
    config.expand_empty_elements = false;
    config.trim_text(false);

    let mut builder = Builder::new();
    loop {
        let event = reader
            .read_event()
            .map_err(|error| refusal_from(&error, reader.error_position()))?;
        let at = reader.buffer_position();
        match event {
            Event::Decl(decl) => builder.declaration(&decl, at)?,
            Event::DocType(doctype) => builder.doctype(&doctype)?,
            Event::PI(_) => builder.seen = true,
            Event::Comment(comment) => {
                check_length(comment.len(), LONGEST_TEXT, XmlRefusal::TextTooLong)?;
                let comment = comment
                    .xml10_content()
                    .map_err(|error| malformed(&error, at))?;
                builder.comment(&comment)?;
            }
            Event::Start(start) => builder.element(&start, &reader, true)?,
            Event::Empty(start) => builder.element(&start, &reader, false)?,
            Event::End(_) => builder.end(at)?,
            Event::Text(text) => {
                check_length(text.len(), LONGEST_TEXT, XmlRefusal::TextTooLong)?;
                let text = text
                    .xml10_content()
                    .map_err(|error| malformed(&error, at))?;
                if text.contains("]]>") {
                    return Err(XmlRefusal::Malformed {
                        message: "`]]>` in text".to_owned(),
                        at,
                    });
                }
                builder.text(&text, false)?;
            }
            Event::CData(data) => {
                check_length(data.len(), LONGEST_TEXT, XmlRefusal::TextTooLong)?;
                let data = data
                    .xml10_content()
                    .map_err(|error| malformed(&error, at))?;
                builder.text(&data, true)?;
            }
            Event::GeneralRef(reference) => {
                check_length(reference.len(), LONGEST_NAME, XmlRefusal::NameTooLong)?;
                let resolved = if reference.is_char_ref() {
                    let character = reference
                        .resolve_char_ref()
                        .map_err(|error| refusal_from(&error, at))?;
                    character.map(String::from)
                } else {
                    let name = reference.decode().map_err(|error| malformed(&error, at))?;
                    match resolve_predefined_entity(&name) {
                        Some(replacement) => Some(replacement.to_owned()),
                        None => return Err(XmlRefusal::UndeclaredEntity(name.into_owned())),
                    }
                };
                builder.text(resolved.as_deref().unwrap_or_default(), true)?;
            }
            Event::Eof => return builder.finish(at),
        }
    }
}

/// The document as it is built, and what the order of events has shown so
/// far.
struct Builder {
    document: Document,
    /// The open elements, innermost last. Ours rather than the reader's, so
    /// that [`DEEPEST`] is enforced before anything deeper is made.
    open: Vec<NodeId>,
    elements: usize,
    /// The bytes of text appended since the last thing that was not text, so
    /// that [`LONGEST_TEXT`] bounds a run however many events it came in.
    run: usize,
    /// Whether any event has been read: a declaration must come first.
    seen: bool,
    doctype: bool,
    root_done: bool,
}

impl Builder {
    fn new() -> Self {
        Self {
            document: Document::new(),
            open: Vec::new(),
            elements: 0,
            run: 0,
            seen: false,
            doctype: false,
            root_done: false,
        }
    }

    /// Where a node made now goes: the innermost open element, or the
    /// document.
    fn parent(&self) -> NodeId {
        self.open.last().copied().unwrap_or(self.document.root())
    }

    fn declaration(&mut self, decl: &BytesDecl<'_>, at: u64) -> Result<(), XmlRefusal> {
        if self.seen {
            return Err(XmlRefusal::Malformed {
                message: "an XML declaration that is not at the start".to_owned(),
                at,
            });
        }
        self.seen = true;
        let version = decl.version().map_err(|error| refusal_from(&error, at))?;
        if *version != *b"1.0" {
            return Err(XmlRefusal::NotXml10(
                String::from_utf8_lossy(&version).into_owned(),
            ));
        }
        match decl.encoding() {
            None => Ok(()),
            Some(Ok(name)) if name.eq_ignore_ascii_case(b"utf-8") => Ok(()),
            Some(Ok(name)) => Err(XmlRefusal::NotUtf8Declared(
                String::from_utf8_lossy(&name).into_owned(),
            )),
            Some(Err(error)) => Err(XmlRefusal::Malformed {
                message: error.to_string(),
                at,
            }),
        }
    }

    fn doctype(&mut self, doctype: &[u8]) -> Result<(), XmlRefusal> {
        if self.doctype || self.root_done || !self.open.is_empty() {
            return Err(XmlRefusal::MisplacedDoctype);
        }
        self.seen = true;
        self.doctype = true;
        if has_internal_subset(doctype) {
            return Err(XmlRefusal::InternalSubset);
        }
        Ok(())
    }

    fn comment(&mut self, comment: &str) -> Result<(), XmlRefusal> {
        self.seen = true;
        self.run = 0;
        check_characters(comment)?;
        let parent = self.parent();
        let id = self.document.create(NodeKind::Comment(comment.to_owned()));
        self.document.attach_last(parent, id);
        Ok(())
    }

    fn element(
        &mut self,
        start: &BytesStart<'_>,
        reader: &NsReader<&[u8]>,
        opens: bool,
    ) -> Result<(), XmlRefusal> {
        self.seen = true;
        self.run = 0;
        if self.root_done {
            return Err(XmlRefusal::NotOneRoot);
        }
        if self.elements >= MOST_ELEMENTS {
            return Err(XmlRefusal::TooManyElements);
        }
        if self.open.len() >= DEEPEST {
            return Err(XmlRefusal::TooDeep);
        }
        let at = reader.buffer_position();
        let qname = start.name();
        check_length(qname.as_ref().len(), LONGEST_NAME, XmlRefusal::NameTooLong)?;
        let (namespace, local) = reader.resolver().resolve_element(qname);
        let name = QualifiedName {
            ns: namespace_of(namespace)?,
            local: utf8(local.as_ref(), at)?.into(),
            prefix: qname
                .prefix()
                .map(|prefix| utf8(prefix.as_ref(), at).map(Into::into))
                .transpose()?,
        };
        if self.open.is_empty() && !(name.ns == Namespace::Svg && &*name.local == "svg") {
            return Err(XmlRefusal::NotSvg(utf8(qname.as_ref(), at)?.to_owned()));
        }
        let attrs = attributes(start, reader, at)?;

        self.elements += 1;
        let parent = self.parent();
        let id = self.document.create(NodeKind::Element(Element {
            name,
            attrs,
            template_contents: None,
            mathml_annotation_xml_integration_point: false,
            had_duplicate_attributes: false,
            declared: None,
        }));
        self.document.attach_last(parent, id);
        if opens {
            self.open.push(id);
        } else if self.open.is_empty() {
            self.root_done = true;
        }
        Ok(())
    }

    fn end(&mut self, at: u64) -> Result<(), XmlRefusal> {
        self.run = 0;
        if self.open.pop().is_none() {
            return Err(XmlRefusal::Malformed {
                message: "an end tag with nothing open".to_owned(),
                at,
            });
        }
        if self.open.is_empty() {
            self.root_done = true;
        }
        Ok(())
    }

    /// Append character data. Outside the root only white space is allowed,
    /// and only when it was written as text (`as_data` is false) rather than
    /// as `CDATA` or a reference, which belong inside an element.
    fn text(&mut self, text: &str, as_data: bool) -> Result<(), XmlRefusal> {
        self.seen = true;
        check_characters(text)?;
        let Some(&parent) = self.open.last() else {
            if as_data || !text.chars().all(|c| matches!(c, ' ' | '\t' | '\n' | '\r')) {
                return Err(XmlRefusal::NotOneRoot);
            }
            return Ok(());
        };
        self.run = self.run.saturating_add(text.len());
        check_length(self.run, LONGEST_TEXT, XmlRefusal::TextTooLong)?;
        if !text.is_empty() {
            self.document.append_text(parent, text);
        }
        Ok(())
    }

    fn finish(self, at: u64) -> Result<Document, XmlRefusal> {
        if !self.open.is_empty() {
            return Err(XmlRefusal::Malformed {
                message: "the file ended inside an element".to_owned(),
                at,
            });
        }
        if !self.root_done {
            return Err(XmlRefusal::NotOneRoot);
        }
        Ok(self.document)
    }
}

/// An element's attributes, named and valued, in the order written.
fn attributes(
    start: &BytesStart<'_>,
    reader: &NsReader<&[u8]>,
    at: u64,
) -> Result<Vec<Attribute>, XmlRefusal> {
    let mut attrs: Vec<Attribute> = Vec::new();
    // The reader's own duplicate check is off: it compares names as written,
    // and ours below compares namespace and local name, which is XML's rule
    // with namespaces and catches everything the reader's would — naming the
    // attribute, where the reader's error names only where it was.
    for attribute in start.attributes().with_checks(false) {
        if attrs.len() >= MOST_ATTRIBUTES {
            return Err(XmlRefusal::TooManyAttributes);
        }
        let attribute = attribute.map_err(|error| malformed(&error, at))?;
        let name = attribute_name(&attribute, reader, at)?;
        if attrs
            .iter()
            .any(|held| held.name.ns == name.ns && held.name.local == name.local)
        {
            return Err(XmlRefusal::DuplicateAttribute(
                utf8(attribute.key.as_ref(), at)?.to_owned(),
            ));
        }
        attrs.push(Attribute {
            name,
            value: attribute_value(&attribute, at)?,
        });
    }
    Ok(attrs)
}

fn attribute_name(
    attribute: &XmlAttribute<'_>,
    reader: &NsReader<&[u8]>,
    at: u64,
) -> Result<QualifiedName, XmlRefusal> {
    let key = attribute.key;
    check_length(key.as_ref().len(), LONGEST_NAME, XmlRefusal::NameTooLong)?;
    // The default namespace declaration is an attribute in the XMLNS
    // namespace, as the DOM has it; the reader resolves only prefixed names
    // to a namespace and leaves an unprefixed `xmlns` with none.
    if key.as_ref() == b"xmlns" {
        return Ok(QualifiedName::new(Namespace::XmlNs, "xmlns"));
    }
    let (namespace, local) = reader.resolver().resolve_attribute(key);
    Ok(QualifiedName {
        ns: namespace_of(namespace)?,
        local: utf8(local.as_ref(), at)?.into(),
        prefix: key
            .prefix()
            .map(|prefix| utf8(prefix.as_ref(), at).map(Into::into))
            .transpose()?,
    })
}

/// An attribute's value, normalised as XML 1.0 says, with only the five
/// predefined entities and character references resolved.
fn attribute_value(attribute: &XmlAttribute<'_>, at: u64) -> Result<String, XmlRefusal> {
    check_length(
        attribute.value.len(),
        LONGEST_VALUE,
        XmlRefusal::ValueTooLong,
    )?;
    if attribute.value.contains(&b'<') {
        return Err(XmlRefusal::Malformed {
            message: "a `<` in an attribute value".to_owned(),
            at,
        });
    }
    // Depth one: the predefines' replacements contain no references, so
    // nothing is ever expanded twice.
    let value = attribute
        .normalized_value_with(XmlVersion::Explicit1_0, 1, resolve_predefined_entity)
        .map_err(|error| refusal_from(&error, at))?;
    check_characters(&value)?;
    Ok(value.into_owned())
}

fn namespace_of(resolved: ResolveResult<'_>) -> Result<Namespace, XmlRefusal> {
    match resolved {
        ResolveResult::Bound(namespace) => Ok(Namespace::from_uri(&String::from_utf8_lossy(
            namespace.as_ref(),
        ))),
        ResolveResult::Unbound => Ok(Namespace::None),
        ResolveResult::Unknown(prefix) => Err(XmlRefusal::UnboundPrefix(
            String::from_utf8_lossy(&prefix).into_owned(),
        )),
    }
}

/// Whether a document type declaration's content has an internal subset: a
/// `[` outside the quoted identifiers.
fn has_internal_subset(doctype: &[u8]) -> bool {
    let mut quote = None;
    for &byte in doctype {
        match (quote, byte) {
            (None, b'"' | b'\'') => quote = Some(byte),
            (Some(open), _) if open == byte => quote = None,
            (None, b'[') => return true,
            _ => {}
        }
    }
    false
}

/// Refuse a character XML 1.0 does not allow (its `Char` production).
fn check_characters(text: &str) -> Result<(), XmlRefusal> {
    match text.chars().find(|&c| !is_xml_char(c)) {
        Some(c) => Err(XmlRefusal::IllegalCharacter(u32::from(c))),
        None => Ok(()),
    }
}

fn is_xml_char(c: char) -> bool {
    matches!(c,
        '\u{9}' | '\u{A}' | '\u{D}'
        | '\u{20}'..='\u{D7FF}'
        | '\u{E000}'..='\u{FFFD}'
        | '\u{10000}'..='\u{10FFFF}')
}

fn check_length(length: usize, most: usize, refusal: XmlRefusal) -> Result<(), XmlRefusal> {
    if length > most { Err(refusal) } else { Ok(()) }
}

fn utf8(bytes: &[u8], at: u64) -> Result<&str, XmlRefusal> {
    core::str::from_utf8(bytes).map_err(|error| malformed(&error, at))
}

fn malformed(error: &impl fmt::Display, at: u64) -> XmlRefusal {
    XmlRefusal::Malformed {
        message: error.to_string(),
        at,
    }
}

/// The reader's error as a refusal, keeping the named ones named.
fn refusal_from(error: &Error, at: u64) -> XmlRefusal {
    match error {
        Error::Escape(EscapeError::UnrecognizedEntity(_, name)) => {
            XmlRefusal::UndeclaredEntity(name.clone())
        }
        Error::Escape(EscapeError::InvalidCharRef(
            ParseCharRefError::IllegalCharacter(code) | ParseCharRefError::InvalidCodepoint(code),
        )) => XmlRefusal::IllegalCharacter(*code),
        other => malformed(other, at),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bracket_inside_a_quoted_identifier_is_not_a_subset() {
        assert!(!has_internal_subset(br#"svg SYSTEM "a[b].dtd""#));
        assert!(!has_internal_subset(
            br#"svg PUBLIC "-//W3C//DTD SVG 1.1//EN" 'x'"#
        ));
        assert!(has_internal_subset(
            br#"svg SYSTEM "a.dtd" [ <!ENTITY x "y"> ]"#
        ));
        assert!(has_internal_subset(b"svg ["));
    }

    #[test]
    fn the_characters_xml_allows_are_its_char_production() {
        for allowed in [
            '\t',
            '\n',
            '\r',
            ' ',
            '\u{D7FF}',
            '\u{E000}',
            '\u{FFFD}',
            '\u{10000}',
        ] {
            assert!(is_xml_char(allowed), "{allowed:?}");
        }
        for refused in ['\0', '\u{1}', '\u{B}', '\u{1F}', '\u{FFFE}', '\u{FFFF}'] {
            assert!(!is_xml_char(refused), "{refused:?}");
        }
    }

    #[test]
    fn every_refusal_says_what_it_is() {
        let refusal = XmlRefusal::Malformed {
            message: "unclosed tag".to_owned(),
            at: 7,
        };
        assert_eq!(refusal.to_string(), "byte 7: unclosed tag");
        assert_eq!(
            XmlRefusal::IllegalCharacter(1).to_string(),
            "U+0001 is not allowed in XML"
        );
    }
}
