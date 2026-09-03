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

use crate::geometry::{Point, Rect, Size};
use crate::measure::{MeasureText, TextStyle};
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
    },
    /// The end of a nested inline box.
    Close {
        /// The box.
        box_id: BoxId,
        /// The room its own end border and padding take on the line.
        edge: f32,
    },
    /// A box with a size of its own, which is placed whole or not at all.
    Atomic {
        /// The box.
        box_id: BoxId,
        /// How big it is.
        size: Size,
        /// How far above its bottom edge its baseline sits.
        ///
        /// For a box with text in it this is the last line's baseline; for one
        /// without, CSS uses the bottom margin edge, which is a baseline of
        /// zero.
        baseline: f32,
    },
}

impl InlineItem {
    /// The box this item belongs to.
    pub fn box_id(&self) -> BoxId {
        match self {
            InlineItem::Text { box_id, .. }
            | InlineItem::Atomic { box_id, .. }
            | InlineItem::Open { box_id, .. }
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

/// Lay items into lines no wider than `available_width`.
///
/// `available_width` of [`None`] is the max-content question — how wide it
/// would like to be — and puts everything on one line.
pub fn lay_out(
    items: &[InlineItem],
    available_width: Option<f32>,
    measurer: &impl MeasureText,
) -> InlineLayout {
    lay_out_aligned(items, available_width, TextAlignment::Start, measurer)
}

/// The same, with the lines sitting where `text-align` says.
pub fn lay_out_aligned(
    items: &[InlineItem],
    available_width: Option<f32>,
    alignment: TextAlignment,
    measurer: &impl MeasureText,
) -> InlineLayout {
    let mut builder = Builder::new(available_width, measurer);
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
                baseline,
            } => builder.add_atomic(*box_id, *size, *baseline),
            InlineItem::Open {
                box_id,
                edge,
                style,
                over,
                under,
            } => builder.open(*box_id, *edge, style, *over, *under),
            InlineItem::Close { box_id, edge } => builder.close(*box_id, *edge),
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
    /// How far its painted area reaches beyond that.
    over: f32,
    under: f32,
}

impl<'a, M: MeasureText> Builder<'a, M> {
    fn new(available_width: Option<f32>, measurer: &'a M) -> Self {
        Self {
            available_width,
            measurer,
            alignment: TextAlignment::Start,
            lines: Vec::new(),
            current: Vec::new(),
            open: Vec::new(),
            content: false,
            pen: 0.0,
            ascent: 0.0,
            descent: 0.0,
        }
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
        self.current.push(Pending {
            fragment: Fragment {
                box_id,
                rect: Rect::new(self.pen, 0.0, visible, ascent + descent),
                text: Some(range),
                line: self.lines.len(),
            },
            below_baseline: descent,
        });
        self.pen += placed;
        self.ascent = self.ascent.max(ascent);
        self.descent = self.descent.max(descent);
    }

    /// Start a nested inline box.
    ///
    /// Its start border and padding take room on the line here and nowhere
    /// else. Its **content area** — the font's ascent and descent — counts
    /// towards the line's height; its top and bottom border and padding do
    /// not, which is CSS's rule and the reason a padded `<em>` does not push a
    /// paragraph's lines apart.
    fn open(&mut self, box_id: BoxId, edge: f32, style: &TextStyle, over: f32, under: f32) {
        let ascent = self.measurer.ascender(style);
        let descent = self.measurer.descender(style);
        self.pen += edge;
        self.ascent = self.ascent.max(ascent);
        self.descent = self.descent.max(descent);
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
            over,
            under,
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
        });
    }

    fn add_atomic(&mut self, box_id: BoxId, size: Size, baseline: f32) {
        if !self.fits(size.width) {
            self.end_line();
        }
        self.content = true;
        self.current.push(Pending {
            fragment: Fragment {
                box_id,
                rect: Rect::new(self.pen, 0.0, size.width, size.height),
                text: None,
                line: self.lines.len(),
            },
            // An atomic box sits with its own baseline on the line's.
            below_baseline: size.height - baseline,
        });
        self.pen += size.width;
        self.ascent = self.ascent.max(baseline);
        self.descent = self.descent.max(size.height - baseline);
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
            self.pen = 0.0;
            self.ascent = 0.0;
            self.descent = 0.0;
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
        let baseline = self.ascent;
        let height = self.ascent + self.descent;
        let width = self
            .current
            .iter()
            .map(|pending| pending.fragment.rect.right())
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
                // pushing the others up.
                let above = pending.fragment.rect.size.height - pending.below_baseline;
                let y = top + baseline - above;
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
        self.pen = 0.0;
        self.ascent = 0.0;
        self.descent = 0.0;
        self.content = false;
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
            baseline: height,
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
