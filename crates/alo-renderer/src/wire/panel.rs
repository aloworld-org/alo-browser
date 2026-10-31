/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The chats panel's state on the wire (ADR 0074, queue item 699).
//!
//! It goes to a renderer, from the browser process, so a confined renderer
//! reads it as it reads the strip's: every count checked against what is
//! left, what the agent is doing by a closed list of tags, the
//! size and the scale finite numbers. A chat's name and site are text and
//! stay text: the renderer puts them into its document as text nodes
//! ([`crate::panel_document`]).

use super::{Reader, Unreadable, Writer, unreadable};
use crate::panel::{Chat, Doing, Panel};
use alo_layout::geometry::Size;

impl Writer {
    /// The panel: its size and its screen's scale, what was searched for
    /// and whether the search has the keyboard, whether it is hidden as a
    /// rail, then each chat in order.
    pub(super) fn panel(&mut self, panel: &Panel) {
        self.float(panel.viewport.width);
        self.float(panel.viewport.height);
        self.float(panel.scale);
        self.text(&panel.searched);
        self.bool(panel.searching);
        self.bool(panel.hidden);
        self.number(panel.chats.len() as u64);
        for chat in &panel.chats {
            self.text(&chat.name);
            self.text(&chat.site);
            self.bool(chat.open);
            self.bool(chat.current);
            self.tag(match chat.doing {
                Doing::Nothing => 0,
                Doing::Working => 1,
                Doing::Done => 2,
            });
        }
    }
}

impl Reader<'_> {
    /// The panel: its size and its screen's scale, what was searched for
    /// and whether the search has the keyboard, whether it is hidden as a
    /// rail, then each chat in order. A scale that is not a number is refused, as a size's is, and one that
    /// is not a factor above zero is one (ADR 0057 § 3).
    pub(super) fn panel(&mut self) -> Result<Panel, Unreadable> {
        let viewport = Size {
            width: self.float()?,
            height: self.float()?,
        };
        let scale = alo_value::device_scale(self.float()?);
        let searched = self.text()?;
        let searching = self.bool()?;
        let hidden = self.bool()?;
        let how_many = self.count()?;
        let mut chats = Vec::new();
        for _ in 0..how_many {
            let name = self.text()?;
            let site = self.text()?;
            let open = self.bool()?;
            let current = self.bool()?;
            let doing = match self.tag()? {
                0 => Doing::Nothing,
                1 => Doing::Working,
                2 => Doing::Done,
                other => return Err(unreadable(format!("what a chat is doing tagged {other}"))),
            };
            chats.push(Chat {
                name,
                site,
                open,
                current,
                doing,
            });
        }
        Ok(Panel {
            chats,
            viewport,
            scale,
            searched,
            searching,
            hidden,
        })
    }
}
