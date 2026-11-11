/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An app's own views of the document a card shows, which read better than
//! the app's editor (ADR 0109 § 1, queue item 939).
//!
//! Some apps draw what they hold on a canvas, and their editor's tree says
//! little of it. Measured on 11 November 2026 (ADR 0109, *What was found*):
//! Google Docs' edit view gave the tree 6.9% of a doc's words, and the
//! same doc's own `/mobilebasic` view 99.8%; Google Sheets' edit view gave
//! 0.15% of a sheet's words, and its own `/htmlview` 100%, its grid in a
//! frame the tree now reads (queue item 937).
//!
//! These are views the app itself serves, at an address of its own beside
//! the document's, to anyone who may open the document. alo runs nothing in
//! the page for them and asks nothing of the app but the page: the agent
//! is told the view exists, by name and address, and **opens it as it opens
//! any site** — in a card the person sees, kept to the sites a run may open
//! (`crate::agent_opening`). Never a hidden read. And the run says which
//! view it read ([`named`]), in its step and in its result.
//!
//! Read only: a view is for reading. Changing a doc or a sheet is done in
//! its editor, by name, or not at all.
//!
//! A view is offered only for an address whose path names the document the
//! way the app's own links do — `/document/d/ID/…`, `/spreadsheets/d/ID/…`
//! on `docs.google.com` over `https` — and never for the view itself.

use alo_url::Url;

/// One of an app's own views of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    /// What it is, in the words the person and the model read.
    pub name: &'static str,
    /// Where it is.
    pub address: String,
}

/// The longest document id read from an address; Google's are 44
/// characters.
const LONGEST_ID: usize = 128;

/// The app's own views of the document at `url`, or none.
pub fn of(url: &Url) -> Vec<View> {
    if url.scheme != "https" || alo_url::reading::hostname(url) != "docs.google.com" {
        return Vec::new();
    }
    let path = alo_url::reading::pathname(url);
    let mut parts = path.trim_start_matches('/').split('/');
    let (Some(app), Some("d"), Some(id)) = (parts.next(), parts.next(), parts.next()) else {
        return Vec::new();
    };
    if !is_an_id(id) {
        return Vec::new();
    }
    let at = parts.next().unwrap_or_default();
    let (name, view) = match app {
        "document" => (DOCS, "mobilebasic"),
        "spreadsheets" => (SHEETS, "htmlview"),
        _ => return Vec::new(),
    };
    if at == view {
        return Vec::new();
    }
    vec![View {
        name,
        address: format!("https://docs.google.com/{app}/d/{id}/{view}"),
    }]
}

/// What Google Docs' `/mobilebasic` is called.
pub const DOCS: &str = "Google Docs' own reading view of this doc";

/// What Google Sheets' `/htmlview` is called.
pub const SHEETS: &str = "Google Sheets' own view of the whole spreadsheet";

/// The view `opened` is, if it is one of `offered` — so that a run that
/// opened it says which view it read.
pub fn named<'a>(offered: &'a [View], opened: &Url) -> Option<&'a View> {
    offered.iter().find(|view| {
        alo_url::parse(&view.address).is_ok_and(|address| {
            alo_url::reading::hostname(&address) == alo_url::reading::hostname(opened)
                && alo_url::reading::pathname(&address) == alo_url::reading::pathname(opened)
        })
    })
}

/// Whether `id` is one an app's own link names a document by: letters,
/// digits, `-` and `_`, at most [`LONGEST_ID`].
fn is_an_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= LONGEST_ID
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn views(address: &str) -> Vec<View> {
        of(&alo_url::parse(address).unwrap())
    }

    const DOC: &str = "1_jnAifzWv0llivFWolSXxdcFJ4ksjG8eM3MdNjTqAcY";
    const SHEET: &str = "1EHKxumqodrsiYK3MpoB44El58q8jzRatr487vNSRwfg";

    #[test]
    fn a_doc_and_a_sheet_are_offered_their_own_views_by_name() {
        assert_eq!(
            views(&format!(
                "https://docs.google.com/document/d/{DOC}/edit?tab=t.0"
            )),
            vec![View {
                name: DOCS,
                address: format!("https://docs.google.com/document/d/{DOC}/mobilebasic"),
            }]
        );
        assert_eq!(
            views(&format!("https://docs.google.com/document/d/{DOC}")),
            vec![View {
                name: DOCS,
                address: format!("https://docs.google.com/document/d/{DOC}/mobilebasic"),
            }]
        );
        assert_eq!(
            views(&format!(
                "https://docs.google.com/spreadsheets/d/{SHEET}/edit?gid=0#gid=0"
            )),
            vec![View {
                name: SHEETS,
                address: format!("https://docs.google.com/spreadsheets/d/{SHEET}/htmlview"),
            }]
        );
    }

    #[test]
    fn nothing_else_is_offered_a_view() {
        for address in [
            // The views themselves.
            format!("https://docs.google.com/document/d/{DOC}/mobilebasic"),
            format!("https://docs.google.com/spreadsheets/d/{SHEET}/htmlview"),
            // Not over https, not Google's, or not a document.
            format!("http://docs.google.com/document/d/{DOC}/edit"),
            format!("https://docs.google.com.example/document/d/{DOC}/edit"),
            format!("https://example.com/document/d/{DOC}/edit"),
            "https://docs.google.com/presentation/d/abc/edit".to_owned(),
            "https://docs.google.com/document/u/0/".to_owned(),
            "https://docs.google.com/".to_owned(),
            // An id no link of the app's would name.
            "https://docs.google.com/document/d/a.b/edit".to_owned(),
            format!(
                "https://docs.google.com/document/d/{}/edit",
                "a".repeat(129)
            ),
        ] {
            assert_eq!(views(&address), Vec::new(), "{address}");
        }
    }

    #[test]
    fn a_view_opened_is_named_and_another_page_is_not() {
        let offered = views(&format!("https://docs.google.com/document/d/{DOC}/edit"));
        let opened = |address: &str| named(&offered, &alo_url::parse(address).unwrap());
        assert_eq!(
            opened(&format!(
                "https://docs.google.com/document/d/{DOC}/mobilebasic"
            ))
            .map(|view| view.name),
            Some(DOCS)
        );
        assert_eq!(
            opened(&format!("https://docs.google.com/document/d/{DOC}/edit")),
            None
        );
        assert_eq!(opened("https://example.com/"), None);
    }
}
