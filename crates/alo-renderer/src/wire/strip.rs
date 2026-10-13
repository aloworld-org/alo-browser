/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The tab strip's state on the wire (ADR 0024 § 4).
//!
//! It goes to a renderer, from the browser process, so a confined renderer
//! is the one reading it — and reads it as it reads anything: every count
//! checked against what is left, the condition by a closed list of tags, the
//! size a finite number. Each title is text and stays text: it is a page's
//! string, carried whole, and the renderer puts it into its document as a
//! text node ([`crate::strip_document`]), never through a parser.
//!
//! A press on the strip goes the same way, and its claim comes back the
//! other: whether it was on a tab, and the tab's place as a number. The
//! place is any number a renderer chose, and is read as one; whether there
//! is a tab there is the browser process's to decide
//! ([`crate::strip_decide`]), not the reader's to assume.

use super::{Reader, Unreadable, Writer, unreadable};
use crate::strip::{Condition, Strip, StripTab};
use crate::strip_claim::Claim;
use alo_layout::geometry::Size;

impl Writer {
    /// The strip: its size, then each tab in order.
    pub(super) fn strip(&mut self, strip: &Strip) {
        self.float(strip.viewport.width);
        self.float(strip.viewport.height);
        self.number(strip.tabs.len() as u64);
        for tab in &strip.tabs {
            self.text(&tab.title);
            self.text(&tab.address);
            self.bool(tab.selected);
            self.tag(match tab.condition {
                Condition::Shown => 0,
                Condition::Loading => 1,
                Condition::Gone => 2,
            });
        }
    }

    /// A press's claim: whether it was on a tab, then the tab's place.
    pub(super) fn claim(&mut self, claim: Option<&Claim>) {
        match claim {
            None => self.bool(false),
            Some(Claim::Select { at }) => {
                self.bool(true);
                self.number(*at as u64);
            }
        }
    }
}

impl Reader<'_> {
    /// A press's claim: whether it was on a tab, then the tab's place — a
    /// place no `usize` holds is refused.
    pub(super) fn claim(&mut self) -> Result<Option<Claim>, Unreadable> {
        if !self.bool()? {
            return Ok(None);
        }
        let at = usize::try_from(self.number()?)
            .map_err(|_| unreadable("a tab's place larger than any strip"))?;
        Ok(Some(Claim::Select { at }))
    }

    /// The strip: its size, then each tab in order.
    pub(super) fn strip(&mut self) -> Result<Strip, Unreadable> {
        let viewport = Size {
            width: self.float()?,
            height: self.float()?,
        };
        let how_many = self.count()?;
        let mut tabs = Vec::new();
        for _ in 0..how_many {
            let title = self.text()?;
            let address = self.text()?;
            let selected = match self.tag()? {
                0 => false,
                1 => true,
                other => return Err(unreadable(format!("a tab's selection tagged {other}"))),
            };
            let condition = match self.tag()? {
                0 => Condition::Shown,
                1 => Condition::Loading,
                2 => Condition::Gone,
                other => return Err(unreadable(format!("a tab's condition tagged {other}"))),
            };
            tabs.push(StripTab {
                title,
                address,
                selected,
                condition,
            });
        }
        Ok(Strip { tabs, viewport })
    }
}
