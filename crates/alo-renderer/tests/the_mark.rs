/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's mark inside alo is the owner's icon, as it is (queue item 913).
//!
//! The chats panel's head shows the icon before the word *alo*, and the
//! rail shows it alone; both templates write the owner's drawing,
//! `docs/brand/alo-browser-icon.svg`, element for element. Where each is
//! laid out, in numbers, and what each looks like are `the_chats_panel.rs`'s
//! (its `icons` lists and its references).

use alo_renderer::{panel_document, rail_document};

/// The owner's drawing: everything inside its `<svg>`, each line as the
/// file writes it, without its indent.
fn owners() -> Vec<&'static str> {
    let file = include_str!("../../../docs/brand/alo-browser-icon.svg");
    file.split_once('>')
        .map(|(_, inside)| {
            inside
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// The lines of a template's mark: from its `<svg class="logo"` to the end
/// of that `<svg>`, each without its indent.
fn mark_of(template: &str) -> Vec<&str> {
    let Some((_, from)) = template.split_once("<svg class=\"logo\"") else {
        return Vec::new();
    };
    let Some((_, inside)) = from.split_once('>') else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    for line in inside
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if line.starts_with("</svg>") {
            lines.push("</svg>");
            break;
        }
        lines.push(line);
    }
    lines
}

#[test]
fn the_panels_mark_and_the_rails_are_the_owners_icon() {
    let owners = owners();
    assert!(owners.len() > 5, "the owner's file reads: {owners:?}");
    for (name, template) in [
        ("panel.html", panel_document::TEMPLATE),
        ("rail.html", rail_document::TEMPLATE),
    ] {
        assert_eq!(mark_of(template), owners, "{name}'s mark");
    }
}

#[test]
fn the_mark_is_hidden_from_an_agent() {
    for template in [panel_document::TEMPLATE, rail_document::TEMPLATE] {
        assert!(template.contains(
            "<span class=\"mark\" aria-hidden=\"true\"><svg class=\"logo\" aria-hidden=\"true\""
        ));
    }
}
