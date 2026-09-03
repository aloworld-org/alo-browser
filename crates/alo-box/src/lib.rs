//! The box tree: what gets drawn, and what each box means.
//!
//! This is the item ADR 0002 shapes most directly. A box tree that kept only
//! rectangles could not be turned into an agent tree afterwards — it would
//! have thrown away the only thing an agent needs — so every box carries what
//! it *is*, what is true of it, and what it is called, put there when the box
//! is made.
//!
//! That is why queue item 4 sits before layout rather than after it. Layout
//! adds numbers to these boxes; it does not get to decide what they mean.
//!
//! # The two halves
//!
//! - **Structure**, in [`tree`]: which elements make boxes at all, what kind,
//!   and the anonymous boxes a container needs so that its children are all of
//!   one kind. This is what layout walks.
//! - **Meaning**, in [`semantics`] — [`role`] and [`state`] — read from what
//!   the author declared and never from how a box looks. ADR 0002: guessing a
//!   role from appearance is what screen-scraping already does badly, and
//!   owning the engine is exactly what lets us not.
//!
//! # No geometry
//!
//! Not one number. Where a box ends up is queue item 5; mixing the two would
//! make a box's meaning depend on where it landed, which is the failure this
//! whole ordering exists to prevent.

pub mod display;
pub mod role;
pub mod semantics;
pub mod state;
pub mod tree;
pub mod whitespace;

pub use display::{Display, Inside, Outside};
pub use role::{KnownRole, Role};
pub use semantics::Semantics;
pub use state::{Checked, Current, States};
pub use tree::{BoxId, BoxKind, BoxNode, BoxTree, Purpose, build};
pub use whitespace::WhiteSpace;
