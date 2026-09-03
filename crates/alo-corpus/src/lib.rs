/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The reference corpus: small cases with their expected trees and pictures.
//!
//! `docs/features.md` asks for **a committed corpus, each case with its
//! expected image *and* its expected box tree**. This is it, and the reason it
//! is five expectations rather than one is in `CLAUDE.md`: *"A failure that
//! says 'row three moved 4px' is worth ten that say 'the image differs'."*
//! And ADR 0002 adds the fifth: *"Reference renders can assert the tree, not
//! just pixels"* — so what an agent reads is pinned beside what a person sees,
//! and the two cannot drift apart without a test noticing.
//!
//! Each case is a directory of files, so a change shows up as a diff a person
//! can read rather than as a test failure they have to reproduce:
//!
//! | file | what it pins down |
//! |---|---|
//! | `boxes.txt` | what exists, and what each box *means* |
//! | `layout.txt` | where every box ended up, in numbers |
//! | `display.txt` | what is drawn, in what order |
//! | `agent.txt` | what an agent reads: roles, names, states, positions |
//! | `render.png` | everything the others cannot describe |
//!
//! # Running it
//!
//! `cargo test -p alo-corpus` checks every case.
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-corpus` rewrites the
//! expectations — and the diff is then the review.

pub mod case;
pub mod check;

pub use alo_renderer::pipeline::{Rendered, render, render_with, render_with_resources};
pub use case::Case;
pub use check::{Difference, check};

use std::path::PathBuf;

/// Where the cases live.
pub fn cases_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cases")
}

/// The fonts every case is rendered with.
///
/// One font family, committed as a dependency rather than as a file, so that
/// every machine draws the same pixels. A corpus rendered with whatever the
/// machine happened to have would be a corpus that fails on somebody else's
/// laptop for a reason that is not a bug.
pub fn corpus_fonts() -> alo_text::FontDatabase {
    let mut database = alo_text::FontDatabase::new();
    for (family, weight, slant, data) in [
        (
            "DejaVu Sans",
            alo_text::Weight::NORMAL,
            alo_text::Slant::Normal,
            dejavu::sans::regular(),
        ),
        (
            "DejaVu Sans",
            alo_text::Weight::BOLD,
            alo_text::Slant::Normal,
            dejavu::sans::bold(),
        ),
        (
            "DejaVu Serif",
            alo_text::Weight::NORMAL,
            alo_text::Slant::Normal,
            dejavu::serif::regular(),
        ),
    ] {
        if let Some(font) = alo_text::Font::load(family, weight, slant, data.to_vec()) {
            database.add(font);
        }
    }
    database.map_generic("sans-serif", "DejaVu Sans");
    database.map_generic("system-ui", "DejaVu Sans");
    database.map_generic("serif", "DejaVu Serif");
    database
}
