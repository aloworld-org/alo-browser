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
//! as possible, so **`engine.rs` and `arena.rs` are the only files in this
//! repository that name `taffy`**, and `scripts/gate.sh` checks it every run.
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
pub mod engine;
pub mod geometry;
pub mod inline;
pub mod keyword;
pub mod legend;
pub mod measure;
pub mod placement;
pub mod sizing;
pub mod style;
pub mod track;
pub mod tree;

pub use engine::compute;
pub use geometry::{Edges, Point, Rect, Size};
pub use inline::{Fragment, InlineItem, InlineLayout, LineBox, TextAlignment};
pub use keyword::{
    Alignment, BoxSizing, Distribution, FlexDirection, FlexWrap, GridAutoFlow, Overflow,
    Positioning,
};
pub use legend::Band;
pub use measure::{BlockFont, MeasureText, NoText, TextStyle};
pub use placement::{GridLine, GridPlacement};
pub use sizing::{AutoLength, Sizing};
pub use style::LayoutStyle;
pub use track::{RepeatCount, Track, TrackList, TrackSize};
pub use tree::{BoxGeometry, LayoutTree};
