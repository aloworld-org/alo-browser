/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The chats panel hidden, as the design's rail (ADR 0074, queue item 711):
//! a document of its own, drawn by the panel's renderer in place of the
//! panel while a person has hidden it.
//!
//! The rail is [`TEMPLATE`] and nothing else: the mark, *New chat*, a short
//! line and, at its foot, *Show the panel*. Nothing a person or a site said
//! enters it, so it is parsed as it is. The design's rail also holds each
//! open chat as its site's mark; that is cut to its own queue item (760), so
//! a press on this rail chooses no chat.
//!
//! # What an agent reads
//!
//! ADR 0002: a `navigation` named *Chats* holding two buttons, *New chat*
//! and *Show the panel*. The mark and the icons are hidden from the tree.
//!
//! It runs no script: the template has none and nothing here makes one.

use crate::panel_document::Unbuilt;
use alo_dom::{Document, parse_document};

/// The rail's markup, compiled in.
pub const TEMPLATE: &str = include_str!("../interface/rail.html");

/// The rail's stylesheet, after the owner's design, compiled in.
pub const SHEET: &str = include_str!("../interface/rail.css");

/// The rail's document.
///
/// # Errors
///
/// [`Unbuilt`] if the template lacks the rail, which is a defect in its
/// template, never in anything sent.
pub fn build() -> Result<Document, Unbuilt> {
    let document = parse_document(TEMPLATE);
    let rail = document
        .descendants(document.root())
        .any(|node| document.attribute_by_name(node, "class") == Some("rail"));
    if rail {
        Ok(document)
    } else {
        Err(Unbuilt {
            why: "the rail's template has no rail".to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rail_is_the_mark_new_chat_and_show_the_panel_and_no_script() {
        let document = build().unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        let buttons: Vec<Option<&str>> = document
            .descendants(document.root())
            .filter(|&node| {
                document
                    .element(node)
                    .is_some_and(|element| element.name.local.as_ref() == "button")
            })
            .map(|node| document.attribute_by_name(node, "aria-label"))
            .collect();
        assert_eq!(buttons, vec![Some("New chat"), Some("Show the panel")]);
        let scripts = document
            .descendants(document.root())
            .filter_map(|node| document.element(node))
            .filter(|element| element.name.local.as_ref() == "script")
            .count();
        assert_eq!(scripts, 0);
        assert!(SHEET.contains(".rail"));
    }
}
