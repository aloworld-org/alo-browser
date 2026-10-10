/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The number a list item is drawn with: HTML's **ordinal value** (§ 4.4.8,
//! ADR 0044 § 3).
//!
//! Not CSS counters, which no page here writes (item 407). HTML's algorithm
//! numbers every list a page here has, and it is the one Chromium follows.
//!
//! # The algorithm
//!
//! A **list item** is an element whose computed `display` is one that is
//! rendered. Its **list owner** is its nearest `ol`, `ul` or `menu`
//! ancestor, or its parent if it has none, moved up past any ancestor that
//! makes no box of its own (`display: contents`). An owner numbers the items
//! it owns in tree order: from its **starting value** — an `ol`'s `start`,
//! or for a `reversed` one with none the number of items it owns, or 1 —
//! counting up, or down when it is an `ol` that is `reversed`. An `li`'s
//! `value` sets the number it and the items after it count from.
//!
//! # Every number is a stranger's
//!
//! `start` and `value` are read by HTML's rules for parsing integers, and a
//! number that does not fit an `i32` is read as **absent**, as a browser
//! does. Counting saturates rather than overflowing, so a `start` of
//! `2147483647` and a reversed list of a hundred thousand items are each
//! answered. Nothing here indexes: a hundred thousand digits are refused one
//! at a time.

use crate::computed::ComputedStyle;
use crate::display::Display;
use alo_dom::{Document, NodeId};
use std::collections::BTreeMap;

/// The ordinal value of every list item in `document`, by element.
///
/// `style_of` answers each element's computed style; an element it has none
/// for makes no box.
pub(crate) fn ordinal_values<'s>(
    document: &Document,
    style_of: impl Fn(NodeId) -> Option<&'s ComputedStyle>,
) -> BTreeMap<NodeId, i32> {
    let display_of = |id: NodeId| display(style_of(id));
    // Every owner's items, in tree order: the order `descendants` visits.
    let mut owned: BTreeMap<NodeId, Vec<NodeId>> = BTreeMap::new();
    for id in document.descendants(document.root()) {
        if !display_of(id).is_list_item() || !is_rendered(document, id, &display_of) {
            continue;
        }
        if let Some(owner) = list_owner(document, id, &display_of) {
            owned.entry(owner).or_default().push(id);
        }
    }
    let mut values = BTreeMap::new();
    for (owner, items) in owned {
        let element = document.element(owner);
        let is_ol = element.is_some_and(|element| element.name.is_html("ol"));
        let reversed = is_ol && element.is_some_and(|element| element.attr("reversed").is_some());
        let start = element
            .filter(|_| is_ol)
            .and_then(|element| element.attr("start"))
            .and_then(parse_integer);
        let mut numbering = match start {
            Some(start) => start,
            None if reversed => i32::try_from(items.len()).unwrap_or(i32::MAX),
            None => 1,
        };
        for item in items {
            if let Some(value) = document
                .element(item)
                .filter(|element| element.name.is_html("li"))
                .and_then(|element| element.attr("value"))
                .and_then(parse_integer)
            {
                numbering = value;
            }
            values.insert(item, numbering);
            numbering = if reversed {
                numbering.saturating_sub(1)
            } else {
                numbering.saturating_add(1)
            };
        }
    }
    values
}

/// An element's `display`, read the way the box tree reads it: a value
/// this engine does not implement is the initial one.
fn display(style: Option<&ComputedStyle>) -> Display {
    style
        .and_then(|style| style.get("display"))
        .and_then(Display::parse)
        .unwrap_or_default()
}

/// Whether neither `id` nor anything above it is `display: none`.
fn is_rendered(document: &Document, id: NodeId, display_of: &impl Fn(NodeId) -> Display) -> bool {
    let mut at = Some(id);
    while let Some(here) = at {
        if document.element(here).is_some() && display_of(here) == Display::None {
            return false;
        }
        at = document.parent(here);
    }
    true
}

/// The element that numbers `item`: its nearest `ol`, `ul` or `menu`
/// ancestor, or its parent, moved up past what makes no box.
fn list_owner(
    document: &Document,
    item: NodeId,
    display_of: &impl Fn(NodeId) -> Display,
) -> Option<NodeId> {
    let parent = document.parent(item)?;
    let is_a_list = |id: NodeId| {
        document.element(id).is_some_and(|element| {
            ["ol", "ul", "menu"]
                .iter()
                .any(|name| element.name.is_html(name))
        })
    };
    let mut nearest = Some(parent);
    while let Some(here) = nearest
        && !is_a_list(here)
    {
        nearest = document.parent(here);
    }
    let mut ancestor = nearest.unwrap_or(parent);
    while document.element(ancestor).is_some() && display_of(ancestor) == Display::Contents {
        ancestor = document.parent(ancestor)?;
    }
    Some(ancestor)
}

/// HTML's rules for parsing integers, with a number past an `i32` read as
/// absent: leading whitespace, a sign, then at least one digit, and
/// whatever follows the digits ignored.
pub(crate) fn parse_integer(text: &str) -> Option<i32> {
    let text = text.trim_start_matches(|letter: char| letter.is_ascii_whitespace());
    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, text.get(1..)?),
        Some(b'+') => (false, text.get(1..)?),
        _ => (false, text),
    };
    let mut magnitude: i64 = 0;
    let mut any = false;
    for byte in digits.bytes() {
        if !byte.is_ascii_digit() {
            break;
        }
        any = true;
        magnitude = magnitude
            .checked_mul(10)?
            .checked_add(i64::from(byte - b'0'))?;
        // Past every `i32` already: refuse now rather than reading on.
        if magnitude > i64::from(i32::MAX) + 1 {
            return None;
        }
    }
    if !any {
        return None;
    }
    let signed = if negative { -magnitude } else { magnitude };
    i32::try_from(signed).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cascade::SourcedSheet;
    use crate::origin::Origin;
    use crate::user_agent::USER_AGENT_STYLE_SHEET;
    use alo_css::{MediaContext, parse_stylesheet};

    /// Every list item's text and number, in tree order.
    fn numbered(html: &str, author: &str) -> Vec<(String, i32)> {
        let document = alo_dom::parse_document(html);
        let agent = parse_stylesheet(USER_AGENT_STYLE_SHEET);
        let author = parse_stylesheet(author);
        let sheets = [
            SourcedSheet::new(Origin::UserAgent, &agent),
            SourcedSheet::new(Origin::Author, &author),
        ];
        let styles = crate::resolve(&document, &sheets, &MediaContext::default());
        let values = ordinal_values(&document, |id| styles.get(id));
        document
            .descendants(document.root())
            .filter_map(|id| {
                let value = values.get(&id)?;
                let text = document
                    .children(id)
                    .filter_map(|child| document.get(child)?.text().map(str::to_owned))
                    .collect::<String>();
                Some((text.trim().to_owned(), *value))
            })
            .collect()
    }

    fn pairs(expected: &[(&str, i32)]) -> Vec<(String, i32)> {
        expected
            .iter()
            .map(|(text, value)| ((*text).to_owned(), *value))
            .collect()
    }

    #[test]
    fn a_list_counts_from_one_and_start_moves_it() {
        assert_eq!(
            numbered("<ol><li>a<li>b<li>c</ol>", ""),
            pairs(&[("a", 1), ("b", 2), ("c", 3)])
        );
        assert_eq!(
            numbered("<ol start=3><li>Draft<li>Review</ol>", ""),
            pairs(&[("Draft", 3), ("Review", 4)])
        );
        // A `ul` has no `start`.
        assert_eq!(
            numbered("<ul start=5><li>a<li>b</ul>", ""),
            pairs(&[("a", 1), ("b", 2)])
        );
    }

    #[test]
    fn reversed_counts_down_from_how_many_there_are() {
        assert_eq!(
            numbered("<ol reversed><li>a<li>b<li>c</ol>", ""),
            pairs(&[("a", 3), ("b", 2), ("c", 1)])
        );
        assert_eq!(
            numbered("<ol reversed start=10><li>a<li>b</ol>", ""),
            pairs(&[("a", 10), ("b", 9)])
        );
    }

    #[test]
    fn value_sets_this_number_and_the_count_after_it() {
        assert_eq!(
            numbered("<ol><li>a<li value=7>b<li>c<li value=-2>d<li>e</ol>", ""),
            pairs(&[("a", 1), ("b", 7), ("c", 8), ("d", -2), ("e", -1)])
        );
        assert_eq!(
            numbered("<ol reversed><li value=5>a<li>b</ol>", ""),
            pairs(&[("a", 5), ("b", 4)])
        );
    }

    #[test]
    fn a_nested_list_counts_on_its_own_and_its_owner_is_the_nearest_list() {
        assert_eq!(
            numbered("<ol><li>a<ol><li>x<li>y</ol><li>b</ol>", ""),
            pairs(&[("a", 1), ("x", 1), ("y", 2), ("b", 2)])
        );
        // An item in a `<div>` in a list is still the list's.
        assert_eq!(
            numbered("<ol><li>a<div><li>b</div></ol>", ""),
            pairs(&[("a", 1), ("b", 2)])
        );
    }

    #[test]
    fn what_is_not_rendered_is_not_counted_and_contents_is_passed_through() {
        assert_eq!(
            numbered("<ol><li>a<li style='display:none'>b<li>c</ol>", ""),
            pairs(&[("a", 1), ("c", 2)])
        );
        assert_eq!(
            numbered(
                "<div class=own><span class=c><p class=i>a</p><p class=i>b</p></span></div>",
                ".i { display: list-item } .c { display: contents }"
            ),
            pairs(&[("a", 1), ("b", 2)])
        );
        // An item made ordinary by its sheet is no item.
        assert_eq!(
            numbered("<ol><li class=n>a<li>b</ol>", ".n { display: block }"),
            pairs(&[("b", 1)])
        );
    }

    #[test]
    fn integers_are_read_by_htmls_rules() {
        for (text, read) in [
            ("3", Some(3)),
            ("  +3", Some(3)),
            ("-3", Some(-3)),
            ("3.9", Some(3)),
            ("12abc", Some(12)),
            ("2147483647", Some(i32::MAX)),
            ("-2147483648", Some(i32::MIN)),
            ("2147483648", None),
            ("-2147483649", None),
            ("", None),
            ("-", None),
            ("+", None),
            ("abc", None),
            ("\u{a0}3", None),
        ] {
            assert_eq!(parse_integer(text), read, "{text:?}");
        }
    }

    #[test]
    fn hostile_numbers_are_read_or_refused_without_a_panic() {
        let long = "9".repeat(100_000);
        assert_eq!(parse_integer(&long), None);
        assert_eq!(parse_integer(&format!("-{long}")), None);
        assert_eq!(parse_integer(&format!("{}1", "0".repeat(100_000))), Some(1));
        // The top of an `i32`, counted past, stays there.
        assert_eq!(
            numbered("<ol start=2147483647><li>a<li>b</ol>", ""),
            pairs(&[("a", i32::MAX), ("b", i32::MAX)])
        );
        assert_eq!(
            numbered("<ol reversed start=-2147483648><li>a<li>b</ol>", ""),
            pairs(&[("a", i32::MIN), ("b", i32::MIN)])
        );
        // A `start` past an `i32` is absent, so the list counts from one.
        assert_eq!(
            numbered("<ol start=-2147483649><li>a</ol>", ""),
            pairs(&[("a", 1)])
        );
        let many = format!("<ol reversed>{}</ol>", "<li>x".repeat(100_000));
        let values = numbered(&many, "");
        assert_eq!(values.len(), 100_000);
        assert_eq!(values.first().map(|(_, value)| *value), Some(100_000));
        assert_eq!(values.last().map(|(_, value)| *value), Some(1));
    }
}
