/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*' sixth section, **Updates** (ADR 0116 § 4, queue item 721),
//! built into the first screen's document under *Screen readers*
//! ([`crate::settings_document`]) from what the browser process sent
//! ([`UpdatesShown`]).
//!
//! # What is drawn
//!
//! The section's heading, and under it alo's version and its Chromium's as
//! its quiet line. Then one white box, as *Privacy*'s:
//!
//! - in a copy that updates itself, two rows: **Check for updates
//!   automatically**, its name over two choices one under another — *On*
//!   (the default), with the line saying once a day and nothing about you,
//!   and *Off*, with the line saying alo sends nothing until *Check now*
//!   — each a radio drawn as *Privacy*'s are, by the same function; then
//!   **Last checked**, its name over the line saying when and what was
//!   found, and the button **Check now** under it, drawn as *Forget all
//!   sign-ins* is;
//! - in one that does not, one row: **This copy**, its name over the
//!   sentence saying so — *This copy of alo does not update itself.*, or
//!   *Updates come from your software source.* — and nothing to press.
//!
//! # What an agent reads
//!
//! ADR 0002: a region named [`TITLE`]; a radio group named by its row,
//! holding a radio named by each choice, the one made checked, and the
//! line under each read after it; *Check now* a button, described by the
//! line over it. Never a coordinate. The agent socket presses none of it
//! (ADR 0116 § 4): it reaches a card's page, never alo's own window.
//!
//! # Where the keyboard is
//!
//! On the radio group, at its choice made, or on *Check now*: ringed, by
//! the class `keyed`, once Tab has put it there.
//!
//! # What a press is on
//!
//! Each choice is of the class [`CHOICE`] (`privopt updopt on` when made)
//! and *Check now* of [`BUTTON`], claimed by their place among them
//! ([`UpdatesShown::pressables`]) as [`crate::conversation_claim`]'s
//! `Updates`, and decided by the browser process against what it painted.

use crate::settings_privacy_document::{Choice, group, words};
use crate::settings_updates::{Automatic, CHECK_NOW, UpdatesShown, UpdatesState, bounded};
use crate::start::SettingsKeyed;
use crate::start_document::{Unbuilt, append, attribute, element};
use alo_dom::{Document, NodeId};

/// What the section, and the region it is, are named.
pub const TITLE: &str = "Updates";

/// What the switch's row and radio group are named.
pub const AUTOMATIC: &str = "Check for updates automatically";

/// What the row saying when alo last checked is named.
pub const LAST_CHECKED: &str = "Last checked";

/// What the row of a copy that does not update itself is named.
pub const THIS_COPY: &str = "This copy";

/// The class each of its choices is of: drawn as *Privacy*'s, claimed as
/// its own.
pub const CHOICE: &str = "privopt updopt";

/// The class *Check now* is of: drawn as *Forget all sign-ins*, claimed as
/// the section's own.
pub const BUTTON: &str = "pbtn updbtn";

/// The section for `shown`, the keyboard on `keyed`, made for *Settings*
/// to put under *Screen readers*.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which nothing shown can
/// cause.
pub fn make(
    document: &mut Document,
    shown: &UpdatesShown,
    keyed: Option<SettingsKeyed>,
) -> Result<NodeId, Unbuilt> {
    let section = element(document, "section")?;
    attribute(document, section, "class", "pgroup")?;
    attribute(document, section, "aria-labelledby", "updates-title")?;
    let heading = words(document, "h2", "pname", TITLE)?;
    attribute(document, heading, "id", "updates-title")?;
    append(document, section, heading)?;
    let about = words(document, "p", "pabout", &bounded(&shown.version))?;
    append(document, section, about)?;
    let rows = element(document, "div")?;
    attribute(document, rows, "class", "limits")?;
    match &shown.state {
        UpdatesState::Itself { automatic, checked } => {
            let choices: Vec<Choice> = Automatic::ALL
                .into_iter()
                .map(|each| Choice {
                    said: each.said(),
                    note: each.note(),
                    made: each == *automatic,
                    default: each == Automatic::default(),
                })
                .collect();
            let row = group(
                document,
                ("updates", AUTOMATIC),
                &choices,
                (keyed == Some(SettingsKeyed::Updates), CHOICE),
            )?;
            append(document, rows, row)?;
            let row = said_row(
                document,
                (LAST_CHECKED, checked),
                Some(keyed == Some(SettingsKeyed::CheckNow)),
            )?;
            append(document, rows, row)?;
        }
        UpdatesState::Not(said) => {
            let row = said_row(document, (THIS_COPY, said), None)?;
            append(document, rows, row)?;
        }
    }
    append(document, section, rows)?;
    Ok(section)
}

/// One row: its `name` over the line `said`, and *Check now* under it when
/// `button` says whether the keyboard is on it.
fn said_row(
    document: &mut Document,
    (name, said): (&str, &str),
    button: Option<bool>,
) -> Result<NodeId, Unbuilt> {
    let row = element(document, "div")?;
    attribute(document, row, "class", "lrow prow")?;
    let named = words(document, "p", "lname", name)?;
    append(document, row, named)?;
    let line = words(document, "p", "pnote", &bounded(said))?;
    attribute(document, line, "id", "updates-said")?;
    append(document, row, line)?;
    if let Some(keyed) = button {
        let class = if keyed {
            format!("{BUTTON} keyed")
        } else {
            BUTTON.to_owned()
        };
        let pressed = words(document, "button", &class, CHECK_NOW)?;
        attribute(document, pressed, "aria-describedby", "updates-said")?;
        append(document, row, pressed)?;
    }
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::start::Start;
    use crate::start_document::build;

    fn shown(state: UpdatesState) -> UpdatesShown {
        UpdatesShown {
            version: "alo 0.4.1, with Chromium 156.0.8078.96.".into(),
            state,
        }
    }

    fn settings(updates: Option<UpdatesShown>, keyed: Option<SettingsKeyed>) -> Document {
        build(&Start {
            settings: true,
            settings_keyed: keyed,
            updates,
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
    fn two_radios_and_check_now_in_a_copy_that_updates_itself() {
        let document = settings(
            Some(shown(UpdatesState::Itself {
                automatic: Automatic::Off,
                checked: "14 November 2026, 09:00: alo 0.4.1 is up to date.".into(),
            })),
            Some(SettingsKeyed::CheckNow),
        );
        let of = |node, name| document.attribute_by_name(node, name).map(str::to_owned);
        let radios: Vec<(Option<String>, bool)> = document
            .descendants(document.root())
            .filter(|&node| of(node, "name").as_deref() == Some("updates"))
            .map(|node| (of(node, "aria-label"), of(node, "checked").is_some()))
            .collect();
        assert_eq!(
            radios,
            vec![(Some("On".into()), false), (Some("Off".into()), true)]
        );
        let found = classes(&document);
        assert_eq!(
            found
                .iter()
                .filter(|class| class.starts_with(CHOICE) || class.starts_with(BUTTON))
                .collect::<Vec<_>>(),
            [CHOICE, "privopt updopt on", "pbtn updbtn keyed"]
        );
        let text = document.text_content(document.root());
        for said in [
            TITLE,
            AUTOMATIC,
            LAST_CHECKED,
            CHECK_NOW,
            "alo 0.4.1, with Chromium 156.0.8078.96.",
            "alo 0.4.1 is up to date.",
            "alo sends nothing until you press Check now",
        ] {
            assert!(text.contains(said), "{said} in {text}");
        }
    }

    #[test]
    fn a_copy_that_does_not_update_itself_says_so_and_offers_nothing() {
        let document = settings(
            Some(shown(UpdatesState::Not(
                "This copy of alo does not update itself.".into(),
            ))),
            None,
        );
        let text = document.text_content(document.root());
        assert!(text.contains("This copy of alo does not update itself."));
        assert!(!text.contains(CHECK_NOW) && !text.contains(AUTOMATIC));
        assert!(
            !classes(&document)
                .iter()
                .any(|class| class.contains("updopt") || class.contains("updbtn"))
        );
        // None draws no section.
        let none = settings(None, None);
        assert!(!none.text_content(none.root()).contains(LAST_CHECKED));
    }

    #[test]
    fn a_line_longer_than_the_bound_is_cut() {
        let document = settings(Some(shown(UpdatesState::Not("z".repeat(1000)))), None);
        let text = document.text_content(document.root());
        assert!(text.contains(&"z".repeat(400)) && !text.contains(&"z".repeat(401)));
    }
}
