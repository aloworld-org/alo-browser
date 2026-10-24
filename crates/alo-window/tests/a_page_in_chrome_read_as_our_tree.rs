/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page in Chrome, read as the tree an agent reads: queue item 620, ADR
//! 0058 § 4.
//!
//! The page is the frozen `web-a-form` — httpbin's copy of the HTML
//! specification's pizza form, which item 131 reads and drives through this
//! browser's own engine — opened in the machine's Chrome at the address it
//! was taken from, with every request but the page's own refused. Its tree
//! is read through Chrome and through the engine, and the two are compared
//! node by node, in the order a person meets them.
//!
//! They agree on every role, name and state they both carry. Where they do
//! not, the difference is written out below rather than accepted silently,
//! and a difference that stops being one fails this as surely as a new one.

use alo_box::{KnownRole, Role};
use alo_corpus::{Case, cases_directory, corpus_fonts};
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, Snapshot, ToRenderer};
use alo_window::chrome::{Chrome, Failed};
use alo_window::chrome_tab::{Tab, Unopened};

/// Where the form was served from: the first line of its `origin.txt`.
const FORM: &str = "https://httpbin.org/forms/post";

/// The size the case is laid out at, its `size.txt`.
const SIZE: (u32, u32) = (600, 700);

/// What Chrome's tree holds that the engine's does not, as each node reads.
///
/// - Chrome exposes the `<form>` as `form` with no name; ARIA exposes a form
///   only when it is named, and the engine reads an unnamed one through.
/// - Chrome builds the `time` field: a field of its own role, two spin
///   buttons for its hours and minutes, and a button for its picker, each
///   named twice over by Chrome. The engine has no time field yet (388).
const ONLY_IN_CHROME: [&str; 5] = [
    "form",
    r#"InputTime "Preferred delivery time:""#,
    r#"spinbutton "Hours Hours""#,
    r#"spinbutton "Minutes Minutes""#,
    r#"button "Show time picker Show time picker""#,
];

/// What the engine's tree holds that Chrome's does not: its stand-in for the
/// time field, named as Chrome names Chrome's.
const ONLY_IN_OURS: [&str; 1] = [r#"generic "Preferred delivery time:""#];

/// The frozen form's bytes, and the address it was taken from.
fn the_form() -> Result<(Case, Vec<u8>), String> {
    let directory = cases_directory().join("web-a-form");
    let origin = std::fs::read_to_string(directory.join("origin.txt"))
        .map_err(|why| format!("no origin: {why}"))?;
    if origin.lines().next() != Some(FORM) {
        return Err(format!(
            "the form was taken from {:?}",
            origin.lines().next()
        ));
    }
    let case = Case::read(&directory).ok_or("web-a-form is not a case")?;
    if case.size != (f32::from(600u16), f32::from(700u16)) {
        return Err(format!("the case is laid out at {:?}", case.size));
    }
    let page = std::fs::read(directory.join("page.html")).map_err(|why| why.to_string())?;
    Ok((case, page))
}

/// The form read through Chrome, and the addresses it was refused.
fn read_through_chrome() -> Result<(Snapshot, Vec<String>), String> {
    let (_, page) = the_form()?;
    let mut chrome = Chrome::start().map_err(|why| why.to_string())?;
    let mut tab = Tab::open_frozen(&mut chrome, FORM, page, SIZE).map_err(|why| why.to_string())?;
    let snapshot = tab.read().map_err(|why| why.to_string())?;
    Ok((snapshot, tab.refused().to_vec()))
}

/// The form read through this browser's own engine, as item 131 reads it.
fn read_through_ours() -> Result<Snapshot, String> {
    let (case, _) = the_form()?;
    let address = alo_url::parse(FORM).map_err(|why| format!("{why:?}"))?;
    let mut renderer = Renderer::new(corpus_fonts());
    let page = Page::new(case.html.clone(), Size::new(case.size.0, case.size.1)).at(address);
    match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("the form did not load: {other:?}")),
    }
    match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => Ok(*snapshot),
        other => Err(format!("no tree: {other:?}")),
    }
}

/// Every node as it reads — role, name and states — in the order a person
/// meets them.
fn read_out(snapshot: &Snapshot) -> Vec<String> {
    snapshot.nodes().iter().map(ToString::to_string).collect()
}

#[test]
fn the_form_read_through_chrome_answers_what_ours_answers() {
    let (chrome, _) = read_through_chrome().unwrap();
    let ours = read_through_ours().unwrap();
    let (chrome, ours) = (read_out(&chrome), read_out(&ours));

    // Every difference is the one written down, and each is still one.
    for line in ONLY_IN_CHROME {
        assert_eq!(
            chrome.iter().filter(|read| **read == line).count(),
            1,
            "{line}"
        );
        assert!(!ours.iter().any(|read| read == line), "{line}");
    }
    for line in ONLY_IN_OURS {
        assert_eq!(
            ours.iter().filter(|read| **read == line).count(),
            1,
            "{line}"
        );
        assert!(!chrome.iter().any(|read| read == line), "{line}");
    }
    // And apart from them the two trees read the same, node for node, in
    // the same order: every role, every name, every state.
    let shared = |lines: &[String], apart: &[&str]| -> Vec<String> {
        lines
            .iter()
            .filter(|line| !apart.contains(&line.as_str()))
            .cloned()
            .collect()
    };
    assert_eq!(
        shared(&chrome, &ONLY_IN_CHROME),
        shared(&ours, &ONLY_IN_OURS)
    );
    // Which is not a vacuous agreement: it is the whole form.
    let shared = shared(&ours, &ONLY_IN_OURS);
    assert_eq!(shared.len(), 30, "{shared:#?}");
    for named in [
        r#"textbox "Customer name:""#,
        r#"textbox "Telephone:""#,
        r#"textbox "E-mail address:""#,
        r#"group "Pizza Size""#,
        r#"radio "Large" [checked=false]"#,
        r#"group "Pizza Toppings""#,
        r#"checkbox "Extra Cheese" [checked=false]"#,
        r#"textbox "Delivery instructions:""#,
        r#"button "Submit order""#,
    ] {
        assert!(shared.iter().any(|line| line == named), "{named}");
    }
}

#[test]
fn the_form_in_chrome_is_where_chrome_drew_it() {
    let (chrome, _) = read_through_chrome().unwrap();
    let nodes = chrome.nodes();
    let document = nodes.first().copied().unwrap();
    assert_eq!(document.role, Role::Known(KnownRole::Document));
    // The document is the window it sits in.
    assert_eq!(
        (
            document.rect.left(),
            document.rect.top(),
            document.rect.size.width,
            document.rect.size.height
        ),
        (0.0, 0.0, 600.0, 700.0)
    );
    // Chrome draws a radio and a checkbox 13 by 13 in any font, as the
    // engine does; they are read in the order they stand, top to bottom.
    let mut above = f32::NEG_INFINITY;
    for node in nodes.iter().filter(|node| {
        matches!(
            node.role,
            Role::Known(KnownRole::Radio | KnownRole::CheckBox)
        )
    }) {
        assert_eq!(
            (node.rect.size.width, node.rect.size.height),
            (13.0, 13.0),
            "{node}"
        );
        assert!(node.rect.top() > above, "{node}");
        above = node.rect.top();
    }
    assert!(above > 400.0, "seven of them, down the page");
    // Everything else on the form was drawn, inside the window, in one
    // piece, and is on screen.
    for node in &nodes {
        assert_eq!(node.rects.len(), 1, "{node}");
        assert!(!node.offscreen, "{node}");
        assert!(
            node.rect.size.width > 0.0 && node.rect.size.height > 0.0,
            "{node}"
        );
        assert!(
            node.rect.left() >= 0.0 && node.rect.right() <= 600.0,
            "{node}"
        );
        assert!(
            node.rect.top() >= 0.0 && node.rect.bottom() <= 700.0,
            "{node}"
        );
    }
    // A field is inside its paragraph.
    let field = nodes
        .iter()
        .position(|node| node.to_string() == r#"textbox "Customer name:""#)
        .unwrap();
    let (paragraph, field) = (nodes[field - 1], nodes[field]);
    assert_eq!(paragraph.role, Role::Known(KnownRole::Paragraph));
    assert!(field.rect.left() > paragraph.rect.left());
    assert!(field.rect.right() <= paragraph.rect.right());
    // And every node is named by an id of its own.
    let mut ids: Vec<_> = nodes.iter().map(|node| node.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), nodes.len());
}

#[test]
fn nothing_but_the_page_is_fetched() {
    let (_, refused) = read_through_chrome().unwrap();
    // Chrome may or may not have asked for the site's icon by the time the
    // form loaded; if it did, it was refused, and nothing else was asked.
    assert!(
        refused
            .iter()
            .all(|url| url == "https://httpbin.org/favicon.ico"),
        "{refused:?}"
    );
}

#[test]
fn a_page_that_asks_for_more_is_refused_it_and_still_read() {
    let mut chrome = Chrome::start().unwrap();
    let page = br#"<!DOCTYPE html><title>Pictures</title><p><img src="/a.png" alt="A cat"> <a href="/b">Next</a>"#;
    let mut tab =
        Tab::open_frozen(&mut chrome, "https://example.test/", page.to_vec(), SIZE).unwrap();
    assert!(
        tab.refused()
            .iter()
            .any(|url| url == "https://example.test/a.png"),
        "{:?}",
        tab.refused()
    );
    let read = read_out(&tab.read().unwrap());
    assert!(
        read.iter().any(|line| line == r#"image "A cat""#),
        "{read:#?}"
    );
    assert!(
        read.iter().any(|line| line == r#"link "Next""#),
        "{read:#?}"
    );
    // The link's words are its name, not a node beside it.
    assert!(
        !read.iter().any(|line| line == r#"text "Next""#),
        "{read:#?}"
    );
    assert_eq!(
        read.first().map(String::as_str),
        Some(r#"document "Pictures""#)
    );
}

#[test]
fn a_page_chrome_cannot_go_to_is_refused_with_chromes_reason() {
    let mut chrome = Chrome::start().unwrap();
    match Tab::open_frozen(&mut chrome, "http://[", b"<p>".to_vec(), SIZE) {
        Err(Unopened::Failed(Failed::Declined(declined))) => {
            assert!(!declined.message.is_empty());
        }
        Err(Unopened::NotNavigated { reason }) => assert!(!reason.is_empty()),
        Err(other) => panic!("refused as something else: {other}"),
        Ok(_) => panic!("Chrome went to a page that is not an address"),
    }
    // And Chrome is still there to be asked.
    assert!(chrome.version().is_ok());
}
