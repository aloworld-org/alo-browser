/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A canvas app's own dialog, driven by name through Chrome: queue items
//! 941 and 942, ADR 0109 § 1.
//!
//! diagrams.net and Excalidraw draw their boards where Chrome's tree does
//! not reach, and each has a dialog that takes the board as words:
//! diagrams.net's *Extras* → *Edit Diagram...*, a field holding the
//! diagram's XML, and *Apply*; Excalidraw's *More tools* → *Mermaid to
//! Excalidraw*, a field named *Write Mermaid diagram definition here...*,
//! and *Insert* (measured against Chrome 157 on app.diagrams.net and
//! excalidraw.com, 11 November 2026: `button` *Extras*, `menuitem` *Edit
//! Diagram...*, `dialog` *Edit Diagram* holding an unnamed `textbox` and
//! `button` *Apply*; `button` *More tools*, `menuitem` *Mermaid to
//! Excalidraw*, the named `textbox` and `button` *Insert*).
//!
//! Neither app can be frozen — each is megabytes of script — so each page
//! here is **written for the test in the shape measured**, its own script
//! doing what the app does with the words: the first draws the diagram's
//! labels from its XML as SVG text, which the tree reads; the second draws
//! on a canvas, which the tree does not, and puts what it was given in its
//! title, the one place the test can read it. So these say the verbs
//! there are reach the dialog and its field, read the field's words, and
//! put new ones in; that each app takes them is the hand runs' (ADR 0109,
//! amended).

use alo_agent::Outcome;
use alo_agent::verb::{Target, Verb};
use alo_box::{KnownRole, Role};
use alo_renderer::{Snapshot, SnapshotNode};
use alo_window::chrome::Chrome;
use alo_window::chrome_tab::Tab;

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

/// Whether a node of `snapshot` is named `name`.
fn says(snapshot: &Snapshot, name: &str) -> bool {
    every(snapshot)
        .iter()
        .any(|node| node.name.as_deref() == Some(name))
}

/// The one `textbox` of `snapshot` inside something of `role`.
fn field_in(snapshot: &Snapshot, role: KnownRole) -> Option<&SnapshotNode> {
    let inside = every(snapshot)
        .into_iter()
        .find(|node| node.role == Role::Known(role))?;
    let found = every(&Snapshot {
        root: Some(inside.clone()),
    })
    .into_iter()
    .find(|node| node.role == Role::Known(KnownRole::TextBox))?
    .id;
    every(snapshot).into_iter().find(|node| node.id == found)
}

/// A press of what is named `name`, of `role`.
fn press(tab: &mut Tab<'_>, role: KnownRole, name: &str) -> Result<Outcome, String> {
    tab.act(
        &Target::NamedOfRole {
            role: Role::Known(role),
            name: name.to_owned(),
        },
        &Verb::Activate,
    )
    .map_err(|why| format!("{name}: {why}"))
}

/// diagrams.net's menu, dialog and board, as measured.
const DIAGRAMS: &str = "<!doctype html><html lang=en><title>Untitled Diagram - draw.io</title><body>\
<div><button>File</button><button>Extras</button></div>\
<div role=menu hidden id=menu><div role=menuitem tabindex=0 id=edit>Edit Diagram...</div></div>\
<div role=dialog aria-label=\"Edit Diagram\" hidden id=dialog>\
<textarea id=xml></textarea><button>Cancel</button><button id=apply>Apply</button></div>\
<svg id=board width=600 height=100></svg>\
<script>\
let xml = '<mxGraphModel><root><mxCell id=\"0\"/><mxCell id=\"1\" parent=\"0\"/>' +\
  '<mxCell id=\"a\" value=\"Supplier\" vertex=\"1\" parent=\"1\"/>' +\
  '<mxCell id=\"b\" value=\"Warehouse\" vertex=\"1\" parent=\"1\"/>' +\
  '<mxCell id=\"c\" value=\"Shop\" vertex=\"1\" parent=\"1\"/></root></mxGraphModel>';\
const board = document.getElementById('board');\
function draw() {\
  board.replaceChildren();\
  const cells = new DOMParser().parseFromString(xml, 'text/xml').querySelectorAll('mxCell[value]');\
  cells.forEach((cell, at) => {\
    const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');\
    text.setAttribute('x', 20 + at * 150); text.setAttribute('y', 50);\
    text.textContent = cell.getAttribute('value'); board.append(text);\
  });\
}\
draw();\
document.querySelectorAll('button')[1].addEventListener('click', () => { menu.hidden = false; });\
document.getElementById('edit').addEventListener('click', () => {\
  menu.hidden = true; document.getElementById('xml').value = xml; dialog.hidden = false;\
});\
document.getElementById('apply').addEventListener('click', () => {\
  xml = document.getElementById('xml').value; dialog.hidden = true; draw();\
});\
</script>";

#[test]
fn a_diagrams_box_is_renamed_through_its_own_edit_diagram_dialog() {
    let mut chrome = Chrome::start().unwrap();
    let mut tab = Tab::open_frozen(
        &mut chrome,
        "https://app.diagrams.example/",
        DIAGRAMS.into(),
        (800, 500),
    )
    .unwrap();
    let read = tab.read().unwrap();
    assert!(says(&read, "Warehouse"), "the board's labels, as SVG text");

    press(&mut tab, KnownRole::Button, "Extras").unwrap();
    press(&mut tab, KnownRole::MenuItem, "Edit Diagram...").unwrap();
    let read = tab.read().unwrap();
    let field = field_in(&read, KnownRole::Dialog).unwrap();
    // The field's words are the diagram, as an agent reads them.
    let held = words(field);
    assert!(
        held.contains("value=\"Warehouse\"") && held.contains("<mxGraphModel>"),
        "{held:?}"
    );
    let renamed = held.replace("Warehouse", "Depot");
    tab.act(&Target::Node(field.id), &Verb::PutText(renamed))
        .unwrap();
    press(&mut tab, KnownRole::Button, "Apply").unwrap();

    let read = tab.read().unwrap();
    assert!(says(&read, "Depot"), "{read:?}");
    assert!(!says(&read, "Warehouse"));
    assert!(says(&read, "Supplier") && says(&read, "Shop"));
}

/// Excalidraw's tools, its Mermaid dialog and a canvas, as measured.
const EXCALIDRAW: &str = "<!doctype html><html lang=en><title>Excalidraw</title><body>\
<div role=toolbar><button aria-label=Rectangle>R</button><button aria-label=\"More tools\">+</button></div>\
<div role=menu hidden id=menu><div role=menuitem tabindex=0>Text to diagram</div>\
<div role=menuitem tabindex=0 id=mermaid>Mermaid to Excalidraw</div></div>\
<div role=dialog hidden id=dialog>\
<textarea aria-label=\"Write Mermaid diagram definition here...\">flowchart TD\n A[Christmas] --> B(Go shopping)</textarea>\
<button id=insert>Insert</button></div>\
<canvas id=board width=600 height=200></canvas>\
<script>\
const menu = document.getElementById('menu');\
const dialog = document.getElementById('dialog');\
document.querySelectorAll('button')[1].addEventListener('click', () => { menu.hidden = false; });\
document.getElementById('mermaid').addEventListener('click', () => { menu.hidden = true; dialog.hidden = false; });\
document.getElementById('insert').addEventListener('click', () => {\
  const said = document.querySelector('textarea').value;\
  const labels = [...said.matchAll(/\\[([^\\]]+)\\]/g)].map((found) => found[1]);\
  const pen = document.getElementById('board').getContext('2d');\
  labels.forEach((label, at) => pen.fillText(label, 10 + at * 140, 100));\
  dialog.hidden = true;\
  document.title = 'Excalidraw: ' + labels.join(', ');\
});\
</script>";

#[test]
fn a_flowchart_is_drawn_through_its_own_mermaid_dialog_and_is_not_in_the_tree() {
    let mut chrome = Chrome::start().unwrap();
    let mut tab = Tab::open_frozen(
        &mut chrome,
        "https://excalidraw.example/",
        EXCALIDRAW.into(),
        (800, 500),
    )
    .unwrap();
    press(&mut tab, KnownRole::Button, "More tools").unwrap();
    press(&mut tab, KnownRole::MenuItem, "Mermaid to Excalidraw").unwrap();
    let flowchart = "flowchart TD\n  A[Order placed] --> B[Payment checked]\n  \
                     B --> C[Packed]\n  C --> D[Shipped]";
    let put = tab
        .act(
            &Target::Named("Write Mermaid diagram definition here...".to_owned()),
            &Verb::PutText(flowchart.to_owned()),
        )
        .unwrap();
    assert!(matches!(put, Outcome::TextPut { .. }), "{put:?}");
    press(&mut tab, KnownRole::Button, "Insert").unwrap();

    let read = tab.read().unwrap();
    let title = read.root.as_ref().and_then(|root| root.name.clone());
    assert_eq!(
        title.as_deref(),
        Some("Excalidraw: Order placed, Payment checked, Packed, Shipped")
    );
    // Drawn on a canvas: no word of it is in the tree, and the dialog is
    // gone — so a run can say what it inserted, never that it saw it.
    assert!(!says(&read, "Packed"));
    assert!(!says(&read, "Insert"));
}
