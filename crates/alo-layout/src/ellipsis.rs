/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `text-overflow: ellipsis`: a line that runs past the end of a box that
//! clips it ends in "…" inside the box (CSS Overflow 4 § 6.1).
//!
//! **Nothing moves.** The boxes, the lines and every fragment are where they
//! were; Chrome's are the same, and an agent reads the whole text, because the
//! page wrote it (queue item 442). What changes is what is *drawn*: the end of
//! the line is hidden, whole clusters and whole atomic boxes at a time, until
//! what is left and the "…" fit inside the box's content edge, and the "…" is
//! drawn straight after what is left. So this records three things for paint
//! — where each "…" is, how much of each piece of text is drawn, and which
//! atomic boxes are not drawn at all — and changes no number layout already
//! had.
//!
//! # What Chrome 154 does, measured
//!
//! Every rule here was measured on headless Chrome 154 with `DejaVu Sans`
//! (iteration 284) rather than read from its source:
//!
//! - Only a box whose `overflow` is not `visible` draws one: `hidden`,
//!   `clip`, `auto` and `scroll` all do, and on either axis — even
//!   `overflow-y: clip` alone, which leaves `overflow-x` at `visible` by CSS
//!   Overflow 3's own rule.
//! - Only a block container's lines are ended: a run of text beside a block
//!   is, as an anonymous box of the element around it; a flex container's
//!   text is a flex item's, and is not.
//! - Only a line that overflows is ended; in a box whose lines wrap, the one
//!   long word that sticks out is, and its neighbours are not.
//! - Whole clusters are hidden from the end, and a space before the cut stays
//!   — `aaaa bbbbbbbbb` in 61 px is `aaaa …`. The "…" stands straight after
//!   the last glyph kept, not after an inline box's padding beside it.
//! - A decoration stops where what is kept ends: no line is drawn under a
//!   space there, whether it ends a piece of text or is a text box of its
//!   own (iteration 345). A space with no letters before that is decorated;
//!   one there or after it is recorded as hidden (queue item 582). A kept
//!   piece of text's own last space is still drawn under here (queue item
//!   583).
//! - An atomic box that does not fit whole, with its margins, is hidden whole.
//! - An inline box's own background and border are not hidden: they are the
//!   box's, not the text's, and the box's clip cuts them as before.
//! - The "…" is set in the block's font and colour, on the line's baseline,
//!   whatever the text it follows is set in.
//! - The first thing on a line is never hidden: a box too narrow for one
//!   letter and the "…" keeps the letter and the box's clip cuts the "…".
//! - A string, and two values, are refused by Chrome as it parses; here they
//!   are refused as they are read (`crate::keyword::TextOverflow`).
//!
//! # What is not here
//!
//! - **A space that is a text box of its own** — the one between `</a>` and
//!   `<a>` — takes room on its line without a fragment, so the "…" after it
//!   stands where the text before it ended. Chrome's stands after the space.
//! - **Right to left.** The end of a line is its right edge here, because
//!   lines are laid down left to right until bidirectional text is built
//!   (stage 2), and a right-to-left run is one cluster (`alo_text`'s
//!   `cluster.rs`).

use crate::engine::Placed;
use crate::geometry::{Point, Rect};
use crate::inline::{InlineItem, InlineLayout, LineBox};
use crate::keyword::{Overflow, TextOverflow};
use crate::measure::{MeasureText, TextStyle};
use alo_box::{BoxId, BoxKind, BoxTree, Inside};
use alo_style::StyleTree;
use std::collections::{BTreeMap, BTreeSet};

/// What is drawn in place of the end of a line.
pub const ELLIPSIS: &str = "\u{2026}";

/// How far past the edge a line may reach and still fit: the same allowance a
/// line gives a word when it decides whether the word fits.
const ALLOWANCE: f32 = 0.001;

/// The "…" one line ends in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ellipsis {
    /// Which of the box's lines it ends, counting from zero.
    pub line: usize,
    /// Where it is drawn: as wide as it is, and as tall as the block's font,
    /// the font's ascent above the line's baseline — as a piece of text's
    /// fragment is.
    pub rect: Rect,
}

/// How much of one text box's piece on one line is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cut {
    /// Which line the piece is on.
    pub line: usize,
    /// The byte of the box's text where what is drawn stops. The piece's
    /// own start when none of it is.
    pub end: usize,
    /// How wide what is drawn is.
    pub width: f32,
}

/// Everything ellipses hid in one layout, and where they are.
#[derive(Debug, Clone, Default)]
pub(crate) struct Ellipses {
    /// Each block's, by the block, **relative to the top left of its border
    /// box**: a block is moved after its lines are placed when it is inside
    /// an atomic box placed in a line or a box out of flow put at its static
    /// position, and what is relative to it moves with it.
    pub(crate) by_block: BTreeMap<BoxId, Vec<Ellipsis>>,
    /// Each text box's pieces drawn short.
    pub(crate) cuts: BTreeMap<BoxId, Vec<Cut>>,
    /// The atomic boxes not drawn at all.
    pub(crate) hidden: BTreeSet<BoxId>,
}

impl Ellipses {
    /// Take in another layout's, an atomic box's own.
    pub(crate) fn extend(&mut self, other: Ellipses) {
        self.by_block.extend(other.by_block);
        self.cuts.extend(other.cuts);
        self.hidden.extend(other.hidden);
    }
}

/// What ending the lines of one formatting context hid.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Truncated {
    /// Each ended line's "…", where the context's lines are measured from.
    pub ellipses: Vec<Ellipsis>,
    /// Each piece of text drawn short, by its box.
    pub cuts: Vec<(BoxId, Cut)>,
    /// The atomic boxes not drawn.
    pub hidden: Vec<BoxId>,
    /// The spaces with no letters where what is kept ends or after it, each
    /// by its box and the byte its text starts at
    /// ([`crate::inline::LineBox::spaces`]): nothing is drawn under them.
    pub hidden_spaces: Vec<(BoxId, usize)>,
}

/// End every line of `layout` wider than `room` in an ellipsis set in
/// `block`'s font.
///
/// `items` are what the lines were laid out from, for the text and style of
/// each text box and the margins of each atomic one.
pub fn truncate(
    items: &[InlineItem],
    layout: &InlineLayout,
    room: f32,
    block: &TextStyle,
    measurer: &impl MeasureText,
) -> Truncated {
    let mut truncated = Truncated::default();
    if !layout
        .lines
        .iter()
        .any(|line| line.width > room + ALLOWANCE)
    {
        return truncated;
    }
    let mut texts = BTreeMap::new();
    let mut atomics = BTreeMap::new();
    for item in items {
        match item {
            InlineItem::Text {
                box_id,
                text,
                style,
            } => {
                texts.insert(*box_id, (text.as_str(), style));
            }
            InlineItem::Atomic { box_id, margin, .. } => {
                atomics.insert(*box_id, margin.right);
            }
            _ => {}
        }
    }
    let width = measurer.measure(ELLIPSIS, block, None).width;
    let ascent = measurer.ascender(block);
    let descent = measurer.descender(block);
    let content = Content {
        texts: &texts,
        atomics: &atomics,
    };
    for (index, line) in layout.lines.iter().enumerate() {
        if line.width <= room + ALLOWANCE {
            continue;
        }
        let Some(end) = end_line(
            line,
            index,
            &content,
            room - width,
            measurer,
            &mut truncated,
        ) else {
            continue;
        };
        truncated.ellipses.push(Ellipsis {
            line: index,
            rect: Rect::new(
                end,
                line.top + line.baseline - ascent,
                width,
                ascent + descent,
            ),
        });
    }
    truncated
}

/// The text and the atomic boxes on a context's lines.
struct Content<'a> {
    /// Each text box's text and style.
    texts: &'a BTreeMap<BoxId, (&'a str, &'a TextStyle)>,
    /// Each atomic box's right margin.
    atomics: &'a BTreeMap<BoxId, f32>,
}

/// Hide the end of one line until what is left ends by `limit`, and say where
/// what is left ends — or [`None`] for a line with no text and no atomic box
/// on it, which has nothing to hide.
fn end_line(
    line: &LineBox,
    index: usize,
    content: &Content<'_>,
    limit: f32,
    measurer: &impl MeasureText,
    out: &mut Truncated,
) -> Option<f32> {
    // Where what is kept ends; [`None`] until something is.
    let mut end: Option<f32> = None;
    let mut cut = false;
    for fragment in &line.fragments {
        let left = fragment.rect.left();
        if let (Some(range), Some((text, style))) =
            (fragment.text.clone(), content.texts.get(&fragment.box_id))
        {
            if cut {
                out.cuts.push((
                    fragment.box_id,
                    Cut {
                        line: index,
                        end: range.start,
                        width: 0.0,
                    },
                ));
                continue;
            }
            let Some(piece) = text.get(range.clone()) else {
                continue;
            };
            let clusters = measurer.cluster_ends(piece, style);
            let fits = |pen: f32| left + pen <= limit + ALLOWANCE;
            let kept = clusters.iter().take_while(|(_, pen)| fits(*pen)).last();
            if kept.is_some_and(|(byte, _)| *byte >= piece.len()) {
                end = kept.map(|(_, pen)| left + pen);
                continue;
            }
            // The first thing on a line is kept and clipped rather than
            // hidden: an ellipsis alone says nothing about what was there.
            let kept = kept.or_else(|| clusters.first().filter(|_| end.is_none()));
            let (byte, pen) = kept.copied().unwrap_or((0, 0.0));
            out.cuts.push((
                fragment.box_id,
                Cut {
                    line: index,
                    end: range.start + byte,
                    width: pen,
                },
            ));
            if kept.is_some() {
                end = Some(left + pen);
            }
            cut = true;
        } else if let Some(margin) = content.atomics.get(&fragment.box_id) {
            if cut {
                out.hidden.push(fragment.box_id);
                continue;
            }
            let right = fragment.rect.right() + margin;
            if right <= limit + ALLOWANCE || end.is_none() {
                end = Some(right);
            } else {
                out.hidden.push(fragment.box_id);
            }
            cut = right > limit + ALLOWANCE;
        }
    }
    let end = end.or(cut.then_some(0.0));
    // A space with no letters where what is kept ends, or after it, is not
    // decorated: Chrome 154 puts the "…" after such a space and draws no
    // line under it, as it draws none under the space a kept piece of text
    // ends in there (measured, iteration 345).
    if let Some(kept) = end.filter(|_| cut) {
        for space in &line.spaces {
            if let Some(range) = space
                .text
                .as_ref()
                .filter(|_| space.rect.left() >= kept - ALLOWANCE)
            {
                out.hidden_spaces.push((space.box_id, range.start));
            }
        }
    }
    end
}

/// One formatting context's lines, as the engine placed them.
pub(crate) struct Lines<'a> {
    /// What they were laid out from.
    pub(crate) items: &'a [InlineItem],
    /// The lines.
    pub(crate) layout: &'a InlineLayout,
    /// Where they start, on the page.
    pub(crate) origin: Point,
    /// The font of the box that holds them, which a "…" is set in.
    pub(crate) strut: &'a TextStyle,
}

/// End the lines of the formatting context `id`, if its style says to.
pub(crate) fn place(
    boxes: &BoxTree,
    styles: &StyleTree,
    id: BoxId,
    lines: &Lines<'_>,
    measure: &impl MeasureText,
    out: &mut Placed,
) {
    let Lines {
        items,
        layout,
        origin,
        strut,
    } = *lines;
    let Some(container) = out.geometry.get(&id).copied() else {
        return;
    };
    if !ends_its_lines(boxes, styles, id) {
        return;
    }
    let room = container.content_box().size.width;
    let truncated = truncate(items, layout, room, strut, measure);
    if truncated.ellipses.is_empty() {
        return;
    }
    // From where the lines start to where the block's border box does.
    let corner = container.border_box.origin;
    let by = Point::new(origin.x - corner.x, origin.y - corner.y);
    out.ellipses.by_block.insert(
        id,
        truncated
            .ellipses
            .into_iter()
            .map(|ellipsis| Ellipsis {
                rect: ellipsis.rect.translated(by),
                ..ellipsis
            })
            .collect(),
    );
    for (box_id, cut) in truncated.cuts {
        out.ellipses.cuts.entry(box_id).or_default().push(cut);
    }
    out.ellipses.hidden.extend(truncated.hidden);
    for (box_id, start) in truncated.hidden_spaces {
        if let Some(spaces) = out.spaces.get_mut(&box_id) {
            spaces.retain(|space| space.text.as_ref().is_none_or(|range| range.start != start));
        }
    }
}

/// Whether the lines of `id` end in an ellipsis where they overflow:
/// `text-overflow: ellipsis` on a block container whose `overflow` is not
/// `visible` on either axis, as Chrome 154 was measured to decide.
///
/// The lines of an anonymous box are the element's around it — the run of
/// text beside a block in a `<div>`, the inside of a field — when that element
/// is a block container; the property is not inherited, and an anonymous box
/// has no style of its own to have been given it.
fn ends_its_lines(boxes: &BoxTree, styles: &StyleTree, id: BoxId) -> bool {
    let Some(node) = boxes.get(id) else {
        return false;
    };
    let owner = match node.kind {
        BoxKind::Element { .. } => Some(node),
        BoxKind::Anonymous { .. } => node.parent.and_then(|parent| boxes.get(parent)),
        BoxKind::Text { .. } => None,
    };
    // `flow-root` is a block container too: an `inline-block`'s inside.
    let Some(owner) =
        owner.filter(|owner| matches!(owner.kind.inside(), Inside::Flow | Inside::FlowRoot))
    else {
        return false;
    };
    let Some(style) = owner.kind.node().and_then(|element| styles.get(element)) else {
        return false;
    };
    // A value this engine cannot read was recorded when the box's style was
    // read for its own layout; reading it again here is not a second finding.
    let ours = crate::style::read(style, &mut Vec::new());
    ours.text_overflow == TextOverflow::Ellipsis
        && (ours.overflow.horizontal != Overflow::Visible
            || ours.overflow.vertical != Overflow::Visible)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Edges, Size};
    use crate::inline::{TextAlignment, lay_out, lay_out_aligned};
    use crate::measure::{BlockFont, ScaledFont};
    use crate::vertical_align::LineAlign;
    use alo_box::WhiteSpace;

    fn box_id(index: usize) -> BoxId {
        BoxId::from_index_for_tests(index)
    }

    fn one_line() -> TextStyle {
        TextStyle {
            white_space: WhiteSpace::NoWrap,
            ..TextStyle::default()
        }
    }

    fn text(index: usize, text: &str) -> InlineItem {
        InlineItem::Text {
            box_id: box_id(index),
            text: text.to_owned(),
            style: one_line(),
        }
    }

    fn atomic(index: usize, width: f32, margin: f32) -> InlineItem {
        InlineItem::Atomic {
            box_id: box_id(index),
            size: Size::new(width, 16.0),
            margin: Edges {
                left: margin,
                right: margin,
                ..Edges::ZERO
            },
            baseline: 16.0,
            align: LineAlign::Baseline,
        }
    }

    /// Lay `items` out in `room`, in a block that does not wrap as the text
    /// in it does not, and end its lines, with the test font's eight pixels
    /// a character: "…" is eight wide.
    fn ended(items: &[InlineItem], room: f32) -> Truncated {
        let layout = lay_out_aligned(
            items,
            Some(room),
            TextAlignment::Start,
            &one_line(),
            &BlockFont,
        );
        truncate(items, &layout, room, &one_line(), &BlockFont)
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.001
    }

    #[test]
    fn a_line_that_fits_is_left_alone() {
        assert_eq!(ended(&[text(1, "abcd")], 32.0), Truncated::default());
        assert_eq!(ended(&[text(1, "abcd")], 100.0), Truncated::default());
    }

    #[test]
    fn a_long_line_keeps_what_fits_beside_the_ellipsis() {
        // Ten characters, eighty pixels, in forty: four characters and the
        // ellipsis are forty, so "abcd…".
        let truncated = ended(&[text(1, "abcdefghij")], 40.0);
        assert_eq!(
            truncated.cuts,
            vec![(
                box_id(1),
                Cut {
                    line: 0,
                    end: 4,
                    width: 32.0
                }
            )],
        );
        let [ellipsis] = truncated.ellipses.as_slice() else {
            panic!("one ellipsis: {truncated:?}");
        };
        assert_eq!(ellipsis.line, 0);
        assert_eq!(ellipsis.rect, Rect::new(32.0, 0.0, 8.0, 16.0));
    }

    #[test]
    fn a_space_before_the_cut_is_kept() {
        // "abc defghij" in 48: "abc " is 32, and "abc d" and the ellipsis
        // would be 48 — which fits exactly, so the cut falls after the `d`.
        // In 47 it falls after the space, and the ellipsis stands after it,
        // as Chrome's "aaaa …" does.
        let truncated = ended(&[text(1, "abc defghij")], 47.0);
        assert_eq!(
            truncated.cuts,
            vec![(
                box_id(1),
                Cut {
                    line: 0,
                    end: 4,
                    width: 32.0
                }
            )],
        );
        assert!(close(truncated.ellipses[0].rect.left(), 32.0));
    }

    #[test]
    fn everything_after_the_cut_is_hidden() {
        // "abcdef" and a box of 10 after "ab", in 40: the box would end at
        // 26 and the ellipsis has to start by 32, so it stays; the cut falls
        // in "cdefgh" after "cd", and the text after that is hidden.
        let truncated = ended(
            &[
                text(1, "ab"),
                atomic(2, 10.0, 0.0),
                text(3, "cdefgh"),
                text(4, "ijk"),
            ],
            40.0,
        );
        assert!(truncated.hidden.is_empty());
        assert_eq!(
            truncated.cuts,
            vec![
                (
                    box_id(3),
                    Cut {
                        line: 0,
                        end: 0,
                        width: 0.0
                    }
                ),
                (
                    box_id(4),
                    Cut {
                        line: 0,
                        end: 0,
                        width: 0.0
                    }
                ),
            ],
            "{truncated:?}",
        );
        assert!(close(truncated.ellipses[0].rect.left(), 26.0));
        // In 30 the box itself is past the cut, hidden, and everything after.
        let truncated = ended(
            &[text(1, "ab"), atomic(2, 10.0, 0.0), text(3, "cdefgh")],
            30.0,
        );
        assert_eq!(truncated.hidden, vec![box_id(2)]);
        assert_eq!(
            truncated.cuts,
            vec![(
                box_id(3),
                Cut {
                    line: 0,
                    end: 0,
                    width: 0.0
                }
            )],
        );
        assert!(close(truncated.ellipses[0].rect.left(), 16.0));
    }

    #[test]
    fn a_space_of_its_own_where_what_is_kept_ends_is_hidden_and_one_before_is_not() {
        // `<a>ab</a> <a>cd</a> <a>efgh</a>`, eight a character: the spaces
        // run 16..24 and 40..48. In 48 the "…" must start by 40, so `cd` is
        // kept whole, `efgh` is hidden, and the space after `cd`, where what
        // is kept ends, is hidden with it; the one before `cd` is not.
        let items = [
            text(1, "ab"),
            text(2, " "),
            text(3, "cd"),
            text(4, " "),
            text(5, "efgh"),
        ];
        let truncated = ended(&items, 48.0);
        assert_eq!(truncated.hidden_spaces, vec![(box_id(4), 0)]);
        assert!(close(truncated.ellipses[0].rect.left(), 40.0));
        // In 40 the cut falls inside `cd`, after `c`: the space before it is
        // still kept, and the one after it hidden.
        let truncated = ended(&items, 40.0);
        assert_eq!(truncated.hidden_spaces, vec![(box_id(4), 0)]);
        assert!(close(truncated.ellipses[0].rect.left(), 32.0));
        // A line that fits hides nothing.
        assert!(ended(&items, 80.0).hidden_spaces.is_empty());
    }

    #[test]
    fn an_atomic_box_that_does_not_fit_with_its_margins_is_hidden_whole() {
        // "ab" is 16, then a box of 10 with 3 either side reaching 32. In 36
        // the ellipsis needs it to end by 28: hidden, and the ellipsis stands
        // after "ab".
        let truncated = ended(&[text(1, "ab"), atomic(2, 10.0, 3.0), text(3, "cd")], 36.0);
        assert_eq!(truncated.hidden, vec![box_id(2)]);
        assert!(close(truncated.ellipses[0].rect.left(), 16.0));
        // In 44 it ends by 36 and stays, and the ellipsis stands after its
        // margin.
        let truncated = ended(
            &[text(1, "ab"), atomic(2, 10.0, 3.0), text(3, "cdef")],
            44.0,
        );
        assert!(truncated.hidden.is_empty());
        assert!(close(truncated.ellipses[0].rect.left(), 32.0));
    }

    #[test]
    fn the_first_thing_on_a_line_is_kept_however_narrow_the_box() {
        // Eight pixels: not even one character and the ellipsis fit. The
        // first character stays, and the ellipsis after it is the box's clip's.
        let truncated = ended(&[text(1, "abc")], 8.0);
        assert_eq!(
            truncated.cuts,
            vec![(
                box_id(1),
                Cut {
                    line: 0,
                    end: 1,
                    width: 8.0
                }
            )],
        );
        assert!(close(truncated.ellipses[0].rect.left(), 8.0));
        // An atomic box first is kept whole the same way.
        let truncated = ended(&[atomic(1, 30.0, 0.0), text(2, "abc")], 20.0);
        assert!(truncated.hidden.is_empty());
        assert!(close(truncated.ellipses[0].rect.left(), 30.0));
    }

    #[test]
    fn only_the_lines_that_overflow_are_ended() {
        // Wrapping text: "aa" fits, the long word does not and is ended, and
        // "bb" fits on the line after.
        let wraps = TextStyle::default();
        let items = [InlineItem::Text {
            box_id: box_id(1),
            text: "aa supercalifragilistic bb".to_owned(),
            style: wraps.clone(),
        }];
        let layout = lay_out(&items, Some(56.0), &BlockFont);
        assert_eq!(layout.lines.len(), 3);
        let truncated = truncate(&items, &layout, 56.0, &wraps, &BlockFont);
        let lines: Vec<usize> = truncated.ellipses.iter().map(|held| held.line).collect();
        assert_eq!(lines, vec![1]);
        // "superc" is 48 and the ellipsis 8.
        assert_eq!(
            truncated.cuts,
            vec![(
                box_id(1),
                Cut {
                    line: 1,
                    end: 9,
                    width: 48.0
                }
            )],
        );
        assert!(
            close(truncated.ellipses[0].rect.top(), 16.0),
            "the second line"
        );
    }

    #[test]
    fn the_ellipsis_is_set_in_the_blocks_font_on_the_lines_baseline() {
        // A 40-pixel block holding 10-pixel text: the ellipsis is 20 wide
        // and 40 tall, its top the block's ascent above the baseline.
        let small = TextStyle {
            size: 10.0,
            ..one_line()
        };
        let block = TextStyle {
            size: 40.0,
            ..one_line()
        };
        let items = [InlineItem::Text {
            box_id: box_id(1),
            text: "abcdefghijklmnop".to_owned(),
            style: small,
        }];
        let layout = crate::inline::lay_out_aligned(
            &items,
            Some(50.0),
            crate::inline::TextAlignment::Start,
            &block,
            &ScaledFont,
        );
        let truncated = truncate(&items, &layout, 50.0, &block, &ScaledFont);
        let [ellipsis] = truncated.ellipses.as_slice() else {
            panic!("one ellipsis: {truncated:?}");
        };
        // Five-pixel characters ending by 30: six of them.
        assert!(close(ellipsis.rect.left(), 30.0));
        assert!(close(ellipsis.rect.size.width, 20.0));
        assert!(close(ellipsis.rect.size.height, 40.0));
        let line = &layout.lines[0];
        assert!(close(ellipsis.rect.top(), line.top + line.baseline - 30.0));
    }

    #[test]
    fn a_hostile_line_is_ended_in_one_pass() {
        // A hundred thousand characters on one line in a narrow box: one
        // cluster list, not one measurement per character.
        let long = "a".repeat(100_000);
        let truncated = ended(&[text(1, &long)], 40.0);
        assert_eq!(truncated.cuts[0].1.end, 4);
        // And a box with no room at all keeps its first character.
        let truncated = ended(&[text(1, &long)], 0.0);
        assert_eq!(truncated.cuts[0].1.end, 1);
    }

    #[test]
    fn a_line_with_nothing_to_hide_has_no_ellipsis() {
        let items = [
            InlineItem::Open {
                box_id: box_id(1),
                edge: 50.0,
                style: one_line(),
                over: 0.0,
                under: 0.0,
                align: LineAlign::Baseline,
            },
            InlineItem::Close {
                box_id: box_id(1),
                edge: 50.0,
            },
        ];
        assert_eq!(ended(&items, 40.0), Truncated::default());
    }
}
