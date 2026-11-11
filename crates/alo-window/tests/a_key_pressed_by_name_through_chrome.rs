/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A key pressed by name, through Chrome: queue items 938 and 940, ADR 0109
//! § 1.
//!
//! Google Sheets' Name box goes to a cell only on Return, and its formula
//! bar then holds what the cell does (measured against Chrome 157 on the
//! public sheet ADR 0109 names: the Name box an unnamed `textbox` holding
//! `A1`, inside a `generic` named *Name box (⌘ + J)*; the formula bar an
//! unnamed `textbox`). Sheets itself cannot be frozen — megabytes of its
//! script, and a page of Google's — so this page is **written for the test
//! in the shape the spike found**: an unnamed field inside a group named
//! *Name box*, whose own `keydown` listener goes to the cell typed into it
//! on Enter and puts that cell's words in a formula bar, and goes back to
//! the cell it was on on Escape.
//!
//! So: text put into the field moves nothing; Enter pressed on it, by name,
//! does — the page's own listener heard a person's Return — and the formula
//! bar is read back from the tree; Escape is heard too; and a key on words
//! that take no keyboard is refused before Chrome is asked anything.

use alo_agent::verb::{Target, Verb};
use alo_agent::{Key, Outcome, Refusal};
use alo_box::{KnownRole, Role};
use alo_renderer::{Snapshot, SnapshotNode};
use alo_window::chrome::Chrome;
use alo_window::chrome_act::Unacted;
use alo_window::chrome_tab::Tab;

/// The page, shaped as Sheets' Name box and formula bar.
const PAGE: &str = "<!doctype html><html lang=en><title>A sheet</title><body>\
<div aria-label=\"Name box (\u{2318} + J)\" style=\"display:inline-block\"><input value=A1></div>\
<div><span>fx</span> <div role=textbox aria-readonly=true id=bar>Title</div></div>\
<p id=at>On A1</p>\
<script>\
const cells = { A1: 'Title', C29: 'Spending for dec 2012', B2: 'http://openspending.org/' };\
const box = document.querySelector('input');\
const bar = document.getElementById('bar');\
const at = document.getElementById('at');\
let on = 'A1';\
box.addEventListener('keydown', (e) => {\
  if (e.key === 'Enter' && e.isTrusted) {\
    on = box.value.trim().toUpperCase();\
    bar.textContent = cells[on] || '';\
    at.textContent = 'On ' + on;\
  } else if (e.key === 'Escape') {\
    box.value = on;\
    at.textContent = 'Back on ' + on;\
  }\
});\
</script>";

/// Every node of `snapshot`, in the order a person reads them.
fn every(snapshot: &Snapshot) -> Vec<&SnapshotNode> {
    let mut nodes = Vec::new();
    let mut waiting: Vec<&SnapshotNode> = snapshot.root.iter().collect();
    while let Some(node) = waiting.pop() {
        nodes.push(node);
        waiting.extend(node.children.iter().rev());
    }
    nodes
}

/// The words a node holds, its own and those inside it.
fn words(node: &SnapshotNode) -> String {
    let mut words: Vec<String> = node.name.iter().cloned().collect();
    for child in &node.children {
        words.push(self::words(child));
    }
    words.retain(|word| !word.is_empty());
    words.join(" ")
}

/// The field inside the group named *Name box*, as an agent finds it: the
/// one `textbox` under it.
fn the_name_box(snapshot: &Snapshot) -> Option<&SnapshotNode> {
    let group = every(snapshot).into_iter().find(|node| {
        node.name
            .as_deref()
            .is_some_and(|name| name.starts_with("Name box"))
    })?;
    every(&Snapshot {
        root: Some(group.clone()),
    })
    .into_iter()
    .find(|node| node.role == Role::Known(KnownRole::TextBox))
    .and_then(|found| every(snapshot).into_iter().find(|node| node.id == found.id))
}

/// What the formula bar — the `textbox` after *fx* — holds.
fn the_formula_bar(snapshot: &Snapshot) -> Option<String> {
    let nodes = every(snapshot);
    let fx = nodes
        .iter()
        .position(|node| node.name.as_deref() == Some("fx"))?;
    nodes
        .iter()
        .skip(fx)
        .find(|node| node.role == Role::Known(KnownRole::TextBox))
        .map(|bar| words(bar))
}

/// Whether a node of `snapshot` is named `name`.
fn says(snapshot: &Snapshot, name: &str) -> bool {
    every(snapshot)
        .iter()
        .any(|node| node.name.as_deref() == Some(name))
}

#[test]
fn enter_on_the_name_box_goes_to_the_cell_and_the_formula_bar_says_what_it_holds() {
    let mut chrome = Chrome::start().unwrap();
    let mut tab = Tab::open_frozen(
        &mut chrome,
        "https://sheet.example/",
        PAGE.into(),
        (800, 400),
    )
    .unwrap();
    let read = tab.read().unwrap();
    let field = the_name_box(&read).unwrap().id;
    assert_eq!(the_formula_bar(&read).as_deref(), Some("Title"));

    // Text alone moves nothing: Sheets waits for Return.
    let put = tab
        .act(&Target::Node(field), &Verb::PutText("C29".to_owned()))
        .unwrap();
    assert_eq!(
        put,
        Outcome::TextPut {
            node: field,
            text: "C29".to_owned()
        }
    );
    let read = tab.read().unwrap();
    assert!(says(&read, "On A1"));
    assert_eq!(the_formula_bar(&read).as_deref(), Some("Title"));

    // Enter, by name: the page's own listener hears a person's trusted
    // Return, goes to the cell, and the bar says what it holds.
    let pressed = tab
        .act(&Target::Node(field), &Verb::Press(Key::Enter))
        .unwrap();
    assert_eq!(
        pressed,
        Outcome::KeyPressed {
            node: field,
            key: Key::Enter
        }
    );
    let read = tab.read().unwrap();
    assert!(says(&read, "On C29"), "{read:?}");
    assert_eq!(
        the_formula_bar(&read).as_deref(),
        Some("Spending for dec 2012")
    );

    // Escape is heard as Escape: the field is put back to the cell it is
    // on.
    tab.act(&Target::Node(field), &Verb::PutText("B2".to_owned()))
        .unwrap();
    tab.act(&Target::Node(field), &Verb::Press(Key::Escape))
        .unwrap();
    let read = tab.read().unwrap();
    assert!(says(&read, "Back on C29"), "{read:?}");
    assert_eq!(
        the_formula_bar(&read).as_deref(),
        Some("Spending for dec 2012")
    );

    // Words take no keyboard: refused by the rule that refuses them on this
    // engine's page, before Chrome is asked anything.
    let refused = tab.act(
        &Target::Named("Back on C29".to_owned()),
        &Verb::Press(Key::Enter),
    );
    assert!(
        matches!(refused, Err(Unacted::Refused(Refusal::NotOperable { .. }))),
        "{refused:?}"
    );
}
