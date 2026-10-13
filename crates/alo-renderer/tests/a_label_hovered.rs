/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 554, cut from 550: a `<label>` under a person's pointer
//! hovers the control it labels, as Chrome's does (ADR 0024 § 5).
//!
//! The page is `references/label-hovered.html`, put to Chrome 154 over CDP
//! at 400 × 300: labels with a `for` naming an input, a `<div>`, a hidden
//! input, an id a `<div>` took first and a button; a label wrapping two
//! inputs; and a label with a `for` that also wraps an input. Every
//! listener writes the event's type, its target and what
//! `document.querySelectorAll(':hover')` answers there, and a script of the
//! test's own asks the same after each input. Each section of what Chrome's
//! wrote — `references/label-hovered.chrome.txt` — is compared, line for
//! line, with what this engine's write, in order. The boxes the points
//! depend on are asserted in numbers.
//!
//! **Where this engine does not follow Chrome**, the section is titled
//! `DIVERGES`, and the test asserts this engine's answer instead — and that
//! Chrome's really is different, so the record and the reason stay honest:
//! Chrome's hover on a control outlives its label's when the label's
//! control changed while it was hovered, or the label was taken out.
//! `alo-dom`'s `hover.rs` says why it is not copied.
//!
//! What is drawn under the pointer is `a_label_hovered_drawn.rs`'s.

use alo_dom::{Document, NodeId};
use alo_layout::{Rect, Size};
use alo_renderer::pointer::Pointer;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

/// The page Chrome was given.
const PAGE: &str = include_str!("references/label-hovered.html");

/// What Chrome's listeners wrote, a section per input.
const CHROME: &str = include_str!("references/label-hovered.chrome.txt");

/// What the test's own script says after each input: the hovered elements.
const NOW: &str = "say('now ' + hovered());";

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

/// Chrome's sections, in order: each title and its lines.
fn sections() -> Vec<(&'static str, Vec<&'static str>)> {
    CHROME
        .split("### ")
        .skip(1)
        .map(|section| {
            let mut lines = section.lines();
            (lines.next().unwrap_or(""), lines.collect())
        })
        .collect()
}

/// What a scenario does next.
#[derive(Debug, Clone, Copy)]
enum Step {
    /// Nothing: the test's script asks what is hovered.
    Look,
    /// A person's pointer at (`x`, `y`), and then the test's script asks.
    Point(f32, f32),
    /// A script run as a task of its own, as Chrome's `Runtime.evaluate`,
    /// which says what is hovered straight after; then the test's asks.
    Run(&'static str),
}

/// What a step is checked against.
#[derive(Debug, Clone, Copy)]
enum Expect {
    /// Chrome's section, line for line.
    Chromes,
    /// This engine's own answer, where it does not follow Chrome.
    Ours(&'static [&'static str]),
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

/// Take `steps` on a page loaded fresh, each against the next of Chrome's
/// sections from the `first` on, whose title it names: the first
/// difference, if there is one.
fn scenario(first: usize, steps: &[(Step, &str, Expect)]) -> Result<(), String> {
    let chrome = sections();
    for stress in [false, true] {
        let mut renderer = loaded(stress).ok_or("the page does not load without a word")?;
        assert_the_boxes(&renderer);
        for (at, (step, title, expect)) in steps.iter().enumerate() {
            let Some((chromes_title, chromes)) = chrome.get(first + at) else {
                return Err(format!("Chrome's record has no section for {title:?}"));
            };
            if chromes_title != title {
                return Err(format!("{title:?} is Chrome's section {chromes_title:?}"));
            }
            match step {
                Step::Look => {}
                Step::Point(x, y) => {
                    let answer = renderer.handle(ToRenderer::Pointer(Pointer::moved(*x, *y)));
                    if !matches!(&answer, FromRenderer::Delivered { issues, navigation: None, .. }
                        if issues.is_empty())
                    {
                        return Err(format!("{title}: {answer:?}"));
                    }
                }
                Step::Run(source) => {
                    let source = format!("{source}; say('just ' + hovered());");
                    run(&mut renderer, &source).map_err(|why| format!("{title}: {why}"))?;
                }
            }
            run(&mut renderer, NOW).map_err(|why| format!("{title}: {why}"))?;
            let heard = heard(&mut renderer);
            let expected: &[&str] = match expect {
                Expect::Chromes => chromes,
                Expect::Ours(ours) => {
                    if !title.starts_with("DIVERGES") || chromes.as_slice() == *ours {
                        return Err(format!("{title}: Chrome's record says the same"));
                    }
                    ours
                }
            };
            if heard != expected {
                return Err(format!(
                    "{title} (stress {stress}):\n  heard:    {heard:#?}\n  expected: {expected:#?}"
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
    drawing
        .layout
        .get(first)
        .map(|geometry| geometry.border_box)
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

/// The boxes the points fall in are where Chrome's
/// `getBoundingClientRect()` put them (the record's header).
fn assert_the_boxes(renderer: &Renderer) {
    for (id, x, y, width, height) in [
        ("l1", 0.0, 0.0, 60.0, 40.0),
        ("fw", 0.0, 200.0, 100.0, 40.0),
        ("f", 0.0, 200.0, 40.0, 20.0),
        ("l2", 100.0, 0.0, 60.0, 80.0),
        ("s2", 100.0, 0.0, 60.0, 30.0),
        ("g", 100.0, 30.0, 40.0, 20.0),
        ("g2", 100.0, 50.0, 40.0, 20.0),
        ("l3", 200.0, 0.0, 60.0, 40.0),
        ("p", 200.0, 200.0, 40.0, 40.0),
        ("l4", 300.0, 0.0, 60.0, 40.0),
        ("l5", 0.0, 100.0, 60.0, 60.0),
        ("s5", 0.0, 100.0, 60.0, 30.0),
        ("g5", 0.0, 130.0, 40.0, 20.0),
        ("l6", 100.0, 100.0, 60.0, 40.0),
        ("l7", 200.0, 100.0, 60.0, 40.0),
        ("bw", 300.0, 200.0, 60.0, 40.0),
        ("b", 300.0, 200.0, 40.0, 20.0),
    ] {
        let found = rect(renderer, id);
        assert!(
            at(found, x, y, width, height),
            "{id} is {found:?}, and Chrome's at ({x}, {y}) {width}×{height}"
        );
    }
}

/// Where nothing is but the root element.
const NOTHING: Step = Step::Point(380.0, 290.0);
/// Over `l1`, whose `for` names `f`.
const L1: Step = Step::Point(30.0, 20.0);

#[test]
fn a_hovered_label_hovers_the_control_it_labels_and_nothing_else() {
    let outcome = scenario(
        0,
        &[
            (Step::Look, "before anything", Expect::Chromes),
            (L1, "over l1, for f", Expect::Chromes),
            (Step::Point(30.0, 210.0), "over f itself", Expect::Chromes),
            (
                Step::Point(130.0, 15.0),
                "over s2 in l2, wrapping g and g2",
                Expect::Chromes,
            ),
            (Step::Point(120.0, 40.0), "over g in l2", Expect::Chromes),
            (Step::Point(120.0, 60.0), "over g2 in l2", Expect::Chromes),
            (
                Step::Point(130.0, 15.0),
                "back over s2, past g",
                Expect::Chromes,
            ),
            (
                Step::Point(230.0, 20.0),
                "over l3, for a div",
                Expect::Chromes,
            ),
            (
                Step::Point(330.0, 20.0),
                "over l4, for a hidden input",
                Expect::Chromes,
            ),
            (
                Step::Point(30.0, 115.0),
                "over s5 in l5, for f and wrapping g5",
                Expect::Chromes,
            ),
            (Step::Point(30.0, 140.0), "over g5 in l5", Expect::Chromes),
            (
                Step::Point(130.0, 120.0),
                "over l6, for an id a div took first",
                Expect::Chromes,
            ),
            (
                Step::Point(230.0, 120.0),
                "over l7, for a button",
                Expect::Chromes,
            ),
            (NOTHING, "over nothing", Expect::Chromes),
        ],
    );
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

#[test]
fn a_label_keeps_the_control_it_hovered_while_it_is_hovered() {
    let outcome = scenario(
        14,
        &[
            (L1, "over l1", Expect::Chromes),
            (
                Step::Run("document.getElementById('l1').setAttribute('for', 'g')"),
                "its for changed to g",
                Expect::Chromes,
            ),
            (Step::Point(31.0, 20.0), "within l1", Expect::Chromes),
            (
                NOTHING,
                "DIVERGES over nothing",
                Expect::Ours(&[
                    "pointerout l1 html",
                    "pointerover html html",
                    "mouseout l1 html",
                    "mouseover html html",
                    "now html",
                ]),
            ),
            (
                L1,
                "DIVERGES over l1 again",
                Expect::Ours(&[
                    "pointerout html html,body,l1,g",
                    "pointerover l1 html,body,l1,g",
                    "mouseout html html,body,l1,g",
                    "mouseover l1 html,body,l1,g",
                    "now html,body,l1,g",
                ]),
            ),
        ],
    );
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

#[test]
fn a_control_renamed_stays_hovered_until_its_label_is_left() {
    let outcome = scenario(
        19,
        &[
            (L1, "over l1", Expect::Chromes),
            (
                Step::Run("document.getElementById('f').setAttribute('id', 'z')"),
                "f renamed z",
                Expect::Chromes,
            ),
            (
                NOTHING,
                "DIVERGES over nothing, renamed",
                Expect::Ours(&[
                    "pointerout l1 html",
                    "pointerover html html",
                    "mouseout l1 html",
                    "mouseover html html",
                    "now html",
                ]),
            ),
        ],
    );
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

#[test]
fn a_control_taken_out_is_not_hovered_and_putting_it_back_does_not_hover_it() {
    let outcome = scenario(
        22,
        &[
            (L1, "over l1", Expect::Chromes),
            (
                Step::Run("window.F = document.getElementById('f'); F.remove()"),
                "f taken out",
                Expect::Chromes,
            ),
            (
                Step::Run("document.getElementById('fw').appendChild(F)"),
                "f put back",
                Expect::Chromes,
            ),
            (Step::Point(31.0, 20.0), "within l1", Expect::Chromes),
            (NOTHING, "over nothing", Expect::Chromes),
            (L1, "over l1", Expect::Chromes),
            (
                Step::Run("document.getElementById('fw').remove()"),
                "fw taken out",
                Expect::Chromes,
            ),
        ],
    );
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

#[test]
fn a_control_put_before_the_hovered_one_waits_for_the_label_to_be_hovered_again() {
    let outcome = scenario(
        29,
        &[
            (Step::Point(130.0, 15.0), "over s2", Expect::Chromes),
            (
                Step::Run(
                    "document.getElementById('l2').insertBefore(document.createElement('textarea'), \
                     document.getElementById('g')).setAttribute('id', 't')",
                ),
                "a textarea put before g",
                Expect::Chromes,
            ),
            (Step::Point(130.0, 16.0), "within s2", Expect::Chromes),
            (
                NOTHING,
                "DIVERGES over nothing, after the textarea",
                Expect::Ours(&[
                    "pointerout s2 html",
                    "pointerover html html",
                    "mouseout s2 html",
                    "mouseover html html",
                    "now html",
                ]),
            ),
            (
                Step::Point(130.0, 15.0),
                "DIVERGES over s2 again",
                Expect::Ours(&[
                    "pointerout html html,body,l2,s2,t",
                    "pointerover s2 html,body,l2,s2,t",
                    "mouseout html html,body,l2,s2,t",
                    "mouseover s2 html,body,l2,s2,t",
                    "now html,body,l2,s2,t",
                ]),
            ),
        ],
    );
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

#[test]
fn a_label_that_labelled_nothing_when_hovered_hovers_nothing_until_hovered_again() {
    let outcome = scenario(
        34,
        &[
            (Step::Point(230.0, 20.0), "over l3", Expect::Chromes),
            (
                Step::Run("document.getElementById('l3').setAttribute('for', 'f')"),
                "its for changed to f",
                Expect::Chromes,
            ),
            (Step::Point(231.0, 20.0), "within l3", Expect::Chromes),
            (NOTHING, "over nothing", Expect::Chromes),
            (Step::Point(230.0, 20.0), "over l3 again", Expect::Chromes),
        ],
    );
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

#[test]
fn a_label_taken_out_takes_its_controls_hover_with_it() {
    let outcome = scenario(
        39,
        &[
            (L1, "over l1", Expect::Chromes),
            (
                Step::Run("document.getElementById('l1').remove()"),
                "DIVERGES l1 taken out",
                Expect::Ours(&["just html,body", "now html,body"]),
            ),
        ],
    );
    if let Err(why) = outcome {
        panic!("{why}");
    }
}
