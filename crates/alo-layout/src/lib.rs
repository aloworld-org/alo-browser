/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Layout: where every box ends up, in numbers.
//!
//! Item 4 built boxes that know what they are. This gives each of them a
//! rectangle, and nothing else — a box's meaning was settled before it had a
//! position, which is the ordering ADR 0002 asks for and the reason an agent
//! tree can be a *view* of this rather than a second structure.
//!
//! # The boundary
//!
//! Flexbox, grid and block layout come from `taffy`, which ADR 0001 calls a
//! judgement call rather than physics: a real chunk of engine, taken because
//! it gets us laying out sooner, and meant to be replaced when we have an
//! opinion it does not serve. That is only true if it stays behind as little
//! as possible, so **`engine.rs`, `arena.rs` and `auto_repeat.rs` are the only
//! files in this repository that name `taffy`**, and `scripts/gate.sh` checks
//! it every run. The third exists only until `taffy` counts a calculated
//! track in an auto repetition itself (ADR 0047), and is then deleted.
//!
//! The **algorithms** are rented; the **tree they walk** is ours (`arena.rs`,
//! ADR 0004). A list of nodes with styles, children and a cache is storage
//! rather than physics, and owning it is what lets this engine answer
//! `calc(100% - 2rem)` instead of refusing it.
//!
//! # Assert numbers
//!
//! `CLAUDE.md`: a layout is a tree with numbers in it, and a test says where
//! the box is. [`LayoutTree::to_outline`] writes the whole tree out with a
//! rectangle beside every box, which is the strongest form of that — a change
//! that moves something says which box and by how much.
//!
//! # What this does not do yet, named rather than hidden
//!
//! - **Inline formatting is not `taffy`'s.** A box whose children all sit in
//!   a line is handed to the algorithms as a *leaf* and laid out by
//!   [`inline`], because inline layout is a different algorithm from the other
//!   three and every engine has its own.
//! - **`z-index` and stacking**, which decide what is drawn over what rather
//!   than where anything is. That belongs with paint, queue item 7.

pub(crate) mod arena;
pub(crate) mod auto_repeat;
pub(crate) mod baseline;
pub mod border_style;
pub(crate) mod containing;
pub mod ellipsis;
pub(crate) mod end_padding;
pub mod engine;
pub(crate) mod field_width;
pub mod geometry;
pub mod inline;
pub mod keyword;
pub mod layout_unit;
pub mod legend;
pub mod line_height;
pub(crate) mod line_width;
pub(crate) mod marker;
pub mod measure;
pub(crate) mod out_of_flow;
pub mod overflow_wrap;
pub mod placement;
pub mod replaced;
pub mod scroll_container;
pub mod sizing;
pub(crate) mod soft_wrap;
pub mod style;
pub(crate) mod text_run;
pub mod text_style;
pub mod track;
pub mod tree;
pub mod vertical_align;
pub mod viewport;

pub use border_style::border_style;
pub use ellipsis::{Cut, ELLIPSIS, Ellipsis, Truncated, truncate};
pub use engine::compute;
pub use geometry::{Edges, Point, Rect, Size};
pub use inline::{Anchor, Fragment, InlineItem, InlineLayout, LineBox, TextAlignment};
pub use keyword::{
    Alignment, BoxSizing, Distribution, FlexDirection, FlexWrap, GridAutoFlow, Overflow,
    Positioning, TextOverflow,
};
pub use legend::Band;
pub use line_height::used_line_height;
pub use measure::{BlockFont, MeasureText, NoText, ScaledFont, TextStyle};
pub use overflow_wrap::OverflowWrap;
pub use placement::{GridLine, GridPlacement};
pub use sizing::{AutoLength, MaxSizing, Sizing};
pub use style::LayoutStyle;
pub use text_style::text_style_of;
pub use track::{RepeatCount, Track, TrackList, TrackSize};
pub use tree::{BoxGeometry, LayoutTree};
pub use vertical_align::{LineAlign, VerticalAlign};
