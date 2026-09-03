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

use crate::measure::{MeasureText, TextStyle};
use alo_box::{BoxId, BoxTree};
use alo_style::StyleTree;
use alo_value::{FontMetrics, LengthPercentage};
use taffy::{
    AvailableSpace, Cache, CacheTree, LayoutInput, LayoutOutput, NodeId, RunMode,
    Size as TaffySize, Style, TraversePartialTree, compute_block_layout, compute_cached_layout,
    compute_flexbox_layout, compute_grid_layout, compute_hidden_layout, compute_leaf_layout,
    compute_root_layout,
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
    /// so the size arrives from whoever decoded it — see
    /// `BoxTree::set_natural_size`.
    Replaced(TaffySize<f32>),
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
}

/// Every expression that could not be reduced to a number before layout began.
///
/// A `calc()` of lengths only is a plain number by the time it gets here — it
/// is the ones with a percentage in them that have to wait for a basis, which
/// is the containing block's size and is not known until the algorithm is
/// running.
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

    /// What an expression comes to, against a basis.
    ///
    /// A handle this arena did not mint resolves to nothing rather than to
    /// somebody else's expression — the same argument ADR 0003 makes about
    /// node identity.
    fn resolve(&self, handle: *const (), basis: f32) -> f32 {
        let index = (handle as usize) / 8;
        let Some(slot) = index.checked_sub(1) else {
            return 0.0;
        };
        self.held
            .get(slot)
            .map_or(0.0, |(value, metrics)| value.to_px(*metrics, basis))
    }
}

/// The nodes to lay out, and everything needed to measure them.
pub(crate) struct Arena<'a, M: MeasureText> {
    nodes: Vec<Node>,
    unresolved: Unresolved,
    boxes: &'a BoxTree,
    styles: &'a StyleTree,
    measure: &'a M,
}

impl<'a, M: MeasureText> Arena<'a, M> {
    /// An empty tree that can measure text.
    pub(crate) fn new(boxes: &'a BoxTree, styles: &'a StyleTree, measure: &'a M) -> Self {
        Self {
            nodes: Vec::new(),
            unresolved: Unresolved::default(),
            boxes,
            styles,
            measure,
        }
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
        });
        self.nodes.len() - 1
    }

    /// Lay out a node and everything under it.
    ///
    /// **Sub-pixel throughout**: `taffy`'s rounding is a separate pass over a
    /// trait this tree does not implement, so it cannot happen. A box rounded
    /// down to 96 while its text measures 96.16 wraps a word early, which is
    /// how "Remember me" once became two lines in a box wide enough for one.
    pub(crate) fn compute(&mut self, root: usize, available: TaffySize<AvailableSpace>) {
        compute_root_layout(self, NodeId::from(root), available);
    }

    /// Where a node ended up, relative to the one that holds it.
    pub(crate) fn layout(&self, node: usize) -> Option<taffy::Layout> {
        self.nodes.get(node).map(|held| held.layout)
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
    fn style(&self, id: NodeId) -> &Style {
        const MISSING: &Style = &Style::DEFAULT;
        self.node(id).map_or(MISSING, |node| &node.style)
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
        compute_cached_layout(self, node_id, inputs, |tree, node_id, inputs| {
            let display = tree.style(node_id).display;
            let has_children = tree.child_count(node_id) > 0;
            match (display, has_children) {
                (taffy::Display::None, _) => compute_hidden_layout(tree, node_id),
                (taffy::Display::Block | taffy::Display::FlowRoot, true) => {
                    compute_block_layout(tree, node_id, inputs, None)
                }
                (taffy::Display::Flex, true) => compute_flexbox_layout(tree, node_id, inputs),
                (taffy::Display::Grid, true) => compute_grid_layout(tree, node_id, inputs),
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
                    let size = self
                        .measure
                        .measure(text, text_style, width_to_fit(known, room));
                    TaffySize {
                        width: size.width,
                        height: size.height,
                    }
                })
            }
            Some(NodeKind::Replaced(natural)) => {
                let natural = *natural;
                compute_leaf_layout(inputs, style, resolve, |known, _room| {
                    // The three cases, and the third is the one that makes an
                    // image on a page behave: a width given and no height means
                    // the height follows from the picture's own ratio, so a
                    // photograph in a column is the right shape rather than
                    // squashed. Taffy would do this from `aspect_ratio`, and
                    // saying it here keeps the ratio next to the size it came
                    // from.
                    match (known.width, known.height) {
                        (Some(width), Some(height)) => TaffySize { width, height },
                        (Some(width), None) => TaffySize {
                            width,
                            height: if natural.width > 0.0 {
                                width * natural.height / natural.width
                            } else {
                                natural.height
                            },
                        },
                        (None, Some(height)) => TaffySize {
                            width: if natural.height > 0.0 {
                                height * natural.width / natural.height
                            } else {
                                natural.width
                            },
                            height,
                        },
                        (None, None) => natural,
                    }
                })
            }
            Some(NodeKind::InlineFormatting(id)) => {
                let id = *id;
                compute_leaf_layout(inputs, style, resolve, |known, room| {
                    let width = width_to_fit(known, room);
                    let size = crate::engine::measure_inline(
                        self.boxes,
                        self.styles,
                        id,
                        width,
                        self.measure,
                    )
                    .size;
                    TaffySize {
                        width: known.width.unwrap_or(size.width),
                        height: known.height.unwrap_or(size.height),
                    }
                })
            }
            _ => compute_leaf_layout(inputs, style, resolve, |_, _| TaffySize::ZERO),
        }
    }
}

/// How wide a leaf may be when it is measured.
///
/// `MinContent` and `MaxContent` are the questions "how narrow can this be"
/// and "how wide would it like to be"; both are answered by measuring with no
/// width at all.
fn width_to_fit(
    known: TaffySize<Option<f32>>,
    available: TaffySize<AvailableSpace>,
) -> Option<f32> {
    known.width.or(match available.width {
        AvailableSpace::Definite(definite) => Some(definite),
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
}
