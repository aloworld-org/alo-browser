/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Every picture a page asks for: read before layout, which needs their
//! sizes, and handed to paint by box.
//!
//! # Why this is here rather than in `alo-box` or `alo-paint`
//!
//! It needs three things that live in three places: the document, to find an
//! `<img>` and its `src`; the fetched [`Resource`]s, which the caller has; and
//! the readers, `alo_paint::picture` for raster formats and [`SvgPicture`] for
//! SVG. This is the only place that has all three, and putting it in any of
//! them would mean that crate learning about the other two.
//!
//! # Which reader
//!
//! A resource whose type says `image/svg+xml` is read as SVG and **never**
//! handed to a raster decoder. Every other resource is a raster picture,
//! decided by what its bytes begin with, and is never read as SVG whatever
//! its `src` ends in (ADR 0027 § 1). So the same SVG bytes under any other
//! type are refused by the raster decoders, which is the point.
//!
//! A picture that could not be read is **recorded and skipped**. The box keeps
//! whatever size its style asked for, which is what a browser shows for a
//! broken image: an empty box of the right shape rather than a collapsed page.

use crate::resource::Resource;
use crate::svg_picture::SvgPicture;
use alo_box::{BoxId, BoxKind, BoxTree, NaturalSize};
use alo_dom::Document;
use alo_layout::LayoutTree;
use alo_paint::{Canvas, Drawing};
use std::collections::BTreeMap;
use std::sync::Arc;

/// What every `<img>` box shows, read and sized.
#[derive(Debug, Default)]
pub struct Pictures {
    /// Decoded raster pictures, by box.
    pub rasters: BTreeMap<BoxId, Arc<Canvas>>,
    /// SVG files, by box, each with the `src` it was named by, to be drawn
    /// once layout has said how big the box is.
    pub svgs: BTreeMap<BoxId, (String, SvgPicture)>,
    /// What could not be shown, one line each.
    pub issues: Vec<String>,
}

impl Pictures {
    /// Read every picture the page's `<img>` boxes name, and tell the boxes
    /// how big each one is.
    pub fn read(document: &Document, boxes: &mut BoxTree, resources: &[Resource]) -> Self {
        let mut pictures = Self::default();
        // Every box rather than a walk: this is looking for a kind of box
        // rather than following the tree's shape.
        let ids: Vec<BoxId> = boxes.ids().collect();
        for id in ids {
            let Some(BoxKind::Element { node, .. }) = boxes.get(id).map(|node| &node.kind) else {
                continue;
            };
            let Some(element) = document.element(*node) else {
                continue;
            };
            if !element.name.local.eq_ignore_ascii_case("img") {
                continue;
            }
            let Some(src) = element
                .attrs
                .iter()
                .find(|attribute| attribute.name.local.eq_ignore_ascii_case("src"))
                .map(|attribute| attribute.value.trim().to_owned())
                .filter(|src| !src.is_empty())
            else {
                pictures.issues.push("an <img> with no src".to_owned());
                continue;
            };
            let Some(resource) = resources.iter().find(|resource| resource.src == src) else {
                pictures
                    .issues
                    .push(format!("no picture was loaded for {src:?}"));
                continue;
            };
            if resource.is_svg() {
                match SvgPicture::read(&resource.bytes) {
                    Ok(picture) => {
                        boxes.set_natural_size(id, picture.natural_size());
                        pictures.svgs.insert(id, (src, picture));
                    }
                    Err(why) => pictures.issues.push(format!(
                        "{src:?} is not an SVG picture this engine reads: {why}"
                    )),
                }
                continue;
            }
            match alo_paint::picture::read(&resource.bytes) {
                Ok(canvas) => {
                    let (width, height) = (canvas.width(), canvas.height());
                    boxes.set_natural_size(
                        id,
                        NaturalSize::sized(
                            f32::from(u16::try_from(width).unwrap_or(u16::MAX)),
                            f32::from(u16::try_from(height).unwrap_or(u16::MAX)),
                        ),
                    );
                    pictures.rasters.insert(id, Arc::new(canvas));
                }
                Err(why) => pictures
                    .issues
                    .push(format!("{src:?} is not a picture this engine reads: {why}")),
            }
        }
        pictures
    }

    /// Draw every SVG picture at the content box layout gave its `<img>`.
    ///
    /// After layout, because the picture is drawn as vectors at the box's
    /// size and never at its own (ADR 0027 § 5). Each line of what a file
    /// left out or refused is said with the `src` that named it, because the
    /// file is not the page and its problems are not the page's markup.
    pub fn draw_svgs(&self, layout: &LayoutTree) -> (BTreeMap<BoxId, Drawing>, Vec<String>) {
        let mut drawings = BTreeMap::new();
        let mut issues = Vec::new();
        for (id, (src, picture)) in &self.svgs {
            let Some(geometry) = layout.get(*id) else {
                continue;
            };
            let content = geometry.content_box();
            let drawn = picture.draw((content.size.width, content.size.height));
            issues.extend(
                drawn
                    .issues
                    .into_iter()
                    .map(|issue| format!("{src:?}: {issue}")),
            );
            if drawn.drawing.draws_anything() {
                drawings.insert(*id, drawn.drawing);
            }
        }
        (drawings, issues)
    }
}
