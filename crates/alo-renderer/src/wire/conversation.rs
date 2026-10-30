/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A press on the chats panel or the first screen, and its claim, on the
//! wire (ADR 0074, queue item 710).
//!
//! The press goes to a renderer as two finite numbers, as the strip's does.
//! The claim comes back from the renderer that drew the part, and is read
//! as anything a renderer says is: whether it was on anything, then what,
//! by a closed list of tags, then — for a chat, a site or a waiting chat —
//! its place as a number. The place is any number a renderer chose, and is
//! read as one; whether anything was there is the browser process's to
//! decide ([`crate::conversation_decide`]), not the reader's to assume.

use super::{Reader, Unreadable, Writer, unreadable};
use crate::conversation_claim::Claim;

impl Writer {
    /// A press's claim: whether it was on anything, then what, then the
    /// place of a thing claimed by its place.
    pub(super) fn conversation_claim(&mut self, claim: Option<&Claim>) {
        let Some(claim) = claim else {
            self.bool(false);
            return;
        };
        self.bool(true);
        let (tag, at) = match *claim {
            Claim::NewChat => (0, None),
            Claim::Chat { at } => (1, Some(at)),
            Claim::Settings => (2, None),
            Claim::Site { at } => (3, Some(at)),
            Claim::AddASite => (4, None),
            Claim::Waiting { at } => (5, Some(at)),
            Claim::TheBox => (6, None),
            Claim::Go => (7, None),
        };
        self.tag(tag);
        if let Some(at) = at {
            self.number(at as u64);
        }
    }
}

impl Reader<'_> {
    /// A press's claim, as [`Writer::conversation_claim`] wrote it: a tag
    /// nobody knows is refused, and so is a place no `usize` holds.
    pub(super) fn conversation_claim(&mut self) -> Result<Option<Claim>, Unreadable> {
        if !self.bool()? {
            return Ok(None);
        }
        let claim = match self.tag()? {
            0 => Claim::NewChat,
            1 => Claim::Chat { at: self.place()? },
            2 => Claim::Settings,
            3 => Claim::Site { at: self.place()? },
            4 => Claim::AddASite,
            5 => Claim::Waiting { at: self.place()? },
            6 => Claim::TheBox,
            7 => Claim::Go,
            other => return Err(unreadable(format!("a claim tagged {other}"))),
        };
        Ok(Some(claim))
    }

    /// A place in one of a part's lists.
    fn place(&mut self) -> Result<usize, Unreadable> {
        usize::try_from(self.number()?)
            .map_err(|_| unreadable("a place larger than any part could show"))
    }
}
