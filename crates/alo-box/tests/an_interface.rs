//! The boxes of an interface, asserted whole.
//!
//! ADR 0002's example is "invoice list, twelve rows, row three selected", so
//! that is what this builds. The test is the **whole outline**, compared
//! against a written-out expectation: a change that moves a box says which
//! box, which is worth more than ten assertions that each check one thing.
//!
//! This is the shape a reference render will take when there is something to
//! rasterise. Until then the tree is the artefact, and it is asserted the same
//! way — in full, deterministically, and readably.

use alo_box::{BoxId, BoxKind, BoxTree, build};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};

const PAGE: &str = "<!DOCTYPE html>\
<html lang='en'>\
<body>\
<main>\
  <h1>Invoices</h1>\
  <nav aria-label='Filters'><a href='/all'>All</a> <a href='/due'>Due</a></nav>\
  <ul class='rows'>\
    <li>Invoice 11</li>\
    <li aria-selected='true'>Invoice 12</li>\
    <li>Invoice 13</li>\
  </ul>\
  <form aria-label='New invoice'>\
    <label for='amount'>Amount</label>\
    <input id='amount' required>\
    <input type='checkbox' checked aria-label='Recurring'>\
    <button disabled>Save</button>\
  </form>\
  <p hidden>Not drawn</p>\
</main>\
</body>\
</html>";

const SHEET: &str = "
.rows { display: flex }
li { padding: 4px }
";

fn tree_of(html: &str, css: &str) -> BoxTree {
    let document = parse_document(html);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(css);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    build(&document, &styles)
}

#[test]
fn the_whole_interface_becomes_the_boxes_it_should() {
    let tree = tree_of(PAGE, SHEET);
    let expected = "\
block flow · document
  block flow · generic
    block flow · main
      block flow · heading [level=1]
        text \"Invoices\"
      block flow · navigation \"Filters\"
        inline flow · link
          text \"All\"
        text \" \"
        inline flow · link
          text \"Due\"
      block flex · list
        block flow list-item · listitem
          text \"Invoice 11\"
        block flow list-item · listitem [selected=true]
          text \"Invoice 12\"
        block flow list-item · listitem
          text \"Invoice 13\"
      block flow · form \"New invoice\"
        inline flow · generic
          text \"Amount\"
        inline flow-root · textbox [required]
          anonymous block
        inline flow-root · checkbox \"Recurring\" [checked=true]
        inline flow-root · button [disabled]
          anonymous block
            text \"Save\"
";
    assert_eq!(tree.to_outline(), expected);
}

#[test]
fn nothing_the_engine_could_not_build_exactly_was_needed() {
    let tree = tree_of(PAGE, SHEET);
    assert!(
        tree.issues().is_empty(),
        "an ordinary interface should need no approximations: {:?}",
        tree.issues()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
    );
}

#[test]
fn the_hidden_paragraph_is_in_the_document_and_not_in_the_boxes() {
    let document = parse_document(PAGE);
    assert!(
        document.text_content(document.root()).contains("Not drawn"),
        "the document has it",
    );
    assert!(
        !tree_of(PAGE, SHEET).to_outline().contains("Not drawn"),
        "and the boxes do not: `[hidden]` is `display: none`",
    );
}

#[test]
fn an_agent_can_find_the_selected_row_by_what_it_is_rather_than_where() {
    let tree = tree_of(PAGE, SHEET);
    let root = tree.root().expect("a root");

    let rows: Vec<BoxId> = tree
        .descendants(root)
        .into_iter()
        .filter(|id| {
            tree.get(*id)
                .is_some_and(|node| node.semantics.role.to_string() == "listitem")
        })
        .collect();
    assert_eq!(rows.len(), 3);

    let selected: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, id)| {
            tree.get(**id)
                .is_some_and(|node| node.semantics.states.selected == Some(true))
        })
        .map(|(index, _)| index)
        .collect();
    assert_eq!(selected, vec![1], "row two is the selected one");

    // And what it says, which is the other half of ADR 0002's sentence.
    let row = rows.get(1).copied().expect("the selected row");
    let text: String = tree
        .descendants(row)
        .into_iter()
        .filter_map(|id| tree.get(id).and_then(alo_box::BoxNode::text))
        .collect();
    assert_eq!(text, "Invoice 12");
}

#[test]
fn every_box_can_be_traced_back_to_what_asked_for_it() {
    let document = parse_document(PAGE);
    let tree = tree_of(PAGE, SHEET);
    let root = tree.root().expect("a root");

    for id in core::iter::once(root).chain(tree.descendants(root)) {
        let node = tree.get(id).expect("a box we just listed");
        match node.kind.node() {
            Some(source) => assert!(
                document.get(source).is_some(),
                "{id} points at a node that is not in the document",
            ),
            None => assert!(
                matches!(node.kind, BoxKind::Anonymous { .. }),
                "only an anonymous box has no source",
            ),
        }
    }
    // Every container's children are already all of one kind, so no run needs
    // wrapping. The anonymous boxes this interface *does* have are the ones a
    // form control holds its content in — a different reason, and the box tree
    // records which is which.
    for id in tree.descendants(root) {
        if let Some(alo_box::BoxKind::Anonymous { purpose, .. }) =
            tree.get(id).map(|node| &node.kind)
        {
            assert!(
                matches!(purpose, alo_box::Purpose::Control { .. }),
                "{id} is a run wrapper, and nothing here needs one",
            );
        }
    }
}

#[test]
fn an_anonymous_box_appears_where_a_container_needs_one_and_belongs_to_nobody() {
    let tree = tree_of(
        "<body><div>Loose text<p>A block</p>and more</div></body>",
        "",
    );
    let root = tree.root().expect("a root");
    let anonymous: Vec<BoxId> = tree
        .descendants(root)
        .into_iter()
        .filter(|id| {
            tree.get(*id)
                .is_some_and(|node| matches!(node.kind, BoxKind::Anonymous { .. }))
        })
        .collect();

    assert_eq!(anonymous.len(), 2, "one run before the block and one after");
    for id in anonymous {
        let node = tree.get(id).expect("a box we just listed");
        assert_eq!(node.kind.node(), None, "it came from no element");
        assert_eq!(
            node.semantics.role.to_string(),
            "presentation",
            "and it means nothing, because nobody wrote it",
        );
    }
}

#[test]
fn a_narrower_window_does_not_change_the_boxes_unless_a_media_query_says_so() {
    let responsive = "
        .rows { display: flex }
        @media (max-width: 500px) { .rows { display: block } }
    ";
    let build_at = |width: f32| {
        let document = parse_document(PAGE);
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let author = parse_stylesheet(responsive);
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &agent),
            SourcedSheet::new(Origin::Author, &author),
        ];
        let styles = resolve(
            &document,
            &sheets,
            &MediaContext::new(width, alo_css::ColorScheme::Light),
        );
        build(&document, &styles).to_outline()
    };
    assert!(build_at(1200.0).contains("block flex · list"));
    assert!(build_at(400.0).contains("block flow · list"));
}

#[test]
fn a_document_of_nothing_makes_the_boxes_a_document_of_nothing_should() {
    let tree = tree_of("", "");
    assert_eq!(
        tree.to_outline(),
        "block flow · document\n  block flow · generic\n",
        "an empty document is still an html and a body",
    );
    assert!(tree.issues().is_empty());
}
