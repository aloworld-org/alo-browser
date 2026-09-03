/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What ADR 0005 promises, asserted.
//!
//! These are not tests of what the engine draws — the corpus does that. They
//! are tests of the **shape**: that work goes one way, that nothing is
//! ambient, that a request the renderer cannot serve leaves it usable, and
//! that what comes back says the same thing as what it came from.
//!
//! The shape is the half of ADR 0005 that is expensive to retrofit, so it is
//! the half worth pinning while it is still cheap to change.

use alo_agent::{AgentTree, ScrollBy, Target, Verb};
use alo_layout::Size;
use alo_renderer::{Failure, FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

const PAGE: &str = "<!DOCTYPE html><html><body><main>\
<h1>Invoices</h1>\
<ul><li>Invoice 11</li><li aria-selected='true'>Invoice 12</li></ul>\
<button>Save</button>\
<input id=note aria-label='Note'>\
</main></body></html>";

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

fn page() -> Page {
    Page::new(PAGE, Size::new(240.0, 200.0)).with_sheet(SHEET)
}

fn loaded() -> Renderer {
    let mut renderer = Renderer::new(fonts());
    renderer.handle(ToRenderer::Load(Box::new(page())));
    renderer
}

#[test]
fn a_renderer_with_nothing_loaded_answers_every_question_with_a_failure() {
    let mut renderer = Renderer::new(fonts());
    for work in [
        ToRenderer::Paint,
        ToRenderer::ReadTree,
        ToRenderer::Resize(Size::new(100.0, 100.0)),
        ToRenderer::Act {
            target: Target::Named("Save".to_owned()),
            verb: Verb::Activate,
        },
    ] {
        assert_eq!(
            renderer.handle(work.clone()),
            FromRenderer::Failed(Failure::NothingLoaded),
            "{work}",
        );
    }
    // And it is still usable afterwards, which is the point of a failure
    // being an answer rather than a panic.
    assert!(matches!(
        renderer.handle(ToRenderer::Load(Box::new(page()))),
        FromRenderer::Loaded { .. },
    ));
    assert!(matches!(
        renderer.handle(ToRenderer::Paint),
        FromRenderer::Painted(_),
    ));
}

#[test]
fn loading_a_page_reports_what_the_engine_refused_rather_than_swallowing_it() {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(
        Page::new("<p>hello</p>", Size::new(100.0, 100.0))
            .with_sheet("p { width: calc(1px + 2); color: not-a-colour }"),
    )));
    match answer {
        FromRenderer::Loaded { issues } => assert!(
            !issues.is_empty(),
            "a page told something impossible says so",
        ),
        other => panic!("expected a load, got {other:?}"),
    }
}

#[test]
fn painting_gives_back_pixels_the_size_of_the_window() {
    let mut renderer = loaded();
    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => {
            assert_eq!((frame.width, frame.height), (240, 200));
            assert_eq!(frame.pixels.len(), 240 * 200 * 4);
            assert!(frame.at(0, 0).is_some());
        }
        other => panic!("expected a picture, got {other:?}"),
    }
}

#[test]
fn a_window_of_no_size_is_a_failure_rather_than_an_empty_picture() {
    let mut renderer = Renderer::new(fonts());
    renderer.handle(ToRenderer::Load(Box::new(Page::new(
        PAGE,
        Size::new(0.0, 200.0),
    ))));
    assert!(matches!(
        renderer.handle(ToRenderer::Paint),
        FromRenderer::Failed(Failure::Unpaintable { .. }),
    ));
}

#[test]
fn the_snapshot_reads_exactly_as_the_tree_it_came_from() {
    // ADR 0002 forbids a second structure. A snapshot *is* a copy — it has to
    // be, because a borrow cannot cross a process — so the thing to hold is
    // that it says the same thing. If these two ever differ, one of them is
    // the structure that eventually disagrees.
    let mut renderer = loaded();
    let snapshot = match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => snapshot,
        other => panic!("expected a tree, got {other:?}"),
    };
    let inside = renderer.rendered().expect("a render");
    let tree = AgentTree::new(&inside.document, &inside.boxes, &inside.layout);
    assert_eq!(snapshot.to_outline(), tree.to_outline());
    assert!(!snapshot.is_empty());
    assert!(snapshot.nodes().len() > 3, "{}", snapshot.to_outline());
}

#[test]
fn every_node_that_crosses_carries_the_identity_a_verb_needs() {
    // ADR 0003: ids are allocated once and never reused, so a verb naming one
    // finds the same node or nothing. That is what makes acting on a snapshot
    // a moment old safe, and it is why the id has to cross with it.
    let mut renderer = loaded();
    let snapshot = match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => snapshot,
        other => panic!("expected a tree, got {other:?}"),
    };
    let button = snapshot
        .nodes()
        .into_iter()
        .find(|node| node.name.as_deref() == Some("Save"))
        .expect("the button");

    match renderer.handle(ToRenderer::Act {
        target: Target::Node(button.id),
        verb: Verb::Activate,
    }) {
        FromRenderer::Acted(outcome) => assert_eq!(outcome.node(), button.id),
        other => panic!("expected the verb to run, got {other:?}"),
    }
}

#[test]
fn a_verb_that_is_refused_is_a_result_rather_than_a_failure() {
    let mut renderer = loaded();
    let answer = renderer.handle(ToRenderer::Act {
        target: Target::Named("Nothing called this".to_owned()),
        verb: Verb::Activate,
    });
    assert!(
        matches!(answer, FromRenderer::Refused(_)),
        "ADR 0002 makes refusing a result: {answer:?}",
    );
    // Still usable.
    assert!(matches!(
        renderer.handle(ToRenderer::ReadTree),
        FromRenderer::Tree(_),
    ));
}

#[test]
fn a_verb_reaches_the_page_and_the_next_read_can_see_it() {
    let mut renderer = loaded();
    let outcome = match renderer.handle(ToRenderer::Act {
        target: Target::Named("Note".to_owned()),
        verb: Verb::PutText("typed".to_owned()),
    }) {
        FromRenderer::Acted(outcome) => outcome,
        other => panic!("expected the verb to run, got {other:?}"),
    };
    assert!(outcome.to_string().contains("typed"), "{outcome}");

    let snapshot = match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => snapshot,
        other => panic!("expected a tree, got {other:?}"),
    };
    assert!(
        snapshot.to_outline().contains("typed"),
        "what a verb did is there to be read:\n{}",
        snapshot.to_outline(),
    );
}

#[test]
fn resizing_lays_the_same_page_out_again() {
    let mut renderer = loaded();
    assert_eq!(renderer.viewport(), Some(Size::new(240.0, 200.0)));
    assert!(matches!(
        renderer.handle(ToRenderer::Resize(Size::new(120.0, 400.0))),
        FromRenderer::Loaded { .. },
    ));
    assert_eq!(renderer.viewport(), Some(Size::new(120.0, 400.0)));
    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => assert_eq!((frame.width, frame.height), (120, 400)),
        other => panic!("expected a picture, got {other:?}"),
    }
}

#[test]
fn nothing_is_ambient_so_two_renderers_answer_the_same_way() {
    // If anything reached out to a global — a font cache, a clock, a
    // thread-local — the second renderer would differ from the first, and in
    // another process it would differ from nothing at all.
    let mut first = Renderer::new(fonts());
    let mut second = Renderer::new(fonts());

    // Deliberately out of step: the second one is asked things before it is
    // loaded, and loaded second.
    assert!(matches!(
        second.handle(ToRenderer::Paint),
        FromRenderer::Failed(_),
    ));
    first.handle(ToRenderer::Load(Box::new(page())));
    second.handle(ToRenderer::Load(Box::new(page())));

    assert_eq!(
        first.handle(ToRenderer::ReadTree),
        second.handle(ToRenderer::ReadTree),
    );
    assert_eq!(
        first.handle(ToRenderer::Paint),
        second.handle(ToRenderer::Paint),
    );
}

#[test]
fn a_scroll_crosses_the_boundary_like_anything_else() {
    let mut renderer = Renderer::new(fonts());
    renderer.handle(ToRenderer::Load(Box::new(
        Page::new(
            "<!DOCTYPE html><html><body><ul id=rows aria-label='Rows'>\
<li>one</li><li>two</li><li>three</li><li>four</li><li>five</li></ul></body></html>",
            Size::new(200.0, 60.0),
        )
        .with_sheet(
            "body { margin: 0; font-family: system-ui; font-size: 14px }
             #rows { height: 40px; overflow: auto; margin: 0; padding: 0 }",
        ),
    )));
    let answer = renderer.handle(ToRenderer::Act {
        target: Target::Named("Rows".to_owned()),
        verb: Verb::Scroll(ScrollBy::ToEnd),
    });
    assert!(
        matches!(answer, FromRenderer::Acted(_)),
        "a list with more rows than room scrolls: {answer:?}",
    );
}

#[test]
fn a_page_can_come_from_something_that_was_fetched() {
    // Queue item 51's other half. The renderer is handed bytes rather than a
    // place to go and get them — ADR 0005 gives it no filesystem and no
    // network, so the fetch happens out here, in what will be the browser
    // process.
    let fetched = alo_net::fetch(&alo_net::Request::get(
        alo_url::parse("data:text/html,%3Cp%3Efetched%3C/p%3E").expect("a URL"),
    ))
    .expect("a response");

    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(Page::from_response(
        &fetched,
        Size::new(200.0, 100.0),
    ))));
    assert!(matches!(answer, FromRenderer::Loaded { .. }), "{answer:?}");

    match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => assert!(
            snapshot.to_outline().contains("fetched"),
            "the bytes reached the page:\n{}",
            snapshot.to_outline(),
        ),
        other => panic!("expected a tree, got {other:?}"),
    }
}
