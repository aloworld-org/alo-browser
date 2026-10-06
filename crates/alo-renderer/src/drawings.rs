/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What every `<svg>` box draws inside itself.
//!
//! After layout, because a drawing's viewport is its box and the box's size is
//! layout's answer (ADR 0022 § 2); before paint, which is handed the drawings
//! by box beside the pictures. Here rather than in `alo-paint` for the reason
//! pictures are: finding which element a box is and what that element holds
//! needs the document, and paint does not have one.

use alo_box::{BoxId, BoxKind, BoxTree};
use alo_dom::Document;
use alo_layout::LayoutTree;
use alo_paint::Drawing;
use alo_style::StyleTree;
use std::collections::BTreeMap;

/// The drawing of every outermost `<svg>` box that draws anything, and what
/// each left out or refused.
pub fn drawings_for(
    document: &Document,
    boxes: &BoxTree,
    styles: &StyleTree,
    layout: &LayoutTree,
) -> (BTreeMap<BoxId, Drawing>, Vec<String>) {
    let mut drawings = BTreeMap::new();
    let mut issues = Vec::new();
    for id in boxes.ids() {
        let Some(BoxKind::Element { node, .. }) = boxes.get(id).map(|box_node| &box_node.kind)
        else {
            continue;
        };
        let Some(element) = document.element(*node) else {
            continue;
        };
        if !alo_box::svg::is_outermost(document, *node, element) {
            continue;
        }
        let Some(geometry) = layout.get(id) else {
            continue;
        };
        let content = geometry.content_box();
        let drawn = alo_svg::draw(
            document,
            styles,
            *node,
            (content.size.width, content.size.height),
        );
        issues.extend(drawn.issues);
        if drawn.drawing.draws_anything() {
            drawings.insert(id, drawn.drawing);
        }
    }
    (drawings, issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_text::{FontDatabase, TextMeasurer};

    fn drawings_of(html: &str, css: &str) -> (BTreeMap<BoxId, Drawing>, Vec<String>) {
        let document = alo_dom::parse_document(html);
        let agent = alo_css::parse_stylesheet(alo_style::USER_AGENT_STYLE_SHEET);
        let author = alo_css::parse_stylesheet(css);
        let styles = alo_style::resolve(
            &document,
            &[
                alo_style::SourcedSheet::new(alo_style::Origin::UserAgent, &agent),
                alo_style::SourcedSheet::new(alo_style::Origin::Author, &author),
            ],
            &alo_css::MediaContext::default(),
        );
        let boxes = alo_box::build(&document, &styles);
        let fonts = FontDatabase::new();
        let layout = alo_layout::compute(
            &boxes,
            &styles,
            alo_layout::Size::new(200.0, 200.0),
            &TextMeasurer::new(&fonts),
        );
        drawings_for(&document, &boxes, &styles, &layout)
    }

    #[test]
    fn each_svg_box_draws_into_its_content_box() {
        let (drawings, issues) = drawings_of(
            r#"<svg viewBox="0 0 10 10"><rect width="10" height="10"/></svg><svg><path d="M0 0 oops"/></svg>"#,
            "svg { width: 40px; height: 40px; padding: 5px }",
        );
        assert_eq!(drawings.len(), 1, "the second draws nothing, and says why");
        let Some(alo_paint::DrawingItem::Fill { path, .. }) = drawings
            .values()
            .next()
            .and_then(|drawing| drawing.items().first())
        else {
            panic!("a fill");
        };
        assert_eq!(
            path.bounds(),
            Some((0.0, 0.0, 40.0, 40.0)),
            "the content box, not the padding box, is the viewport",
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
    }
}
