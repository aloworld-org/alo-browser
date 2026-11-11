/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*, built into the first screen's document over the chat on
//! screen (queue items 854, 886, `docs/design/shell.md`, *Settings*;
//! approved by the owner 2026-11-07, as a popup 2026-11-08).
//!
//! The screen's `main` is built as it would be with *Settings* closed — the
//! conversation, a live card's page among it, scrolled as it was — and
//! hidden from an agent, since a modal dialog stands over it. Over it, the
//! template's `<template id="settings">`: **a veil** the whole screen,
//! which blurs and dims what is under it (`backdrop-filter`, queue item
//! 885) and is pressed as *Close*, and in its middle **a dialog named
//! *Settings***: its heading and *Close* — the × at its top right — and
//! under them two sections: **Font** — a radio group named *Font* holding a
//! radio for each face a person may set the interface's words in
//! ([`InterfaceFace::ALL`]), each named by the face and checked when it is
//! the one chosen ([`Start::face`]) — an `<input type=radio>`, drawn as
//! Chrome draws one, in the ink — and, under it, **alo at work** (queue
//! item 878, [`crate::settings_work_document`]) and **Search and answers**
//! (ADR 0106, queue item 905, [`crate::settings_services_document`]), and
//! **Privacy** (ADR 0108, queue item 920,
//! [`crate::settings_privacy_document`]).
//!
//! # Each face shown as it would be
//!
//! A radio holds a sample — one sentence, [`SAMPLE`], the owner's words of
//! 9 November 2026 (queue item 901), in place of the result's title and
//! description it held before — set in that face: its own
//! style names it ([`InterfaceFace::declaration`]), so each is drawn in its
//! face whatever the page is set in. The face's name and the sample are
//! hidden from an agent, which reads the radio by its name.
//!
//! # Where the keyboard is
//!
//! Tab and ⇧Tab move the keyboard between *Close* and the radio group
//! *Font* and on through *alo at work* (queue items 856, 878), which the
//! browser process holds and says ([`Start::settings_keyed`]): the one it
//! is on is of the class `keyed` too, *Close*'s button or the face chosen —
//! on the group, the keyboard is on its checked radio, as the ARIA radio
//! group pattern has it — and drawn with a ring outside its edge.
//!
//! # What a press is on
//!
//! A face — its radio, its name, its sample, the room around them — is of
//! the class `face`, `face on` when chosen, and claimed by its place among
//! the faces drawn ([`crate::conversation_claim`]); *Close* is of the class
//! `pclose`, one of a kind, and so is the veil around the dialog — a press
//! there is *Close*'s, and nothing's under it — while a press on the
//! dialog's own room is on nothing. Which face a place was is the browser
//! process's to say, from the faces it holds, never anything the renderer
//! said.

use crate::conversation_scroll::Room;
use crate::interface_face::InterfaceFace;
use crate::start::{SettingsKeyed, Start};
use crate::start_document::{Unbuilt, append, attribute, by_id, element};
use alo_dom::{Document, NodeId};

/// The id of the template's `<template>` holding the veil and the dialog.
const SETTINGS: &str = "settings";

/// The id of what scrolls in the dialog (queue item 880): the page under
/// its heading, measured for how far it reaches
/// ([`crate::conversation_scroll::reach_of`]).
pub const PAGE: &str = "settings-page";

/// The id of the place for the faces.
const FACES: &str = "faces";

/// The id of the template's `main`, the whole screen.
const SCREEN: &str = "start";

/// What *Settings* is named.
pub const TITLE: &str = "Settings";

/// What its one section, and the radio group in it, is named.
pub const FONT: &str = "Font";

/// What the × at its top right is named (queue item 886).
pub const CLOSE: &str = "Close";

/// What stands after the default face's name.
pub const DEFAULT: &str = "Default";

/// The sentence each face is shown setting (queue item 901, the owner's
/// words).
pub const SAMPLE: &str = "If you live each day as if it was your last, someday you\u{2019}ll most \
                          certainly be right.";

/// Put *Settings* into `document`, the first screen's, over everything
/// the screen's `main` holds, which is hidden from an agent.
///
/// # Errors
///
/// [`Unbuilt`] if the template lacks a place it names, or `alo-dom`
/// refused an operation, which no state can cause.
pub fn put(document: &mut Document, start: &Start) -> Result<(), Unbuilt> {
    let screen = by_id(document, SCREEN)?;
    // A modal dialog stands over it: what is under it is read by nobody.
    attribute(document, screen, "aria-hidden", "true")?;
    let body = document.parent(screen).ok_or_else(|| Unbuilt {
        why: "the first screen's main has no body around it".to_owned(),
    })?;
    let veil = take_veil(document)?;
    append(document, body, veil)?;
    let page = by_id(document, PAGE)?;
    // Scrolled in its panel (queue items 880, 886): moved up by how far
    // the browser process scrolled it — a number of its own — with the
    // sign that it goes on above or below in the clip's right gutter.
    if start.settings_down != 0.0 && start.settings_down.is_finite() {
        attribute(
            document,
            page,
            "style",
            &format!("top: {}px", -start.settings_down),
        )?;
    }
    if let Some(bar) = start.settings_bar {
        let clip = document.parent(page).ok_or_else(|| Unbuilt {
            why: "the first screen's settings hold their page in no clip".to_owned(),
        })?;
        let room = Room::settings(start.viewport).height;
        crate::conversation_document::append_bar(document, clip, bar, &bar.said_of(TITLE, room))?;
    }
    // The keyboard on *Close* (queue item 856): ringed.
    if start.settings_keyed == Some(SettingsKeyed::Close) {
        let close = document
            .descendants(veil)
            .find(|&node| document.attribute_by_name(node, "class") == Some("pclose"))
            .ok_or_else(|| Unbuilt {
                why: "the first screen's settings hold no Close".to_owned(),
            })?;
        attribute(document, close, "class", "pclose keyed")?;
    }
    let faces = by_id(document, FACES)?;
    let font = start.settings_keyed == Some(SettingsKeyed::Font);
    for face in InterfaceFace::ALL {
        let chosen = face == start.face;
        let radio = make_face(document, face, chosen, chosen && font)?;
        append(document, faces, radio)?;
    }
    // *alo at work*, under *Font* (queue item 878).
    let work = crate::settings_work_document::make(document, start.limits, start.settings_keyed)?;
    append(document, page, work)?;
    // *Search and answers*, under it (ADR 0106, queue item 905).
    if let Some(services) = &start.services {
        let keyed = match start.settings_keyed {
            Some(SettingsKeyed::Service(on)) => Some(on),
            _ => None,
        };
        let section = crate::settings_services_document::make(document, services, keyed)?;
        append(document, page, section)?;
    }
    // *Privacy*, under it (ADR 0108, queue item 920).
    if let Some(privacy) = start.privacy {
        let section = crate::settings_privacy_document::make(
            document,
            (privacy, start.sign_ins),
            start.settings_keyed,
        )?;
        append(document, page, section)?;
    }
    Ok(())
}

/// The veil the template's `<template>` holds, the dialog in it, to be
/// moved over the screen.
fn take_veil(document: &Document) -> Result<NodeId, Unbuilt> {
    let holder = by_id(document, SETTINGS)?;
    document
        .element(holder)
        .and_then(|holder| holder.template_contents)
        .and_then(|contents| {
            document
                .children(contents)
                .find(|&child| document.element(child).is_some())
        })
        .ok_or_else(|| Unbuilt {
            why: "the first screen's template holds no settings".to_owned(),
        })
}

/// One face: a radio named by it, checked when `chosen`, and the face's
/// name and its sample, drawn in it.
///
/// The radio is an `<input type=radio>`, drawn as Chrome draws one in the
/// ink (`accent-color`), never an element given `role="radio"`: this engine
/// draws an author's checked radio as a fill of its whole box (queue item
/// 182), which here would be the whole face.
fn make_face(
    document: &mut Document,
    face: InterfaceFace,
    chosen: bool,
    keyed: bool,
) -> Result<NodeId, Unbuilt> {
    let made = element(document, "div")?;
    // The keyboard on the radio group is on the face chosen, as the ARIA
    // radio group pattern has it (queue item 856): ringed.
    let class = match (chosen, keyed) {
        (true, true) => "face on keyed",
        (true, false) => "face on",
        (false, _) => "face",
    };
    attribute(document, made, "class", class)?;
    attribute(document, made, "style", &face.declaration())?;
    let head = element(document, "div")?;
    attribute(document, head, "class", "fhead")?;
    let radio = element(document, "input")?;
    attribute(document, radio, "class", "dot")?;
    attribute(document, radio, "type", "radio")?;
    attribute(document, radio, "name", "face")?;
    attribute(document, radio, "aria-label", face.name())?;
    if chosen {
        attribute(document, radio, "checked", "")?;
    }
    append(document, head, radio)?;
    let name = hidden(document, "span", "fname")?;
    let said = if face == InterfaceFace::default() {
        format!("{} \u{b7} {DEFAULT}", face.name())
    } else {
        face.name().to_owned()
    };
    let words = document.create_text_node(&said);
    append(document, name, words)?;
    append(document, head, name)?;
    append(document, made, head)?;
    let sample = hidden(document, "p", "sample")?;
    let words = document.create_text_node(SAMPLE);
    append(document, sample, words)?;
    append(document, made, sample)?;
    Ok(made)
}

/// An element of `class` hidden from an agent: what a radio holds is
/// drawn for a person, and the radio is read by its name.
fn hidden(document: &mut Document, local: &str, class: &str) -> Result<NodeId, Unbuilt> {
    let made = element(document, local)?;
    attribute(document, made, "class", class)?;
    attribute(document, made, "aria-hidden", "true")?;
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::start_document::build;

    fn settings(face: InterfaceFace) -> Start {
        Start {
            settings: true,
            face,
            greeting: "Hello.".to_owned(),
            ..Start::default()
        }
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
    fn settings_stands_over_the_screen_which_is_hidden_from_an_agent() {
        // Queue item 886: the greeting, the box and the groups are still
        // there, under a veil and a dialog.
        let document = build(&settings(InterfaceFace::Inter)).expect("built");
        let found = classes(&document);
        for kept in ["hello", "box", "blocks", "veil", "sdialog", "pclose"] {
            assert!(
                found.iter().any(|class| class == kept),
                "{kept} in {found:?}"
            );
        }
        let faces = found
            .iter()
            .filter(|class| *class == "face" || *class == "face on")
            .count();
        assert_eq!(faces, 5);
        let text = document.text_content(document.root());
        assert!(
            text.contains(TITLE) && text.contains(FONT) && text.contains("Hello."),
            "{text}"
        );
        let of = |id: &str| {
            document
                .descendants(document.root())
                .find(|&node| document.attribute_by_name(node, "id") == Some(id))
        };
        let screen = of("start").expect("the screen");
        assert_eq!(
            document.attribute_by_name(screen, "aria-hidden"),
            Some("true")
        );
        let dialog = document
            .descendants(document.root())
            .find(|&node| document.attribute_by_name(node, "role") == Some("dialog"))
            .expect("a dialog");
        assert_eq!(
            document.attribute_by_name(dialog, "aria-modal"),
            Some("true")
        );
        assert!(of(PAGE).is_some());
        // Closed, nothing of it.
        let closed = build(&Start {
            settings: false,
            ..settings(InterfaceFace::Inter)
        })
        .expect("built");
        assert!(!classes(&closed).iter().any(|class| class == "veil"));
        let screen = closed
            .descendants(closed.root())
            .find(|&node| closed.attribute_by_name(node, "id") == Some("start"))
            .expect("the screen");
        assert_eq!(closed.attribute_by_name(screen, "aria-hidden"), None);
    }

    #[test]
    fn the_keyboard_on_close_or_on_the_font_rings_it_and_nothing_else() {
        // Queue item 856.
        let keyed = |settings_keyed| {
            let start = Start {
                settings_keyed,
                ..settings(InterfaceFace::Geist)
            };
            let document = build(&start).expect("built");
            classes(&document)
                .into_iter()
                .filter(|class| class.contains("keyed"))
                .collect::<Vec<_>>()
        };
        assert!(keyed(None).is_empty());
        assert_eq!(keyed(Some(SettingsKeyed::Close)), vec!["pclose keyed"]);
        assert_eq!(keyed(Some(SettingsKeyed::Font)), vec!["face on keyed"]);
    }

    #[test]
    fn the_face_chosen_is_checked_and_each_is_named_and_set_in_itself() {
        let document = build(&settings(InterfaceFace::DmSans)).expect("built");
        let of = |node, name| document.attribute_by_name(node, name).map(str::to_owned);
        let radios: Vec<(Option<String>, bool)> = document
            .descendants(document.root())
            .filter(|&node| of(node, "name").as_deref() == Some("face"))
            .map(|node| (of(node, "aria-label"), of(node, "checked").is_some()))
            .collect();
        assert_eq!(
            radios,
            vec![
                (Some("Inter".into()), false),
                (Some("Geist".into()), false),
                (Some("DM Sans".into()), true),
                (Some("Plus Jakarta Sans".into()), false),
                (Some("Instrument Sans".into()), false),
            ]
        );
        let faces: Vec<(Option<String>, Option<String>)> = document
            .descendants(document.root())
            .filter(|&node| matches!(of(node, "class").as_deref(), Some("face" | "face on")))
            .map(|node| (of(node, "class"), of(node, "style")))
            .collect();
        let set = |class: &str, family: &str| {
            (
                Some(class.to_owned()),
                Some(format!("--face: \"{family}\"")),
            )
        };
        assert_eq!(
            faces,
            vec![
                set("face", "Inter Variable"),
                set("face", "Geist"),
                set("face on", "DM Sans"),
                set("face", "Plus Jakarta Sans"),
                set("face", "Instrument Sans"),
            ]
        );
        // The page itself is set in the face chosen, on its root.
        let root = document.document_element();
        assert_eq!(
            root.and_then(|root| document.attribute_by_name(root, "style")),
            Some("--face: \"DM Sans\"")
        );
    }
}
