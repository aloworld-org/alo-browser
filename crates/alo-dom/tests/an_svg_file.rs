/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An SVG file, read into a document of its own (queue item 309, ADR 0027).
//!
//! Three things close the item, and each has its part of this file:
//!
//! - **Meet's hand reads.** `alo-waving-hand.svg`, frozen byte for byte in
//!   `alo-corpus/pictures/`, becomes `svg`, `title`, `g` and two `path`s in
//!   the SVG namespace, with the attributes the file wrote.
//! - **Every refusal and every bound is named and pinned at its edge**: the
//!   largest thing allowed reads, and one more is refused with its name.
//! - **Hostile bytes refuse rather than panic**: every prefix of the hand,
//!   every byte of it flipped, and the classic attacks — a billion laughs, an
//!   external entity, a depth bomb, an attribute storm.

use alo_dom::xml::{
    DEEPEST, LONGEST_NAME, LONGEST_TEXT, LONGEST_VALUE, MOST_ATTRIBUTES, MOST_BYTES, MOST_ELEMENTS,
};
use alo_dom::{Document, Namespace, NodeId, NodeKind, XmlRefusal, read_svg};
use std::path::PathBuf;

const SVG: &str = "http://www.w3.org/2000/svg";

/// Where the frozen hand is. Read by path, because `alo-corpus` depends on
/// this crate and not the other way round.
fn where_the_hand_is() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../alo-corpus/pictures/alo-waving-hand/picture.svg")
}

/// The frozen hand, or nothing if it is not there —
/// `the_hand_is_frozen_where_this_looks` is the test that says so.
fn the_hand() -> Vec<u8> {
    std::fs::read(where_the_hand_is()).unwrap_or_default()
}

/// A file whose root is an SVG `<svg>` holding `inner`.
fn svg(inner: &str) -> String {
    format!(r#"<svg xmlns="{SVG}">{inner}</svg>"#)
}

/// `count` distinct empty attributes, ` a0="" a1="" …`.
fn attributes(count: usize) -> String {
    (0..count).fold(String::new(), |mut written, n| {
        use std::fmt::Write as _;
        let _ = write!(written, " a{n}=\"\"");
        written
    })
}

fn read(text: &str) -> Result<Document, XmlRefusal> {
    read_svg(text.as_bytes())
}

/// The element children of a node, by local name.
fn element_children(document: &Document, id: NodeId) -> Vec<NodeId> {
    document
        .children(id)
        .filter(|child| document.element(*child).is_some())
        .collect()
}

fn local_name(document: &Document, id: NodeId) -> String {
    document
        .element(id)
        .map(|element| element.name.local.to_string())
        .unwrap_or_default()
}

/// The root `<svg>` of a document read without refusal.
fn root_element(document: &Document) -> Option<NodeId> {
    element_children(document, document.root()).first().copied()
}

fn refusal(text: &str) -> Option<XmlRefusal> {
    read(text).err()
}

// --- Meet's hand ----------------------------------------------------------

#[test]
fn the_hand_is_frozen_where_this_looks() {
    assert_eq!(
        the_hand().len(),
        13_684,
        "{} should be alo-workplace's file, byte for byte",
        where_the_hand_is().display(),
    );
}

#[test]
fn meets_hand_reads_into_svg_title_g_and_two_paths() {
    let document = read_svg(&the_hand()).expect("the hand is a well-formed SVG file");
    let svg = root_element(&document).expect("a root element");
    let root = document.element(svg).expect("an element");
    assert_eq!(root.name.ns, Namespace::Svg);
    assert_eq!(&*root.name.local, "svg");
    assert_eq!(root.attr("viewBox"), Some("0 0 395 385"));
    assert_eq!(root.attr("role"), Some("img"));
    assert_eq!(root.attr("aria-label"), Some("Alo waving hand icon"));
    assert_eq!(root.attr("width"), None);

    let children = element_children(&document, svg);
    let names: Vec<String> = children
        .iter()
        .map(|id| local_name(&document, *id))
        .collect();
    assert_eq!(names, ["title", "g"]);
    assert_eq!(document.text_content(children[0]), "Alo waving hand");

    let group = document.element(children[1]).expect("the <g>");
    assert_eq!(group.attr("fill-rule"), Some("evenodd"));
    assert_eq!(group.attr("clip-rule"), Some("evenodd"));

    let paths = element_children(&document, children[1]);
    assert_eq!(paths.len(), 2);
    let described: Vec<(String, Option<String>, usize)> = paths
        .iter()
        .map(|id| {
            let path = document.element(*id).expect("a <path>");
            (
                path.name.local.to_string(),
                path.attr("fill").map(str::to_owned),
                path.attr("d").map_or(0, str::len),
            )
        })
        .collect();
    assert_eq!(
        described,
        [
            ("path".to_owned(), Some("#E76F51".to_owned()), 10_780),
            ("path".to_owned(), Some("#102A43".to_owned()), 2_639),
        ],
    );
    for id in document.descendants(document.root()) {
        if let Some(element) = document.element(id) {
            assert_eq!(element.name.ns, Namespace::Svg, "{}", element.name.local);
        }
    }
}

#[test]
fn the_default_namespace_declaration_is_an_xmlns_attribute() {
    let document = read_svg(&the_hand()).expect("the hand reads");
    let svg = root_element(&document).expect("a root");
    let declaration = document
        .element(svg)
        .and_then(|element| element.attrs.first())
        .expect("xmlns is written first");
    assert_eq!(declaration.name.ns, Namespace::XmlNs);
    assert_eq!(&*declaration.name.local, "xmlns");
    assert_eq!(declaration.value, SVG);
}

// --- hostile bytes ----------------------------------------------------------

#[test]
fn every_prefix_of_the_hand_is_refused_until_the_root_closes() {
    let hand = the_hand();
    let closed = hand
        .windows(6)
        .position(|window| window == b"</svg>")
        .map(|at| at + 6)
        .expect("the hand ends its root");
    for length in 0..=hand.len() {
        let result = read_svg(&hand[..length]);
        assert_eq!(
            result.is_ok(),
            length >= closed,
            "a prefix of {length} bytes: {:?}",
            result.err(),
        );
    }
}

#[test]
fn every_flipped_byte_of_the_hand_is_a_document_or_a_refusal() {
    let hand = the_hand();
    assert!(!hand.is_empty());
    for mask in [0xFF_u8, 0x20, 0x80] {
        let mut flipped = hand.clone();
        for at in 0..flipped.len() {
            flipped[at] ^= mask;
            // Either answer will do; what may not happen is a panic.
            let _answer = read_svg(&flipped);
            flipped[at] ^= mask;
        }
    }
}

#[test]
fn a_billion_laughs_is_refused_at_its_internal_subset() {
    let bomb = concat!(
        r#"<?xml version="1.0"?>"#,
        r#"<!DOCTYPE svg [ <!ENTITY a "lol"> <!ENTITY b "&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;">"#,
        r#" <!ENTITY c "&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;"> ]>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text>&c;</text></svg>"#,
    );
    assert_eq!(refusal(bomb), Some(XmlRefusal::InternalSubset));
}

#[test]
fn an_external_entity_is_refused_as_undeclared() {
    assert_eq!(
        refusal(&svg("<desc>&secret;</desc>")),
        Some(XmlRefusal::UndeclaredEntity("secret".to_owned())),
    );
    assert_eq!(
        refusal(&svg(r#"<g id="&secret;"/>"#)),
        Some(XmlRefusal::UndeclaredEntity("secret".to_owned())),
    );
}

#[test]
fn a_depth_bomb_is_refused_at_the_depth_bound() {
    let deep = format!(r#"<svg xmlns="{SVG}">{}"#, "<g>".repeat(200_000));
    assert_eq!(refusal(&deep), Some(XmlRefusal::TooDeep));
}

#[test]
fn an_attribute_storm_is_refused_at_the_attribute_bound() {
    let storm = attributes(100_000);
    assert_eq!(
        refusal(&svg(&format!("<g{storm}/>"))),
        Some(XmlRefusal::TooManyAttributes),
    );
}

// --- the bounds, each at its edge -------------------------------------------

#[test]
fn the_byte_bound_admits_its_last_byte_and_refuses_the_next() {
    // Filled with comments, each inside the text bound, so that the byte
    // bound is the only one this reaches.
    let comment = format!("<!--{}-->", "x".repeat(LONGEST_TEXT - 1));
    let mut file = svg(&comment.repeat(MOST_BYTES / comment.len()));
    file.push_str(&" ".repeat(MOST_BYTES - file.len()));
    assert_eq!(file.len(), MOST_BYTES);
    assert!(read(&file).is_ok(), "{:?}", refusal(&file));
    file.push(' ');
    assert_eq!(refusal(&file), Some(XmlRefusal::TooLarge(MOST_BYTES + 1)));
}

#[test]
fn the_element_bound_counts_the_root() {
    let at = svg(&"<g/>".repeat(MOST_ELEMENTS - 1));
    assert!(read(&at).is_ok());
    let over = svg(&"<g/>".repeat(MOST_ELEMENTS));
    assert_eq!(refusal(&over), Some(XmlRefusal::TooManyElements));
}

#[test]
fn the_depth_bound_counts_the_root_as_one_deep() {
    let nest = |depth: usize| {
        svg(&format!(
            "{}{}",
            "<g>".repeat(depth - 1),
            "</g>".repeat(depth - 1)
        ))
    };
    assert!(read(&nest(DEEPEST)).is_ok());
    assert_eq!(refusal(&nest(DEEPEST + 1)), Some(XmlRefusal::TooDeep));
}

#[test]
fn the_attribute_bound_is_per_element() {
    assert!(read(&svg(&format!("<g{}/>", attributes(MOST_ATTRIBUTES)))).is_ok());
    assert_eq!(
        refusal(&svg(&format!("<g{}/>", attributes(MOST_ATTRIBUTES + 1)))),
        Some(XmlRefusal::TooManyAttributes),
    );
}

#[test]
fn the_name_bound_holds_for_elements_and_attributes() {
    let name = |length: usize| "n".repeat(length);
    assert!(read(&svg(&format!("<{0}></{0}>", name(LONGEST_NAME)))).is_ok());
    assert_eq!(
        refusal(&svg(&format!("<{0}></{0}>", name(LONGEST_NAME + 1)))),
        Some(XmlRefusal::NameTooLong),
    );
    assert!(read(&svg(&format!(r#"<g {}=""/>"#, name(LONGEST_NAME)))).is_ok());
    assert_eq!(
        refusal(&svg(&format!(r#"<g {}=""/>"#, name(LONGEST_NAME + 1)))),
        Some(XmlRefusal::NameTooLong),
    );
}

#[test]
fn the_value_bound_is_on_the_value_as_written() {
    let value = |length: usize| format!(r#"<path d="{}"/>"#, "M".repeat(length));
    assert!(read(&svg(&value(LONGEST_VALUE))).is_ok());
    assert_eq!(
        refusal(&svg(&value(LONGEST_VALUE + 1))),
        Some(XmlRefusal::ValueTooLong),
    );
}

#[test]
fn the_text_bound_is_on_a_run_however_many_pieces_it_came_in() {
    let text = |length: usize| format!("<style>{}</style>", "x".repeat(length));
    assert!(read(&svg(&text(LONGEST_TEXT))).is_ok());
    assert_eq!(
        refusal(&svg(&text(LONGEST_TEXT + 1))),
        Some(XmlRefusal::TextTooLong),
    );
    let half = "x".repeat(LONGEST_TEXT / 2);
    let pieces = svg(&format!("<style>{half}&amp;{half}</style>"));
    assert_eq!(refusal(&pieces), Some(XmlRefusal::TextTooLong));
    let comment = svg(&format!("<!--{}-->", "x".repeat(LONGEST_TEXT + 1)));
    assert_eq!(refusal(&comment), Some(XmlRefusal::TextTooLong));
}

// --- what is accepted, and what is refused by name (ADR 0027 § 3) -----------

#[test]
fn utf8_is_read_with_or_without_its_byte_order_mark() {
    let with_mark = [b"\xEF\xBB\xBF".as_slice(), svg("").as_bytes()].concat();
    assert!(read_svg(&with_mark).is_ok());
    assert!(
        read(r#"<?xml version="1.0" encoding="UTF-8"?><svg xmlns="http://www.w3.org/2000/svg"/>"#)
            .is_ok()
    );
    assert!(
        read(r#"<?xml version="1.0" encoding="utf-8"?><svg xmlns="http://www.w3.org/2000/svg"/>"#)
            .is_ok()
    );
}

#[test]
fn utf16_is_refused() {
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain(svg("").encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    assert_eq!(read_svg(&utf16).err(), Some(XmlRefusal::NotUtf8));
}

#[test]
fn another_declared_encoding_is_refused() {
    assert_eq!(
        refusal(
            r#"<?xml version="1.0" encoding="ISO-8859-1"?><svg xmlns="http://www.w3.org/2000/svg"/>"#
        ),
        Some(XmlRefusal::NotUtf8Declared("ISO-8859-1".to_owned())),
    );
}

#[test]
fn only_version_one_point_zero_is_read() {
    assert!(read(r#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"/>"#).is_ok());
    assert_eq!(
        refusal(r#"<?xml version="1.1"?><svg xmlns="http://www.w3.org/2000/svg"/>"#),
        Some(XmlRefusal::NotXml10("1.1".to_owned())),
    );
}

#[test]
fn a_declaration_anywhere_but_the_start_is_malformed() {
    assert!(matches!(
        refusal(r#" <?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"/>"#),
        Some(XmlRefusal::Malformed { .. }),
    ));
}

#[test]
fn an_external_doctype_is_ignored_and_never_kept() {
    let document = read(concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        "\n",
        r#"<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">"#,
        "\n",
        r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#,
    ))
    .expect("an external identifier alone is accepted");
    let kinds: Vec<bool> = document
        .children(document.root())
        .map(|id| matches!(document.kind(id), Some(NodeKind::Doctype { .. })))
        .collect();
    assert_eq!(kinds, [false], "the root's only child is the <svg>");
}

#[test]
fn a_doctype_after_the_root_or_twice_is_misplaced() {
    assert_eq!(
        refusal(&format!("{}<!DOCTYPE svg>", svg(""))),
        Some(XmlRefusal::MisplacedDoctype),
    );
    assert_eq!(
        refusal(&format!("<!DOCTYPE svg><!DOCTYPE svg>{}", svg(""))),
        Some(XmlRefusal::MisplacedDoctype),
    );
}

#[test]
fn the_five_predefines_and_character_references_are_resolved() {
    let document = read(&svg(
        r#"<desc a="&lt;&#x41;&#66;&quot;">&lt;&gt;&amp;&apos;&quot;&#x263A;</desc>"#,
    ))
    .expect("references XML defines");
    let svg = root_element(&document).expect("a root");
    let desc = element_children(&document, svg)[0];
    assert_eq!(document.text_content(desc), "<>&'\"\u{263A}");
    assert_eq!(
        document.element(desc).and_then(|element| element.attr("a")),
        Some("<AB\""),
    );
    assert_eq!(
        document.children(desc).count(),
        1,
        "text and references are one text node",
    );
}

#[test]
fn a_character_xml_does_not_allow_is_refused_however_it_is_written() {
    assert_eq!(
        refusal(&svg("<desc>&#1;</desc>")),
        Some(XmlRefusal::IllegalCharacter(1))
    );
    assert_eq!(
        refusal(&svg("<desc>&#0;</desc>")),
        Some(XmlRefusal::IllegalCharacter(0))
    );
    assert_eq!(
        refusal(&svg("<desc>\u{1}</desc>")),
        Some(XmlRefusal::IllegalCharacter(1)),
    );
    assert_eq!(
        refusal(&svg(r#"<g id="&#xFFFE;"/>"#)),
        Some(XmlRefusal::IllegalCharacter(0xFFFE)),
    );
    assert_eq!(
        refusal(&svg("<!--\u{B}-->")),
        Some(XmlRefusal::IllegalCharacter(0xB)),
    );
}

#[test]
fn there_is_exactly_one_root() {
    assert_eq!(refusal(""), Some(XmlRefusal::NotOneRoot));
    assert_eq!(refusal("<!-- nothing -->"), Some(XmlRefusal::NotOneRoot));
    assert_eq!(
        refusal(&format!("{0}{0}", svg(""))),
        Some(XmlRefusal::NotOneRoot),
    );
    assert_eq!(
        refusal(&format!("words{}", svg(""))),
        Some(XmlRefusal::NotOneRoot),
    );
    assert_eq!(
        refusal(&format!("{}<![CDATA[x]]>", svg(""))),
        Some(XmlRefusal::NotOneRoot),
    );
    assert_eq!(
        refusal(&format!("&amp;{}", svg(""))),
        Some(XmlRefusal::NotOneRoot),
    );
    assert!(read(&format!("\n<!-- before --> {}\n<!-- after -->\n", svg(""))).is_ok());
}

#[test]
fn the_root_must_be_svg_in_the_svg_namespace() {
    assert_eq!(
        refusal("<svg/>"),
        Some(XmlRefusal::NotSvg("svg".to_owned())),
    );
    assert_eq!(
        refusal(r#"<html xmlns="http://www.w3.org/1999/xhtml"/>"#),
        Some(XmlRefusal::NotSvg("html".to_owned())),
    );
    assert_eq!(
        refusal(&format!(r#"<g xmlns="{SVG}"/>"#)),
        Some(XmlRefusal::NotSvg("g".to_owned())),
    );
    let prefixed = read(&format!(r#"<s:svg xmlns:s="{SVG}"><s:g/></s:svg>"#))
        .expect("a prefixed SVG root is an SVG root");
    let root = root_element(&prefixed).expect("a root");
    let element = prefixed.element(root).expect("an element");
    assert_eq!(element.name.ns, Namespace::Svg);
    assert_eq!(element.name.prefix.as_deref(), Some("s"));
}

#[test]
fn every_prefix_must_be_declared() {
    assert_eq!(
        refusal(&svg("<x:g/>")),
        Some(XmlRefusal::UnboundPrefix("x".to_owned())),
    );
    assert_eq!(
        refusal(&svg(r##"<use x:href="#a"/>"##)),
        Some(XmlRefusal::UnboundPrefix("x".to_owned())),
    );
    let document = read(&format!(
        r##"<svg xmlns="{SVG}" xmlns:xlink="http://www.w3.org/1999/xlink"><use xlink:href="#a"/></svg>"##
    ))
    .expect("a declared prefix");
    let svg = root_element(&document).expect("a root");
    let href = document
        .element(element_children(&document, svg)[0])
        .and_then(|element| element.attrs.first())
        .expect("the href");
    assert_eq!(href.name.ns, Namespace::XLink);
    assert_eq!(&*href.name.local, "href");
    assert_eq!(href.name.prefix.as_deref(), Some("xlink"));
}

#[test]
fn an_attribute_may_not_be_repeated_by_name_or_by_namespace() {
    assert_eq!(
        refusal(&svg(r#"<g x="1" x="2"/>"#)),
        Some(XmlRefusal::DuplicateAttribute("x".to_owned())),
    );
    assert_eq!(
        refusal(&svg(
            r#"<g xmlns:a="urn:alo" xmlns:b="urn:alo" a:x="1" b:x="2"/>"#
        )),
        Some(XmlRefusal::DuplicateAttribute("b:x".to_owned())),
    );
}

#[test]
fn what_the_reader_finds_malformed_refuses_the_whole_file() {
    for broken in [
        svg("<g></h>"),
        svg("<g>"),
        format!("{}</g>", svg("")),
        svg("<desc>a & b</desc>"),
        svg("<!-- a -- b -->"),
        svg(r#"<g id="<"/>"#),
        svg("<desc>]]></desc>"),
        svg(r#"<g id="unterminated/>"#),
    ] {
        assert!(
            matches!(refusal(&broken), Some(XmlRefusal::Malformed { .. })),
            "{broken}: {:?}",
            refusal(&broken),
        );
    }
}

#[test]
fn processing_instructions_are_ignored_and_cdata_is_text() {
    let document = read(&format!(
        r#"<?xml-stylesheet href="evil.css"?>{}"#,
        svg("<style><![CDATA[path { fill: red }]]></style><?ignored?>")
    ))
    .expect("a stylesheet instruction is ignored");
    let svg = root_element(&document).expect("a root");
    let style = element_children(&document, svg)[0];
    assert_eq!(document.text_content(style), "path { fill: red }");
    let instructions = document
        .descendants(document.root())
        .filter(|id| {
            matches!(
                document.kind(*id),
                Some(NodeKind::ProcessingInstruction { .. })
            )
        })
        .count();
    assert_eq!(instructions, 0);
}

#[test]
fn elements_of_other_namespaces_and_comments_are_kept() {
    let document = read(&format!(
        r#"<svg xmlns="{SVG}" xmlns:sodipodi="urn:sodipodi"><!-- an editor --><sodipodi:namedview/></svg>"#
    ))
    .expect("an editor's metadata is kept");
    let svg = root_element(&document).expect("a root");
    let kept: Vec<String> = document
        .children(svg)
        .map(|id| match document.kind(id) {
            Some(NodeKind::Comment(text)) => format!("<!--{text}-->"),
            Some(NodeKind::Element(element)) => {
                format!("{}|{}", element.name.ns, element.name.local)
            }
            _ => String::new(),
        })
        .collect();
    assert_eq!(kept, ["<!-- an editor -->", "urn:sodipodi|namedview"]);
}

#[test]
fn line_ends_and_attribute_white_space_are_normalised_as_xml_says() {
    let document = read(&svg("<desc a=\"x\ty\r\nz\">one\r\ntwo\rthree</desc>"))
        .expect("white space is not an error");
    let svg = root_element(&document).expect("a root");
    let desc = element_children(&document, svg)[0];
    assert_eq!(document.text_content(desc), "one\ntwo\nthree");
    assert_eq!(
        document.element(desc).and_then(|element| element.attr("a")),
        Some("x y z"),
    );
}
