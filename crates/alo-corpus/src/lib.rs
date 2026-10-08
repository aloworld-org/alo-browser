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
//! A case whose page fetches also says where it was served from
//! (`address.txt`) and what it was answered with (`responses.txt` and the
//! files it names), and is answered from those alone ([`answering`], ADR
//! 0032 § 7): a fetch it froze nothing for is a network error.
//!
//! # A second kind of frozen thing, beside the cases
//!
//! `scripts/` holds **frozen scripts**, one directory each, with an
//! `origin.txt` saying where it came from and when. They are not cases and
//! nothing here reads them: a case is a page with an expected box tree and an
//! expected picture, and nothing renders a service worker. What they share with
//! a case is the property `LOOP.md` actually asks for — an item is judged
//! against something real, and that something is **frozen, never fetched**.
//!
//! `alo-js`'s tests read them by path rather than through this crate, because
//! ADR 0013 § 5 gives that crate no dependencies and a route through here would
//! put the whole renderer behind a lexer.
//!
//! `pictures/` is a third kind, on the same terms: **frozen picture files**,
//! one directory each with an `origin.txt`, read by path by the tests of a
//! crate beneath this one and named by a case's `linked.txt`, so a file two
//! things need is frozen once. Meet's waving hand is there for `alo-dom`'s
//! SVG reader (queue item 309) and for `cases/alo-meet-greeting`, which
//! shows it (item 310).
//!
//! # Running it
//!
//! `cargo test -p alo-corpus` checks every case.
//! `ALO_UPDATE_REFERENCES=1 cargo test -p alo-corpus` rewrites the
//! expectations — and the diff is then the review.

pub mod answering;
pub mod case;
pub mod check;
pub mod rendering;

pub use alo_renderer::pipeline::{Rendered, render, render_with, render_with_resources};
pub use answering::Answered;
pub use case::Case;
pub use check::{Difference, check};
pub use rendering::Rendering;

use std::path::PathBuf;

/// Where the cases live.
pub fn cases_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cases")
}

/// The kind of system every case is rendered as: ADR 0030 § 2's macOS row.
///
/// A page's script can read `navigator.platform` and `navigator.userAgent`
/// and draw differently for each (`cases/alo-downloads` marks the visitor's
/// card), and a renderer is told this machine's row by default. A corpus
/// rendered as whatever machine ran it would commit a Mac's picture on a Mac
/// and fail on Linux for a reason that is not a bug — the fonts' reason
/// again. So the corpus states one row, and a case's own test checks the
/// other two where it matters. The macOS row because the machine the
/// references have been committed from is one, and so no reference moved
/// when the corpus began saying it.
pub const SYSTEM: alo_net::user_agent::System = alo_net::user_agent::MACOS;

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
