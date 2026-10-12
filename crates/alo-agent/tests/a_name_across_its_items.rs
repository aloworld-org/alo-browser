/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a name taken from content has a space in it (queue item 530).
//!
//! A thing named by what is inside it reads its boxes' text in order, and a
//! space goes wherever one box stands apart from the next. Chrome 154's
//! accessibility tree says which boxes do: every one that is not an inline
//! box in its parent's lines. So a flex or grid item is apart even though
//! it was written `inline`, and so are an `inline-block`, an `inline-flex`
//! and an `inline-grid`, because each is a box of its own on its line;
//! two `<span>`s side by side in a line are not.
//!
//! Every expected name here was read from Chrome 154's accessibility tree
//! (`Accessibility.getFullAXTree`, headless, iteration 321), not reasoned
//! from a specification.

use alo_agent::AgentTree;
use alo_box::{KnownRole, Role, build as build_boxes};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};

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
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

/// The name of the first thing with `role` in a page whose body is `body`.
fn name_of(body: &str, role: KnownRole) -> Option<String> {
    let page = format!("<!DOCTYPE html><html><body>{body}</body></html>");
    let document = parse_document(&page);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let sheets = [SourcedSheet::new(Origin::UserAgent, &agent)];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(400.0, 300.0), &measurer);
    let tree = AgentTree::new(&document, &boxes, &layout);
    tree.with_role(&Role::Known(role))
        .first()
        .and_then(alo_agent::AgentNode::name)
}

/// Each body, and the name Chrome 154 gives its tab.
fn assert_tabs(cases: &[(&str, &str)]) {
    let wrong: Vec<String> = cases
        .iter()
        .filter_map(|(body, chrome)| {
            let ours = name_of(body, KnownRole::Tab);
            (ours.as_deref() != Some(*chrome))
                .then(|| format!("{body}\n  Chrome: {chrome:?}\n  ours:   {ours:?}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_flex_or_grid_item_is_apart_from_its_neighbour() {
    assert_tabs(&[
        (
            "<div role=tab style=display:flex><span>Notes</span><span>Stopped</span></div>",
            "Notes Stopped",
        ),
        (
            "<div role=tab style=display:grid><span>Notes</span><span>Stopped</span></div>",
            "Notes Stopped",
        ),
        (
            "<div role=tab style=display:inline-flex><span>Notes</span><span>Stopped</span></div>",
            "Notes Stopped",
        ),
        (
            "<div role=tab style=display:inline-grid><span>Notes</span><span>Stopped</span></div>",
            "Notes Stopped",
        ),
        (
            "<div role=tab style=display:flex;flex-direction:column><span>Notes</span><span>Stopped</span></div>",
            "Notes Stopped",
        ),
        (
            "<div role=tab style=display:flex><div style=display:inline>Notes</div><div style=display:inline>Stopped</div></div>",
            "Notes Stopped",
        ),
        (
            "<div role=tab><div style=display:flex><span>Notes</span><span>Stopped</span></div></div>",
            "Notes Stopped",
        ),
    ]);
}

#[test]
fn text_written_straight_into_a_flex_container_is_an_item_of_its_own() {
    assert_tabs(&[
        (
            "<div role=tab style=display:flex>Notes<span>Stopped</span></div>",
            "Notes Stopped",
        ),
        (
            "<div role=tab style=display:flex><span>Notes</span>Stopped</div>",
            "Notes Stopped",
        ),
    ]);
}

#[test]
fn inside_one_item_its_inline_boxes_still_run_together() {
    assert_tabs(&[
        (
            "<div role=tab style=display:flex><span><b>No</b><i>tes</i></span><span>Stopped</span></div>",
            "Notes Stopped",
        ),
        (
            "<div role=tab><span style=display:inline-block><span>b</span><span>c</span></span></div>",
            "bc",
        ),
    ]);
}

#[test]
fn an_atomic_box_on_a_line_is_apart_from_the_text_beside_it() {
    assert_tabs(&[
        (
            "<div role=tab><span style=display:inline-block>Notes</span><span style=display:inline-block>Stopped</span></div>",
            "Notes Stopped",
        ),
        (
            "<div role=tab>a<span style=display:inline-block>b</span>c</div>",
            "a b c",
        ),
        (
            "<div role=tab>a<span style=display:inline-block><span>b</span><span>c</span></span>d</div>",
            "a bc d",
        ),
        (
            "<div role=tab><span>a<span style=display:inline-block>b</span></span>c</div>",
            "a b c",
        ),
        (
            "<div role=tab><span style=display:inline-flex>a</span>b</div>",
            "a b",
        ),
        (
            "<div role=tab><span style=display:inline-grid>a</span><span style=display:inline-grid>b</span></div>",
            "a b",
        ),
        (
            "<div role=tab><span>A</span><span style=display:inline-flex><span>B</span><span>C</span></span><span>D</span></div>",
            "A B C D",
        ),
    ]);
}

#[test]
fn two_inline_boxes_in_one_line_are_one_word() {
    assert_tabs(&[(
        "<div role=tab><span>Notes</span><span>Stopped</span></div>",
        "NotesStopped",
    )]);
}

#[test]
fn a_box_taken_out_of_the_line_is_apart() {
    assert_tabs(&[
        (
            "<div role=tab style=display:flex><span>Notes</span><span style=position:absolute>Stopped</span></div>",
            "Notes Stopped",
        ),
        (
            "<div role=tab>a<span style=position:absolute>b</span>c</div>",
            "a b c",
        ),
        (
            "<div role=tab style=display:flex><span style=display:block>a</span><span>b</span></div>",
            "a b",
        ),
    ]);
}

#[test]
fn what_is_hidden_or_already_spaced_adds_nothing() {
    assert_tabs(&[
        (
            "<div role=tab style=display:flex><span>Notes</span><span hidden>Hid</span><span>Stopped</span></div>",
            "Notes Stopped",
        ),
        (
            "<div role=tab style=display:flex><span>Notes </span><span>Stopped</span></div>",
            "Notes Stopped",
        ),
        (
            "<div role=tab style=display:flex> <span>a</span> <span>b</span> </div>",
            "a b",
        ),
    ]);
}

#[test]
fn a_button_or_a_link_laid_out_as_flex_is_named_as_chrome_names_it() {
    assert_eq!(
        name_of(
            "<button style=display:flex><span>Save</span><span>Draft</span></button>",
            KnownRole::Button,
        )
        .as_deref(),
        Some("Save Draft"),
    );
    assert_eq!(
        name_of(
            "<button style=display:inline-flex><span>Save</span><span>Draft</span></button>",
            KnownRole::Button,
        )
        .as_deref(),
        Some("Save Draft"),
    );
    assert_eq!(
        name_of(
            "<button><span style=display:inline-block>Save</span><span style=display:inline-block>Draft</span></button>",
            KnownRole::Button,
        )
        .as_deref(),
        Some("Save Draft"),
    );
    assert_eq!(
        name_of(
            "<a href=/docs style=display:flex><span>Read</span><span>Docs</span></a>",
            KnownRole::Link,
        )
        .as_deref(),
        Some("Read Docs"),
    );
}
