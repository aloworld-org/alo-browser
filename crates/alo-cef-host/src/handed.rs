/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which addresses a page in the host is never sent to: those Chromium
//! does not load itself, and would hand to another program (queue item
//! 777, ADR 0060 § 2 as amended).
//!
//! A link to `mailto:` pressed in a page — or a script's `location` set to
//! it — asks Chromium for an address it cannot load: the machine's Chrome
//! offers it to the program the machine names for it and leaves the page
//! as it was, nothing committed; CEF, by default, commits Chrome's error
//! page (`chrome-error://chromewebdata/`) over the person's page, so a card
//! showed an error where the page had been (measured, 770). alo refuses a
//! page that sends its tab to such a scheme (ADR 0060 § 2, `chrome_gate.rs`
//! in `alo-window`), and a refusal is no error page: so the host cancels
//! the navigation **before** Chromium decides it (`window.rs`, CEF's
//! `OnBeforeBrowse`), and the page stays where it was, drawn, with nothing
//! handed to another program.
//!
//! What the host still sends a page to is every scheme Chromium loads
//! itself ([`LOADED`]) — the web, the documents a page makes (`data:`,
//! `blob:`, `about:`), Chromium's own pages — so that whatever alo's gate
//! decides of those is still alo's to decide, and nothing the host was
//! started on (its `--app=data:,`) is cancelled. Everything else is a
//! scheme of another program's, or of nobody's, and is never asked for.

/// The schemes Chromium loads in a page itself, lowercase: every other is
/// handed to another program, or to none.
pub const LOADED: [&str; 16] = [
    "http",
    "https",
    "ws",
    "wss",
    "file",
    "data",
    "blob",
    "filesystem",
    "about",
    "javascript",
    "chrome",
    "chrome-error",
    "chrome-untrusted",
    "chrome-extension",
    "devtools",
    "view-source",
];

/// Whether a navigation to `address` — as Chromium gives it, already
/// canonical — is one the host cancels: an address whose scheme Chromium
/// does not load itself. An address with no scheme to read is Chromium's
/// to refuse, and is not cancelled here.
pub fn cancelled(address: &str) -> bool {
    scheme(address).is_some_and(|scheme| {
        !LOADED
            .iter()
            .any(|loaded| loaded.eq_ignore_ascii_case(scheme))
    })
}

/// The scheme `address` begins with, as URL's grammar reads one: a letter,
/// then letters, digits, `+`, `-` or `.`, up to the first `:`.
fn scheme(address: &str) -> Option<&str> {
    let (scheme, _) = address.split_once(':')?;
    let mut letters = scheme.chars();
    let first = letters.next()?;
    (first.is_ascii_alphabetic()
        && letters.all(|letter| letter.is_ascii_alphanumeric() || "+-.".contains(letter)))
    .then_some(scheme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scheme_handed_to_another_program_is_cancelled() {
        for address in [
            "mailto:someone@example.com",
            "MAILTO:someone@example.com",
            "tel:+15550100",
            "sms:+15550100",
            "facetime:someone@example.com",
            "ftp://example.com/file",
            "intent://scan/#Intent;scheme=zxing;end",
            "web+alo:anything",
            "x-nobodys.scheme:at-all",
        ] {
            assert!(cancelled(address), "{address}");
        }
    }

    #[test]
    fn what_chromium_loads_itself_is_let_go() {
        for address in [
            "http://127.0.0.1:8080/one.html",
            "https://example.com/",
            "HTTPS://example.com/",
            "data:,",
            "data:text/html,<p>hi",
            "blob:http://127.0.0.1/1f2e",
            "about:blank",
            "about:srcdoc",
            "file:///tmp/x.html",
            "javascript:void(0)",
            "chrome-error://chromewebdata/",
            "chrome://version/",
            "devtools://devtools/bundled/inspector.html",
            "view-source:https://example.com/",
            "ws://127.0.0.1/socket",
            "filesystem:http://127.0.0.1/temporary/x",
        ] {
            assert!(!cancelled(address), "{address}");
        }
    }

    #[test]
    fn an_address_with_no_scheme_to_read_is_chromiums_to_refuse() {
        for address in [
            "",
            "no scheme here",
            ":empty",
            "1http://x/",
            "ht tp://x/",
            "a/b:c",
        ] {
            assert!(!cancelled(address), "{address:?}");
        }
    }

    #[test]
    fn every_scheme_let_go_is_written_as_a_scheme() {
        for loaded in LOADED {
            let address = format!("{loaded}:x");
            assert_eq!(scheme(&address), Some(loaded));
            assert_eq!(loaded.to_ascii_lowercase(), loaded);
        }
    }
}
