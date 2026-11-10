/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's mark inside alo is the owner's icon (queue item 913), in its small
//! drawing (queue item 914).
//!
//! The chats panel's head shows the icon before the word *alo*, and the
//! rail shows it alone, 24 and 28 square: under 64 device pixels at 1 and
//! at 2, where the owner's thin arcs fade. So both templates write the small
//! drawing, `docs/brand/alo-browser-icon-small.svg`, element for element —
//! the owner's paths and colours, each also stroked in its own paint. Where each is
//! laid out, in numbers, and what each looks like are `the_chats_panel.rs`'s
//! (its `icons` lists and its references).

use alo_renderer::{panel_document, rail_document};

/// Everything inside a drawing's `<svg>`, each line as the file writes it,
/// without its indent.
fn inside(file: &'static str) -> Vec<&'static str> {
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

/// The small drawing.
fn small() -> Vec<&'static str> {
    inside(include_str!(
        "../../../docs/brand/alo-browser-icon-small.svg"
    ))
}

/// The owner's drawing.
fn owners() -> Vec<&'static str> {
    inside(include_str!("../../../docs/brand/alo-browser-icon.svg"))
}

#[test]
fn the_panels_mark_and_the_rails_are_the_small_drawing() {
    let small = small();
    assert!(small.len() > 5, "the small file reads: {small:?}");
    for (name, template) in [
        ("panel.html", panel_document::TEMPLATE),
        ("rail.html", rail_document::TEMPLATE),
    ] {
        assert_eq!(mark_of(template), small, "{name}'s mark");
    }
}

#[test]
fn the_small_drawing_is_the_owners_with_strokes_and_nothing_else() {
    // Line for line the owner's file, but for the gradient's name (so both
    // can stand on one page) and each path also stroked 14 in its own
    // paint, with round corners and ends.
    let (owners, small) = (owners(), small());
    assert_eq!(owners.len(), small.len());
    let mut stroked = 0;
    for (theirs, ours) in owners.iter().zip(&small) {
        let renamed = theirs
            .replace("#teal)", "#teal-small)")
            .replace("id=\"teal\"", "id=\"teal-small\"");
        if renamed == *ours {
            continue;
        }
        let paint = if ours.contains("stroke=\"url(#teal-small)\"") {
            "url(#teal-small)"
        } else {
            "#1b3e5e"
        };
        let with_strokes = format!(
            "fill=\"{paint}\" stroke=\"{paint}\" stroke-width=\"14\" stroke-linejoin=\"round\" stroke-linecap=\"round\" transform"
        );
        assert_eq!(
            renamed.replace(&format!("fill=\"{paint}\" transform"), &with_strokes),
            *ours
        );
        stroked += 1;
    }
    assert_eq!(stroked, 3, "the two arcs and the swoosh");
}

#[test]
fn the_mark_is_hidden_from_an_agent() {
    for template in [panel_document::TEMPLATE, rail_document::TEMPLATE] {
        assert!(template.contains(
            "<span class=\"mark\" aria-hidden=\"true\"><svg class=\"logo\" aria-hidden=\"true\""
        ));
    }
}
