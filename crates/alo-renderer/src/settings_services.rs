/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*' third section, **Search and answers**, as the browser
//! process sends it (ADR 0106, queue item 905): which search service and
//! which AI service alo asks, a `SearXNG`'s address, and a person's own keys
//! — drawn by [`crate::settings_services_document`].
//!
//! # What is sent, and what never is
//!
//! The choices by their names, which one is made, a sentence for each
//! key saying whose is used, and what is typed in a field. **A key is never
//! sent**: a key's field is sent as the number of characters typed, drawn
//! as dots ([`Entry::masked`]); the key itself stays in the browser
//! process, which alone keeps it, in the person's keychain. A renderer has
//! no network and is never sent a key (ADR 0005).
//!
//! # What a press is on
//!
//! Every thing of it that can be pressed, in the order it is drawn
//! ([`ServicesShown::pressables`]) — the browser process decides a press by
//! its place among them, against the same list it painted, as *alo at
//! work*'s are ([`crate::conversation_claim`]).

/// The most dots a key's field draws: past it, more are typed but no more
/// drawn.
pub const MOST_DOTS: usize = 48;

/// The longest a field's words are sent: a `SearXNG`'s address is at most
/// 512 bytes, and a key 256 characters.
pub const LONGEST_TYPED: usize = 512;

/// One of the section's fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Entry {
    /// A `SearXNG`'s address.
    Address,
    /// The search service's key.
    SearchKey,
    /// The AI service's key.
    AiKey,
}

impl Entry {
    /// Whether what it holds is a key, drawn as dots.
    pub fn masked(self) -> bool {
        !matches!(self, Self::Address)
    }

    /// What its row, and the field, are named.
    pub fn name(self) -> &'static str {
        match self {
            Self::Address => "Address",
            Self::SearchKey => "Search key",
            Self::AiKey => "AI key",
        }
    }

    /// What an empty field says.
    pub fn placeholder(self) -> &'static str {
        match self {
            Self::Address => "https://search.example.org",
            Self::SearchKey | Self::AiKey => "Paste your own key",
        }
    }

    /// What its *Keep* button is named.
    pub fn keep_named(self) -> &'static str {
        match self {
            Self::Address => "Keep the address",
            Self::SearchKey => "Keep your search key",
            Self::AiKey => "Keep your AI key",
        }
    }

    /// What its *Remove* button is named.
    pub fn remove_named(self) -> &'static str {
        match self {
            Self::Address => "Remove the address",
            Self::SearchKey => "Remove your search key",
            Self::AiKey => "Remove your AI key",
        }
    }
}

/// Something of the section a person presses, or the keyboard is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServicePress {
    /// The search service at this place among those offered.
    Search(usize),
    /// The AI service at this place among those offered.
    Answers(usize),
    /// A field: the keyboard into it.
    Field(Entry),
    /// A field's *Keep*.
    Keep(Entry),
    /// A field's *Remove*: a person's own key taken out of the keychain.
    Remove(Entry),
}

/// One field as it is drawn.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Typed {
    /// What it holds: an address as written, or — for a key — as many
    /// dots as characters typed, at most [`MOST_DOTS`].
    pub shown: String,
    /// Whether the keyboard is in it.
    pub focused: bool,
}

/// One row holding a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRow {
    /// Which field.
    pub entry: Entry,
    /// What it holds and whether it has the keyboard.
    pub typed: Typed,
    /// A sentence under it: whose key is used, or the address kept.
    pub state: String,
    /// Whether it offers *Remove*: a person's own key is kept.
    pub removable: bool,
}

/// The section, as the browser process sends it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServicesShown {
    /// The search services, by their own names, in order.
    pub search: Vec<String>,
    /// The one chosen, by its place.
    pub search_on: usize,
    /// The AI services, by their own names, in order.
    pub answers: Vec<String>,
    /// The one chosen, by its place.
    pub answers_on: usize,
    /// The search service's rows: its address, its key — each where the
    /// service chosen takes one.
    pub search_rows: Vec<EntryRow>,
    /// The AI service's rows: its key.
    pub answers_rows: Vec<EntryRow>,
    /// What the last *Keep* or *Remove* came to, if anything.
    pub said: Option<String>,
}

impl ServicesShown {
    /// Every thing a person may press, in the order it is drawn: each
    /// search service, each field of its rows with *Keep* (and *Remove*
    /// where offered), each AI service, each field of its rows likewise.
    pub fn pressables(&self) -> Vec<ServicePress> {
        let mut all: Vec<ServicePress> = (0..self.search.len()).map(ServicePress::Search).collect();
        rows_pressed(&self.search_rows, &mut all);
        all.extend((0..self.answers.len()).map(ServicePress::Answers));
        rows_pressed(&self.answers_rows, &mut all);
        all
    }

    /// The places Tab stops at, in order: each radio group once, at its
    /// choice made, and each field and button.
    pub fn stops(&self) -> Vec<ServicePress> {
        let mut all = vec![ServicePress::Search(self.search_on)];
        rows_pressed(&self.search_rows, &mut all);
        all.push(ServicePress::Answers(self.answers_on));
        rows_pressed(&self.answers_rows, &mut all);
        all
    }

    /// The row of `entry`, if it is drawn.
    pub fn row(&self, entry: Entry) -> Option<&EntryRow> {
        self.search_rows
            .iter()
            .chain(&self.answers_rows)
            .find(|row| row.entry == entry)
    }
}

/// Each field of `rows` and its buttons, in the order drawn, onto `all`.
fn rows_pressed(rows: &[EntryRow], all: &mut Vec<ServicePress>) {
    for row in rows {
        all.push(ServicePress::Field(row.entry));
        all.push(ServicePress::Keep(row.entry));
        if row.removable {
            all.push(ServicePress::Remove(row.entry));
        }
    }
}

/// `count` characters typed into a key's field, as it is drawn: dots, at
/// most [`MOST_DOTS`].
pub fn dots(count: usize) -> String {
    "\u{2022}".repeat(count.min(MOST_DOTS))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(entry: Entry, removable: bool) -> EntryRow {
        EntryRow {
            entry,
            typed: Typed::default(),
            state: String::new(),
            removable,
        }
    }

    #[test]
    fn the_pressables_and_the_stops_are_in_the_order_drawn() {
        let shown = ServicesShown {
            search: vec!["A".into(), "B".into()],
            search_on: 1,
            answers: vec!["C".into()],
            answers_on: 0,
            search_rows: vec![row(Entry::Address, false)],
            answers_rows: vec![row(Entry::AiKey, true)],
            said: None,
        };
        assert_eq!(
            shown.pressables(),
            vec![
                ServicePress::Search(0),
                ServicePress::Search(1),
                ServicePress::Field(Entry::Address),
                ServicePress::Keep(Entry::Address),
                ServicePress::Answers(0),
                ServicePress::Field(Entry::AiKey),
                ServicePress::Keep(Entry::AiKey),
                ServicePress::Remove(Entry::AiKey),
            ]
        );
        assert_eq!(
            shown.stops(),
            vec![
                ServicePress::Search(1),
                ServicePress::Field(Entry::Address),
                ServicePress::Keep(Entry::Address),
                ServicePress::Answers(0),
                ServicePress::Field(Entry::AiKey),
                ServicePress::Keep(Entry::AiKey),
                ServicePress::Remove(Entry::AiKey),
            ]
        );
        assert!(shown.row(Entry::SearchKey).is_none());
        assert!(shown.row(Entry::AiKey).is_some_and(|row| row.removable));
    }

    #[test]
    fn a_key_is_drawn_as_dots_never_past_their_most() {
        assert_eq!(dots(0), "");
        assert_eq!(dots(3), "\u{2022}\u{2022}\u{2022}");
        assert_eq!(dots(500).chars().count(), MOST_DOTS);
        assert!(Entry::SearchKey.masked() && Entry::AiKey.masked());
        assert!(!Entry::Address.masked());
    }
}
