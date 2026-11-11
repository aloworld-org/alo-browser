/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The chats panel hidden, as the design's rail (ADR 0074, queue items 711
//! and 760): a document of its own, drawn by the panel's renderer in place
//! of the panel while a person has hidden it or the window is too narrow.
//!
//! The rail is [`TEMPLATE`] with the chats put in: the mark, *New chat* — the
//! "+" at its top — a short line, then **the chats the browser process sent
//! it**, at most [`crate::panel::RAIL_CHATS`], the most recent first, each a
//! 44-square button holding the chat's initials in a 32 px circle
//! ([`crate::panel::Chat::initials`]), the one on screen white with a
//! shadow; and at its foot *Show the panel*. The chat the pointer is over is
//! a shade lighter, and **its name stands beside it**, past the rail's edge,
//! in a dark pill — over the first screen, which is why the rail is drawn
//! on nothing rather than on white ([`crate::screen::Screen::on_nothing`])
//! and the browser process lays it out wider while it shows a name. A name
//! is a stranger's string, so it enters as a text node, and as an
//! attribute's value where a button is named by it.
//!
//! # What an agent reads
//!
//! ADR 0002: a `navigation` named *Chats* holding *New chat*, a button for
//! each chat named by the chat — with *Done* after a finished one's name,
//! busy while the agent works in it, and the one on screen current — and
//! *Show the panel*. The mark, the initials, the name beside a hovered chat
//! (it repeats the button's), the line under a chat at work or finished —
//! alo's mark and *Working*, or a check mark and *Done* (queue item 985) —
//! and the icons are hidden from the tree.
//!
//! It runs no script: the template has none and nothing here makes one.

use crate::panel::{Chat, Doing, Panel};
use crate::panel_document::{DONE, TICK, Unbuilt};
use alo_dom::{Document, NodeId, parse_document};

/// The rail's markup, compiled in.
pub const TEMPLATE: &str = include_str!("../interface/rail.html");

/// The rail's stylesheet, after the owner's design, compiled in.
pub const SHEET: &str = include_str!("../interface/rail.css");

/// How wide the rail is laid out while it shows the name of the chat the
/// pointer is over, in CSS pixels: its own 64, and the pill beside it —
/// 4 past the rail's edge, at most 240 wide — with room to spare.
pub const NAMED_WIDTH: u32 = 320;

/// The rail's document for `panel`, its chats in the order sent.
///
/// # Errors
///
/// [`Unbuilt`] if the template lacks the rail or the room under its chats,
/// or `alo-dom` refused an operation, which no state can cause.
pub fn build(panel: &Panel) -> Result<Document, Unbuilt> {
    let mut document = parse_document(TEMPLATE);
    // Set in the face chosen (queue items 853, 854).
    panel
        .face
        .set_on(&mut document)
        .map_err(|why| Unbuilt { why })?;
    let of_class = |document: &Document, class: &str| {
        document
            .descendants(document.root())
            .find(|&node| document.attribute_by_name(node, "class") == Some(class))
    };
    let (Some(rail), Some(room)) = (of_class(&document, "rail"), of_class(&document, "room"))
    else {
        return Err(Unbuilt {
            why: "the rail's template has no rail, or no room under its chats".to_owned(),
        });
    };
    for chat in panel.chats.iter().take(crate::panel::RAIL_CHATS) {
        let button = make_chat(&mut document, chat)?;
        document
            .insert_before(rail, button, Some(room))
            .map(|_| ())
            .map_err(|refused| Unbuilt {
                why: format!("a chat could not be put on the rail: {refused}"),
            })?;
    }
    Ok(document)
}

/// One chat on the rail: a button named by the chat, holding its initials,
/// under them alo's mark and *Working* while the agent works in it or a
/// check mark and *Done* once it finished, and — hovered — its name beside
/// it.
fn make_chat(document: &mut Document, chat: &Chat) -> Result<NodeId, Unbuilt> {
    let button = element(document, "button")?;
    attribute(document, button, "class", "rbtn rchat")?;
    attribute(document, button, "type", "button")?;
    let named = match chat.doing {
        Doing::Done => format!("{} {DONE}", chat.label()),
        Doing::Nothing | Doing::Working => chat.label().to_owned(),
    };
    // A stranger's string, entering as an attribute's value.
    attribute(document, button, "aria-label", &named)?;
    if chat.current {
        attribute(document, button, "aria-current", "page")?;
    }
    if chat.doing == Doing::Working {
        attribute(document, button, "aria-busy", "true")?;
    }
    let initials = element(document, "span")?;
    attribute(document, initials, "class", "initials")?;
    attribute(document, initials, "aria-hidden", "true")?;
    let letters = document.create_text_node(&chat.initials());
    append(document, initials, letters)?;
    append(document, button, initials)?;
    match chat.doing {
        Doing::Nothing => {}
        Doing::Working => {
            // The initials alone on the first line and, under the circle,
            // alo's mark and the word with the acting colour, never the
            // colour alone (queue items 974, 985). The button is named by
            // the chat and `aria-busy`, so both are hidden from an agent, as
            // the initials are.
            let said = element(document, "span")?;
            attribute(document, said, "class", "working")?;
            attribute(document, said, "aria-hidden", "true")?;
            let mark = element(document, "img")?;
            attribute(document, mark, "class", "acting-mark")?;
            attribute(document, mark, "src", crate::acting::SRC)?;
            attribute(document, mark, "alt", "")?;
            attribute(document, mark, "aria-hidden", "true")?;
            append(document, said, mark)?;
            let words = document.create_text_node(crate::acting::WORKING);
            append(document, said, words)?;
            append(document, button, said)?;
        }
        Doing::Done => {
            // A check mark and *Done* under the circle, in the ink (queue
            // item 985), hidden from an agent: the button's name says it.
            attribute(document, button, "data-done", "true")?;
            let said = element(document, "span")?;
            attribute(document, said, "class", "done")?;
            attribute(document, said, "aria-hidden", "true")?;
            let tick = element(document, "span")?;
            attribute(document, tick, "class", "tick")?;
            let mark = document.create_text_node(TICK);
            append(document, tick, mark)?;
            append(document, said, tick)?;
            let words = document.create_text_node(DONE);
            append(document, said, words)?;
            append(document, button, said)?;
        }
    }
    if chat.hovered {
        attribute(document, button, "data-hovered", "true")?;
        let name = element(document, "span")?;
        attribute(document, name, "class", "named")?;
        attribute(document, name, "aria-hidden", "true")?;
        // A stranger's string, entering as a text node's data.
        let words = document.create_text_node(chat.label());
        append(document, name, words)?;
        append(document, button, name)?;
    }
    Ok(button)
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
            why: format!("a node could not be put on the rail: {refused}"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buttons(document: &Document) -> Vec<Option<&str>> {
        document
            .descendants(document.root())
            .filter(|&node| {
                document
                    .element(node)
                    .is_some_and(|element| element.name.local.as_ref() == "button")
            })
            .map(|node| document.attribute_by_name(node, "aria-label"))
            .collect()
    }

    fn rail(chats: Vec<Chat>) -> Panel {
        Panel {
            chats,
            hidden: true,
            ..Panel::default()
        }
    }

    #[test]
    fn the_rail_is_the_mark_new_chat_and_show_the_panel_and_no_script() {
        let document = build(&rail(Vec::new())).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        assert_eq!(
            buttons(&document),
            vec![Some("New chat"), Some("Show the panel")]
        );
        let scripts = document
            .descendants(document.root())
            .filter_map(|node| document.element(node))
            .filter(|element| element.name.local.as_ref() == "script")
            .count();
        assert_eq!(scripts, 0);
        assert!(SHEET.contains(".rail"));
    }

    #[test]
    fn the_rail_holds_at_most_five_chats_between_new_chat_and_show_the_panel() {
        let chats = [
            "Weekend in Lisbon",
            "rust ownership",
            "",
            "Invoice for March",
            "Compare phone plans",
            "Birthday present ideas",
        ]
        .into_iter()
        .map(|name| Chat::new(name, ""))
        .collect();
        let document = build(&rail(chats)).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        assert_eq!(
            buttons(&document),
            vec![
                Some("New chat"),
                Some("Weekend in Lisbon"),
                Some("rust ownership"),
                Some("New chat"),
                Some("Invoice for March"),
                Some("Compare phone plans"),
                Some("Show the panel"),
            ]
        );
    }

    #[test]
    fn a_hovered_chat_has_its_name_beside_it_and_a_name_is_only_ever_text() {
        let hostile = "<img src=x onerror=alert(1)> & \"quoted\"";
        let chats = vec![
            Chat::new(hostile, "").current().hovered(),
            Chat::new("Invoice for March", "").doing(Doing::Done),
            Chat::new("Weekend in Lisbon", "").doing(Doing::Working),
        ];
        let document = build(&rail(chats)).unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        let of_class = |class: &str| -> Vec<NodeId> {
            document
                .descendants(document.root())
                .filter(|&node| document.attribute_by_name(node, "class") == Some(class))
                .collect()
        };
        let named = of_class("named");
        let [named] = named.as_slice() else {
            panic!("one name beside the hovered chat: {named:?}");
        };
        assert_eq!(document.text_content(*named), hostile);
        // The only pictures are alo's mark beside the chat at work.
        assert!(
            !document
                .descendants(document.root())
                .filter(|&node| document.attribute_by_name(node, "class") != Some("acting-mark"))
                .filter_map(|node| document.element(node))
                .any(|element| element.name.local.as_ref() == "img"),
            "no element came of a name"
        );
        assert_eq!(
            buttons(&document).get(2).copied().flatten(),
            Some("Invoice for March Done")
        );
        let current: Vec<NodeId> = of_class("rbtn rchat")
            .into_iter()
            .filter(|&node| document.attribute_by_name(node, "aria-current") == Some("page"))
            .collect();
        assert_eq!(current.len(), 1);
        // The working chat: alo's mark and the word, never the colour alone.
        let marks = of_class("acting-mark");
        assert_eq!(marks.len(), 1, "alo's mark on the working chat");
        assert_eq!(
            marks
                .first()
                .and_then(|&mark| document.attribute_by_name(mark, "src")),
            Some(crate::acting::SRC)
        );
        let said = of_class("working");
        assert_eq!(
            said.iter()
                .map(|&node| document.text_content(node))
                .collect::<Vec<_>>(),
            vec![crate::acting::WORKING]
        );
        // alo's mark on the word's line, under the circle (queue item 985).
        assert_eq!(
            marks
                .first()
                .and_then(|&mark| document.parent(mark))
                .and_then(|line| document.attribute_by_name(line, "class")),
            Some("working")
        );
        // The finished chat: a check mark and Done under its circle.
        let done = of_class("done");
        assert_eq!(
            done.iter()
                .map(|&node| document.text_content(node))
                .collect::<Vec<_>>(),
            vec![format!("{TICK}{DONE}")]
        );
        let busy = of_class("rbtn rchat")
            .into_iter()
            .filter(|&node| document.attribute_by_name(node, "aria-busy") == Some("true"))
            .count();
        assert_eq!(busy, 1);
    }
}
