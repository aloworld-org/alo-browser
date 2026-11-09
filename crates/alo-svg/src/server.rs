/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A paint server found by its name, and what is drawn in its place when it
//! cannot be used (queue item 275, ADR 0107).
//!
//! `fill="url(#teal)"` names an element anywhere in the document — in this
//! `<svg>`'s `<defs>`, or in another `<svg>` on the same page, as SVG
//! allows. The first element with the `id` is the one named, as
//! `getElementById` answers. What is drawn:
//!
//! - **a `<linearGradient>`**: the colours along its line
//!   ([`crate::gradient`]);
//! - **anything else** — a `<radialGradient>` or a `<pattern>`, which this
//!   engine does not draw yet, an element that is no paint server, a name
//!   nothing has, or a reference into another document, which is never
//!   followed — the colour written after the reference if there is one,
//!   and nothing if there is not, which is what SVG 2 says to do with a
//!   reference that cannot be used. Each is recorded.

use crate::bbox::Rect;
use crate::length::Viewport;
use crate::paint::{Brush, Server};
use alo_dom::{Document, Namespace, NodeId};
use alo_paint::Ramp;
use alo_style::StyleTree;
use alo_value::{FontMetrics, Matrix, Rgba};
use std::collections::HashMap;

/// What a shape is painted with once every reference is followed.
#[derive(Debug, Clone, PartialEq)]
pub enum Shade {
    /// One colour.
    Colour(Rgba),
    /// A colour along a line, in the drawing's coordinates.
    Ramp(Ramp),
}

/// What a paint server is measured against: the shape it paints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    /// The shape's object bounding box, in its own user space, which
    /// `objectBoundingBox` units are fractions of.
    pub bbox: Option<Rect>,
    /// From the shape's user space to the drawing's coordinates.
    pub matrix: Matrix,
    /// The viewport a percentage in user space is a share of.
    pub viewport: Viewport,
    /// What an `em` is, where the shape is.
    pub metrics: FontMetrics,
}

/// Every element with an `id` in a document, by it, the first one winning.
#[derive(Debug, Clone, Default)]
pub struct Servers {
    by_id: HashMap<String, NodeId>,
}

impl Servers {
    /// Every element with an `id` in `document`.
    pub fn of(document: &Document) -> Self {
        let mut by_id = HashMap::new();
        for id in document.descendants(document.root()) {
            if let Some(name) = document.element(id).and_then(|element| element.attr("id")) {
                by_id.entry(name.to_owned()).or_insert(id);
            }
        }
        Self { by_id }
    }

    /// The element named `id`.
    pub fn named(&self, id: &str) -> Option<NodeId> {
        self.by_id.get(id).copied()
    }
}

/// What a brush paints `target` with, or [`None`] for nothing.
///
/// # Errors
///
/// A gradient past one of [`crate::bounds`]'s bounds, which refuses the
/// whole drawing.
pub fn shade(
    brush: Brush,
    document: &Document,
    styles: &StyleTree,
    servers: &Servers,
    target: &Target,
    issues: &mut Vec<String>,
) -> Result<Option<Shade>, String> {
    let server = match brush {
        Brush::Colour(color) => return Ok(Some(Shade::Colour(color))),
        Brush::Server(server) => server,
    };
    let Some(id) = &server.id else {
        return Ok(instead(
            &server,
            "a paint server in another document is never followed",
            issues,
        ));
    };
    let Some(node) = servers.named(id) else {
        return Ok(instead(
            &server,
            "nothing in the document has that id",
            issues,
        ));
    };
    let Some(element) = document
        .element(node)
        .filter(|element| element.name.ns == Namespace::Svg)
    else {
        return Ok(instead(&server, "not a paint server", issues));
    };
    match &*element.name.local {
        "linearGradient" => {
            crate::gradient::linear(document, styles, node, &server, target, issues)
        }
        "radialGradient" => Ok(instead(
            &server,
            "a <radialGradient> is not drawn yet (item 910)",
            issues,
        )),
        "pattern" => Ok(instead(
            &server,
            "a <pattern> is not drawn yet (item 911)",
            issues,
        )),
        _ => Ok(instead(&server, "not a paint server", issues)),
    }
}

/// What is drawn in a server's place, recorded with why.
fn instead(server: &Server, why: &str, issues: &mut Vec<String>) -> Option<Shade> {
    let Server {
        property, written, ..
    } = server;
    if let Some(color) = server.fallback {
        issues.push(format!("{property}: {written}: {why}, so its fallback"));
        return Some(Shade::Colour(color));
    }
    issues.push(format!(
        "{property}: {written}: {why}, and there is no fallback"
    ));
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::parse_document;

    fn server(id: Option<&str>, fallback: Option<Rgba>) -> Brush {
        Brush::Server(Server {
            property: "fill",
            written: "url(#x)".to_owned(),
            id: id.map(str::to_owned),
            fallback,
            opacity: 1.0,
        })
    }

    fn shaded(markup: &str, brush: Brush) -> (Option<Shade>, Vec<String>) {
        let document = parse_document(markup);
        let styles = alo_style::resolve(&document, &[], &alo_css::MediaContext::default());
        let target = Target {
            bbox: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            }),
            matrix: Matrix::IDENTITY,
            viewport: Viewport {
                width: 10.0,
                height: 10.0,
            },
            metrics: FontMetrics::default(),
        };
        let mut issues = Vec::new();
        let found = shade(
            brush,
            &document,
            &styles,
            &Servers::of(&document),
            &target,
            &mut issues,
        );
        (found.ok().flatten(), issues)
    }

    #[test]
    fn a_colour_is_itself() {
        let (found, issues) = shaded("<svg></svg>", Brush::Colour(Rgba::BLACK));
        assert_eq!(found, Some(Shade::Colour(Rgba::BLACK)));
        assert!(issues.is_empty());
    }

    #[test]
    fn what_cannot_be_used_is_its_fallback_or_nothing_and_is_recorded() {
        let red = Rgba::from_rgba8(255, 0, 0, 255);
        for markup in [
            "<svg></svg>",
            "<svg><radialGradient id=x><stop/></radialGradient></svg>",
            "<svg><pattern id=x></pattern></svg>",
            "<svg><rect id=x /></svg>",
            "<div id=x></div>",
        ] {
            let (found, issues) = shaded(markup, server(Some("x"), Some(red)));
            assert_eq!(found, Some(Shade::Colour(red)), "{markup}");
            assert_eq!(issues.len(), 1, "{markup}");
            let (found, issues) = shaded(markup, server(Some("x"), None));
            assert_eq!(found, None, "{markup}");
            assert_eq!(issues.len(), 1, "{markup}");
        }
        let (found, issues) = shaded("<svg></svg>", server(None, Some(red)));
        assert_eq!(found, Some(Shade::Colour(red)));
        assert!(
            issues
                .iter()
                .all(|issue| issue.contains("another document"))
        );
    }

    #[test]
    fn the_first_element_with_an_id_is_the_one_named() {
        let document = parse_document("<p id=a></p><svg><g id=a></g></svg>");
        let servers = Servers::of(&document);
        let first = servers.named("a").and_then(|id| document.element(id));
        assert_eq!(first.map(|element| &*element.name.local), Some("p"));
        assert_eq!(servers.named("b"), None);
    }
}
