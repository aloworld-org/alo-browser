/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The application's credits: where a person reaches the licences of what
//! `alo browser.app` carries (queue item 993; ADR 0079 § 1, ADR 0077 *What
//! it costs* 5).
//!
//! Until 993 the application's own page linked CEF's `LICENSE.txt` and
//! Chromium's `CREDITS.html` (queue item 720). Since the application opens
//! the conversation window instead, the licences are reached as on any Mac
//! application: **its menu's *About alo***, the panel `AppKit` draws, whose
//! credits are the main bundle's `Contents/Resources/Credits.html` when
//! there is one. That panel and its menu item are macOS's and `winit`'s
//! default menu's, not ours — nothing of alo's is drawn or called to show
//! them, and no `unsafe` is added.
//!
//! [`put_in`] writes `Credits.html` once everything the application
//! carries is in it and before the seal: a sentence, and a link to every
//! file in `Contents/Resources/licences` — the typefaces' licences and the
//! filter lists' always, CEF's and Chromium's when it carries a Chromium —
//! and to that folder. The links are relative; `AppKit` reads the file at
//! its own address, so each resolves inside the application (measured with
//! `NSAttributedString`'s HTML import on macOS 26.5: `licences/LICENSE.txt`
//! became `file:///…/Contents/Resources/licences/LICENSE.txt`). A link
//! pressed in the panel is opened by macOS — a folder in the Finder, a
//! file in the person's own application for it — never by a Chromium alo
//! starts.

use crate::bundle_carried::LICENCES;
use core::fmt::Write as _;
use std::path::Path;

/// The credits' file name in `Contents/Resources`, the one `AppKit`'s
/// standard *About* panel reads.
pub const FILE: &str = "Credits.html";

/// The credits for licences called `names`, in `Contents/Resources/licences`:
/// a sentence, a link to each, and one to the folder.
pub fn text(names: &[String]) -> String {
    let mut links = String::new();
    for name in names {
        let _ = writeln!(
            links,
            "<li><a href=\"{LICENCES}/{}\">{}</a></li>",
            href(name),
            escaped(name)
        );
    }
    format!(
        "<!doctype html>\n<meta charset=\"utf-8\">\n\
         <p>alo browser carries the work of others, each under its own licence: the \
         Chromium it draws sites with (CEF's licence and Chromium's credits, when it \
         carries one), the typefaces of its interface, and the EasyList and EasyPrivacy \
         lists.</p>\n<ul>\n{links}</ul>\n\
         <p>All of them are in the application's <a href=\"{LICENCES}/\">licences</a> \
         folder.</p>\n"
    )
}

/// Write `Credits.html` into the application at `application`, linking every
/// file now in its `Contents/Resources/licences`, in byte order.
///
/// # Errors
///
/// What could not be read or written, and why, in words.
pub fn put_in(application: &Path) -> Result<(), String> {
    let resources = application.join("Contents").join("Resources");
    let folder = resources.join(LICENCES);
    let mut names = Vec::new();
    for entry in std::fs::read_dir(&folder)
        .map_err(|why| format!("{} could not be read: {why}", folder.display()))?
    {
        let entry =
            entry.map_err(|why| format!("{} could not be read: {why}", folder.display()))?;
        let name = entry.file_name().into_string().map_err(|name| {
            format!(
                "{} holds {}, a name that is not text",
                folder.display(),
                name.display()
            )
        })?;
        names.push(name);
    }
    names.sort();
    let file = resources.join(FILE);
    std::fs::write(&file, text(&names))
        .map_err(|why| format!("{} could not be written: {why}", file.display()))
}

/// `name` as a relative address's last part: what is not a letter, a
/// digit, `.`, `-` or `_` written as `%XX`.
fn href(name: &str) -> String {
    name.bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_') {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

/// `text` as markup's text, its markup characters as references.
fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_licence_is_linked_where_the_application_carries_it() {
        let names = ["CREDITS.html".to_owned(), "LICENSE.txt".to_owned()];
        let credits = text(&names);
        assert!(credits.contains("<li><a href=\"licences/CREDITS.html\">CREDITS.html</a></li>"));
        assert!(credits.contains("<li><a href=\"licences/LICENSE.txt\">LICENSE.txt</a></li>"));
        assert!(credits.contains("<a href=\"licences/\">licences</a>"));
        assert_eq!(credits.matches("<li>").count(), 2);
    }

    #[test]
    fn a_name_is_an_address_and_text_never_markup() {
        let credits = text(&["a b&<c>.txt".to_owned()]);
        assert!(
            credits.contains("<a href=\"licences/a%20b%26%3Cc%3E.txt\">a b&amp;&lt;c&gt;.txt</a>"),
            "{credits}"
        );
    }

    #[test]
    fn the_credits_link_what_the_folder_holds_in_byte_order() {
        let root = std::env::temp_dir().join(format!("alo-credits-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let licences = root.join("Contents/Resources").join(LICENCES);
        assert!(std::fs::create_dir_all(&licences).is_ok());
        for name in ["OFL-Manrope.txt", "LICENSE.txt", "CREDITS.html"] {
            assert!(std::fs::write(licences.join(name), "text").is_ok());
        }
        assert_eq!(put_in(&root), Ok(()));
        let written = std::fs::read_to_string(root.join("Contents/Resources").join(FILE));
        let names = ["CREDITS.html", "LICENSE.txt", "OFL-Manrope.txt"];
        assert_eq!(
            written.ok(),
            Some(text(&names.map(str::to_owned))),
            "every file, in byte order"
        );
        let _ = std::fs::remove_dir_all(&root);
        assert!(put_in(&root).is_err(), "no folder, said");
    }
}
