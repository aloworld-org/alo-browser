/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 563 (ADR 0057 § 3): a renderer is told the scale of the
//! screen its page is shown on with the page, and lays the page out and
//! draws it there for the page's life.
//!
//! The box measured is a `<div>` `0.31px` wide with a `0.31px` padding: a
//! length truncated onto 1/64 px at one and 1/128 px at two (ADR 0057,
//! Evidence 2), so its border box is 38/64 px wide at one and 39/64 at two,
//! as Chrome 154 makes it under `--force-device-scale-factor=2`. The frame is
//! in device pixels: the viewport times the scale.

use alo_dom::Document;
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;

const PAGE: &str = "<!doctype html><body style=\"margin:0\">\
                    <div style=\"width:0.31px;padding-left:0.31px;height:1px\"></div>";

/// The first `<div>`'s border box's width, in sixty-fourths of a CSS pixel,
/// from the renderer's last drawing.
fn div_width(renderer: &Renderer) -> Option<f32> {
    let document: &Document = renderer.document()?;
    let drawing = renderer.rendered()?;
    let div = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.name.is_html("div"))
    })?;
    drawing
        .boxes
        .ids()
        .find_map(|id| match drawing.boxes.get(id).map(|found| &found.kind) {
            Some(alo_box::BoxKind::Element { node, .. }) if *node == div => drawing
                .layout
                .get(id)
                .map(|at| at.border_box.size.width * 64.0),
            _ => None,
        })
}

/// The size of the frame a `Paint` answers with, in device pixels.
fn painted(renderer: &mut Renderer) -> Option<(u32, u32)> {
    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Some((frame.width, frame.height)),
        _ => None,
    }
}

fn load(renderer: &mut Renderer, page: Page) -> bool {
    matches!(
        renderer.handle(ToRenderer::Load(Box::new(page))),
        FromRenderer::Loaded { .. }
    )
}

#[test]
fn a_page_is_laid_out_and_painted_at_the_scale_it_was_loaded_at() {
    let mut renderer = Renderer::new(FontDatabase::new());
    assert!(load(
        &mut renderer,
        Page::new(PAGE, Size::new(100.0, 50.0)).at_scale(2.0)
    ));
    assert_eq!(div_width(&renderer), Some(39.0));
    assert_eq!(painted(&mut renderer), Some((200, 100)));
}

#[test]
fn a_page_keeps_its_scale_when_its_window_is_resized() {
    let mut renderer = Renderer::new(FontDatabase::new());
    assert!(load(
        &mut renderer,
        Page::new(PAGE, Size::new(100.0, 50.0)).at_scale(2.0)
    ));
    let _ = renderer.handle(ToRenderer::Resize(Size::new(120.0, 60.0)));
    assert_eq!(painted(&mut renderer), Some((240, 120)));
    assert_eq!(div_width(&renderer), Some(39.0));
}

#[test]
fn the_next_page_is_at_its_own_scale() {
    let mut renderer = Renderer::new(FontDatabase::new());
    assert!(load(
        &mut renderer,
        Page::new(PAGE, Size::new(100.0, 50.0)).at_scale(2.0)
    ));
    assert!(load(&mut renderer, Page::new(PAGE, Size::new(100.0, 50.0))));
    assert_eq!(div_width(&renderer), Some(38.0));
    assert_eq!(painted(&mut renderer), Some((100, 50)));
}

#[test]
fn a_page_whose_script_measures_it_is_measured_at_its_scale() {
    // The script's measurement draws the page and keeps the drawing
    // (ADR 0038 § 4), so the drawing a `Paint` shows was laid out by a
    // measurement: it must have been laid out at the page's scale too.
    let mut renderer = Renderer::new(FontDatabase::new());
    let measured = format!(
        "{PAGE}<script>document.documentElement.setAttribute('data-read', \
         '' + document.documentElement.scrollWidth)</script>"
    );
    assert!(load(
        &mut renderer,
        Page::new(measured, Size::new(100.0, 50.0)).at_scale(2.0)
    ));
    let read = renderer.document().and_then(|document| {
        let html = document.document_element()?;
        document
            .element(html)?
            .attr("data-read")
            .map(ToOwned::to_owned)
    });
    assert_eq!(read.as_deref(), Some("100"), "the script did not measure");
    let draws = renderer.draws();
    assert_eq!(div_width(&renderer), Some(39.0));
    assert_eq!(painted(&mut renderer), Some((200, 100)));
    assert_eq!(renderer.draws(), draws, "a Paint laid the page out again");
}
