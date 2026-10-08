/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's downloads page, in numbers: the page that opened queue item 319.
//!
//! `cases/alo-downloads` pins the whole page as files. This says out loud
//! the part item 319 is closed by: the note under the two cards is three
//! paragraphs' worth of text separated by `<br><br>`, and each `<br>` ends
//! its line. Before 319 the note was five lines with every sentence run
//! into the one before it; it is eight, two of them blank.
//!
//! It also pins what the page's script does today, because that is what
//! the render shows. Until item 74 it was refused at its first regular
//! expression, and until item 325 it stopped at its second line,
//! `navigator.platform`. Since item 327 it reads the `navigator` the
//! browser was told, marks that system's card `.rec` and shows its "Your
//! device" badge — for a Mac and a Windows machine, and neither on Linux.
//! Since item 329 its `querySelectorAll` finds the two buttons, and it stops
//! at the `forEach` it calls on them (item 331), so no button is greyed. The
//! corpus renders it as the Mac `alo_corpus::SYSTEM` says it is.

use alo_corpus::{Case, Rendering, cases_directory, corpus_fonts};
use alo_layout::Rect;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};

/// The note's line height: `0.86rem` at `line-height: 1.55` from `body`.
const NOTE_LINE: f32 = 0.86 * 16.0 * 1.55;

/// The case, read and rendered.
fn downloads() -> Option<(Case, Rendering)> {
    let case = Case::read(&cases_directory().join("alo-downloads"))?;
    let rendering = Rendering::of(&case).ok()?;
    Some((case, rendering))
}

/// The note's border box, the border boxes of the `<br>`s in it in document
/// order, and the box of the text that starts "Prefer the web?".
fn the_note(rendering: &Rendering) -> Option<(Rect, Vec<Rect>, Rect)> {
    let document = rendering.document()?;
    let drawing = rendering.drawing()?;
    let boxes = &drawing.boxes;
    let note = boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|node| &node.kind),
            Some(alo_box::BoxKind::Element { node, .. })
                if document
                    .element(*node)
                    .is_some_and(|element| element.attr("class") == Some("note"))
        )
    })?;
    let inside = boxes.descendants(note);
    let breaks = inside
        .iter()
        .filter(|id| boxes.is_forced_break(**id))
        .map(|id| drawing.layout.get(*id).map(|geometry| geometry.border_box))
        .collect::<Option<Vec<Rect>>>()?;
    let prefer = inside.iter().find(|id| {
        boxes
            .get(**id)
            .and_then(alo_box::BoxNode::text)
            .is_some_and(|text| text.trim_start().starts_with("Prefer the web?"))
    })?;
    let prefer = drawing.layout.get(*prefer)?.border_box;
    Some((drawing.layout.get(note)?.border_box, breaks, prefer))
}

#[test]
fn each_br_in_the_note_ends_its_line() {
    let Some((_, rendering)) = downloads() else {
        panic!("the case renders");
    };
    let Some((note, breaks, prefer)) = the_note(&rendering) else {
        panic!("the note, its breaks and its second sentence are laid out");
    };
    assert_eq!(breaks.len(), 4, "two pairs of <br>");

    // Eight lines under a 1 px border and 18 px of padding: two of the first
    // sentence, a blank one, the second sentence, a blank one, and three of
    // the third. Before item 319 it was five lines, 125.64 tall.
    let tall = 1.0 + 18.0 + 8.0 * NOTE_LINE;
    assert!(
        (note.size.height - tall).abs() < 0.01,
        "the note is {} tall, not {tall}",
        note.size.height,
    );

    // The breaks are one line apart: the first ends the first sentence's
    // second line, the second is a blank line of its own, the third ends the
    // second sentence and the fourth is the second blank line.
    for pair in breaks.windows(2) {
        let [above, below] = pair else { continue };
        assert!(
            (below.top() - above.top() - NOTE_LINE).abs() < 0.01,
            "{above:?} then {below:?}",
        );
    }
    // A break takes no room across its line. The two alone on theirs stand
    // at the note's left edge, which is the page's 40 px gutter.
    assert!(breaks.iter().all(|held| held.size.width == 0.0));
    let alone: Vec<f32> = [breaks.get(1), breaks.get(3)]
        .into_iter()
        .flatten()
        .map(|held| held.left())
        .collect();
    assert_eq!(alone, [40.0, 40.0]);

    // The second sentence starts a line of its own, at the left edge and not
    // after its leading space, one line under the blank one. It is in the
    // same font as the breaks, so it sits in its line exactly as they do.
    assert!(
        (prefer.left() - 40.0).abs() < 0.001,
        "\"Prefer the web?\" starts at {}",
        prefer.left(),
    );
    let Some(blank) = breaks.get(1) else {
        panic!("a second break");
    };
    assert!(
        (prefer.top() - blank.top() - NOTE_LINE).abs() < 0.01,
        "{prefer:?} under {blank:?}",
    );
}

/// ADR 0030 § 2's rows, and which card each marks: the Mac's, the
/// Windows machine's, or — on Linux, where neither of the script's branches
/// runs — none.
const SYSTEMS: [(&str, &str, Option<&str>); 3] = [
    (
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) alo/0.0",
        "MacIntel",
        Some("mac"),
    ),
    (
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) alo/0.0",
        "Win32",
        Some("win"),
    ),
    (
        "Mozilla/5.0 (X11; Linux x86_64) alo/0.0",
        "Linux x86_64",
        None,
    ),
];

/// Whether the element with `id` is marked as the visitor's, in the
/// document — its card's `class` has `rec` and its badge no `hidden` — and
/// in the box tree: the badge has a box only when it is shown.
fn marked(renderer: &Renderer, id: &str) -> Option<(bool, bool, bool)> {
    let document = renderer.document()?;
    let element = |wanted: &str| {
        document.descendants(document.root()).find(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.attr("id") == Some(wanted))
        })
    };
    let card = element(&format!("card-{id}"))?;
    let badge = element(&format!("badge-{id}"))?;
    let rec = document
        .element(card)?
        .attr("class")?
        .split_ascii_whitespace()
        .any(|token| token == "rec");
    let shown = document.element(badge)?.attr("hidden").is_none();
    let boxes = &renderer.rendered()?.boxes;
    let drawn = boxes.ids().any(|held| {
        matches!(
            boxes.get(held).map(|node| &node.kind),
            Some(alo_box::BoxKind::Element { node, .. }) if *node == badge
        )
    });
    Some((rec, shown, drawn))
}

#[test]
fn the_pages_script_marks_the_card_of_the_system_it_is_told() {
    let Some(case) = Case::read(&cases_directory().join("alo-downloads")) else {
        panic!("the case is read");
    };
    for (user_agent, platform, chosen) in SYSTEMS {
        let mut renderer = Renderer::new(corpus_fonts());
        let mut page = Page::new(case.html.clone(), alo_layout::Size::new(800.0, 780.0));
        page.user_agent = user_agent.to_owned();
        page.platform = platform.to_owned();
        let FromRenderer::Loaded { issues, .. } = renderer.handle(ToRenderer::Load(Box::new(page)))
        else {
            panic!("the page loads");
        };
        // It runs past both branches now, every system alike, and past the
        // `querySelectorAll` that finds the buttons (queue item 329), and
        // stops at the `forEach` that would grey them (queue item 331). The
        // position is the call's start for either stop; `alo-bindings`'
        // `what_a_selector_finds.rs` runs the page's query and says which.
        assert!(
            issues.iter().any(|issue| issue.contains(
                "uncaught: TypeError: undefined is not a function (at script 1, line 17, column 7)"
            )),
            "{platform}: {issues:?}",
        );
        for id in ["mac", "win"] {
            let mark = chosen == Some(id);
            assert_eq!(
                marked(&renderer, id),
                Some((mark, mark, mark)),
                "{platform}: the {id} card is marked {mark}",
            );
        }
    }
}

#[test]
fn the_corpus_renders_it_as_the_mac_it_says_it_is() {
    let Some((_, Rendering::Loaded(renderer))) = downloads() else {
        panic!("the case is loaded by a renderer");
    };
    assert_eq!(alo_corpus::SYSTEM.platform, "MacIntel");
    assert_eq!(marked(&renderer, "mac"), Some((true, true, true)));
    assert_eq!(marked(&renderer, "win"), Some((false, false, false)));
}
