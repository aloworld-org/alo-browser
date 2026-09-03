/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A link that wraps is in two places, and neither of them is the box between
//! them.
//!
//! Found by the first web page (corpus case `web-first-website`), where `link
//! "Frequently Asked Questions"` came back as 778 pixels wide starting at the
//! left margin — a rectangle covering most of the paragraph, including two
//! other links and the text between them.
//!
//! Nothing **acts** on a rectangle — ADR 0002 means no verb takes a coordinate
//! — so the cost is not a misclick. It is that "is this on screen" was answered
//! from a box the thing does not occupy, and that a person reading the tree was
//! told something untrue about where a link is.

use alo_agent::AgentTree;
use alo_box::build as build_boxes;
use alo_box::{KnownRole, Role};
use alo_css::{MediaContext, parse_stylesheet};
use alo_dom::parse_document;
use alo_layout::{Size, compute};
use alo_style::{Origin, SourcedSheet, USER_AGENT_STYLE_SHEET, resolve};
use alo_text::{Font, FontDatabase, Slant, TextMeasurer, Weight};

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
    database
}

/// A paragraph whose link is long enough to wrap, at a viewport of this size.
/// The spacer is there so that the link does not begin at the top of the page:
/// a viewport can then be short enough to exclude *every* piece of it, which is
/// the case the offscreen rule is about and which a link at y=0 cannot produce.
const PAGE: &str = "<div class=spacer></div><p>before <a href='/x'>a link long \
                    enough that it has to wrap onto a second line</a> after</p>";
const SHEET: &str = "body { margin: 0 } .spacer { height: 100px } \
                     p { margin: 0; font-family: 'DejaVu Sans'; font-size: 16px }";

/// Lay the page out at this size and hand back what the agent sees of the link.
fn link_at(width: f32, height: f32) -> (Vec<alo_layout::Rect>, bool) {
    let document = parse_document(PAGE);
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(SHEET);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(width, height), &measurer);
    let tree = AgentTree::new(&document, &boxes, &layout);

    let links = tree.with_role(&Role::Known(KnownRole::Link));
    let Some(link) = links.first() else {
        return (Vec::new(), true);
    };
    (link.rects(), link.is_offscreen())
}

#[test]
fn a_link_that_wraps_is_two_rectangles_rather_than_one() {
    let (rects, _) = link_at(300.0, 400.0);
    assert_eq!(
        rects.len(),
        2,
        "a link wrapping onto a second line should be in two places"
    );

    // And neither of them is the union: the first ends at the right edge of its
    // line, the second starts at the left of the next.
    let first = rects.first().copied().unwrap_or(alo_layout::Rect::ZERO);
    let second = rects.get(1).copied().unwrap_or(alo_layout::Rect::ZERO);
    assert!(
        second.top() > first.top(),
        "the two pieces should be on different lines"
    );
    assert!(
        first.left() > second.left(),
        "the first piece starts after the text before it; the second starts at \
         the left margin"
    );
}

/// The union covers text on the line between the two pieces, which belongs to
/// somebody else. It is still the answer to *roughly where is this*, and the
/// point of this test is that it is not the same answer.
#[test]
fn the_union_is_wider_than_anything_the_link_occupies() {
    let (rects, _) = link_at(300.0, 400.0);
    let widest = rects
        .iter()
        .map(|rect| rect.right() - rect.left())
        .fold(0.0f32, f32::max);
    let union_left = rects
        .iter()
        .map(|rect| rect.left())
        .fold(f32::MAX, f32::min);
    let union_right = rects.iter().map(|rect| rect.right()).fold(0.0f32, f32::max);
    assert!(
        union_right - union_left > widest,
        "the union should be wider than either piece, or this page is not \
         testing what it was written to test"
    );
}

// --- What changed: when a thing counts as offscreen --------------------------

/// The whole point. A viewport that cuts between the two lines leaves the first
/// piece on screen and the second below it — and answering from the union
/// would have said the same thing for the wrong reason, so the next test is the
/// one that separates them.
#[test]
fn a_link_with_one_piece_on_screen_is_on_screen() {
    // The spacer is a hundred tall, so a window of a hundred and ten shows the
    // link's first line and not its second.
    let (rects, offscreen) = link_at(300.0, 110.0);
    assert_eq!(rects.len(), 2);
    assert!(!offscreen, "its first line is visible, so it is visible");
}

/// A viewport shorter than the page, positioned so that **every** piece is
/// below it. This is the case a union gets wrong in the other direction: a
/// union that straddles the viewport edge looks visible even when neither piece
/// is inside it.
#[test]
fn a_link_is_offscreen_only_when_every_piece_of_it_is() {
    // Fifty tall, and the link starts at a hundred: every piece of it is below
    // the window.
    let (rects, offscreen) = link_at(300.0, 50.0);
    assert_eq!(rects.len(), 2, "it should still be two pieces");
    assert!(
        offscreen,
        "no piece of it is within the window, so it is offscreen"
    );
}

/// A thing that is not wrapped is one rectangle, and none of this changes for
/// it — which is what stops the change being a special case that only the
/// unusual path takes.
#[test]
fn a_thing_that_does_not_wrap_is_still_one_rectangle() {
    let document = parse_document("<p><a href='/x'>short</a></p>");
    let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
    let author = parse_stylesheet(SHEET);
    let sheets = [
        SourcedSheet::new(Origin::UserAgent, &agent),
        SourcedSheet::new(Origin::Author, &author),
    ];
    let styles = resolve(&document, &sheets, &MediaContext::default());
    let boxes = build_boxes(&document, &styles);
    let database = fonts();
    let measurer = TextMeasurer::new(&database);
    let layout = compute(&boxes, &styles, Size::new(300.0, 400.0), &measurer);
    let tree = AgentTree::new(&document, &boxes, &layout);

    let links = tree.with_role(&Role::Known(KnownRole::Link));
    let Some(link) = links.first() else {
        panic!("the link should be in the tree");
    };
    assert_eq!(link.rects().len(), 1);
    assert_eq!(
        link.rects().first().copied(),
        Some(link.rect()),
        "with one piece, the union is the piece"
    );
    assert!(!link.is_offscreen());
}
