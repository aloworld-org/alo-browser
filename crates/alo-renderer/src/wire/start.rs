/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The first screen's state on the wire (ADR 0074, queue item 700).
//!
//! Read by a confined renderer as the panel's is ([`super::panel`]): every
//! count checked against what is left, the size and the scale finite
//! numbers, and every name text that stays text — the renderer puts each
//! into its document as a text node ([`crate::start_document`]).

use super::{Reader, Unreadable, Writer};
use crate::start::{KeptSite, Start, Waiting};
use alo_layout::geometry::Size;

impl Writer {
    /// The first screen: its size and scale, its greeting, then each site
    /// and each waiting chat in order.
    pub(super) fn start(&mut self, start: &Start) {
        self.float(start.viewport.width);
        self.float(start.viewport.height);
        self.float(start.scale);
        self.text(&start.greeting);
        self.number(start.sites.len() as u64);
        for site in &start.sites {
            self.text(&site.name);
            self.text(&site.address);
        }
        self.number(start.waiting.len() as u64);
        for chat in &start.waiting {
            self.text(&chat.name);
            self.text(&chat.said);
        }
    }
}

impl Reader<'_> {
    /// The first screen, as [`Writer::start`] wrote it. A scale that is not
    /// a factor above zero is one (ADR 0057 § 3).
    pub(super) fn start(&mut self) -> Result<Start, Unreadable> {
        let viewport = Size {
            width: self.float()?,
            height: self.float()?,
        };
        let scale = alo_value::device_scale(self.float()?);
        let greeting = self.text()?;
        let mut sites = Vec::new();
        for _ in 0..self.count()? {
            let name = self.text()?;
            sites.push(KeptSite {
                name,
                address: self.text()?,
            });
        }
        let mut waiting = Vec::new();
        for _ in 0..self.count()? {
            let name = self.text()?;
            waiting.push(Waiting {
                name,
                said: self.text()?,
            });
        }
        Ok(Start {
            greeting,
            sites,
            waiting,
            viewport,
            scale,
        })
    }
}
