/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 366 (ADR 0038 §§ 2, 3 and 5): a page's script reads the size
//! the renderer lays it out at, and where it is scrolled to, from the
//! renderer's own view.
//!
//! - A script run as the page loads at 800 × 600 reads `innerWidth` 800 and
//!   `innerHeight` 600, and every scroll position 0.
//! - After a `Resize` to 640 × 480, a script run in the same page reads 640
//!   and 480: the view is asked at every read, never copied at load. The
//!   page is drawn at that size too, so the two agree.
//! - A new page is shown at its own size.
//!
//! What a test does between messages — queueing a task on the page's loop
//! and running it — is what a timer or an event will do once one can (items
//! 92 and 81). Each later script runs ordinarily and with the collector at
//! every allocation.

use alo_dom::Document;
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;

/// A page whose script writes what it read of its window into the root
/// element's `data-read`.
const READS: &str = "<!DOCTYPE html><html><body><script>\
document.documentElement.setAttribute('data-read', \
  innerWidth + 'x' + innerHeight + ' at ' + scrollX + ',' + scrollY + \
  ' ' + pageXOffset + ',' + pageYOffset);\
</script></body></html>";

/// The script a later task runs: the same reading, written the same way.
const READ_AGAIN: &str = "document.documentElement.setAttribute('data-read', \
  innerWidth + 'x' + innerHeight + ' at ' + scrollX + ',' + scrollY + \
  ' ' + pageXOffset + ',' + pageYOffset);";

/// A renderer with `html` loaded at `width` × `height`, and the issues its
/// load answered.
fn loaded(html: &str, width: f32, height: f32) -> (Renderer, Vec<String>) {
    let mut renderer = Renderer::new(FontDatabase::new());
    let issues = match renderer.handle(ToRenderer::Load(Box::new(Page::new(
        html,
        Size::new(width, height),
    )))) {
        FromRenderer::Loaded { issues, .. } => issues,
        other => vec![format!("not loaded: {other:?}")],
    };
    (renderer, issues)
}

/// The root element's `data-read`, if its script wrote one.
fn read(document: Option<&Document>) -> Option<String> {
    let document = document?;
    let html = document
        .descendants(document.root())
        .find(|id| document.element(*id).is_some())?;
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

#[test]
fn a_script_at_load_reads_the_size_the_page_is_laid_out_at() {
    let (renderer, issues) = loaded(READS, 800.0, 600.0);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        read(renderer.document()).as_deref(),
        Some("800x600 at 0,0 0,0")
    );
}

#[test]
fn after_a_resize_a_script_reads_the_new_size() {
    for stress in [false, true] {
        let (mut renderer, issues) = loaded(READS, 800.0, 600.0);
        assert!(issues.is_empty(), "{issues:?}");
        let resized = renderer.handle(ToRenderer::Resize(Size::new(640.0, 480.0)));
        assert!(
            matches!(resized, FromRenderer::Loaded { .. }),
            "{resized:?}"
        );
        // Nothing ran on the resize: what the page read at load stands.
        assert_eq!(
            read(renderer.document()).as_deref(),
            Some("800x600 at 0,0 0,0")
        );
        later(&mut renderer, READ_AGAIN, stress).unwrap_or_else(|why| panic!("{why}"));
        assert_eq!(
            read(renderer.document()).as_deref(),
            Some("640x480 at 0,0 0,0"),
            "stress: {stress}"
        );
        // And the page is drawn at the size it reads.
        assert_eq!(renderer.viewport(), Some(Size::new(640.0, 480.0)));
        let Some(drawing) = renderer.rendered() else {
            panic!("the page was drawn");
        };
        let widest = drawing
            .boxes
            .ids()
            .filter_map(|id| drawing.layout.get(id))
            .map(|laid| laid.border_box.size.width)
            .fold(0.0_f32, f32::max);
        assert!((widest - 640.0).abs() < 0.01, "{widest}");
    }
}

#[test]
fn a_fractional_viewport_reads_as_whole_pixels() {
    let (renderer, issues) = loaded(READS, 800.5, 599.4);
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        read(renderer.document()).as_deref(),
        Some("801x599 at 0,0 0,0")
    );
}

#[test]
fn a_new_page_is_shown_at_its_own_size() {
    let (mut renderer, _) = loaded(READS, 800.0, 600.0);
    let _ = renderer.handle(ToRenderer::Resize(Size::new(640.0, 480.0)));
    let second = renderer.handle(ToRenderer::Load(Box::new(Page::new(
        READS,
        Size::new(300.0, 200.0),
    ))));
    assert!(matches!(second, FromRenderer::Loaded { .. }), "{second:?}");
    assert_eq!(
        read(renderer.document()).as_deref(),
        Some("300x200 at 0,0 0,0")
    );
}
