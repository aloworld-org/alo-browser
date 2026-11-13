/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*' fifth section, **Screen readers** (ADR 0115, *Phase
//! (c)'s guard*, (a); queue item 989), built into the first screen's
//! document under *Privacy* ([`crate::settings_document`]) from the choice
//! the browser process sent ([`Bus`]).
//!
//! # What is drawn
//!
//! The section's heading and one quiet line; then one white box, as
//! *Privacy*'s, of one row: **What they may do**, its
//! name over three choices one under another — *Read and act* (the
//! default), *Read only* with the quiet line saying nothing pressed is
//! done, and *Off* with the line saying a screen reader will not be able
//! to read alo. Each choice a radio drawn as *Privacy*'s are, in the ink,
//! by the same function ([`crate::settings_privacy_document`]'s `group`).
//!
//! # What an agent reads
//!
//! ADR 0002: a region named [`TITLE`]; a radio group named by its row,
//! holding a radio named by each choice, the one made checked; the line
//! under a choice read after it, as text, and named its description.
//! Never a coordinate.
//!
//! # Where the keyboard is
//!
//! On the radio group, at its choice made: ringed, by the class `keyed`,
//! once Tab has put it there.
//!
//! # What a press is on
//!
//! Each choice — its radio, its words, the room around them — is of the
//! class [`CHOICE`] (`privopt busopt on` when made), claimed by its place
//! among them ([`Bus::ALL`]) as [`crate::conversation_claim`]'s `Bus`, and
//! decided by the browser process against what it painted. It is drawn as
//! *Privacy*'s choices are, being of the class `privopt` too.

use crate::settings_bus::Bus;
use crate::settings_privacy_document::{Choice, group, words};
use crate::start::SettingsKeyed;
use crate::start_document::{Unbuilt, append, attribute, element};
use alo_dom::{Document, NodeId};

/// What the section, and the region it is, are named.
pub const TITLE: &str = "Screen readers";

/// What the section says under its heading.
pub const ABOUT: &str = "What a screen reader, or any other program you let read the screen, \
may read of alo and do in it. A choice is used at once.";

/// What its row and radio group are named.
pub const PROGRAMS: &str = "What they may do";

/// The class each of its choices is of: drawn as *Privacy*'s, claimed as
/// its own.
pub const CHOICE: &str = "privopt busopt";

/// The section for `bus`, the keyboard on `keyed`, made for *Settings* to
/// put under *Privacy*.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no choice can
/// cause.
pub fn make(
    document: &mut Document,
    bus: Bus,
    keyed: Option<SettingsKeyed>,
) -> Result<NodeId, Unbuilt> {
    let section = element(document, "section")?;
    attribute(document, section, "class", "pgroup")?;
    attribute(document, section, "aria-labelledby", "bus-title")?;
    let heading = words(document, "h2", "pname", TITLE)?;
    attribute(document, heading, "id", "bus-title")?;
    append(document, section, heading)?;
    let about = words(document, "p", "pabout", ABOUT)?;
    append(document, section, about)?;
    let rows = element(document, "div")?;
    attribute(document, rows, "class", "limits")?;
    let choices: Vec<Choice> = Bus::ALL
        .into_iter()
        .map(|each| Choice {
            said: each.said(),
            note: each.note(),
            made: each == bus,
            default: each == Bus::default(),
        })
        .collect();
    let row = group(
        document,
        ("bus", PROGRAMS),
        &choices,
        (keyed == Some(SettingsKeyed::Bus), CHOICE),
    )?;
    append(document, rows, row)?;
    append(document, section, rows)?;
    Ok(section)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::start::Start;
    use crate::start_document::build;

    fn settings(bus: Option<Bus>, keyed: Option<SettingsKeyed>) -> Document {
        build(&Start {
            settings: true,
            settings_keyed: keyed,
            bus,
            ..Start::default()
        })
        .expect("built")
    }

    /// Every element's class, in order.
    fn classes(document: &Document) -> Vec<String> {
        document
            .descendants(document.root())
            .filter_map(|node| document.attribute_by_name(node, "class"))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn three_radios_named_by_their_words_the_one_made_checked() {
        let document = settings(Some(Bus::ReadOnly), None);
        let of = |node, name| document.attribute_by_name(node, name).map(str::to_owned);
        let radios: Vec<(Option<String>, bool)> = document
            .descendants(document.root())
            .filter(|&node| of(node, "name").as_deref() == Some("bus"))
            .map(|node| (of(node, "aria-label"), of(node, "checked").is_some()))
            .collect();
        assert_eq!(
            radios,
            vec![
                (Some("Read and act".into()), false),
                (Some("Read only".into()), true),
                (Some("Off".into()), false),
            ]
        );
        assert_eq!(
            classes(&document)
                .into_iter()
                .filter(|class| class.starts_with(CHOICE))
                .collect::<Vec<_>>(),
            [CHOICE, "privopt busopt on", CHOICE]
        );
        let text = document.text_content(document.root());
        for said in [
            TITLE,
            PROGRAMS,
            "A screen reader will not be able to read alo.",
        ] {
            assert!(text.contains(said), "{said} in {text}");
        }
        // The group is named by its row, as *Privacy*'s are.
        let group = document
            .descendants(document.root())
            .find(|&node| of(node, "aria-labelledby").as_deref() == Some("bus-name"))
            .expect("the radio group");
        assert_eq!(of(group, "role").as_deref(), Some("radiogroup"));
    }

    #[test]
    fn the_keyboard_rings_the_choice_made_and_none_draws_no_section() {
        let keyed = settings(Some(Bus::Off), Some(SettingsKeyed::Bus));
        assert_eq!(
            classes(&keyed)
                .into_iter()
                .filter(|class| class.contains("keyed"))
                .collect::<Vec<_>>(),
            ["privopt busopt on keyed"]
        );
        let none = settings(None, None);
        assert!(!document_says(&none, TITLE));
        assert!(document_says(&settings(Some(Bus::default()), None), TITLE));
    }

    fn document_says(document: &Document, said: &str) -> bool {
        document.text_content(document.root()).contains(said)
    }
}
