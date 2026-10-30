/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A name taken from content leaves out what is `aria-hidden` (queue item
//! 699).
//!
//! An icon in a link, the keys that press a button, a dot that says a chat
//! is busy: each is drawn for a person and hidden from a reader, and none is
//! what the thing is called. The agent tree already leaves a hidden box out;
//! a name read from content has to leave it out too, or an agent asks for
//! *"E Charging my EV"* where a person sees a row called *Charging my EV*.
//!
//! Every expected name here was read from Chrome 156.0.8078.76's
//! accessibility tree (`Accessibility.getFullAXTree`, headless, 30 October
//! 2026), not reasoned from a specification.

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

/// Each body, the role read, and the name Chrome 156 gives it — [`None`]
/// where Chrome's is empty.
fn assert_names(cases: &[(&str, KnownRole, Option<&str>)]) {
    let wrong: Vec<String> = cases
        .iter()
        .filter_map(|(body, role, chrome)| {
            let ours = name_of(body, *role);
            (ours.as_deref() != *chrome)
                .then(|| format!("{body}\n  Chrome: {chrome:?}\n  ours:   {ours:?}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_hidden_mark_or_key_is_not_part_of_a_name() {
    assert_names(&[
        (
            "<a href=#><span aria-hidden=true>E</span><span>Charging my EV</span></a>",
            KnownRole::Link,
            Some("Charging my EV"),
        ),
        (
            "<a href=# style=display:flex><span aria-hidden=true>E</span>\
             <span>Charging my EV</span></a>",
            KnownRole::Link,
            Some("Charging my EV"),
        ),
        (
            "<a href=# style=display:flex>New chat\
             <span aria-hidden=true style=margin-left:auto>⌘T</span></a>",
            KnownRole::Link,
            Some("New chat"),
        ),
        (
            "<a href=# style=display:flex><span aria-hidden=true>B</span>\
             <span>Invoice for March</span><span>Done</span></a>",
            KnownRole::Link,
            Some("Invoice for March Done"),
        ),
    ]);
}

#[test]
fn everything_under_a_hidden_box_is_left_out_with_it() {
    assert_names(&[
        (
            "<button><span aria-hidden=true><b>x</b> y</span>Save</button>",
            KnownRole::Button,
            Some("Save"),
        ),
        (
            "<a href=#><span aria-hidden=true>all hidden</span></a>",
            KnownRole::Link,
            None,
        ),
    ]);
}

#[test]
fn hidden_said_false_is_read_as_any_other_text() {
    assert_names(&[(
        "<button>Save<span aria-hidden=false>now</span></button>",
        KnownRole::Button,
        Some("Savenow"),
    )]);
}
