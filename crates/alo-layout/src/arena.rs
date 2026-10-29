/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The tree the layout algorithms walk, and the answers they ask it for.
//!
//! ADR 0004: **we own the tree, `taffy` owns the algorithms.** Flexbox, grid
//! and block sizing are thousands of lines of specification with decades of
//! interoperability in them, and ADR 0001 says to rent that. A list of nodes
//! with styles, children, a cache and a result is not that — it is storage, and
//! it is the one place a browser has to be able to answer its own questions.
//!
//! # The question that forced it
//!
//! `width: calc(100% - 2rem)`. `taffy` carries a `calc()` as an opaque handle
//! and asks the tree to resolve it against a basis only the running algorithm
//! knows. Its ready-made `TaffyTree` answers `0.0` and offers no hook, so every
//! such value was a refusal. Here it is [`Arena::resolve_calc_value`], and it
//! is a real answer.
//!
//! # The handle is an index, and there is no `unsafe`
//!
//! `taffy` types the handle as `*const ()` and documents that it "may be a
//! pointer, index, etc." — it only has to be non-null with its low three bits
//! clear. So it is `(index + 1) * 8`: casting an integer to a pointer is safe,
//! casting it back is safe, and nothing ever dereferences it. An index cannot
//! dangle, survives the `Vec` growing, and a handle from another arena resolves
//! to nothing rather than to somebody else's expression.
//!
//! # The grid is reached here
//!
//! ADR 0054 § 6 puts two of its three places in this file, because they are
//! where `taffy` asks this tree a question:
//!
//! - **Every expression this tree resolves is truncated onto the grid**, and
//!   a plain percentage is handed over as an expression so that it reaches
//!   the same place: `taffy` resolves a percentage of its own as `basis ×
//!   percent`, with no hook.
//! - **A leaf's text is measured rounded up onto the grid.**
//!
//! The third place, the edges `taffy` hands back, is where they are read
//! back (`crate::engine`), because a box is placed against where its parent
//! was placed and that is known only once the parent is.
//!
//! What `taffy` computes between its own passes, before it hands anything
//! back, is out of reach here: it waits for a hook in `taffy` (ADR 0056,
//! queue item 502).

use crate::auto_repeat::{self, Axis};
use crate::layout_unit;
use crate::measure::{MeasureText, TextStyle};
use alo_box::{BoxId, BoxTree, NaturalSize};
use alo_style::StyleTree;
use alo_value::{FontMetrics, LengthPercentage};
use std::collections::{BTreeMap, BTreeSet};
use taffy::util::{MaybeResolve, ResolveOrZero};
use taffy::{
    AvailableSpace, Cache, CacheTree, DetailedGridInfo, LayoutInput, LayoutOutput, NodeId, RunMode,
    Size as TaffySize, SizingMode, Style, TraversePartialTree, compute_block_layout,
    compute_cached_layout, compute_flexbox_layout, compute_grid_layout, compute_hidden_layout,
    compute_leaf_layout, compute_root_layout,
};

/// What a node in this tree stands for.
#[derive(Debug, Clone)]
pub(crate) enum NodeKind {
    /// A box whose children are in this tree and are laid out by an algorithm.
    Container,
    /// A text box on its own, outside any line, with the font it is set in.
    Text(String, TextStyle),
    /// A box whose children are a line of inline content. Its children are not
    /// in this tree at all: what is inside a line is `crate::inline`'s, and
    /// this tree is told only how big the result is.
    InlineFormatting(BoxId),
    /// A **replaced** box: one sized by its content rather than by its style.
    ///
    /// An image, and later a video. Nothing in CSS says how big the content is,
    /// so the size arrives from whoever decoded it, through `alo_box::Shown`
    /// before the box tree is built, and may be only a ratio, or nothing.
    Replaced(NaturalSize),
    /// A box with nothing to measure — an empty element, or a container
    /// whose children all turned out to be nothing.
    Empty,
}

struct Node {
    style: Style,
    kind: NodeKind,
    children: Vec<usize>,
    cache: Cache,
    layout: taffy::Layout,
    /// What `taffy` resolved this node's percentages against when it was
    /// last laid out where it stands — its parent's `parent_size`: a block's
    /// content box, its height only where definite; a flex container's
    /// content box where definite; a grid item's area.
    basis: TaffySize<Option<f32>>,
    /// Where a grid's last column and last row end, from its border box's
    /// top left, when `taffy` last laid it out where it stands — `None` for
    /// a node that is not a grid, or a grid of no tracks in an axis.
    tracks: TaffySize<Option<f32>>,
}

/// Every expression that could not be reduced to a number before layout began.
///
/// A `calc()` of lengths only is a plain number by the time it gets here — it
/// is the ones with a percentage in them that have to wait for a basis, which
/// is the containing block's size and is not known until the algorithm is
/// running. A plain percentage waits here too, so that it is truncated onto
/// the grid once it is resolved, as a `calc()` is (ADR 0054 § 2).
#[derive(Debug, Default)]
pub(crate) struct Unresolved {
    held: Vec<(LengthPercentage, FontMetrics)>,
}

impl Unresolved {
    /// A handle `taffy` can carry, for an expression it will ask about later.
    pub(crate) fn handle(&mut self, value: &LengthPercentage, metrics: FontMetrics) -> *const () {
        self.held.push((value.clone(), metrics));
        // One-based so it is never null, times eight so its low three bits are
        // clear — the two things `taffy` asks of a handle.
        let handle = self.held.len().saturating_mul(8);
        handle as *const ()
    }

    /// What an expression comes to, against a basis, truncated onto the
    /// grid: resolved whole and truncated once, so `calc(0.01px + 0.01px +
    /// 0.01px)` is one unit where its terms truncated alone would be none
    /// (ADR 0054 § 2).
    ///
    /// A handle this arena did not mint resolves to nothing rather than to
    /// somebody else's expression — the same argument ADR 0003 makes about
    /// node identity.
    fn resolve(&self, handle: *const (), basis: f32) -> f32 {
        self.held(handle).map_or(0.0, |(value, metrics)| {
            layout_unit::truncated(value.to_px(*metrics, basis), metrics.scale())
        })
    }

    /// The fraction a handle stands for, when it was a plain percentage
    /// rather than an expression: what `taffy` would have been handed had
    /// percentages not been carried here (`crate::auto_repeat` asks).
    fn plain_percentage(&self, handle: *const ()) -> Option<f32> {
        match self.held(handle)? {
            (LengthPercentage::Percentage(percent), _) => Some(percent / 100.0),
            _ => None,
        }
    }

    fn held(&self, handle: *const ()) -> Option<&(LengthPercentage, FontMetrics)> {
        let index = (handle as usize) / 8;
        self.held.get(index.checked_sub(1)?)
    }
}

/// The nodes to lay out, and everything needed to measure them.
pub(crate) struct Arena<'a, M: MeasureText> {
    nodes: Vec<Node>,
    unresolved: Unresolved,
    /// The style a grid is being laid out with, for the length of one
    /// `compute_grid_layout` call, where it is not the node's own: see
    /// `crate::auto_repeat`. A grid inside a grid holds its own.
    lowered: BTreeMap<usize, Style>,
    /// The grids, and the axes, whose template `taffy` discarded in some
    /// call because a calculated track in an auto repetition had no
    /// definite size to be counted against (ADR 0047 § 4).
    refused: BTreeSet<(usize, Axis)>,
    boxes: &'a BoxTree,
    styles: &'a StyleTree,
    measure: &'a M,
}

impl auto_repeat::Expressions for &Unresolved {
    fn resolve(&self, handle: *const (), basis: f32) -> f32 {
        Unresolved::resolve(self, handle, basis)
    }

    fn plain_percentage(&self, handle: *const ()) -> Option<f32> {
        Unresolved::plain_percentage(self, handle)
    }
}

impl<'a, M: MeasureText> Arena<'a, M> {
    /// An empty tree that can measure text.
    pub(crate) fn new(boxes: &'a BoxTree, styles: &'a StyleTree, measure: &'a M) -> Self {
        Self {
            nodes: Vec::new(),
            unresolved: Unresolved::default(),
            lowered: BTreeMap::new(),
            refused: BTreeSet::new(),
            boxes,
            styles,
            measure,
        }
    }

    /// What this tree measures text with.
    pub(crate) fn measure(&self) -> &'a M {
        self.measure
    }

    /// Device pixels per CSS pixel on the screen this tree is laid out for,
    /// whose grid every box is placed on (ADR 0057 § 2).
    pub(crate) fn scale(&self) -> f32 {
        self.styles.scale()
    }

    /// The expressions waiting for a basis, to add one to.
    pub(crate) fn unresolved(&mut self) -> &mut Unresolved {
        &mut self.unresolved
    }

    /// Add a node, and say what it is and what is under it.
    pub(crate) fn push(&mut self, style: Style, kind: NodeKind, children: Vec<usize>) -> usize {
        self.nodes.push(Node {
            style,
            kind,
            children,
            cache: Cache::new(),
            layout: taffy::Layout::with_order(0),
            basis: TaffySize::NONE,
            tracks: TaffySize::NONE,
        });
        self.nodes.len() - 1
    }

    /// Give a node one more child, after the ones it was made with.
    ///
    /// How an absolutely positioned box reaches its containing block when
    /// that is not its parent: the containing block's node is made after the
    /// box's, because a node is made after everything under it, so the box is
    /// handed over once the whole tree exists. See `crate::containing`.
    pub(crate) fn adopt(&mut self, parent: usize, child: usize) {
        if let Some(node) = self.nodes.get_mut(parent) {
            node.children.push(child);
        }
    }

    /// The style a node was made with.
    pub(crate) fn style_of(&self, node: usize) -> Option<&Style> {
        self.nodes.get(node).map(|held| &held.style)
    }

    /// Lay out a node and everything under it.
    ///
    /// **Never to whole pixels**: `taffy`'s rounding is a separate pass over
    /// a trait this tree does not implement, so it cannot happen. A box
    /// rounded down to 96 while its text measures 96.16 wraps a word early,
    /// which is how "Remember me" once became two lines in a box wide enough
    /// for one. What this engine does instead is Chrome's grid of 1/64 px
    /// (ADR 0054), on which a text's width is rounded *up*, so a box that
    /// shrinks to its text still holds it.
    pub(crate) fn compute(&mut self, root: usize, available: TaffySize<AvailableSpace>) {
        compute_root_layout(self, NodeId::from(root), available);
    }

    /// The grids whose template `taffy` discarded in some call, and
    /// in which axis, for the engine to say so once per grid and axis rather
    /// than once per call.
    pub(crate) fn refused(&self) -> impl Iterator<Item = (usize, Axis)> + '_ {
        self.refused.iter().copied()
    }

    /// Where a node ended up, relative to the one that holds it.
    pub(crate) fn layout(&self, node: usize) -> Option<taffy::Layout> {
        self.nodes.get(node).map(|held| held.layout)
    }

    /// What a node's percentages were resolved against when it was placed:
    /// the basis `taffy` hands each algorithm's children, which a box's
    /// geometry does not hold (queue item 695).
    pub(crate) fn basis_of(&self, node: usize) -> TaffySize<Option<f32>> {
        self.nodes
            .get(node)
            .map_or(TaffySize::NONE, |held| held.basis)
    }

    /// Where a grid's tracks end across and down, from its border box's top
    /// left, as `taffy` last placed them: a grid that scrolls pads them,
    /// and no box's geometry holds them (queue item 694).
    pub(crate) fn tracks_of(&self, node: usize) -> TaffySize<Option<f32>> {
        self.nodes
            .get(node)
            .map_or(TaffySize::NONE, |held| held.tracks)
    }

    /// What an expression `taffy` carries as a handle comes to against a
    /// basis, as `taffy` itself is told when it asks.
    pub(crate) fn calc(&self, handle: *const (), basis: f32) -> f32 {
        self.unresolved.resolve(handle, basis)
    }

    fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(usize::from(id))
    }

    fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(usize::from(id))
    }

    /// The style of a node, or the default for an id from another tree.
    ///
    /// `taffy` asks for a style by reference and has nowhere to put a failure,
    /// so a node that is not there answers with the default rather than
    /// stopping the layout. It cannot happen: every id `taffy` has came from
    /// [`Arena::push`].
    ///
    /// A grid being laid out with a lowered copy answers with the copy, to
    /// every question, while it is held: its item properties are its own,
    /// so asking as an item gets the same answer either way.
    fn style(&self, id: NodeId) -> &Style {
        const MISSING: &Style = &Style::DEFAULT;
        let index = usize::from(id);
        if let Some(lowered) = self.lowered.get(&index) {
            return lowered;
        }
        self.nodes.get(index).map_or(MISSING, |node| &node.style)
    }

    /// Lay a grid out, with its calculated tracks counted where `taffy`
    /// would discard them (ADR 0047).
    ///
    /// The copy is held for this one call and dropped after it, so the next
    /// call, with other inputs, starts from what the author wrote. A refusal
    /// is recorded from whichever call meets it, once per grid and axis
    /// however many calls do (ADR 0047 § 4): a grid sized by its content is
    /// measured with no definite size, and what it measures as is the
    /// template discarded, whatever the final call is then handed.
    fn lay_out_grid(&mut self, node_id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        let index = usize::from(node_id);
        let unresolved = &self.unresolved;
        let lowering = auto_repeat::lower(self.style(node_id), &inputs, &unresolved);
        self.refused
            .extend(lowering.refused.iter().map(|axis| (index, *axis)));
        let Some(style) = lowering.style else {
            return compute_grid_layout(self, node_id, inputs);
        };
        self.lowered.insert(index, style);
        let output = compute_grid_layout(self, node_id, inputs);
        self.lowered.remove(&index);
        output
    }
}

pub(crate) struct ChildIter<'a>(core::slice::Iter<'a, usize>);

impl Iterator for ChildIter<'_> {
    type Item = NodeId;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().copied().map(NodeId::from)
    }
}

impl<M: MeasureText> taffy::TraversePartialTree for Arena<'_, M> {
    type ChildIter<'c>
        = ChildIter<'c>
    where
        Self: 'c;

    fn child_ids(&self, node_id: NodeId) -> Self::ChildIter<'_> {
        ChildIter(match self.node(node_id) {
            Some(node) => node.children.iter(),
            None => [].iter(),
        })
    }

    fn child_count(&self, node_id: NodeId) -> usize {
        self.node(node_id).map_or(0, |node| node.children.len())
    }

    fn get_child_id(&self, node_id: NodeId, index: usize) -> NodeId {
        self.node(node_id)
            .and_then(|node| node.children.get(index).copied())
            .map_or(NodeId::from(usize::MAX), NodeId::from)
    }
}

impl<M: MeasureText> taffy::TraverseTree for Arena<'_, M> {}

impl<M: MeasureText> taffy::LayoutPartialTree for Arena<'_, M> {
    type CoreContainerStyle<'c>
        = &'c Style
    where
        Self: 'c;

    type CustomIdent = <Style as taffy::CoreStyle>::CustomIdent;

    fn get_core_container_style(&self, node_id: NodeId) -> Self::CoreContainerStyle<'_> {
        self.style(node_id)
    }

    /// Keep a node's layout as `taffy` computed it. It is placed on the grid
    /// when it is read back (`crate::engine`), against where its parent was
    /// placed, which is not known yet when `taffy` hands a child its layout.
    fn set_unrounded_layout(&mut self, node_id: NodeId, layout: &taffy::Layout) {
        if let Some(node) = self.node_mut(node_id) {
            node.layout = *layout;
        }
    }

    /// The reason this tree is ours. See ADR 0004.
    fn resolve_calc_value(&self, val: *const (), basis: f32) -> f32 {
        self.unresolved.resolve(val, basis)
    }

    fn compute_child_layout(&mut self, node_id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        // An ancestor is `display: none`, so nothing under it has a layout —
        // whatever this node's own `display` says.
        if inputs.run_mode == RunMode::PerformHiddenLayout {
            return compute_hidden_layout(self, node_id);
        }
        // The last call that places a node is the one its layout is from.
        if inputs.run_mode == RunMode::PerformLayout
            && let Some(node) = self.node_mut(node_id)
        {
            node.basis = inputs.parent_size;
        }
        compute_cached_layout(self, node_id, inputs, |tree, node_id, inputs| {
            let display = tree.style(node_id).display;
            let has_children = tree.child_count(node_id) > 0;
            match (display, has_children) {
                (taffy::Display::None, _) => compute_hidden_layout(tree, node_id),
                (taffy::Display::Block | taffy::Display::FlowRoot, true) => {
                    compute_block_layout(tree, node_id, inputs, None)
                }
                (taffy::Display::Flex, true) => compute_flexbox_layout(tree, node_id, inputs),
                (taffy::Display::Grid, true) => tree.lay_out_grid(node_id, inputs),
                // An empty grid still has its tracks, which size it and which
                // a grid that scrolls pads (queue item 694).
                (taffy::Display::Grid, false)
                    if matches!(
                        tree.node(node_id).map(|node| &node.kind),
                        Some(NodeKind::Empty)
                    ) =>
                {
                    tree.lay_out_grid(node_id, inputs)
                }
                // A node with nothing under it is measured rather than laid
                // out, and what it measures is what it stands for.
                (_, false) => tree.measure_leaf(node_id, inputs),
            }
        })
    }
}

impl<M: MeasureText> Arena<'_, M> {
    /// How big a leaf is.
    ///
    /// Text is measured with the caller's fonts; a box whose children are a
    /// line of inline content is measured by laying that line out, one
    /// formatting context down; anything else has nothing to measure.
    fn measure_leaf(&self, node_id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        let style = self.style(node_id);
        let resolve = |handle: *const (), basis: f32| self.unresolved.resolve(handle, basis);
        match self.node(node_id).map(|node| &node.kind) {
            Some(NodeKind::Text(text, text_style)) => {
                compute_leaf_layout(inputs, style, resolve, |known, room| {
                    if let (Some(width), Some(height)) = (known.width, known.height) {
                        return TaffySize { width, height };
                    }
                    let wraps = text_style.white_space.wraps();
                    let size =
                        self.measure
                            .measure(text, text_style, width_to_fit(known, room, wraps));
                    // Rounded up onto the grid, so a box that shrinks to its
                    // text always holds it (ADR 0054 § 4).
                    TaffySize {
                        width: known
                            .width
                            .unwrap_or(layout_unit::rounded_up(size.width, self.scale())),
                        height: size.height,
                    }
                })
            }
            // At its own size, a replaced box is sized here rather than by
            // `taffy`, which was never handed its ratio (`crate::replaced`).
            // Only an author's `aspect-ratio` is `taffy`'s, as on any box.
            Some(NodeKind::Replaced(natural))
                if inputs.sizing_mode == SizingMode::InherentSize
                    && style.aspect_ratio.is_none() =>
            {
                let (border_box, content) =
                    replaced_size(&self.unresolved, *natural, style, &inputs);
                let inputs = LayoutInput {
                    known_dimensions: border_box.map(Some),
                    sizing_mode: SizingMode::ContentSize,
                    ..inputs
                };
                compute_leaf_layout(inputs, style, resolve, |_, _| content)
            }
            Some(NodeKind::Replaced(natural)) => {
                let natural = *natural;
                compute_leaf_layout(inputs, style, resolve, |known, room| {
                    // CSS's rule for a replaced box, in `crate::replaced`. The
                    // case that makes a picture on a page behave is a width
                    // given and no height: the height follows from the
                    // content's own ratio, so a photograph in a column is the
                    // right shape rather than squashed.
                    let room = match room.width {
                        AvailableSpace::Definite(definite) => Some(definite),
                        AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
                    };
                    let size =
                        crate::replaced::concrete_size(natural, known.width, known.height, room);
                    TaffySize {
                        width: size.width,
                        height: size.height,
                    }
                })
            }
            Some(NodeKind::InlineFormatting(id)) => {
                let id = *id;
                compute_leaf_layout(inputs, style, resolve, |known, room| {
                    // The line builder knows which runs may wrap, and keeps
                    // `nowrap` on one line however little room it is given.
                    let width = width_to_fit(known, room, true);
                    // Asked how narrow it can be, rather than given a width
                    // of nought: a cut `overflow-wrap: break-word` makes is
                    // not counted then (`crate::overflow_wrap`).
                    let narrowest =
                        known.width.is_none() && room.width == AvailableSpace::MinContent;
                    let lay_out = |width: Option<f32>, narrowest: bool| {
                        crate::engine::measure_inline(
                            self.boxes,
                            self.styles,
                            id,
                            width,
                            narrowest,
                            self.measure,
                        )
                    };
                    let laid_out = lay_out(width, narrowest);
                    // Shrinking to fit the room, a box is never narrower
                    // than its min-content width (CSS Sizing 3 § 5.1), which
                    // a word `break-word` cut is in whole. A width of its
                    // own is handed over as the room, so it is asked too.
                    let shrinks = known.width.is_none() && style.size.width.is_auto();
                    let size = match room.width {
                        AvailableSpace::Definite(room) if shrinks && laid_out.cut_uncounted => {
                            let least = lay_out(Some(0.0), true).size.width;
                            let least = layout_unit::rounded_up(least, self.scale());
                            if least > room {
                                lay_out(Some(least), false).size
                            } else {
                                laid_out.size
                            }
                        }
                        _ => laid_out.size,
                    };
                    // Its lines' width rounded up onto the grid, as a run of
                    // text's is (ADR 0054 § 4).
                    TaffySize {
                        width: known
                            .width
                            .unwrap_or(layout_unit::rounded_up(size.width, self.scale())),
                        height: known.height.unwrap_or(size.height),
                    }
                })
            }
            _ => compute_leaf_layout(inputs, style, resolve, |_, _| TaffySize::ZERO),
        }
    }
}

/// A replaced box's border box and content box at its own size: what
/// its style fixed and limited, made content-box lengths, handed to
/// [`crate::replaced::used_size`].
///
/// A dimension its parent fixed (`known_dimensions`, a border-box
/// length — a flex item's final size, a stretched one) stands in for
/// the style's and is limited as the style's would be, which leaves a
/// size `taffy` already limited where it was.
fn replaced_size(
    unresolved: &Unresolved,
    natural: NaturalSize,
    style: &Style,
    inputs: &LayoutInput,
) -> (TaffySize<f32>, TaffySize<f32>) {
    let calc = |handle: *const (), basis: f32| unresolved.resolve(handle, basis);
    let parent = inputs.parent_size;
    let edges = style.padding.resolve_or_zero(parent.width, calc)
        + style.border.resolve_or_zero(parent.width, calc);
    let edges = edges.sum_axes();
    // What the style writes is a content-box length, unless it says the
    // border box.
    let written = match style.box_sizing {
        taffy::BoxSizing::ContentBox => TaffySize::ZERO,
        taffy::BoxSizing::BorderBox => edges,
    };
    let inside = |length: Option<f32>, edge: f32| length.map(|length| (length - edge).max(0.0));
    let size = style.size.maybe_resolve(parent, calc);
    let min = style.min_size.maybe_resolve(parent, calc);
    let max = style.max_size.maybe_resolve(parent, calc);
    let known = inputs.known_dimensions;
    let asked = crate::replaced::Asked {
        width: inside(known.width, edges.width).or(inside(size.width, written.width)),
        height: inside(known.height, edges.height).or(inside(size.height, written.height)),
        min: crate::geometry::Size::new(
            inside(min.width, written.width).unwrap_or(0.0),
            inside(min.height, written.height).unwrap_or(0.0),
        ),
        max: crate::geometry::Size::new(
            inside(max.width, written.width).unwrap_or(f32::INFINITY),
            inside(max.height, written.height).unwrap_or(f32::INFINITY),
        ),
    };
    let margins = style.margin.resolve_or_zero(parent.width, calc);
    let room = match inputs.available_space.width {
        AvailableSpace::Definite(room) => {
            Some((room - margins.horizontal_axis_sum() - edges.width).max(0.0))
        }
        AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
    };
    let content = crate::replaced::used_size(natural, asked, room);
    let content = TaffySize {
        width: content.width,
        height: content.height,
    };
    (
        TaffySize {
            width: content.width + edges.width,
            height: content.height + edges.height,
        },
        content,
    )
}

/// How wide a leaf may be when it is measured.
///
/// `MaxContent` is the question "how wide would it like to be", answered by
/// measuring with no width at all: nothing breaks. `MinContent` is "how
/// narrow can this be", and CSS Sizing 3 § 5.1 answers it with the widest
/// piece that cannot be broken, which is what laying the content out in no
/// room at all gives: every break is taken, and the widest line is the widest
/// piece. Content that may not wrap is as wide either way.
///
/// Both were once answered by measuring with no width, and a paragraph in a
/// grid column then claimed its whole sentence as its narrowest: alo Sites'
/// closed booking section split its two `1fr` columns 320 and 366 rather
/// than 343 and 343, because one of its lines would not wrap (item 394).
fn width_to_fit(
    known: TaffySize<Option<f32>>,
    available: TaffySize<AvailableSpace>,
    wraps: bool,
) -> Option<f32> {
    known.width.or(match available.width {
        AvailableSpace::Definite(definite) => Some(definite),
        AvailableSpace::MinContent if wraps => Some(0.0),
        AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
    })
}

impl<M: MeasureText> CacheTree for Arena<'_, M> {
    fn cache_get(&mut self, node_id: NodeId, inputs: &LayoutInput) -> Option<LayoutOutput> {
        self.node_mut(node_id)
            .and_then(|node| node.cache.get(inputs))
    }

    fn cache_store(&mut self, node_id: NodeId, inputs: &LayoutInput, output: LayoutOutput) {
        if let Some(node) = self.node_mut(node_id) {
            node.cache.store(inputs, output);
        }
    }

    fn cache_clear(&mut self, node_id: NodeId) {
        if let Some(node) = self.node_mut(node_id) {
            node.cache.clear();
        }
    }
}

impl<M: MeasureText> taffy::LayoutFlexboxContainer for Arena<'_, M> {
    type FlexboxContainerStyle<'c>
        = &'c Style
    where
        Self: 'c;
    type FlexboxItemStyle<'c>
        = &'c Style
    where
        Self: 'c;

    fn get_flexbox_container_style(&self, node_id: NodeId) -> Self::FlexboxContainerStyle<'_> {
        self.style(node_id)
    }

    fn get_flexbox_child_style(&self, child_node_id: NodeId) -> Self::FlexboxItemStyle<'_> {
        self.style(child_node_id)
    }
}

impl<M: MeasureText> taffy::LayoutGridContainer for Arena<'_, M> {
    type GridContainerStyle<'c>
        = &'c Style
    where
        Self: 'c;
    type GridItemStyle<'c>
        = &'c Style
    where
        Self: 'c;

    fn get_grid_container_style(&self, node_id: NodeId) -> Self::GridContainerStyle<'_> {
        self.style(node_id)
    }

    fn get_grid_child_style(&self, child_node_id: NodeId) -> Self::GridItemStyle<'_> {
        self.style(child_node_id)
    }

    /// Told only by a call that places the grid's items, so what is kept is
    /// from the last call that placed the grid, as its layout is. A track's
    /// end is its offset — after the grid's border, padding, gaps and
    /// content alignment — and its size.
    fn set_detailed_grid_info(
        &mut self,
        node_id: NodeId,
        detailed_grid_info: DetailedGridInfo<Self::CustomIdent>,
    ) {
        let end =
            |tracks: &[taffy::Line<f32>]| tracks.iter().map(|track| track.end).reduce(f32::max);
        if let Some(node) = self.node_mut(node_id) {
            node.tracks = TaffySize {
                width: end(&detailed_grid_info.columns.positions),
                height: end(&detailed_grid_info.rows.positions),
            };
        }
    }
}

impl<M: MeasureText> taffy::LayoutBlockContainer for Arena<'_, M> {
    type BlockContainerStyle<'c>
        = &'c Style
    where
        Self: 'c;
    type BlockItemStyle<'c>
        = &'c Style
    where
        Self: 'c;

    fn get_block_container_style(&self, node_id: NodeId) -> Self::BlockContainerStyle<'_> {
        self.style(node_id)
    }

    fn get_block_child_style(&self, child_node_id: NodeId) -> Self::BlockItemStyle<'_> {
        self.style(child_node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_value::{CalcNode, Length};

    fn metrics() -> FontMetrics {
        FontMetrics::estimated(16.0, 16.0)
    }

    /// `calc(100% - 2rem)`, the value that decided ADR 0004.
    fn full_width_less_a_gutter() -> LengthPercentage {
        LengthPercentage::Calc(Box::new(CalcNode::Sum(vec![
            CalcNode::Percentage(100.0),
            CalcNode::Negate(Box::new(CalcNode::Length(Length {
                value: 2.0,
                unit: alo_value::Unit::Rem,
            }))),
        ])))
    }

    /// A picture at its own size, asked with `known` fixed by its parent,
    /// with room to fill and nothing to resolve a percentage against.
    fn picture_sized(style: &Style, known: TaffySize<Option<f32>>) -> TaffySize<f32> {
        let inputs = LayoutInput {
            run_mode: RunMode::PerformLayout,
            known_dimensions: known,
            available_space: TaffySize {
                width: AvailableSpace::Definite(500.0),
                height: AvailableSpace::MaxContent,
            },
            ..LayoutInput::HIDDEN
        };
        replaced_size(
            &Unresolved::default(),
            NaturalSize::sized(20.0, 20.0),
            style,
            &inputs,
        )
        .0
    }

    /// A square picture at `width: 30px; height: 20px` with a two-pixel
    /// border is 34 by 24, its ratio set aside (queue item 590).
    #[test]
    fn a_picture_is_the_size_its_style_fixed_plus_its_edges() {
        let style = Style {
            box_sizing: taffy::BoxSizing::ContentBox,
            size: TaffySize {
                width: taffy::Dimension::length(30.0),
                height: taffy::Dimension::length(20.0),
            },
            border: taffy::Rect {
                left: taffy::LengthPercentage::length(2.0),
                right: taffy::LengthPercentage::length(2.0),
                top: taffy::LengthPercentage::length(2.0),
                bottom: taffy::LengthPercentage::length(2.0),
            },
            ..Style::default()
        };
        assert_eq!(
            picture_sized(&style, TaffySize::NONE),
            TaffySize {
                width: 34.0,
                height: 24.0
            }
        );
        let border_box = Style {
            box_sizing: taffy::BoxSizing::BorderBox,
            ..style
        };
        assert_eq!(
            picture_sized(&border_box, TaffySize::NONE),
            TaffySize {
                width: 30.0,
                height: 20.0
            }
        );
    }

    /// A width its parent fixed — a flex item's final size — stands in for
    /// the style's, and the height follows from it through the picture's
    /// ratio.
    #[test]
    fn a_width_the_parent_fixed_stands_in_for_the_style() {
        let style = Style {
            size: TaffySize {
                width: taffy::Dimension::length(48.0),
                height: taffy::Dimension::auto(),
            },
            ..Style::default()
        };
        assert_eq!(
            picture_sized(&style, TaffySize::NONE),
            TaffySize {
                width: 48.0,
                height: 48.0
            }
        );
        assert_eq!(
            picture_sized(
                &style,
                TaffySize {
                    width: Some(40.0),
                    height: None
                }
            ),
            TaffySize {
                width: 40.0,
                height: 40.0
            }
        );
    }

    #[test]
    fn a_handle_is_never_null_and_never_carries_a_tag() {
        let mut unresolved = Unresolved::default();
        for _ in 0..8 {
            let handle = unresolved.handle(&full_width_less_a_gutter(), metrics());
            assert_ne!(handle as usize, 0, "a handle taffy would reject");
            assert_eq!(handle as usize % 8, 0, "the low three bits are taffy's");
        }
    }

    #[test]
    fn an_expression_comes_back_resolved_against_its_basis() {
        let mut unresolved = Unresolved::default();
        let handle = unresolved.handle(&full_width_less_a_gutter(), metrics());
        // A hundred per cent of four hundred, less two sixteen-pixel rems.
        assert!((unresolved.resolve(handle, 400.0) - 368.0).abs() < 0.001);
        assert!((unresolved.resolve(handle, 100.0) - 68.0).abs() < 0.001);
    }

    #[test]
    fn each_expression_keeps_its_own_handle() {
        let mut unresolved = Unresolved::default();
        let first = unresolved.handle(&LengthPercentage::Percentage(50.0), metrics());
        let second = unresolved.handle(&LengthPercentage::Percentage(25.0), metrics());
        assert_ne!(first, second);
        assert!((unresolved.resolve(first, 200.0) - 100.0).abs() < 0.001);
        assert!((unresolved.resolve(second, 200.0) - 50.0).abs() < 0.001);
    }

    #[test]
    fn a_handle_from_nowhere_resolves_to_nothing_rather_than_to_somebody_elses() {
        let mut unresolved = Unresolved::default();
        unresolved.handle(&LengthPercentage::Percentage(50.0), metrics());
        assert!((unresolved.resolve(core::ptr::null(), 200.0)).abs() < f32::EPSILON);
        assert!((unresolved.resolve(800 as *const (), 200.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn the_narrowest_is_asked_in_no_room_and_the_widest_in_unlimited_room() {
        let unknown = TaffySize {
            width: None,
            height: None,
        };
        let asked = |width| TaffySize {
            width,
            height: AvailableSpace::MaxContent,
        };
        let narrowest = asked(AvailableSpace::MinContent);
        assert_eq!(width_to_fit(unknown, narrowest, true), Some(0.0));
        assert_eq!(
            width_to_fit(unknown, narrowest, false),
            None,
            "text that may not wrap is as narrow as its line",
        );
        assert_eq!(
            width_to_fit(unknown, asked(AvailableSpace::MaxContent), true),
            None
        );
        assert_eq!(
            width_to_fit(unknown, asked(AvailableSpace::Definite(120.0)), true),
            Some(120.0)
        );
        let known = TaffySize {
            width: Some(40.0),
            height: None,
        };
        assert_eq!(
            width_to_fit(known, narrowest, true),
            Some(40.0),
            "a width already settled is the answer to every question",
        );
    }
}
