/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*' third section, **Search and answers** (ADR 0106, queue item
//! 905, `docs/design/shell.md`, *Settings*), built into the first screen's
//! document under *alo at work* ([`crate::settings_document`]) from what
//! the browser process sent ([`ServicesShown`]).
//!
//! # What is drawn
//!
//! The section's heading and one quiet line; then one white box, as *alo
//! at work*'s, of rows ruled off from each other: **Search with**, a radio
//! for each search service; under it, where the service chosen takes one,
//! its **Address** and its **Search key**; then **Answers by**, a radio for
//! each AI service, and its **AI key**. A field's row holds the field, 28
//! tall, *Keep*, and *Remove* where a person's own key is kept, and under
//! them one quiet sentence: whose key is used, or the address kept. Under
//! the box, what the last *Keep* or *Remove* came to.
//!
//! **A service's own name is drawn here, in its radio, and nowhere else**
//! (ADR 0106 § 5): each option is of the class `svc`, and
//! `no_providers_name_on_screen.rs` allows a name inside one alone.
//!
//! # A key is dots
//!
//! A key's field holds as many dots as characters were typed
//! ([`crate::settings_services::dots`]), hidden from an agent, which reads
//! the field by its name: the key is never in this process.
//!
//! # Where the keyboard is
//!
//! On a radio group at its choice made, a field, or a button: ringed, by
//! the class `keyed`, once Tab has put it there. A field with the keyboard
//! is `efield on`, its caret at its end.
//!
//! # What a press is on
//!
//! Each option, field and button by its place among them all
//! ([`ServicesShown::pressables`]), claimed as [`crate::conversation_claim`]'s
//! `Service` and decided by the browser process against what it painted.

use crate::settings_services::{Entry, EntryRow, ServicePress, ServicesShown};
use crate::start_document::{Unbuilt, append, attribute, element};
use alo_dom::{Document, NodeId};

/// What the section, and the region it is, are named.
pub const TITLE: &str = "Search and answers";

/// What the section says under its heading.
pub const ABOUT: &str = "The services alo searches and writes its answers with. A change is used \
from alo\u{2019}s next start.";

/// What the search services' row and radio group are named.
pub const SEARCH_WITH: &str = "Search with";

/// What the AI services' row and radio group are named.
pub const ANSWERS_BY: &str = "Answers by";

/// What a field's *Keep* says.
pub const KEEP: &str = "Keep";

/// What a field's *Remove* says.
pub const REMOVE: &str = "Remove";

/// The section for `shown`, the keyboard on `keyed`, made for *Settings*
/// to put under *alo at work*.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no state can
/// cause.
pub fn make(
    document: &mut Document,
    shown: &ServicesShown,
    keyed: Option<ServicePress>,
) -> Result<NodeId, Unbuilt> {
    let section = element(document, "section")?;
    attribute(document, section, "class", "pgroup")?;
    attribute(document, section, "aria-labelledby", "services-title")?;
    let heading = words(document, "h2", "pname", TITLE)?;
    attribute(document, heading, "id", "services-title")?;
    append(document, section, heading)?;
    let about = words(document, "p", "pabout", ABOUT)?;
    append(document, section, about)?;
    let rows = element(document, "div")?;
    attribute(document, rows, "class", "limits")?;
    let group = chooser(
        document,
        ("search", SEARCH_WITH),
        (&shown.search, shown.search_on),
        |at| keyed == Some(ServicePress::Search(at)),
    )?;
    append(document, rows, group)?;
    for row in &shown.search_rows {
        let made = entry_row(document, row, keyed)?;
        append(document, rows, made)?;
    }
    let group = chooser(
        document,
        ("answers", ANSWERS_BY),
        (&shown.answers, shown.answers_on),
        |at| keyed == Some(ServicePress::Answers(at)),
    )?;
    append(document, rows, group)?;
    for row in &shown.answers_rows {
        let made = entry_row(document, row, keyed)?;
        append(document, rows, made)?;
    }
    append(document, section, rows)?;
    if let Some(said) = &shown.said {
        let line = words(document, "p", "psaid", said)?;
        attribute(document, line, "role", "status")?;
        append(document, section, line)?;
    }
    Ok(section)
}

/// One chooser's row: its name, and a radio for each of `names`, the one
/// at `on` checked, ringed where `keyed` says.
fn chooser(
    document: &mut Document,
    (group_name, label): (&str, &str),
    (names, on): (&[String], usize),
    keyed: impl Fn(usize) -> bool,
) -> Result<NodeId, Unbuilt> {
    let row = element(document, "div")?;
    attribute(document, row, "class", "lrow")?;
    let id = format!("{group_name}-name");
    let name = words(document, "p", "lname", label)?;
    attribute(document, name, "id", &id)?;
    append(document, row, name)?;
    let group = element(document, "div")?;
    attribute(document, group, "class", "askopts")?;
    attribute(document, group, "role", "radiogroup")?;
    attribute(document, group, "aria-labelledby", &id)?;
    for (at, service) in names.iter().enumerate() {
        let chosen = at == on;
        let option = element(document, "div")?;
        let class = match (chosen, keyed(at)) {
            (true, true) => "svc on keyed",
            (true, false) => "svc on",
            (false, _) => "svc",
        };
        attribute(document, option, "class", class)?;
        let radio = element(document, "input")?;
        attribute(document, radio, "class", "dot")?;
        attribute(document, radio, "type", "radio")?;
        attribute(document, radio, "name", group_name)?;
        attribute(document, radio, "aria-label", service)?;
        if chosen {
            attribute(document, radio, "checked", "")?;
        }
        append(document, option, radio)?;
        let shown = words(document, "span", "aname", service)?;
        attribute(document, shown, "aria-hidden", "true")?;
        append(document, option, shown)?;
        append(document, group, option)?;
    }
    append(document, row, group)?;
    Ok(row)
}

/// One field's row: its name, the field, *Keep* and *Remove* where
/// offered, and the sentence under them.
fn entry_row(
    document: &mut Document,
    row: &EntryRow,
    keyed: Option<ServicePress>,
) -> Result<NodeId, Unbuilt> {
    let entry = row.entry;
    let made = element(document, "div")?;
    attribute(document, made, "class", "lrow erow")?;
    let id = match entry {
        Entry::Address => "address-name",
        Entry::SearchKey => "search-key-name",
        Entry::AiKey => "ai-key-name",
    };
    let name = words(document, "p", "lname", entry.name())?;
    attribute(document, name, "id", id)?;
    append(document, made, name)?;
    let tools = element(document, "div")?;
    attribute(document, tools, "class", "ebox")?;
    let field = element(document, "div")?;
    let class = match (row.typed.focused, keyed == Some(ServicePress::Field(entry))) {
        (true, true) => "efield on keyed",
        (true, false) => "efield on",
        (false, true) => "efield keyed",
        (false, false) => "efield",
    };
    attribute(document, field, "class", class)?;
    attribute(document, field, "role", "textbox")?;
    attribute(document, field, "aria-labelledby", id)?;
    let caret = |document: &mut Document| -> Result<(), Unbuilt> {
        if row.typed.focused {
            let caret = element(document, "span")?;
            attribute(document, caret, "class", "ecaret")?;
            attribute(document, caret, "aria-hidden", "true")?;
            append(document, field, caret)?;
        }
        Ok(())
    };
    if row.typed.shown.is_empty() {
        // An empty field's caret stands before its placeholder.
        caret(document)?;
        attribute(document, field, "aria-placeholder", entry.placeholder())?;
        let placeholder = words(document, "span", "eplace", entry.placeholder())?;
        attribute(document, placeholder, "aria-hidden", "true")?;
        append(document, field, placeholder)?;
    } else {
        let text = words(document, "span", "etext", &row.typed.shown)?;
        if entry.masked() {
            // Dots, for a person: an agent reads the field by its name.
            attribute(document, text, "aria-hidden", "true")?;
        }
        append(document, field, text)?;
        caret(document)?;
    }
    append(document, tools, field)?;
    let keep = button(
        document,
        (KEEP, entry.keep_named()),
        keyed == Some(ServicePress::Keep(entry)),
    )?;
    append(document, tools, keep)?;
    if row.removable {
        let remove = button(
            document,
            (REMOVE, entry.remove_named()),
            keyed == Some(ServicePress::Remove(entry)),
        )?;
        append(document, tools, remove)?;
    }
    append(document, made, tools)?;
    let state = words(document, "p", "estate", &row.state)?;
    append(document, made, state)?;
    Ok(made)
}

/// A button saying `said`, named `named`, ringed when `keyed`.
fn button(
    document: &mut Document,
    (said, named): (&str, &str),
    keyed: bool,
) -> Result<NodeId, Unbuilt> {
    let made = words(
        document,
        "button",
        if keyed { "kbtn keyed" } else { "kbtn" },
        said,
    )?;
    attribute(document, made, "type", "button")?;
    attribute(document, made, "aria-label", named)?;
    Ok(made)
}

/// An element `local` of `class` holding `said` as a text node.
fn words(document: &mut Document, local: &str, class: &str, said: &str) -> Result<NodeId, Unbuilt> {
    let made = element(document, local)?;
    attribute(document, made, "class", class)?;
    let text = document.create_text_node(said);
    append(document, made, text)?;
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_services::{Typed, dots};
    use crate::start::{SettingsKeyed, Start};
    use crate::start_document::build;

    pub(crate) fn shown() -> ServicesShown {
        ServicesShown {
            search: vec!["Search One".into(), "Search Two".into()],
            search_on: 1,
            answers: vec!["Answer One".into()],
            answers_on: 0,
            search_rows: vec![EntryRow {
                entry: Entry::Address,
                typed: Typed {
                    shown: "https://search.example.org".into(),
                    focused: false,
                },
                state: "Kept.".into(),
                removable: false,
            }],
            answers_rows: vec![EntryRow {
                entry: Entry::AiKey,
                typed: Typed {
                    shown: dots(5),
                    focused: true,
                },
                state: "Your own key, kept in this Mac\u{2019}s keychain.".into(),
                removable: true,
            }],
            said: Some("Kept.".into()),
        }
    }

    fn settings(keyed: Option<SettingsKeyed>) -> Document {
        build(&Start {
            settings: true,
            settings_keyed: keyed,
            services: Some(shown()),
            ..Start::default()
        })
        .expect("built")
    }

    fn classes(document: &Document) -> Vec<String> {
        document
            .descendants(document.root())
            .filter_map(|node| document.attribute_by_name(node, "class"))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn the_section_stands_last_with_each_chooser_its_rows_and_what_was_said() {
        let document = settings(None);
        let text = document.text_content(document.root());
        let work = text.find("alo at work").expect("alo at work");
        let here = text.find(TITLE).expect("the section");
        assert!(work < here, "{text}");
        for said in [
            SEARCH_WITH,
            ANSWERS_BY,
            "Address",
            "AI key",
            KEEP,
            REMOVE,
            "Kept.",
        ] {
            assert!(text.contains(said), "{said} in {text}");
        }
        let found = classes(&document);
        let svc: Vec<&String> = found
            .iter()
            .filter(|class| class.starts_with("svc"))
            .collect();
        assert_eq!(svc, ["svc", "svc on", "svc on"]);
        assert_eq!(
            found
                .iter()
                .filter(|class| class.starts_with("kbtn"))
                .count(),
            3,
            "Keep, Keep and Remove"
        );
        assert!(found.iter().any(|class| class == "efield on"));
        assert!(found.iter().any(|class| class == "ecaret"));
        // The key is dots, hidden from an agent; the address is read.
        let hidden: Vec<String> = document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some("etext"))
            .map(|node| {
                format!(
                    "{}:{}",
                    document.text_content(node),
                    document
                        .attribute_by_name(node, "aria-hidden")
                        .unwrap_or("")
                )
            })
            .collect();
        assert_eq!(
            hidden,
            [
                "https://search.example.org:".to_owned(),
                format!("{}:true", dots(5))
            ]
        );
    }

    #[test]
    fn the_keyboard_rings_the_one_thing_it_is_on() {
        let keyed = |on| {
            classes(&settings(Some(SettingsKeyed::Service(on))))
                .into_iter()
                .filter(|class| class.contains("keyed"))
                .collect::<Vec<_>>()
        };
        assert_eq!(keyed(ServicePress::Search(1)), ["svc on keyed"]);
        assert_eq!(keyed(ServicePress::Answers(0)), ["svc on keyed"]);
        assert_eq!(keyed(ServicePress::Field(Entry::Address)), ["efield keyed"]);
        assert_eq!(
            keyed(ServicePress::Field(Entry::AiKey)),
            ["efield on keyed"]
        );
        assert_eq!(keyed(ServicePress::Remove(Entry::AiKey)), ["kbtn keyed"]);
        assert!(
            classes(&settings(None))
                .iter()
                .all(|class| !class.contains("keyed"))
        );
    }
}
