/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Text: fonts, shaping, line breaking, and how wide a line is.
//!
//! Layout left a seam open — `alo_layout::MeasureText`, with no default
//! implementation, because layout does not know how wide a piece of text is
//! and a built-in guess would have been a wrong number every layout quietly
//! depended on. This crate fills it.
//!
//! # The awkward scripts first
//!
//! `docs/features.md` asks for the awkward scripts before the easy ones,
//! because *a pipeline that assumed left-to-right and one glyph per character
//! is a pipeline that gets rewritten.* So nothing here is indexed by
//! character: a shaped glyph names the **byte range** it came from, several
//! glyphs may name the same range, and one glyph may cover several characters.
//! Arabic joins, Hebrew runs right to left, and `e` followed by a combining
//! acute is one glyph — all three are tested, and none of them is
//! special-cased.
//!
//! # What is rented and where
//!
//! - **Shaping** is `rustybuzz`, and [`shape`] is the only file that names it.
//! - **Where a line may break** is `unicode-linebreak` — UAX #14, a table —
//!   and [`linebreak`] is the only file that names it.
//! - **Which byte is which character in an encoding older than Unicode** is
//!   `encoding_rs`, and `macintosh` is the only file that names it. It exists
//!   because a font may state its own name in one of those encodings, and a
//!   font whose name nobody can read is a font no page can ask for.
//! - **Which font, and what to do when it lacks the character**, is ours:
//!   [`database`]. That is a policy question rather than a physics one.
//! - **Where a line does break** is ours: [`line`]. Take the last opportunity
//!   that fits; overflow rather than cut a word in half.
//!
//! `scripts/gate.sh` checks both boundaries on every run.
//!
//! # What is not here
//!
//! **Inline formatting** — several inline boxes on one line, with their
//! baselines aligned — is queue item 16 and belongs in layout, where the boxes
//! are. What is here is the half that item needs first.
//!
//! **Rasterisation** is queue item 17, beside paint: a glyph bitmap with no
//! canvas to draw into can only be tested against itself.
//!
//! **Bidirectional reordering** is stage 2. Each run knows which way it goes
//! and the runs are laid down in the order the text is written, which is right
//! for text that is one direction and honest about text that is two.

pub mod database;
pub mod font;
pub mod line;
pub mod linebreak;
mod macintosh;
pub mod measure;
pub mod run;
pub mod shape;

pub use database::{Absent, FontDatabase, Instead};
pub use font::{
    FaceMetrics, Font, FontRequest, LONGEST_NAME, Slant, Style, THINNEST_CSS_NAMES, WEIGHT_AXIS,
    Weight, WeightAxis, family_in, style_in,
};
pub use line::{Line, Paragraph, lay_out, measure_unwrapped};
pub use linebreak::{BreakPoint, opportunities};
pub use measure::TextMeasurer;
pub use run::{TextRun, split};
pub use shape::{Direction, ShapedGlyph, ShapedRun, shape, spaced};
