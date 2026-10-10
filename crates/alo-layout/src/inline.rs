/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A line box: what actually happens when text and boxes share a line.
//!
//! Until this file, an inline formatting context was a wrapping flex row —
//! which got boxes side by side and got nothing else right. This is the real
//! thing, and the three differences are the three reasons a row of boxes is
//! not a line of text:
//!
//! - **Text breaks across boxes.** "the <em>quick brown</em> fox" is one
//!   sentence and wraps between any two of its words, not only between the
//!   `<em>` and what is around it.
//! - **Everything sits on a baseline.** Two pieces of text at different sizes
//!   line up along the bottoms of their letters, not along their tops. A row
//!   cannot express that.
//! - **A box that wraps has more than one rectangle.** An `<a>` broken across
//!   two lines is two rectangles, and drawing it as the union of them would
//!   paint a background across the gap between the lines.
//!
//! # What is here and what is not
//!
//! Text and nested inline boxes are laid out here, in full. An **atomic**
//! inline-level box — an `inline-block`, an image, a button — has a size of
//! its own that only its own layout can give, so the caller supplies it; the
//! line places it and aligns it on the baseline.
//!
//! # An inline box has a box of its own
//!
//! A nested inline box is not only a bracket around its text. It has a
//! background, a border and a padding, and CSS puts them in a particular
//! place: its **horizontal** border and padding take room on the line, once at
//! its start and once at its end, and its **vertical** ones draw without
//! changing the height of the line. That is why an inline box arrives as an
//! [`InlineItem::Open`] and an [`InlineItem::Close`] around its content rather
//! than as one item, and why it gets **one fragment per line it is on** like
//! anything else that wraps.
//!
//! # Every line starts as tall as its container's font
//!
//! CSS begins each line box with a **strut**: a zero-width inline box in the
//! font of the block that holds the lines. Nothing draws it and nothing can
//! be placed against it, but its ascent and descent count towards the line's
//! height like anything else on it. That is why a line holding only a picture
//! still has the font's descent below the picture's bottom edge — the room a
//! descender would need, were there text — and why a line of small text in a
//! block set large is as tall as the large font. Without it, a line is only
//! as tall as what happens to be on it, and the same block grows and shrinks
//! with its content's font rather than its own.
//!
//! The strut does not make a line exist. A line with nothing on it worth a
//! line box is still no line at all, strut or not.
//!
//! # `line-height` is room around the font, not the font
//!
//! A piece of text, an inline box and the strut each take as much room on
//! the line as their `line-height`, not as their font. The difference between
//! the two is the **leading**, and CSS puts half of it above the font's ascent
//! and half below its descent, so the text stays in the middle of the room it
//! was given. A 16 px line at `line-height: 1.5` is 24 tall with four pixels
//! over the letters and four under them; a `line-height` smaller than the font
//! is a negative leading, and the letters reach past the room they take.
//!
//! Only the room changes. What is drawn — a text fragment, an inline box's
//! background — is still the font's height, placed on the baseline, which is
//! why a link's background in a loosely set paragraph does not fill the gap
//! between its lines. `normal` adds no leading at all: the line is as tall as
//! its font.
//!
//! # Not everything stands on the baseline
//!
//! `vertical-align` moves an atomic box or an inline box, and everything
//! inside it, up or down from its parent's baseline; [`crate::vertical_align`]
//! says how far. What moved still counts towards how tall the line is. `top`
//! and `bottom` are different: a box held by the edge of the line box is laid
//! out around a baseline of its own, in a **group** of its own, and only once
//! everything else has said how tall the line is does the group find out where
//! that edge is.
//!
//! # A `<br>` ends its line
//!
//! A forced break is an [`InlineItem::Break`]: whatever comes after it starts
//! a new line, full or not. It takes no room across the line and draws
//! nothing, and it still stands on the line it ends like a piece of text in
//! its own font — so that font and its `line-height` count towards that
//! line's height, and the break has a place an agent can be told about. A
//! break with nothing before it on its line makes the line anyway, which is
//! why two in a row leave a blank line; one with nothing after it starts no
//! line at all, which is why a paragraph ending in a `<br>` is no taller for
//! it. That is the rule a kept newline in `white-space: pre` follows too.

use crate::geometry::{Edges, Point, Rect, Size};
use crate::measure::{MeasureText, TextStyle};
use crate::vertical_align::{Edge, LineAlign, Parent};
use alo_box::BoxId;
use core::ops::Range;

/// One thing that takes room on a line.
#[derive(Debug, Clone)]
pub enum InlineItem {
    /// Text, which may be broken between lines.
    Text {
        /// The box the text came from.
        box_id: BoxId,
        /// The text.
        text: String,
        /// The font it is set in. Per item, because two pieces of text on one
        /// line can be different sizes and still share a baseline.
        style: TextStyle,
    },
    /// The start of a nested inline box: everything until its
    /// [`InlineItem::Close`] is inside it.
    Open {
        /// The box.
        box_id: BoxId,
        /// The room its own start border and padding take on the line. Only
        /// here, and only once: a box broken across two lines has a start edge
        /// on the first piece and an end edge on the last.
        edge: f32,
        /// The font it is set in, which is what the height of its content area
        /// comes from.
        style: TextStyle,
        /// How far its painted area reaches above its content area: its top
        /// border and padding. It does **not** make the line taller, which is
        /// what CSS says and what stops a padded `<em>` pushing a paragraph's
        /// lines apart.
        over: f32,
        /// The same below: its bottom border and padding.
        under: f32,
        /// Its `vertical-align`, which moves everything inside it too.
        align: LineAlign,
    },
    /// A forced line break: a `<br>`.
    ///
    /// What follows it starts a new line. See the module's own section.
    Break {
        /// The box.
        box_id: BoxId,
        /// Its font, which the line it ends is at least as tall as.
        style: TextStyle,
    },
    /// The end of a nested inline box.
    Close {
        /// The box.
        box_id: BoxId,
        /// The room its own end border and padding take on the line.
        edge: f32,
    },
    /// A box with a size of its own, which is placed whole or not at all.
    ///
    /// What sits on the line is its **margin box**, not its border box: its
    /// margins take room across the line and make the line taller, which is
    /// what leaves the gap an author wrote under a picture rather than putting
    /// the next block straight against it. The fragment it gets is still its
    /// border box, because that is what draws.
    Atomic {
        /// The box.
        box_id: BoxId,
        /// How big its border box is.
        size: Size,
        /// Its margins, which take room on the line around that border box.
        margin: Edges,
        /// How far below the top of its **margin box** its baseline sits.
        ///
        /// For a box with no line in it, CSS puts the baseline on its bottom
        /// margin edge — the whole of the margin box above it.
        baseline: f32,
        /// Its `vertical-align`: what its margin box is lined up with.
        align: LineAlign,
    },
}

impl InlineItem {
    /// The box this item belongs to.
    pub fn box_id(&self) -> BoxId {
        match self {
            InlineItem::Text { box_id, .. }
            | InlineItem::Atomic { box_id, .. }
            | InlineItem::Open { box_id, .. }
            | InlineItem::Break { box_id, .. }
            | InlineItem::Close { box_id, .. } => *box_id,
        }
    }
}

/// A fragment while its line is still being built.
///
/// It carries how far below the baseline the piece reaches, which the line
/// needs to place it and nobody needs afterwards.
#[derive(Debug, Clone)]
struct Pending {
    fragment: Fragment,
    below_baseline: f32,
    /// Which baseline it hangs from, and how far above that one its own is.
    place: Place,
    /// Room after the fragment that is still its own: an atomic box's right
    /// margin. It counts in how wide the line is, so that aligning a line
    /// moves the margin box rather than the border box.
    after: f32,
}

/// A piece of one box, on one line.
///
/// A box that fits on one line has one of these. A box that wraps has one per
/// line, which is what makes a background on a wrapped link stop at the end of
/// each line rather than crossing the gap between them.
#[derive(Debug, Clone, PartialEq)]
pub struct Fragment {
    /// The box this is a piece of.
    pub box_id: BoxId,
    /// Where it is, relative to the top-left of the formatting context.
    pub rect: Rect,
    /// Which bytes of the box's text this piece covers, for a text box.
    pub text: Option<Range<usize>>,
    /// Which line it is on, counting from zero.
    pub line: usize,
}

/// One line of a formatting context.
#[derive(Debug, Clone, PartialEq)]
pub struct LineBox {
    /// The pieces on it, in the order they were laid down.
    pub fragments: Vec<Fragment>,
    /// How wide the line's content is.
    pub width: f32,
    /// How far the baseline sits below the top of the line.
    pub baseline: f32,
    /// How tall the line is.
    pub height: f32,
    /// How far the top of the line is below the top of the context.
    pub top: f32,
}

/// Everything an inline formatting context worked out.
#[derive(Debug, Clone, Default)]
pub struct InlineLayout {
    /// The lines, top to bottom.
    pub lines: Vec<LineBox>,
    /// How big the whole thing is.
    pub size: Size,
}

impl InlineLayout {
    /// Every piece of every box, in the order they were laid down.
    pub fn fragments(&self) -> impl Iterator<Item = &Fragment> {
        self.lines.iter().flat_map(|line| line.fragments.iter())
    }

    /// The rectangle a box occupies: the union of its pieces.
    ///
    /// Useful for "where is this", and **not** what should be painted — a box
    /// on two lines has a gap in the middle that the union covers over. Paint
    /// wants [`InlineLayout::fragments`].
    pub fn union_for(&self, box_id: BoxId) -> Option<Rect> {
        let mut found: Option<Rect> = None;
        for fragment in self.fragments().filter(|held| held.box_id == box_id) {
            found = Some(match found {
                None => fragment.rect,
                Some(held) => union(held, fragment.rect),
            });
        }
        found
    }
}

fn union(left: Rect, right: Rect) -> Rect {
    let x = left.left().min(right.left());
    let y = left.top().min(right.top());
    Rect::new(
        x,
        y,
        left.right().max(right.right()) - x,
        left.bottom().max(right.bottom()) - y,
    )
}

/// Where a line sits in the room it was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlignment {
    /// At the start, which is what a line does when nobody says otherwise.
    #[default]
    Start,
    /// In the middle.
    Center,
    /// At the end.
    End,
}

impl TextAlignment {
    /// What `text-align` says, or [`None`] for a value this engine does not
    /// implement — `justify`, which needs to stretch the spaces between words.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        for (alignment, names) in [
            (TextAlignment::Start, ["start", "left"]),
            (TextAlignment::Center, ["center", "center"]),
            (TextAlignment::End, ["end", "right"]),
        ] {
            if names.iter().any(|name| text.eq_ignore_ascii_case(name)) {
                return Some(alignment);
            }
        }
        None
    }

    /// How far along the leftover room a line starts.
    fn share(self) -> f32 {
        match self {
            TextAlignment::Start => 0.0,
            TextAlignment::Center => 0.5,
            TextAlignment::End => 1.0,
        }
    }
}

/// Lay items into lines no wider than `available_width`, in a block set in
/// the default font.
///
/// `available_width` of [`None`] is the max-content question — how wide it
/// would like to be — and puts everything on one line.
pub fn lay_out(
    items: &[InlineItem],
    available_width: Option<f32>,
    measurer: &impl MeasureText,
) -> InlineLayout {
    lay_out_aligned(
        items,
        available_width,
        TextAlignment::Start,
        &TextStyle::default(),
        measurer,
    )
}

/// The same, with the lines sitting where `text-align` says, in a block whose
/// font is `strut` — the font every line starts at least as tall as.
pub fn lay_out_aligned(
    items: &[InlineItem],
    available_width: Option<f32>,
    alignment: TextAlignment,
    strut: &TextStyle,
    measurer: &impl MeasureText,
) -> InlineLayout {
    let mut builder = Builder::new(available_width, strut, measurer);
    builder.alignment = alignment;
    for item in items {
        match item {
            InlineItem::Text {
                box_id,
                text,
                style,
            } => builder.add_text(*box_id, text, style),
            InlineItem::Atomic {
                box_id,
                size,
                margin,
                baseline,
                align,
            } => builder.add_atomic(*box_id, *size, *margin, *baseline, *align),
            InlineItem::Open {
                box_id,
                edge,
                style,
                over,
                under,
                align,
            } => builder.open(*box_id, *edge, style, (*over, *under), *align),
            InlineItem::Close { box_id, edge } => builder.close(*box_id, *edge),
            InlineItem::Break { box_id, style } => builder.add_break(*box_id, style),
        }
    }
    builder.finish()
}

/// Lays items down, one line at a time.
struct Builder<'a, M: MeasureText> {
    available_width: Option<f32>,
    measurer: &'a M,
    alignment: TextAlignment,
    lines: Vec<LineBox>,
    current: Vec<Pending>,
    /// The inline boxes this line is currently inside, outermost first.
    open: Vec<OpenBox>,
    /// Whether anything on this line is worth a line box.
    ///
    /// CSS: a line box holding no text, no preserved space and no inline box
    /// with a margin, padding or border is **zero-height and treated as not
    /// existing**. That rule is why an inline box broken around a block can
    /// keep its empty piece — the piece draws its border when it has one, and
    /// costs nothing at all when it does not.
    content: bool,
    pen: f32,
    /// How far what is on this line reaches, one entry for the line's own
    /// baseline and one for each box held by its top or bottom edge.
    groups: Vec<Group>,
    /// The container's font: what a box that is not inside another inline box
    /// is aligned against.
    strut: Parent,
    /// How far the strut reaches above and below the baseline, leading and
    /// all: where every line's ascent and descent start.
    strut_reach: Reach,
}

/// How far something takes room above and below its baseline on a line: its
/// font's ascent and descent with half its leading added to each.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Reach {
    above: f32,
    below: f32,
}

/// Which baseline a piece hangs from.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Place {
    /// The group: zero is the line's own baseline.
    group: usize,
    /// How far above that group's baseline the piece's own baseline is.
    raise: f32,
}

/// The line's own baseline, at the start of every line.
const ON_THE_LINE: Place = Place {
    group: 0,
    raise: 0.0,
};

/// Everything hanging from one baseline, and how far it reaches.
#[derive(Debug, Clone, Copy)]
struct Group {
    /// [`None`] for the line's own baseline; otherwise the edge of the line
    /// box that holds this group.
    edge: Option<Edge>,
    ascent: f32,
    descent: f32,
}

/// A nested inline box that has started and not yet finished.
///
/// It becomes a fragment when it closes, and another one every time the line
/// ends before it does — which is what gives a wrapped `<em>` one rectangle
/// per line instead of one rectangle with a hole in the middle.
struct OpenBox {
    box_id: BoxId,
    /// Where on the current line it began.
    start: f32,
    /// The first fragment on this line that is inside it.
    from: usize,
    /// How far its content area reaches above and below the baseline.
    ascent: f32,
    descent: f32,
    /// How far it takes room above and below the baseline: its content area
    /// with its leading. It is what the box is aligned by and what it adds to
    /// the line's height.
    reach: Reach,
    /// How far its painted area reaches beyond that.
    over: f32,
    under: f32,
    /// Its font, which is what anything inside it is aligned against.
    font: Parent,
    /// Its `vertical-align`, kept so that it can be placed again on every
    /// line it carries on to.
    align: LineAlign,
    /// Where it hangs on the current line.
    place: Place,
}

impl<'a, M: MeasureText> Builder<'a, M> {
    fn new(available_width: Option<f32>, strut: &TextStyle, measurer: &'a M) -> Self {
        let strut_reach = reach_of(strut, measurer);
        let strut = font_of(strut, measurer);
        Self {
            available_width,
            measurer,
            alignment: TextAlignment::Start,
            lines: Vec::new(),
            current: Vec::new(),
            open: Vec::new(),
            content: false,
            pen: 0.0,
            groups: vec![Group {
                edge: None,
                ascent: strut_reach.above,
                descent: strut_reach.below,
            }],
            strut,
            strut_reach,
        }
    }

    /// Begin a new line: back at the left edge, and as tall as the strut.
    ///
    /// A box still open from the line before is placed again, because the
    /// group a `top` or `bottom` box hung from belonged to that line.
    fn start_line(&mut self) {
        self.pen = 0.0;
        self.groups = vec![Group {
            edge: None,
            ascent: self.strut_reach.above,
            descent: self.strut_reach.below,
        }];
        for depth in 0..self.open.len() {
            let Some(held) = self.open.get(depth) else {
                continue;
            };
            let (align, reach) = (held.align, held.reach);
            let place = self.place_at(depth, align, reach.above, reach.below);
            if let Some(held) = self.open.get_mut(depth) {
                held.place = place;
            }
        }
    }

    /// What a box `depth` boxes deep is inside: the open box above it, or the
    /// block that holds the lines.
    fn parent_at(&self, depth: usize) -> (Place, Parent) {
        depth
            .checked_sub(1)
            .and_then(|above| self.open.get(above))
            .map_or((ON_THE_LINE, self.strut), |held| (held.place, held.font))
    }

    /// Where a box reaching `above` and `below` its own baseline hangs, given
    /// its `vertical-align` and what it is inside.
    fn place_at(&mut self, depth: usize, align: LineAlign, above: f32, below: f32) -> Place {
        let (outer, parent) = self.parent_at(depth);
        match align.raise(above, below, parent) {
            Ok(raise) => Place {
                group: outer.group,
                raise: outer.raise + raise,
            },
            Err(edge) => {
                self.groups.push(Group {
                    edge: Some(edge),
                    ascent: 0.0,
                    descent: 0.0,
                });
                Place {
                    group: self.groups.len() - 1,
                    raise: 0.0,
                }
            }
        }
    }

    /// Something reaching `above` and `below` its own baseline, hung at
    /// `place`, makes its group reach at least that far.
    fn reach(&mut self, place: Place, above: f32, below: f32) {
        if let Some(group) = self.groups.get_mut(place.group) {
            group.ascent = group.ascent.max(above + place.raise);
            group.descent = group.descent.max(below - place.raise);
        }
    }

    /// Where text laid down now hangs: with the box it is in.
    fn innermost(&self) -> Place {
        self.open.last().map_or(ON_THE_LINE, |held| held.place)
    }

    /// Whether something of this width still fits on the line being built.
    ///
    /// Something wider than the whole line goes on it anyway when the line is
    /// empty: there is nowhere else for it, and overflowing is what CSS says
    /// to do rather than dropping it.
    fn fits(&self, width: f32) -> bool {
        match self.available_width {
            None => true,
            Some(available) => self.current.is_empty() || self.pen + width <= available + 0.001,
        }
    }

    /// Whether a line may break here at all.
    ///
    /// `pre` and `nowrap` say no, and a line then overflows rather than
    /// wrapping — which is what the author asked for by writing them.
    fn may_wrap(style: &TextStyle) -> bool {
        style.white_space.wraps()
    }

    fn add_text(&mut self, box_id: BoxId, text: &str, style: &TextStyle) {
        if style.white_space.keeps_newlines() && text.contains('\n') {
            // A kept newline is a break that **must** happen, so the text is
            // laid down one line's worth at a time. The byte offsets are the
            // original string's throughout, because a fragment names a range
            // of the box's own text and the newline is still in it.
            let mut at = 0usize;
            for (index, piece) in text.split('\n').enumerate() {
                if index > 0 {
                    self.break_line();
                    at += 1;
                }
                self.add_run(box_id, text, at..at + piece.len(), style);
                at += piece.len();
            }
            return;
        }
        self.add_run(box_id, text, 0..text.len(), style);
    }

    /// End this line because the text said to, rather than because it is full.
    ///
    /// A forced break makes a line even when there is nothing on it: two
    /// newlines in a row are a blank line, and a page that quietly dropped it
    /// would be a page missing a paragraph's worth of space.
    fn break_line(&mut self) {
        if self.current.is_empty() {
            self.content = true;
        }
        self.end_line();
    }

    /// A `<br>`: stand on this line as a piece of text with no width, then
    /// end it.
    ///
    /// Its fragment is the font's height where what is drawn on the line
    /// ends, so that the box has a rectangle where the line ended rather than
    /// none, and its reach makes the line it ends at least as tall as its
    /// `line-height`. Not at the pen: CSS removes the spaces at the end of a
    /// line, and `once. <br>` ends at the full stop.
    fn add_break(&mut self, box_id: BoxId, style: &TextStyle) {
        let ascent = self.measurer.ascender(style);
        let descent = self.measurer.descender(style);
        let reach = reach_of(style, self.measurer);
        let place = self.innermost();
        let at = self
            .current
            .iter()
            .map(|pending| pending.fragment.rect.right() + pending.after)
            .chain(self.open.last().map(|held| held.start))
            .fold(0.0, f32::max);
        self.current.push(Pending {
            fragment: Fragment {
                box_id,
                rect: Rect::new(at, 0.0, 0.0, ascent + descent),
                text: None,
                line: self.lines.len(),
            },
            below_baseline: descent,
            place,
            after: 0.0,
        });
        self.reach(place, reach.above, reach.below);
        // A break is content even alone on its line: `<br><br>` is a blank
        // line, and `<p><br></p>` is a paragraph one line tall.
        self.content = true;
        self.end_line();
    }

    /// One stretch of text with no forced break in it.
    fn add_run(&mut self, box_id: BoxId, whole: &str, range: Range<usize>, style: &TextStyle) {
        let Some(text) = whole.get(range.clone()) else {
            return;
        };
        let offset = range.start;
        let mut start = 0usize;
        for end in self.measurer.break_opportunities(text) {
            if end <= start {
                continue;
            }
            let Some(piece) = text.get(start..end) else {
                continue;
            };
            // The trailing space of a piece does not count towards the width
            // that has to fit: a line may end in a space, and counting it
            // would break a line one word early.
            let visible = piece.trim_end();
            let width = self.measurer.measure(visible, style, None).width;
            if Self::may_wrap(style) && !self.fits(width) {
                self.end_line();
            }
            if visible.is_empty() {
                // Whitespace of its own — the space between two `<a>`s, which
                // arrives as a text box in its own right. It draws nothing, and
                // it still takes room: without this, `All` and `Due` touch.
                // At the start of a line it takes none, because a line does not
                // begin with a space.
                if !self.current.is_empty() {
                    self.pen += self.measurer.measure(piece, style, None).width;
                }
                start = end;
                continue;
            }
            let placed = self.measurer.measure(piece, style, None).width;
            self.content = true;
            self.place_text(box_id, offset + start..offset + end, width, placed, style);
            start = end;
        }
    }

    fn place_text(
        &mut self,
        box_id: BoxId,
        range: Range<usize>,
        visible: f32,
        placed: f32,
        style: &TextStyle,
    ) {
        let ascent = self.measurer.ascender(style);
        let descent = self.measurer.descender(style);
        let reach = reach_of(style, self.measurer);
        let place = self.innermost();
        self.current.push(Pending {
            fragment: Fragment {
                box_id,
                rect: Rect::new(self.pen, 0.0, visible, ascent + descent),
                text: Some(range),
                line: self.lines.len(),
            },
            below_baseline: descent,
            place,
            after: 0.0,
        });
        self.pen += placed;
        self.reach(place, reach.above, reach.below);
    }

    /// Start a nested inline box.
    ///
    /// Its start border and padding take room on the line here and nowhere
    /// else. Its **content area** — the font's ascent and descent — with its
    /// leading counts towards the line's height; its top and bottom border
    /// and padding do not, which is CSS's rule and the reason a padded `<em>`
    /// does not push a paragraph's lines apart.
    ///
    /// Its `vertical-align` moves its content area, and so everything inside
    /// it, which is why it is placed here rather than piece by piece.
    fn open(
        &mut self,
        box_id: BoxId,
        edge: f32,
        style: &TextStyle,
        (over, under): (f32, f32),
        align: LineAlign,
    ) {
        let font = font_of(style, self.measurer);
        let (ascent, descent) = (font.ascent, font.descent);
        let reach = reach_of(style, self.measurer);
        self.pen += edge;
        // CSS aligns an inline box by the room it takes, leading included,
        // not by its font: `text-top` puts the top of its line height at the
        // top of its parent's font.
        let place = self.place_at(self.open.len(), align, reach.above, reach.below);
        self.reach(place, reach.above, reach.below);
        // An inline box with an edge of its own is content; one without is
        // only a bracket, and a line made of nothing but brackets is not a
        // line.
        self.content = self.content || edge > 0.0 || over > 0.0 || under > 0.0;
        self.open.push(OpenBox {
            box_id,
            start: self.pen - edge,
            from: self.current.len(),
            ascent,
            descent,
            reach,
            over,
            under,
            font,
            align,
            place,
        });
    }

    /// Finish the innermost nested inline box, and give it its last fragment.
    fn close(&mut self, box_id: BoxId, edge: f32) {
        self.content = self.content || edge > 0.0;
        let Some(index) = self.open.iter().rposition(|held| held.box_id == box_id) else {
            return;
        };
        self.pen += edge;
        let held = self.open.remove(index);
        // Mid-line the pen is the right answer: the end edge comes after
        // whatever the box held, trailing space included, because that space
        // is inside the box.
        let right = self.pen;
        self.piece_for(&held, right);
    }

    /// How far right the content of an open box actually reaches.
    ///
    /// Not the pen: a line that ends in a space has advanced past the last
    /// glyph, and a background painted to the pen would run out past the end
    /// of the text. A fragment's rectangle is the visible part, so the
    /// rightmost of them is where the box really ends.
    fn content_right(&self, held: &OpenBox) -> f32 {
        self.current
            .get(held.from..)
            .into_iter()
            .flatten()
            .map(|pending| pending.fragment.rect.right())
            .fold(held.start, f32::max)
    }

    /// One rectangle for an inline box, from where it started on this line to
    /// where it ends on it.
    fn piece_for(&mut self, held: &OpenBox, right: f32) {
        let height = held.ascent + held.descent + held.over + held.under;
        self.current.push(Pending {
            fragment: Fragment {
                box_id: held.box_id,
                rect: Rect::new(held.start, 0.0, (right - held.start).max(0.0), height),
                text: None,
                line: self.lines.len(),
            },
            below_baseline: held.descent + held.under,
            place: held.place,
            after: 0.0,
        });
    }

    /// Place an atomic box, margins and all.
    ///
    /// The margin box is what fits or does not, what moves the pen and what
    /// sits on the baseline; the fragment is the border box inside it.
    fn add_atomic(
        &mut self,
        box_id: BoxId,
        size: Size,
        margin: Edges,
        baseline: f32,
        align: LineAlign,
    ) {
        let width = margin.left + size.width + margin.right;
        let height = margin.top + size.height + margin.bottom;
        if !self.fits(width) {
            self.end_line();
        }
        self.content = true;
        // Placed only once it is known which line it is on: a `top` box's
        // group belongs to the line.
        let place = self.place_at(self.open.len(), align, baseline, height - baseline);
        self.current.push(Pending {
            fragment: Fragment {
                box_id,
                rect: Rect::new(self.pen + margin.left, 0.0, size.width, size.height),
                text: None,
                line: self.lines.len(),
            },
            // An atomic box sits with its own baseline on the line's. Measured
            // from its border box, which is the rectangle placed; with a bottom
            // margin the baseline is below that box, and this is negative.
            below_baseline: size.height - (baseline - margin.top),
            place,
            after: margin.right,
        });
        self.pen += width;
        self.reach(place, baseline, height - baseline);
    }

    /// Finish the line being built: put every fragment on the baseline, and
    /// start a new one.
    fn end_line(&mut self) {
        if self.current.is_empty() || !self.content {
            // Either nothing was laid down at all, or what was laid down is
            // only empty inline boxes with no edges of their own — which CSS
            // says is a zero-height line box, treated as not existing. Either
            // way there is no line, and any open box carries on to the next.
            self.current.clear();
            self.start_line();
            for held in &mut self.open {
                held.start = 0.0;
                held.from = 0;
            }
            return;
        }
        // A box still open when the line ends gets its piece for this line,
        // and starts again at the left edge of the next one — with no start
        // edge, because it has already had one.
        let open = core::mem::take(&mut self.open);
        // Every extent is worked out before any piece is pushed, so that a box
        // inside another does not change where the outer one is measured to.
        let extents: Vec<f32> = open.iter().map(|held| self.content_right(held)).collect();
        for (held, right) in open.iter().zip(extents) {
            self.piece_for(held, right);
        }
        self.open = open
            .into_iter()
            .map(|held| OpenBox {
                start: 0.0,
                from: 0,
                ..held
            })
            .collect();

        let top = self.lines.last().map_or(0.0, |line| line.top + line.height);
        let (baseline, height, hung) = self.settle();
        let width = self
            .current
            .iter()
            .map(|pending| pending.fragment.rect.right() + pending.after)
            .fold(0.0, f32::max);

        // Where the line sits in the room it was given. Leftover room only
        // exists when there is a width to have room in.
        let leftover = self
            .available_width
            .map_or(0.0, |available| (available - width).max(0.0));
        let start = leftover * self.alignment.share();

        let fragments: Vec<Fragment> = merge_adjacent(core::mem::take(&mut self.current))
            .into_iter()
            .map(|pending| {
                // This is what a line box is for: everything hangs from one
                // baseline, so a taller piece pushes the line down rather than
                // pushing the others up. A piece raised or lowered hangs that
                // far from it, and one in a `top` or `bottom` group hangs from
                // that group's.
                let above = pending.fragment.rect.size.height - pending.below_baseline;
                let from = hung.get(pending.place.group).copied().unwrap_or(baseline);
                let y = top + from - pending.place.raise - above;
                Fragment {
                    rect: pending.fragment.rect.translated(Point::new(start, y)),
                    ..pending.fragment
                }
            })
            .collect();

        self.lines.push(LineBox {
            fragments,
            width,
            baseline,
            height,
            top,
        });
        self.start_line();
        self.content = false;
    }

    /// How tall the line is and where its baseline is, and where each
    /// group's baseline is below the line's top.
    ///
    /// The line's own baseline group decides first. A group held by the top
    /// edge that is taller than that grows the line downwards, and one held by
    /// the bottom grows it upwards, so that neither pushes the other off the
    /// line.
    fn settle(&self) -> (f32, f32, Vec<f32>) {
        let (mut ascent, mut descent) = self
            .groups
            .first()
            .map_or((0.0, 0.0), |group| (group.ascent, group.descent));
        for group in self.groups.iter().skip(1) {
            let tall = group.ascent + group.descent;
            match group.edge {
                Some(Edge::Top) => descent = descent.max(tall - ascent),
                Some(Edge::Bottom) => ascent = ascent.max(tall - descent),
                None => {}
            }
        }
        let height = ascent + descent;
        let hung = self
            .groups
            .iter()
            .map(|group| match group.edge {
                None => ascent,
                Some(Edge::Top) => group.ascent,
                Some(Edge::Bottom) => height - group.descent,
            })
            .collect();
        (ascent, height, hung)
    }

    fn finish(mut self) -> InlineLayout {
        self.end_line();
        // Anything still open was never closed, which is the caller's mistake
        // rather than the line's; it keeps the piece `end_line` gave it.
        self.open.clear();
        let width = self.lines.iter().map(|line| line.width).fold(0.0, f32::max);
        let height = self.lines.iter().map(|line| line.height).sum();
        InlineLayout {
            lines: self.lines,
            size: Size::new(width, height),
        }
    }
}

/// Where the baseline of a line holding nothing but the strut would stand,
/// below that line's top: the strut's ascent and half its leading.
///
/// Nothing makes a line box exist — that rule is above — so this is not a
/// line. It answers for a box that stands on the line it *would* hold, which
/// is what a one-line field with nothing typed in it does, so that it does not
/// move when something is.
pub fn empty_line_baseline(strut: &TextStyle, measurer: &impl MeasureText) -> f32 {
    reach_of(strut, measurer).above
}

/// How far text in this style takes room above and below its baseline.
///
/// With `normal`, as far as its font reaches. With a line height that was
/// set, half of the difference between the two is added on each side — or
/// taken away, when the line height is the smaller.
fn reach_of(style: &TextStyle, measurer: &impl MeasureText) -> Reach {
    let above = measurer.ascender(style);
    let below = measurer.descender(style);
    let half = style
        .line_height
        .filter(|height| height.is_finite())
        .map_or(0.0, |height| (height - (above + below)) / 2.0);
    Reach {
        above: above + half,
        below: below + half,
    }
}

/// What a font is to a box aligned against it.
fn font_of(style: &TextStyle, measurer: &impl MeasureText) -> Parent {
    Parent {
        ascent: measurer.ascender(style),
        descent: measurer.descender(style),
        x_height: measurer.x_height(style),
        font_size: style.size,
    }
}

/// Join pieces of the same box that ended up next to each other.
///
/// The breaker works a word at a time, so "one two" from one box arrives as
/// two pieces. On the page it is one: **a box gets one rectangle per line it
/// is on**, and that is the promise paint relies on to draw a background that
/// stops at the end of each line.
fn merge_adjacent(pending: Vec<Pending>) -> Vec<Pending> {
    let mut merged: Vec<Pending> = Vec::with_capacity(pending.len());
    for item in pending {
        let joins = merged.last().is_some_and(|last| {
            last.fragment.box_id == item.fragment.box_id
                && match (&last.fragment.text, &item.fragment.text) {
                    (Some(before), Some(after)) => before.end == after.start,
                    // Two atomic boxes are two boxes even side by side.
                    _ => false,
                }
        });
        if joins && let Some(last) = merged.last_mut() {
            last.fragment.rect.size.width = item.fragment.rect.right() - last.fragment.rect.left();
            last.fragment.rect.size.height = last
                .fragment
                .rect
                .size
                .height
                .max(item.fragment.rect.size.height);
            last.below_baseline = last.below_baseline.max(item.below_baseline);
            if let (Some(before), Some(after)) = (&mut last.fragment.text, &item.fragment.text) {
                before.end = after.end;
            }
            continue;
        }
        merged.push(item);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::BlockFont;

    fn box_id(index: usize) -> BoxId {
        BoxId::from_index_for_tests(index)
    }

    fn text(index: usize, text: &str) -> InlineItem {
        InlineItem::Text {
            box_id: box_id(index),
            text: text.to_owned(),
            style: TextStyle::default(),
        }
    }

    fn atomic(index: usize, width: f32, height: f32) -> InlineItem {
        InlineItem::Atomic {
            box_id: box_id(index),
            size: Size::new(width, height),
            margin: Edges::ZERO,
            baseline: height,
            align: LineAlign::Baseline,
        }
    }

    fn lines_of(layout: &InlineLayout) -> Vec<Vec<String>> {
        layout
            .lines
            .iter()
            .map(|line| {
                line.fragments
                    .iter()
                    .map(|fragment| {
                        format!("{}@{}", fragment.box_id.as_usize(), fragment.rect.left())
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn nothing_lays_out_to_nothing() {
        let layout = lay_out(&[], Some(100.0), &BlockFont);
        assert!(layout.lines.is_empty());
        assert_eq!(layout.size, Size::ZERO);
        assert!(layout.union_for(box_id(0)).is_none());
    }

    #[test]
    fn text_that_fits_is_one_line() {
        let layout = lay_out(&[text(1, "one two")], Some(1000.0), &BlockFont);
        assert_eq!(layout.lines.len(), 1);
        assert!((layout.size.height - 16.0).abs() < 0.001);
    }

    #[test]
    fn a_sentence_breaks_between_two_inline_boxes_rather_than_only_inside_one() {
        // "the " from one box, "quick brown " from another, "fox" from a third
        // — one sentence, and it must wrap wherever the words allow.
        let items = [text(1, "the "), text(2, "quick brown "), text(3, "fox")];
        let layout = lay_out(&items, Some(80.0), &BlockFont);

        assert!(layout.lines.len() > 1);
        let second_line_starts_mid_box = layout
            .lines
            .get(1)
            .is_some_and(|line| line.fragments.iter().any(|f| f.box_id == box_id(2)));
        assert!(
            second_line_starts_mid_box,
            "the middle box was broken across the two lines: {:?}",
            lines_of(&layout),
        );
    }

    fn line_break(index: usize) -> InlineItem {
        InlineItem::Break {
            box_id: box_id(index),
            style: TextStyle::default(),
        }
    }

    #[test]
    fn a_break_ends_a_line_that_was_not_full() {
        let items = [text(1, "one "), line_break(2), text(3, "two")];
        let layout = lay_out(&items, Some(1000.0), &BlockFont);
        assert_eq!(
            lines_of(&layout),
            [vec!["1@0", "2@24"], vec!["3@0"]],
            "the break stands where the text ended, not after its space",
        );
        assert!((layout.size.height - 32.0).abs() < 0.001);
        // It takes no room across the line, and it is as tall as its font.
        let the_break = layout.union_for(box_id(2)).expect("the break has a place");
        assert_eq!(the_break, Rect::new(24.0, 0.0, 0.0, 16.0));
    }

    #[test]
    fn two_breaks_in_a_row_leave_a_blank_line() {
        let items = [
            text(1, "one "),
            line_break(2),
            line_break(3),
            text(4, " two"),
        ];
        let layout = lay_out(&items, Some(1000.0), &BlockFont);
        assert_eq!(
            lines_of(&layout),
            [vec!["1@0", "2@24"], vec!["3@0"], vec!["4@0"]],
            "the space after a break is the start of a line, and takes no room",
        );
        let tops: Vec<f32> = layout.lines.iter().map(|line| line.top).collect();
        assert_eq!(tops, [0.0, 16.0, 32.0]);
    }

    #[test]
    fn a_break_with_nothing_after_it_starts_no_line() {
        let layout = lay_out(&[text(1, "one"), line_break(2)], Some(1000.0), &BlockFont);
        assert_eq!(layout.lines.len(), 1);
        assert!((layout.size.height - 16.0).abs() < 0.001);
    }

    #[test]
    fn a_break_alone_is_a_line_of_its_own() {
        // `<p><br></p>`: a paragraph one line tall, not an empty one.
        let layout = lay_out(&[line_break(1)], Some(1000.0), &BlockFont);
        assert_eq!(layout.lines.len(), 1);
        assert!((layout.size.height - 16.0).abs() < 0.001);
        // Inside an inline box with no edges, which alone would be no line.
        let items = [
            InlineItem::Open {
                box_id: box_id(1),
                edge: 0.0,
                style: TextStyle::default(),
                over: 0.0,
                under: 0.0,
                align: LineAlign::Baseline,
            },
            line_break(2),
            InlineItem::Close {
                box_id: box_id(1),
                edge: 0.0,
            },
        ];
        let layout = lay_out(&items, Some(1000.0), &BlockFont);
        assert_eq!(layout.lines.len(), 1);
    }

    #[test]
    fn a_break_makes_the_line_it_ends_as_tall_as_its_own_line_height() {
        let tall = TextStyle {
            line_height: Some(40.0),
            ..TextStyle::default()
        };
        let items = [
            text(1, "one"),
            InlineItem::Break {
                box_id: box_id(2),
                style: tall,
            },
            text(3, "two"),
        ];
        let layout = lay_out(&items, Some(1000.0), &BlockFont);
        let heights: Vec<f32> = layout.lines.iter().map(|line| line.height).collect();
        assert_eq!(heights, [40.0, 16.0], "the line it ends, and not the next");
        // Its leading is split above and below its font, as text's is.
        let the_break = layout.union_for(box_id(2)).expect("the break has a place");
        assert!((the_break.top() - 12.0).abs() < 0.001, "{the_break:?}");
    }

    #[test]
    fn a_break_ends_a_line_inside_an_inline_box_and_the_box_carries_on() {
        let items = [
            InlineItem::Open {
                box_id: box_id(1),
                edge: 0.0,
                style: TextStyle::default(),
                over: 0.0,
                under: 0.0,
                align: LineAlign::Baseline,
            },
            text(2, "one"),
            line_break(3),
            text(4, "two"),
            InlineItem::Close {
                box_id: box_id(1),
                edge: 0.0,
            },
        ];
        let layout = lay_out(&items, Some(1000.0), &BlockFont);
        assert_eq!(layout.lines.len(), 2);
        let pieces = layout
            .fragments()
            .filter(|fragment| fragment.box_id == box_id(1))
            .count();
        assert_eq!(pieces, 2, "one piece of the box on each line");
    }

    #[test]
    fn a_break_ends_a_line_even_where_lines_may_not_wrap() {
        let nowrap = TextStyle {
            white_space: alo_box::WhiteSpace::NoWrap,
            ..TextStyle::default()
        };
        let items = [
            InlineItem::Text {
                box_id: box_id(1),
                text: "one".to_owned(),
                style: nowrap.clone(),
            },
            line_break(2),
            InlineItem::Text {
                box_id: box_id(3),
                text: "two".to_owned(),
                style: nowrap,
            },
        ];
        assert_eq!(lay_out(&items, Some(1.0), &BlockFont).lines.len(), 2);
        assert_eq!(lay_out(&items, None, &BlockFont).lines.len(), 2);
    }

    #[test]
    fn a_box_that_wraps_has_a_rectangle_for_each_line_it_is_on() {
        let layout = lay_out(&[text(1, "one two three four")], Some(80.0), &BlockFont);
        let pieces: Vec<&Fragment> = layout
            .fragments()
            .filter(|fragment| fragment.box_id == box_id(1))
            .collect();

        assert!(pieces.len() > 1, "more than one line, more than one piece");
        let union = layout.union_for(box_id(1)).expect("a union");
        assert!(
            union.size.height > pieces.first().expect("a piece").rect.size.height,
            "and the union covers the gap between them, which is why paint uses the pieces",
        );
    }

    #[test]
    fn every_line_fits_inside_the_width_it_was_given() {
        let layout = lay_out(
            &[text(1, "the quick brown fox jumps over the lazy dog")],
            Some(80.0),
            &BlockFont,
        );
        assert!(layout.lines.len() > 1);
        for line in &layout.lines {
            assert!(line.width <= 80.001, "a line of {}", line.width);
        }
    }

    #[test]
    fn something_wider_than_the_whole_line_goes_on_it_anyway() {
        let layout = lay_out(&[text(1, "extraordinarily")], Some(10.0), &BlockFont);
        assert_eq!(layout.lines.len(), 1);
        assert!(
            layout.size.width > 10.0,
            "it overflows rather than vanishing"
        );
    }

    #[test]
    fn a_taller_thing_on_a_line_pushes_the_line_down_rather_than_the_others_up() {
        // A forty-pixel box beside sixteen-pixel text: the text's baseline is
        // the box's bottom, so the text moves down and the box does not move.
        let layout = lay_out(
            &[atomic(1, 20.0, 40.0), text(2, "x")],
            Some(1000.0),
            &BlockFont,
        );
        let line = layout.lines.first().expect("one line");

        assert!(
            (line.baseline - 40.0).abs() < 0.001,
            "the tallest thing sets the baseline",
        );
        assert!(
            (line.height - 44.0).abs() < 0.001,
            "and the line is taller than the box, because the text's descender \
             hangs below the baseline the box set",
        );

        let boxed = line.fragments.first().expect("the box");
        let written = line.fragments.get(1).expect("the text");
        assert!(
            boxed.rect.top().abs() < 0.001,
            "the box is at the top of the line",
        );
        assert!(
            written.rect.top() > boxed.rect.top(),
            "and the text sits down on the baseline: {} against {}",
            written.rect.top(),
            boxed.rect.top(),
        );
        assert!(
            (written.rect.bottom() - (line.baseline + BlockFont.descender(&TextStyle::default())))
                .abs()
                < 0.001,
            "with its descender hanging below the baseline",
        );
    }

    #[test]
    fn text_and_boxes_share_a_line_in_the_order_they_were_given() {
        let items = [text(1, "a "), atomic(2, 20.0, 16.0), text(3, " b")];
        let layout = lay_out(&items, Some(1000.0), &BlockFont);
        let placed: Vec<usize> = layout
            .fragments()
            .map(|fragment| fragment.box_id.as_usize())
            .collect();
        assert_eq!(placed, vec![1, 2, 3]);

        let lefts: Vec<f32> = layout.fragments().map(|f| f.rect.left()).collect();
        assert!(
            lefts.windows(2).all(|pair| pair[0] <= pair[1]),
            "and each starts where the one before it ended: {lefts:?}",
        );
    }

    #[test]
    fn an_atomic_box_moves_to_the_next_line_whole_rather_than_being_cut() {
        let items = [text(1, "aaaaaaaa"), atomic(2, 60.0, 16.0)];
        let layout = lay_out(&items, Some(80.0), &BlockFont);
        assert_eq!(layout.lines.len(), 2);
        assert_eq!(
            layout.lines.get(1).map(|line| line.fragments.len()),
            Some(1),
        );
    }

    #[test]
    fn with_no_width_everything_goes_on_one_line() {
        let items = [text(1, "the quick brown fox jumps over the lazy dog")];
        let layout = lay_out(&items, None, &BlockFont);
        assert_eq!(layout.lines.len(), 1);
        assert!(
            (layout.size.width - 43.0 * 8.0).abs() < 0.001,
            "forty-three characters at eight pixels each",
        );
    }

    #[test]
    fn lines_stack_downwards_without_overlapping() {
        let layout = lay_out(
            &[text(1, "one two three four five")],
            Some(60.0),
            &BlockFont,
        );
        let mut expected_top = 0.0;
        for line in &layout.lines {
            assert!((line.top - expected_top).abs() < 0.001);
            expected_top += line.height;
        }
        assert!((layout.size.height - expected_top).abs() < 0.001);
    }

    #[test]
    fn a_box_gets_one_piece_a_line_rather_than_one_a_word() {
        let layout = lay_out(&[text(1, "one two three four")], Some(60.0), &BlockFont);
        for line in &layout.lines {
            assert_eq!(
                line.fragments.len(),
                1,
                "one box on one line is one piece, however many words: {:?}",
                line.fragments,
            );
        }
        let pieces: Vec<&Fragment> = layout.fragments().collect();
        for pair in pieces.windows(2) {
            assert!(
                pair[1].rect.top() >= pair[0].rect.bottom() - 0.001,
                "and the pieces stack downwards",
            );
        }
    }

    #[test]
    fn two_boxes_side_by_side_stay_two_pieces() {
        let layout = lay_out(&[text(1, "a "), text(2, "b")], Some(1000.0), &BlockFont);
        assert_eq!(
            layout.lines.first().map(|line| line.fragments.len()),
            Some(2),
        );
    }

    #[test]
    fn an_atomic_box_is_aligned_with_the_line_it_is_on() {
        // A box alone on its line, and a box after text: the line is aligned
        // as one thing, so the box moves exactly as far as the line does.
        for (alignment, alone, after_text) in [
            (TextAlignment::Start, 0.0, 16.0),
            (TextAlignment::Center, 80.0, 72.0 + 16.0),
            (TextAlignment::End, 160.0, 144.0 + 16.0),
        ] {
            let layout = lay_out_aligned(
                &[atomic(1, 40.0, 20.0)],
                Some(200.0),
                alignment,
                &TextStyle::default(),
                &BlockFont,
            );
            assert_eq!(
                lines_of(&layout),
                vec![vec![format!("1@{alone}")]],
                "{alignment:?}"
            );

            let layout = lay_out_aligned(
                &[text(1, "ab"), atomic(2, 40.0, 20.0)],
                Some(200.0),
                alignment,
                &TextStyle::default(),
                &BlockFont,
            );
            let start = after_text - 16.0;
            assert_eq!(
                lines_of(&layout),
                vec![vec![format!("1@{start}"), format!("2@{after_text}")]],
                "{alignment:?}",
            );
        }
    }

    #[test]
    fn an_atomic_boxs_margin_box_is_what_sits_on_the_line() {
        // A 40×20 box with margins 6, 10, 8 and 4 between "ab" and "c": its
        // margin box is 54 wide and 34 tall, and with no line in it the
        // baseline is its bottom margin edge.
        let boxed = InlineItem::Atomic {
            box_id: box_id(2),
            size: Size::new(40.0, 20.0),
            margin: Edges {
                top: 6.0,
                right: 10.0,
                bottom: 8.0,
                left: 4.0,
            },
            baseline: 34.0,
            align: LineAlign::Baseline,
        };
        let layout = lay_out(
            &[text(1, "ab"), boxed.clone(), text(3, "c")],
            Some(1000.0),
            &BlockFont,
        );
        let line = layout.lines.first().expect("one line");
        assert_eq!(
            lines_of(&layout),
            vec![vec!["1@0".to_owned(), "2@20".to_owned(), "3@70".to_owned()]],
            "the left margin is before the box and the right one before \"c\"",
        );
        assert!(close(line.baseline, 34.0), "{}", line.baseline);
        assert!(
            close(line.height, 38.0),
            "the margin box above the baseline and the text's descender below \
             it: {}",
            line.height,
        );
        let placed = line.fragments.get(1).expect("the box").rect;
        assert_eq!(
            placed,
            Rect::new(20.0, 6.0, 40.0, 20.0),
            "the fragment is the border box, under its top margin",
        );

        // Alone on a line, the margin box is what is aligned: 54 in 200.
        for (alignment, left) in [
            (TextAlignment::Start, 4.0),
            (TextAlignment::Center, 73.0 + 4.0),
            (TextAlignment::End, 146.0 + 4.0),
        ] {
            let layout = lay_out_aligned(
                core::slice::from_ref(&boxed),
                Some(200.0),
                alignment,
                &TextStyle::default(),
                &BlockFont,
            );
            assert_eq!(
                lines_of(&layout),
                vec![vec![format!("2@{left}")]],
                "{alignment:?}"
            );
            assert!(close(layout.size.width, 54.0), "{alignment:?}");
            // The margin box above the baseline, and the strut's descent
            // below it: the line is not only as tall as the box.
            assert!(close(layout.size.height, 38.0), "{alignment:?}");
        }
    }

    #[test]
    fn an_atomic_box_and_its_margins_wrap_together() {
        // Sixty-four pixels of text, then a 10-wide box with 10 pixels of
        // margin either side: the box alone would fit in 80, its margin box
        // does not.
        let boxed = InlineItem::Atomic {
            box_id: box_id(2),
            size: Size::new(10.0, 16.0),
            margin: Edges {
                left: 10.0,
                right: 10.0,
                ..Edges::ZERO
            },
            baseline: 16.0,
            align: LineAlign::Baseline,
        };
        let layout = lay_out(&[text(1, "aaaaaaaa"), boxed], Some(80.0), &BlockFont);
        assert_eq!(
            lines_of(&layout),
            vec![vec!["1@0".to_owned()], vec!["2@10".to_owned()]],
        );
    }

    #[test]
    fn a_line_of_only_a_picture_has_the_fonts_descent_below_it() {
        // A 20-pixel box alone in a 16-pixel block: the box's bottom edge is
        // the baseline, and the strut's 4-pixel descent hangs below it.
        let layout = lay_out(&[atomic(1, 20.0, 20.0)], Some(200.0), &BlockFont);
        let line = layout.lines.first().expect("one line");
        assert!(close(line.baseline, 20.0), "{}", line.baseline);
        assert!(close(line.height, 24.0), "{}", line.height);
        assert_eq!(
            line.fragments.first().map(|fragment| fragment.rect),
            Some(Rect::new(0.0, 0.0, 20.0, 20.0)),
            "the box sits at the top of its line, on the baseline",
        );

        // A box taller than the font still sets the ascent; a box shorter
        // than it does not, and the line is the strut's 16.
        let layout = lay_out(&[atomic(1, 20.0, 6.0)], Some(200.0), &BlockFont);
        let line = layout.lines.first().expect("one line");
        assert!(close(line.baseline, 12.0), "{}", line.baseline);
        assert!(close(line.height, 16.0), "{}", line.height);
        assert_eq!(
            line.fragments.first().map(|fragment| fragment.rect),
            Some(Rect::new(0.0, 6.0, 20.0, 6.0)),
        );
    }

    #[test]
    fn a_line_of_small_text_in_a_large_block_is_as_tall_as_the_large_font() {
        use crate::measure::ScaledFont;
        let small = InlineItem::Text {
            box_id: box_id(1),
            text: "ab".to_owned(),
            style: TextStyle {
                size: 10.0,
                ..TextStyle::default()
            },
        };
        let large = TextStyle {
            size: 40.0,
            ..TextStyle::default()
        };
        let layout = lay_out_aligned(
            core::slice::from_ref(&small),
            Some(200.0),
            TextAlignment::Start,
            &large,
            &ScaledFont,
        );
        let line = layout.lines.first().expect("one line");
        assert!(close(line.baseline, 30.0), "{}", line.baseline);
        assert!(close(line.height, 40.0), "{}", line.height);
        assert_eq!(
            line.fragments.first().map(|fragment| fragment.rect),
            Some(Rect::new(0.0, 22.5, 10.0, 10.0)),
            "the small text stands on the large font's baseline",
        );

        // Every line, not only the first: two lines are 80.
        let two = InlineItem::Text {
            box_id: box_id(1),
            text: "ab cd".to_owned(),
            style: TextStyle {
                size: 10.0,
                ..TextStyle::default()
            },
        };
        let layout = lay_out_aligned(
            &[two],
            Some(12.0),
            TextAlignment::Start,
            &large,
            &ScaledFont,
        );
        assert_eq!(layout.lines.len(), 2);
        assert!(close(layout.size.height, 80.0), "{}", layout.size.height);
    }

    #[test]
    fn the_strut_does_not_make_an_empty_line_exist() {
        let large = TextStyle {
            size: 40.0,
            ..TextStyle::default()
        };
        // A bracket with no edges, and a lone space: nothing worth a line.
        for items in [
            vec![
                InlineItem::Open {
                    box_id: box_id(1),
                    edge: 0.0,
                    style: TextStyle::default(),
                    over: 0.0,
                    under: 0.0,
                    align: LineAlign::Baseline,
                },
                InlineItem::Close {
                    box_id: box_id(1),
                    edge: 0.0,
                },
            ],
            vec![text(1, " ")],
            vec![],
        ] {
            let layout = lay_out_aligned(
                &items,
                Some(200.0),
                TextAlignment::Start,
                &large,
                &BlockFont,
            );
            assert!(layout.lines.is_empty(), "{items:?}");
            assert_eq!(layout.size, Size::ZERO, "{items:?}");
        }
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.001
    }

    fn aligned(index: usize, width: f32, height: f32, align: LineAlign) -> InlineItem {
        InlineItem::Atomic {
            box_id: box_id(index),
            size: Size::new(width, height),
            margin: Edges::ZERO,
            baseline: height,
            align,
        }
    }

    fn rect_of(layout: &InlineLayout, index: usize) -> Option<Rect> {
        layout
            .fragments()
            .find(|fragment| fragment.box_id == box_id(index))
            .map(|fragment| fragment.rect)
    }

    #[test]
    fn a_box_held_by_the_top_grows_the_line_downwards() {
        // A 40-pixel box against the top beside text of 12 up and 4 down:
        // the text keeps its baseline at 12, and the line is the box's 40.
        let layout = lay_out(
            &[text(1, "x"), aligned(2, 10.0, 40.0, LineAlign::Top)],
            Some(200.0),
            &BlockFont,
        );
        let line = layout.lines.first().expect("one line");
        assert!(close(line.baseline, 12.0), "{}", line.baseline);
        assert!(close(line.height, 40.0), "{}", line.height);
        assert_eq!(rect_of(&layout, 2), Some(Rect::new(8.0, 0.0, 10.0, 40.0)));
        assert_eq!(rect_of(&layout, 1), Some(Rect::new(0.0, 0.0, 8.0, 16.0)));
    }

    #[test]
    fn a_box_held_by_the_bottom_grows_the_line_upwards() {
        let layout = lay_out(
            &[text(1, "x"), aligned(2, 10.0, 40.0, LineAlign::Bottom)],
            Some(200.0),
            &BlockFont,
        );
        let line = layout.lines.first().expect("one line");
        assert!(close(line.baseline, 36.0), "{}", line.baseline);
        assert!(close(line.height, 40.0), "{}", line.height);
        assert_eq!(rect_of(&layout, 2), Some(Rect::new(8.0, 0.0, 10.0, 40.0)));
        assert_eq!(rect_of(&layout, 1), Some(Rect::new(0.0, 24.0, 8.0, 16.0)));

        // A short one sits at the bottom of a line something else made tall.
        let layout = lay_out(
            &[
                atomic(1, 10.0, 30.0),
                aligned(2, 10.0, 6.0, LineAlign::Bottom),
            ],
            Some(200.0),
            &BlockFont,
        );
        let line = layout.lines.first().expect("one line");
        assert!(close(line.height, 34.0), "{}", line.height);
        assert_eq!(rect_of(&layout, 2), Some(Rect::new(10.0, 28.0, 10.0, 6.0)));
    }

    #[test]
    fn a_top_box_and_a_bottom_box_on_one_line_do_not_push_each_other_off_it() {
        let layout = lay_out(
            &[
                aligned(1, 10.0, 30.0, LineAlign::Top),
                aligned(2, 10.0, 50.0, LineAlign::Bottom),
            ],
            Some(200.0),
            &BlockFont,
        );
        let line = layout.lines.first().expect("one line");
        assert!(close(line.height, 50.0), "{}", line.height);
        assert_eq!(rect_of(&layout, 1), Some(Rect::new(0.0, 0.0, 10.0, 30.0)));
        assert_eq!(rect_of(&layout, 2), Some(Rect::new(10.0, 0.0, 10.0, 50.0)));
    }

    #[test]
    fn an_inline_box_held_by_the_top_is_held_there_on_every_line_it_reaches() {
        use crate::measure::ScaledFont;
        // A 40-pixel atomic box makes the first line tall; a 10-pixel `<span>`
        // held by the top wraps on to a second line, where it is held by that
        // line's top instead.
        let small = TextStyle {
            size: 10.0,
            ..TextStyle::default()
        };
        let items = [
            atomic(1, 20.0, 40.0),
            InlineItem::Open {
                box_id: box_id(2),
                edge: 0.0,
                style: small.clone(),
                over: 0.0,
                under: 0.0,
                align: LineAlign::Top,
            },
            InlineItem::Text {
                box_id: box_id(3),
                text: "aaaa bbbb".to_owned(),
                style: small,
            },
            InlineItem::Close {
                box_id: box_id(2),
                edge: 0.0,
            },
        ];
        let layout = lay_out(&items, Some(50.0), &ScaledFont);
        assert_eq!(layout.lines.len(), 2);
        let pieces: Vec<Rect> = layout
            .fragments()
            .filter(|fragment| fragment.box_id == box_id(3))
            .map(|fragment| fragment.rect)
            .collect();
        let tops: Vec<f32> = layout.lines.iter().map(|line| line.top).collect();
        assert_eq!(
            pieces,
            vec![
                Rect::new(20.0, 0.0, 20.0, 10.0),
                Rect::new(0.0, tops.get(1).copied().unwrap_or(f32::NAN), 20.0, 10.0),
            ],
            "at the top of each line it is on",
        );
    }

    #[test]
    fn what_was_raised_makes_room_for_itself_above_the_line() {
        // Raised 10 pixels, a 20-pixel box reaches 30 over the baseline.
        let layout = lay_out(
            &[text(1, "x"), aligned(2, 10.0, 20.0, LineAlign::Raise(10.0))],
            Some(200.0),
            &BlockFont,
        );
        let line = layout.lines.first().expect("one line");
        assert!(close(line.baseline, 30.0), "{}", line.baseline);
        assert!(close(line.height, 34.0), "{}", line.height);
        assert_eq!(rect_of(&layout, 2), Some(Rect::new(8.0, 0.0, 10.0, 20.0)));

        // Lowered 10, its bottom is 10 under the baseline, below the strut's
        // descent of 4.
        let layout = lay_out(
            &[
                text(1, "x"),
                aligned(2, 10.0, 20.0, LineAlign::Raise(-10.0)),
            ],
            Some(200.0),
            &BlockFont,
        );
        let line = layout.lines.first().expect("one line");
        assert!(close(line.baseline, 12.0), "{}", line.baseline);
        assert!(close(line.height, 22.0), "{}", line.height);
        assert_eq!(rect_of(&layout, 2), Some(Rect::new(8.0, 2.0, 10.0, 20.0)));
    }

    #[test]
    fn a_fragment_names_the_bytes_of_the_text_it_drew() {
        let text_of = "one two";
        let layout = lay_out(&[text(1, text_of)], Some(1000.0), &BlockFont);
        for fragment in layout.fragments() {
            let range = fragment.text.clone().expect("a text fragment");
            assert!(
                text_of.get(range).is_some(),
                "the range names real bytes of the text",
            );
        }
    }
}
