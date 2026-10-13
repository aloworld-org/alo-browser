/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 550: a page's `:hover` rules are drawn under a person's
//! pointer, and taken away when it goes (ADR 0024 § 5).
//!
//! The page is `references/hover-drawn.html` at 200 × 150: a grey flex
//! container `a` holding `b` and `c`, where `a:hover` turns `a` blue,
//! `b:hover` turns `b` red and 20 px taller, and `a:hover #c` turns `c`
//! green — an element's own rule, its ancestor's, and a rule that changes
//! layout. Each picture is a reference render; each was compared, pixel for
//! pixel, with Chrome 154's screenshot of the same page under the same
//! pointer, and was the same. The boxes are asserted in numbers.
//!
//! The same page is drawn twice: as written, which never runs script, and
//! with a `<script>` of one line, which makes a heap and an event loop. Both are
//! drawn alike.
//!
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-renderer --test
//! hover_drawn_under_a_persons_pointer` rewrites the references. A rewritten
//! reference is a diff somebody reads.

use alo_dom::Document;
use alo_layout::{Rect, Size};
use alo_paint::from_png;
use alo_renderer::frame::frame_to_png;
use alo_renderer::pointer::Pointer;
use alo_renderer::{Frame, FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;
use std::path::{Path, PathBuf};

/// The page.
const PAGE: &str = include_str!("references/hover-drawn.html");

/// The page with a script, so that it runs one.
fn scripted() -> String {
    PAGE.replace("</body>", "<script>let quiet = true;</script></body>")
}

fn reference(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/references")
        .join(name)
}

/// Compare `frame` with the committed reference, or write it when updating.
/// What differs, in words, if anything does.
fn holds(name: &str, frame: &Frame) -> Result<(), String> {
    let path = reference(name);
    let drawn = frame_to_png(frame).map_err(|why| format!("{name} could not be encoded: {why}"))?;
    if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
        return std::fs::write(&path, &drawn)
            .map_err(|why| format!("{} could not be written: {why}", path.display()));
    }
    let committed = std::fs::read(&path).map_err(|why| {
        format!(
            "{} is not there ({why}); ALO_UPDATE_REFERENCES=1 writes it",
            path.display()
        )
    })?;
    let expected = from_png(&committed).map_err(|why| format!("{name} is not a picture: {why}"))?;
    same(name, frame, &Frame::from_canvas(&expected))
}

/// Whether two frames are the same picture: what differs, in words.
fn same(name: &str, frame: &Frame, expected: &Frame) -> Result<(), String> {
    if (frame.width, frame.height) != (expected.width, expected.height) {
        return Err(format!(
            "{name}: the page is {}×{}, and the reference {}×{}",
            frame.width, frame.height, expected.width, expected.height
        ));
    }
    let moved = frame
        .pixels
        .chunks(4)
        .zip(expected.pixels.chunks(4))
        .position(|(drawn, committed)| drawn != committed);
    match moved {
        None => Ok(()),
        Some(at) => {
            let width = usize::try_from(frame.width).unwrap_or(1).max(1);
            Err(format!(
                "{name}: pixels moved, the first at ({}, {}). If this is intended, \
                 ALO_UPDATE_REFERENCES=1 rewrites the reference",
                at % width,
                at / width
            ))
        }
    }
}

/// `markup` loaded at 200 × 150. No fonts: the page has no text.
fn loaded(markup: &str) -> Renderer {
    let mut renderer = Renderer::new(FontDatabase::new());
    let answer = renderer.handle(ToRenderer::Load(Box::new(Page::new(
        markup,
        Size::new(200.0, 150.0),
    ))));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "{answer:?}"
    );
    renderer
}

/// The pointer `pointer`, heard without a word, and then what is painted.
fn under(renderer: &mut Renderer, pointer: Pointer) -> Result<Frame, String> {
    let answer = renderer.handle(ToRenderer::Pointer(pointer));
    if !matches!(&answer, FromRenderer::Delivered { issues, .. } if issues.is_empty()) {
        return Err(format!("{pointer}: {answer:?}"));
    }
    painted(renderer)
}

/// What a paint answers.
fn painted(renderer: &mut Renderer) -> Result<Frame, String> {
    match renderer.handle(ToRenderer::Paint) {
        FromRenderer::Painted(frame) => Ok(frame),
        other => Err(format!("not painted: {other:?}")),
    }
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

fn by_id(document: &Document, id: &str) -> Option<alo_dom::NodeId> {
    document
        .descendants(document.root())
        .find(|node| document.element(*node).and_then(|e| e.attr("id")) == Some(id))
}

/// Whether `rect` is at (`x`, `y`), `width` × `height`, to within a 64th
/// of a pixel — Chrome's `getBoundingClientRect()` for the same state.
fn at(rect: Option<Rect>, x: f32, y: f32, width: f32, height: f32) -> bool {
    let near = |a: f32, b: f32| (a - b).abs() < 1.0 / 64.0;
    rect.is_some_and(|rect| {
        near(rect.left(), x)
            && near(rect.top(), y)
            && near(rect.size.width, width)
            && near(rect.size.height, height)
    })
}

/// The page drawn at rest, under `b`, under `c`, beside `a` and left,
/// each picture what it should be and each box where Chrome put it.
fn drawn_as_chrome_draws_it(markup: &str) -> Result<(), String> {
    let mut renderer = loaded(markup);
    let rest = painted(&mut renderer)?;
    assert_eq!(holds("hover-drawn-at-rest.png", &rest), Ok(()));
    assert!(at(rect(&renderer, "a"), 10.0, 10.0, 120.0, 100.0));
    assert!(at(rect(&renderer, "b"), 20.0, 20.0, 40.0, 30.0));

    let over_b = under(&mut renderer, Pointer::moved(30.0, 30.0))?;
    assert_eq!(holds("hover-drawn-under-b.png", &over_b), Ok(()));
    assert!(
        at(rect(&renderer, "b"), 20.0, 20.0, 40.0, 50.0),
        "b:hover is taller"
    );
    assert!(at(rect(&renderer, "c"), 70.0, 20.0, 40.0, 30.0));

    let over_c = under(&mut renderer, Pointer::moved(80.0, 30.0))?;
    assert_eq!(holds("hover-drawn-under-c.png", &over_c), Ok(()));
    assert!(at(rect(&renderer, "b"), 20.0, 20.0, 40.0, 30.0), "and back");

    for (pointer, what) in [
        (Pointer::moved(180.0, 130.0), "beside a, over the root"),
        (Pointer::moved(30.0, 30.0), "back over b"),
        (Pointer::left(-1.0, 30.0), "off the page"),
    ] {
        let frame = under(&mut renderer, pointer)?;
        let expected = if what == "back over b" {
            &over_b
        } else {
            &rest
        };
        assert_eq!(same(what, &frame, expected), Ok(()));
    }
    Ok(())
}

#[test]
fn a_hover_rule_is_drawn_under_the_pointer_on_a_page_that_never_ran_script() {
    assert_eq!(drawn_as_chrome_draws_it(PAGE), Ok(()));
}

#[test]
fn a_hover_rule_is_drawn_under_the_pointer_on_a_page_that_ran_script() {
    let markup = scripted();
    let mut renderer = loaded(&markup);
    assert!(renderer.event_loop().is_some(), "the page made a heap");
    assert_eq!(drawn_as_chrome_draws_it(&markup), Ok(()));
}

#[test]
fn a_press_where_nothing_was_said_draws_the_hover_first() {
    let mut renderer = loaded(PAGE);
    let pressed = under(&mut renderer, Pointer::down(30.0, 30.0));
    assert_eq!(
        pressed.and_then(|frame| holds("hover-drawn-under-b.png", &frame)),
        Ok(())
    );
    let released = under(&mut renderer, Pointer::up(80.0, 30.0));
    assert_eq!(
        released.and_then(|frame| holds("hover-drawn-under-c.png", &frame)),
        Ok(())
    );
}
