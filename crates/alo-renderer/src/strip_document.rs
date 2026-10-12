/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The tab strip's document, built from its state.
//!
//! ADR 0024 § 4: *"The strip's renderer builds its document with `alo-dom`'s
//! own operations from a template it was compiled with, and a title goes in
//! as a text node, so there is no string a page can choose that becomes an
//! element. No markup is ever assembled by concatenation."*
//!
//! So the only markup here is [`TEMPLATE`], ours and fixed, parsed once per
//! strip; everything a tab says is put into it with `create_element`,
//! `set_attribute`, `create_text_node` and `append_child`. A title holding
//! `<b>`, `</style>` or a NUL is a text node holding those characters, and
//! the tree an agent reads and the picture a person sees both show them as
//! written.
//!
//! # What an agent reads
//!
//! ADR 0002: the strip is read as the interface it is — *"tab list, four
//! tabs, the second selected"* — so the tabs are a `tablist` named "Tabs",
//! each tab a `tab` named by its title, `aria-selected` saying which one is
//! chosen and `aria-busy` which are loading. A tab whose renderer has gone
//! says [`GONE`] after its title, in words a person and an agent both read.
//!
//! It runs no script (§ 4): nothing here is ever a `<script>`, and the
//! template has none.

use crate::strip::{Condition, Strip, StripTab};
use alo_dom::{Document, NodeId, parse_document};

/// The strip's markup, compiled in: a document with one empty `tablist`.
pub const TEMPLATE: &str = include_str!("../interface/strip.html");

/// The strip's stylesheet, after alo's design, compiled in.
pub const SHEET: &str = include_str!("../interface/strip.css");

/// What a tab whose renderer has gone says after its title.
pub const GONE: &str = "Stopped";

/// The id of the template's `tablist`, which every tab is put into.
const TABS: &str = "tabs";

/// Why the strip could not be built: an `alo-dom` operation refused, or the
/// template had no `tablist`. Either is a defect in this file or its
/// template, never in what the browser process sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unbuilt {
    /// In words.
    pub why: String,
}

/// The strip's document for `strip`: the template, with one tab in its
/// `tablist` for each tab, in order.
///
/// # Errors
///
/// [`Unbuilt`] if the template has no `tablist` or `alo-dom` refused an
/// operation, which no state can cause.
pub fn build(strip: &Strip) -> Result<Document, Unbuilt> {
    let mut document = parse_document(TEMPLATE);
    let list = tablist(&document).ok_or_else(|| Unbuilt {
        why: format!("the strip's template has no element with the id {TABS:?}"),
    })?;
    for tab in &strip.tabs {
        let made = make_tab(&mut document, tab)?;
        append(&mut document, list, made)?;
    }
    Ok(document)
}

/// The template's `tablist`.
fn tablist(document: &Document) -> Option<NodeId> {
    document
        .descendants(document.root())
        .find(|&node| document.attribute_by_name(node, "id") == Some(TABS))
}

/// One tab: a `tab` holding a mark if it is loading, its title as text, and
/// [`GONE`] if its renderer has gone.
fn make_tab(document: &mut Document, tab: &StripTab) -> Result<NodeId, Unbuilt> {
    let made = element(document, "div")?;
    attribute(document, made, "class", "tab")?;
    attribute(document, made, "role", "tab")?;
    let selected = if tab.selected { "true" } else { "false" };
    attribute(document, made, "aria-selected", selected)?;
    match tab.condition {
        Condition::Shown => {}
        Condition::Loading => {
            attribute(document, made, "aria-busy", "true")?;
            let mark = element(document, "span")?;
            attribute(document, mark, "class", "loading")?;
            attribute(document, mark, "aria-hidden", "true")?;
            append(document, made, mark)?;
        }
        Condition::Gone => attribute(document, made, "data-condition", "gone")?,
    }
    let title = element(document, "span")?;
    attribute(document, title, "class", "title")?;
    // The one place a stranger's string enters the document, and it enters
    // as a text node's data: no parser ever sees it.
    let text = document.create_text_node(tab.label());
    append(document, title, text)?;
    append(document, made, title)?;
    if tab.condition == Condition::Gone {
        let said = element(document, "span")?;
        attribute(document, said, "class", "gone")?;
        // A flex item of its own, so the tab is read as "Notes Stopped":
        // a flex item stands apart from its neighbour in a name, as Chrome
        // reads one (queue item 530).
        let text = document.create_text_node(GONE);
        append(document, said, text)?;
        append(document, made, said)?;
    }
    Ok(made)
}

fn element(document: &mut Document, local: &str) -> Result<NodeId, Unbuilt> {
    document.create_element(local).map_err(|refused| Unbuilt {
        why: format!("a <{local}> could not be made: {refused}"),
    })
}

fn attribute(
    document: &mut Document,
    node: NodeId,
    local: &str,
    value: &str,
) -> Result<(), Unbuilt> {
    document
        .set_attribute(node, local, value)
        .ok_or_else(|| Unbuilt {
            why: format!("{local} could not be set on a node that is not an element"),
        })
}

fn append(document: &mut Document, parent: NodeId, child: NodeId) -> Result<(), Unbuilt> {
    document
        .append_child(parent, child)
        .map(|_| ())
        .map_err(|refused| Unbuilt {
            why: format!("a node could not be put into the strip: {refused}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::NodeKind;

    fn strip(tabs: Vec<StripTab>) -> Strip {
        Strip {
            tabs,
            viewport: alo_layout::Size::new(400.0, 40.0),
        }
    }

    fn built(tabs: Vec<StripTab>) -> Document {
        build(&strip(tabs)).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why))
    }

    /// Every element's local name, in document order.
    fn elements(document: &Document) -> Vec<String> {
        document
            .descendants(document.root())
            .filter_map(|node| document.element(node))
            .map(|element| element.name.local.to_string())
            .collect()
    }

    fn tabs_of(document: &Document) -> Vec<NodeId> {
        let Some(list) = tablist(document) else {
            return Vec::new();
        };
        document.children(list).collect()
    }

    #[test]
    fn the_template_has_a_tablist_and_no_script() {
        let document = built(Vec::new());
        assert!(tablist(&document).is_some());
        assert!(!elements(&document).iter().any(|name| name == "script"));
        assert!(tabs_of(&document).is_empty());
    }

    #[test]
    fn each_tab_is_put_in_order_with_its_state() {
        let document = built(vec![
            StripTab::new("Inbox", "https://mail.example/"),
            StripTab::new("Invoices", "https://books.example/")
                .selected()
                .in_condition(Condition::Loading),
            StripTab::new("Calendar", "https://cal.example/").in_condition(Condition::Gone),
        ]);
        let tabs = tabs_of(&document);
        assert_eq!(tabs.len(), 3);
        let said: Vec<String> = tabs.iter().map(|&tab| document.text_content(tab)).collect();
        // The document's text has no space between the title and the word:
        // the tab is *read* with one, from its two flex items (queue item
        // 530), which `tests/the_tab_strip.rs` asserts through the agent.
        assert_eq!(said, vec!["Inbox", "Invoices", "CalendarStopped"]);
        let selected: Vec<Option<&str>> = tabs
            .iter()
            .map(|&tab| document.attribute_by_name(tab, "aria-selected"))
            .collect();
        assert_eq!(selected, vec![Some("false"), Some("true"), Some("false")]);
        let busy: Vec<Option<&str>> = tabs
            .iter()
            .map(|&tab| document.attribute_by_name(tab, "aria-busy"))
            .collect();
        assert_eq!(busy, vec![None, Some("true"), None]);
        assert_eq!(
            tabs.get(2)
                .and_then(|&tab| document.attribute_by_name(tab, "data-condition")),
            Some("gone")
        );
    }

    #[test]
    fn a_title_that_looks_like_markup_is_one_text_node() {
        for title in [
            "<b>bold</b>",
            "</style><script>x()</script>",
            "a\0b",
            "\u{202e}abc",
        ] {
            let document = built(vec![StripTab::new(title, "https://a.example/")]);
            let names = elements(&document);
            assert!(
                !names.iter().any(|name| name == "b"),
                "{title:?}: {names:?}"
            );
            assert_eq!(
                names.iter().filter(|name| *name == "script").count(),
                0,
                "{title:?}"
            );
            let texts: Vec<String> = document
                .descendants(document.root())
                .filter_map(|node| match document.kind(node) {
                    Some(NodeKind::Text(data)) if data == title => Some(data.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(texts, vec![title.to_owned()], "{title:?}");
        }
    }

    #[test]
    fn a_tab_with_no_title_is_its_address() {
        let document = built(vec![StripTab::new("", "https://example.com/")]);
        let tabs = tabs_of(&document);
        assert_eq!(
            tabs.first().map(|&tab| document.text_content(tab)),
            Some("https://example.com/".to_owned())
        );
    }
}
