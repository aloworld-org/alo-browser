/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings › Search and answers*, as the browser process holds it (ADR
//! 0106, queue item 905, `docs/design/shell.md`, *Settings*): which
//! services a person chose, what they are typing into the section's
//! fields, and what is to be kept.
//!
//! # Choices
//!
//! A service chosen is the person's line in `settings.txt` — kept by the
//! window ([`ToDo::KeepServices`]) — and is used from alo's next start
//! (§ 3). What is shown chosen is the person's line over the application's
//! default over Brave and `OpenAI`.
//!
//! # Fields
//!
//! Three, each shown only where the service chosen takes it: a `SearXNG`'s
//! **Address**, the **Search key** and the **AI key**. The keyboard is in
//! one once Tab or a press put it there; what is typed is put at its end,
//! ⌫ takes the last character, ⌘V puts the clipboard's first line in,
//! Escape empties it, and Return keeps it, as its *Keep* does. (The caret
//! stands at its end: moving it, and selecting, are queued, 909.)
//!
//! # A key is never said
//!
//! What is typed into a key's field is held here alone, in memory, and the
//! renderer is sent only as many dots ([`ServicesShown`]). *Keep* hands it
//! to the window, once, to put in the person's keychain
//! ([`crate::service_keys::Keys::keep`]), and empties the field; leaving
//! *Settings* empties it too. This type's `Debug` says nothing typed.

use crate::service_keys::Kept;
use crate::services::{AiChoice, Keyed, Named, SearchChoice, Services};
use alo_renderer::settings_services::{
    Entry, EntryRow, LONGEST_TYPED, ServicePress, ServicesShown, Typed, dots,
};
use core::fmt;

/// What the window is to do for the section, once.
#[derive(Clone, PartialEq, Eq)]
pub enum ToDo {
    /// Write the settings file: a service, or an address, was chosen.
    KeepServices,
    /// Put this key in the person's keychain as theirs for this service.
    KeepKey(Keyed, String),
    /// Take the person's own key for this service out of their keychain.
    RemoveKey(Keyed),
}

impl fmt::Debug for ToDo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KeepServices => f.write_str("KeepServices"),
            Self::KeepKey(keyed, _) => write!(f, "KeepKey({keyed:?}, never said)"),
            Self::RemoveKey(keyed) => write!(f, "RemoveKey({keyed:?})"),
        }
    }
}

/// Which key a service is asked with, or why that cannot be told.
pub type KeyState = Result<Kept, String>;

/// The section's state.
#[derive(Clone, PartialEq, Eq)]
pub struct ServicesPage {
    /// What the person's settings file names.
    named: Named,
    /// What the application names, under it.
    defaults: Named,
    /// Whose search key and AI key are used.
    kept: (KeyState, KeyState),
    /// The field the keyboard is in, if one.
    typing: Option<Entry>,
    /// What is typed in each field.
    address: String,
    search_key: String,
    ai_key: String,
    /// What the last *Keep* or *Remove* came to.
    said: Option<String>,
    /// What the window is to do.
    to_do: Vec<ToDo>,
}

impl fmt::Debug for ServicesPage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServicesPage")
            .field("named", &self.named)
            .field("defaults", &self.defaults)
            .field("kept", &self.kept)
            .field("typing", &self.typing)
            .field("said", &self.said)
            .finish_non_exhaustive()
    }
}

impl Default for ServicesPage {
    fn default() -> Self {
        Self::of(
            Named::default(),
            Named::default(),
            (Ok(Kept::None), Ok(Kept::None)),
        )
    }
}

/// Whose key is used, as the section says it under a key's field.
pub fn state_of(kept: &KeyState) -> String {
    match kept {
        Ok(Kept::Yours) => "Your own key is used, kept in this Mac\u{2019}s keychain.".to_owned(),
        Ok(Kept::Carried) => "The key this application carries is used.".to_owned(),
        Ok(Kept::Older) => "A key in this Mac\u{2019}s keychain is used.".to_owned(),
        Ok(Kept::None) => "No key yet.".to_owned(),
        Err(why) => format!("Its key could not be read: {why}."),
    }
}

impl ServicesPage {
    /// The person's `named` lines over the application's `defaults`, its
    /// keys `kept` as they are now (the search service's, then the AI
    /// service's).
    pub fn of(named: Named, defaults: Named, kept: (KeyState, KeyState)) -> Self {
        Self {
            named,
            defaults,
            kept,
            typing: None,
            address: String::new(),
            search_key: String::new(),
            ai_key: String::new(),
            said: None,
            to_do: Vec::new(),
        }
    }

    /// The same section, over the application's `defaults`, its keys
    /// `kept` as they are now — the person's lines as they were.
    #[must_use]
    pub fn under(mut self, defaults: Named, kept: (KeyState, KeyState)) -> Self {
        self.defaults = defaults;
        self.kept = kept;
        self
    }

    /// What the person's settings file is to name.
    pub fn named(&self) -> &Named {
        &self.named
    }

    /// The services shown chosen.
    pub fn services(&self) -> Services {
        self.named.clone().over(&self.defaults).services()
    }

    /// The fields the service chosen for search takes, in order.
    fn search_entries(&self) -> Vec<Entry> {
        match self.services().search {
            SearchChoice::Brave => vec![Entry::SearchKey],
            SearchChoice::Searxng => vec![Entry::Address],
        }
    }

    /// Every field drawn.
    fn entries(&self) -> Vec<Entry> {
        let mut all = self.search_entries();
        all.push(Entry::AiKey);
        all
    }

    /// What is typed in `entry`.
    fn typed_mut(&mut self, entry: Entry) -> &mut String {
        match entry {
            Entry::Address => &mut self.address,
            Entry::SearchKey => &mut self.search_key,
            Entry::AiKey => &mut self.ai_key,
        }
    }

    /// The key `entry` holds, if it is a key's field.
    fn keyed(&self, entry: Entry) -> Option<Keyed> {
        match entry {
            Entry::Address => None,
            Entry::SearchKey => self.services().search.keyed(),
            Entry::AiKey => Some(self.services().answers.keyed()),
        }
    }

    /// Whose key `keyed`'s service is asked with.
    fn kept_of(&self, keyed: Keyed) -> &KeyState {
        if keyed == self.services().answers.keyed() {
            &self.kept.1
        } else {
            &self.kept.0
        }
    }

    /// The section as it is drawn: never a key, only its dots.
    pub fn shown(&self) -> ServicesShown {
        let services = self.services();
        let row = |entry: Entry| {
            let typed = match entry {
                Entry::Address => self.address.clone(),
                Entry::SearchKey => dots(self.search_key.chars().count()),
                Entry::AiKey => dots(self.ai_key.chars().count()),
            };
            let (state, removable) = match self.keyed(entry) {
                Some(keyed) => {
                    let kept = self.kept_of(keyed);
                    (state_of(kept), matches!(kept, Ok(Kept::Yours)))
                }
                None => (
                    services.searxng.as_ref().map_or_else(
                        || "No address yet.".to_owned(),
                        |at| format!("Searches go to {}.", at.serialised),
                    ),
                    false,
                ),
            };
            EntryRow {
                entry,
                typed: Typed {
                    // Before anything is typed, the address kept.
                    shown: if entry == Entry::Address && self.typing != Some(entry) {
                        services
                            .searxng
                            .as_ref()
                            .map(|at| at.serialised.clone())
                            .filter(|_| typed.is_empty())
                            .unwrap_or(typed)
                    } else {
                        typed
                    },
                    focused: self.typing == Some(entry),
                },
                state,
                removable,
            }
        };
        ServicesShown {
            search: SearchChoice::ALL
                .iter()
                .map(|choice| choice.name().to_owned())
                .collect(),
            search_on: SearchChoice::ALL
                .iter()
                .position(|choice| *choice == services.search)
                .unwrap_or_default(),
            answers: AiChoice::ALL
                .iter()
                .map(|choice| choice.name().to_owned())
                .collect(),
            answers_on: AiChoice::ALL
                .iter()
                .position(|choice| *choice == services.answers)
                .unwrap_or_default(),
            search_rows: self.search_entries().into_iter().map(row).collect(),
            answers_rows: vec![row(Entry::AiKey)],
            said: self.said.clone(),
        }
    }

    /// The field the keyboard is in, if one.
    pub fn typing(&self) -> Option<Entry> {
        self.typing
    }

    /// The keyboard into `entry`, or out of every field: whether it moved.
    pub fn type_in(&mut self, entry: Option<Entry>) -> bool {
        let entry = entry.filter(|entry| self.entries().contains(entry));
        if self.typing == entry {
            return false;
        }
        self.typing = entry;
        // An address is edited from the one kept.
        if entry == Some(Entry::Address) && self.address.is_empty() {
            self.address = self
                .services()
                .searxng
                .map(|at| at.serialised)
                .unwrap_or_default();
        }
        true
    }

    /// `pressed`, a press or Space or Return on it: whether anything shown
    /// changed.
    pub fn press(&mut self, pressed: ServicePress) -> bool {
        match pressed {
            ServicePress::Search(at) => SearchChoice::ALL
                .get(at)
                .is_some_and(|choice| self.choose_search(*choice)),
            ServicePress::Answers(at) => AiChoice::ALL.get(at).is_some_and(|choice| {
                let changed = self.services().answers != *choice || self.named.answers.is_none();
                self.named.answers = Some(*choice);
                if changed {
                    self.to_do.push(ToDo::KeepServices);
                    self.said = None;
                }
                changed
            }),
            ServicePress::Field(entry) => self.type_in(Some(entry)),
            ServicePress::Keep(entry) => self.keep(entry),
            ServicePress::Remove(entry) => match self.keyed(entry) {
                Some(keyed) => {
                    self.to_do.push(ToDo::RemoveKey(keyed));
                    true
                }
                None => false,
            },
        }
    }

    /// The search service `choice` chosen.
    fn choose_search(&mut self, choice: SearchChoice) -> bool {
        let changed = self.services().search != choice || self.named.search.is_none();
        self.named.search = Some(choice);
        if changed {
            self.to_do.push(ToDo::KeepServices);
            self.said = None;
            // A field the new service does not take is left.
            if self
                .typing
                .is_some_and(|entry| !self.entries().contains(&entry))
            {
                self.typing = None;
            }
        }
        changed
    }

    /// The arrows on a chooser: the next search or AI service when `on`,
    /// or the one before, round. Whether another was chosen, and what of
    /// the section the keyboard is now on.
    pub fn step(&mut self, on_search: bool, on: bool) -> (bool, ServicePress) {
        let services = self.services();
        if on_search {
            let all = SearchChoice::ALL;
            let at = all
                .iter()
                .position(|each| *each == services.search)
                .unwrap_or(0);
            let next = if on {
                (at + 1) % all.len()
            } else {
                (at + all.len() - 1) % all.len()
            };
            let chosen = all
                .get(next)
                .is_some_and(|choice| self.choose_search(*choice));
            (chosen, ServicePress::Search(next))
        } else {
            let all = AiChoice::ALL;
            let at = all
                .iter()
                .position(|each| *each == services.answers)
                .unwrap_or(0);
            let next = if on {
                (at + 1) % all.len()
            } else {
                (at + all.len() - 1) % all.len()
            };
            let chosen = next != at && self.press(ServicePress::Answers(next));
            (chosen, ServicePress::Answers(next))
        }
    }

    /// *Keep* on `entry`, or Return in it.
    fn keep(&mut self, entry: Entry) -> bool {
        let typed = core::mem::take(self.typed_mut(entry));
        match self.keyed(entry) {
            None => {
                if let Ok(at) = crate::services::address(&typed) {
                    self.named.searxng = Some(at);
                    self.to_do.push(ToDo::KeepServices);
                    self.said = Some(
                        "The address is kept. alo searches there from its next start.".to_owned(),
                    );
                    self.typing = None;
                } else {
                    *self.typed_mut(entry) = typed;
                    self.said = Some(
                        "That is not a site\u{2019}s address: it begins https:// or http://."
                            .to_owned(),
                    );
                }
            }
            Some(_) if typed.trim().is_empty() => {
                self.said = Some("Paste or type a key first.".to_owned());
            }
            Some(keyed) => {
                self.to_do.push(ToDo::KeepKey(keyed, typed));
                self.typing = None;
            }
        }
        true
    }

    /// `text` typed into the field the keyboard is in, at its end: whether
    /// there was one to take it. Its line's end, and anything past the
    /// longest a field holds, are left out.
    pub fn put(&mut self, text: &str) -> bool {
        let Some(entry) = self.typing else {
            return false;
        };
        let line = text.lines().next().unwrap_or_default();
        let field = self.typed_mut(entry);
        let room = LONGEST_TYPED.saturating_sub(field.chars().count());
        let before = field.len();
        field.extend(line.chars().filter(|c| !c.is_control()).take(room));
        field.len() != before
    }

    /// ⌫ in the field the keyboard is in: its last character taken out.
    pub fn erase(&mut self) -> bool {
        self.typing
            .is_some_and(|entry| self.typed_mut(entry).pop().is_some())
    }

    /// Escape in the field the keyboard is in: whether there was something
    /// to empty — with nothing typed, Escape is *Settings*' own.
    pub fn empty(&mut self) -> bool {
        let Some(entry) = self.typing else {
            return false;
        };
        let field = self.typed_mut(entry);
        let had = !field.is_empty();
        field.clear();
        had
    }

    /// *Settings* closed: the keyboard out of every field, and nothing
    /// typed into a key's kept in memory.
    pub fn leave(&mut self) {
        self.typing = None;
        self.search_key.clear();
        self.ai_key.clear();
        self.address.clear();
        self.said = None;
    }

    /// What the window is to do, once.
    pub fn take_to_do(&mut self) -> Vec<ToDo> {
        core::mem::take(&mut self.to_do)
    }

    /// Whose key `keyed`'s service is asked with now, and what is to be
    /// said of what was just done, if anything.
    pub fn kept_now(&mut self, keyed: Keyed, kept: KeyState, said: Option<String>) {
        if keyed == self.services().answers.keyed() {
            self.kept.1 = kept;
        } else {
            self.kept.0 = kept;
        }
        if said.is_some() {
            self.said = said;
        }
    }

    /// Say `said` under the section.
    pub fn say(&mut self, said: String) {
        self.said = Some(said);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> ServicesPage {
        ServicesPage::of(
            Named::default(),
            Named::default(),
            (Ok(Kept::Carried), Ok(Kept::None)),
        )
    }

    #[test]
    fn brave_and_openai_are_shown_chosen_with_their_keys_and_no_address() {
        let shown = page().shown();
        assert_eq!(shown.search, ["Brave Search", "SearXNG"]);
        assert_eq!(shown.search_on, 0);
        assert_eq!(shown.answers, ["OpenAI"]);
        let rows: Vec<(Entry, String, bool)> = shown
            .search_rows
            .iter()
            .chain(&shown.answers_rows)
            .map(|row| (row.entry, row.state.clone(), row.removable))
            .collect();
        assert_eq!(
            rows,
            [
                (
                    Entry::SearchKey,
                    "The key this application carries is used.".to_owned(),
                    false
                ),
                (Entry::AiKey, "No key yet.".to_owned(), false),
            ]
        );
    }

    #[test]
    fn a_key_typed_is_dots_on_the_wire_kept_once_and_never_said() {
        let mut page = page();
        assert!(page.press(ServicePress::Field(Entry::SearchKey)));
        assert!(page.put("not-a-real-key-42\nsecond line"));
        assert!(page.erase());
        let shown = page.shown();
        let row = shown.row(Entry::SearchKey).expect("the row");
        assert_eq!(row.typed.shown, dots(16));
        assert!(row.typed.focused);
        assert!(!format!("{shown:?}").contains("not-a-real"));
        assert!(!format!("{page:?}").contains("not-a-real"));
        assert!(page.press(ServicePress::Keep(Entry::SearchKey)));
        let to_do = page.take_to_do();
        assert_eq!(
            to_do,
            [ToDo::KeepKey(
                Keyed::BraveSearch,
                "not-a-real-key-4".to_owned()
            )]
        );
        assert!(!format!("{to_do:?}").contains("not-a-real"));
        assert!(page.take_to_do().is_empty(), "once");
        assert_eq!(page.typing(), None, "the keyboard out of it");
        assert_eq!(
            page.shown()
                .row(Entry::SearchKey)
                .map(|row| row.typed.shown.clone()),
            Some(String::new())
        );
        // Nothing typed: nothing to keep, and said.
        assert!(page.press(ServicePress::Keep(Entry::AiKey)));
        assert!(page.take_to_do().is_empty());
        assert_eq!(
            page.shown().said.as_deref(),
            Some("Paste or type a key first.")
        );
        // Kept, it may be removed.
        page.kept_now(
            Keyed::BraveSearch,
            Ok(Kept::Yours),
            Some("Kept.".to_owned()),
        );
        assert!(
            page.shown()
                .row(Entry::SearchKey)
                .is_some_and(|row| row.removable)
        );
        assert!(page.press(ServicePress::Remove(Entry::SearchKey)));
        assert_eq!(page.take_to_do(), [ToDo::RemoveKey(Keyed::BraveSearch)]);
        // Leaving Settings forgets whatever was typed.
        assert!(page.press(ServicePress::Field(Entry::AiKey)));
        assert!(page.put("half-a-key"));
        page.leave();
        assert_eq!(
            page.shown()
                .row(Entry::AiKey)
                .map(|row| row.typed.shown.clone()),
            Some(String::new())
        );
    }

    #[test]
    fn searxng_chosen_takes_an_address_not_a_key_and_an_odd_one_is_said() {
        let mut page = page();
        assert!(page.press(ServicePress::Search(1)));
        assert_eq!(page.take_to_do(), [ToDo::KeepServices]);
        assert!(!page.press(ServicePress::Search(1)), "chosen already");
        let shown = page.shown();
        assert_eq!(shown.search_on, 1);
        assert_eq!(
            shown
                .search_rows
                .iter()
                .map(|row| row.entry)
                .collect::<Vec<_>>(),
            [Entry::Address]
        );
        assert_eq!(shown.search_rows[0].state, "No address yet.");
        assert!(!page.type_in(Some(Entry::SearchKey)), "not drawn");
        assert!(page.press(ServicePress::Field(Entry::Address)));
        assert!(page.put("search.example.org"));
        assert!(page.press(ServicePress::Keep(Entry::Address)));
        assert!(page.take_to_do().is_empty());
        assert_eq!(
            page.shown().said.as_deref(),
            Some("That is not a site\u{2019}s address: it begins https:// or http://.")
        );
        assert!(page.empty(), "Escape empties it");
        assert!(page.put("https://search.example.org"));
        assert!(page.press(ServicePress::Keep(Entry::Address)));
        assert_eq!(page.take_to_do(), [ToDo::KeepServices]);
        assert_eq!(
            page.named().lines(),
            "search searxng\nsearxng https://search.example.org/\n"
        );
        assert_eq!(
            page.shown().search_rows[0].state,
            "Searches go to https://search.example.org/."
        );
        // The arrows on the chooser, round.
        assert_eq!(page.step(true, true), (true, ServicePress::Search(0)));
        assert_eq!(page.step(true, false), (true, ServicePress::Search(1)));
        assert_eq!(
            page.step(false, true),
            (false, ServicePress::Answers(0)),
            "one, round"
        );
    }

    #[test]
    fn whose_key_is_said_in_words_naming_no_service() {
        for kept in [
            Ok(Kept::Yours),
            Ok(Kept::Carried),
            Ok(Kept::Older),
            Ok(Kept::None),
            Err("the keychain is locked".to_owned()),
        ] {
            let said = state_of(&kept).to_lowercase();
            assert!(
                !said.contains("brave") && !said.contains("openai"),
                "{said}"
            );
        }
    }
}
