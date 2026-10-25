/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where every box ended up, in numbers.
//!
//! Positions are **on the page**, not relative to a parent. A caller asking
//! where a box is nearly always wants the page, and a caller that wants the
//! parent-relative offset can subtract — whereas the other way round means
//! walking up the tree, which is how "where is this" becomes an O(depth)
//! question inside a loop.
//!
//! Every number is a CSS pixel.

use crate::ellipsis::{Cut, Ellipses, Ellipsis};
use crate::geometry::{Edges, Rect, Size};
use crate::inline::Fragment;
use alo_box::BoxId;
use alo_css::StyleIssue;
use core::fmt;
use core::fmt::Write as _;
use std::collections::{BTreeMap, BTreeSet};

/// Where one box is and how big it is.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BoxGeometry {
    /// The box including its padding and border, positioned on the page.
    ///
    /// This is the rectangle a background is painted in and the one a person
    /// would point at, which is why it is the one named without qualification.
    pub border_box: Rect,
    /// How thick the border is on each side.
    pub border: Edges,
    /// How much padding there is inside the border.
    pub padding: Edges,
    /// How much margin there is outside the box.
    pub margin: Edges,
    /// How big the content inside is, including whatever spills out of the box
    /// — what a scrollbar would be sized against.
    pub scrollable: Size,
    /// How far the content reaches **toward the box's end edges**, measured
    /// from the top left of its padding box: rightward and downward.
    ///
    /// [`BoxGeometry::scrollable`] counts content that spills out above or to
    /// the left as well, which nothing can scroll to. This counts only what a
    /// scroll could reach, and is what an element's scrolling area is made of
    /// ([`BoxGeometry::scrolling_area`]). Never negative.
    pub reach: Size,
    /// The band this box's block-start border is drawn in, when something sits
    /// in that border rather than under it.
    ///
    /// [`None`] for every box but a `<fieldset>` showing a legend, which is
    /// the one thing in CSS that breaks a border. See [`crate::legend`], which
    /// is where it is worked out and what it means; [`BoxGeometry::border`]'s
    /// top is **zero** whenever this is set, because the band is the border.
    pub band: Option<crate::legend::Band>,
}

impl BoxGeometry {
    /// The rectangle inside the border, where padding starts.
    pub fn padding_box(self) -> Rect {
        self.border_box.shrunk_by(self.border)
    }

    /// The rectangle the content sits in, inside the padding.
    pub fn content_box(self) -> Rect {
        self.padding_box().shrunk_by(self.padding)
    }

    /// The box's **scrolling area**, as CSSOM View measures it for
    /// `scrollWidth` and `scrollHeight` (ADR 0038 § 4): its padding box,
    /// extended by its content's overflow **toward its end edges only**,
    /// right and bottom. Overflow above or to the left cannot be scrolled to,
    /// so it is not in the area.
    pub fn scrolling_area(self) -> Size {
        let padding = self.padding_box().size;
        Size::new(
            padding.width.max(self.reach.width),
            padding.height.max(self.reach.height),
        )
    }

    /// Whether the box has something to scroll at the screen's `scale`: its
    /// scrolling area ([`BoxGeometry::scrolling_area`], CSSOM's `scrollWidth`
    /// and `scrollHeight`) is larger than its padding box (`clientWidth` and
    /// `clientHeight`) by **half a device pixel** or more (queue item 645,
    /// measured against Chrome 155).
    ///
    /// Not asked of [`BoxGeometry::scrollable`] against the content box: the
    /// first is measured from the padding box's corner, so a button whose
    /// text fills its content box exactly was said to overflow by the padding
    /// its text starts after — where Chrome measures that button's
    /// `scrollHeight` and `clientHeight` as 17 and 17.
    ///
    /// Chrome scrolls a box whose content passes its content box by 0.5 CSS
    /// pixels and not by 0.45 at a scale of one, and by 0.25 but not 0.24 at
    /// two, wherever the box is on the page and with or without padding,
    /// across and down alike — its end padding added to what it scrolls to,
    /// as `taffy` adds it to [`BoxGeometry::reach`] for a block's children.
    pub fn overflows(self, scale: f32) -> bool {
        let room = self.padding_box().size;
        let area = self.scrolling_area();
        let half = 0.5 / alo_value::device_scale(scale);
        area.width - room.width >= half || area.height - room.height >= half
    }
}

impl fmt::Display for BoxGeometry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.border_box)
    }
}

/// Where every box of one document ended up.
#[derive(Debug, Clone, Default)]
pub struct LayoutTree {
    geometry: BTreeMap<BoxId, BoxGeometry>,
    fragments: BTreeMap<BoxId, Vec<Fragment>>,
    spaces: BTreeMap<BoxId, Vec<Fragment>>,
    ellipses: Ellipses,
    issues: Vec<StyleIssue>,
    viewport: Size,
    scale: f32,
    scroll_containers: BTreeSet<BoxId>,
    root: Option<BoxId>,
    overflow_given: BTreeSet<BoxId>,
}

impl LayoutTree {
    /// Where one box is, or [`None`] for a box that was not laid out.
    pub fn get(&self, id: BoxId) -> Option<BoxGeometry> {
        self.geometry.get(&id).copied()
    }

    /// The rectangle a box occupies on the page, border included.
    pub fn border_box(&self, id: BoxId) -> Option<Rect> {
        Some(self.get(id)?.border_box)
    }

    /// Everything layout could not do exactly, with the value that caused it.
    pub fn issues(&self) -> &[StyleIssue] {
        &self.issues
    }

    /// The window this layout was done for.
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// Device pixels per CSS pixel on the screen this layout was done for
    /// (ADR 0057): 1 for a layout done for no screen.
    pub fn scale(&self) -> f32 {
        alo_value::device_scale(self.scale)
    }

    /// Whether a box's content reaches far enough past it for Chrome to have
    /// something to scroll, at this layout's scale; see
    /// [`BoxGeometry::overflows`]. False for a box that was not laid out.
    pub fn overflows(&self, id: BoxId) -> bool {
        self.get(id)
            .is_some_and(|geometry| geometry.overflows(self.scale()))
    }

    /// Whether a box is a **scroll container** — one Chrome scrolls at all —
    /// whether or not it has anything to scroll; see
    /// [`crate::scroll_container`]. The root always is, standing for the
    /// viewport.
    pub fn is_scroll_container(&self, id: BoxId) -> bool {
        self.scroll_containers.contains(&id)
    }

    /// Whether Chrome would scroll a box: it is a scroll container
    /// ([`LayoutTree::is_scroll_container`]) and its content reaches far
    /// enough past it ([`LayoutTree::overflows`]). A `div` of 10 pixels
    /// holding 126 with `overflow: visible` spills and is not scrolled
    /// (queue item 661).
    ///
    /// The root answers for the viewport (queue item 662): whether the
    /// viewport's scrolling area passes the viewport
    /// ([`crate::viewport::scrolling_area`]), not whether the root's content
    /// passes the root's own box — a root as tall as its content still
    /// scrolls a page taller than the window.
    pub fn scrolls(&self, id: BoxId) -> bool {
        if self.root == Some(id) {
            return crate::viewport::overflows(self.viewport, self.get(id), self.scale());
        }
        self.is_scroll_container(id) && self.overflows(id)
    }

    /// Whether a box's `overflow` is the viewport's rather than its own —
    /// the root's, and the body's under a root of `visible` — so that it
    /// neither clips nor scrolls what it holds; see [`crate::viewport`].
    pub fn gives_its_overflow_to_the_viewport(&self, id: BoxId) -> bool {
        self.overflow_given.contains(&id)
    }

    /// How many boxes were laid out.
    pub fn len(&self) -> usize {
        self.geometry.len()
    }

    /// Whether nothing was laid out.
    pub fn is_empty(&self) -> bool {
        self.geometry.is_empty()
    }

    /// Every box, by id, in a stable order.
    pub fn iter(&self) -> impl Iterator<Item = (BoxId, BoxGeometry)> + '_ {
        self.geometry.iter().map(|(id, geometry)| (*id, *geometry))
    }

    /// The pieces a box was drawn in.
    ///
    /// A box that fits on one line has one. A box that wrapped has one per
    /// line, and **that is what should be painted**: the union of them covers
    /// the gap between the lines, so a background drawn from the union would
    /// paint across it. [`LayoutTree::border_box`] gives the union, which is
    /// the answer to "where is this" and not to "what do I draw".
    pub fn fragments(&self, id: BoxId) -> &[Fragment] {
        self.fragments.get(&id).map_or(&[], Vec::as_slice)
    }

    /// The white space with no letters a text box holds on each line that
    /// goes on past it — the space between `</a>` and `<a>`, or the one its
    /// text begins with — each a [`Fragment`] with no width whose
    /// [`Fragment::space`] is the room it takes. Not where the box is, and
    /// nothing is drawn for it but a decoration under it, over it or
    /// through it (queue item 582); one an ellipsis hid is not here.
    pub fn spaces(&self, id: BoxId) -> &[Fragment] {
        self.spaces.get(&id).map_or(&[], Vec::as_slice)
    }

    /// The "…" each of a box's lines ends in where `text-overflow` cut it,
    /// on the page, in the order of its lines; see [`crate::ellipsis`].
    pub fn ellipses(&self, id: BoxId) -> Vec<Ellipsis> {
        let Some(corner) = self.get(id).map(|geometry| geometry.border_box.origin) else {
            return Vec::new();
        };
        self.ellipses
            .by_block
            .get(&id)
            .into_iter()
            .flatten()
            .map(|ellipsis| Ellipsis {
                rect: ellipsis.rect.translated(corner),
                ..*ellipsis
            })
            .collect()
    }

    /// How much of a piece of text is drawn, when an ellipsis hid the end of
    /// its line: [`None`] when all of it is, which is every piece but those.
    ///
    /// The piece is where it was and as wide as it was; this is what paint
    /// leaves out, never what layout measured.
    ///
    /// A box can have two pieces on one line — preserved spaces after a
    /// piece, one of them a space of its own between it and the next — so a
    /// cut is the piece's whose text holds where it ends, not merely its
    /// box's on its line.
    pub fn cut(&self, fragment: &Fragment) -> Option<Cut> {
        let range = fragment.text.as_ref()?;
        self.ellipses
            .cuts
            .get(&fragment.box_id)?
            .iter()
            .find(|cut| cut.line == fragment.line && (range.start..=range.end).contains(&cut.end))
            .copied()
    }

    /// Whether an atomic box is not drawn at all, because an ellipsis hid
    /// the end of the line it stands on. It is still laid out, and an agent
    /// still reads it.
    pub fn is_hidden(&self, id: BoxId) -> bool {
        self.ellipses.hidden.contains(&id)
    }

    /// Whether a box was drawn in more than one piece.
    pub fn is_fragmented(&self, id: BoxId) -> bool {
        self.fragments(id).len() > 1
    }

    pub(crate) fn from_parts(
        geometry: BTreeMap<BoxId, BoxGeometry>,
        fragments: BTreeMap<BoxId, Vec<Fragment>>,
        issues: Vec<StyleIssue>,
        viewport: Size,
    ) -> Self {
        Self {
            geometry,
            fragments,
            spaces: BTreeMap::new(),
            ellipses: Ellipses::default(),
            issues,
            viewport,
            scale: 1.0,
            scroll_containers: BTreeSet::new(),
            root: None,
            overflow_given: BTreeSet::new(),
        }
    }

    pub(crate) fn with_scale(self, scale: f32) -> Self {
        Self { scale, ..self }
    }

    pub(crate) fn with_scroll_containers(self, scroll_containers: BTreeSet<BoxId>) -> Self {
        Self {
            scroll_containers,
            ..self
        }
    }

    /// The root's box, which answers for the viewport, and the boxes whose
    /// `overflow` the viewport takes; see [`crate::viewport`].
    pub(crate) fn with_viewport_overflow(
        self,
        root: Option<BoxId>,
        overflow_given: BTreeSet<BoxId>,
    ) -> Self {
        Self {
            root,
            overflow_given,
            ..self
        }
    }

    pub(crate) fn with_spaces(self, spaces: BTreeMap<BoxId, Vec<Fragment>>) -> Self {
        Self { spaces, ..self }
    }

    pub(crate) fn with_ellipses(self, ellipses: Ellipses) -> Self {
        Self { ellipses, ..self }
    }

    /// The layout as one line per box: what it is, and where it ended up.
    ///
    /// This is what a test asserts on. `CLAUDE.md` asks for a **layout
    /// assertion in numbers** for anything that positions or sizes, and a
    /// whole tree written out is the strongest form of one: a change that
    /// moves a box shows which box and by how much, which no assertion on a
    /// single rectangle does.
    pub fn to_outline(&self, boxes: &alo_box::BoxTree) -> String {
        let mut out = String::new();
        if let Some(root) = boxes.root() {
            self.write_outline(boxes, root, 0, &mut out);
        }
        out
    }

    fn write_outline(
        &self,
        boxes: &alo_box::BoxTree,
        id: alo_box::BoxId,
        depth: usize,
        out: &mut String,
    ) {
        let Some(node) = boxes.get(id) else { return };
        for _ in 0..depth {
            out.push_str("  ");
        }
        let description = match &node.kind {
            alo_box::BoxKind::Element { display, .. } => {
                format!("{display} · {}", node.semantics)
            }
            alo_box::BoxKind::Text { text, .. } => {
                let mut description = format!("text {text:?}");
                // What an ellipsis left of it, piece by piece. A piece kept
                // whole is cut only short of its space's decoration (queue
                // item 583), and its text is all drawn.
                for fragment in self.fragments(id) {
                    if let (Some(cut), Some(range)) = (self.cut(fragment), fragment.text.clone())
                        && cut.end < range.end
                        && let Some(drawn) = text.get(range.start..cut.end)
                    {
                        let _ = write!(description, " · drawn {drawn:?}");
                    }
                }
                description
            }
            alo_box::BoxKind::Anonymous { .. } => "anonymous".to_owned(),
        };
        let hidden = if self.is_hidden(id) { " · hidden" } else { "" };
        match self.get(id) {
            Some(geometry) => {
                let _ = writeln!(out, "{description} → {}{hidden}", geometry.border_box);
            }
            None => {
                let _ = writeln!(out, "{description} → not laid out");
            }
        }
        for ellipsis in self.ellipses(id) {
            for _ in 0..=depth {
                out.push_str("  ");
            }
            let _ = writeln!(
                out,
                "ellipsis on line {} → {}",
                ellipsis.line, ellipsis.rect
            );
        }
        for child in node.children.clone() {
            self.write_outline(boxes, child, depth + 1, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    fn geometry() -> BoxGeometry {
        BoxGeometry {
            border_box: Rect::new(10.0, 20.0, 100.0, 60.0),
            border: Edges::all(2.0),
            padding: Edges::all(8.0),
            margin: Edges::all(4.0),
            scrollable: Size::new(80.0, 40.0),
            reach: Size::new(88.0, 48.0),
            band: None,
        }
    }

    #[test]
    fn the_boxes_nest_inside_each_other_in_the_order_css_says() {
        let geometry = geometry();
        assert_eq!(geometry.border_box, Rect::new(10.0, 20.0, 100.0, 60.0));
        assert_eq!(
            geometry.padding_box(),
            Rect::new(12.0, 22.0, 96.0, 56.0),
            "inside the border",
        );
        assert_eq!(
            geometry.content_box(),
            Rect::new(20.0, 30.0, 80.0, 40.0),
            "and then inside the padding",
        );
    }

    #[test]
    fn a_box_says_whether_its_content_spills_out_of_it() {
        // Content reaching 88 × 48 of a padding box of 96 × 56.
        let fits = geometry();
        assert!(!fits.overflows(1.0), "80×40 of content in 80×40 of room");

        let spills = BoxGeometry {
            reach: Size::new(200.0, 48.0),
            ..geometry()
        };
        assert!(spills.overflows(1.0));
        let spills_down = BoxGeometry {
            reach: Size::new(88.0, 60.0),
            ..geometry()
        };
        assert!(spills_down.overflows(1.0));
    }

    #[test]
    fn the_start_padding_is_not_content_that_spills() {
        // Queue item 645: a button's text filling its content box, reached
        // from the padding box's corner, 6 across and 1 down. What the content
        // reaches is larger than the content box, and still nothing spills:
        // it is inside the padding box, 45.578125 × 17.
        let button = BoxGeometry {
            border_box: Rect::new(0.0, 38.0, 49.578_125, 21.0),
            border: Edges::all(2.0),
            padding: Edges {
                top: 1.0,
                right: 6.0,
                bottom: 1.0,
                left: 6.0,
            },
            margin: Edges::all(0.0),
            scrollable: Size::new(39.578_125, 16.0),
            reach: Size::new(39.578_125, 16.0),
            band: None,
        };
        assert!(button.reach.width > button.content_box().size.width);
        assert!(!button.overflows(1.0));
        assert!(!button.overflows(2.0));
    }

    #[test]
    fn content_spills_once_it_passes_the_edge_by_half_a_device_pixel() {
        // Measured against Chrome 155: 0.5 past the content box scrolls at a
        // scale of one and 0.45 does not; at two, 0.25 and not 0.24. Past it
        // and the end padding after it, which is what is scrolled to.
        let past = |across: f32, down: f32| BoxGeometry {
            reach: Size::new(96.0 + across, 56.0 + down),
            ..geometry()
        };
        assert!(past(0.5, 0.0).overflows(1.0));
        assert!(past(0.0, 0.5).overflows(1.0));
        assert!(!past(0.45, 0.45).overflows(1.0));
        assert!(past(0.25, 0.0).overflows(2.0));
        assert!(past(0.0, 0.25).overflows(2.0));
        assert!(!past(0.24, 0.24).overflows(2.0));
        // A scale no screen has is read as one.
        assert!(!past(0.45, 0.0).overflows(0.0));
        assert!(past(0.5, 0.0).overflows(f32::NAN));
    }

    #[test]
    fn a_layout_tree_answers_for_a_box_at_its_own_scale() {
        let id = BoxId::from_index_for_tests(0);
        let mut geometry = BTreeMap::new();
        geometry.insert(
            id,
            BoxGeometry {
                reach: Size::new(96.3, 48.0),
                ..self::geometry()
            },
        );
        let at = |scale| {
            LayoutTree::from_parts(
                geometry.clone(),
                BTreeMap::new(),
                Vec::new(),
                Size::default(),
            )
            .with_scale(scale)
        };
        assert!(!at(1.0).overflows(id), "0.3 past is under half a pixel");
        assert!(
            at(2.0).overflows(id),
            "and over half of a device pixel at 2"
        );
        assert!(!at(2.0).overflows(BoxId::from_index_for_tests(1)));
        assert!((LayoutTree::default().scale() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_box_is_scrolled_only_when_it_spills_and_is_a_scroll_container() {
        let (spills, fits) = (
            BoxId::from_index_for_tests(0),
            BoxId::from_index_for_tests(1),
        );
        let mut geometry = BTreeMap::new();
        geometry.insert(
            spills,
            BoxGeometry {
                reach: Size::new(96.0, 126.0),
                ..self::geometry()
            },
        );
        geometry.insert(fits, self::geometry());
        let tree = |containers: &[BoxId]| {
            LayoutTree::from_parts(
                geometry.clone(),
                BTreeMap::new(),
                Vec::new(),
                Size::default(),
            )
            .with_scroll_containers(containers.iter().copied().collect())
        };
        let visible = tree(&[]);
        assert!(visible.overflows(spills), "it spills");
        assert!(!visible.is_scroll_container(spills));
        assert!(!visible.scrolls(spills), "and is not scrolled");
        let auto = tree(&[spills, fits]);
        assert!(auto.scrolls(spills));
        assert!(auto.is_scroll_container(fits));
        assert!(!auto.scrolls(fits), "a container with nothing to scroll");
        assert!(!LayoutTree::default().scrolls(spills), "not laid out");
    }

    #[test]
    fn a_scrolling_area_is_the_padding_box_extended_toward_its_end_edges() {
        // Content that reaches no further than the padding box: the padding
        // box, 96 × 56.
        assert_eq!(geometry().scrolling_area(), Size::new(96.0, 56.0));
        // Content reaching 300 rightward and 20 downward: wider, and no
        // shorter than the padding box.
        let wide = BoxGeometry {
            reach: Size::new(300.0, 20.0),
            ..geometry()
        };
        assert_eq!(wide.scrolling_area(), Size::new(300.0, 56.0));
        // Spilling far out to the left counts nothing: `scrollable` is huge,
        // but what reaches rightward is within the box.
        let leftward = BoxGeometry {
            scrollable: Size::new(16_000.0, 40.0),
            ..geometry()
        };
        assert_eq!(leftward.scrolling_area(), Size::new(96.0, 56.0));
    }

    #[test]
    fn a_box_that_was_not_laid_out_says_so_rather_than_reporting_zero() {
        let tree = LayoutTree::default();
        assert_eq!(tree.get(alo_box::BoxId::from_index_for_tests(0)), None);
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.viewport(), Size::ZERO);
    }

    #[test]
    fn each_of_a_boxs_pieces_on_one_line_is_found_its_own_cut() {
        // `pre`'s "ab  cdefghij": "ab " and "cdefghij" are two pieces of one
        // box on one line, a space of its own between them. Nothing of the
        // second is kept and the first is drawn short of its space (queue
        // item 583); each is found the cut whose end its text holds.
        let id = alo_box::BoxId::from_index_for_tests(1);
        let piece = |text: core::ops::Range<usize>| Fragment {
            box_id: id,
            rect: Rect::new(0.0, 0.0, 16.0, 16.0),
            text: Some(text),
            line: 0,
            space: 0.0,
        };
        let (none, short) = (
            Cut {
                line: 0,
                end: 4,
                width: 0.0,
            },
            Cut {
                line: 0,
                end: 3,
                width: 16.0,
            },
        );
        let mut ellipses = Ellipses::default();
        ellipses.cuts.insert(id, vec![none, short]);
        let tree = LayoutTree {
            ellipses,
            ..LayoutTree::default()
        };
        assert_eq!(tree.cut(&piece(0..3)), Some(short));
        assert_eq!(tree.cut(&piece(4..12)), Some(none));
        // On another line, or holding neither end, nothing is cut.
        assert_eq!(
            tree.cut(&Fragment {
                line: 1,
                ..piece(0..3)
            }),
            None
        );
        assert_eq!(tree.cut(&piece(5..12)), None);
    }

    #[test]
    fn a_position_is_on_the_page_rather_than_relative_to_a_parent() {
        // The contract this file states, asserted so that a change to it
        // breaks a test rather than a caller.
        let geometry = geometry();
        assert_eq!(geometry.border_box.origin, Point::new(10.0, 20.0));
        assert_eq!(geometry.to_string(), "100×60 at (10, 20)");
    }
}
