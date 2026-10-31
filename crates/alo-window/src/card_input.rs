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
//! send is turned into any of these.

/// A person's input over the card's page.
#[derive(Debug, Clone, PartialEq)]
pub enum CardInput {
    /// The main button, at a point of the page in its CSS pixels.
    Pointer {
        /// What the button did.
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
}

/// What the main button did over the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Went {
    /// It went down over the page.
    Pressed,
    /// The pointer moved while it was held, from a press over the page.
    Dragged,
    /// It came up, after a press over the page — wherever the pointer then
    /// is.
    Released,
}

/// A key a person pressed, as the page is sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonsKey {
    /// Which.
    pub key: Keyed,
    /// Whether Shift was held.
    pub shift: bool,
    /// Whether ⌥, or Alt, was held.
    pub option: bool,
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
}
