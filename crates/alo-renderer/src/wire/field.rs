/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A field's caret and selection, and where its words were laid out, on
//! the wire (queue items 861, 848).
//!
//! The editing goes to a renderer from the browser process: two places in
//! characters, an input method's composition as text and a place in it,
//! whether the caret is drawn, and how far the words were scrolled in what
//! room — each place no further than any field holds, each length a finite
//! one from nothing to [`FURTHEST`]. A renderer clamps the places to the
//! words it is sent before it draws them ([`crate::field::Editing::drawn`]).
//!
//! Where the words were laid out comes back from the renderer, and is read
//! as anything a renderer says is: finite lengths no further than
//! [`FURTHEST`], and at most as many stops as a field's characters and a
//! composition's, their places never going back. What they say is used only
//! to place the caret a person's press asks for and the input method's
//! candidates, both inside the field: a renderer lying about them misplaces
//! a caret in its own field, and nothing else.

use super::{Reader, Unreadable, Writer, unreadable};
use crate::field::{Composing, Editing, FURTHEST, FieldLaidOut, LONGEST_COMPOSING};
use crate::typed::LONGEST_TYPED;

/// The most stops a field's words have: a place either side of each
/// character it holds, with a composition's.
pub const MOST_STOPS: usize = LONGEST_TYPED + LONGEST_COMPOSING + 1;

impl Writer {
    /// A field's editing: the caret, the anchor, the composition if any,
    /// whether the caret is drawn, how far the words were scrolled and in
    /// what room, if one was measured.
    pub(super) fn editing(&mut self, editing: &Editing) {
        self.number(editing.caret as u64);
        self.number(editing.anchor as u64);
        match &editing.composing {
            Some(composing) => {
                self.bool(true);
                self.text(&composing.text);
                self.number(composing.caret as u64);
            }
            None => self.bool(false),
        }
        self.bool(editing.caret_shown);
        self.float(editing.scrolled);
        match editing.room {
            Some(room) => {
                self.bool(true);
                self.float(room);
            }
            None => self.bool(false),
        }
    }

    /// Where a field's words were laid out, if the part has the field.
    pub(super) fn field_laid_out(&mut self, laid: Option<&FieldLaidOut>) {
        self.tag(13);
        let Some(laid) = laid else {
            self.bool(false);
            return;
        };
        self.bool(true);
        self.float(laid.left);
        self.float(laid.top);
        self.float(laid.height);
        self.float(laid.room);
        self.float(laid.scrolled);
        self.number(laid.stops.len() as u64);
        for (at, x) in &laid.stops {
            self.number(*at as u64);
            self.float(*x);
        }
    }
}

impl Reader<'_> {
    /// A length a field is laid out by: finite, and no further either way
    /// than [`FURTHEST`].
    fn field_length(&mut self, what: &str) -> Result<f32, Unreadable> {
        let length = self.float()?;
        if length.is_finite() && length.abs() <= FURTHEST {
            Ok(length)
        } else {
            Err(unreadable(format!("a field's {what} of {length}")))
        }
    }

    /// A field's editing, as [`Writer::editing`] wrote it.
    pub(super) fn editing(&mut self) -> Result<Editing, Unreadable> {
        let caret = self.character()?;
        let anchor = self.character()?;
        let composing = if self.bool()? {
            let text = self.text()?;
            if text.chars().count() > LONGEST_COMPOSING {
                return Err(unreadable(format!(
                    "a composition longer than the {LONGEST_COMPOSING} characters drawn"
                )));
            }
            Some(Composing {
                text,
                caret: self.character()?,
            })
        } else {
            None
        };
        let caret_shown = self.bool()?;
        let scrolled = self.field_length("scroll")?;
        let room = if self.bool()? {
            Some(self.field_length("room")?)
        } else {
            None
        };
        if scrolled < 0.0 || room.is_some_and(|room| room < 0.0) {
            return Err(unreadable("a field scrolled or roomed less than nothing"));
        }
        Ok(Editing {
            caret,
            anchor,
            composing,
            caret_shown,
            scrolled,
            room,
        })
    }

    /// Where a field's words were laid out, after its tag.
    pub(super) fn field_laid_out(&mut self) -> Result<Option<FieldLaidOut>, Unreadable> {
        if !self.bool()? {
            return Ok(None);
        }
        let left = self.field_length("left")?;
        let top = self.field_length("top")?;
        let height = self.field_length("height")?;
        let room = self.field_length("room")?;
        let scrolled = self.field_length("scroll")?;
        let count = self.count()?;
        if count > MOST_STOPS {
            return Err(unreadable(format!(
                "a field of {count} stops, and at most {MOST_STOPS} are laid out"
            )));
        }
        let mut stops = Vec::with_capacity(count);
        for _ in 0..count {
            let at = self.character()?;
            if stops.last().is_some_and(|(before, _)| *before > at) {
                return Err(unreadable("a field's stops going back"));
            }
            stops.push((at, self.field_length("stop")?));
        }
        Ok(Some(FieldLaidOut {
            left,
            top,
            height,
            room,
            scrolled,
            stops,
        }))
    }
}
