/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The frame's band on the wire (queue item 991), read as the tab strip's
//! is ([`super::strip`]): the size and the scale finite numbers, the title
//! and the address text that stays text — a page's strings, carried whole,
//! put into the band's document as text nodes ([`crate::band_document`]),
//! never through a parser.

use super::{Reader, Unreadable, Writer};
use crate::band::Band;
use alo_layout::geometry::Size;

impl Writer {
    /// A band's message, tagged `tag`: the band's size and its screen's
    /// scale, then the page's title and address.
    pub(super) fn band(&mut self, tag: u8, band: &Band) {
        self.tag(tag);
        self.float(band.viewport.width);
        self.float(band.viewport.height);
        self.float(band.scale);
        self.text(&band.title);
        self.text(&band.address);
    }
}

impl Reader<'_> {
    /// The band: its size and its screen's scale, then the page's title and
    /// address. A scale that is not a factor above zero is one.
    pub(super) fn band(&mut self) -> Result<Band, Unreadable> {
        let viewport = Size {
            width: self.float()?,
            height: self.float()?,
        };
        let scale = alo_value::device_scale(self.float()?);
        let title = self.text()?;
        let address = self.text()?;
        Ok(Band {
            title,
            address,
            viewport,
            scale,
        })
    }
}
