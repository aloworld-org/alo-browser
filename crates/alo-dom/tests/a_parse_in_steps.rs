/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 247, `alo-dom`'s half (ADR 0017 § 7): a document parsed a step
//! at a time, each step ending at a script's end tag, so the page's script can
//! run against the document parsed so far.
//!
//! What is asserted here is the parser's promise to whoever runs the script:
//! at each stop the script's element is the last thing parsed and nothing
//! after it is there; a parse in steps builds exactly what a parse in one go
//! does, ids and all; what changed the document between steps is kept, and
//! the parser carries on around it; and nothing the parser still holds is
//! released under it.

use alo_dom::{Document, NodeId, Parsing, Reached, parse_document};

/// The first element named `name`, in tree order.
fn first(document: &Document, name: &str) -> Option<NodeId> {
    document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.name.is_html(name))
    })
}

/// Parse `html` in steps, with nothing done between them, and say what each
/// stop's script was and the whole document.
fn in_steps(html: &str) -> (Vec<NodeId>, Document) {
    let (mut parsing, mut document) = Parsing::start(html);
    let mut stops = Vec::new();
    while let Reached::Script(script) = parsing.resume(&mut document) {
        stops.push(script);
    }
    (stops, document)
}

#[test]
fn each_step_ends_at_a_scripts_end_tag_with_nothing_after_it() {
    let (mut parsing, mut document) = Parsing::start(
        "<!DOCTYPE html><body><p>a</p><script>one</script><p>b</p>\
         <script>two</script><p>c</p>",
    );
    let Reached::Script(one) = parsing.resume(&mut document) else {
        panic!("no stop at the first script");
    };
    let body = first(&document, "body");
    assert_eq!(body.and_then(|body| document.last_child(body)), Some(one));
    assert_eq!(
        body.map(|body| document.serialize_node(body)),
        Some("<body><p>a</p><script>one</script></body>".to_owned()),
        "the script's own element is the last thing parsed, and `b` is not \
         there yet",
    );
    assert!(document.is_being_parsed());

    let Reached::Script(two) = parsing.resume(&mut document) else {
        panic!("no stop at the second script");
    };
    assert_eq!(body.and_then(|body| document.last_child(body)), Some(two));
    assert_eq!(document.text_content(two), "two");
    assert_eq!(
        body.map(|body| document.serialize_node(body)),
        Some("<body><p>a</p><script>one</script><p>b</p><script>two</script></body>".to_owned()),
    );

    assert_eq!(parsing.resume(&mut document), Reached::End);
    assert!(!document.is_being_parsed());
    assert_eq!(
        body.map(|body| document.serialize_node(body)),
        Some(
            "<body><p>a</p><script>one</script><p>b</p><script>two</script><p>c</p></body>"
                .to_owned()
        ),
    );
    assert_eq!(
        parsing.resume(&mut document),
        Reached::End,
        "and stays ended"
    );
}

#[test]
fn a_parse_in_steps_builds_what_a_parse_in_one_go_does() {
    for html in [
        "",
        "<p>no script at all",
        "<script>a</script><script>b</script>",
        "<head><script>h</script><meta charset=utf-8></head><body>x<script>y</script>z",
        // The tree builder's repairs, with a stop in the middle of each.
        "<table><script>t</script><tr><td>cell<script>c</script></table>",
        "<b><i>bold<script>s</script>both</b>italic</i>",
        "<template><script>inert</script></template><svg><script>s</script></svg>",
        "<select><script>o</script><option>one</select>",
        "<script type=text/plain>data</script><script>unterminated",
    ] {
        let (_, stepped) = in_steps(html);
        let whole = parse_document(html);
        assert_eq!(
            stepped.serialize_node(stepped.root()),
            whole.serialize_node(whole.root()),
            "{html:?}"
        );
        assert_eq!(stepped.node_count(), whole.node_count(), "{html:?}");
        assert_eq!(stepped.issues(), whole.issues(), "{html:?}");
    }
}

#[test]
fn the_parser_stops_at_every_script_end_tag_and_never_at_an_unterminated_one() {
    // Which of these is a script to run is `scripts::prepared`'s question:
    // the parser stops at every `</script>` the tree builder reaches.
    let (stops, document) = in_steps(
        "<script type=text/plain>data</script><template><script>t</script></template>\
         <script>last</script><script>never closed",
    );
    let texts: Vec<String> = stops.iter().map(|id| document.text_content(*id)).collect();
    assert_eq!(texts, vec!["data", "t", "last"]);
}

#[test]
fn what_changed_between_steps_is_kept_and_the_parser_carries_on_around_it() {
    let (mut parsing, mut document) =
        Parsing::start("<body><ul><li>one</li><script>s</script><li>three</li></ul><p>after");
    let Reached::Script(script) = parsing.resume(&mut document) else {
        panic!("no stop");
    };
    // What a script does at its end tag: a node of its own, beside itself.
    let list = document.parent(script);
    let made = document.create_element("li");
    let (Some(list), Ok(made)) = (list, made) else {
        panic!("no list to grow");
    };
    let text = document.create_text_node("two");
    assert!(document.append_child(made, text).is_ok());
    assert!(document.append_child(list, made).is_ok());
    let ids_before = document.node_count();

    assert_eq!(parsing.resume(&mut document), Reached::End);
    assert_eq!(
        document.serialize_node(list),
        "<ul><li>one</li><script>s</script><li>two</li><li>three</li></ul>",
        "the row the script made is where it put it, and the parser's next \
         row follows it"
    );
    // ADR 0003: one counter. The script's nodes came before everything the
    // parser made after them.
    assert!(made.as_usize() < ids_before);
    let after = first(&document, "p");
    assert!(after.is_some_and(|after| after.as_usize() >= ids_before));
}

#[test]
fn a_parser_whose_open_element_a_script_took_out_inserts_into_it_still() {
    let (mut parsing, mut document) = Parsing::start(
        "<!DOCTYPE html><html><body><div>kept</div><script>s</script><p>after</p>\
         <script>in a tree nobody sees</script></body></html>text after html",
    );
    let Reached::Script(_) = parsing.resume(&mut document) else {
        panic!("no stop");
    };
    let issues = document.issues().len();
    let body = first(&document, "body");
    let Some(body) = body else {
        panic!("no body");
    };
    assert!(document.remove(body), "a script takes the body out");

    // A collection now would find a detached tree nothing wraps, and it is
    // the parser's open element: it is not released while the parse lasts.
    assert_eq!(document.release(body), None);
    let Reached::Script(second) = parsing.resume(&mut document) else {
        panic!("no stop at the second script");
    };
    assert!(!document.is_attached(second));
    assert_eq!(parsing.resume(&mut document), Reached::End);

    // The parser went on inserting into the body it had open, as HTML's
    // does, and every repair it made was one it would have made anyway: it
    // never asked a node that was not there for its name.
    assert_eq!(
        document.serialize_node(body),
        "<body><div>kept</div><script>s</script><p>after</p>\
         <script>in a tree nobody sees</script>text after html</body>"
    );
    assert!(
        document
            .issues()
            .iter()
            .skip(issues)
            .all(|issue| !issue.message.contains("tree builder called")),
        "{:?}",
        document.issues()
    );
    let html = first(&document, "html");
    assert_eq!(
        html.map(|html| document.serialize_node(html)),
        Some("<html><head></head></html>".to_owned())
    );

    // And once the parse is over, what nothing holds can go: the body, its
    // four children and their five texts.
    assert_eq!(document.release(body), Some(10));
}

#[test]
fn a_document_the_parse_did_not_start_is_not_built_into() {
    let (mut parsing, mut document) = Parsing::start("<p>mine</p><script>s</script><p>more");
    let mut stranger = parse_document("<p>stranger</p>");
    let before = stranger.serialize_node(stranger.root());
    assert_eq!(parsing.resume(&mut stranger), Reached::End);
    assert_eq!(stranger.serialize_node(stranger.root()), before);

    // The parse it did start is untouched by the refusal.
    assert!(matches!(parsing.resume(&mut document), Reached::Script(_)));
    assert_eq!(parsing.resume(&mut document), Reached::End);
    assert_eq!(
        document.serialize_node(document.root()),
        "<html><head></head><body><p>mine</p><script>s</script><p>more</p></body></html>"
    );
}

#[test]
fn every_meta_is_handed_over_once_in_the_order_it_was_made() {
    let (mut parsing, mut document) = Parsing::start(
        "<head><meta charset=utf-8><meta name=a><script>s</script>\
         <meta http-equiv=content-security-policy content=x></head>\
         <body><meta name=b>",
    );
    let names = |document: &Document, metas: Vec<NodeId>| -> Vec<String> {
        metas
            .into_iter()
            .map(|id| {
                document
                    .element(id)
                    .and_then(|element| {
                        element
                            .attr("name")
                            .or_else(|| element.attr("charset"))
                            .or_else(|| element.attr("http-equiv"))
                    })
                    .unwrap_or("?")
                    .to_owned()
            })
            .collect()
    };
    assert!(matches!(parsing.resume(&mut document), Reached::Script(_)));
    let metas = parsing.take_metas();
    assert_eq!(names(&document, metas), vec!["utf-8", "a"]);
    assert!(parsing.take_metas().is_empty(), "each handed over once");
    assert_eq!(parsing.resume(&mut document), Reached::End);
    let metas = parsing.take_metas();
    assert_eq!(
        names(&document, metas),
        vec!["content-security-policy", "b"]
    );
}

// --- Hostile markup ----------------------------------------------------------

#[test]
fn every_prefix_of_a_page_with_scripts_parses_in_steps_to_the_same_document() {
    let whole = "<!DOCTYPE html><head><meta http-equiv=Content-Security-Policy content=x>\
                 <script>a</script></head><body><table><script>b</script><tr><td>\
                 <script>c</script>cell</table><template><script>t</script></template>\
                 <svg><script>s</script></svg><script>d";
    for (at, _) in whole.char_indices() {
        let Some(prefix) = whole.get(..at) else {
            continue;
        };
        let (stops, stepped) = in_steps(prefix);
        let at_once = parse_document(prefix);
        assert_eq!(
            stepped.serialize_node(stepped.root()),
            at_once.serialize_node(at_once.root()),
            "cut at {at}"
        );
        assert!(
            stops.len() <= prefix.matches("</script>").count(),
            "cut at {at}"
        );
        assert!(!stepped.is_being_parsed(), "cut at {at}");
    }
}

#[test]
fn ten_thousand_scripts_are_ten_thousand_stops() {
    let html = "<script>x</script>".repeat(10_000);
    let (stops, document) = in_steps(&html);
    assert_eq!(stops.len(), 10_000);
    let mut distinct = stops.clone();
    distinct.dedup();
    assert_eq!(distinct.len(), 10_000);
    assert!(stops.iter().all(|id| document.is_attached(*id)));
}
