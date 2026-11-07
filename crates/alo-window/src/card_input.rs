/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's input over a card's page, as the conversation's window sends
//! it to the page (ADR 0076 B, ADR 0024 § 5, queue item 718): the words,
//! and nothing that decides them.
//!
//! The window decides from its own composition that a point is over the
//! card's page box and where in the page it is ([`crate::card_aim`]), and
//! which keys reach the page ([`crate::card_keys`]); the thread holding the
//! card's Chrome sends each to the page as a person's input
//! ([`crate::chrome_card_input`]). **Only the window makes one**, for a
//! person: an agent's request is an [`crate::message::Order::Agent`], which
//! carries no point and becomes none (ADR 0002), and no verb an agent can
//! send is turned into any of these. A coordinate here is only ever a
//! person's pointer, translated by [`crate::card_aim`] — never one an agent
//! named.

use crate::keys::{Arrow, Held};

/// A person's input over the card's page.
#[derive(Debug, Clone, PartialEq)]
pub enum CardInput {
    /// The pointer, or the main button, at a point of the page in its CSS
    /// pixels.
    Pointer {
        /// What the pointer or the button did.
        went: Went,
        /// From the page's left edge, in its CSS pixels.
        x: f64,
        /// From the page's top edge, in its CSS pixels.
        y: f64,
    },
    /// A wheel or a trackpad turned over a point of the page.
    Wheel {
        /// From the page's left edge, in its CSS pixels.
        x: f64,
        /// From the page's top edge, in its CSS pixels.
        y: f64,
        /// How far right the page is asked to scroll, in CSS pixels.
        right: f64,
        /// How far down the page is asked to scroll, in CSS pixels.
        down: f64,
    },
    /// A key, pressed and let go, while the card has the keyboard.
    Key(PersonsKey),
    /// An input method's composition while the card has the keyboard
    /// (queue item 802): what is composed so far, not yet the field's, and
    /// the selection in it in UTF-16 code units, as Chromium counts; empty
    /// ends it, choosing nothing.
    Compose {
        /// What is composed.
        text: String,
        /// The selection in it: where it begins and ends.
        selection: (usize, usize),
    },
    /// What an input method's composition came to, put into the field as
    /// typing puts words (queue item 802).
    Commit(String),
    /// A press that is not the main button's alone, pressed and let go at a
    /// point of the page (queue item 798): what the page then does — a
    /// context menu, a link opened elsewhere — the Chromium alo carries
    /// tells ([`crate::chrome_told`]).
    Other {
        /// Which.
        press: OtherPress,
        /// From the page's left edge, in its CSS pixels.
        x: f64,
        /// From the page's top edge, in its CSS pixels.
        y: f64,
    },
}

/// A press that is not the main button's alone (queue item 798).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtherPress {
    /// The secondary button — or, on a Mac, the main one with Ctrl, which
    /// macOS takes as it: the page's context menu.
    Secondary,
    /// The middle button: a link opened elsewhere.
    Middle,
    /// The main button with ⌘: a link opened elsewhere.
    Command,
}

impl CardInput {
    /// Whether this is the pointer moving, with no button held or with the
    /// main button held from a press on the page: what a newer move of the
    /// same kind says the whole of (queue item 740,
    /// [`crate::card_moves`]).
    pub fn moving(&self) -> Option<Went> {
        match self {
            Self::Pointer {
                went: went @ (Went::Hovered | Went::Dragged),
                ..
            } => Some(*went),
            _ => None,
        }
    }
}

/// What the pointer, or the main button, did over the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Went {
    /// The pointer moved with no button held (queue item 740): over the
    /// page, so the page's hover follows it; or off it, once, so the
    /// page's hover ends.
    Hovered,
    /// It went down over the page, the `clicks`th press in quick
    /// succession at that place ([`crate::card_clicks`]): two a double
    /// click, three a triple.
    Pressed {
        /// How many, counted from one.
        clicks: u32,
    },
    /// The pointer moved while it was held, from a press over the page.
    Dragged,
    /// It came up, after a press over the page — wherever the pointer then
    /// is — as the press's `clicks`th, which is how the page hears a double
    /// click.
    Released {
        /// The press's.
        clicks: u32,
    },
}

/// A key a person pressed, as the page is sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonsKey {
    /// Which.
    pub key: Keyed,
    /// The modifier keys held as it went down: Shift and ⌥ with any key,
    /// and ⌘ or Ctrl only with the keys [`crate::card_keys`] gives the page.
    pub held: Held,
}

/// Which key, as the page is sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keyed {
    /// A key that writes this text, as the window server says it writes it
    /// — a capital with Shift, `π` for ⌥P — with no control character in
    /// it.
    Text(String),
    /// Return, or Enter.
    Return,
    /// Backspace, which macOS calls Delete.
    Backspace,
    /// Tab.
    Tab,
    /// Escape.
    Escape,
    /// An arrow (queue item 741): the caret moved in a field — with Shift
    /// the selection grown, with ⌥ by a word, with ⌘ to the line's end or
    /// the field's — or the page scrolled.
    Arrow(Arrow),
    /// Home: the page scrolled to its top, as Chrome's own window on a Mac
    /// does with it, in a field too (queue item 741).
    Home,
    /// End: to its foot.
    End,
    /// Page Up: a page back up.
    PageUp,
    /// Page Down: a page on down.
    PageDown,
    /// The forward delete: what is after the caret erased.
    Delete,
    /// A function key, by its number from one: heard by the page's script,
    /// as in Chrome's own window, where the system has not taken it.
    Function(u8),
    /// One of the page's own editing shortcuts, with ⌘ on a Mac and Ctrl
    /// elsewhere (queue item 741): the page's to carry out, never the
    /// browser's.
    Editing(Editing),
}

/// The page's own editing shortcuts (queue item 741, [`crate::card_keys`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Editing {
    /// ⌘A: everything in the field, or on the page, selected.
    SelectAll,
    /// ⌘C: what is selected copied.
    Copy,
    /// ⌘X: what is selected cut.
    Cut,
    /// ⌘V: what was copied pasted.
    Paste,
    /// ⌘Z: the last edit undone.
    Undo,
    /// ⌘⇧Z, or Ctrl+Y elsewhere: it done again.
    Redo,
}
