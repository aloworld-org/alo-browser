/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 542, cut from 535: a person's pointer moving over a page fires
//! what Chrome fires (ADR 0024 § 5).
//!
//! The page is `references/a-persons-pointer-moving.html`, the page put to
//! Chrome 154 over CDP at 400 × 300: `a` at the corner, a flex container
//! holding `b` and `c` side by side,
//! `c` holding the span `d`, and `o` beside `a`. Every listener writes what
//! each event carries, and each section of what Chrome's wrote —
//! `references/a-persons-pointer-moving.chrome.txt` — is compared, line for
//! line, with what this engine's write for the same input. The boxes the
//! points depend on are asserted in numbers beside them.
//!
//! Every scenario runs twice, once with the collector at every allocation,
//! and the two must say the same.

use alo_dom::{Document, NodeId};
use alo_layout::{Rect, Size};
use alo_renderer::pointer::Pointer;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

/// The page Chrome was given.
const PAGE: &str = include_str!("references/a-persons-pointer-moving.html");

/// What Chrome's listeners wrote, a section per input.
const CHROME: &str = include_str!("references/a-persons-pointer-moving.chrome.txt");

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
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

/// The lines of the section of [`CHROME`] titled `title`, the first of
/// that title at or after the `nth` section — titles repeat across
/// scenarios.
fn chromes(title: &str, nth: usize) -> Vec<&'static str> {
    CHROME
        .split("### ")
        .skip(1)
        .skip(nth)
        .find(|section| section.lines().next() == Some(title))
        .map(|section| section.lines().skip(1).collect())
        .unwrap_or_default()
}

/// What a scenario does next.
#[derive(Debug, Clone, Copy)]
enum Step {
    /// A person's pointer.
    Point(Pointer),
    /// A script run as a task of its own, as Chrome's `Runtime.evaluate`.
    Run(&'static str),
}

/// The page loaded at 400 × 300 at an address, with the collector at every
/// allocation from then on if `stress`.
fn loaded(stress: bool) -> Option<Renderer> {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(
        Page::new(PAGE, Size::new(400.0, 300.0)).at(alo_url::parse("https://example.com/").ok()?),
    )));
    if !matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()) {
        return None;
    }
    renderer
        .event_loop()?
        .engine()
        .objects()
        .heap_mut()
        .stress(stress);
    Some(renderer)
}

/// The element whose `id` is `id`.
fn by_id(document: &Document, id: &str) -> Option<NodeId> {
    document
        .descendants(document.root())
        .find(|node| document.element(*node).and_then(|e| e.attr("id")) == Some(id))
}

/// What the listeners wrote since the last time this was asked, one entry
/// each — read, and the record emptied, by a script of the test's own.
fn heard(renderer: &mut Renderer) -> Vec<String> {
    let written = renderer
        .document()
        .and_then(|document| Some(document.text_content(by_id(document, "out")?)))
        .unwrap_or_default();
    let emptied = run(renderer, "out.textContent = '';");
    let mut lines: Vec<String> = written.split('|').skip(1).map(str::to_owned).collect();
    if let Err(why) = emptied {
        lines.push(why);
    }
    lines
}

/// Run `source` as a task of its own.
fn run(renderer: &mut Renderer, source: &str) -> Result<(), String> {
    let looping = renderer.event_loop().ok_or("no script of the page's ran")?;
    looping
        .queue_script("the test's", source)
        .map_err(|stopped| stopped.to_string())?;
    let turn = looping.run_next().ok_or("the task did not run")?;
    if turn.reports.is_empty() && turn.stopped.is_none() {
        Ok(())
    } else {
        Err(format!("{:?} {:?}", turn.reports, turn.stopped))
    }
}

/// Take `steps`, each with the title of Chrome's section for it, on a page
/// loaded fresh, and compare what the listeners wrote at each with what
/// Chrome's did, from the `nth` section of Chrome's record on: the first
/// difference, if there is one.
fn scenario(steps: &[(Step, &str)], nth: usize) -> Result<(), String> {
    for stress in [false, true] {
        let mut renderer = loaded(stress).ok_or("the page does not load without a word")?;
        assert_the_boxes(&renderer);
        for (step, title) in steps {
            match step {
                Step::Point(pointer) => {
                    let answer = renderer.handle(ToRenderer::Pointer(*pointer));
                    if !matches!(&answer, FromRenderer::Delivered { issues, navigation: None, .. }
                        if issues.is_empty())
                    {
                        return Err(format!("{title}: {answer:?}"));
                    }
                }
                Step::Run(source) => {
                    run(&mut renderer, source).map_err(|why| format!("{title}: {why}"))?;
                }
            }
            if title.is_empty() {
                continue;
            }
            let expected = chromes(title, nth);
            if expected.is_empty() {
                return Err(format!("Chrome's record has no section {title:?}"));
            }
            let heard = heard(&mut renderer);
            if heard != expected {
                return Err(format!(
                    "{title} (stress {stress}):\n  heard:  {heard:#?}\n  Chrome: {expected:#?}"
                ));
            }
        }
    }
    Ok(())
}

/// Where the element `id` was laid out.
fn rect(renderer: &Renderer, id: &str) -> Option<Rect> {
    let (drawing, document) = (renderer.rendered()?, renderer.document()?);
    let node = by_id(document, id)?;
    let first = drawing
        .boxes
        .ids()
        .find(|box_id| drawing.boxes.get(*box_id).and_then(|b| b.kind.node()) == Some(node))?;
    match drawing.layout.fragments(first) {
        [] => drawing
            .layout
            .get(first)
            .map(|geometry| geometry.border_box),
        some => some.first().map(|fragment| fragment.rect),
    }
}

/// Whether `rect` is at (`x`, `y`), `width` × `height`, to within a 64th
/// of a pixel.
fn at(rect: Option<Rect>, x: f32, y: f32, width: f32, height: f32) -> bool {
    let near = |a: f32, b: f32| (a - b).abs() < 1.0 / 64.0;
    rect.is_some_and(|rect| {
        near(rect.left(), x)
            && near(rect.top(), y)
            && near(rect.size.width, width)
            && near(rect.size.height, height)
    })
}

/// The boxes are where Chrome put them, the span holds the point it was
/// entered at, and the points over `c` alone are not over it.
fn assert_the_boxes(renderer: &Renderer) {
    assert!(at(rect(renderer, "a"), 0.0, 0.0, 200.0, 200.0));
    assert!(at(rect(renderer, "b"), 20.0, 20.0, 60.0, 60.0));
    assert!(at(rect(renderer, "c"), 100.0, 20.0, 60.0, 60.0));
    assert!(at(rect(renderer, "o"), 300.0, 0.0, 50.0, 50.0));
    let span = rect(renderer, "d");
    assert!(
        span.is_some_and(|rect| {
            (rect.left()..rect.right()).contains(&103.0)
                && (rect.top()..rect.bottom()).contains(&25.0)
                && rect.right() < 130.0
                && rect.bottom() < 60.0
        }),
        "the span holds (103, 25), and not (130, 60): {span:?}"
    );
}

/// The pointer moving across the page and off it, as Chrome's first
/// scenario; `off` is how it goes off.
fn moves(off: Pointer) -> Vec<(Step, &'static str)> {
    vec![
        (
            Step::Point(Pointer::moved(30.6, 30.3)),
            "first move into b at 30.6,30.3",
        ),
        (Step::Point(Pointer::moved(35.0, 35.0)), "within b"),
        (Step::Point(Pointer::moved(130.0, 60.0)), "b -> c"),
        (Step::Point(Pointer::moved(103.0, 25.0)), "c -> d (span)"),
        (Step::Point(Pointer::moved(190.0, 190.0)), "d -> a"),
        (Step::Point(Pointer::moved(320.0, 10.0)), "a -> o"),
        (
            Step::Point(Pointer::moved(380.0, 250.0)),
            "o -> root (nothing)",
        ),
        (Step::Point(Pointer::moved(30.0, 30.0)), "root -> b"),
        (Step::Point(off), "b -> outside viewport"),
        (Step::Point(Pointer::moved(30.0, 30.0)), "outside -> b"),
    ]
}

#[test]
fn moving_across_the_page_fires_chromes_crossings_and_moves() {
    let outcome = scenario(&moves(Pointer::moved(-5.0, 30.0)), 0);
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

#[test]
fn leaving_the_page_is_moving_outside_its_viewport() {
    let outcome = scenario(&moves(Pointer::left(-5.0, 30.0)), 0);
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

#[test]
fn a_press_and_a_release_cross_first_carrying_the_button_that_changed() {
    let outcome = scenario(
        &[
            (
                Step::Point(Pointer::down(30.0, 30.0)),
                "press at b, nothing said before",
            ),
            (Step::Point(Pointer::up(130.0, 60.0)), "release at c"),
        ],
        0,
    );
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

#[test]
fn a_drag_reports_the_button_held_and_outside_the_viewport_is_the_root() {
    let outcome = scenario(
        &[
            (Step::Point(Pointer::moved(30.0, 30.0)), "move b"),
            (Step::Point(Pointer::down(30.0, 30.0)), "press b"),
            (Step::Point(Pointer::moved(130.0, 60.0)), "drag to c"),
            (Step::Point(Pointer::moved(-5.0, 60.0)), "drag outside"),
            (Step::Point(Pointer::up(-5.0, 60.0)), "release outside"),
            (Step::Point(Pointer::moved(30.0, 30.0)), "back to b"),
        ],
        12,
    );
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

#[test]
fn a_cancelled_pointerdown_holds_back_every_mousemove_until_the_release() {
    let outcome = scenario(
        &[
            (Step::Run("cancelDown = true;"), ""),
            (Step::Point(Pointer::moved(30.0, 30.0)), "into b"),
            (Step::Point(Pointer::down(30.0, 30.0)), "press b cancelled"),
            (
                Step::Point(Pointer::moved(130.0, 60.0)),
                "drag to c after cancelled down",
            ),
            (Step::Point(Pointer::up(130.0, 60.0)), "release c"),
            (Step::Point(Pointer::moved(131.0, 60.0)), "move c after"),
        ],
        18,
    );
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

#[test]
fn a_node_removed_from_under_the_pointer_is_not_told_it_was_left() {
    let outcome = scenario(
        &[
            (Step::Point(Pointer::moved(70.0, 70.0)), "into b at 70,70"),
            (Step::Run("document.getElementById('b').remove();"), ""),
            (
                Step::Point(Pointer::moved(130.0, 60.0)),
                "b removed, straight to c",
            ),
        ],
        23,
    );
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

#[test]
fn a_removed_ancestor_is_left_from_the_nearest_node_still_in_the_document() {
    let outcome = scenario(
        &[
            (Step::Point(Pointer::moved(70.0, 70.0)), "into b at 70,70"),
            (Step::Run("document.getElementById('a').remove();"), ""),
            (
                Step::Point(Pointer::moved(71.0, 70.0)),
                "a removed, move (now html)",
            ),
        ],
        25,
    );
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

#[test]
fn a_page_that_never_ran_script_is_moved_over_and_left_and_makes_no_heap() {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(Page::new(
        "<body style=margin:0><div style='width:100px;height:100px'></div>",
        Size::new(400.0, 300.0),
    ))));
    assert!(matches!(answer, FromRenderer::Loaded { .. }), "{answer:?}");
    for pointer in [
        Pointer::moved(10.0, 10.0),
        Pointer::moved(200.0, 200.0),
        Pointer::left(-1.0, 10.0),
        Pointer::down(10.0, 10.0),
        Pointer::moved(-50.0, 10.0),
        Pointer::up(-50.0, 10.0),
    ] {
        let answer = renderer.handle(ToRenderer::Pointer(pointer));
        assert!(
            matches!(&answer, FromRenderer::Delivered { issues, .. } if issues.is_empty()),
            "{pointer}: {answer:?}"
        );
    }
    assert!(
        renderer.event_loop().is_none(),
        "nobody listens, so no heap"
    );
}

#[test]
fn a_move_with_nothing_loaded_is_refused() {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Pointer(Pointer::moved(1.0, 1.0)));
    assert!(matches!(answer, FromRenderer::Failed(_)), "{answer:?}");
}
