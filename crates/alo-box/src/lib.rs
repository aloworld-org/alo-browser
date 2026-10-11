/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

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
//! # What is rented
//!
//! One thing: **case mapping**, for `text-transform` (ADR 0049), which
//! [`case`] is the only file to name. A text box holds its text as drawn —
//! white space collapsed by [`whitespace`], then put in its case by
//! [`transform`] — so everything after this crate reads one text.
//!
//! # No geometry
//!
//! Not one number. Where a box ends up is queue item 5; mixing the two would
//! make a box's meaning depend on where it landed, which is the failure this
//! whole ordering exists to prevent.

pub mod case;
mod details;
pub mod line_break;
pub mod natural;
pub mod picture;
pub mod role;
pub mod semantics;
pub mod state;
pub mod svg;
pub mod toggle;
pub mod transform;
pub mod tree;
pub mod whitespace;

/// `display` is read where style is computed, because whether an element is a
/// list item decides whether it makes a `::marker` there (ADR 0044 § 1); the
/// box tree reads the same reading.
pub use alo_style::display::{self, Display, Inside, Outside};
pub use natural::NaturalSize;
pub use picture::Shown;
pub use role::{KnownRole, Role};
pub use semantics::Semantics;
pub use state::{Checked, Current, States};
pub use transform::TextTransform;
pub use tree::{BoxId, BoxKind, BoxNode, BoxTree, Purpose, build, build_showing};
pub use whitespace::WhiteSpace;
