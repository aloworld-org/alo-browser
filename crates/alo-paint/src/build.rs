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

use crate::canvas::Canvas;
use crate::control::{self, Mark};
use crate::corner::{Corners, between, ring, rounded_rectangle};
use crate::display::{DecorationLine, DisplayItem, DisplayList, TextShadow, lines_in};
use crate::paint::Paint;
use crate::path::Path;
use alo_box::{BoxId, BoxKind, BoxTree};
use alo_layout::{LayoutTree, Rect};
use alo_style::StyleTree;
use alo_text::{FontDatabase, FontRequest, Slant, Weight};
use alo_value::{DrawnShadow, Gradient, Rgba};

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
    };
    let Some(root) = boxes.root() else {
        return DisplayList::default();
    };
    // The root always establishes a stacking context, so nothing is left over
    // to escape past it.
    DisplayList::from_items(builder.paint(root).items)
}

struct Builder<'a> {
    boxes: &'a BoxTree,
    layout: &'a LayoutTree,
    styles: &'a StyleTree,
    context: PaintContext<'a>,
    /// Which layer was reached first, so that two with the same `z-index`
    /// stack in the order they were written.
    next_order: usize,
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
        let mut own = Vec::new();
        self.draw_box(id, &mut own);

        // A box that clips holds its children inside its own shape — which,
        // if it has rounded corners, is a rounded shape. One question asked
        // twice: what shape is this box.
        let clip = self.clip_of(id);

        let mut flow = Vec::new();
        let mut escaped: Vec<Layer> = Vec::new();
        let children: Vec<BoxId> = self.boxes.children(id).collect();
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
        let padding_box = geometry.padding_box();
        Some(rounded_rectangle(
            padding_box,
            Corners::of(style, Self::extent_of(padding_box)),
        ))
    }

    fn draw_box(&self, id: BoxId, out: &mut Vec<DisplayItem>) {
        let Some(geometry) = self.layout.get(id) else {
            return;
        };
        let Some(node) = self.boxes.get(id) else {
            return;
        };

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
        let last = areas.len().saturating_sub(1);
        for (index, area) in areas.iter().enumerate() {
            // A box broken across lines has its start edge only on its first
            // piece and its end edge only on its last, which is what CSS says
            // and what stops a wrapped `<em>` growing a border down the middle
            // of a paragraph.
            self.draw_one_area(id, *area, index == 0, index == last, out);
        }

        // After the background and border, before the text: a picture is
        // content, and content sits on top of what the box painted for itself.
        self.control_state_of(id, out);
        self.picture_of(id, out);

        if let BoxKind::Text { text, .. } = &node.kind {
            self.draw_text(id, text, out);
        }
    }

    /// The shadows, background and border of one piece of a box.
    fn draw_one_area(
        &self,
        id: BoxId,
        area: Rect,
        first: bool,
        last: bool,
        out: &mut Vec<DisplayItem>,
    ) {
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
        if let Some(style) = self.style_of(id) {
            let shape = rounded_rectangle(area, corners);
            if let Some(color) = background_color(style)
                && !color.is_invisible()
            {
                out.push(DisplayItem::Fill {
                    box_id: id,
                    path: shape.clone(),
                    paint: Paint::Solid(color),
                });
            }
            // A gradient is measured against the padding box, which is what
            // `background-origin` starts at, and drawn over the border box.
            if let Some(gradient) = background_gradient(style) {
                out.push(DisplayItem::Fill {
                    box_id: id,
                    path: shape,
                    paint: Paint::Gradient {
                        gradient,
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
        self.draw_borders(id, area, first, last, out);
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
    /// Only `solid` is drawn either way. `none` and `hidden` draw nothing
    /// whatever their width, which is what CSS says and is why a width alone
    /// never shows a border; every other style is not implemented, and drawing
    /// a dashed border as a solid one would be a wrong pixel that looks nearly
    /// right.
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
        let corners = Corners::of(style, Self::extent_of(geometry.border_box));

        // One width and one colour on every side: a ring. Only for a box drawn
        // in one piece — a piece in the middle of a broken box has no start
        // edge and no end edge, so there is no ring to draw.
        if first
            && last
            && let Some(color) = self.uniform_border(id, style)
        {
            out.push(DisplayItem::Fill {
                box_id: id,
                path: ring(border_box, corners, geometry.border),
                paint: Paint::Solid(color),
            });
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
                box_id: id,
                path: rect_path(rect),
                paint: Paint::Solid(color),
            });
        }
        if rounded {
            out.push(DisplayItem::PopClip);
        }
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
        let Some(style) = self.nearest_styled_ancestor(id) else {
            return;
        };
        let color = style.color("color").unwrap_or(style.current_color());
        let size = style.font_size();
        let request = self.font_request(id);
        let Some(font) = self
            .context
            .fonts
            .chain(&request)
            .first()
            .map(|font| (*font).clone())
        else {
            return;
        };
        let ascender = font.metrics(size).ascender;
        let shadows = self.text_shadows_of(id);
        let letter_spacing = style
            .get("letter-spacing")
            .filter(|value| !value.eq_ignore_ascii_case("normal"))
            .and_then(|_| style.px("letter-spacing", 0.0))
            .unwrap_or(0.0);

        let decorations = self.decorations_of(id);
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
                let baseline = geometry.border_box.top() + ascender;
                out.push(DisplayItem::Text {
                    box_id: id,
                    text: trimmed.to_owned(),
                    origin: (geometry.border_box.left(), baseline),
                    font: font.clone(),
                    size,
                    letter_spacing,
                    color,
                    shadows,
                });
                for (line, colour) in &decorations {
                    out.push(Self::decoration_at(
                        id,
                        *line,
                        *colour,
                        geometry.border_box.left(),
                        geometry.border_box.right() - geometry.border_box.left(),
                        baseline,
                        font.metrics(size),
                    ));
                }
            }
            return;
        }
        for fragment in fragments {
            let Some(range) = fragment.text.clone() else {
                continue;
            };
            let Some(piece) = text.get(range) else {
                continue;
            };
            let trimmed = piece.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            let baseline = fragment.rect.top() + ascender;
            out.push(DisplayItem::Text {
                box_id: id,
                text: trimmed.to_owned(),
                // The pen sits on the baseline, which is the ascender below
                // the top of the piece.
                origin: (fragment.rect.left(), baseline),
                font: font.clone(),
                size,
                letter_spacing,
                color,
                shadows: shadows.clone(),
            });
            // One line per **fragment**, which is what makes a decoration stop
            // at the end of the inline rather than running to the edge of the
            // line it sits on. A fragment is one piece of one inline on one
            // line, so drawing per fragment is the whole of that rule.
            for (line, colour) in &decorations {
                out.push(Self::decoration_at(
                    id,
                    *line,
                    *colour,
                    fragment.rect.left(),
                    fragment.rect.right() - fragment.rect.left(),
                    baseline,
                    font.metrics(size),
                ));
            }
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
    fn control_state_of(&self, id: BoxId, out: &mut Vec<DisplayItem>) {
        let Some(node) = self.boxes.get(id) else {
            return;
        };
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
            box_id: id,
            path: rounded_rectangle(area, corners),
            paint: Paint::Solid(accent),
        });
        out.push(DisplayItem::Fill {
            box_id: id,
            path: control::mark(kind, area),
            paint: Paint::Solid(control::mark_color(accent)),
        });
    }

    /// Into the *content* box rather than the border box, because a picture
    /// sits inside its own padding and border like any other content — an
    /// `<img>` with a border draws the border around the picture rather than
    /// over it.
    fn picture_of(&self, id: BoxId, out: &mut Vec<DisplayItem>) {
        let Some(picture) = self.context.pictures.get(&id) else {
            return;
        };
        let Some(geometry) = self.layout.get(id) else {
            return;
        };
        out.push(DisplayItem::Picture {
            box_id: id,
            rect: geometry.content_box(),
            picture: std::sync::Arc::clone(picture),
        });
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

    /// One decoration line under, over or through a piece of text.
    ///
    /// `at` is the baseline, which is where every one of these is measured
    /// from — the face says how far below it an underline goes, and the other
    /// two are placed against the same ascent the letters use.
    fn decoration_at(
        id: BoxId,
        line: DecorationLine,
        colour: Rgba,
        left: f32,
        width: f32,
        baseline: f32,
        metrics: alo_text::FaceMetrics,
    ) -> DisplayItem {
        let thickness = metrics.underline_thickness.max(1.0);
        let top = match line {
            DecorationLine::Underline => baseline + metrics.underline_offset,
            // Along the top of the letters rather than the top of the line
            // box, which is where a reader expects it and which keeps it clear
            // of the line above.
            DecorationLine::Overline => baseline - metrics.ascender,
            // Half way up an `x`, which is what "through" means for lowercase
            // text and which is where every implementation puts it.
            DecorationLine::LineThrough => baseline - metrics.x_height / 2.0 - thickness / 2.0,
        };
        DisplayItem::Fill {
            box_id: id,
            // A plain rectangle: a decoration has no corners of its own, so
            // the rounding machinery would only be a way to get it wrong.
            path: rounded_rectangle(Rect::new(left, top, width, thickness), Corners::default()),
            paint: Paint::Solid(colour),
        }
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
            weight: style
                .number("font-weight")
                .map_or(Weight::NORMAL, |number| {
                    let clamped = number.clamp(1.0, 1000.0).round();
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "clamped to one..=1000 and rounded"
                    )]
                    let weight = clamped as u16;
                    Weight::new(weight)
                }),
            slant: match style.get("font-style") {
                Some(value) if value.eq_ignore_ascii_case("italic") => Slant::Italic,
                Some(value) if value.eq_ignore_ascii_case("oblique") => Slant::Italic,
                _ => Slant::Normal,
            },
        }
    }

    fn style_of(&self, id: BoxId) -> Option<&alo_style::ComputedStyle> {
        self.boxes
            .get(id)
            .and_then(|node| node.kind.node())
            .and_then(|source| self.styles.get(source))
    }

    /// The style a box's text is set with.
    ///
    /// A text box came from a text node, which has no style of its own — text
    /// inherits everything from the element that holds it, so that is the
    /// element to ask.
    fn nearest_styled_ancestor(&self, id: BoxId) -> Option<&alo_style::ComputedStyle> {
        let mut current = Some(id);
        while let Some(box_id) = current {
            if let Some(style) = self.style_of(box_id) {
                return Some(style);
            }
            current = self.boxes.get(box_id).and_then(|node| node.parent);
        }
        None
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

/// The colour a box's background is painted in.
///
/// `background-color` if it was written; otherwise `background`, when the
/// whole of it is a colour. Stage 1 does not expand shorthands in the cascade
/// — a shorthand arrives whole — and `background: #fff` is how a style sheet
/// actually says this, so reading it here is the difference between drawing
/// the page and drawing nothing.
///
/// A `background` that is an image or a gradient is not a colour and is
/// ignored, which is right: those are queue item 18, and painting the colour
/// out of a gradient would be a wrong pixel that looks nearly right.
fn background_color(style: &alo_style::ComputedStyle) -> Option<Rgba> {
    if let Some(color) = style.color("background-color") {
        return Some(color);
    }
    style.color("background")
}

fn rect_path(rect: Rect) -> Path {
    Path::rectangle(rect.left(), rect.top(), rect.size.width, rect.size.height)
}

/// The shape an outset shadow blurs: the box, grown by the spread and moved by
/// the offset.
///
/// The corners grow with it, so a shadow under a rounded card is rounded
/// rather than a rounded shape inside a square blur.
fn cast(border_box: Rect, corners: Corners, shadow: DrawnShadow) -> Path {
    rounded_rectangle(
        grown_rect(border_box, shadow.spread)
            .translated(alo_layout::Point::new(shadow.offset.0, shadow.offset.1)),
        grown_corners(corners, shadow.spread),
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
        grown_corners(corners, -shadow.spread),
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

/// Corners for a shape that grew: a radius grows with the shape around it, and
/// never past straight.
fn grown_corners(corners: Corners, by: f32) -> Corners {
    let grow = |(across, down): (f32, f32)| ((across + by).max(0.0), (down + by).max(0.0));
    Corners {
        top_left: grow(corners.top_left),
        top_right: grow(corners.top_right),
        bottom_right: grow(corners.bottom_right),
        bottom_left: grow(corners.bottom_left),
    }
}

/// The gradient a box's background is painted with, if it is painted with one.
///
/// `background-image` if it was written, otherwise `background` — the same
/// order, and for the same reason, as the colour beside it: stage 1 does not
/// expand shorthands in the cascade, so a shorthand arrives whole.
fn background_gradient(style: &alo_style::ComputedStyle) -> Option<Gradient> {
    for property in ["background-image", "background"] {
        if let Some(text) = style.get(property)
            && let Some(gradient) = alo_value::parse_gradient(text)
        {
            return Some(gradient);
        }
    }
    None
}
