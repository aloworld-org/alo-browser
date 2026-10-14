/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Turning a laid-out document into a display list.
//!
//! What to draw, and in what order, worked out once from the box tree, the
//! layout and the styles. It is separate from [`crate::display`] on purpose: a
//! new CSS property changes this file, a new *kind* of drawing changes that
//! one, and a file with both reasons to change is a file two changes collide
//! in.
//!
//! # Paint order
//!
//! Shadows, then backgrounds and borders, then content; parents before
//! children; positioned boxes after everything in the flow, ordered by
//! `z-index` and, where two agree, by which came first. That is the small,
//! honest part of CSS's stacking model: enough for an interface, and short of
//! the full painting order, which `docs/features.md` reaches for with
//! transforms and opacity.

use crate::background::{self, Backdrop};
use crate::banded;
use crate::border::{self, DrawnSide, Line, Side};
use crate::canvas::Canvas;
use crate::control::{self, Mark};
use crate::corner::{Corners, between, ring, rounded_rectangle};
use crate::display::{DecorationLine, DisplayItem, DisplayList, TextShadow, lines_in};
use crate::drawing::{Drawing, DrawingItem};
use crate::face;
use crate::fill_rule::FillRule;
use crate::paint::Paint;
use crate::path::Path;
use crate::skip_ink::SkipInk;
use crate::snap;
use crate::toggle;
use alo_box::{BoxId, BoxKind, BoxTree};
use alo_layout::{LayoutTree, Rect};
use alo_style::StyleTree;
use alo_text::{FontDatabase, FontRequest, Slant, Weight};
use alo_value::{DrawnShadow, Rgba};

/// How one box's text is set, read once for all of its pieces.
struct Pen {
    font: alo_text::Font,
    oblique: f32,
    size: f32,
    letter_spacing: f32,
    color: Rgba,
    /// How far below the top of a piece its baseline is.
    ascender: f32,
}

/// The decoration lines on a box's text, and how they meet its letters.
struct Decorations {
    /// Each line, with the colour of the element that asked for it.
    lines: Vec<(DecorationLine, Rgba)>,
    /// Whether an underline and an overline step aside for the letters: the
    /// text's own `text-decoration-skip-ink` (`crate::skip_ink`).
    skips_ink: bool,
    /// The text's own `text-shadow`, which each line casts as well as the
    /// letters (queue item 584).
    shadows: Vec<TextShadow>,
}

impl Decorations {
    /// Whether `line` is drawn before the letters rather than after them:
    /// an underline and an overline are, so that a letter is on top of a
    /// line it crosses, and a line through is not (queue item 581).
    fn beneath(line: DecorationLine) -> bool {
        line != DecorationLine::LineThrough
    }
}

/// One piece of text as it is drawn, for the lines that step aside for its
/// letters.
struct Letters<'a> {
    text: &'a str,
    font: &'a alo_text::Font,
    size: f32,
    letter_spacing: f32,
    /// Where the pen starts, on the baseline.
    origin: (f32, f32),
    oblique: f32,
}

impl Letters<'_> {
    /// The letters as a text item of `id`'s, in `color`, casting `shadows`.
    fn item(&self, id: BoxId, color: Rgba, shadows: Vec<TextShadow>) -> DisplayItem {
        DisplayItem::Text {
            box_id: id,
            text: self.text.to_owned(),
            origin: self.origin,
            font: self.font.clone(),
            oblique: self.oblique,
            size: self.size,
            letter_spacing: self.letter_spacing,
            color,
            shadows,
        }
    }

    /// The letters' shadows, furthest back first, each as its own item:
    /// the same run [`crate::render`] outlines for a text item, moved by the
    /// shadow's offset. For text whose lines go between its shadows and its
    /// letters, which one text item cannot hold.
    fn shadow_items(&self, id: BoxId, shadows: &[TextShadow], scale: f32) -> Vec<DisplayItem> {
        let mut run = Path::new();
        for glyph in self.shapes(scale) {
            run.extend(&glyph);
        }
        shadows
            .iter()
            .rev()
            .map(|shadow| DisplayItem::Shadow {
                box_id: id,
                path: run.translated(crate::path::Point::new(shadow.offset.0, shadow.offset.1)),
                blur: shadow.blur,
                color: shadow.color,
            })
            .collect()
    }

    /// Each glyph's shape where it is drawn on a screen at `scale`, leaned as
    /// the text is: the same shapes `crate::render` fills.
    fn shapes(&self, scale: f32) -> Vec<crate::path::Path> {
        crate::render::outlined_glyphs_at(
            self.text,
            self.font,
            self.size,
            scale,
            self.letter_spacing,
            self.origin,
        )
        .iter()
        .map(|glyph| crate::synthesis::leaned(glyph, self.oblique, self.origin.1))
        .collect()
    }
}

/// What paint needs that is not in the layout: the fonts, and the colour of
/// the page behind everything.
#[derive(Debug, Clone, Copy)]
pub struct PaintContext<'a> {
    /// The fonts to draw text with.
    pub fonts: &'a FontDatabase,
    /// The pictures a box has, by box.
    ///
    /// Decoded elsewhere and handed in, because decoding is `alo_paint::encode`
    /// and deciding *which* picture belongs to which box needs a document,
    /// which this crate does not have.
    pub pictures: &'a std::collections::BTreeMap<BoxId, std::sync::Arc<Canvas>>,
    /// The drawings a box has, by box: what an `<svg>` holds.
    ///
    /// Handed in beside the pictures and for the same reason (ADR 0022 § 2):
    /// making one needs the document and the styles of elements that have no
    /// box, and this crate has neither.
    pub drawings: &'a std::collections::BTreeMap<BoxId, Drawing>,
}

/// Build the display list for a laid-out document.
pub fn build(
    boxes: &BoxTree,
    layout: &LayoutTree,
    styles: &StyleTree,
    context: PaintContext<'_>,
) -> DisplayList {
    let mut builder = Builder {
        boxes,
        layout,
        styles,
        context,
        next_order: 0,
        refused: Vec::new(),
    };
    let Some(root) = boxes.root() else {
        return DisplayList::default();
    };
    // The root always establishes a stacking context, so nothing is left over
    // to escape past it.
    let items = builder.paint(root).items;
    DisplayList::from_parts(items, builder.refused, styles.scale())
}

struct Builder<'a> {
    boxes: &'a BoxTree,
    layout: &'a LayoutTree,
    styles: &'a StyleTree,
    context: PaintContext<'a>,
    /// Which layer was reached first, so that two with the same `z-index`
    /// stack in the order they were written.
    next_order: usize,
    /// What the styles asked paint for and did not get, box by box.
    refused: Vec<alo_css::StyleIssue>,
}

/// One positioned box's subtree, waiting for a stacking context to place it.
struct Layer {
    z: i32,
    order: usize,
    items: Vec<DisplayItem>,
}

/// What a subtree draws: its own items, and the layers that have not yet found
/// the stacking context they belong to.
struct Painted {
    items: Vec<DisplayItem>,
    escaped: Vec<Layer>,
}

impl Builder<'_> {
    /// Everything a box and its descendants draw, in paint order.
    ///
    /// A positioned box does not simply go last: it goes last **in the
    /// stacking context it belongs to**, which may be several ancestors up.
    /// That is why this returns escaped layers rather than pushing them onto
    /// one list for the page — a positioned box inside a transformed one is
    /// painted inside that transform, not over the whole document.
    fn paint(&mut self, id: BoxId) -> Painted {
        // An atomic box an ellipsis hid is laid out and read, and not drawn
        // (`alo_layout::ellipsis`).
        if self.layout.is_hidden(id) {
            return Painted {
                items: Vec::new(),
                escaped: Vec::new(),
            };
        }
        let mut own = Vec::new();
        let refused = self.draw_box(id, &mut own);
        self.refused.extend(refused);

        let children: Vec<BoxId> = self.boxes.children(id).collect();
        // A box that clips holds its children inside its own shape — which,
        // if it has rounded corners, is a rounded shape. One question asked
        // twice: what shape is this box. A box with no children has nothing
        // to hold in, which is every `<svg>`: its drawing is clipped to its
        // viewport where the drawing is placed.
        let clip = if children.is_empty() {
            None
        } else {
            self.clip_of(id)
        };

        let mut flow = Vec::new();
        let mut escaped: Vec<Layer> = Vec::new();
        for child in children {
            let painted = self.paint(child);
            match self.layer_of(child) {
                Some(z) => {
                    let order = self.next_order;
                    self.next_order += 1;
                    escaped.push(Layer {
                        z,
                        order,
                        items: painted.items,
                    });
                }
                None => flow.extend(painted.items),
            }
            escaped.extend(painted.escaped);
        }
        // Over the lines it ends, as the last of them.
        self.draw_ellipses(id, &mut flow);

        let mut items = own;
        if let Some(path) = clip.clone() {
            items.push(DisplayItem::PushClip { box_id: id, path });
        }
        if self.establishes_stacking_context(id) {
            // A negative `z-index` is behind the content of the box that holds
            // it, which is the one part of stacking that surprises people.
            escaped.sort_by_key(|layer| (layer.z, layer.order));
            let behind = escaped.iter().position(|layer| layer.z >= 0);
            let split = behind.unwrap_or(escaped.len());
            let over = escaped.split_off(split);
            for layer in escaped {
                items.extend(layer.items);
            }
            items.extend(flow);
            for layer in over {
                items.extend(layer.items);
            }
            escaped = Vec::new();
        } else {
            items.extend(flow);
        }
        if clip.is_some() {
            items.push(DisplayItem::PopClip);
        }

        // The transform is inside the group: `opacity` fades what the box
        // actually looks like, transform and all.
        if let Some(matrix) = self.transform_of(id) {
            items.insert(0, DisplayItem::PushTransform { box_id: id, matrix });
            items.push(DisplayItem::PopTransform);
        }
        if let Some(opacity) = self.group_opacity_of(id) {
            items.insert(
                0,
                DisplayItem::PushGroup {
                    box_id: id,
                    opacity,
                },
            );
            items.push(DisplayItem::PopGroup);
        }
        Painted { items, escaped }
    }

    /// Whether a box gathers the layers below it, rather than passing them up.
    ///
    /// The root does; a positioned box with a `z-index` of its own does; and
    /// anything that is drawn as a unit does, because a group cannot be half
    /// painted over by something outside it — which is what `opacity` and
    /// `transform` both are.
    fn establishes_stacking_context(&self, id: BoxId) -> bool {
        if self.boxes.get(id).is_none_or(|node| node.parent.is_none()) {
            return true;
        }
        if self.group_opacity_of(id).is_some() || self.transform_of(id).is_some() {
            return true;
        }
        let Some(style) = self.style_of(id) else {
            return false;
        };
        self.is_positioned(id) && style.number("z-index").is_some()
    }

    /// Whether a box is taken out of the flow's paint order.
    fn is_positioned(&self, id: BoxId) -> bool {
        self.style_of(id)
            .and_then(|style| style.get("position"))
            .is_some_and(|position| !position.eq_ignore_ascii_case("static"))
    }

    /// Which layer a box paints in, or [`None`] for one that paints where it
    /// sits in the flow.
    ///
    /// `z-index` only means anything on a positioned box, which is a rule
    /// people are surprised by often enough that it is worth saying here.
    fn layer_of(&self, id: BoxId) -> Option<i32> {
        if !self.is_positioned(id) {
            return None;
        }
        let z = self
            .style_of(id)
            .and_then(|style| style.number("z-index"))
            .unwrap_or(0.0)
            .clamp(-1.0e6, 1.0e6);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "clamped to a range i32 represents exactly"
        )]
        let whole = z as i32;
        Some(whole)
    }

    /// The transform a box is drawn under, if it has one.
    ///
    /// [`None`] rather than the identity for a box with no transform, which is
    /// almost every box: the renderer leaves paths alone when nothing asked
    /// for them to move.
    fn transform_of(&self, id: BoxId) -> Option<alo_value::Matrix> {
        let style = self.style_of(id)?;
        let transform = alo_value::parse_transform(style.get("transform")?)?;
        if transform.is_empty() {
            return None;
        }
        let geometry = self.layout.get(id)?;
        let size = (
            geometry.border_box.size.width,
            geometry.border_box.size.height,
        );
        // `transform-origin` is a point inside the border box, and the middle
        // of it unless the author says otherwise — which is what makes
        // `rotate` turn a box about itself.
        let (across, down) = style
            .get("transform-origin")
            .and_then(alo_value::parse_transform_origin)
            .unwrap_or((
                alo_value::LengthPercentage::Percentage(50.0),
                alo_value::LengthPercentage::Percentage(50.0),
            ));
        let metrics = style.metrics();
        let origin = (
            geometry.border_box.left() + across.to_px(metrics, size.0),
            geometry.border_box.top() + down.to_px(metrics, size.1),
        );
        let matrix = transform.matrix(metrics, size, origin);
        (!matrix.is_identity()).then_some(matrix)
    }

    /// How faded a box and everything in it is, or [`None`] when it is not
    /// faded at all.
    ///
    /// A fully opaque box is not a group: drawing it to its own surface and
    /// compositing that back would cost a whole page of pixels to change
    /// nothing.
    fn group_opacity_of(&self, id: BoxId) -> Option<f32> {
        let style = self.style_of(id)?;
        let text = style.get("opacity")?;
        // `opacity: 50%` and `opacity: 0.5` are the same value written two
        // ways, and neither reading answers for the other.
        let opacity = match alo_value::parse_length_percentage(text) {
            Some(alo_value::LengthPercentage::Percentage(percent)) => percent / 100.0,
            _ => alo_value::parse_number(text)?,
        };
        let opacity = opacity.clamp(0.0, 1.0);
        (opacity < 1.0).then_some(opacity)
    }

    /// The shape a box clips its content to, if it clips at all.
    ///
    /// `visible` — the initial value — does not clip, which is why content can
    /// spill out of a box by default. Everything else does.
    ///
    /// The clip is on whole device pixels, as the box's own background is:
    /// its border box with each edge rounded as [`snap::to_device_pixels`]
    /// rounds, less its border, which is already whole device pixels. So a
    /// child reaching its parent's edge is cut where the parent's background
    /// ends, not a fraction short of it. Measured in Chrome 154 at one and at
    /// two, not read from its source: 2,016 of 2,016 clips, where the
    /// padding box as laid out fits only the 27 already on whole pixels
    /// (queue item 575; ADR 0057, *Measured since*).
    fn clip_of(&self, id: BoxId) -> Option<Path> {
        let style = self.style_of(id)?;
        let clips = |name: &str| {
            style
                .get(name)
                .is_some_and(|value| !value.eq_ignore_ascii_case("visible"))
        };
        if !clips("overflow") && !clips("overflow-x") && !clips("overflow-y") {
            return None;
        }
        let geometry = self.layout.get(id)?;
        // Content is clipped to the padding box: a border is drawn over the
        // content, not clipped by it.
        let border_box = snap::to_device_pixels(geometry.border_box, self.styles.scale());
        let padding_box = border_box.shrunk_by(geometry.border);
        Some(rounded_rectangle(
            padding_box,
            Corners::of(style, Self::extent_of(padding_box)),
        ))
    }

    /// Everything one box draws for itself, and what it could not.
    fn draw_box(&self, id: BoxId, out: &mut Vec<DisplayItem>) -> Vec<alo_css::StyleIssue> {
        let Some(geometry) = self.layout.get(id) else {
            // A text box of nothing but white space has no rectangle, and a
            // line that goes on past it still runs its decorations under it.
            if let Some(BoxKind::Text { text, .. }) = self.boxes.get(id).map(|node| &node.kind) {
                self.draw_text(id, text, out);
            }
            return Vec::new();
        };
        let Some(node) = self.boxes.get(id) else {
            return Vec::new();
        };
        // Read once, not once per piece: a box broken across three lines has
        // one background, and one refusal if it has any.
        let backdrop = self.style_of(id).map(background::read).unwrap_or_default();

        // **One area per piece.** A box that sits on a line and wrapped has a
        // rectangle per line it is on, and drawing its background from the
        // union of them would paint straight across the gap between the lines.
        // A block box has no pieces and is drawn once, at its border box.
        let pieces: Vec<Rect> = self
            .layout
            .fragments(id)
            .iter()
            .map(|fragment| fragment.rect)
            .collect();
        let areas: Vec<Rect> = if pieces.is_empty() {
            vec![geometry.border_box]
        } else {
            pieces
        };
        self.occupied(id, &node.kind, &areas, out);
        let last = areas.len().saturating_sub(1);
        for (index, area) in areas.iter().enumerate() {
            // A box broken across lines has its start edge only on its first
            // piece and its end edge only on its last, which is what CSS says
            // and what stops a wrapped `<em>` growing a border down the middle
            // of a paragraph.
            self.draw_one_area(id, *area, (index == 0, index == last), &backdrop, out);
        }

        // After the background and border, before the text: a picture is
        // content, and content sits on top of what the box painted for itself.
        self.control_state_of(id, out);
        self.picture_of(id, out);
        self.drawing_of(id, out);

        if let BoxKind::Text { text, .. } = &node.kind {
            self.draw_text(id, text, out);
        }
        backdrop.refused
    }

    /// Where a person's pointer finds a box: one [`DisplayItem::Area`] per
    /// piece, its border box with its rounded corners, in the box's place in
    /// paint order ([`crate::hit`]).
    ///
    /// A piece of an **inline box** — a `<span>`, a run of text — is moved
    /// onto whole pixels first, each edge rounded on its own as
    /// [`snap::to_whole_pixels`] rounds; any other box is found where layout
    /// put it. Measured in Chrome 154, not read from its source: a `<span>`
    /// whose padding starts at 26.953125 is first found by a point at 26.05,
    /// a block starting there by a point at 26.
    ///
    /// Whole CSS pixels at every scale, as measured at one: where a pointer
    /// lands on a screen at two is measured with queue item 562 (ADR 0057,
    /// *What is not decided here*), and until then this does not guess.
    fn occupied(&self, id: BoxId, kind: &BoxKind, pieces: &[Rect], out: &mut Vec<DisplayItem>) {
        let in_lines = kind.is_inline_box() && self.boxes.natural_size(id).is_none();
        for piece in pieces {
            let rect = if in_lines {
                snap::to_whole_pixels(*piece)
            } else {
                *piece
            };
            let corners = self.style_of(id).map_or(Corners::SQUARE, |style| {
                Corners::of(style, Self::extent_of(rect))
            });
            out.push(DisplayItem::Area {
                box_id: id,
                path: rounded_rectangle(rect, corners),
            });
        }
    }

    /// The shadows, background and border of one piece of a box.
    ///
    /// `ends` says whether this is the box's first piece and its last.
    ///
    /// All of it is drawn from the piece moved onto whole device pixels, each
    /// edge rounded on its own as [`snap::to_device_pixels`] rounds (queue
    /// item 565): a background, its image, a border on both its edges, a
    /// rounded corner, and every shadow, which is the snapped box grown and
    /// then moved by its offset. A transform is applied after, so a box is
    /// snapped where it would be without one. Measured in Chrome 154 at one
    /// and at two, not read from its source (ADR 0057, *Measured since*).
    fn draw_one_area(
        &self,
        id: BoxId,
        area: Rect,
        ends: (bool, bool),
        backdrop: &Backdrop,
        out: &mut Vec<DisplayItem>,
    ) {
        let (first, last) = ends;
        let area = snap::to_device_pixels(area, self.styles.scale());
        // CSS's own order for one box: the shadows it casts outwards, then
        // its background colour, then its background image over that, then
        // the shadows cast inwards, then the border, then what is inside it.
        let corners = self.style_of(id).map_or(Corners::SQUARE, |style| {
            Corners::of(style, Self::extent_of(area))
        });
        let shadows = self.shadows_of(id);
        for shadow in shadows.iter().rev().filter(|shadow| !shadow.inset) {
            out.push(DisplayItem::Shadow {
                box_id: id,
                path: cast(area, corners, *shadow),
                blur: shadow.blur,
                color: shadow.color,
            });
        }
        let border = self.border_of(id);
        let face = self.face_of(id);
        if let Some(layers) = &face {
            for layer in layers {
                out.push(DisplayItem::Fill {
                    rule: FillRule::NonZero,
                    box_id: id,
                    path: layer.path.clone(),
                    paint: Paint::Solid(layer.color),
                });
            }
        } else if let Some(style) = self.style_of(id) {
            let shape = rounded_rectangle(area, corners);
            if let Some(color) = backdrop.color
                && !color.is_invisible()
            {
                out.push(DisplayItem::Fill {
                    rule: FillRule::NonZero,
                    box_id: id,
                    path: shape.clone(),
                    paint: Paint::Solid(color),
                });
            }
            // Each layer is measured against the padding box, which is what
            // `background-origin` starts at, and drawn over the border box,
            // the last written first so that the first ends up on top.
            for gradient in &backdrop.gradients {
                out.push(DisplayItem::Fill {
                    rule: FillRule::NonZero,
                    box_id: id,
                    path: shape.clone(),
                    paint: Paint::Gradient {
                        gradient: gradient.clone(),
                        area: area.shrunk_by(border),
                        current: style.current_color(),
                    },
                });
            }
        }
        for shadow in shadows.iter().rev().filter(|shadow| shadow.inset) {
            // An inset shadow is the shadow of a hole: the shape *outside* the
            // box, blurred, and kept inside the box by a clip. That is the
            // same blur as an outset shadow, which is why there is one.
            let inside = area.shrunk_by(border);
            out.push(DisplayItem::PushClip {
                box_id: id,
                path: rounded_rectangle(inside, corners),
            });
            out.push(DisplayItem::Shadow {
                box_id: id,
                path: hole(inside, corners, *shadow),
                blur: shadow.blur,
                color: shadow.color,
            });
            out.push(DisplayItem::PopClip);
        }
        if face.is_none() {
            self.draw_borders(id, area, first, last, out);
        }
    }

    /// A button's, a text field's or a text area's own picture, in place of
    /// its background and border, while the page has written neither: drawn
    /// over its border box moved onto whole device pixels, as Chrome draws
    /// it, as a checkbox's is (queue items 505, 510 and 561). See
    /// [`crate::face`].
    fn face_of(&self, id: BoxId) -> Option<Vec<toggle::Layer>> {
        let kind = self.boxes.face(id)?;
        if !face::drawn(self.style_of(id)?) {
            return None;
        }
        let (node, geometry) = (self.boxes.get(id)?, self.layout.get(id)?);
        Some(face::artwork(
            kind,
            node.semantics.states.disabled,
            snap::to_device_pixels(geometry.border_box, self.styles.scale()),
        ))
    }

    /// How thick a box's border is on each side.
    ///
    /// From the layout, which worked it out from the style — including for an
    /// inline box, whose border used to be neither laid out nor drawn.
    fn border_of(&self, id: BoxId) -> alo_layout::Edges {
        self.layout
            .get(id)
            .map_or(alo_layout::Edges::ZERO, |geometry| geometry.border)
    }

    /// The border.
    ///
    /// A border of one width and one colour all the way round is drawn as a
    /// **ring** — the box's shape with the box's shape inside it, wound the
    /// other way — so that it follows the corners. Four rectangles would have
    /// square corners over a rounded background.
    ///
    /// A border whose sides differ is still four rectangles, clipped to the
    /// box's shape so that they do not stick out of the corners. The inner
    /// corner of such a border is squarer than CSS draws it; that shows only
    /// with a thick border and a large radius, and the alternative is four
    /// mitred trapezoids, which is queue item 19's kind of work.
    ///
    /// A border with any side that is not `solid` — `inset`, `outset`,
    /// `groove`, `ridge`, `dashed`, `dotted` or `double` — is drawn by
    /// [`border::draw_mitred`] instead, with each side mitred so that the
    /// corners are split where the sides differ.
    ///
    /// A border with a legend sitting in it is [`banded::draw`]'s.
    ///
    /// `none` and `hidden` draw nothing whatever their width, which is what
    /// CSS says and is why a width alone never shows a border.
    fn draw_borders(
        &self,
        id: BoxId,
        border_box: Rect,
        first: bool,
        last: bool,
        out: &mut Vec<DisplayItem>,
    ) {
        let Some(geometry) = self.layout.get(id) else {
            return;
        };
        let Some(style) = self.style_of(id) else {
            return;
        };
        if let Some(band) = geometry.band {
            self.draw_band(id, geometry.border_box, band, style, out);
            return;
        }
        let corners = Corners::of(style, Self::extent_of(geometry.border_box));

        // One width and one colour on every side: a ring. Only for a box drawn
        // in one piece — a piece in the middle of a broken box has no start
        // edge and no end edge, so there is no ring to draw.
        if first
            && last
            && let Some(color) = self.uniform_border(id, style)
        {
            out.push(DisplayItem::Fill {
                rule: FillRule::NonZero,
                box_id: id,
                path: ring(border_box, corners, geometry.border),
                paint: Paint::Solid(color),
            });
            return;
        }
        if let Some(sides) = Self::mitred_sides(style) {
            let widths = alo_layout::Edges {
                left: if first { geometry.border.left } else { 0.0 },
                right: if last { geometry.border.right } else { 0.0 },
                ..geometry.border
            };
            border::draw_mitred(id, border_box, corners, widths, &sides, out);
            return;
        }
        let sides = [
            ("top", geometry.border.top),
            ("right", geometry.border.right),
            ("bottom", geometry.border.bottom),
            ("left", geometry.border.left),
        ];
        // Which sides are actually drawn, worked out before anything is
        // pushed: a box with no border at all must not push a clip for it,
        // which is what the first version did — and a stray clip changes what
        // everything inside the box looks like.
        let drawn: Vec<(&str, f32, Rgba)> = sides
            .into_iter()
            .filter(|(side, _)| match *side {
                "left" => first,
                "right" => last,
                _ => true,
            })
            .filter(|(_, width)| *width > 0.0)
            .filter(|(side, _)| border_style(style, side).is_some_and(|kind| kind == "solid"))
            .filter_map(|(side, width)| {
                let color = border_color(style, side);
                (!color.is_invisible()).then_some((side, width, color))
            })
            .collect();
        if drawn.is_empty() {
            return;
        }

        let rounded = !corners.fitted_to(border_box.size).are_square();
        if rounded {
            out.push(DisplayItem::PushClip {
                box_id: id,
                path: rounded_rectangle(border_box, corners),
            });
        }
        for (side, width, color) in drawn {
            let rect = match side {
                "top" => Rect::new(
                    border_box.left(),
                    border_box.top(),
                    border_box.size.width,
                    width,
                ),
                "right" => Rect::new(
                    border_box.right() - width,
                    border_box.top(),
                    width,
                    border_box.size.height,
                ),
                "bottom" => Rect::new(
                    border_box.left(),
                    border_box.bottom() - width,
                    border_box.size.width,
                    width,
                ),
                _ => Rect::new(
                    border_box.left(),
                    border_box.top(),
                    width,
                    border_box.size.height,
                ),
            };
            out.push(DisplayItem::Fill {
                rule: FillRule::NonZero,
                box_id: id,
                path: rect_path(rect),
                paint: Paint::Solid(color),
            });
        }
        if rounded {
            out.push(DisplayItem::PopClip);
        }
    }

    /// The border of a box with a legend in its band, from the box as laid
    /// out: the band's line is snapped on its own, not moved down from the
    /// snapped box ([`banded::draw`]).
    fn draw_band(
        &self,
        id: BoxId,
        laid_out: Rect,
        band: alo_layout::Band,
        style: &alo_style::ComputedStyle,
        out: &mut Vec<DisplayItem>,
    ) {
        let sides = Self::drawn_sides(style);
        let scale = self.styles.scale();
        banded::draw(id, laid_out, scale, band, self.border_of(id), &sides, out);
    }

    /// Every side a border draws, when any of them is drawn as anything but
    /// `solid`.
    ///
    /// `None` for a border that is solid wherever it is drawn, which keeps
    /// its rectangles; one side of any other style is enough to draw all four
    /// mitred, since a rectangle beside a mitred side would take its corner.
    fn mitred_sides(style: &alo_style::ComputedStyle) -> Option<Vec<DrawnSide>> {
        let sides = Self::drawn_sides(style);
        sides
            .iter()
            .any(|drawn| drawn.line != Line::Solid)
            .then_some(sides)
    }

    /// Every side a border's style draws, with the line and colour it asks
    /// for — whatever its width, which is the layout's to say.
    fn drawn_sides(style: &alo_style::ComputedStyle) -> Vec<DrawnSide> {
        [
            (Side::Top, "top"),
            (Side::Right, "right"),
            (Side::Bottom, "bottom"),
            (Side::Left, "left"),
        ]
        .into_iter()
        .filter_map(|(side, name)| {
            let line = border_style(style, name).as_deref().and_then(Line::of)?;
            Some(DrawnSide {
                side,
                line,
                color: border_color(style, name),
            })
        })
        .collect()
    }

    /// The colour of a border that is the same on all four sides, if it is.
    fn uniform_border(&self, id: BoxId, style: &alo_style::ComputedStyle) -> Option<Rgba> {
        let geometry = self.layout.get(id)?;
        let widths = [
            geometry.border.top,
            geometry.border.right,
            geometry.border.bottom,
            geometry.border.left,
        ];
        let first = widths.first().copied()?;
        if first <= 0.0 || widths.iter().any(|width| (width - first).abs() > 0.001) {
            return None;
        }
        let mut color: Option<Rgba> = None;
        for side in ["top", "right", "bottom", "left"] {
            if border_style(style, side).as_deref() != Some("solid") {
                return None;
            }
            let side_color = border_color(style, side);
            match color {
                None => color = Some(side_color),
                Some(held) if held == side_color => {}
                Some(_) => return None,
            }
        }
        color.filter(|color| !color.is_invisible())
    }

    /// The pieces of a text box, one per line it is on.
    ///
    /// Fragments rather than the box's rectangle: a box that wrapped has one
    /// rectangle per line, and drawing from the union would put the second
    /// line's text on the first line's row.
    fn draw_text(&self, id: BoxId, text: &str, out: &mut Vec<DisplayItem>) {
        let Some(pen) = self.pen_for(id) else {
            return;
        };
        let decorations = Decorations {
            lines: self.decorations_of(id),
            skips_ink: self.skip_ink_of(id).steps_aside(),
            shadows: self.text_shadows_of(id),
        };
        self.draw_spaces(id, &decorations, &pen, out);
        let Pen {
            font,
            oblique,
            size,
            letter_spacing,
            color,
            ascender,
        } = pen;
        let scale = self.styles.scale();

        let fragments = self.layout.fragments(id);
        if fragments.is_empty() {
            // A text box that is not inside a line — a flex or grid item —
            // has no fragments, because fragments come from a line box. It is
            // still text, and it is still drawn, at its own rectangle.
            let Some(geometry) = self.layout.get(id) else {
                return;
            };
            let trimmed = text.trim_end();
            if !trimmed.is_empty() {
                // Measured as a `normal` line, whose top half of the line gap
                // is above the letters (ADR 0050 § 2).
                let room = alo_text::LineRoom::at(font.metrics_at(size, scale), scale);
                let baseline = geometry.border_box.top() + room.normal_above();
                let piece = crate::decoration::Piece {
                    top: baseline - room.ascent,
                    ascent: room.ascent,
                    left: geometry.border_box.left(),
                    width: geometry.border_box.right() - geometry.border_box.left(),
                    size,
                };
                let origin = (geometry.border_box.left(), baseline);
                let letters = Letters {
                    text: trimmed,
                    font: &font,
                    size,
                    letter_spacing,
                    origin,
                    oblique,
                };
                Self::between_lines(id, &decorations, piece, &letters, color, scale, out);
            }
            return;
        }
        for fragment in fragments {
            let Some(range) = fragment.text.clone() else {
                continue;
            };
            // Where an ellipsis ended the line, only what it left is drawn,
            // and only that is underlined.
            let cut = self.layout.cut(fragment);
            let range = match cut {
                Some(cut) => range.start..cut.end.max(range.start),
                None => range,
            };
            let Some(piece) = text.get(range) else {
                continue;
            };
            let trimmed = piece.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            // The pen sits on the baseline, which is the ascender below the
            // top of the piece.
            let origin = (fragment.rect.left(), fragment.rect.top() + ascender);
            // One line per **fragment**, which is what makes a decoration stop
            // at the end of the inline rather than running to the edge of the
            // line it sits on. A fragment is one piece of one inline on one
            // line, so drawing per fragment is the whole of that rule. It
            // runs under the space the piece ends in when the line goes on
            // past it, and not under one that ends the line (queue item 580).
            let piece = crate::decoration::Piece {
                top: fragment.rect.top(),
                ascent: ascender,
                left: fragment.rect.left(),
                width: cut.map_or(fragment.rect.size.width + fragment.space, |cut| cut.width),
                size,
            };
            let letters = Letters {
                text: trimmed,
                font: &font,
                size,
                letter_spacing,
                origin,
                oblique,
            };
            Self::between_lines(id, &decorations, piece, &letters, color, scale, out);
        }
    }

    /// A piece of text's letters, in `color`, with its decoration lines, in
    /// Chrome 154's order (queue item 584): the letters' shadows, then the
    /// shadows of the lines drawn beneath the letters, those lines, the
    /// letters, and the shadows of the rest and the rest
    /// ([`Decorations::beneath`]). With no line beneath them the letters'
    /// shadows stay with the letters, in one text item.
    fn between_lines(
        id: BoxId,
        decorations: &Decorations,
        piece: crate::decoration::Piece,
        letters: &Letters<'_>,
        color: Rgba,
        scale: f32,
        out: &mut Vec<DisplayItem>,
    ) {
        let beneath = Self::decorations_at(decorations, true, piece, letters, scale);
        let item = if beneath.is_empty() {
            letters.item(id, color, decorations.shadows.clone())
        } else {
            out.extend(letters.shadow_items(id, &decorations.shadows, scale));
            letters.item(id, color, Vec::new())
        };
        Self::draw_lines(id, &beneath, &decorations.shadows, out);
        out.push(item);
        let after = Self::decorations_at(decorations, false, piece, letters, scale);
        Self::draw_lines(id, &after, &decorations.shadows, out);
    }

    /// Decoration lines, each drawn piece in its colour, after the shadows
    /// the text casts them: every piece's shadow first, furthest back first,
    /// then the pieces, so that one line is on top of another's shadow as
    /// in Chrome 154. A line's shadow is the piece as drawn, on whole device
    /// pixels and stepped aside for the letters, moved by the shadow's
    /// offset and not put on whole pixels again.
    fn draw_lines(
        id: BoxId,
        pieces: &[(Rect, Rgba)],
        shadows: &[TextShadow],
        out: &mut Vec<DisplayItem>,
    ) {
        for shadow in shadows.iter().rev() {
            let by = alo_layout::Point::new(shadow.offset.0, shadow.offset.1);
            for (rect, _) in pieces {
                out.push(DisplayItem::Shadow {
                    box_id: id,
                    path: rounded_rectangle(rect.translated(by), Corners::default()),
                    blur: shadow.blur,
                    color: shadow.color,
                });
            }
        }
        for (rect, colour) in pieces {
            out.push(DisplayItem::Fill {
                rule: FillRule::NonZero,
                box_id: id,
                // A plain rectangle: a decoration has no corners of its own,
                // so the rounding machinery would only be a way to get it
                // wrong.
                path: rounded_rectangle(*rect, Corners::default()),
                paint: Paint::Solid(*colour),
            });
        }
    }

    /// White space with no letters that a line goes on past — the space
    /// between `</a>` and `<a>`, or the one a box's text begins with — drawn
    /// as its decorations and nothing else (queue item 582). It has no
    /// letters for a line to step aside for.
    fn draw_spaces(
        &self,
        id: BoxId,
        decorations: &Decorations,
        pen: &Pen,
        out: &mut Vec<DisplayItem>,
    ) {
        if decorations.lines.is_empty() {
            return;
        }
        let (size, ascender) = (pen.size, pen.ascender);
        let scale = self.styles.scale();
        for space in self.layout.spaces(id) {
            let piece = crate::decoration::Piece {
                top: space.rect.top(),
                ascent: ascender,
                left: space.rect.left(),
                width: space.rect.size.width + space.space,
                size,
            };
            let letters = Letters {
                text: "",
                font: &pen.font,
                size,
                letter_spacing: pen.letter_spacing,
                origin: (space.rect.left(), space.rect.top() + ascender),
                oblique: pen.oblique,
            };
            for beneath in [true, false] {
                let pieces = Self::decorations_at(decorations, beneath, piece, &letters, scale);
                Self::draw_lines(id, &pieces, &decorations.shadows, out);
            }
        }
    }

    /// How a box's text is set: the face, size, spacing and colour, and
    /// where the baseline is below the top of a piece. [`None`] with no style
    /// to ask or no font at all.
    fn pen_for(&self, id: BoxId) -> Option<Pen> {
        let style = self.nearest_styled_ancestor(id)?;
        let color = style.color("color").unwrap_or(style.current_color());
        let size = style.font_size();
        let request = self.font_request(id);
        let font = self.context.fonts.chain(&request).into_iter().next()?;
        // Where a line puts the baseline below the top of a piece of text:
        // the face's ascent in whole device pixels, as layout took it (ADR
        // 0050, ADR 0057 § 2).
        let scale = self.styles.scale();
        let ascender = alo_text::LineRoom::at(font.metrics_at(size, scale), scale).ascent;
        let oblique = crate::synthesis::oblique_for(request.slant, font.slant());
        let letter_spacing = style
            .get("letter-spacing")
            .filter(|value| !value.eq_ignore_ascii_case("normal"))
            .and_then(|_| style.px("letter-spacing", 0.0))
            .unwrap_or(0.0);
        Some(Pen {
            font,
            oblique,
            size,
            letter_spacing,
            color,
            ascender,
        })
    }

    /// The "…" a block's lines end in where `text-overflow` cut them: set in
    /// the block's own font and colour, whatever the text before it is set
    /// in, on the line's baseline (`alo_layout::ellipsis`, measured in Chrome
    /// 154). Its shadow is the block's too, as the text's is the text's.
    fn draw_ellipses(&self, id: BoxId, out: &mut Vec<DisplayItem>) {
        let ellipses = self.layout.ellipses(id);
        if ellipses.is_empty() {
            return;
        }
        let Some(pen) = self.pen_for(id) else {
            return;
        };
        let shadows = self.text_shadows_of(id);
        for ellipsis in ellipses {
            out.push(DisplayItem::Text {
                box_id: id,
                text: alo_layout::ELLIPSIS.to_owned(),
                origin: (ellipsis.rect.left(), ellipsis.rect.top() + pen.ascender),
                font: pen.font.clone(),
                oblique: pen.oblique,
                size: pen.size,
                letter_spacing: pen.letter_spacing,
                color: pen.color,
                shadows: shadows.clone(),
            });
        }
    }

    /// The picture a box holds, drawn into its content box.
    ///
    /// What a control draws to say what state it is in: a tick, a dash, a dot.
    ///
    /// It is drawn like content rather than like a background, because that is
    /// what it is — the control's own appearance, which no style sheet can
    /// express and which [`crate::control`] says why. A control with nothing
    /// to say about its state draws nothing at all, which is every box on a
    /// page that is not a checkbox or a radio.
    ///
    /// The area is the **padding box**: inside the author's border, which they
    /// can see and set, and which this does not paint over.
    ///
    /// A checkbox or a radio draws more than its state: its whole picture,
    /// which is [`crate::toggle`]'s.
    fn control_state_of(&self, id: BoxId, out: &mut Vec<DisplayItem>) {
        let Some(node) = self.boxes.get(id) else {
            return;
        };
        if let Some(kind) = self.boxes.toggle(id) {
            self.toggle_of(id, kind, out);
            return;
        }
        let Some(kind) = Mark::for_state(&node.semantics.role, node.semantics.states.checked)
        else {
            return;
        };
        let Some(geometry) = self.layout.get(id) else {
            return;
        };
        let style = self.style_of(id);
        let area = geometry.padding_box();
        let accent = control::accent(style, node.semantics.states.disabled);
        // The corners come from the border box and are fitted to the padding
        // box, which is what turns a radio's `border-radius: 50%` into a
        // circle inside its border rather than a rounded square.
        let corners = style.map_or(Corners::SQUARE, |style| {
            Corners::of(style, Self::extent_of(geometry.border_box))
        });
        out.push(DisplayItem::Fill {
            rule: FillRule::NonZero,
            box_id: id,
            path: rounded_rectangle(area, corners),
            paint: Paint::Solid(accent),
        });
        out.push(DisplayItem::Fill {
            rule: FillRule::NonZero,
            box_id: id,
            path: control::mark(kind, area),
            paint: Paint::Solid(control::mark_color(accent)),
        });
    }

    /// A checkbox's or a radio's own picture, drawn over its border box
    /// moved onto whole device pixels, as Chrome draws it (queue items 457
    /// and 561); the box itself stays where layout put it.
    fn toggle_of(&self, id: BoxId, kind: alo_box::Toggle, out: &mut Vec<DisplayItem>) {
        let (Some(node), Some(geometry)) = (self.boxes.get(id), self.layout.get(id)) else {
            return;
        };
        let state = toggle::State {
            checked: node.semantics.states.checked,
            disabled: node.semantics.states.disabled,
            accent: self
                .style_of(id)
                .and_then(|style| style.color("accent-color")),
        };
        let snapped = snap::to_device_pixels(geometry.border_box, self.styles.scale());
        for layer in toggle::artwork(kind, state, snapped) {
            out.push(DisplayItem::Fill {
                rule: FillRule::NonZero,
                box_id: id,
                path: layer.path,
                paint: Paint::Solid(layer.color),
            });
        }
    }

    /// Into the *content* box rather than the border box, because a picture
    /// sits inside its own padding and border like any other content — an
    /// `<img>` with a border draws the border around the picture rather than
    /// over it.
    ///
    /// On whole device pixels, each edge of the content box rounded on its
    /// own as [`snap::to_device_pixels`] rounds, as Chrome 154 draws it
    /// (queue item 587): the content box itself, not the snapped border box
    /// less its border and padding, so a padding of a fraction leaves the
    /// picture's edge on a whole pixel all the same.
    fn picture_of(&self, id: BoxId, out: &mut Vec<DisplayItem>) {
        let Some(picture) = self.context.pictures.get(&id) else {
            return;
        };
        let Some(geometry) = self.layout.get(id) else {
            return;
        };
        out.push(DisplayItem::Picture {
            box_id: id,
            rect: snap::to_device_pixels(geometry.content_box(), self.styles.scale()),
            picture: std::sync::Arc::clone(picture),
        });
    }

    /// What an `<svg>` box holds, or an `<img>` showing an SVG file, drawn
    /// inside its content box.
    ///
    /// Into the content box, as a picture is, because the drawing's viewport
    /// is the box's content (ADR 0022 § 1). It is clipped there unless the
    /// box's `overflow` is `visible`: the user-agent sheet makes every `<svg>`
    /// `overflow: hidden`, as SVG's own does, so a shape that runs past the
    /// viewport stops at its edge unless an author says otherwise.
    fn drawing_of(&self, id: BoxId, out: &mut Vec<DisplayItem>) {
        let Some(drawing) = self.context.drawings.get(&id) else {
            return;
        };
        if !drawing.draws_anything() {
            return;
        }
        let Some(geometry) = self.layout.get(id) else {
            return;
        };
        let content = geometry.content_box();
        let at = crate::path::Point::new(content.left(), content.top());
        // A picture's drawing is clipped whatever the box says: an SVG file
        // shown by an `<img>` is the rectangle it was given (ADR 0027 § 5).
        let clips = drawing.is_confined()
            || self
                .style_of(id)
                .and_then(|style| style.get("overflow"))
                .is_some_and(|overflow| !overflow.eq_ignore_ascii_case("visible"));
        if clips {
            out.push(DisplayItem::PushClip {
                box_id: id,
                path: rect_path(content),
            });
        }
        for item in drawing.items() {
            out.push(match item {
                DrawingItem::Fill { path, color, rule } => DisplayItem::Fill {
                    box_id: id,
                    path: path.translated(at),
                    paint: Paint::Solid(*color),
                    rule: *rule,
                },
                DrawingItem::PushGroup { opacity } => DisplayItem::PushGroup {
                    box_id: id,
                    opacity: *opacity,
                },
                DrawingItem::PopGroup => DisplayItem::PopGroup,
            });
        }
        if clips {
            out.push(DisplayItem::PopClip);
        }
    }

    /// Which decoration lines cover this text, and in what colour.
    ///
    /// # Why this walks up rather than reading the text box's own style
    ///
    /// `text-decoration` does **not inherit**. It *propagates*: an underlined
    /// `<a>` underlines everything inside it, and a descendant cannot turn that
    /// off — `text-decoration: none` on a child of an underlined element
    /// removes nothing, in every browser, and that is the specified behaviour
    /// rather than a quirk.
    ///
    /// So making it an inherited property would be close and wrong in a way
    /// somebody would eventually hit. Walking the ancestors is what the
    /// propagation actually is.
    ///
    /// The colour comes from the element that **declared** the decoration
    /// rather than from the text, which is why a black `<span>` inside a blue
    /// link is still underlined in blue.
    fn decorations_of(&self, id: BoxId) -> Vec<(DecorationLine, Rgba)> {
        let mut found = Vec::new();
        let mut walking = Some(id);
        while let Some(at) = walking {
            if let Some(style) = self.style_of(at) {
                let written = style
                    .get("text-decoration-line")
                    .or_else(|| style.get("text-decoration"));
                if let Some(written) = written {
                    let colour = style
                        .color("text-decoration-color")
                        .or_else(|| style.color("color"))
                        .unwrap_or(style.current_color());
                    for line in lines_in(written) {
                        if !found.iter().any(|(already, _)| *already == line) {
                            found.push((line, colour));
                        }
                    }
                }
            }
            walking = self.boxes.get(at).and_then(|node| node.parent);
        }
        found
    }

    /// The text's own `text-decoration-skip-ink`, which decides for every
    /// line under or over it, whichever element asked for the line (queue
    /// item 581).
    ///
    /// It inherits, so the nearest element's computed style holds it, and
    /// one that holds nothing has the initial `auto`. A value that is none of
    /// the three was never a value: the cascade keeps what was written, so
    /// such a value is passed over here for the one its parent has, as a
    /// declaration CSS drops would have left it.
    fn skip_ink_of(&self, id: BoxId) -> SkipInk {
        let mut walking = Some(id);
        while let Some(at) = walking {
            if let Some(style) = self.style_of(at) {
                match style.get("text-decoration-skip-ink") {
                    None => return SkipInk::Auto,
                    Some(written) => {
                        if let Some(value) = SkipInk::parse(written) {
                            return value;
                        }
                    }
                }
            }
            walking = self.boxes.get(at).and_then(|node| node.parent);
        }
        SkipInk::Auto
    }

    /// A rectangle's width and height, which is what a percentage radius is a
    /// percentage of.
    fn extent_of(rect: alo_layout::Rect) -> (f32, f32) {
        (rect.right() - rect.left(), rect.bottom() - rect.top())
    }

    /// The shadows a box casts, front to back as they were written.
    ///
    /// A shadow's lengths are resolved against the box's own font, because
    /// `box-shadow: 0 0.25em 0.5em` is a shadow that grows with the text it
    /// sits under — and an unreadable value is refused whole rather than half
    /// drawn, which is what CSS does with a list it cannot parse.
    fn shadows_of(&self, id: BoxId) -> Vec<DrawnShadow> {
        let Some(style) = self.style_of(id) else {
            return Vec::new();
        };
        let Some(text) = style.get("box-shadow") else {
            return Vec::new();
        };
        alo_value::parse_box_shadows(text)
            .unwrap_or_default()
            .iter()
            .map(|shadow| shadow.drawn(style.metrics(), style.current_color()))
            .filter(|shadow| !shadow.is_invisible())
            .collect()
    }

    /// The decoration lines under, over and through a piece of text that are
    /// drawn `beneath` its letters, or the ones drawn after them
    /// ([`Decorations::beneath`]), as the pieces drawn and their colours,
    /// each where `crate::decoration` puts it: from the top of the piece, in
    /// whole device pixels at `scale`. An underline and an overline step
    /// aside where a letter's ink crosses them (`crate::skip_ink`), unless
    /// the text's `text-decoration-skip-ink` is `none`, so they are the
    /// pieces left; a line through the text is whole.
    fn decorations_at(
        decorations: &Decorations,
        beneath: bool,
        piece: crate::decoration::Piece,
        letters: &Letters<'_>,
        scale: f32,
    ) -> Vec<(Rect, Rgba)> {
        let drawn_here = decorations
            .lines
            .iter()
            .filter(|(line, _)| Decorations::beneath(*line) == beneath);
        // Shaped and outlined only when a line steps aside for them: most
        // text is not decorated, and a line through does not look.
        let skips =
            |line: &DecorationLine| decorations.skips_ink && *line != DecorationLine::LineThrough;
        let glyphs = if drawn_here.clone().any(|(line, _)| skips(line)) {
            letters.shapes(scale)
        } else {
            Vec::new()
        };
        let thickness = crate::decoration::thickness(piece.size, scale);
        let mut pieces = Vec::new();
        for (line, colour) in drawn_here {
            let whole = crate::decoration::line_at(*line, piece, scale);
            let drawn = if skips(line) {
                crate::skip_ink::pieces(whole, thickness, &glyphs, scale)
            } else {
                vec![whole]
            };
            pieces.extend(drawn.into_iter().map(|rect| (rect, *colour)));
        }
        pieces
    }

    /// The shadows a run of text casts.
    ///
    /// `text-shadow` inherits, so this asks the same element the text takes
    /// its colour and its font from rather than the text box, which has no
    /// style of its own.
    fn text_shadows_of(&self, id: BoxId) -> Vec<TextShadow> {
        let Some(style) = self.nearest_styled_ancestor(id) else {
            return Vec::new();
        };
        let Some(text) = style.get("text-shadow") else {
            return Vec::new();
        };
        alo_value::parse_text_shadows(text)
            .unwrap_or_default()
            .iter()
            .map(|shadow| shadow.drawn(style.metrics(), style.current_color()))
            .filter(|shadow| !shadow.is_invisible())
            .map(|shadow| TextShadow {
                offset: shadow.offset,
                blur: shadow.blur,
                color: shadow.color,
            })
            .collect()
    }

    /// What font a box's text is set in.
    fn font_request(&self, id: BoxId) -> FontRequest {
        let Some(style) = self.nearest_styled_ancestor(id) else {
            return FontRequest::default();
        };
        FontRequest {
            families: style
                .get("font-family")
                .map(FontRequest::parse_families)
                .unwrap_or_default(),
            // The weight layout measured this run in, read in the one place
            // both ask (queue item 402).
            weight: Weight::new(style.font_weight()),
            slant: match style.get("font-style") {
                Some(value) if value.eq_ignore_ascii_case("italic") => Slant::Italic,
                Some(value) if value.eq_ignore_ascii_case("oblique") => Slant::Italic,
                _ => Slant::Normal,
            },
        }
    }

    /// The style a box was given for itself.
    ///
    /// A run of text has none: it is not an element, and has no background,
    /// border, shadow, opacity or transform of its own. Most text comes from a
    /// text node, which has no style anyway; the text a field shows — its
    /// value, a date field's format — comes from the `<input>` itself, and
    /// answering with the input's style here drew the field's background and
    /// border a second time, as a ring around its text (found by queue item
    /// 385). Its colour and font are read through
    /// [`Self::nearest_styled_ancestor`], as any text's are.
    fn style_of(&self, id: BoxId) -> Option<&alo_style::ComputedStyle> {
        self.boxes
            .get(id)
            .filter(|node| node.text().is_none())
            .and_then(|node| node.kind.node())
            .and_then(|source| self.styles.get(source))
    }

    /// The style a box's text is set with, which the box tree answers.
    fn nearest_styled_ancestor(&self, id: BoxId) -> Option<&alo_style::ComputedStyle> {
        self.boxes.nearest_style(self.styles, id)
    }
}

/// What kind of line one border draws.
///
/// The longhand beats the per-side shorthand, which beats `border` — CSS's own
/// order, and the one a sheet relies on when it writes `border: 1px solid` and
/// then overrides one side.
fn border_style(style: &alo_style::ComputedStyle, side: &str) -> Option<String> {
    if let Some(text) = style.get(&format!("border-{side}-style")) {
        return Some(text.trim().to_ascii_lowercase());
    }
    for property in [format!("border-{side}"), "border".to_owned()] {
        if let Some(text) = style.get(&property)
            && let Some(kind) = alo_value::parse_border(text).style
        {
            return Some(kind);
        }
    }
    None
}

/// What colour one border is.
///
/// A border with no colour of its own is the colour of the text beside it,
/// which is what `currentColor` means and what CSS makes the initial value.
fn border_color(style: &alo_style::ComputedStyle, side: &str) -> Rgba {
    if let Some(color) = style.color(&format!("border-{side}-color")) {
        return color;
    }
    for property in [format!("border-{side}"), "border".to_owned()] {
        if let Some(text) = style.get(&property)
            && let Some(color) = alo_value::parse_border(text).color
        {
            return color.resolve(style.current_color());
        }
    }
    style.current_color()
}

fn rect_path(rect: Rect) -> Path {
    Path::rectangle(rect.left(), rect.top(), rect.size.width, rect.size.height)
}

/// The shape an outset shadow blurs: the box, grown by the spread and moved by
/// the offset.
///
/// The corners grow with it, so a shadow under a rounded card is rounded
/// rather than a rounded shape inside a square blur — and a square box's
/// stays square ([`Corners::spread`]).
fn cast(border_box: Rect, corners: Corners, shadow: DrawnShadow) -> Path {
    rounded_rectangle(
        grown_rect(border_box, shadow.spread)
            .translated(alo_layout::Point::new(shadow.offset.0, shadow.offset.1)),
        corners.fitted_to(border_box.size).spread(shadow.spread),
    )
}

/// The shape an inset shadow blurs: everything outside the box, once the box
/// has been shrunk by the spread and moved by the offset.
///
/// Bounded by a rectangle large enough that the blur reaches every pixel of
/// the box it will be clipped to — otherwise the shadow would stop at a
/// straight edge somewhere inside it.
fn hole(padding_box: Rect, corners: Corners, shadow: DrawnShadow) -> Path {
    let reach = crate::blur::reach(shadow.blur)
        + shadow.spread.abs()
        + shadow.offset.0.abs()
        + shadow.offset.1.abs()
        + 2.0;
    let outer = grown_rect(padding_box, reach);
    let inner = rounded_rectangle(
        grown_rect(padding_box, -shadow.spread)
            .translated(alo_layout::Point::new(shadow.offset.0, shadow.offset.1)),
        corners.fitted_to(padding_box.size).spread(-shadow.spread),
    );
    between(
        &Path::rectangle(
            outer.left(),
            outer.top(),
            outer.size.width,
            outer.size.height,
        ),
        &inner,
    )
}

/// A rectangle grown by the same amount on every side; a negative amount
/// shrinks it, and it never turns inside out.
fn grown_rect(rect: Rect, by: f32) -> Rect {
    Rect::new(
        rect.left() - by,
        rect.top() - by,
        (rect.size.width + by * 2.0).max(0.0),
        (rect.size.height + by * 2.0).max(0.0),
    )
}
