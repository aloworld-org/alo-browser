/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page's policies have objected to in its inline style, kept for the
//! page's life, and what of that has still to cross (ADR 0034 § 4, queue
//! item 346).
//!
//! # Once per element, placement and text
//!
//! A page is drawn many times, and each draw asks its policies about every
//! piece of inline style it holds ([`crate::inline_style`]). Without a memory
//! one refused `style` attribute would be objected to at every draw, and a
//! page that is painted sixty times a second would ask the browser process to
//! post sixty reports a second about one fact. So an objection is made the
//! **first** time a draw finds a given element holding a given text in a
//! given placement, and never again while the page is loaded. The same
//! attribute written twice is one report. A different text on the same
//! element is a different piece of inline style, and is reported.
//!
//! What is kept for each is the element's id, the placement and the SHA-256
//! of the text — not the text, which can be a megabyte a stranger chose. An
//! id is never reused (ADR 0003), so an element that is removed and another
//! made in its place are two elements.
//!
//! # Waiting for an answer that can carry it
//!
//! Only `Loaded`, `Acted` and `Delivered` carry objections. A draw that a
//! `Paint` or a `ReadTree` asked for answers with a frame or a tree, so what
//! it found waits here for the next answer that can carry it. At most
//! [`MOST_OBJECTIONS`] wait. Any more are counted, and the answer that takes
//! them says how many were not passed on: the bound is on what the browser
//! process is asked to post, never on what is refused. An objection counted
//! and not passed on is still remembered. Otherwise the next draw would find
//! it again and a flood would become a stream.

use std::collections::BTreeSet;

use alo_dom::NodeId;
use alo_net::csp::Placement;
use alo_net::digest::Digest;

use crate::violations::{MOST_OBJECTIONS, Objection};

/// One piece of inline style, as remembered: the element, whether the style
/// was its own element or its attribute, and the SHA-256 of its text.
type Seen = (NodeId, bool, Vec<u8>);

/// What a page's policies have objected to in its inline style, and what is
/// waiting to cross.
#[derive(Debug, Default)]
pub struct Objected {
    /// Every piece of inline style a policy has objected to on this page.
    seen: BTreeSet<Seen>,
    /// What draws found and no answer has carried yet.
    waiting: Vec<Objection>,
    /// How many more were found than could wait.
    beyond: usize,
}

impl Objected {
    /// Nothing objected to: a page just loaded.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether this is the first time a policy objected to `text`, placed so,
    /// in `element` — and remember that it has been.
    pub fn first_time(&mut self, element: NodeId, placement: Placement, text: &str) -> bool {
        let attribute = matches!(placement, Placement::Attribute);
        let digest = Digest::Sha256.of(text.as_bytes());
        self.seen.insert((element, attribute, digest))
    }

    /// Keep what one draw found to cross with the next answer, and count
    /// `more`, which that draw found and did not keep.
    pub fn owe(&mut self, found: &[Objection], more: usize) {
        let room = MOST_OBJECTIONS.saturating_sub(self.waiting.len());
        self.waiting.extend(found.iter().take(room).copied());
        self.beyond = self
            .beyond
            .saturating_add(found.len().saturating_sub(room))
            .saturating_add(more);
    }

    /// Everything waiting, into `objections`, after whatever it already
    /// holds, within the one bound an answer's objections share. How many
    /// were not passed on is said into `said`. Nothing waits afterwards.
    pub fn take(&mut self, objections: &mut Vec<Objection>, said: &mut Vec<String>) {
        let room = MOST_OBJECTIONS.saturating_sub(objections.len());
        let waiting = core::mem::take(&mut self.waiting);
        let left_out = waiting
            .len()
            .saturating_sub(room)
            .saturating_add(core::mem::take(&mut self.beyond));
        objections.extend(waiting.into_iter().take(room));
        if left_out > 0 {
            said.push(format!(
                "{left_out} more policy objections to this page's inline style were not passed \
                 on to be reported: one answer carries at most {MOST_OBJECTIONS}"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_net::csp::Inline;

    fn attribute() -> Objection {
        Objection {
            policy: 0,
            kind: Inline::Style,
            placement: Placement::Attribute,
        }
    }

    /// Two distinct elements' ids.
    fn two() -> (NodeId, NodeId) {
        let document = alo_dom::parse_document("<p></p>");
        let mut ids = document.descendants(document.root());
        let first = ids.next().unwrap_or_else(|| document.root());
        (document.root(), first)
    }

    #[test]
    fn each_element_placement_and_text_is_objected_to_once() {
        let mut objected = Objected::new();
        let (a, b) = two();
        assert_ne!(a, b);
        assert!(objected.first_time(a, Placement::Attribute, "color: red"));
        assert!(!objected.first_time(a, Placement::Attribute, "color: red"));
        assert!(objected.first_time(a, Placement::Attribute, "color: blue"));
        assert!(objected.first_time(a, Placement::Element, "color: red"));
        assert!(objected.first_time(b, Placement::Attribute, "color: red"));
        assert!(!objected.first_time(b, Placement::Attribute, "color: red"));
    }

    #[test]
    fn what_waits_is_taken_once_and_after_what_the_answer_holds() {
        let mut objected = Objected::new();
        objected.owe(&[attribute()], 0);
        objected.owe(&[attribute()], 0);
        let mut objections = vec![Objection {
            policy: 1,
            kind: Inline::Script,
            placement: Placement::Element,
        }];
        let mut said = Vec::new();
        objected.take(&mut objections, &mut said);
        assert_eq!(objections.len(), 3);
        assert_eq!(objections.first().map(|o| o.kind), Some(Inline::Script));
        assert!(said.is_empty(), "{said:?}");
        let mut again = Vec::new();
        objected.take(&mut again, &mut said);
        assert!(again.is_empty() && said.is_empty());
    }

    #[test]
    fn more_than_the_bound_are_counted_across_draws_and_said_once() {
        let mut objected = Objected::new();
        objected.owe(&vec![attribute(); MOST_OBJECTIONS - 4], 0);
        objected.owe(&[attribute(); 10], 3);
        let mut objections = vec![attribute(); 2];
        let mut said = Vec::new();
        objected.take(&mut objections, &mut said);
        assert_eq!(objections.len(), MOST_OBJECTIONS);
        // Six beyond the bound and three a draw did not keep, waiting; two
        // more that had no room beside what the answer already held.
        assert_eq!(said.len(), 1);
        assert!(
            said.first()
                .is_some_and(|line| line.starts_with("11 more policy objections")),
            "{said:?}"
        );
        let mut again = Vec::new();
        said.clear();
        objected.take(&mut again, &mut said);
        assert!(again.is_empty() && said.is_empty(), "{said:?}");
    }

    /// The text is a stranger's: a megabyte, a NUL, nothing at all — each is
    /// remembered by its digest without trouble.
    #[test]
    fn hostile_text_is_remembered_by_its_digest() {
        let mut objected = Objected::new();
        let (a, _) = two();
        let huge = "x".repeat(1 << 20);
        for text in [huge.as_str(), "\u{0}", "", "color: red !important"] {
            assert!(objected.first_time(a, Placement::Attribute, text));
            assert!(!objected.first_time(a, Placement::Attribute, text));
        }
        objected.owe(&[], usize::MAX);
        objected.owe(&[attribute()], usize::MAX);
        let mut objections = Vec::new();
        let mut said = Vec::new();
        objected.take(&mut objections, &mut said);
        assert_eq!(objections.len(), 1);
        assert_eq!(said.len(), 1);
    }
}
