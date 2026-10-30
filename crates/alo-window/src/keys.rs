/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The browser's own keys: which presses are the browser's, and what each
//! asks of the tabs (queue item 529, ADR 0024 § 5).
//!
//! *"The browser's own keys — a new tab, close this one, the next one — are
//! the browser process's, read before any page sees them, so no page can
//! take them."* The window's event loop turns what the window server says
//! into a [`Press`] and asks [`order`] what it means. In a window of tabs that
//! is the whole of the keyboard: a press that is not one of these goes
//! nowhere, because a page has no keyboard yet (item 258), and there is no
//! message that could carry a key to a renderer. In the conversation's
//! window a key edits the one box instead ([`crate::typing`], queue item
//! 701), which the browser process holds; no key goes to a renderer there
//! either.
//!
//! # Which keys
//!
//! | | macOS | elsewhere |
//! |---|---|---|
//! | A new tab | ⌘T | Ctrl+T |
//! | Close this one | ⌘W | Ctrl+W |
//! | The next tab | ⌃Tab | Ctrl+Tab |
//! | The previous tab | ⌃⇧Tab | Ctrl+Shift+Tab |
//!
//! Chrome's, so a person's hands already know them. The modifiers must be
//! exactly these: ⌘⇧T and ⌘⇧W are other things in other browsers (a closed
//! tab reopened, the window closed), and reading them as ⌘T and ⌘W would open
//! a tab for a person asking for something else.
//!
//! The letter is the key's **without its modifiers**, as the person's
//! keyboard layout names it, so ⌘T is the key marked T wherever a layout puts
//! it, and Caps Lock changes nothing.
//!
//! # What is not a press
//!
//! A key held down repeats, and the window server says so. A repeat is not
//! read: holding ⌘W is one tab closed, not every tab in the window, which a
//! person cannot stop in time. A key the window server says was already down
//! when the window gained focus is not a press either: the person pressed it
//! somewhere else.

use crate::message::{Order, TabOrder};

/// A key a person pressed, as far as the browser reads one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Press {
    /// The key, without its modifiers.
    pub key: Key,
    /// The modifier keys held as it went down.
    pub held: Held,
    /// Whether the window server says this is the key held down, repeating.
    pub repeat: bool,
}

/// A modifier key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    /// ⌘ on macOS; the Windows or Super key elsewhere.
    Command,
    /// ⌃ or Ctrl.
    Control,
    /// ⇧ or Shift.
    Shift,
    /// ⌥ or Alt.
    Option,
}

impl Modifier {
    /// Its place in a [`Held`].
    fn bit(self) -> u8 {
        match self {
            Modifier::Command => 1,
            Modifier::Control => 2,
            Modifier::Shift => 4,
            Modifier::Option => 8,
        }
    }
}

/// Which modifier keys are held: a set of [`Modifier`]s.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Held(u8);

impl Held {
    /// No modifier held.
    pub const NOTHING: Held = Held(0);

    /// This set with `modifier` in it too.
    #[must_use]
    pub fn with(self, modifier: Modifier) -> Self {
        Self(self.0 | modifier.bit())
    }

    /// Whether `modifier` is held, with or without others.
    pub fn holds(self, modifier: Modifier) -> bool {
        self.0 & modifier.bit() != 0
    }

    /// Exactly `modifiers`, and nothing else.
    fn exactly(modifiers: &[Modifier]) -> Self {
        modifiers
            .iter()
            .fold(Held::NOTHING, |held, modifier| held.with(*modifier))
    }
}

/// Which key, as the browser tells keys apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// A key that writes a character, by the character it writes with no
    /// modifiers held.
    Character(String),
    /// Tab.
    Tab,
    /// Return, or Enter: what chooses in the one box (queue item 701).
    Return,
    /// Backspace, which macOS calls Delete: what erases in the one box.
    Backspace,
    /// Escape: what empties the one box.
    Escape,
    /// Any other key.
    Other,
}

/// Whose keys: macOS's, where the browser's keys are ⌘'s, or everybody
/// else's, where they are Ctrl's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS.
    Mac,
    /// Windows and Linux.
    Elsewhere,
}

impl Platform {
    /// The platform this browser was built for.
    pub const HERE: Platform = if cfg!(target_os = "macos") {
        Platform::Mac
    } else {
        Platform::Elsewhere
    };
}

impl Press {
    /// A press of `key` with no modifiers held.
    pub fn of(key: Key) -> Self {
        Self {
            key,
            held: Held::NOTHING,
            repeat: false,
        }
    }

    /// A press of the key that writes `character`.
    pub fn character(character: &str) -> Self {
        Self::of(Key::Character(character.to_owned()))
    }

    /// This press with `modifier` held too.
    #[must_use]
    pub fn with(self, modifier: Modifier) -> Self {
        Self {
            held: self.held.with(modifier),
            ..self
        }
    }

    /// This press, repeating because the key is held.
    #[must_use]
    pub fn repeating(self) -> Self {
        Self {
            repeat: true,
            ..self
        }
    }

    /// Whether the key is the one that writes `letter`, whatever the case.
    fn is_letter(&self, letter: char) -> bool {
        match &self.key {
            Key::Character(written) => {
                let mut characters = written.chars().flat_map(char::to_lowercase);
                characters.next() == Some(letter) && characters.next().is_none()
            }
            Key::Tab | Key::Return | Key::Backspace | Key::Escape | Key::Other => false,
        }
    }
}

/// What a press asks of the tabs on `platform`, if it is one of the browser's
/// own keys.
pub fn meaning(press: &Press, platform: Platform) -> Option<TabOrder> {
    if press.repeat {
        return None;
    }
    // The key that holds the letter shortcuts: ⌘ on macOS, Ctrl elsewhere.
    let letters = match platform {
        Platform::Mac => Modifier::Command,
        Platform::Elsewhere => Modifier::Control,
    };
    if press.key == Key::Tab {
        return if press.held == Held::exactly(&[Modifier::Control]) {
            Some(TabOrder::Next)
        } else if press.held == Held::exactly(&[Modifier::Control, Modifier::Shift]) {
            Some(TabOrder::Previous)
        } else {
            None
        };
    }
    if press.held != Held::exactly(&[letters]) {
        return None;
    }
    if press.is_letter('t') {
        Some(TabOrder::New)
    } else if press.is_letter('w') {
        Some(TabOrder::CloseSelected)
    } else {
        None
    }
}

/// The order a press gives the conductor on this platform, if it gives one:
/// what the window's event loop sends for each key a person presses.
pub fn order(press: &Press) -> Option<Order> {
    meaning(press, Platform::HERE).map(Order::Tabs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_a_mac(press: &Press) -> Option<TabOrder> {
        meaning(press, Platform::Mac)
    }

    fn elsewhere(press: &Press) -> Option<TabOrder> {
        meaning(press, Platform::Elsewhere)
    }

    #[test]
    fn the_four_keys_on_a_mac() {
        let t = Press::character("t").with(Modifier::Command);
        let w = Press::character("w").with(Modifier::Command);
        let tab = Press::of(Key::Tab).with(Modifier::Control);
        assert_eq!(on_a_mac(&t), Some(TabOrder::New));
        assert_eq!(on_a_mac(&w), Some(TabOrder::CloseSelected));
        assert_eq!(on_a_mac(&tab), Some(TabOrder::Next));
        assert_eq!(
            on_a_mac(&tab.with(Modifier::Shift)),
            Some(TabOrder::Previous)
        );
    }

    #[test]
    fn the_four_keys_elsewhere_are_ctrls() {
        let t = Press::character("t").with(Modifier::Control);
        let w = Press::character("w").with(Modifier::Control);
        let tab = Press::of(Key::Tab).with(Modifier::Control);
        assert_eq!(elsewhere(&t), Some(TabOrder::New));
        assert_eq!(elsewhere(&w), Some(TabOrder::CloseSelected));
        assert_eq!(elsewhere(&tab), Some(TabOrder::Next));
        assert_eq!(
            elsewhere(&tab.with(Modifier::Shift)),
            Some(TabOrder::Previous)
        );
        // And ⌘ is not the browser's there.
        assert_eq!(
            elsewhere(&Press::character("t").with(Modifier::Command)),
            None
        );
        assert_eq!(
            on_a_mac(&Press::character("t").with(Modifier::Control)),
            None
        );
    }

    #[test]
    fn a_capital_letter_is_the_same_key() {
        assert_eq!(
            on_a_mac(&Press::character("T").with(Modifier::Command)),
            Some(TabOrder::New)
        );
    }

    #[test]
    fn another_modifier_held_is_another_shortcut() {
        let t = Press::character("t").with(Modifier::Command);
        assert_eq!(on_a_mac(&t.clone().with(Modifier::Shift)), None, "⌘⇧T");
        assert_eq!(on_a_mac(&t.clone().with(Modifier::Option)), None, "⌥⌘T");
        assert_eq!(on_a_mac(&t.with(Modifier::Control)), None, "⌃⌘T");
        assert_eq!(
            on_a_mac(
                &Press::character("w")
                    .with(Modifier::Command)
                    .with(Modifier::Shift)
            ),
            None,
            "⌘⇧W"
        );
        let tab = Press::of(Key::Tab).with(Modifier::Control);
        assert_eq!(
            on_a_mac(&tab.clone().with(Modifier::Command)),
            None,
            "⌃⌘Tab"
        );
        assert_eq!(on_a_mac(&tab.with(Modifier::Option)), None, "⌃⌥Tab");
        assert_eq!(on_a_mac(&Press::of(Key::Tab)), None, "Tab alone");
        assert_eq!(
            on_a_mac(&Press::of(Key::Tab).with(Modifier::Command)),
            None,
            "⌘Tab is the system's"
        );
    }

    #[test]
    fn a_key_with_nothing_held_is_not_the_browsers() {
        for key in ["t", "w", "a", "\t"] {
            assert_eq!(on_a_mac(&Press::character(key)), None, "{key:?}");
            assert_eq!(elsewhere(&Press::character(key)), None, "{key:?}");
        }
        for key in [Key::Other, Key::Return, Key::Backspace, Key::Escape] {
            assert_eq!(
                on_a_mac(&Press::of(key.clone()).with(Modifier::Command)),
                None
            );
            assert_eq!(on_a_mac(&Press::of(key)), None);
        }
    }

    #[test]
    fn other_letters_and_longer_text_are_not_the_browsers() {
        for written in ["q", "h", "tw", "", "ť", "\u{0}"] {
            assert_eq!(
                on_a_mac(&Press::character(written).with(Modifier::Command)),
                None,
                "{written:?}"
            );
        }
    }

    #[test]
    fn a_held_key_repeating_does_nothing_more() {
        assert_eq!(
            on_a_mac(&Press::character("w").with(Modifier::Command).repeating()),
            None
        );
        assert_eq!(
            on_a_mac(&Press::of(Key::Tab).with(Modifier::Control).repeating()),
            None
        );
    }

    #[test]
    fn the_order_sent_is_the_meaning_here() {
        let press = match Platform::HERE {
            Platform::Mac => Press::character("t").with(Modifier::Command),
            Platform::Elsewhere => Press::character("t").with(Modifier::Control),
        };
        assert!(matches!(order(&press), Some(Order::Tabs(TabOrder::New))));
        assert!(order(&Press::character("t")).is_none());
    }
}
