/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's press on the tab strip (ADR 0024 § 5, queue item 539).
//!
//! The window has decided from its own composition that the point is over
//! the strip, and sent it in the strip's CSS pixels. The conductor sends it
//! to the strip's renderer, which claims the tab it was on by its place in
//! the strip; the claim is decided against **the tabs the strip showed when
//! it last painted** — the picture the person pressed — and a tab chosen
//! that way is chosen as a key chooses one ([`super::choosing`]): the tab
//! left told it is hidden before the tab chosen that it is seen.
//!
//! A claim refused is said in the strip's line, by name, and nothing is
//! chosen. So is a strip renderer that went while it was asked; one that had
//! already gone is not asked, and its line is already said. Before the strip
//! has painted, a press is on the band's background and is nobody's.
//!
//! A claim in any other answer — the strip's renderer claiming when it was
//! sent its state, or a page's renderer claiming at all — has no press
//! behind it, and is refused where that answer is read
//! ([`alo_renderer::strip_decide::unasked`]).

use super::Conducting;
use crate::message::News;
use alo_paint::Point;
use alo_renderer::strip_decide::Decided;

impl Conducting {
    /// Send a person's press on the strip to its renderer, and choose the
    /// tab it claims, if the claim is about a tab the strip showed that is
    /// still open.
    pub(super) fn strip_pressed(&mut self, at: Point) -> Vec<News> {
        if self.strip_shown.is_empty() || self.tabs.strip_renderer().gone().is_some() {
            return Vec::new();
        }
        let shown = self.strip_shown.clone();
        match self.tabs.press_strip(at, &shown) {
            Ok(Decided::Choose(id)) => self.choose(id),
            Ok(Decided::Nothing) => Vec::new(),
            Ok(Decided::Refused(refusal)) => vec![News::StripSaid(refusal.to_string())],
            Ok(Decided::Failed(failure)) => vec![News::StripSaid(format!(
                "the tab strip could not answer a press: {failure}"
            ))],
            Err(lost) => vec![News::StripSaid(
                self.tabs
                    .strip_renderer()
                    .what_happened()
                    .unwrap_or_else(|| lost.to_string()),
            )],
        }
    }
}
