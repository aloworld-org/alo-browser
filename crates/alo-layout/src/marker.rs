/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where a list item's marker stands: outside its first line, taking no
//! room in it (ADR 0044 § 4).
//!
//! The marker is a text box the box tree puts first among its item's
//! children. It joins none of the item's lines and is no node of the
//! layout engine's, so the item, its lines and everything in them are
//! exactly where they would be without it; once they are placed, it is
//! stood beside them here.
//!
//! - **Down**: its baseline is the item's **first line's** baseline — the
//!   first line box in the item's flow, inside a block descendant if the
//!   item starts with one, as CSS says. An item with **no line** has
//!   nothing to stand a marker on; it draws none, and that is said (item
//!   409).
//! - **Across**, from the item's content edge on the start side, by
//!   Chromium's rule (`InlineMarginsForOutside`): a **number** ends where
//!   the line starts, its suffix's space being the gap; a **disc, circle
//!   or square** starts `⌊ascent × 2 / 3⌋ + 8` before it, the ascent being
//!   the marker font's, a whole number of pixels as Chromium's font
//!   metrics give it.
//!
//! Only left-to-right lists; a right-to-left marker stands on the right
//! and comes with writing modes (item 98).

use crate::baseline::Lines;
use crate::engine::{self, Placed};
use crate::geometry::Rect;
use crate::inline::Fragment;
use crate::keyword::Positioning;
use crate::measure::MeasureText;
use crate::style;
use crate::text_style::text_style_of;
use crate::tree::BoxGeometry;
use alo_box::{BoxId, BoxTree, Inside};
use alo_css::{IssueKind, Location, StyleIssue};
use alo_style::StyleTree;
use std::collections::BTreeMap;

/// Stand every marker under `root` beside its item's first line.
///
/// A marker already placed was placed by the layout of an atomic box it is
/// inside, against lines only that layout recorded, and is left there.
pub(crate) fn place(
    boxes: &BoxTree,
    styles: &StyleTree,
    root: BoxId,
    measure: &impl MeasureText,
    placed: &mut Placed,
    issues: &mut Vec<StyleIssue>,
) {
    let Placed {
        geometry,
        fragments,
        lines,
        ..
    } = placed;
    let items: Vec<(BoxId, BoxId)> = core::iter::once(root)
        .chain(boxes.descendants(root))
        .filter_map(|item| Some((item, boxes.marker_of(item)?)))
        .collect();
    for (item, marker) in items {
        if fragments.contains_key(&marker) {
            continue;
        }
        let Some(text) = boxes.get(marker).and_then(|node| node.text()) else {
            continue;
        };
        let Some(start) = geometry.get(&item).map(|held| held.content_box().origin.x) else {
            continue;
        };
        let Some(baseline) = first_line(boxes, styles, item, lines) else {
            issues.push(StyleIssue {
                kind: IssueKind::UnsupportedValue,
                source: format!(
                    "the marker {:?} of a list item with no line to stand on is not drawn \
                     (item 409)",
                    text.trim_end()
                ),
                at: Location { line: 0, column: 0 },
            });
            continue;
        };
        let style = boxes
            .nearest_style(styles, marker)
            .map(text_style_of)
            .unwrap_or_default();
        let width = measure.measure(text, &style, None).width;
        let ascent = measure.ascender(&style);
        let descent = measure.descender(&style);
        let left = if alo_style::marker::is_symbol(text) {
            // Chromium's integer ascent, two thirds of it in whole pixels,
            // and its seven pixels of padding and one more.
            let whole = ascent.round();
            start - ((whole * 2.0 / 3.0).floor() + 8.0)
        } else {
            start - width
        };
        let rect = Rect::new(left, baseline - ascent, width, ascent + descent);
        fragments.insert(
            marker,
            vec![Fragment {
                box_id: marker,
                rect,
                text: Some(0..text.len()),
                line: 0,
                space: 0.0,
            }],
        );
        geometry.insert(
            marker,
            BoxGeometry {
                border_box: rect,
                ..BoxGeometry::default()
            },
        );
    }
}

/// Where the baseline of the first line in `id`'s flow stands, if it has
/// one: its own first line, or the first found going down into the boxes
/// in its flow, first child first. A flex or grid container, a picture and
/// an inline formatting context with no line hold none.
fn first_line(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    lines: &BTreeMap<BoxId, Lines>,
) -> Option<f32> {
    if let Some(held) = lines.get(&id) {
        return Some(held.first);
    }
    let node = boxes.get(id)?;
    if boxes.natural_size(id).is_some()
        || !matches!(node.kind.inside(), Inside::Flow | Inside::FlowRoot)
        || engine::is_inline_formatting_context(boxes, id)
    {
        return None;
    }
    boxes
        .children(id)
        .filter(|child| !boxes.is_marker(*child) && !is_out_of_flow(boxes, styles, *child))
        .find_map(|child| first_line(boxes, styles, child, lines))
}

/// Whether a box is absolutely positioned, and so in no line of its
/// parent's.
fn is_out_of_flow(boxes: &BoxTree, styles: &StyleTree, id: BoxId) -> bool {
    boxes
        .get(id)
        .and_then(|node| node.kind.node())
        .and_then(|source| styles.get(source))
        // Every issue in this style was recorded when the box was built;
        // reading it again here is not a second finding.
        .is_some_and(|computed| {
            style::read(computed, &mut Vec::new()).position == Positioning::Absolute
        })
}
