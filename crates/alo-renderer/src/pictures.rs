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
//! A picture that could not be read is **recorded and skipped**, and its
//! `<img>` has no picture: its `alt` drawn as text, or a box of nothing
//! (ADR 0046).
//!
//! # Before the boxes
//!
//! Whether an `<img>` shows a picture decides what box it is, so it is
//! decided **once, here, before the box tree is built**, and handed to it as
//! an [`alo_box::Shown`] (ADR 0046 § 1). Layout, paint and the agent then read
//! the one answer the tree holds. Paint is handed the pictures by box
//! afterwards, through [`Pictures::by_box`].

use crate::resource::Resource;
use crate::svg_picture::SvgPicture;
use alo_box::{BoxId, BoxKind, BoxTree, NaturalSize, Shown};
use alo_dom::{Document, NodeId};
use alo_layout::LayoutTree;
use alo_paint::{Canvas, Drawing};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// The raster pictures a drawing decoded, by `src`, each with the bytes it
/// was decoded from: handed to the next drawing of the same renderer, which
/// takes a picture of the same `src` and the very same bytes as it was
/// decoded rather than decoding it again (queue item 886). The same bytes
/// decode to the same pixels, so nothing drawn differs; and the picture
/// being the very same picture lets a list painted over the last match it
/// ([`alo_paint::damage`]), so a first screen holding a card's frame is not
/// painted whole for every change beside it.
///
/// A picture a drawing did not show is **laid aside** in it too, up to
/// [`ASIDE_BYTES`] of pixels (queue item 894): a card's frame drawn under
/// *Settings*, then beneath the first screen while it is closed, is the very
/// same picture when *Settings* opens again over the same frame — so it is
/// neither decoded again nor anything but alike to the list its veil's blur
/// was kept with ([`alo_paint::Kept`]).
pub type Decoded = BTreeMap<String, (Vec<u8>, Arc<Canvas>)>;

/// The most pixels' bytes of pictures a drawing did not show kept for the
/// next: a card's frame at two, 1756 × 1040, is about 29 MB.
pub const ASIDE_BYTES: usize = 32 << 20;

/// Every picture a page's `<img>` elements show, read and sized.
#[derive(Debug, Default)]
pub struct Pictures {
    /// Which `<img>` elements show a picture, and at what natural size: what
    /// the box tree is built knowing.
    pub shown: Shown,
    /// Decoded raster pictures, by `<img>` element.
    rasters: BTreeMap<NodeId, Arc<Canvas>>,
    /// SVG files, by `<img>` element, each with the `src` it was named by.
    svgs: BTreeMap<NodeId, (String, SvgPicture)>,
    /// The `<img>` elements whose picture is drawn beneath the page (queue
    /// item 869, ADR 0100).
    beneath: BTreeSet<NodeId>,
    /// What could not be shown, one line each.
    pub issues: Vec<String>,
    /// The raster pictures it decoded or took, for the next drawing.
    pub decoded: Decoded,
}

/// The pictures of one drawing, by the box each is drawn in.
#[derive(Debug, Default)]
pub struct Boxed<'p> {
    /// Decoded raster pictures, by box: what paint is handed.
    pub rasters: BTreeMap<BoxId, Arc<Canvas>>,
    /// SVG files, by box, each with the `src` it was named by, to be drawn
    /// once layout has said how big the box is.
    svgs: BTreeMap<BoxId, &'p (String, SvgPicture)>,
    /// The boxes whose picture is drawn beneath the page: what paint leaves
    /// clear (queue item 869, ADR 0100).
    pub beneath: BTreeSet<BoxId>,
}

impl Pictures {
    /// Read every picture the page's `<img>` elements name — taking from
    /// `before`, what the last drawing decoded, a picture of the same `src`
    /// and the very same bytes.
    ///
    /// Every `<img>` in the document, in document order, including one that
    /// ends up with no box: a hidden picture is still the page's, and what
    /// could not be read about it is still said.
    pub fn read(document: &Document, resources: &[Resource], before: &Decoded) -> Self {
        let mut pictures = Self::default();
        // One decoded picture per file, shared by every `<img>` that names
        // it, as Chrome's decode cache holds it: what a picture is drawn
        // through can depend on how the same file was drawn before
        // (`alo_paint::picture_filter`).
        let mut decoded: BTreeMap<String, Arc<Canvas>> = BTreeMap::new();
        for id in document.descendants(document.root()) {
            let Some(element) = document.element(id) else {
                continue;
            };
            if !element.name.is_html("img") {
                continue;
            }
            let Some(src) = element
                .attr("src")
                .map(|src| src.trim().to_owned())
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
            if let Some((width, height)) = resource.beneath {
                pictures
                    .shown
                    .show(id, NaturalSize::sized(f32::from(width), f32::from(height)));
                pictures.beneath.insert(id);
                continue;
            }
            if resource.is_svg() {
                match SvgPicture::read(&resource.bytes) {
                    Ok(picture) => {
                        pictures.shown.show(id, picture.natural_size());
                        pictures.svgs.insert(id, (src, picture));
                    }
                    Err(why) => pictures.issues.push(format!(
                        "{src:?} is not an SVG picture this engine reads: {why}"
                    )),
                }
                continue;
            }
            let kept = before
                .get(&src)
                .filter(|(bytes, _)| *bytes == resource.bytes)
                .map(|(_, canvas)| Arc::clone(canvas));
            let read = match decoded.get(&src).map(Arc::clone).or(kept) {
                Some(canvas) => Ok(canvas),
                None => alo_paint::picture::read(&resource.bytes).map(Arc::new),
            };
            match read {
                Ok(canvas) => {
                    let (width, height) = (canvas.width(), canvas.height());
                    pictures.shown.show(
                        id,
                        NaturalSize::sized(
                            f32::from(u16::try_from(width).unwrap_or(u16::MAX)),
                            f32::from(u16::try_from(height).unwrap_or(u16::MAX)),
                        ),
                    );
                    pictures
                        .decoded
                        .insert(src.clone(), (resource.bytes.clone(), Arc::clone(&canvas)));
                    decoded.insert(src, Arc::clone(&canvas));
                    pictures.rasters.insert(id, canvas);
                }
                Err(why) => pictures
                    .issues
                    .push(format!("{src:?} is not a picture this engine reads: {why}")),
            }
        }
        // What the drawing before had and this one did not show, laid aside
        // while there is room, in the order of their names.
        let mut aside = 0_usize;
        for (src, (bytes, canvas)) in before {
            if pictures.decoded.contains_key(src) {
                continue;
            }
            let held = core::mem::size_of_val(canvas.pixels());
            if aside.saturating_add(held) <= ASIDE_BYTES {
                aside += held;
                pictures
                    .decoded
                    .insert(src.clone(), (bytes.clone(), Arc::clone(canvas)));
            }
        }
        pictures
    }

    /// The pictures by the box each `<img>` made, once the tree is built.
    ///
    /// An `<img>` that made no box — `display: none` — draws nothing.
    pub fn by_box(&self, boxes: &BoxTree) -> Boxed<'_> {
        let mut by_box = Boxed::default();
        for id in boxes.ids() {
            let Some(BoxKind::Element { node, .. }) = boxes.get(id).map(|node| &node.kind) else {
                continue;
            };
            if let Some(canvas) = self.rasters.get(node) {
                by_box.rasters.insert(id, Arc::clone(canvas));
            }
            if let Some(svg) = self.svgs.get(node) {
                by_box.svgs.insert(id, svg);
            }
            if self.beneath.contains(node) {
                by_box.beneath.insert(id);
            }
        }
        by_box
    }
}

impl Boxed<'_> {
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
