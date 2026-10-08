/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page's linked style sheet on the wire: the asks in an answer, and the
//! answer to one (ADR 0035, queue item 348).
//!
//! The asks come from a renderer, so every field is read as a stranger's, as
//! a fetch's are ([`super::fetch`]): lengths checked against what is left,
//! enums by a closed list of tags — every mode `alo-net` names, so that a
//! renderer asking for a sheet in `navigate` mode is refused by the browser
//! process as one that broke the boundary rather than lost here — and the
//! URL as text the browser process parses itself. **How many** is the
//! browser process's to bound ([`crate::sheet_owed`]).
//!
//! The answer goes to a renderer: the ask's number, and the sheet's bytes or
//! nothing.

use super::fetch::{CREDENTIALS, MODES, tag_of, tagged};
use super::{POLICIES, Reader, Unreadable, Writer, unreadable};
use crate::sheet::{SheetAnswer, SheetAsk};

impl Writer {
    /// Every sheet asked for in an answer, in order.
    pub(super) fn sheets(&mut self, asks: &[SheetAsk]) {
        self.number(asks.len() as u64);
        for ask in asks {
            self.number(ask.number);
            self.text(&ask.url);
            self.tag(tag_of(&MODES, &ask.mode));
            self.tag(tag_of(&CREDENTIALS, &ask.credentials));
            let policy = ask
                .referrer
                .map_or(0, |policy| tag_of(&POLICIES, &policy).saturating_add(1));
            self.tag(policy);
            self.maybe_text(ask.nonce.as_deref());
        }
    }

    /// The answer to one sheet.
    pub(super) fn sheet_answer(&mut self, answer: &SheetAnswer) {
        self.number(answer.number);
        match &answer.bytes {
            None => self.tag(0),
            Some(bytes) => {
                self.tag(1);
                self.bytes(bytes);
            }
        }
    }
}

impl Reader<'_> {
    /// Every sheet asked for in an answer: each part a claim.
    pub(super) fn sheets(&mut self) -> Result<Vec<SheetAsk>, Unreadable> {
        let how_many = self.count()?;
        let mut asks = Vec::new();
        for _ in 0..how_many {
            let number = self.number()?;
            let url = self.text()?;
            let mode = tagged(&MODES, self.tag()?, "a style sheet's mode")?;
            let credentials = tagged(&CREDENTIALS, self.tag()?, "a style sheet's credentials")?;
            let referrer = match self.tag()? {
                0 => None,
                tag => Some(tagged(
                    &POLICIES,
                    tag - 1,
                    "a style sheet's referrer policy",
                )?),
            };
            let nonce = self.maybe_text()?;
            asks.push(SheetAsk {
                number,
                url,
                mode,
                credentials,
                referrer,
                nonce,
            });
        }
        Ok(asks)
    }

    /// The answer to one sheet.
    pub(super) fn sheet_answer(&mut self) -> Result<SheetAnswer, Unreadable> {
        let number = self.number()?;
        let bytes = match self.tag()? {
            0 => None,
            1 => Some(self.bytes()?),
            other => {
                return Err(unreadable(format!("a style sheet's answer tagged {other}")));
            }
        };
        Ok(SheetAnswer { number, bytes })
    }
}

/// How many bytes `answer` is as a message, without writing it: what
/// [`crate::sheet_make`] asks before it sends a sheet, so that one too large
/// for one message is a failure rather than a message nobody reads.
pub fn sheet_answer_size(answer: &SheetAnswer) -> usize {
    const NUMBER: usize = 8;
    // The message's tag, the ask's number and the answer's tag.
    let size = 1 + NUMBER + 1;
    answer.bytes.as_ref().map_or(size, |bytes| {
        size.saturating_add(NUMBER).saturating_add(bytes.len())
    })
}
