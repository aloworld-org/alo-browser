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
//! opinion it does not serve. That is only true if it stays behind one file,
//! so **`engine.rs` is the only file in this repository that names `taffy`**,
//! and `scripts/gate.sh` checks it on every run.
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
//! - **Inline formatting.** `taffy` has no inline layout, so a run of inline
//!   content is laid out as a wrapping flex row: boxes go side by side and
//!   wrap, without baselines or breaking inside a run of text. Queue item 6
//!   brings shaping and line breaking, and replaces it.
//! - **`calc()` with a percentage in it**, in a layout property. Refused and
//!   recorded; queue item 15.
//! - **`z-index` and stacking**, which decide what is drawn over what rather
//!   than where anything is. That belongs with paint, queue item 7.

pub mod engine;
pub mod geometry;
pub mod inline;
pub mod keyword;
pub mod measure;
pub mod placement;
pub mod sizing;
pub mod style;
pub mod track;
pub mod tree;

pub use engine::compute;
pub use geometry::{Edges, Point, Rect, Size};
pub use inline::{Fragment, InlineItem, InlineLayout, LineBox};
pub use keyword::{
    Alignment, BoxSizing, Distribution, FlexDirection, FlexWrap, GridAutoFlow, Overflow,
    Positioning,
};
pub use measure::{BlockFont, MeasureText, NoText};
pub use placement::{GridLine, GridPlacement};
pub use sizing::{AutoLength, Sizing};
pub use style::LayoutStyle;
pub use track::{RepeatCount, Track, TrackList, TrackSize};
pub use tree::{BoxGeometry, LayoutTree};
