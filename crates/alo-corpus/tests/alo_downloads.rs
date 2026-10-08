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
//! expression; now it compiles, four patterns and all, and stops at its
//! second line, `navigator.platform`, because no `Navigator` is built yet
//! (item 325). So neither card is marked as the visitor's and no button is
//! greyed, as before. `alo-js`'s `a_frozen_page_tests_its_platform.rs`
//! runs the page's patterns under a stand-in `navigator`.

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

#[test]
fn the_pages_script_compiles_and_stops_at_navigator() {
    let Some(case) = Case::read(&cases_directory().join("alo-downloads")) else {
        panic!("the case is read");
    };
    let mut renderer = Renderer::new(corpus_fonts());
    let page = Page::new(case.html.clone(), alo_layout::Size::new(800.0, 760.0));
    let FromRenderer::Loaded { issues, .. } = renderer.handle(ToRenderer::Load(Box::new(page)))
    else {
        panic!("the page loads");
    };
    assert!(
        !issues
            .iter()
            .any(|issue| issue.contains("regular expression")),
        "no pattern is refused any more: {issues:?}",
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("'navigator' is not defined")),
        "{issues:?}",
    );
}
