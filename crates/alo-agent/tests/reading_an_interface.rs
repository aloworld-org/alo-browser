/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ★ Reading an interface as what it is.
//!
//! `docs/decisions/0002` opens with a sentence: *"invoice list, twelve rows,
//! row three selected."* This test builds that page out of HTML and CSS and
//! asserts that the engine can say it — by role and by name, never by
//! coordinate, and without a screenshot anywhere in the chain.

use alo_agent::AgentTree;
use alo_box::{KnownRole, Role, build as build_boxes};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};

const PAGE: &str = "<!DOCTYPE html><html><body>\
<main>\
<h1>Invoices</h1>\
<nav aria-label='Filters'><a href='/all'>All</a> <a href='/due'>Due</a></nav>\
<ul>\
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
<p aria-hidden='true'>Not for reading</p>\
<div>a plain div with no meaning</div>\
</main>\
</body></html>";

const SHEET: &str = "body { margin: 0; font-family: system-ui; font-size: 14px }
main { padding: 8px } h1 { font-size: 20px; margin: 0 }
ul { margin: 0; padding: 0 } li { padding: 4px }";

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    if let Some(font) = Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    ) {
        database.add(font);
    }
    database.map_generic("system-ui", "DejaVu Sans");
    database
}

/// Render the page and read it, doing everything in one place so the test
/// bodies say only what they are asserting.
macro_rules! read {
    ($document:ident, $boxes:ident, $layout:ident, $tree:ident) => {
        let $document = parse_document(PAGE);
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let author = parse_stylesheet(SHEET);
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &agent),
            SourcedSheet::new(Origin::Author, &author),
        ];
        let styles = resolve(&$document, &sheets, &MediaContext::default());
        let $boxes = build_boxes(&$document, &styles);
        let database = fonts();
        let measurer = TextMeasurer::new(&database);
        let $layout = compute(&$boxes, &styles, Size::new(240.0, 200.0), &measurer);
        let $tree = AgentTree::new(&$document, &$boxes, &$layout);
    };
}

#[test]
fn an_agent_finds_the_rows_by_what_they_are() {
    read!(document, boxes, layout, tree);
    let rows = tree.with_role(&Role::Known(KnownRole::ListItem));
    assert_eq!(
        rows.len(),
        3,
        "three rows, found by role rather than by looking"
    );

    let names: Vec<Option<String>> = rows.iter().map(alo_agent::AgentNode::name).collect();
    assert_eq!(
        names,
        vec![
            Some("Invoice 11".to_owned()),
            Some("Invoice 12".to_owned()),
            Some("Invoice 13".to_owned()),
        ],
        "and each says what it is called",
    );
}

#[test]
fn an_agent_finds_the_selected_row_by_its_state_and_can_say_which_it_is() {
    read!(document, boxes, layout, tree);
    let rows = tree.with_role(&Role::Known(KnownRole::ListItem));
    let selected: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.states().selected == Some(true))
        .map(|(index, _)| index)
        .collect();

    assert_eq!(selected, vec![1], "row two is the selected one");
    assert_eq!(
        rows.get(1).and_then(alo_agent::AgentNode::name),
        Some("Invoice 12".to_owned()),
        "which is ADR 0002's sentence, answered from the tree",
    );
}

#[test]
fn a_thing_can_be_found_by_the_name_a_person_would_read_out() {
    read!(document, boxes, layout, tree);
    let found = tree.named("save");
    assert_eq!(found.len(), 1, "one button called Save, whatever its case");
    let button = found.first().expect("the button");
    assert_eq!(button.role(), Role::Known(KnownRole::Button));
    assert!(button.states().disabled, "and it says it cannot be pressed");
}

#[test]
fn a_field_is_named_by_the_label_that_points_at_it() {
    read!(document, boxes, layout, tree);
    let fields = tree.with_role(&Role::Known(KnownRole::TextBox));
    let field = fields.first().expect("the amount field");
    assert_eq!(
        field.name(),
        Some("Amount".to_owned()),
        "the `<label for>` names it, which is HTML rather than anything we invented",
    );
    assert!(field.states().required);
}

#[test]
fn what_the_author_hid_is_not_read() {
    read!(document, boxes, layout, tree);
    assert!(
        document
            .text_content(document.root())
            .contains("Not for reading"),
        "the document has it",
    );
    assert!(
        !tree.to_outline().contains("Not for reading"),
        "and the agent tree does not: `aria-hidden` is the author saying so\n{}",
        tree.to_outline(),
    );
}

#[test]
fn a_div_that_means_nothing_is_read_through_rather_than_reported() {
    read!(document, boxes, layout, tree);
    let outline = tree.to_outline();
    assert!(
        !outline.contains("generic"),
        "a page is mostly divs, and a tree that showed them would bury the rows:\n{outline}",
    );
    assert!(
        outline.contains("a plain div with no meaning"),
        "but the text inside one is still read",
    );
}

#[test]
fn every_node_knows_where_it_is_without_anybody_having_looked() {
    read!(document, boxes, layout, tree);
    for row in tree.with_role(&Role::Known(KnownRole::ListItem)) {
        let rect = row.rect();
        assert!(rect.size.width > 0.0 && rect.size.height > 0.0, "{row}");
        assert!(!row.is_offscreen(), "and all three rows are on screen");
    }
}

#[test]
fn something_below_the_window_says_that_it_is_off_screen() {
    // The same page in a very short window: the rows fall off the bottom, and
    // the tree can say so — which is the thing a DOM cannot, because a
    // scrolled-away row looks identical to a visible one in it.
    let document = parse_document(PAGE);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(SHEET);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(240.0, 40.0), &measurer);
    let tree = AgentTree::new(&document, &boxes, &layout);

    let rows = tree.with_role(&Role::Known(KnownRole::ListItem));
    assert!(!rows.is_empty());
    assert!(
        rows.iter().all(alo_agent::AgentNode::is_offscreen),
        "in a forty-pixel window the rows are below it",
    );
}

#[test]
fn the_whole_tree_reads_as_what_the_interface_is() {
    read!(document, boxes, layout, tree);
    // The twenty-eight pixels between the form and the div are the
    // `aria-hidden` paragraph's margins: hidden from an agent, and still on the
    // page. The user-agent sheet gained the specification's block margins with
    // queue item 171, and this is a place where "not in the tree" and "takes no
    // room" are usefully different things.
    let expected = "\
document at (0, 0) 240×233.96426
  main at (0, 0) 240×233.96426
    heading \"Invoices\" [level=1] at (8, 8) 224×23.28125
    navigation \"Filters\" at (8, 31.28125) 224×16.296875
      link \"All\" at (8, 31.28125) 17.356445×16.296875
      link \"Due\" at (29.80664, 31.28125) 28.266602×16.296875
    list at (8, 47.578125) 224×72.890625
      listitem \"Invoice 11\" at (8, 47.578125) 224×24.296875
      listitem \"Invoice 12\" [selected=true] at (8, 71.875) 224×24.296875
      listitem \"Invoice 13\" at (8, 96.171875) 224×24.296875
    form \"New invoice\" at (8, 120.46875) 224×44.90176
      textbox \"Amount\" [required] at (63.015625, 120.46875) 146×20.800001
      checkbox \"Recurring\" [checked=true] at (209.01563, 126.26875) 15×15
      button \"Save\" [disabled] at (8, 144.57051) 48.364258×20.800001
    text \"a plain div with no meaning\" at (8, 209.66739) 194.50293×16.296875
";
    assert_eq!(tree.to_outline(), expected);
}

#[test]
fn a_link_broken_around_a_block_is_still_one_link() {
    // CSS breaks the `<a>` into two boxes around the `<div>`. An agent that
    // saw two would have to choose between them, and both would answer to the
    // same name — which is exactly the ambiguity ADR 0002's verbs refuse.
    let page = "<!DOCTYPE html><html><body><section>\
<a href='/docs' id=link>Read the<div>docs</div></a>\
</section></body></html>";
    let document = parse_document(page);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(SHEET);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(240.0, 200.0), &measurer);
    let tree = AgentTree::new(&document, &boxes, &layout);

    let links = tree.with_role(&Role::Known(KnownRole::Link));
    assert_eq!(
        links.len(),
        1,
        "one link, in two pieces:\n{}",
        tree.to_outline(),
    );
    assert_eq!(
        links
            .first()
            .and_then(alo_agent::AgentNode::name)
            .as_deref(),
        Some("Read the docs"),
        "the block between the pieces is inside the link, so it is part of what \
         the link is called",
    );
}

#[test]
fn the_empty_piece_of_a_broken_link_is_not_a_second_link() {
    // CSS keeps a piece with nothing in it so that it can draw its border. A
    // border is not something to read, so the agent reads the piece that has
    // something in it and reads the other through.
    let page = "<!DOCTYPE html><html><body><section>\
<a href='/docs' id=link><div>docs</div>Read the</a>\
</section></body></html>";
    let document = parse_document(page);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(SHEET);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(240.0, 200.0), &measurer);
    let tree = AgentTree::new(&document, &boxes, &layout);

    let links = tree.with_role(&Role::Known(KnownRole::Link));
    assert_eq!(
        links.len(),
        1,
        "the empty piece before the block is not a link of its own:\n{}",
        tree.to_outline(),
    );
    assert_eq!(
        links
            .first()
            .and_then(alo_agent::AgentNode::name)
            .as_deref(),
        Some("docs Read the"),
    );
}

/// Read a page of its own, rather than the one every other test shares.
fn read_page(page: &str) -> String {
    let document = parse_document(page);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(SHEET);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(240.0, 200.0), &measurer);
    AgentTree::new(&document, &boxes, &layout).to_outline()
}

const BROKEN: &str = "<!DOCTYPE html><html><body><section>\
<a href='/docs' id=link>Read the<p>manual</p>carefully</a>\
</section></body></html>";

#[test]
fn the_block_between_two_pieces_is_read_inside_the_link_rather_than_beside_it() {
    let outline = read_page(BROKEN);
    let lines: Vec<&str> = outline.lines().collect();
    let link = lines
        .iter()
        .position(|line| line.contains("link"))
        .expect("a link");
    let paragraph = lines
        .iter()
        .position(|line| line.contains("\"manual\""))
        .expect("the block");
    assert!(
        paragraph > link,
        "the block comes after the link:\n{outline}"
    );

    let indent = |line: &str| line.len() - line.trim_start().len();
    assert!(
        indent(lines[paragraph]) > indent(lines[link]),
        "and inside it, not beside it:\n{outline}",
    );
}

#[test]
fn nothing_inside_a_broken_link_is_read_twice() {
    let outline = read_page(BROKEN);
    // The link's own text is what the link is *called*, so it is not also a
    // run of text beside it — which is the rule for any thing named by its
    // content, and has to keep holding once that content is in two pieces.
    assert!(!outline.contains("text \"Read the\""), "{outline}");
    assert!(!outline.contains("text \"carefully\""), "{outline}");
    assert_eq!(outline.matches("link \"").count(), 1, "{outline}");
    assert_eq!(outline.matches("paragraph").count(), 1, "{outline}");
}

#[test]
fn a_broken_link_is_where_all_of_it_is() {
    let document = parse_document(BROKEN);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(SHEET);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(240.0, 200.0), &measurer);
    let tree = AgentTree::new(&document, &boxes, &layout);

    let link = tree
        .with_role(&Role::Known(KnownRole::Link))
        .into_iter()
        .next()
        .expect("a link");
    let rect = link.rect();
    // Three lines of it: the text before, the block, the text after. A single
    // piece would be a third as tall.
    assert!(
        rect.size.height > 40.0,
        "the link is everywhere it was drawn: {rect:?}",
    );
    assert_eq!(link.name().as_deref(), Some("Read the manual carefully"));
}
