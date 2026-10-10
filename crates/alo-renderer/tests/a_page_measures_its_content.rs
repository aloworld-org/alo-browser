/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 370 (ADR 0038 § 4): an element's `scrollWidth` and
//! `scrollHeight`, measured by the renderer **at the moment a script asks**,
//! from the layout the page would be drawn with then, which is kept as the
//! next drawing.
//!
//! - A script that appends a tall element and reads `scrollHeight` in the
//!   same task reads the new height, and the `Paint` after it lays nothing
//!   out again. A read before anything changed is the last drawing's, and
//!   draws nothing.
//! - The root element measures the viewport when the page is shorter, and
//!   what the page reaches when it is longer; what spills out to the left or
//!   above is not counted, and what spills out to the right is.
//! - An element with no box, and one in a tree not in the document, measure
//!   `0`.
//! - A script run as the page loads measures the page parsed so far.
//! - A listener an agent's click calls measures the page as it is then.
//!
//! What a test does between messages — queueing a task on the page's loop
//! and running it — is what a timer or an event will do once one can (items
//! 92 and 81). Each later script runs ordinarily and with the collector at
//! every allocation.

use alo_agent::{Target, Verb};
use alo_dom::Document;
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

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
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

/// A renderer with `html` loaded at 800 × 600, and the issues its load
/// answered.
fn loaded(html: &str) -> (Renderer, Vec<String>) {
    let mut renderer = Renderer::new(fonts());
    let issues = match renderer.handle(ToRenderer::Load(Box::new(Page::new(
        html,
        Size::new(800.0, 600.0),
    )))) {
        FromRenderer::Loaded { issues, .. } => issues,
        other => vec![format!("not loaded: {other:?}")],
    };
    (renderer, issues)
}

/// The root element's `data-read`, if a script wrote one.
fn read(document: Option<&Document>) -> Option<String> {
    let document = document?;
    let html = document.document_element()?;
    document
        .element(html)?
        .attr("data-read")
        .map(ToOwned::to_owned)
}

/// Run `source` as a task of the page's loop, with the collector at every
/// allocation if `stress`.
fn later(renderer: &mut Renderer, source: &str, stress: bool) -> Result<(), String> {
    let looping = renderer.event_loop().ok_or("no script of the page's ran")?;
    looping.engine().objects().heap_mut().stress(stress);
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

/// `root`, `body`, and what each measures, as one string.
const MEASURE: &str = "function measure() { var root = document.documentElement, \
                       body = document.body; \
                       return root.scrollWidth + 'x' + root.scrollHeight + ' ' + \
                       body.scrollWidth + 'x' + body.scrollHeight; }";

/// A page 100 pixels tall, with no margin, that has run script.
const SHORT: &str = "<!DOCTYPE html><html><body style='margin: 0'>\
                     <div style='height: 100px'></div><script>var measured = '';</script>\
                     </body></html>";

#[test]
fn a_script_reads_what_it_just_appended_and_the_next_paint_lays_out_nothing() {
    for stress in [false, true] {
        let (mut renderer, issues) = loaded(SHORT);
        assert!(issues.is_empty(), "{issues:?}");
        let before = renderer.draws();
        later(
            &mut renderer,
            &format!(
                "{MEASURE} measured = measure(); \
                 var tall = document.createElement('div'); \
                 tall.setAttribute('style', 'height: 2000px'); \
                 document.body.appendChild(tall); \
                 measured += '; ' + measure();"
            ),
            stress,
        )
        .unwrap_or_else(|why| panic!("{why}"));
        // The first read was the load's drawing — nothing had changed — and
        // the second drew once, with the new element in it.
        assert_eq!(renderer.draws(), before + 1, "stress: {stress}");
        let painted = renderer.handle(ToRenderer::Paint);
        assert!(matches!(painted, FromRenderer::Painted(_)), "{painted:?}");
        assert_eq!(
            renderer.draws(),
            before + 1,
            "the paint is the measurement's drawing, laid out once"
        );
        // What the drawing the paint showed says, in numbers: `body` is the
        // two blocks, 2100 tall.
        let Some(drawing) = renderer.rendered() else {
            panic!("the page was drawn");
        };
        let tallest = drawing
            .boxes
            .ids()
            .filter_map(|id| drawing.layout.get(id))
            .map(|laid| laid.border_box.size.height)
            .fold(0.0_f32, f32::max);
        assert!((tallest - 2100.0).abs() < 0.01, "{tallest}");
        later(
            &mut renderer,
            "document.documentElement.setAttribute('data-read', measured);",
            stress,
        )
        .unwrap_or_else(|why| panic!("{why}"));
        // Shorter than the viewport, the root measures the viewport; longer,
        // what it reaches. `body` measures its own box either way.
        assert_eq!(
            read(renderer.document()).as_deref(),
            Some("800x600 800x100; 800x2100 800x2100"),
            "stress: {stress}"
        );
    }
}

#[test]
fn what_spills_left_or_above_is_not_counted_and_what_spills_right_is() {
    for stress in [false, true] {
        let (mut renderer, issues) = loaded(
            "<!DOCTYPE html><html><body style='margin: 0'>\
             <div style='position: absolute; left: -5000px; top: 0; width: 100px; \
               height: 10px'></div>\
             <div style='position: absolute; left: 0; top: -3000px; width: 10px; \
               height: 100px'></div>\
             <div style='height: 50px'></div><script>var ran = true;</script></body></html>",
        );
        assert!(issues.is_empty(), "{issues:?}");
        later(
            &mut renderer,
            &format!("{MEASURE} document.documentElement.setAttribute('data-read', measure());"),
            stress,
        )
        .unwrap_or_else(|why| panic!("{why}"));
        assert_eq!(
            read(renderer.document()).as_deref(),
            Some("800x600 800x50"),
            "stress: {stress}"
        );
        // One past the right edge, 1500 + 100: counted, by the root and by
        // `body`, whose overflow it is.
        later(
            &mut renderer,
            "var wide = document.createElement('div'); \
             wide.setAttribute('style', 'position: absolute; left: 1500px; top: 0; \
               width: 100px; height: 10px'); \
             document.body.appendChild(wide); \
             document.documentElement.setAttribute('data-read', measure());",
            stress,
        )
        .unwrap_or_else(|why| panic!("{why}"));
        assert_eq!(
            read(renderer.document()).as_deref(),
            Some("1600x600 1600x50"),
            "stress: {stress}"
        );
    }
}

#[test]
fn an_element_with_no_box_and_one_in_no_tree_measure_zero() {
    for stress in [false, true] {
        let (mut renderer, issues) = loaded(
            "<!DOCTYPE html><html><body><p id=gone style='display: none'>Gone</p>\
             <script>var ran = true;</script></body></html>",
        );
        assert!(issues.is_empty(), "{issues:?}");
        later(
            &mut renderer,
            "var gone = document.querySelectorAll('#gone')[0]; \
             var made = document.createElement('div'); \
             var held = document.createElement('section'); held.appendChild(made); \
             made.setAttribute('style', 'height: 500px'); \
             document.documentElement.setAttribute('data-read', \
               gone.scrollWidth + ',' + gone.scrollHeight + ',' + made.scrollWidth + ',' + \
               made.scrollHeight + ',' + held.scrollHeight);",
            stress,
        )
        .unwrap_or_else(|why| panic!("{why}"));
        assert_eq!(
            read(renderer.document()).as_deref(),
            Some("0,0,0,0,0"),
            "stress: {stress}"
        );
    }
}

#[test]
fn a_script_at_load_measures_the_page_parsed_so_far() {
    let (renderer, issues) = loaded(
        "<!DOCTYPE html><html><body style='margin: 0'><div style='height: 700px'></div>\
         <script>document.documentElement.setAttribute('data-read', \
           document.body.scrollHeight + ' of ' + document.documentElement.scrollHeight);\
         </script><div style='height: 700px'></div></body></html>",
    );
    assert!(issues.is_empty(), "{issues:?}");
    // The second block was not parsed yet: 700 of a root 700 tall.
    assert_eq!(read(renderer.document()).as_deref(), Some("700 of 700"));
    // And the page, drawn at the end of the load, has both.
    let Some(drawing) = renderer.rendered() else {
        panic!("the page was drawn");
    };
    let tallest = drawing
        .boxes
        .ids()
        .filter_map(|id| drawing.layout.get(id))
        .map(|laid| laid.border_box.size.height)
        .fold(0.0_f32, f32::max);
    assert!((tallest - 1400.0).abs() < 0.01, "{tallest}");
}

#[test]
fn a_listener_an_agents_click_calls_measures_the_page_as_it_is_then() {
    for stress in [false, true] {
        let (mut renderer, issues) = loaded(
            "<!DOCTYPE html><html><body style='margin: 0'>\
             <button style='display: block; height: 40px; margin: 0'>Go</button>\
             <script>document.body.firstChild.addEventListener('click', function () { \
               var more = document.createElement('div'); \
               more.setAttribute('style', 'height: 1000px'); \
               document.body.appendChild(more); \
               document.documentElement.setAttribute('data-read', document.body.scrollHeight); \
             });</script></body></html>",
        );
        assert!(issues.is_empty(), "{issues:?}");
        if let Some(looping) = renderer.event_loop() {
            looping.engine().objects().heap_mut().stress(stress);
        }
        let acted = renderer.handle(ToRenderer::Act {
            target: Target::Named("Go".to_owned()),
            verb: Verb::Activate,
        });
        assert!(
            matches!(&acted, FromRenderer::Acted { issues, .. } if issues.is_empty()),
            "{acted:?}"
        );
        // What the listener read is `body` as the page is drawn after it:
        // the button's 40 pixels, its own edges, and the 1000 appended.
        let Some(drawing) = renderer.rendered() else {
            panic!("the page was drawn");
        };
        let tallest = drawing
            .boxes
            .ids()
            .filter_map(|id| drawing.layout.get(id))
            .map(|laid| laid.border_box.size.height)
            .fold(0.0_f32, f32::max);
        assert!(tallest > 1040.0 && tallest < 1060.0, "{tallest}");
        assert_eq!(
            read(renderer.document()),
            Some(format!("{}", tallest.round())),
            "stress: {stress}"
        );
    }
}
