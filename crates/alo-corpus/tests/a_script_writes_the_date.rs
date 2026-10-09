/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page that writes the date into itself draws the same date on every day
//! it is rendered (queue item 356, ADR 0036 § 5).
//!
//! `cases/a-script-writes-the-date` pins the page as files: its script reads
//! `new Date()` and writes the day, the hour and `toISOString()` into its own
//! heading and paragraph. This says out loud why those files can be committed
//! at all — the corpus loads every case with a clock stopped at
//! [`INSTANT`] — by reading the text the page drew and the clock it was given
//! and checking that they are the same instant.

use alo_corpus::{Case, INSTANT, Rendering, cases_directory};

/// The text of every text node in the document, in order.
fn texts(rendering: &Rendering) -> Option<Vec<String>> {
    let document = rendering.document()?;
    Some(
        document
            .descendants(document.root())
            .filter_map(|id| document.get(id)?.text().map(ToOwned::to_owned))
            .collect(),
    )
}

/// The case, read and rendered.
fn dated() -> Option<Rendering> {
    let case = Case::read(&cases_directory().join("a-script-writes-the-date"))?;
    Rendering::of(&case).ok()
}

#[test]
fn the_page_draws_the_corpus_instant_and_nothing_of_the_day_it_ran() {
    // 2026-10-09T00:00:00.000Z, a Friday at midnight.
    assert_eq!(INSTANT.to_bits(), 1_791_504_000_000.0_f64.to_bits());
    let Some(rendering) = dated() else {
        panic!("the case is loaded by a renderer");
    };
    let Some(drawn) = texts(&rendering) else {
        panic!("the case has a document");
    };
    assert!(
        drawn.iter().any(|text| text == "Day 5, 0:00"),
        "the heading says a Friday at midnight: {drawn:?}"
    );
    assert!(
        drawn.iter().any(|text| text == "2026-10-09T00:00:00.000Z"),
        "the paragraph says the instant: {drawn:?}"
    );
    // Rendered again, it says the same.
    let Some(again) = dated().as_ref().and_then(texts) else {
        panic!("the case renders twice");
    };
    assert_eq!(drawn, again);
}
