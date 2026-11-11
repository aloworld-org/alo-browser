/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*' fourth section, **Privacy** (ADR 0108, queue item 920),
//! built into the first screen's document under *Search and answers*
//! ([`crate::settings_document`]) from the choices the browser process
//! sent ([`Privacy`]).
//!
//! # What is drawn
//!
//! The section's heading and one quiet line; then one white box, as *alo
//! at work*'s, of three rows ruled off from each other: **Cookies**, its
//! name over three choices one under another — *Refuse other sites’
//! cookies* (the default), *Refuse all cookies* with the quiet line saying
//! signing in will then fail, *Keep all cookies* — and **Cookie banners**,
//! its name over two: *Answer with the most private choice* (the default)
//! and *Leave them to me*. Each choice a radio drawn as the faces' are, in
//! the ink. Then **Sign-ins** (ADR 0111): its name over the quiet line
//! saying whether they are kept on this Mac ([`SignInsShown::said`]), and
//! the button **Forget all sign-ins** under it, drawn as *Search and
//! answers*' buttons are.
//!
//! # What an agent reads
//!
//! ADR 0002: a region named [`TITLE`]; two radio groups, each named by
//! its row, holding a radio named by each choice, the one made checked.
//! The words drawn beside a radio are hidden from an agent, which reads
//! the radio by its name; the line under *Refuse all cookies* is read
//! after it, as text, and named its description (`aria-describedby`) for
//! a reader that takes one. *Forget all sign-ins* is a button, named so,
//! described by the line over it. Never a coordinate.
//!
//! # Where the keyboard is
//!
//! On a radio group, at its choice made, or on the button: ringed, by the
//! class `keyed`, once Tab has put it there.
//!
//! # What a press is on
//!
//! Each choice — its radio, its words, the room around them — is of the
//! class `privopt` (`privopt on` when made), claimed by its place among
//! them all ([`Privacy::pressables`]) as [`crate::conversation_claim`]'s
//! `Privacy`, and decided by the browser process against what it painted;
//! the button, `pbtn`, is the place after the choices'.

use crate::settings_privacy::{Banners, Cookies, FORGET_ALL, Privacy, SignInsShown};
use crate::start::SettingsKeyed;
use crate::start_document::{Unbuilt, append, attribute, element};
use alo_dom::{Document, NodeId};

/// What the section, and the region it is, are named.
pub const TITLE: &str = "Privacy";

/// What the section says under its heading.
pub const ABOUT: &str = "What sites may keep on this Mac, and how alo answers their cookie \
banners. A cookie choice is used from the next page alo opens.";

/// What the cookies' row and radio group are named.
pub const COOKIES: &str = "Cookies";

/// What the banners' row and radio group are named.
pub const BANNERS: &str = "Cookie banners";

/// What the sign-ins' row is named.
pub const SIGN_INS: &str = "Sign-ins";

/// What stands after a default choice's words.
const DEFAULT: &str = "Default";

/// The section for `privacy`, its sign-ins as `sign_ins` says, the
/// keyboard on `keyed`, made for *Settings* to put under *Search and
/// answers*.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no choice can
/// cause.
pub fn make(
    document: &mut Document,
    (privacy, sign_ins): (Privacy, SignInsShown),
    keyed: Option<SettingsKeyed>,
) -> Result<NodeId, Unbuilt> {
    let section = element(document, "section")?;
    attribute(document, section, "class", "pgroup")?;
    attribute(document, section, "aria-labelledby", "privacy-title")?;
    let heading = words(document, "h2", "pname", TITLE)?;
    attribute(document, heading, "id", "privacy-title")?;
    append(document, section, heading)?;
    let about = words(document, "p", "pabout", ABOUT)?;
    append(document, section, about)?;
    let rows = element(document, "div")?;
    attribute(document, rows, "class", "limits")?;
    let cookies: Vec<Choice> = Cookies::ALL
        .into_iter()
        .map(|each| Choice {
            said: each.said(),
            note: each.note(),
            made: each == privacy.cookies,
            default: each == Cookies::default(),
        })
        .collect();
    let row = group(
        document,
        ("cookies", COOKIES),
        &cookies,
        keyed == Some(SettingsKeyed::Cookies),
    )?;
    append(document, rows, row)?;
    let banners: Vec<Choice> = Banners::ALL
        .into_iter()
        .map(|each| Choice {
            said: each.said(),
            note: None,
            made: each == privacy.banners,
            default: each == Banners::default(),
        })
        .collect();
    let row = group(
        document,
        ("banners", BANNERS),
        &banners,
        keyed == Some(SettingsKeyed::Banners),
    )?;
    append(document, rows, row)?;
    let row = signed_in(
        document,
        sign_ins,
        keyed == Some(SettingsKeyed::ForgetSignIns),
    )?;
    append(document, rows, row)?;
    append(document, section, rows)?;
    Ok(section)
}

/// The row *Sign-ins*: its name, the line `sign_ins` says, and the button
/// that forgets them all, ringed when `keyed`.
fn signed_in(
    document: &mut Document,
    sign_ins: SignInsShown,
    keyed: bool,
) -> Result<NodeId, Unbuilt> {
    let row = element(document, "div")?;
    attribute(document, row, "class", "lrow prow")?;
    let name = words(document, "p", "lname", SIGN_INS)?;
    append(document, row, name)?;
    let said = words(document, "p", "pnote", sign_ins.said())?;
    attribute(document, said, "id", "signins-said")?;
    append(document, row, said)?;
    let button = words(
        document,
        "button",
        if keyed { "pbtn keyed" } else { "pbtn" },
        FORGET_ALL,
    )?;
    attribute(document, button, "aria-describedby", "signins-said")?;
    append(document, row, button)?;
    Ok(row)
}

/// One choice as it is drawn.
struct Choice {
    /// Its words, and its radio's name.
    said: &'static str,
    /// The quiet line under it, if any.
    note: Option<&'static str>,
    /// Whether it is the one made.
    made: bool,
    /// Whether it is the default, said after its words.
    default: bool,
}

/// One row: its name over a radio group of `choices`, ringed at the one
/// made when `keyed`.
fn group(
    document: &mut Document,
    (group_name, label): (&str, &str),
    choices: &[Choice],
    keyed: bool,
) -> Result<NodeId, Unbuilt> {
    let row = element(document, "div")?;
    attribute(document, row, "class", "lrow prow")?;
    let id = format!("{group_name}-name");
    let name = words(document, "p", "lname", label)?;
    attribute(document, name, "id", &id)?;
    append(document, row, name)?;
    let options = element(document, "div")?;
    attribute(document, options, "class", "privopts")?;
    attribute(document, options, "role", "radiogroup")?;
    attribute(document, options, "aria-labelledby", &id)?;
    for (at, choice) in choices.iter().enumerate() {
        let option = element(document, "div")?;
        let class = match (choice.made, keyed) {
            (true, true) => "privopt on keyed",
            (true, false) => "privopt on",
            (false, _) => "privopt",
        };
        attribute(document, option, "class", class)?;
        let radio = element(document, "input")?;
        attribute(document, radio, "class", "dot")?;
        attribute(document, radio, "type", "radio")?;
        attribute(document, radio, "name", group_name)?;
        attribute(document, radio, "aria-label", choice.said)?;
        if choice.made {
            attribute(document, radio, "checked", "")?;
        }
        append(document, option, radio)?;
        let text = element(document, "div")?;
        attribute(document, text, "class", "ptext")?;
        let shown = if choice.default {
            format!("{} \u{b7} {DEFAULT}", choice.said)
        } else {
            choice.said.to_owned()
        };
        let drawn = words(document, "span", "aname", &shown)?;
        attribute(document, drawn, "aria-hidden", "true")?;
        append(document, text, drawn)?;
        if let Some(note) = choice.note {
            let note_id = format!("{group_name}-note-{at}");
            let line = words(document, "p", "pnote", note)?;
            attribute(document, line, "id", &note_id)?;
            append(document, text, line)?;
            attribute(document, radio, "aria-describedby", &note_id)?;
        }
        append(document, option, text)?;
        append(document, options, option)?;
    }
    append(document, row, options)?;
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

    fn settings(privacy: Privacy, keyed: Option<SettingsKeyed>) -> Document {
        build(&Start {
            settings: true,
            privacy: Some(privacy),
            settings_keyed: keyed,
            ..Start::default()
        })
        .expect("built")
    }

    #[test]
    fn the_sign_ins_row_says_what_the_browser_said_and_holds_the_one_button() {
        for shown in SignInsShown::ALL {
            let document = build(&Start {
                settings: true,
                privacy: Some(Privacy::default()),
                sign_ins: shown,
                ..Start::default()
            })
            .expect("built");
            let text = document.text_content(document.root());
            assert!(
                text.contains(SIGN_INS) && text.contains(shown.said()),
                "{text}"
            );
            let buttons: Vec<_> = document
                .descendants(document.root())
                .filter(|&node| {
                    of(&document, node, "class").is_some_and(|class| class.starts_with("pbtn"))
                })
                .collect();
            assert_eq!(buttons.len(), 1);
            let button = buttons[0];
            assert_eq!(document.text_content(button), FORGET_ALL);
            assert_eq!(
                of(&document, button, "aria-describedby").as_deref(),
                Some("signins-said")
            );
        }
        let keyed = settings(Privacy::default(), Some(SettingsKeyed::ForgetSignIns));
        assert_eq!(of_class(&keyed, "pbtn keyed"), 1);
        assert_eq!(of_class(&keyed, "privopt on keyed"), 0);
        let unkeyed = settings(Privacy::default(), Some(SettingsKeyed::Cookies));
        assert_eq!(of_class(&unkeyed, "pbtn keyed"), 0);
        assert_eq!(of_class(&unkeyed, "pbtn"), 1);
    }

    fn of(document: &Document, node: NodeId, name: &str) -> Option<String> {
        document.attribute_by_name(node, name).map(str::to_owned)
    }

    fn radios(document: &Document, group: &str) -> Vec<(Option<String>, bool)> {
        document
            .descendants(document.root())
            .filter(|&node| of(document, node, "name").as_deref() == Some(group))
            .map(|node| {
                (
                    of(document, node, "aria-label"),
                    of(document, node, "checked").is_some(),
                )
            })
            .collect()
    }

    fn of_class(document: &Document, class: &str) -> usize {
        document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some(class))
            .count()
    }

    #[test]
    fn the_section_holds_both_groups_each_choice_named_and_the_one_made_checked() {
        let document = settings(Privacy::default(), None);
        let text = document.text_content(document.root());
        assert!(text.contains(TITLE) && text.contains(COOKIES) && text.contains(BANNERS));
        assert_eq!(
            radios(&document, "cookies"),
            vec![
                (Some(Cookies::RefuseOthers.said().into()), true),
                (Some(Cookies::RefuseAll.said().into()), false),
                (Some(Cookies::KeepAll.said().into()), false),
            ]
        );
        assert_eq!(
            radios(&document, "banners"),
            vec![
                (Some(Banners::Answer.said().into()), true),
                (Some(Banners::Leave.said().into()), false),
            ]
        );
        // Refusing every site's says, beside its radio, that signing in
        // will fail — and the radio is described by it.
        assert!(text.contains("Signing in to sites will fail"));
        let described = document
            .descendants(document.root())
            .filter(|&node| of(&document, node, "aria-describedby").is_some())
            .map(|node| of(&document, node, "aria-label"))
            .collect::<Vec<_>>();
        // And *Forget all sign-ins*, a button named by its words, by the
        // line over it (ADR 0111).
        assert_eq!(
            described,
            vec![Some(Cookies::RefuseAll.said().into()), None]
        );
        // The defaults say so.
        assert!(text.contains("Refuse other sites\u{2019} cookies \u{b7} Default"));
        assert!(text.contains("Answer with the most private choice \u{b7} Default"));
    }

    #[test]
    fn another_choice_is_checked_and_the_keyboard_rings_its_group_alone() {
        let privacy = Privacy {
            cookies: Cookies::KeepAll,
            banners: Banners::Leave,
        };
        let document = settings(privacy, Some(SettingsKeyed::Banners));
        assert_eq!(
            radios(&document, "cookies")
                .into_iter()
                .map(|(_, checked)| checked)
                .collect::<Vec<_>>(),
            vec![false, false, true]
        );
        assert_eq!(of_class(&document, "privopt on keyed"), 1);
        assert_eq!(of_class(&document, "privopt on"), 1);
        assert_eq!(of_class(&document, "privopt"), 3);
        let document = settings(privacy, None);
        assert_eq!(of_class(&document, "privopt on keyed"), 0);
        // None sent draws no section.
        let none = build(&Start {
            settings: true,
            ..Start::default()
        })
        .expect("built");
        assert!(!none.text_content(none.root()).contains(BANNERS));
    }
}
