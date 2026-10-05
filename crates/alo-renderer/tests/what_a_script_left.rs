/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 250, cut from 246 (ADR 0017 §§ 2 and 6): the renderer hands a
//! page's document to its script, and renders what the script left.
//!
//! *A page's script appends an element and the next render's box tree and
//! layout have it, in numbers; the agent names the node the script made and
//! acts on it; every parsed node keeps its id; and a page that changes its
//! document ten thousand times in one task is rendered once.* The reference
//! render is the corpus case `a-script-grows-a-list`, which the corpus loads
//! through a renderer because its page carries script.
//!
//! What a test does between messages — queueing a task on the page's loop
//! and running it — is what a timer or an event will do once one can (items
//! 92 and 81); it is how these tests change a document **after** its load,
//! so that a `Paint`, a `ReadTree` and an `Act` are each seen drawing the
//! page again because of it.

use alo_agent::{Target, Verb};
use alo_dom::{Document, NodeId};
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, Snapshot, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

const WINDOW: Size = Size {
    width: 300.0,
    height: 200.0,
};

const SHEET: &str = "body { margin: 0; font-family: system-ui; font-size: 14px } \
                     div { height: 20px } input { display: block; width: 120px }";

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

/// A renderer with `html` loaded, and the issues its load answered.
fn loaded(html: &str) -> (Renderer, Vec<String>) {
    let mut renderer = Renderer::new(fonts());
    let issues = match renderer.handle(ToRenderer::Load(Box::new(
        Page::new(html, WINDOW).with_sheet(SHEET),
    ))) {
        FromRenderer::Loaded { issues, .. } => issues,
        other => vec![format!("not loaded: {other:?}")],
    };
    (renderer, issues)
}

/// Read the tree, or an empty one when the renderer answered something else
/// — so the assertion that asked is what fails, saying what it wanted.
fn read(renderer: &mut Renderer) -> Snapshot {
    match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => *snapshot,
        _ => Snapshot::default(),
    }
}

/// Run `source` as a task of the page's loop, as a timer or an event will.
fn later(renderer: &mut Renderer, source: &str) -> Result<(), String> {
    let looping = renderer.event_loop().ok_or("no script of the page's ran")?;
    looping
        .queue_script("later", source)
        .map_err(|stopped| stopped.to_string())?;
    let turn = looping.run_next().ok_or("the task did not run")?;
    if turn.reports.is_empty() && turn.stopped.is_none() {
        Ok(())
    } else {
        Err(format!("{:?} {:?}", turn.reports, turn.stopped))
    }
}

/// The node the box an agent read was made from.
fn node_of_box(renderer: &Renderer, snapshot: &Snapshot, name: &str) -> Option<NodeId> {
    let found = snapshot
        .nodes()
        .into_iter()
        .find(|node| node.name.as_deref() == Some(name))?;
    renderer.rendered()?.boxes.get(found.id)?.kind.node()
}

/// Every attached element's id and name, in tree order.
fn elements(document: &Document) -> Vec<(usize, String)> {
    document
        .descendants(document.root())
        .filter_map(|id| {
            document
                .element(id)
                .map(|element| (id.as_usize(), element.name.local.to_string()))
        })
        .collect()
}

/// A page whose script makes a field, appends it, and changes the text of
/// what was parsed — item 80's members only.
const MAKES_A_FIELD: &str = "<!DOCTYPE html><html><body><div>first</div><div>second</div>\
<script>\
const body = document.documentElement.lastChild;\
const field = document.createElement('input');\
field.setAttribute('aria-label', 'Made by script');\
body.insertBefore(field, body.firstChild.nextSibling);\
body.firstChild.textContent = 'changed';\
</script></body></html>";

#[test]
fn what_a_script_appends_is_in_the_next_render_in_numbers() {
    let (renderer, issues) = loaded(MAKES_A_FIELD);
    assert!(issues.is_empty(), "{issues:?}");
    let (Some(drawing), Some(document)) = (renderer.rendered(), renderer.document()) else {
        panic!("nothing was rendered");
    };
    assert_eq!(
        drawing.layout.to_outline(&drawing.boxes),
        "block flow · document → 300×60.800003 at (0, 0)\n\
         \x20 block flow · generic → 300×60.800003 at (0, 0)\n\
         \x20   block flow · generic → 300×20 at (0, 0)\n\
         \x20     text \"changed\" → 60.40918×16.296875 at (0, 0)\n\
         \x20   block flow · textbox \"Made by script\" → 126×20.800001 at (0, 20)\n\
         \x20     anonymous → 120×16.800001 at (3, 22)\n\
         \x20   block flow · generic → 300×20 at (0, 40.800003)\n\
         \x20     text \"second\" → 49.929688×16.296875 at (0, 40.800003)\n",
        "the field the script made sits between the two blocks, and the text \
         it changed is drawn",
    );
    let body = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|e| e.name.is_html("body"))
    });
    let serialized = body.map(|body| document.serialize_node(body));
    assert!(
        serialized.as_deref().is_some_and(|body| body.starts_with(
            "<body><div>changed</div><input aria-label=\"Made by script\"><div>second</div>\
             <script>"
        )),
        "{serialized:?}"
    );
}

#[test]
fn the_agent_names_the_node_a_script_made_and_acts_on_it() {
    let parsed = alo_dom::parse_document(MAKES_A_FIELD);
    let (mut renderer, _) = loaded(MAKES_A_FIELD);
    let snapshot = read(&mut renderer);
    assert!(
        snapshot.to_outline().contains("textbox \"Made by script\""),
        "{}",
        snapshot.to_outline()
    );
    // ADR 0003 / 0017 § 5: the script's node takes the next id from the
    // parser's counter, so it is the first past every parsed one — and the
    // agent reaches it like any other.
    let made = node_of_box(&renderer, &snapshot, "Made by script");
    assert_eq!(made.map(NodeId::as_usize), Some(parsed.node_count()));

    let acted = renderer.handle(ToRenderer::Act {
        target: Target::Named("Made by script".to_owned()),
        verb: Verb::PutText("typed by an agent".to_owned()),
    });
    assert!(matches!(acted, FromRenderer::Acted(_)), "{acted:?}");
    let after = read(&mut renderer).to_outline();
    assert!(after.contains("typed by an agent"), "{after}");
    let Some(document) = renderer.document() else {
        panic!("no document");
    };
    let field = made.and_then(|made| document.element(made));
    assert_eq!(
        field.and_then(|field| field.attr("value")),
        Some("typed by an agent"),
        "the agent's change went into the document the script's heap holds",
    );
}

#[test]
fn every_parsed_node_keeps_its_id() {
    let parsed = alo_dom::parse_document(MAKES_A_FIELD);
    let (renderer, _) = loaded(MAKES_A_FIELD);
    let Some(document) = renderer.document() else {
        panic!("no document");
    };
    let before = elements(&parsed);
    let after = elements(document);
    // Every parsed element is still where it was, with the id it was given;
    // the one new element is the script's, numbered after all of them.
    let new: Vec<_> = after
        .iter()
        .filter(|(id, _)| *id >= parsed.node_count())
        .collect();
    assert_eq!(new, vec![&(parsed.node_count(), "input".to_owned())]);
    let kept: Vec<_> = after
        .iter()
        .filter(|(id, _)| *id < parsed.node_count())
        .cloned()
        .collect();
    assert_eq!(kept, before);
}

#[test]
fn a_page_that_changes_its_document_ten_thousand_times_in_one_task_is_drawn_once() {
    let (mut renderer, issues) = loaded(
        "<!DOCTYPE html><html><body><div>count</div><script>\
         const div = document.documentElement.lastChild.firstChild;\
         for (let i = 0; i < 10000; i++) { div.setAttribute('data-n', `${i}`); }\
         div.textContent = 'done';\
         </script></body></html>",
    );
    assert!(issues.is_empty(), "{issues:?}");
    let changes = renderer.document().map(Document::change_count);
    assert!(
        changes.is_some_and(|changes| changes > 10_000),
        "{changes:?}"
    );
    assert_eq!(renderer.draws(), 1, "drawn once, after the script");

    // Reading what has not changed draws nothing again.
    assert!(matches!(
        renderer.handle(ToRenderer::Paint),
        FromRenderer::Painted(_)
    ));
    assert!(read(&mut renderer).to_outline().contains("done"));
    assert_eq!(renderer.draws(), 1);
}

#[test]
fn a_paint_after_a_task_draws_what_the_task_left() {
    let (mut renderer, _) = loaded(MAKES_A_FIELD);
    assert_eq!(renderer.draws(), 1);
    let ran = later(
        &mut renderer,
        "const p = document.createElement('div');\
         p.textContent = 'later';\
         document.documentElement.lastChild.appendChild(p);",
    );
    assert_eq!(ran, Ok(()));
    assert_eq!(renderer.draws(), 1, "a task draws nothing");
    let FromRenderer::Painted(_) = renderer.handle(ToRenderer::Paint) else {
        panic!("not painted");
    };
    assert_eq!(renderer.draws(), 2, "the paint after it does");
    let drawn = renderer
        .rendered()
        .map(|drawing| drawing.display.to_outline())
        .unwrap_or_default();
    assert!(drawn.contains("\"later\""), "{drawn}");
}

#[test]
fn a_tree_read_after_a_task_is_the_tree_the_task_left() {
    let (mut renderer, _) = loaded(MAKES_A_FIELD);
    let ran = later(
        &mut renderer,
        "document.documentElement.lastChild.firstChild.nextSibling\
         .setAttribute('aria-label', 'Renamed later');",
    );
    assert_eq!(ran, Ok(()));
    let tree = read(&mut renderer).to_outline();
    assert!(tree.contains("textbox \"Renamed later\""), "{tree}");
    assert!(!tree.contains("Made by script"), "{tree}");
}

#[test]
fn an_act_is_decided_against_the_page_a_task_left() {
    let (mut renderer, _) = loaded(MAKES_A_FIELD);
    let ran = later(
        &mut renderer,
        "document.documentElement.lastChild.firstChild.nextSibling\
         .setAttribute('aria-label', 'Renamed later');",
    );
    assert_eq!(ran, Ok(()));
    // No read in between: the decision itself must see the new name.
    let acted = renderer.handle(ToRenderer::Act {
        target: Target::Named("Renamed later".to_owned()),
        verb: Verb::PutText("found".to_owned()),
    });
    assert!(matches!(acted, FromRenderer::Acted(_)), "{acted:?}");
    let gone = renderer.handle(ToRenderer::Act {
        target: Target::Named("Made by script".to_owned()),
        verb: Verb::PutText("lost".to_owned()),
    });
    assert!(matches!(gone, FromRenderer::Refused(_)), "{gone:?}");
}

#[test]
fn a_resize_lays_out_the_document_the_page_has_not_its_markup() {
    let (mut renderer, _) = loaded(MAKES_A_FIELD);
    renderer.handle(ToRenderer::Act {
        target: Target::Named("Made by script".to_owned()),
        verb: Verb::PutText("kept".to_owned()),
    });
    let draws = renderer.draws();
    let answer = renderer.handle(ToRenderer::Resize(Size::new(150.0, 100.0)));
    assert!(matches!(answer, FromRenderer::Loaded { .. }), "{answer:?}");
    assert_eq!(renderer.draws(), draws + 1);
    let Some(drawing) = renderer.rendered() else {
        panic!("nothing was rendered");
    };
    let laid = drawing.layout.to_outline(&drawing.boxes);
    assert!(
        laid.starts_with("block flow · document → 150×60.800003 at (0, 0)"),
        "{laid}"
    );
    assert!(laid.contains("textbox \"Made by script\""), "{laid}");
    assert!(
        drawing.display.to_outline().contains("\"kept\""),
        "{}",
        drawing.display.to_outline()
    );
    // And the script did not run again: the field is there once.
    assert_eq!(laid.matches("Made by script").count(), 1, "{laid}");
}

#[test]
fn a_page_none_of_whose_scripts_may_run_never_builds_a_heap() {
    let (mut renderer, issues) = loaded(
        "<!DOCTYPE html><html><head>\
         <meta http-equiv=Content-Security-Policy content=\"script-src 'none'\">\
         </head><body><div>still here</div>\
         <script>document.documentElement.remove()</script>\
         <script src=elsewhere.js></script></body></html>",
    );
    assert_eq!(issues.len(), 2, "{issues:?}");
    assert!(renderer.event_loop().is_none(), "a heap was built");
    assert_eq!(renderer.draws(), 1);
    assert!(read(&mut renderer).to_outline().contains("still here"));
}

// --- Hostile pages -----------------------------------------------------------

#[test]
fn what_a_script_changed_before_it_threw_is_drawn() {
    let (mut renderer, issues) = loaded(
        "<!DOCTYPE html><html><body><div>before</div><script>\
         document.documentElement.lastChild.firstChild.textContent = 'changed';\
         document.documentElement.lastChild.appendChild(document);\
         </script></body></html>",
    );
    assert_eq!(issues.len(), 1, "{issues:?}");
    // The refusal is the `HierarchyRequestError` a `catch` would receive
    // (item 249), and the report names it (item 252).
    assert!(
        issues.first().is_some_and(|issue| issue.starts_with(
            "script 1: uncaught: HierarchyRequestError: a node cannot be put inside itself"
        )),
        "{issues:?}"
    );
    let tree = read(&mut renderer).to_outline();
    assert!(tree.contains("changed"), "{tree}");
    assert!(!tree.contains("before"), "{tree}");
}

#[test]
fn a_page_whose_script_removes_everything_is_drawn_empty_and_still_answers() {
    let (mut renderer, issues) = loaded(
        "<!DOCTYPE html><html><body><div>gone</div><input aria-label=Gone><script>\
         document.documentElement.remove();\
         </script></body></html>",
    );
    assert!(issues.is_empty(), "{issues:?}");
    let tree = read(&mut renderer).to_outline();
    assert!(!tree.contains("gone"), "{tree}");
    assert!(matches!(
        renderer.handle(ToRenderer::Paint),
        FromRenderer::Painted(_)
    ));
    let acted = renderer.handle(ToRenderer::Act {
        target: Target::Named("Gone".to_owned()),
        verb: Verb::PutText("nowhere".to_owned()),
    });
    assert!(matches!(acted, FromRenderer::Refused(_)), "{acted:?}");
    let resized = renderer.handle(ToRenderer::Resize(Size::new(10.0, 10.0)));
    assert!(
        matches!(resized, FromRenderer::Loaded { .. }),
        "{resized:?}"
    );
}
