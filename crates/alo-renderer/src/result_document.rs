/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A finished run's result, built into a document as the lines and lists
//! it was written as (queue item 883), on the live card
//! ([`crate::agent_document`]) and in a kept run ([`crate::ran_document`])
//! alike — every element made with `alo-dom`'s operations, every word the
//! model wrote a text node.
//!
//! # What is drawn, and what an agent reads
//!
//! Each block [`crate::result_blocks::blocks`] reads, of the class
//! [`RESULT`]: a paragraph as a `p`, its lines parted by `br`; a list as a
//! `ul`, or for a numbered one an `ol` whose items keep their numbers as
//! written (drawn without a marker of its own), each item an `li`. So an
//! agent reads the hours as a paragraph of lines, or a list of so many
//! items, as the model wrote them. A result of one line is one `p`, as it
//! was before.

use crate::result_blocks::{Block, blocks};
use crate::start_document::{Unbuilt, append, attribute, element};
use alo_dom::{Document, NodeId};

/// The class of each of a result's blocks.
pub const RESULT: &str = "result";

/// Append `said`, a run's result, to `into`, block by block.
///
/// # Errors
///
/// [`Unbuilt`] if `alo-dom` refused an operation, which no result can
/// cause.
pub fn append_result(document: &mut Document, into: NodeId, said: &str) -> Result<(), Unbuilt> {
    for block in blocks(said) {
        let made = match block {
            Block::Lines(lines) => {
                let paragraph = element(document, "p")?;
                for (at, line) in lines.iter().enumerate() {
                    if at > 0 {
                        let ends = element(document, "br")?;
                        append(document, paragraph, ends)?;
                    }
                    let text = document.create_text_node(line);
                    append(document, paragraph, text)?;
                }
                attribute(document, paragraph, "class", RESULT)?;
                paragraph
            }
            Block::List { numbered, items } => {
                let list = element(document, if numbered { "ol" } else { "ul" })?;
                attribute(document, list, "class", RESULT)?;
                for words in items {
                    let item = element(document, "li")?;
                    let text = document.create_text_node(&words);
                    append(document, item, text)?;
                    append(document, list, item)?;
                }
                list
            }
        };
        append(document, into, made)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each child of `node`, as its name and its text.
    fn children(document: &Document, node: NodeId) -> Vec<(String, String)> {
        document
            .children(node)
            .map(|child| {
                (
                    document
                        .element(child)
                        .map(|element| element.name.local.to_string())
                        .unwrap_or_default(),
                    document.text_content(child),
                )
            })
            .collect()
    }

    fn built(said: &str) -> (Document, NodeId) {
        let mut document = Document::new();
        let into =
            element(&mut document, "div").unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        append_result(&mut document, into, said)
            .unwrap_or_else(|unbuilt| panic!("{}", unbuilt.why));
        (document, into)
    }

    #[test]
    fn one_line_is_one_paragraph_as_before() {
        let (document, into) = built("Sent.");
        assert_eq!(
            children(&document, into),
            vec![("p".to_owned(), "Sent.".to_owned())]
        );
    }

    #[test]
    fn lines_are_parted_by_breaks_and_lists_are_lists() {
        let (document, into) = built(
            "Monday\u{2013}Saturday: 8:00 am\u{2013}6:00 pm\nSunday: Closed\n\n- Adults: 10,000\n- Children: 5,000\n1. First\n2. Second",
        );
        let made: Vec<NodeId> = document.children(into).collect();
        assert_eq!(
            children(&document, into),
            vec![
                (
                    "p".to_owned(),
                    "Monday\u{2013}Saturday: 8:00 am\u{2013}6:00 pmSunday: Closed".to_owned()
                ),
                ("ul".to_owned(), "Adults: 10,000Children: 5,000".to_owned()),
                ("ol".to_owned(), "1. First2. Second".to_owned()),
            ]
        );
        let paragraph = children(&document, made[0]);
        assert_eq!(
            paragraph
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            vec!["", "br", ""],
            "a line, a break, a line"
        );
        for list in &made[1..] {
            assert!(
                children(&document, *list)
                    .iter()
                    .all(|(name, _)| name == "li")
            );
            assert_eq!(document.attribute_by_name(*list, "class"), Some(RESULT));
        }
    }
}
