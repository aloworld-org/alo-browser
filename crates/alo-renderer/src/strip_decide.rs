/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the browser process decides of the strip renderer's claim (ADR 0024
//! § 5, queue item 539).
//!
//! The strip's renderer answers a person's press with a claim
//! ([`crate::strip_claim`]), and the claim is a stranger's word: the strip
//! shows every page's title, so a title that found a way into its renderer
//! would be speaking here. So the browser process acts **only on a claim
//! about a tab it holds, answering a press it sent**, and refuses every
//! other by name:
//!
//! - **A place the strip was not shown.** The claim names a place in the
//!   strip, and the browser process kept which tab was at each place of the
//!   strip it last saw painted. A place past them is refused.
//! - **A tab closed since.** The tab at that place may have been closed
//!   between the strip being painted and the press being answered; it is
//!   refused, never moved along to the tab now in its place, which is not
//!   the one the person pressed.
//! - **A claim with no press behind it.** A claim answers only
//!   [`crate::message::ToRenderer::StripPress`]. The strip's renderer
//!   claiming in answer to anything else — being sent its state, being
//!   asked to paint — or a page's renderer claiming at all is refused
//!   ([`unasked`]), and nothing is chosen.
//!
//! A refusal is said to the person, in the strip's line, because it is a
//! renderer lying or a defect in this one, and either is somebody's to see.
//! It is not written into the session's record: a claim asks for nothing
//! outside the person's own tabs, and the record is of requests (ADR 0012
//! § 5).

use crate::message::{Failure, FromRenderer};
use crate::strip_claim::Claim;
use crate::tab::TabId;
use core::fmt;

/// What a press on the strip came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decided {
    /// Choose this tab.
    Choose(TabId),
    /// The press was on no tab, and nothing is done.
    Nothing,
    /// The claim was refused, and nothing is done.
    Refused(Refusal),
    /// The strip's renderer could not answer the press.
    Failed(Failure),
}

/// Why a claim was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The claim named a place the strip was not shown.
    NotShown {
        /// The place claimed.
        at: usize,
        /// How many tabs the strip was shown.
        shown: usize,
    },
    /// The tab at the place claimed has been closed since the strip was
    /// painted.
    Closed {
        /// The place claimed.
        at: usize,
    },
    /// A claim that answered no press: the strip's renderer claimed in
    /// answer to something else, or a page's renderer claimed at all.
    Unasked,
    /// The strip's renderer answered a press with something that is not a
    /// claim.
    NotAClaim,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let refused = "and nothing was chosen";
        match self {
            Refusal::NotShown { at, shown } => write!(
                f,
                "the tab strip's renderer claimed the tab at place {at}, and it was shown \
                 {shown}, {refused}"
            ),
            Refusal::Closed { at } => write!(
                f,
                "the tab strip's renderer claimed the tab at place {at}, which has closed since \
                 it was drawn, {refused}"
            ),
            Refusal::Unasked => write!(
                f,
                "a renderer claimed a tab with no press behind it, {refused}"
            ),
            Refusal::NotAClaim => write!(
                f,
                "the tab strip's renderer answered a press with something that is not a claim, \
                 {refused}"
            ),
        }
    }
}

/// What the strip renderer's `answer` to a press comes to, when the strip
/// it holds showed the tabs `shown`, in order, and `open` says which tabs
/// are still open.
pub fn decide(answer: FromRenderer, shown: &[TabId], open: impl Fn(TabId) -> bool) -> Decided {
    let claim = match answer {
        FromRenderer::Claimed(claim) => claim,
        FromRenderer::Failed(failure) => return Decided::Failed(failure),
        _ => return Decided::Refused(Refusal::NotAClaim),
    };
    let Some(Claim::Select { at }) = claim else {
        return Decided::Nothing;
    };
    match shown.get(at) {
        None => Decided::Refused(Refusal::NotShown {
            at,
            shown: shown.len(),
        }),
        Some(&id) if open(id) => Decided::Choose(id),
        Some(_) => Decided::Refused(Refusal::Closed { at }),
    }
}

/// The refusal for an answer that claims a tab when no press was sent: any
/// [`FromRenderer::Claimed`] that is not the answer [`decide`] is given.
/// [`None`] for every other answer.
pub fn unasked(answer: &FromRenderer) -> Option<Refusal> {
    matches!(answer, FromRenderer::Claimed(_)).then_some(Refusal::Unasked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn select(at: usize) -> FromRenderer {
        FromRenderer::Claimed(Some(Claim::Select { at }))
    }

    /// Three tabs shown, the second of them closed since.
    fn shown() -> ([TabId; 3], impl Fn(TabId) -> bool) {
        let mut identities = alo_net::cause::Identities::default();
        let ids = [identities.a_tab(), identities.a_tab(), identities.a_tab()];
        let [_, closed, _] = ids;
        (ids, move |id| id != closed)
    }

    #[test]
    fn a_claim_on_a_tab_shown_and_open_chooses_it() {
        let (ids, open) = shown();
        let [first, _, third] = ids;
        assert_eq!(decide(select(0), &ids, &open), Decided::Choose(first));
        assert_eq!(decide(select(2), &ids, &open), Decided::Choose(third));
    }

    #[test]
    fn a_press_on_no_tab_does_nothing() {
        let (ids, open) = shown();
        assert_eq!(
            decide(FromRenderer::Claimed(None), &ids, &open),
            Decided::Nothing
        );
    }

    #[test]
    fn a_place_past_the_strip_is_refused_by_name() {
        let (ids, open) = shown();
        for at in [3, 4, usize::MAX] {
            let decided = decide(select(at), &ids, &open);
            assert_eq!(
                decided,
                Decided::Refused(Refusal::NotShown { at, shown: 3 }),
                "{at}"
            );
        }
        assert_eq!(
            decide(select(0), &[], &open),
            Decided::Refused(Refusal::NotShown { at: 0, shown: 0 })
        );
        assert_eq!(
            Refusal::NotShown { at: 5, shown: 3 }.to_string(),
            "the tab strip's renderer claimed the tab at place 5, and it was shown 3, and \
             nothing was chosen"
        );
    }

    #[test]
    fn a_tab_closed_since_is_refused_and_not_moved_along() {
        let (ids, open) = shown();
        assert_eq!(
            decide(select(1), &ids, &open),
            Decided::Refused(Refusal::Closed { at: 1 })
        );
        assert!(Refusal::Closed { at: 1 }.to_string().contains("has closed"));
    }

    #[test]
    fn a_press_answered_with_anything_else_is_not_a_claim() {
        let (ids, open) = shown();
        assert_eq!(
            decide(FromRenderer::Failed(Failure::NotTheStrip), &ids, &open),
            Decided::Failed(Failure::NotTheStrip)
        );
        assert_eq!(
            decide(
                FromRenderer::UsingFont {
                    family: "x".to_owned()
                },
                &ids,
                &open
            ),
            Decided::Refused(Refusal::NotAClaim)
        );
    }

    #[test]
    fn a_claim_answering_no_press_is_refused_by_name() {
        assert_eq!(unasked(&select(0)), Some(Refusal::Unasked));
        assert_eq!(
            unasked(&FromRenderer::Claimed(None)),
            Some(Refusal::Unasked)
        );
        assert_eq!(unasked(&FromRenderer::Failed(Failure::NothingLoaded)), None);
        assert_eq!(
            Refusal::Unasked.to_string(),
            "a renderer claimed a tab with no press behind it, and nothing was chosen"
        );
    }
}
