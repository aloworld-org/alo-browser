/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The chats panel's document, built from its state (ADR 0074, queue item
//! 699), the way the tab strip's is (ADR 0024 § 4): the only markup is
//! [`TEMPLATE`], ours and fixed, and every chat is put into it with
//! `create_element`, `set_attribute`, `create_text_node` and
//! `append_child`. A name holding `<b>` or `</style>` is a text node holding
//! those characters.
//!
//! # What an agent reads
//!
//! ADR 0002: *"navigation Chats: Open, three chats, the first current, the
//! second at work"*. The panel is a `navigation` named *Chats*; each group a
//! heading and a list; each chat a link named by its name, `aria-current`
//! when it is on screen and `aria-busy` while the agent works in it; a chat
//! the agent finished says [`DONE`] after its name, in words a person and an
//! agent both read. The site's mark is decoration and is hidden from the
//! tree: the row is named by its chat, not by a letter. So are the icons
//! before *New chat* and *Settings* (queue item 712): inline SVG written in
//! the template, stroked in `currentColor`, and `aria-hidden`.
//!
//! # Search chats, and hiding the panel
//!
//! Queue item 711. *Search chats* is a `search` holding a field named so;
//! what a person typed into it is the field's `value`, set with `alo-dom`'s
//! operations and never parsed, and while it has the keyboard the panel's
//! `nav` is of the class `panel searching`. The chats it is sent are the
//! ones the browser process found ([`crate::panel::matching`]); when none
//! is, a line says nobody is called that ([`none_called`]). The button that
//! hides the panel is a `button` named *Hide the panel*. Hidden, the panel
//! is the rail, built by [`crate::rail_document`].
//!
//! It runs no script: the template has none and nothing here makes one.

use crate::panel::{Chat, Doing, Panel};
use alo_dom::{Document, NodeId, parse_document};

/// The panel's markup, compiled in: the mark, *New chat*, an empty place for
/// the chats, and *Settings*.
pub const TEMPLATE: &str = include_str!("../interface/panel.html");

/// The panel's stylesheet, after the owner's design, compiled in.
pub const SHEET: &str = include_str!("../interface/panel.css");

/// What a chat the agent finished says after its name.
pub const DONE: &str = "Done";

/// The heading over the open chats.
pub const OPEN: &str = "Open";

/// The heading over the chats from before.
pub const EARLIER: &str = "Earlier";

/// The id of the template's place for the chats.
const CHATS: &str = "chats";

/// The id of the template's `nav`, the whole panel.
const PANEL: &str = "panel";

/// The id of the template's *Search chats* field.
const FIND: &str = "find";

/// What the panel says when no chat is called what was `searched`.
pub fn none_called(searched: &str) -> String {
    format!("No chat is called \u{201c}{}\u{201d}", searched.trim())
}

/// Why the panel could not be built: an `alo-dom` operation refused, or the
/// template had no place for the chats. Either is a defect in this file or
/// its template, never in what the browser process sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unbuilt {
    /// In words.
    pub why: String,
}

/// The panel's document for `panel`: the template, with a group for the
/// open chats and one for the earlier ones — each only if it has a chat —
/// in the order the panel keeps them.
///
/// # Errors
///
/// [`Unbuilt`] if the template has no place for the chats or `alo-dom`
/// refused an operation, which no state can cause.
pub fn build(panel: &Panel) -> Result<Document, Unbuilt> {
    if panel.hidden {
        return crate::rail_document::build();
    }
    let mut document = parse_document(TEMPLATE);
    let place = by_id(&document, CHATS)?;
    if panel.searching {
        let nav = by_id(&document, PANEL)?;
        attribute(&mut document, nav, "class", "panel searching")?;
    }
    if !panel.searched.is_empty() {
        let field = by_id(&document, FIND)?;
        // A stranger's string, entering as an attribute's value.
        attribute(&mut document, field, "value", &panel.searched)?;
        if panel.chats.is_empty() {
            let none = element(&mut document, "p")?;
            attribute(&mut document, none, "class", "none")?;
            let words = document.create_text_node(&none_called(&panel.searched));
            append(&mut document, none, words)?;
            append(&mut document, place, none)?;
        }
    }
    let open: Vec<&Chat> = panel.chats.iter().filter(|chat| chat.open).collect();
    let earlier: Vec<&Chat> = panel.chats.iter().filter(|chat| !chat.open).collect();
    for (heading, chats, class) in [(OPEN, open, "open"), (EARLIER, earlier, "earlier")] {
        if chats.is_empty() {
            continue;
        }
        let group = make_group(&mut document, heading, class, &chats)?;
        append(&mut document, place, group)?;
    }
    Ok(document)
}

/// One group: its heading, and a list holding each of its chats.
fn make_group(
    document: &mut Document,
    heading: &str,
    class: &str,
    chats: &[&Chat],
) -> Result<NodeId, Unbuilt> {
    let group = element(document, "section")?;
    attribute(document, group, "class", class)?;
    let id = format!("{class}-heading");
    attribute(document, group, "aria-labelledby", &id)?;
    let title = element(document, "h2")?;
    attribute(document, title, "class", "group")?;
    attribute(document, title, "id", &id)?;
    let words = document.create_text_node(heading);
    append(document, title, words)?;
    append(document, group, title)?;
    let list = element(document, "ul")?;
    attribute(document, list, "class", "chats")?;
    for chat in chats {
        let item = element(document, "li")?;
        let row = make_chat(document, chat)?;
        append(document, item, row)?;
        append(document, list, item)?;
    }
    append(document, group, list)?;
    Ok(group)
}

/// One chat's row: a link holding its site's mark, its name as text, and
/// what the agent is doing in it.
fn make_chat(document: &mut Document, chat: &Chat) -> Result<NodeId, Unbuilt> {
    let row = element(document, "a")?;
    attribute(document, row, "class", "chat")?;
    attribute(document, row, "href", "#chat")?;
    if chat.current {
        attribute(document, row, "aria-current", "page")?;
    }
    if let Some(mark) = chat.mark() {
        let site = element(document, "span")?;
        attribute(document, site, "class", "site")?;
        attribute(document, site, "aria-hidden", "true")?;
        let letter = document.create_text_node(&mark);
        append(document, site, letter)?;
        append(document, row, site)?;
    }
    let name = element(document, "span")?;
    attribute(document, name, "class", "name")?;
    // The one place a stranger's string enters the document, and it enters
    // as a text node's data: no parser ever sees it.
    let text = document.create_text_node(chat.label());
    append(document, name, text)?;
    append(document, row, name)?;
    match chat.doing {
        Doing::Nothing => {}
        Doing::Working => {
            attribute(document, row, "aria-busy", "true")?;
            let dot = element(document, "span")?;
            attribute(document, dot, "class", "working")?;
            attribute(document, dot, "aria-hidden", "true")?;
            append(document, row, dot)?;
        }
        Doing::Done => {
            // A flex item of its own, so the row is read as "Charging my EV
            // Done", as a gone tab is read (queue item 530).
            let said = element(document, "span")?;
            attribute(document, said, "class", "done")?;
            let words = document.create_text_node(DONE);
            append(document, said, words)?;
            append(document, row, said)?;
        }
    }
    Ok(row)
}

/// The template's element with `id`.
fn by_id(document: &Document, id: &str) -> Result<NodeId, Unbuilt> {
    document
        .descendants(document.root())
        .find(|&node| document.attribute_by_name(node, "id") == Some(id))
        .ok_or_else(|| Unbuilt {
            why: format!("the panel's template has no element with the id {id:?}"),
        })
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
            why: format!("a node could not be put into the panel: {refused}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_dom::NodeKind;

    fn built(chats: Vec<Chat>) -> Document {
        let panel = Panel {
            chats,
            viewport: alo_layout::Size::new(240.0, 640.0),
            ..Panel::default()
        };
        build(&panel).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why))
    }

    fn of_class(document: &Document, class: &str) -> Vec<NodeId> {
        document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some(class))
            .collect()
    }

    #[test]
    fn search_chats_holds_what_was_typed_as_its_value_and_says_when_none_is_called_that() {
        let empty = built(Vec::new());
        let field = by_id(&empty, FIND).ok();
        assert_eq!(
            field.and_then(|field| empty.attribute_by_name(field, "value")),
            None
        );
        assert_eq!(of_class(&empty, "find").len(), 1);
        assert_eq!(of_class(&empty, "hide").len(), 1);
        assert!(of_class(&empty, "none").is_empty(), "nothing searched");
        let nav = by_id(&empty, PANEL).ok();
        assert_eq!(
            nav.and_then(|nav| empty.attribute_by_name(nav, "class")),
            Some("panel")
        );

        let hostile = "</nav><script>x()</script>";
        let searching = |chats: Vec<Chat>| {
            build(&Panel {
                chats,
                searched: hostile.to_owned(),
                searching: true,
                ..Panel::default()
            })
            .unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why))
        };
        let none = searching(Vec::new());
        let field = by_id(&none, FIND).ok();
        assert_eq!(
            field.and_then(|field| none.attribute_by_name(field, "value")),
            Some(hostile)
        );
        let nav = by_id(&none, PANEL).ok();
        assert_eq!(
            nav.and_then(|nav| none.attribute_by_name(nav, "class")),
            Some("panel searching")
        );
        let said: Vec<String> = of_class(&none, "none")
            .into_iter()
            .map(|node| none.text_content(node))
            .collect();
        assert_eq!(said, vec![none_called(hostile)]);
        assert!(!elements(&none).iter().any(|name| name == "script"));
        // Some found: they are the chats, and no line.
        let found = searching(vec![Chat::new("a", "").current()]);
        assert!(of_class(&found, "none").is_empty());
        assert_eq!(rows(&found).len(), 1);
    }

    #[test]
    fn a_hidden_panel_is_the_rail() {
        let rail = build(&Panel {
            chats: vec![Chat::new("a", "").current()],
            hidden: true,
            searched: "a".to_owned(),
            ..Panel::default()
        })
        .unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        assert!(rows(&rail).is_empty(), "the rail draws no chat");
        assert_eq!(of_class(&rail, "rbtn new").len(), 1);
        assert_eq!(of_class(&rail, "rbtn show").len(), 1);
        assert!(of_class(&rail, "find").is_empty());
        assert!(of_class(&rail, "hide").is_empty());
    }

    fn elements(document: &Document) -> Vec<String> {
        document
            .descendants(document.root())
            .filter_map(|node| document.element(node))
            .map(|element| element.name.local.to_string())
            .collect()
    }

    /// Every chat row, in document order.
    fn rows(document: &Document) -> Vec<NodeId> {
        document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some("chat"))
            .collect()
    }

    /// Every group heading's words, in order.
    fn headings(document: &Document) -> Vec<String> {
        document
            .descendants(document.root())
            .filter(|&node| document.attribute_by_name(node, "class") == Some("group"))
            .map(|node| document.text_content(node))
            .collect()
    }

    #[test]
    fn an_empty_panel_has_its_frame_no_groups_and_no_script() {
        let document = built(Vec::new());
        assert!(rows(&document).is_empty());
        assert!(headings(&document).is_empty());
        assert!(!elements(&document).iter().any(|name| name == "script"));
        let text = document.text_content(document.root());
        assert!(
            text.contains("New chat") && text.contains("Settings"),
            "{text}"
        );
    }

    #[test]
    fn open_chats_and_earlier_ones_are_put_in_their_groups_in_order() {
        let document = built(vec![
            Chat::new("Charging my EV", "evhomeguide.org").current(),
            Chat::new("Compare phone plans", "").earlier(),
            Chat::new("Weekend in Lisbon", "lisbon.example").doing(Doing::Working),
            Chat::new("Invoice for March", "books.example").doing(Doing::Done),
        ]);
        assert_eq!(headings(&document), vec![OPEN, EARLIER]);
        let said: Vec<String> = rows(&document)
            .iter()
            .map(|&row| document.text_content(row))
            .collect();
        // The mark's letter is in the text; the agent reads past it, because
        // it is hidden (`tests/the_chats_panel.rs` asserts through the agent).
        assert_eq!(
            said,
            vec![
                "ECharging my EV",
                "LWeekend in Lisbon",
                "BInvoice for MarchDone",
                "Compare phone plans",
            ]
        );
        let states: Vec<(Option<&str>, Option<&str>)> = rows(&document)
            .iter()
            .map(|&row| {
                (
                    document.attribute_by_name(row, "aria-current"),
                    document.attribute_by_name(row, "aria-busy"),
                )
            })
            .collect();
        assert_eq!(
            states,
            vec![
                (Some("page"), None),
                (None, Some("true")),
                (None, None),
                (None, None)
            ]
        );
    }

    #[test]
    fn a_group_with_no_chats_is_not_drawn() {
        let document = built(vec![Chat::new("Old", "").earlier()]);
        assert_eq!(headings(&document), vec![EARLIER]);
    }

    #[test]
    fn a_name_that_looks_like_markup_is_one_text_node() {
        for name in ["<b>bold</b>", "</style><script>x()</script>", "a\0b"] {
            let document = built(vec![Chat::new(name, "<i>site</i>")]);
            let names = elements(&document);
            assert!(!names.iter().any(|n| n == "b" || n == "i"), "{name:?}");
            assert!(!names.iter().any(|n| n == "script"), "{name:?}");
            let texts: Vec<String> = document
                .descendants(document.root())
                .filter_map(|node| match document.kind(node) {
                    Some(NodeKind::Text(data)) if data == name => Some(data.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(texts, vec![name.to_owned()], "{name:?}");
        }
    }
}
