//! The display list: what to draw, in what order.
//!
//! A step between layout and pixels, and it earns its place twice over. It is
//! **what a reference render diffs against when a picture differs** — a list
//! says "the background moved four pixels" where an image says only "these
//! bytes differ" — and it is where paint order is decided, once, rather than
//! being implied by the order some loop happens to visit boxes in.
//!
//! # Paint order
//!
//! Backgrounds and borders before content, parents before children, and
//! positioned boxes after everything in the flow — ordered by `z-index` and,
//! where two agree, by which came first. That is the small, honest part of
//! CSS's stacking model: enough for an interface, and short of the full
//! painting order, which `docs/features.md` reaches for with transforms and
//! opacity.

use crate::corner::{Corners, ring, rounded_rectangle};
use crate::path::Path;
use alo_box::{BoxId, BoxKind, BoxTree};
use alo_layout::{LayoutTree, Rect};
use alo_style::StyleTree;
use alo_text::{Font, FontDatabase, FontRequest, Slant, Weight};
use alo_value::Rgba;
use core::fmt::Write as _;

/// One thing to draw.
#[derive(Debug, Clone)]
pub enum DisplayItem {
    /// A solid colour inside a shape.
    Fill {
        /// The box it belongs to, so that a difference can be named.
        box_id: BoxId,
        /// What to fill.
        path: Path,
        /// What colour.
        color: Rgba,
    },
    /// Everything until the matching [`DisplayItem::PopClip`] is drawn only
    /// where this shape covers.
    ///
    /// A pair rather than a field on every item: a clip applies to a whole
    /// subtree, and repeating it on each item would be the same shape stored a
    /// hundred times and rasterised a hundred times.
    PushClip {
        /// The box that asked for it.
        box_id: BoxId,
        /// What to clip to.
        path: Path,
    },
    /// The end of the innermost clip.
    PopClip,
    /// A run of text.
    Text {
        /// The box it belongs to.
        box_id: BoxId,
        /// What it says.
        text: String,
        /// Where the pen starts: the left end of the baseline.
        origin: (f32, f32),
        /// What font.
        font: Font,
        /// How big.
        size: f32,
        /// What colour.
        color: Rgba,
    },
}

impl DisplayItem {
    /// The box this came from, or [`None`] for the end of a clip, which came
    /// from wherever the clip did.
    pub fn box_id(&self) -> Option<BoxId> {
        match self {
            DisplayItem::Fill { box_id, .. }
            | DisplayItem::Text { box_id, .. }
            | DisplayItem::PushClip { box_id, .. } => Some(*box_id),
            DisplayItem::PopClip => None,
        }
    }
}

/// Everything to draw, in the order to draw it.
#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    items: Vec<DisplayItem>,
}

impl DisplayList {
    /// The items, in paint order.
    pub fn items(&self) -> &[DisplayItem] {
        &self.items
    }

    /// How many there are.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there is nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Add an item, for a test that needs a list without a document.
    ///
    /// There is no other way to make one: a display list comes from a laid-out
    /// document, which is what keeps it in step with one. A test about the
    /// renderer alone needs a list anyway, and this is it.
    pub fn push_for_tests(&mut self, item: DisplayItem) {
        self.items.push(item);
    }

    /// The list as one line per item, for a test to compare.
    ///
    /// This is what makes a picture that changed *diagnosable*: the line says
    /// which box, what colour and where, so a failure reads "the row's
    /// background moved four pixels" rather than "the image differs".
    pub fn to_outline(&self) -> String {
        let mut out = String::new();
        for item in &self.items {
            // Writing to a `String` cannot fail.
            let _ = match item {
                DisplayItem::Fill {
                    box_id,
                    path,
                    color,
                } => {
                    let (left, top, right, bottom) = path.bounds().unwrap_or((0.0, 0.0, 0.0, 0.0));
                    writeln!(
                        out,
                        "fill {box_id} {color} at ({left}, {top}) {}×{}",
                        right - left,
                        bottom - top,
                    )
                }
                DisplayItem::Text {
                    box_id,
                    text,
                    origin,
                    size,
                    color,
                    ..
                } => writeln!(
                    out,
                    "text {box_id} {text:?} {color} {size}px at ({}, {})",
                    origin.0, origin.1,
                ),
                DisplayItem::PushClip { box_id, path } => {
                    let (left, top, right, bottom) = path.bounds().unwrap_or((0.0, 0.0, 0.0, 0.0));
                    writeln!(
                        out,
                        "clip {box_id} to ({left}, {top}) {}×{}",
                        right - left,
                        bottom - top,
                    )
                }
                DisplayItem::PopClip => writeln!(out, "unclip"),
            };
        }
        out
    }
}

/// What paint needs that is not in the layout: the fonts, and the colour of
/// the page behind everything.
#[derive(Debug, Clone, Copy)]
pub struct PaintContext<'a> {
    /// The fonts to draw text with.
    pub fonts: &'a FontDatabase,
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
        in_flow: Vec::new(),
        positioned: Vec::new(),
    };
    if let Some(root) = boxes.root() {
        builder.walk(root);
    }
    // Positioned boxes go over everything in the flow, in `z-index` order and
    // then in the order they were written — which is what makes two boxes with
    // the same `z-index` stack predictably.
    builder.positioned.sort_by_key(|(z, order, _)| (*z, *order));
    let mut items = builder.in_flow;
    for (_, _, item) in builder.positioned {
        items.push(item);
    }
    DisplayList { items }
}

struct Builder<'a> {
    boxes: &'a BoxTree,
    layout: &'a LayoutTree,
    styles: &'a StyleTree,
    context: PaintContext<'a>,
    in_flow: Vec<DisplayItem>,
    positioned: Vec<(i32, usize, DisplayItem)>,
}

impl Builder<'_> {
    fn walk(&mut self, id: BoxId) {
        let z_index = self.z_index_of(id);
        let mut mine = Vec::new();
        self.draw_box(id, &mut mine);

        match z_index {
            Some(z) => {
                for item in mine {
                    let order = self.positioned.len();
                    self.positioned.push((z, order, item));
                }
            }
            None => self.in_flow.extend(mine),
        }

        // A box that clips holds its children inside its own shape — which,
        // if it has rounded corners, is a rounded shape. One question asked
        // twice: what shape is this box.
        let clip = self.clip_of(id);
        if let Some(path) = clip.clone() {
            self.in_flow
                .push(DisplayItem::PushClip { box_id: id, path });
        }
        let children: Vec<BoxId> = self.boxes.children(id).collect();
        for child in children {
            self.walk(child);
        }
        if clip.is_some() {
            self.in_flow.push(DisplayItem::PopClip);
        }
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
        Some(rounded_rectangle(
            geometry.padding_box(),
            Corners::of(style),
        ))
    }

    /// The `z-index` of a positioned box, or [`None`] for one in the flow.
    ///
    /// `z-index` only means anything on a positioned box, which is a rule
    /// people are surprised by often enough that it is worth saying here.
    fn z_index_of(&self, id: BoxId) -> Option<i32> {
        let style = self.style_of(id)?;
        let position = style.get("position")?;
        if position.eq_ignore_ascii_case("static") {
            return None;
        }
        let z = style
            .number("z-index")
            .map_or(0.0, |number| number)
            .clamp(-1.0e6, 1.0e6);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "clamped to a range i32 represents exactly"
        )]
        let whole = z as i32;
        Some(whole)
    }

    fn draw_box(&self, id: BoxId, out: &mut Vec<DisplayItem>) {
        let Some(geometry) = self.layout.get(id) else {
            return;
        };
        let Some(node) = self.boxes.get(id) else {
            return;
        };

        // A background fills the border box; a border is drawn over it.
        let corners = self.style_of(id).map_or(Corners::SQUARE, Corners::of);
        if let Some(style) = self.style_of(id)
            && let Some(color) = background_color(style)
            && !color.is_invisible()
        {
            out.push(DisplayItem::Fill {
                box_id: id,
                path: rounded_rectangle(geometry.border_box, corners),
                color,
            });
        }
        self.draw_borders(id, geometry.border_box, out);

        if let BoxKind::Text { text, .. } = &node.kind {
            self.draw_text(id, text, out);
        }
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
    fn draw_borders(&self, id: BoxId, border_box: Rect, out: &mut Vec<DisplayItem>) {
        let Some(geometry) = self.layout.get(id) else {
            return;
        };
        let Some(style) = self.style_of(id) else {
            return;
        };
        let corners = Corners::of(style);

        // One width and one colour on every side: a ring.
        if let Some(color) = self.uniform_border(id, style) {
            out.push(DisplayItem::Fill {
                box_id: id,
                path: ring(border_box, corners, geometry.border),
                color,
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
            .filter(|(_, width)| *width > 0.0)
            .filter(|(side, _)| {
                style
                    .get(&format!("border-{side}-style"))
                    .is_some_and(|value| value.eq_ignore_ascii_case("solid"))
            })
            .filter_map(|(side, width)| {
                let color = style.color(&format!("border-{side}-color"))?;
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
                color,
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
            let drawn = style
                .get(&format!("border-{side}-style"))
                .is_some_and(|value| value.eq_ignore_ascii_case("solid"));
            if !drawn {
                return None;
            }
            let side_color = style.color(&format!("border-{side}-color"))?;
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

        for fragment in self.layout.fragments(id) {
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
            out.push(DisplayItem::Text {
                box_id: id,
                text: trimmed.to_owned(),
                // The pen sits on the baseline, which is the ascender below
                // the top of the piece.
                origin: (fragment.rect.left(), fragment.rect.top() + ascender),
                font: font.clone(),
                size,
                color,
            });
        }
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
