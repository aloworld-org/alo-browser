//! A document goes in, the same document comes out.
//!
//! This is the honest test of a tree: if parsing and serialising disagree, one
//! of them has lost something, and the diff says which one. Every input here is
//! already in the form the serialiser writes, so any difference is a defect
//! rather than a normalisation.

use alo_dom::{Document, NodeKind, QualifiedName, parse_document, parse_fragment};

fn round_trips(html: &str) {
    let document = parse_document(html);
    assert_eq!(
        document.serialize_children(document.root()),
        html,
        "round trip changed the document",
    );
}

#[test]
fn a_whole_document_round_trips() {
    round_trips(concat!(
        "<!DOCTYPE html>",
        "<html lang=\"en\">",
        "<head><title>alo</title></head>",
        "<body><p class=\"lead\">alo <em>browser</em></p></body>",
        "</html>",
    ));
}

#[test]
fn attributes_keep_the_order_they_were_written_in() {
    round_trips(
        "<html><head></head><body><input type=\"text\" name=\"who\" value=\"\"></body></html>",
    );
}

#[test]
fn void_elements_are_written_without_an_end_tag() {
    // `<hr>` is not inside the paragraph: a block-level start tag closes an
    // open `<p>`, which is the parser being right rather than us being loose.
    round_trips("<html><head></head><body><p>a<br>b</p><hr></body></html>");
}

#[test]
fn comments_and_doctypes_survive() {
    round_trips("<!DOCTYPE html><!--a note--><html><head></head><body></body></html>");
}

#[test]
fn character_references_are_written_back() {
    round_trips("<html><head></head><body><p>a &amp; b &lt; c &gt; d&nbsp;e</p></body></html>");
}

#[test]
fn a_quote_in_an_attribute_value_is_escaped_and_an_angle_bracket_is_not() {
    let document = parse_document("<p title='1 &lt; 2 and &quot;three&quot;'></p>");
    assert!(
        document
            .serialize_children(document.root())
            .contains(r#"<p title="1 < 2 and &quot;three&quot;">"#),
        "got {}",
        document.serialize_children(document.root()),
    );
}

#[test]
fn raw_text_content_is_not_escaped() {
    round_trips("<html><head><style>.a > .b { --x: 1 }</style></head><body></body></html>");
}

#[test]
fn a_pre_keeps_the_newline_the_parser_swallows() {
    round_trips("<html><head></head><body><pre>\n\nindented\n</pre></body></html>");
}

#[test]
fn a_template_round_trips_through_its_contents() {
    round_trips("<html><head><template><p>row</p></template></head><body></body></html>");
}

#[test]
fn a_templates_contents_are_not_its_children() {
    let document = parse_document("<template><p>row</p></template>");
    let template =
        find_html_element(&document, "template").expect("the template element is in the head");
    assert_eq!(
        document.children(template).count(),
        0,
        "a template holds nothing directly",
    );

    let contents = document
        .element(template)
        .and_then(|element| element.template_contents)
        .expect("a template element is given a contents fragment");
    assert_eq!(document.kind(contents), Some(&NodeKind::Fragment));
    assert_eq!(document.text_content(contents), "row");
    assert!(
        !document.is_attached(contents),
        "the contents fragment is beside the tree, not in it",
    );
    assert_eq!(
        document.text_content(document.root()),
        "",
        "and so it contributes no text to the document",
    );
}

#[test]
fn foreign_content_keeps_its_namespace_and_round_trips() {
    let html = "<html><head></head><body><svg><circle r=\"4\"></circle></svg></body></html>";
    round_trips(html);

    let document = parse_document(html);
    let svg = find_html_element(&document, "body").expect("every document gets a body");
    let child = document
        .children(svg)
        .next()
        .expect("the svg element is in the body");
    let element = document.element(child).expect("it is an element");
    assert_eq!(element.name.ns, alo_dom::Namespace::Svg);
    assert_eq!(&*element.name.local, "svg");
}

#[test]
fn a_fragment_round_trips_without_the_parsers_scaffolding() {
    let document = parse_fragment("<li>one</li><li>two</li>", &QualifiedName::html("ul"));
    assert_eq!(
        document.serialize_children(document.root()),
        "<li>one</li><li>two</li>",
    );
    assert_eq!(document.children(document.root()).count(), 2);
    for child in document.children(document.root()) {
        assert_eq!(document.parent(child), Some(document.root()));
    }
}

#[test]
fn serialising_one_node_writes_that_node_and_its_subtree() {
    let document = parse_document("<div id=\"a\"><span>x</span></div>");
    let div = find_html_element(&document, "div").expect("the div we wrote is in the body");
    assert_eq!(
        document.serialize_node(div),
        "<div id=\"a\"><span>x</span></div>",
    );
    assert_eq!(document.serialize_children(div), "<span>x</span>");
}

#[test]
fn ids_are_stable_across_a_read_of_the_whole_tree() {
    let document = parse_document("<!DOCTYPE html><p>a</p><p>b</p>");
    let first: Vec<_> = document.descendants(document.root()).collect();
    let second: Vec<_> = document.descendants(document.root()).collect();
    assert_eq!(first, second, "reading the tree twice names the same nodes");

    let mut ids: Vec<_> = first.iter().map(|id| id.as_usize()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), first.len(), "no id names two nodes");
    assert!(
        first.iter().all(|id| document.get(*id).is_some()),
        "every id read from the tree resolves in it",
    );
}

/// The first HTML element with this local name, in document order.
fn find_html_element(document: &Document, local: &str) -> Option<alo_dom::NodeId> {
    document.descendants(document.root()).find(|id| {
        document
            .get(*id)
            .is_some_and(|node| node.is_html_element(local))
    })
}
