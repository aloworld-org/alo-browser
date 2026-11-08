/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*, built into the first screen's document in place of the chat
//! on screen (queue item 854, `docs/design/shell.md`, *Settings*; approved
//! by the owner 2026-11-07).
//!
//! The screen's `main` loses the greeting, the box and the groups, and
//! holds the template's `<template id="settings">` instead: *Back to the
//! chat*, the page's heading, and two sections: **Font** — a radio group
//! named *Font* holding a radio for each face a person may set the
//! interface's words in ([`InterfaceFace::ALL`]), each named by the face
//! and checked when it is the one chosen ([`Start::face`]) — an `<input
//! type=radio>`, drawn as Chrome draws one, in the ink — and, under it,
//! **alo at work** (queue item 878, [`crate::settings_work_document`]).
//!
//! # Each face shown as it would be
//!
//! A radio holds a sample of a search's result — its title and its
//! description, [`SAMPLE_TITLE`] and [`SAMPLE_DESCRIPTION`], at the sizes
//! and in the colours a search draws them — set in that face: its own
//! style names it ([`InterfaceFace::declaration`]), so each is drawn in its
//! face whatever the page is set in. The face's name and the sample are
//! hidden from an agent, which reads the radio by its name.
//!
//! # Where the keyboard is
//!
//! Tab and ⇧Tab move the keyboard between *Back to the chat* and the radio
//! group *Font* (queue item 856), which the browser process holds and says
//! ([`Start::settings_keyed`]): the one it is on is of the class `keyed`
//! too, *Back to the chat*'s button or the face chosen — on the group, the
//! keyboard is on its checked radio, as the ARIA radio group pattern has
//! it — and drawn with a ring outside its edge.
//!
//! # What a press is on
//!
//! A face — its radio, its name, its sample, the room around them — is of
//! the class `face`, `face on` when chosen, and claimed by its place among
//! the faces drawn ([`crate::conversation_claim`]); *Back
//! to the chat* is of the class `pback`, one of a kind. Which face a place
//! was is the browser process's to say, from the faces it holds, never
//! anything the renderer said.

use crate::conversation_scroll::{NEWEST, Room};
use crate::interface_face::InterfaceFace;
use crate::start::{SettingsKeyed, Start};
use crate::start_document::{Unbuilt, append, attribute, by_id, element};
use alo_dom::{Document, NodeId};

/// The id of the template's `<template>` holding the page.
const SETTINGS: &str = "settings";

/// The id of the place for the faces.
const FACES: &str = "faces";

/// The id of the template's `main`, the whole screen.
const SCREEN: &str = "start";

/// What *Settings* is named.
pub const TITLE: &str = "Settings";

/// What its one section, and the radio group in it, is named.
pub const FONT: &str = "Font";

/// What *Back to the chat* is named.
pub const BACK: &str = "Back to the chat";

/// What stands after the default face's name.
pub const DEFAULT: &str = "Default";

/// The title of the result each face is shown setting.
pub const SAMPLE_TITLE: &str = "Kampala \u{2013} Wikipedia";

/// Its description.
pub const SAMPLE_DESCRIPTION: &str =
    "The capital and largest city of Uganda, set on hills near the shore of Lake Victoria.";

/// Put *Settings* into `document`, the first screen's, in place of
/// everything the screen's `main` held.
///
/// # Errors
///
/// [`Unbuilt`] if the template lacks a place it names, or `alo-dom`
/// refused an operation, which no state can cause.
pub fn put(document: &mut Document, start: &Start) -> Result<(), Unbuilt> {
    let screen = by_id(document, SCREEN)?;
    attribute(document, screen, "class", "start set")?;
    let held: Vec<NodeId> = document.children(screen).collect();
    for child in held {
        document
            .remove_child(screen, child)
            .map_err(|refused| Unbuilt {
                why: format!("a node could not be taken out of the first screen: {refused}"),
            })?;
    }
    let page = take_page(document)?;
    // Scrolled as the conversation is (queue item 880): the page in a clip
    // the screen's height, moved up by how far the browser process
    // scrolled it — a number of its own — with the sign that it goes on
    // above or below in the clip's right gutter.
    let clip = element(document, "div")?;
    attribute(document, clip, "class", "sclip")?;
    let scrolled = element(document, "div")?;
    attribute(document, scrolled, "class", "spage")?;
    attribute(document, scrolled, "id", NEWEST)?;
    if start.down != 0.0 && start.down.is_finite() {
        attribute(
            document,
            scrolled,
            "style",
            &format!("top: {}px", -start.down),
        )?;
    }
    append(document, scrolled, page)?;
    append(document, clip, scrolled)?;
    if let Some(bar) = start.bar {
        let room = Room::settings(start.viewport).height;
        crate::conversation_document::append_bar(document, clip, bar, &bar.said_of(TITLE, room))?;
    }
    append(document, screen, clip)?;
    // The keyboard on *Back to the chat* (queue item 856): ringed.
    if start.settings_keyed == Some(SettingsKeyed::Back) {
        let back = document
            .descendants(page)
            .find(|&node| document.attribute_by_name(node, "class") == Some("pback"))
            .ok_or_else(|| Unbuilt {
                why: "the first screen's settings hold no Back to the chat".to_owned(),
            })?;
        attribute(document, back, "class", "pback keyed")?;
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
    Ok(())
}

/// The page the template's `<template>` holds, to be moved into the screen.
fn take_page(document: &Document) -> Result<NodeId, Unbuilt> {
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
    for (class, text) in [("sample-t", SAMPLE_TITLE), ("sample-d", SAMPLE_DESCRIPTION)] {
        let line = hidden(document, "p", class)?;
        let words = document.create_text_node(text);
        append(document, line, words)?;
        append(document, made, line)?;
    }
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
    fn settings_stands_in_place_of_the_greeting_the_box_and_the_groups() {
        let document = build(&settings(InterfaceFace::Inter)).expect("built");
        let classes = classes(&document);
        for gone in ["hello", "sub", "box", "ask", "blocks"] {
            assert!(
                !classes.iter().any(|class| class == gone),
                "{gone} in {classes:?}"
            );
        }
        assert!(
            classes.iter().any(|class| class == "start set"),
            "{classes:?}"
        );
        let faces = classes
            .iter()
            .filter(|class| *class == "face" || *class == "face on")
            .count();
        assert_eq!(faces, 5);
        let text = document.text_content(document.root());
        assert!(text.contains(TITLE) && text.contains(FONT), "{text}");
        assert!(!text.contains("Hello."), "{text}");
    }

    #[test]
    fn the_keyboard_on_back_or_on_the_font_rings_it_and_nothing_else() {
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
        assert_eq!(keyed(Some(SettingsKeyed::Back)), vec!["pback keyed"]);
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
