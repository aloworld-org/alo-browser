/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A person's press on a closed list on a card's page (queue item 810,
//! `docs/design/shell.md`, approved by the owner 2026-11-06: *when the
//! person uses the page, the normal dropdown*).
//!
//! A closed list's menu is a window of Chromium's own, opened where the
//! list is in the page's window. A card's window is parked, unseen, so the
//! menu opened there was moved by macOS onto the nearest display, away
//! from the card (measured, queue item 808: 1637, 341, 67 × 58, opaque,
//! left open), taking the person's keys. So a person's first press on a
//! closed list of a card's page is held — not sent — and the card is
//! expanded: its page shown in its own window over the conversation (ADR
//! 0076's answer for a menu, a picker or an input method), where the press
//! is then made again, at the list's middle as the page is laid out there.
//! Chromium opens the list's own menu under it, as Chrome's does: the
//! normal dropdown. The press's release, and any drag before it, are the
//! held press's, and are not sent either.
//!
//! What a closed list is, here: a `<select>` with neither `multiple` nor a
//! `size` over one, as Chromium draws a list that drops down. Asked of
//! Chromium at the point (`DOM.getNodeForLocation`, `DOM.describeNode`),
//! never decided from a picture.

use crate::chrome_protocol::{self, Command, Described, Mouse};
use crate::chrome_tab::Tab;

/// Whether `described` is a closed list: a `<select>` that drops down.
pub fn is_closed_list(described: &Described) -> bool {
    let attribute = |name: &str| {
        described
            .attributes
            .iter()
            .find(|(each, _)| each.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim())
    };
    let size = attribute("size").map_or(Some(0), |size| size.parse::<u64>().ok());
    described.local_name.eq_ignore_ascii_case("select")
        && attribute("multiple").is_none()
        && size.is_some_and(|size| size <= 1)
}

/// The closed list at `x`, `y` of `tab`'s page, in its CSS pixels, by
/// Chrome's `backendNodeId` — or [`None`] when the point is over anything
/// else, or Chrome could not say.
pub fn closed_list_at(tab: &mut Tab<'_>, x: f64, y: f64) -> Option<u64> {
    // Chrome takes the point in whole CSS pixels.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a point of a card's page, rounded to Chrome's whole CSS pixels, far inside an i64"
    )]
    let (x, y) = (x.round() as i64, y.round() as i64);
    let node = chrome_protocol::node_at(&tab.ask(&Command::NodeAt { x, y }).ok()?).ok()?;
    let described = chrome_protocol::described(&tab.ask(&Command::Describe { node }).ok()?).ok()?;
    is_closed_list(&described).then_some(node)
}

/// Press the list `node` of `tab`'s page at its middle, as the page is laid
/// out now — the pointer arriving, the main button going down and up, a
/// first press — so Chromium opens its menu. Chrome declining any of it, or
/// the list no longer drawn, presses nothing.
pub fn press(tab: &mut Tab<'_>, node: u64) {
    let Some(quad) = tab
        .ask(&Command::WhereDrawn { node })
        .ok()
        .and_then(|drawn| chrome_protocol::quads(&drawn).ok())
        .and_then(|quads| quads.first().copied())
    else {
        return;
    };
    let (x, y) = middle(&quad);
    for (kind, clicks) in [(Mouse::Moved, 0), (Mouse::Pressed, 1), (Mouse::Released, 1)] {
        if tab.ask(&Command::Pointed { kind, x, y, clicks }).is_err() {
            return;
        }
    }
}

/// The middle of a quad Chrome drew a box as: its four corners' mean.
fn middle(quad: &[f64; 8]) -> (f64, f64) {
    let [x1, y1, x2, y2, x3, y3, x4, y4] = *quad;
    ((x1 + x2 + x3 + x4) / 4.0, (y1 + y2 + y3 + y4) / 4.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(name: &str, attributes: &[(&str, &str)]) -> Described {
        Described {
            node_name: name.to_ascii_uppercase(),
            local_name: name.to_owned(),
            attributes: attributes
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn a_select_that_drops_down_is_a_closed_list_and_nothing_else_is() {
        assert!(is_closed_list(&element("select", &[])));
        assert!(is_closed_list(&element(
            "select",
            &[("id", "s"), ("size", "1")]
        )));
        assert!(is_closed_list(&element("select", &[("size", "0")])));
        assert!(!is_closed_list(&element("select", &[("multiple", "")])));
        assert!(!is_closed_list(&element("select", &[("size", "4")])));
        assert!(!is_closed_list(&element("select", &[("size", "many")])));
        assert!(!is_closed_list(&element("input", &[("list", "x")])));
        assert!(!is_closed_list(&element("option", &[])));
    }

    #[test]
    fn a_quads_middle_is_its_corners_mean() {
        let (x, y) = middle(&[40.0, 40.0, 140.0, 40.0, 140.0, 64.0, 40.0, 64.0]);
        assert!((x - 90.0).abs() < 1e-9 && (y - 52.0).abs() < 1e-9);
    }
}
