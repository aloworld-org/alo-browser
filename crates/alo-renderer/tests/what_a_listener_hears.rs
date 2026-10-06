/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 254: events from script, in a page's load (ADR 0018 §§ 1–3).
//!
//! The page is the corpus case `a-script-hears-an-event`, read from the
//! corpus rather than copied, so the page this asserts on and the page whose
//! reference render is committed cannot drift apart. Its script adds capture
//! and bubble listeners to a section, a div and a button, dispatches a
//! `CustomEvent` at the button twice, and writes what was heard into two
//! paragraphs. One listener throws on each dispatch: each throw is **reported
//! in the load's issues, placed in the script**, and the dispatch carries on —
//! which is the part a reference render cannot show.

use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

const PAGE: &str = include_str!("../../alo-corpus/cases/a-script-hears-an-event/page.html");
const SHEET: &str = include_str!("../../alo-corpus/cases/a-script-hears-an-event/style.css");

const WINDOW: Size = Size {
    width: 320.0,
    height: 120.0,
};

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

/// A renderer with the page loaded, and the issues its load answered.
fn loaded() -> (Renderer, Vec<String>) {
    let mut renderer = Renderer::new(fonts());
    let page = Page::new(PAGE, WINDOW).with_sheet(SHEET);
    let issues = match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded { issues, .. } => issues,
        other => vec![format!("not loaded: {other:?}")],
    };
    (renderer, issues)
}

#[test]
fn each_listener_heard_in_order_and_the_page_says_so_in_numbers() {
    let (renderer, _) = loaded();
    let Some(drawing) = renderer.rendered() else {
        panic!("nothing was rendered");
    };
    let outline = drawing.layout.to_outline(&drawing.boxes);
    // Capture down to the button, the button's three listeners, the div's
    // bubble listener stopping the event before the section's; the passive
    // listener's `preventDefault` did nothing, so `dispatchEvent` answered
    // true; and the `once` listener ran on the first dispatch alone.
    assert!(
        outline.contains(
            "    block flow · paragraph → 304×24.296875 at (8, 44.800003)\n\
             \x20     text \"section1 div1 button2 n once div3 true\" → 272.40527×16.296875 at \
             (12, 48.800003)\n\
             \x20   block flow · paragraph → 304×24.296875 at (8, 77.09688)\n\
             \x20     text \"section1 div1 button2 n div3 true\" → 234.20605×16.296875 at \
             (12, 81.09688)\n"
        ),
        "{outline}"
    );
}

#[test]
fn a_listener_that_threw_is_reported_where_it_threw_on_each_dispatch() {
    let (_, issues) = loaded();
    let said = "script 1: uncaught: Error: a listener threw (at script 1, line 16, column 70)";
    assert_eq!(
        issues,
        [said, said],
        "one report per dispatch, and nothing else"
    );
}
