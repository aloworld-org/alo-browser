/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the conversation's window is called, and the name the system knows
//! the application by (queue item 958, ADR 0112 § 1).
//!
//! alo OS asks of a browser that its window's title be the page's title and
//! its Wayland `app_id` the name of its desktop entry. So the window is
//! called what the open card's page is called ([`of`]), and **alo** with no
//! card open, or a card whose page has said no title yet.
//!
//! # Who draws the title
//!
//! A page's title is a stranger's string, and ADR 0024 § 4 keeps a
//! stranger's string from being shaped in the browser process. Under
//! Wayland with server-side decorations — the only decorations alo OS
//! allows — the compositor draws a window's title, in its own process, so
//! the title is handed to it as data ([`shown`]). On macOS `AppKit` draws the
//! title bar, the *Window* menu and the Dock's menu **in this process**, so
//! there the window stays **alo** whatever the page is called, until an
//! ADR says otherwise: the rule ADR 0024 § 4 made is not loosened for a
//! platform that did not ask.
//!
//! # The application's name
//!
//! [`APP_ID`] is the reverse-DNS identifier queue item 483 chose,
//! `org.aloworld.Browser`: the desktop entry's name on Linux and the
//! bundle's identifier on macOS, where the system already names the
//! application by its bundle and `winit` has no `app_id` to set.

/// The name the system knows alo by: its desktop entry's, and its bundle's.
pub const APP_ID: &str = crate::info_plist::IDENTIFIER;

/// What the window is called with no page in a card.
pub const ALONE: &str = "alo";

/// What the window is called: the open card's page's title, or [`ALONE`]
/// with none, or one that is empty or only spaces.
pub fn of(page: Option<&str>) -> String {
    page.map(str::trim)
        .filter(|title| !title.is_empty())
        .map_or_else(|| ALONE.to_owned(), str::to_owned)
}

/// What is handed to the window server as the window's title, of `title`
/// ([`of`]): `title` where the window server draws it in its own process,
/// and [`ALONE`] where this process would draw it — macOS.
pub fn shown(title: &str) -> &str {
    if cfg!(target_os = "macos") {
        ALONE
    } else {
        title
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_is_called_as_its_cards_page_is_and_alo_with_none() {
        assert_eq!(of(Some("Invoices — Acme")), "Invoices — Acme");
        assert_eq!(of(Some("  Inbox  ")), "Inbox");
        assert_eq!(of(Some("")), ALONE);
        assert_eq!(of(Some("   ")), ALONE);
        assert_eq!(of(None), ALONE);
    }

    #[test]
    fn a_pages_title_is_handed_only_to_a_window_server_that_draws_it_itself() {
        let shown = shown("Invoices — Acme");
        if cfg!(target_os = "macos") {
            assert_eq!(shown, ALONE);
        } else {
            assert_eq!(shown, "Invoices — Acme");
        }
        assert_eq!(super::shown(ALONE), ALONE);
    }

    #[test]
    fn the_application_is_named_by_its_reverse_dns_identifier() {
        assert_eq!(APP_ID, "org.aloworld.Browser");
    }
}
