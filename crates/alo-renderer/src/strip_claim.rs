/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a person's press on the tab strip claims (ADR 0024 § 5, queue item
//! 539).
//!
//! *"What a click on the strip means is claimed and decided, ADR 0020's
//! shape: the strip's renderer answers with a claim — select tab 4, close
//! tab 4, open a new tab — and the browser process acts only on a claim
//! about a tab it holds, answering a pointer event it sent."* This is the
//! claim, and how the strip's renderer finds it; [`crate::strip_decide`] is
//! the deciding.
//!
//! # A tab is named by its place
//!
//! The strip's renderer is told the tabs as data and in order
//! ([`crate::strip::Strip`]), and knows no tab by any other name: a claim
//! says *the tab at this place in the strip you sent me*, and the browser
//! process, which kept which tabs those were, decides which tab that is.
//! So a strip renderer that lies can name only a place, and the worst it
//! can do is choose among a person's own tabs at the moment they pressed.
//!
//! # Which tab a press is on
//!
//! The element under the point, found as a page's is ([`crate::hit`]), and
//! the nearest `tab` in the strip's `tablist` that holds it — the tab's own
//! box, its title, the mark that it is loading, the word that it has gone.
//! A press anywhere else — the gap between two tabs, above them, past the
//! last — is on no tab, and claims nothing.
//!
//! Only selecting is claimed: the strip draws no control to close a tab or
//! to open one yet, so no press can mean either (queue item 558).

use crate::hit;
use crate::pipeline::Drawing;
use alo_dom::{Document, NodeId};
use alo_paint::Point;

/// What a press on the strip means, as its renderer claims it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claim {
    /// Select the tab at this place in the strip, counting from zero.
    Select {
        /// Its place, in the order the strip was sent.
        at: usize,
    },
}

/// What a press at `at`, in the strip's CSS pixels, claims in `drawing`, a
/// drawing of `document`, the strip's document: the tab it is on, or
/// [`None`] when it is on no tab.
pub fn claimed(drawing: &Drawing, document: &Document, at: Point) -> Option<Claim> {
    let found = hit::element_at(drawing, document, at)?;
    let tab = tab_holding(document, found)?;
    let list = document.parent(tab)?;
    let place = document
        .children(list)
        .filter(|&child| is_tab(document, child))
        .position(|child| child == tab)?;
    Some(Claim::Select { at: place })
}

/// The nearest element from `node` up that is a tab in a `tablist`.
fn tab_holding(document: &Document, node: NodeId) -> Option<NodeId> {
    let mut next = Some(node);
    while let Some(id) = next {
        let parent = document.parent(id);
        if is_tab(document, id)
            && parent
                .is_some_and(|list| document.attribute_by_name(list, "role") == Some("tablist"))
        {
            return Some(id);
        }
        next = parent;
    }
    None
}

/// Whether `node` is an element whose role is `tab`.
fn is_tab(document: &Document, node: NodeId) -> bool {
    document.attribute_by_name(node, "role") == Some("tab")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{FromRenderer, ToRenderer};
    use crate::renderer::Renderer;
    use crate::strip::{Condition, Strip, StripTab};
    use alo_layout::Size;
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
        database.map_generic("system-ui", "DejaVu Sans");
        database.map_generic("sans-serif", "DejaVu Sans");
        database
    }

    /// What a press at (`x`, `y`) claims on a strip of three tabs, 480 × 36:
    /// the second loading, the third gone.
    fn pressed(x: f32, y: f32) -> FromRenderer {
        let mut renderer = Renderer::new(fonts());
        let strip = Strip {
            tabs: vec![
                StripTab::new("Inbox", "https://mail.example/"),
                StripTab::new("Invoices", "https://books.example/")
                    .selected()
                    .in_condition(Condition::Loading),
                StripTab::new("Calendar", "https://cal.example/").in_condition(Condition::Gone),
            ],
            viewport: Size::new(480.0, 36.0),
        };
        let built = renderer.handle(ToRenderer::Strip(Box::new(strip)));
        assert!(matches!(built, FromRenderer::Loaded { .. }), "{built:?}");
        renderer.handle(ToRenderer::StripPress(Point::new(x, y)))
    }

    fn select(at: usize) -> FromRenderer {
        FromRenderer::Claimed(Some(Claim::Select { at }))
    }

    #[test]
    fn a_press_on_a_tab_claims_its_place() {
        // Three tabs of 152 from x 8, 4 apart, from y 6 to the bottom
        // (`tests/the_tab_strip.rs` asserts the boxes), their top corners
        // rounded by 8.
        assert_eq!(pressed(8.0, 35.5), select(0));
        assert_eq!(pressed(84.0, 6.0), select(0));
        assert_eq!(pressed(159.5, 35.5), select(0));
        assert_eq!(pressed(164.0, 20.0), select(1));
        assert_eq!(pressed(320.0, 20.0), select(2));
        assert_eq!(pressed(471.5, 30.0), select(2));
    }

    #[test]
    fn a_press_on_what_a_tab_holds_is_on_the_tab() {
        // The title's text, the loading mark at the start of the second tab,
        // and the third's "Stopped" at its end.
        assert_eq!(pressed(30.0, 21.0), select(0));
        assert_eq!(pressed(179.0, 21.0), select(1));
        assert_eq!(pressed(440.0, 21.0), select(2));
    }

    #[test]
    fn a_press_on_no_tab_claims_nothing() {
        for (x, y) in [
            // Outside the first tab's rounded corner.
            (8.0, 6.0),
            (162.0, 20.0),
            (100.0, 3.0),
            (4.0, 20.0),
            (476.0, 20.0),
            (-1.0, 20.0),
            (100.0, 36.0),
        ] {
            assert_eq!(
                pressed(x, y),
                FromRenderer::Claimed(None),
                "({x}, {y}) is on no tab"
            );
        }
    }

    #[test]
    fn a_renderer_holding_a_page_claims_nothing_of_it() {
        let mut renderer = Renderer::new(fonts());
        let page = crate::page::Page::new(
            "<div role=tablist><div role=tab style='height:40px'>a</div></div>",
            Size::new(200.0, 100.0),
        );
        let loaded = renderer.handle(ToRenderer::Load(Box::new(page)));
        assert!(matches!(loaded, FromRenderer::Loaded { .. }), "{loaded:?}");
        assert_eq!(
            renderer.handle(ToRenderer::StripPress(Point::new(10.0, 10.0))),
            FromRenderer::Failed(crate::message::Failure::NotTheStrip)
        );
        // Nor does one that held the strip and was then given a page.
        let mut was_strip = Renderer::new(fonts());
        let strip = Strip {
            tabs: vec![StripTab::new("a", "about:blank")],
            viewport: Size::new(200.0, 36.0),
        };
        let _ = was_strip.handle(ToRenderer::Strip(Box::new(strip)));
        assert!(matches!(
            was_strip.handle(ToRenderer::StripPress(Point::new(20.0, 20.0))),
            FromRenderer::Claimed(Some(_))
        ));
        let page = crate::page::Page::new("<p>a page</p>", Size::new(200.0, 100.0));
        let _ = was_strip.handle(ToRenderer::Load(Box::new(page)));
        assert_eq!(
            was_strip.handle(ToRenderer::StripPress(Point::new(20.0, 20.0))),
            FromRenderer::Failed(crate::message::Failure::NotTheStrip)
        );
        let mut empty = Renderer::new(fonts());
        assert_eq!(
            empty.handle(ToRenderer::StripPress(Point::new(10.0, 10.0))),
            FromRenderer::Failed(crate::message::Failure::NotTheStrip)
        );
    }
}
