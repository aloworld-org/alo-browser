/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*' second section, **alo at work** (queue item 878,
//! `docs/design/shell.md`, *Settings*; asked by the owner with 821, 8
//! November 2026), built into the first screen's document under *Font*
//! ([`crate::settings_document`]): how far alo's agent may go on one task
//! ([`Limits`]).
//!
//! # What is drawn
//!
//! The section's heading and one quiet line saying what is always asked
//! and never done; then one white box of three rows, each ruled off from
//! the one above:
//!
//! - **Sites a task may open**, with a stepper at its right: *Fewer sites*
//!   (−), the number, *More sites* (+).
//! - **Actions a task may take**, the same: *Fewer actions*, the number,
//!   *More actions*.
//! - **Ask before opening**, with two radios at its right, drawn as the
//!   faces' are: *A site not found or named* (the default) and *Any site*.
//!
//! A button that would pass its number's end is `lbtn off`, quiet and half
//! seen, `aria-disabled`, and claims nothing.
//!
//! # What an agent reads
//!
//! ADR 0002: a region named [`AT_WORK`] holding its heading and line; each
//! number a `spinbutton` named by its row, holding the number, beside its
//! two buttons named by what they do; and a radio group named [`ASK`]
//! holding a radio named by each choice, the one made checked. Never a
//! coordinate.
//!
//! # Where the keyboard is
//!
//! Tab moves the keyboard on from *Font* to the sites, the actions and the
//! radio group (queue item 878, as 856 does the rest): the one it is on is
//! ringed — the stepper, or the choice made — by the class `keyed`.
//!
//! # What a press is on
//!
//! A stepper's button by its place among the buttons that can be pressed
//! (`lbtn`), and a choice by its place among the two (`askopt`), claimed
//! by the renderer ([`crate::conversation_claim`]) and decided by the
//! browser process against what it painted ([`Limits::presses`],
//! [`Asks::ALL`]).

use crate::agent_limits::{Asks, Limit, Limits, Stepped};
use crate::start::SettingsKeyed;
use crate::start_document::{Unbuilt, append, attribute, element};
use alo_dom::{Document, NodeId};

/// What the section, and the region it is, are named.
pub const AT_WORK: &str = "alo at work";

/// What the section says under its heading.
pub const ABOUT: &str = "How far alo goes on one task. It always asks before it pays, sends or \
deletes.";

/// What the sites' row and its spinbutton are named.
pub const SITES: &str = "Sites a task may open";

/// What the actions' row and its spinbutton are named.
pub const ACTIONS: &str = "Actions a task may take";

/// What the radio group, and its row, are named.
pub const ASK: &str = "Ask before opening";

/// The words of the default choice: asking only before a site neither
/// found by searching nor named by the person.
pub const UNNAMED: &str = "A site not found or named";

/// The words of the other: asking before every site.
pub const EVERY: &str = "Any site";

/// What stands after the default choice's words.
const DEFAULT: &str = "Default";

/// What a stepper's button is named, by what it does.
pub fn named(stepped: Stepped) -> &'static str {
    match (stepped.limit, stepped.more) {
        (Limit::Sites, false) => "Fewer sites",
        (Limit::Sites, true) => "More sites",
        (Limit::Actions, false) => "Fewer actions",
        (Limit::Actions, true) => "More actions",
    }
}

/// What a choice of asking says.
pub fn said(asks: Asks) -> &'static str {
    match asks {
        Asks::Unnamed => UNNAMED,
        Asks::Every => EVERY,
    }
}

/// The section for `limits`, the keyboard on `keyed`, made for *Settings*
/// to put under *Font*.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no limits can
/// cause.
pub fn make(
    document: &mut Document,
    limits: Limits,
    keyed: Option<SettingsKeyed>,
) -> Result<NodeId, Unbuilt> {
    let section = element(document, "section")?;
    attribute(document, section, "class", "pgroup")?;
    attribute(document, section, "aria-labelledby", "work-title")?;
    let heading = words(document, "h2", "pname", AT_WORK)?;
    attribute(document, heading, "id", "work-title")?;
    append(document, section, heading)?;
    let about = words(document, "p", "pabout", ABOUT)?;
    append(document, section, about)?;
    let rows = element(document, "div")?;
    attribute(document, rows, "class", "limits")?;
    for (limit, name, shown) in [
        (Limit::Sites, SITES, u32::from(limits.sites)),
        (Limit::Actions, ACTIONS, u32::from(limits.actions)),
    ] {
        let on = match limit {
            Limit::Sites => SettingsKeyed::Sites,
            Limit::Actions => SettingsKeyed::Actions,
        };
        let row = stepper(document, limits, limit, (name, shown), keyed == Some(on))?;
        append(document, rows, row)?;
    }
    let asking = asks(document, limits.asks, keyed == Some(SettingsKeyed::Asks))?;
    append(document, rows, asking)?;
    append(document, section, rows)?;
    Ok(section)
}

/// One number's row: its name, then *Fewer*, the number as a spinbutton,
/// and *More*.
fn stepper(
    document: &mut Document,
    limits: Limits,
    limit: Limit,
    (name, shown): (&str, u32),
    keyed: bool,
) -> Result<NodeId, Unbuilt> {
    let row = element(document, "div")?;
    attribute(document, row, "class", "lrow")?;
    let id = match limit {
        Limit::Sites => "sites-name",
        Limit::Actions => "actions-name",
    };
    let label = words(document, "p", "lname", name)?;
    attribute(document, label, "id", id)?;
    append(document, row, label)?;
    let steps = element(document, "div")?;
    attribute(
        document,
        steps,
        "class",
        if keyed { "stepper keyed" } else { "stepper" },
    )?;
    let (least, most) = match limit {
        Limit::Sites => (
            u32::from(*crate::agent_limits::SITES.start()),
            u32::from(*crate::agent_limits::SITES.end()),
        ),
        Limit::Actions => (
            u32::from(*crate::agent_limits::ACTIONS.start()),
            u32::from(*crate::agent_limits::ACTIONS.end()),
        ),
    };
    for more in [false, true] {
        let stepped = Stepped { limit, more };
        if more {
            let value = words(document, "span", "lvalue", &shown.to_string())?;
            attribute(document, value, "role", "spinbutton")?;
            attribute(document, value, "aria-labelledby", id)?;
            attribute(document, value, "aria-valuenow", &shown.to_string())?;
            attribute(document, value, "aria-valuemin", &least.to_string())?;
            attribute(document, value, "aria-valuemax", &most.to_string())?;
            append(document, steps, value)?;
        }
        let button = element(document, "button")?;
        let can = limits.can(stepped);
        attribute(
            document,
            button,
            "class",
            if can { "lbtn" } else { "lbtn off" },
        )?;
        attribute(document, button, "type", "button")?;
        attribute(document, button, "aria-label", named(stepped))?;
        if !can {
            attribute(document, button, "aria-disabled", "true")?;
        }
        let sign = words(
            document,
            "span",
            "lsign",
            if more { "+" } else { "\u{2212}" },
        )?;
        attribute(document, sign, "aria-hidden", "true")?;
        append(document, button, sign)?;
        append(document, steps, button)?;
    }
    append(document, row, steps)?;
    Ok(row)
}

/// The row of asking: its name, and a radio for each choice.
fn asks(document: &mut Document, chosen: Asks, keyed: bool) -> Result<NodeId, Unbuilt> {
    let row = element(document, "div")?;
    attribute(document, row, "class", "lrow")?;
    let label = words(document, "p", "lname", ASK)?;
    attribute(document, label, "id", "asks-name")?;
    append(document, row, label)?;
    let group = element(document, "div")?;
    attribute(document, group, "class", "askopts")?;
    attribute(document, group, "role", "radiogroup")?;
    attribute(document, group, "aria-labelledby", "asks-name")?;
    for asks in Asks::ALL {
        let on = asks == chosen;
        let option = element(document, "div")?;
        let class = match (on, keyed) {
            (true, true) => "askopt on keyed",
            (true, false) => "askopt on",
            (false, _) => "askopt",
        };
        attribute(document, option, "class", class)?;
        let radio = element(document, "input")?;
        attribute(document, radio, "class", "dot")?;
        attribute(document, radio, "type", "radio")?;
        attribute(document, radio, "name", "asks")?;
        attribute(document, radio, "aria-label", said(asks))?;
        if on {
            attribute(document, radio, "checked", "")?;
        }
        append(document, option, radio)?;
        let shown = if asks == Asks::default() {
            format!("{} \u{b7} {DEFAULT}", said(asks))
        } else {
            said(asks).to_owned()
        };
        let name = words(document, "span", "aname", &shown)?;
        attribute(document, name, "aria-hidden", "true")?;
        append(document, option, name)?;
        append(document, group, option)?;
    }
    append(document, row, group)?;
    Ok(row)
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
    use crate::start::Start;
    use crate::start_document::build;

    fn settings(limits: Limits, keyed: Option<SettingsKeyed>) -> Document {
        build(&Start {
            settings: true,
            limits,
            settings_keyed: keyed,
            ..Start::default()
        })
        .expect("built")
    }

    fn of(document: &Document, node: NodeId, name: &str) -> Option<String> {
        document.attribute_by_name(node, name).map(str::to_owned)
    }

    fn of_class(document: &Document, class: &str) -> Vec<NodeId> {
        document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some(class))
            .collect()
    }

    #[test]
    fn the_section_stands_under_font_with_both_numbers_and_the_choice_made() {
        let document = settings(Limits::default(), None);
        let text = document.text_content(document.root());
        let font = text.find("Font").expect("font");
        let work = text.find(AT_WORK).expect("alo at work");
        assert!(font < work, "{text}");
        let spins: Vec<(Option<String>, Option<String>, String)> = document
            .descendants(document.root())
            .filter(|&node| of(&document, node, "role").as_deref() == Some("spinbutton"))
            .map(|node| {
                (
                    of(&document, node, "aria-labelledby"),
                    of(&document, node, "aria-valuenow"),
                    document.text_content(node),
                )
            })
            .collect();
        assert_eq!(
            spins,
            vec![
                (Some("sites-name".into()), Some("3".into()), "3".to_owned()),
                (
                    Some("actions-name".into()),
                    Some("20".into()),
                    "20".to_owned()
                ),
            ]
        );
        let buttons: Vec<Option<String>> = of_class(&document, "lbtn")
            .into_iter()
            .map(|node| of(&document, node, "aria-label"))
            .collect();
        assert_eq!(
            buttons,
            vec![
                Some("Fewer sites".into()),
                Some("More sites".into()),
                Some("Fewer actions".into()),
                Some("More actions".into()),
            ]
        );
        let radios: Vec<(Option<String>, bool)> = document
            .descendants(document.root())
            .filter(|&node| of(&document, node, "name").as_deref() == Some("asks"))
            .map(|node| {
                (
                    of(&document, node, "aria-label"),
                    of(&document, node, "checked").is_some(),
                )
            })
            .collect();
        assert_eq!(
            radios,
            vec![(Some(UNNAMED.into()), true), (Some(EVERY.into()), false)]
        );
    }

    #[test]
    fn a_button_past_its_end_is_off_and_the_keyboard_rings_what_it_is_on() {
        let least = Limits::checked(1, 100, Asks::Every).expect("within");
        let document = settings(least, Some(SettingsKeyed::Asks));
        let off: Vec<Option<String>> = of_class(&document, "lbtn off")
            .into_iter()
            .map(|node| of(&document, node, "aria-label"))
            .collect();
        assert_eq!(
            off,
            vec![Some("Fewer sites".into()), Some("More actions".into())]
        );
        assert_eq!(of_class(&document, "lbtn").len(), 2);
        assert_eq!(of_class(&document, "askopt on keyed").len(), 1);
        assert!(of_class(&document, "stepper keyed").is_empty());
        let document = settings(least, Some(SettingsKeyed::Actions));
        assert_eq!(of_class(&document, "stepper keyed").len(), 1);
        assert_eq!(of_class(&document, "askopt on").len(), 1);
        let document = settings(least, None);
        assert!(of_class(&document, "stepper keyed").is_empty());
    }
}
