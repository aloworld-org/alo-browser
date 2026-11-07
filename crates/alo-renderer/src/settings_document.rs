/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! *Settings*, built into the first screen's document in place of the chat
//! on screen (queue item 854, `docs/design/shell.md`, *Settings*; approved
//! by the owner 2026-11-07).
//!
//! The screen's `main` loses the greeting, the box and the groups, and
//! holds the template's `<template id="settings">` instead: *Back to the
//! chat*, the page's heading, and one section, **Font** — a radio group
//! named *Font* holding a radio for each face a person may set the
//! interface's words in ([`InterfaceFace::ALL`]), each named by the face
//! and checked when it is the one chosen ([`Start::face`]) — an `<input
//! type=radio>`, drawn as Chrome draws one, in the ink.
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
//! # What a press is on
//!
//! A face — its radio, its name, its sample, the room around them — is of
//! the class `face`, `face on` when chosen, and claimed by its place among
//! the faces drawn ([`crate::conversation_claim`]); *Back
//! to the chat* is of the class `pback`, one of a kind. Which face a place
//! was is the browser process's to say, from the faces it holds, never
//! anything the renderer said.

use crate::interface_face::InterfaceFace;
use crate::start::Start;
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
    append(document, screen, page)?;
    let faces = by_id(document, FACES)?;
    for face in InterfaceFace::ALL {
        let radio = make_face(document, face, face == start.face)?;
        append(document, faces, radio)?;
    }
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
) -> Result<NodeId, Unbuilt> {
    let made = element(document, "div")?;
    attribute(
        document,
        made,
        "class",
        if chosen { "face on" } else { "face" },
    )?;
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
    fn the_face_chosen_is_checked_and_each_is_named_and_set_in_itself() {
        let document = build(&settings(InterfaceFace::DmSans)).expect("built");
        let of = |node, name| document.attribute_by_name(node, name).map(str::to_owned);
        let radios: Vec<(Option<String>, bool)> = document
            .descendants(document.root())
            .filter(|&node| of(node, "type").as_deref() == Some("radio"))
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
