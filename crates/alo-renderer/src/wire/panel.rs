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
use crate::interface_face::InterfaceFace;
use crate::panel::{Chat, Closing, Doing, Panel};
use alo_layout::geometry::Size;

impl Writer {
    /// The panel: its size and its screen's scale, what was searched for
    /// and whether the search has the keyboard, whether it is hidden as a
    /// rail, the name of a chat just deleted if *Undo* is offered, then
    /// each chat in order, whether its row is hovered and whether it offers
    /// its × (queue item 791); the face its words are set in, and whether
    /// *Settings* is the page on screen (queue item 854).
    pub(super) fn panel(&mut self, panel: &Panel) {
        self.float(panel.viewport.width);
        self.float(panel.viewport.height);
        self.float(panel.scale);
        self.face(panel.face);
        self.bool(panel.settings);
        self.text(&panel.searched);
        self.bool(panel.searching);
        self.editing(&panel.editing);
        self.bool(panel.hidden);
        self.bool(panel.deleted.is_some());
        if let Some(deleted) = &panel.deleted {
            self.text(deleted);
        }
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
            self.bool(chat.hovered);
            self.bool(chat.closing == Closing::Offered);
        }
    }

    /// The face the interface's words are set in (queue items 853, 854,
    /// 975), by a tag of its own: Manrope, the default since 975, took the
    /// next tag rather than moving the others'.
    pub(super) fn face(&mut self, face: InterfaceFace) {
        self.tag(match face {
            InterfaceFace::Inter => 0,
            InterfaceFace::Geist => 1,
            InterfaceFace::DmSans => 2,
            InterfaceFace::PlusJakartaSans => 3,
            InterfaceFace::InstrumentSans => 4,
            InterfaceFace::Manrope => 5,
        });
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
        let face = self.face()?;
        let settings = self.bool()?;
        let searched = self.text()?;
        let searching = self.bool()?;
        let editing = self.editing()?;
        let hidden = self.bool()?;
        let deleted = if self.bool()? {
            Some(self.text()?)
        } else {
            None
        };
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
            let hovered = self.bool()?;
            let closing = if self.bool()? {
                Closing::Offered
            } else {
                Closing::Withheld
            };
            chats.push(Chat {
                name,
                site,
                open,
                current,
                doing,
                hovered,
                closing,
            });
        }
        Ok(Panel {
            chats,
            viewport,
            scale,
            searched,
            searching,
            editing,
            hidden,
            deleted,
            face,
            settings,
        })
    }

    /// The face the interface's words are set in, one of the five; any
    /// other tag is refused.
    pub(super) fn face(&mut self) -> Result<InterfaceFace, Unreadable> {
        match self.tag()? {
            0 => Ok(InterfaceFace::Inter),
            1 => Ok(InterfaceFace::Geist),
            2 => Ok(InterfaceFace::DmSans),
            3 => Ok(InterfaceFace::PlusJakartaSans),
            4 => Ok(InterfaceFace::InstrumentSans),
            5 => Ok(InterfaceFace::Manrope),
            other => Err(unreadable(format!("a face tagged {other}"))),
        }
    }
}
