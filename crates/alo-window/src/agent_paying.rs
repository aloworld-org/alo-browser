/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a press that pays pays, read off the page's tree by the browser
//! process (queue item 826, `docs/design/shell.md`, *Paying*, approved by
//! the owner 2026-11-06; ADR 0099): shown to the person with the question
//! the person's gate asks ([`crate::agent_gate`]), never asked of the model
//! and never sent to it.
//!
//! **No script, and named nodes only**: the tree an agent reads (ADR 0002),
//! its roles and names and the words inside them — the same tree the gate
//! judges the press on, read again just before.
//!
//! # The amount
//!
//! The first that holds a price — a currency's sign or its code beside a
//! number, *UGX 45,000*, *$42.00*, *42,00 €* ([`price_in`]):
//!
//! 1. the words of what is pressed — *Pay UGX 45,000*;
//! 2. else the last line before it on the page that says it is a total —
//!    *Total*, *Order total*, *Amount due*, *To pay*, *You pay*, *Balance
//!    due* ([`says_total`]; *Subtotal* is not one) — or, with none before
//!    it, the first after it.
//!
//! A line saying it is a total with no price on it — *Total* in one cell,
//! the price in the next, as a table lays a summary out — takes the price
//! of the line after it.
//!
//! Otherwise there is no amount, and the question says so plainly
//! ([`alo_renderer::agent_paying::NO_AMOUNT`]). A price nearest the button
//! that is not a total is never taken for one: it is as likely an item's.
//!
//! # The merchant
//!
//! The page's own name for itself — its title, the tree's root's name —
//! and the site's host, which the browser knows and the page cannot say
//! otherwise: *Kampala Coffee — Checkout (shop.example)*; the host alone
//! when the page names itself nothing.
//!
//! # The summary
//!
//! The page's own summary of what is paid for: the first node named as one
//! — whatever its role but a field, a button, a link or a heading, whose
//! name says *summary*, *your order*, *order*, *basket*, *cart* or *bag* —
//! or, under a heading saying so, what follows the heading until the next.
//! A line each: a row, a list item, a paragraph, or words standing
//! together; the words of a button or a field inside never.
//!
//! **Chrome's tables**: a table Chrome judges is for layout — one with no
//! headers, as most checkouts' summaries are — is in its tree as
//! `LayoutTable`, its rows `LayoutTableRow` and its cells `LayoutTableCell`
//! (found by hand on 7 November 2026, `handrun36/14`: the summary and its
//! total read as nothing). A `LayoutTableRow` is a line, as a `row` is. At most [`MOST_SUMMARY`], the last saying how many
//! more there are. None found is said plainly
//! ([`alo_renderer::agent_paying::NO_SUMMARY`]).
//!
//! **Never a secret**: nothing inside a field whose name or hint says it
//! holds a password or a card's details ([`crate::agent_page::is_secret`]),
//! and no line that is only a password's dots.

use alo_box::{BoxId, KnownRole, Role};
use alo_renderer::agent_paying::{MOST_SUMMARY, PAYING_LINE, Paying};
use alo_renderer::{Snapshot, SnapshotNode};

/// The signs a price is written with.
const SIGNS: [char; 16] = [
    '$', '\u{20ac}', '\u{a3}', '\u{a5}', '\u{20b9}', '\u{20a6}', '\u{20b5}', '\u{20a9}',
    '\u{20bd}', '\u{20ba}', '\u{20aa}', '\u{20b1}', '\u{e3f}', '\u{20ab}', '\u{20b4}', '\u{20a1}',
];

/// The currencies' codes a price is written with.
const CODES: [&str; 32] = [
    "USD", "EUR", "GBP", "UGX", "KES", "TZS", "RWF", "NGN", "GHS", "ZAR", "JPY", "CNY", "INR",
    "CAD", "AUD", "CHF", "SEK", "NOK", "DKK", "BRL", "MXN", "AED", "SAR", "EGP", "ETB", "XOF",
    "XAF", "MAD", "BIF", "CDF", "SSP", "ZMW",
];

/// The words that say a line is a total, each a word of its own.
const TOTAL_WORDS: [&str; 2] = ["total", "totaal"];

/// The phrases that say it.
const TOTAL_PHRASES: [&str; 5] = [
    "amount due",
    "to pay",
    "you pay",
    "balance due",
    "amount to pay",
];

/// The words that say a node holds the summary of what is paid for.
const SUMMARY_WORDS: [&str; 6] = ["summary", "order", "basket", "cart", "bag", "items"];

/// Whether `node` is a table's row: a `row`, or Chrome's `LayoutTableRow`.
fn is_a_row(node: &SnapshotNode) -> bool {
    node.role == Role::Known(KnownRole::Row)
        || matches!(&node.role, Role::Declared(declared) if declared.as_ref() == "LayoutTableRow")
}

/// What pressing `pressed` on `tree`, the page at `host`, pays.
pub fn of(tree: &Snapshot, pressed: BoxId, host: &str) -> Paying {
    let lines = lines(tree);
    let pressed_at = lines
        .iter()
        .position(|line| line.within.contains(&pressed))
        .or_else(|| place_of(tree, pressed, &lines));
    let amount = button_words(tree, pressed)
        .as_deref()
        .and_then(price_in)
        .or_else(|| total(&lines, pressed_at));
    Paying {
        amount,
        merchant: merchant(tree, host),
        lines: summary(tree),
    }
}

/// A line of the page: its words, and the nodes it was read from and
/// inside.
#[derive(Debug, Clone)]
struct Line {
    words: String,
    within: Vec<BoxId>,
}

/// The words of `pressed`: its name, or the words inside it.
fn button_words(tree: &Snapshot, pressed: BoxId) -> Option<String> {
    let node = find(tree.root.as_ref()?, pressed)?;
    node.name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| Some(words_inside(node)))
}

/// The total the page says, as a price: the last line saying it is one
/// before `before`, else the first after.
fn total(lines: &[Line], before: Option<usize>) -> Option<String> {
    let totals: Vec<(usize, String)> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| says_total(&line.words))
        .filter_map(|(at, line)| {
            // Its own price, or, with none, the price of the line after it.
            price_in(&line.words)
                .or_else(|| {
                    lines
                        .get(at + 1)
                        .filter(|next| !says_total(&next.words))
                        .and_then(|next| price_in(&next.words))
                })
                .map(|price| (at, price))
        })
        .collect();
    let before = before.unwrap_or(usize::MAX);
    totals
        .iter()
        .rev()
        .find(|(at, _)| *at < before)
        .or_else(|| totals.iter().find(|(at, _)| *at >= before))
        .map(|(_, price)| price.clone())
}

/// Whether `words` say they are a total.
pub fn says_total(words: &str) -> bool {
    let lower = words.to_lowercase();
    lower
        .split(|each: char| !each.is_alphanumeric())
        .any(|word| TOTAL_WORDS.contains(&word))
        || TOTAL_PHRASES.iter().any(|phrase| lower.contains(phrase))
}

/// The first price in `words`, as written: a currency's sign or code beside
/// a number.
pub fn price_in(words: &str) -> Option<String> {
    let letters: Vec<(usize, char)> = words.char_indices().collect();
    let end_of = |at: usize| letters.get(at).map_or(words.len(), |&(byte, _)| byte);
    for (at, &(byte, letter)) in letters.iter().enumerate() {
        // A sign, or a code standing as a word, then a number after it.
        let sign_len = if SIGNS.contains(&letter) {
            Some(1)
        } else {
            code_at(&letters, at).map(|_| 3)
        };
        if let Some(sign_len) = sign_len {
            let mut next = at + sign_len;
            while letters
                .get(next)
                .is_some_and(|&(_, each)| each == ' ' || each == '\u{a0}')
            {
                next += 1;
            }
            if let Some(number_end) = number_from(&letters, next) {
                return Some(words.get(byte..end_of(number_end))?.trim().to_owned());
            }
        }
        // A number, then a sign or a code after it: *42,00 €*, *45,000 UGX*.
        let starts_a_number = letter.is_ascii_digit()
            && (at == 0
                || letters.get(at - 1).is_some_and(|&(_, before)| {
                    !before.is_alphanumeric() && before != '.' && before != ','
                }));
        if starts_a_number && let Some(number_end) = number_from(&letters, at) {
            let mut next = number_end;
            while letters
                .get(next)
                .is_some_and(|&(_, each)| each == ' ' || each == '\u{a0}')
            {
                next += 1;
            }
            let after = if letters
                .get(next)
                .is_some_and(|&(_, each)| SIGNS.contains(&each))
            {
                Some(next + 1)
            } else {
                code_at(&letters, next).map(|_| next + 3)
            };
            if let Some(after) = after {
                return Some(words.get(byte..end_of(after))?.trim().to_owned());
            }
        }
    }
    None
}

/// Whether a currency's code stands at `at` of `letters` as a word of its
/// own.
fn code_at(letters: &[(usize, char)], at: usize) -> Option<&'static str> {
    let code: String = letters
        .get(at..at + 3)?
        .iter()
        .map(|&(_, each)| each)
        .collect();
    let found = CODES.iter().find(|&&known| known == code)?;
    let alone = |near: Option<&(usize, char)>| near.is_none_or(|&(_, each)| !each.is_alphabetic());
    (at == 0 || alone(letters.get(at - 1))).then_some(())?;
    alone(letters.get(at + 3)).then_some(found)
}

/// Where the number starting at `at` of `letters` ends, if one does: digits,
/// and the marks and spaces that group them, ending in a digit.
fn number_from(letters: &[(usize, char)], at: usize) -> Option<usize> {
    if !letters
        .get(at)
        .is_some_and(|&(_, each)| each.is_ascii_digit())
    {
        return None;
    }
    let mut end = at;
    let mut last_digit = at;
    let digit = |at: usize| {
        letters
            .get(at)
            .is_some_and(|&(_, each)| each.is_ascii_digit())
    };
    while let Some(&(_, each)) = letters.get(end) {
        if each.is_ascii_digit() {
            last_digit = end;
        } else if each == ' ' {
            // A plain space groups thousands only before three digits:
            // *45 000*, never *2 45,000*.
            if !(digit(end + 1) && digit(end + 2) && digit(end + 3) && !digit(end + 4)) {
                break;
            }
        } else if !matches!(each, ',' | '.' | '\u{a0}' | '\u{202f}' | '\'') || !digit(end + 1) {
            break;
        }
        end += 1;
    }
    Some(last_digit + 1)
}

/// Who is paid: the page's title and the site's host.
fn merchant(tree: &Snapshot, host: &str) -> String {
    let title = tree
        .root
        .as_ref()
        .and_then(|root| root.name.as_deref())
        .map(str::trim)
        .filter(|title| !title.is_empty() && *title != host);
    let merchant = match (title, host.is_empty()) {
        (Some(title), false) => format!("{title} ({host})"),
        (Some(title), true) => title.to_owned(),
        (None, _) => host.to_owned(),
    };
    crate::agent_protocol::kept(&merchant, PAYING_LINE)
}

/// The page's own summary of what is paid for, a line each.
fn summary(tree: &Snapshot) -> Vec<String> {
    let Some(root) = &tree.root else {
        return Vec::new();
    };
    let mut found = Vec::new();
    // The first node named as a summary, or a heading saying so and what
    // follows it until the next heading.
    let mut waiting: Vec<&SnapshotNode> = vec![root];
    while let Some(node) = waiting.pop() {
        let named = node.name.as_deref().is_some_and(says_summary);
        if named
            && !node.children.is_empty()
            && !is_unread(node)
            && !matches!(
                node.role,
                Role::Known(KnownRole::Document | KnownRole::Heading | KnownRole::Link)
            )
        {
            lines_in(node, &mut found);
            break;
        }
        if let Some(heading) = node.children.iter().position(|child| {
            child.role == Role::Known(KnownRole::Heading)
                && words_named(child).as_deref().is_some_and(says_summary)
        }) {
            for after in node.children.iter().skip(heading + 1) {
                if after.role == Role::Known(KnownRole::Heading) {
                    break;
                }
                lines_in_one(after, &mut found);
            }
            break;
        }
        waiting.extend(node.children.iter().rev());
    }
    let mut lines: Vec<String> = found
        .into_iter()
        .map(|line| line.words)
        .filter(|words| !words.is_empty() && !crate::agent_page::is_dots(words))
        .map(|words| crate::agent_protocol::kept(&words, PAYING_LINE))
        .collect();
    if lines.len() > MOST_SUMMARY {
        let more = lines.len() - (MOST_SUMMARY - 1);
        lines.truncate(MOST_SUMMARY - 1);
        lines.push(format!("and {more} more lines on the page"));
    }
    lines
}

/// Whether `name` says it is the summary of what is paid for.
fn says_summary(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower
        .split(|each: char| !each.is_alphanumeric())
        .any(|word| SUMMARY_WORDS.contains(&word))
}

/// A heading's words: its name, or the words inside it.
fn words_named(node: &SnapshotNode) -> Option<String> {
    node.name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| Some(words_inside(node)))
}

/// Every line of the page, in order.
fn lines(tree: &Snapshot) -> Vec<Line> {
    let mut found = Vec::new();
    if let Some(root) = &tree.root {
        lines_in(root, &mut found);
    }
    found
}

/// The lines inside `node`, in order, added to `found`.
fn lines_in(node: &SnapshotNode, found: &mut Vec<Line>) {
    let mut words: Vec<&SnapshotNode> = Vec::new();
    for child in &node.children {
        if child.role == Role::Known(KnownRole::Text) {
            words.push(child);
            continue;
        }
        flush(&mut words, found);
        lines_in_one(child, found);
    }
    flush(&mut words, found);
}

/// `node`'s lines: one, when it is a line; else those inside it.
fn lines_in_one(node: &SnapshotNode, found: &mut Vec<Line>) {
    if is_unread(node) {
        // A field's or a button's words are not the page's summary, and a
        // secret is never read; but a button is a place on the page.
        found.push(Line {
            words: String::new(),
            within: ids_in(node),
        });
        return;
    }
    if node.role == Role::Known(KnownRole::Text) {
        found.push(Line {
            words: node.name.clone().unwrap_or_default().trim().to_owned(),
            within: vec![node.id],
        });
        return;
    }
    if is_a_row(node)
        || matches!(
            node.role,
            Role::Known(
                KnownRole::ListItem | KnownRole::Paragraph | KnownRole::Heading | KnownRole::Link
            )
        )
    {
        found.push(Line {
            words: words_inside(node),
            within: ids_in(node),
        });
        return;
    }
    lines_in(node, found);
}

/// Words standing together, as one line.
fn flush(words: &mut Vec<&SnapshotNode>, found: &mut Vec<Line>) {
    if words.is_empty() {
        return;
    }
    let said: Vec<&str> = words
        .iter()
        .filter_map(|word| word.name.as_deref())
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .collect();
    found.push(Line {
        words: said.join(" "),
        within: words.iter().map(|word| word.id).collect(),
    });
    words.clear();
}

/// Whether `node`'s words are never read into a line: a field, a button, a
/// secret.
fn is_unread(node: &SnapshotNode) -> bool {
    node.states.takes_text
        || crate::agent_page::is_secret(node)
        || matches!(
            node.role,
            Role::Known(
                KnownRole::Button
                    | KnownRole::TextBox
                    | KnownRole::SearchBox
                    | KnownRole::ComboBox
                    | KnownRole::ListBox
                    | KnownRole::SpinButton
            )
        )
}

/// The words inside `node`, those of fields, buttons and secrets left out,
/// one space between.
fn words_inside(node: &SnapshotNode) -> String {
    let mut said = Vec::new();
    let mut waiting = vec![node];
    while let Some(next) = waiting.pop() {
        if !std::ptr::eq(next, node) && is_unread(next) {
            continue;
        }
        if next.role == Role::Known(KnownRole::Text)
            && let Some(name) = next.name.as_deref()
        {
            let name = name.trim();
            if !name.is_empty() {
                said.push(name.to_owned());
            }
        }
        waiting.extend(next.children.iter().rev());
    }
    if said.is_empty() {
        return node.name.clone().unwrap_or_default().trim().to_owned();
    }
    said.join(" ")
}

/// Every node's number in `node`, it among them.
fn ids_in(node: &SnapshotNode) -> Vec<BoxId> {
    let mut ids = Vec::new();
    let mut waiting = vec![node];
    while let Some(next) = waiting.pop() {
        ids.push(next.id);
        waiting.extend(next.children.iter());
    }
    ids
}

/// The node numbered `id` under `node`.
fn find(node: &SnapshotNode, id: BoxId) -> Option<&SnapshotNode> {
    let mut waiting = vec![node];
    while let Some(next) = waiting.pop() {
        if next.id == id {
            return Some(next);
        }
        waiting.extend(next.children.iter());
    }
    None
}

/// Where `pressed` stands among `lines` when no line holds it: the place
/// of the first line after it in the tree's order.
fn place_of(tree: &Snapshot, pressed: BoxId, lines: &[Line]) -> Option<usize> {
    let order: Vec<BoxId> = tree.nodes().iter().map(|node| node.id).collect();
    let at = order.iter().position(|&id| id == pressed)?;
    lines.iter().position(|line| {
        line.within
            .first()
            .and_then(|first| order.iter().position(|id| id == first))
            .is_some_and(|place| place > at)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_box::States;
    use alo_layout::Rect;

    fn node(id: usize, role: KnownRole, name: &str, children: Vec<SnapshotNode>) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(id),
            role: Role::Known(role),
            name: (!name.is_empty()).then(|| name.to_owned()),
            states: States::default(),
            placeholder: None,
            marker: None,
            rect: Rect::default(),
            rects: Vec::new(),
            offscreen: false,
            scrolls: false,
            children,
        }
    }

    fn text(id: usize, words: &str) -> SnapshotNode {
        node(id, KnownRole::Text, words, Vec::new())
    }

    fn row(id: usize, cells: &[&str]) -> SnapshotNode {
        node(
            id,
            KnownRole::Row,
            "",
            cells
                .iter()
                .enumerate()
                .map(|(at, cell)| {
                    node(
                        id + 1 + at * 2,
                        KnownRole::Cell,
                        "",
                        vec![text(id + 2 + at * 2, cell)],
                    )
                })
                .collect(),
        )
    }

    fn field(id: usize, name: &str, holds: &str) -> SnapshotNode {
        let mut field = node(id, KnownRole::TextBox, name, vec![text(id + 1, holds)]);
        field.states.takes_text = true;
        field
    }

    /// A shop's checkout: a table *Order summary* of two rows and a total
    /// row, a card field, *Total* again by the button, and *Pay now*.
    fn checkout() -> Snapshot {
        Snapshot {
            root: Some(node(
                1,
                KnownRole::Document,
                "Kampala Coffee \u{2014} Checkout",
                vec![
                    node(2, KnownRole::Heading, "Checkout", vec![text(3, "Checkout")]),
                    node(
                        10,
                        KnownRole::Table,
                        "Order summary",
                        vec![
                            row(20, &["Flat white \u{d7} 2", "UGX 16,000"]),
                            row(30, &["Beans, 250 g", "UGX 29,000"]),
                            row(40, &["Total", "UGX 45,000"]),
                        ],
                    ),
                    node(
                        50,
                        KnownRole::Form,
                        "",
                        vec![
                            field(51, "Card number", "4242 4242 4242 4242"),
                            text(53, "Delivery"),
                            text(54, "UGX 2,000"),
                            node(60, KnownRole::Button, "Pay now", Vec::new()),
                        ],
                    ),
                ],
            )),
        }
    }

    #[test]
    fn a_checkouts_amount_merchant_and_summary_are_read_off_its_tree() {
        let paying = of(&checkout(), BoxId::from_wire(60), "shop.example");
        assert_eq!(paying.amount.as_deref(), Some("UGX 45,000"));
        assert_eq!(
            paying.merchant,
            "Kampala Coffee \u{2014} Checkout (shop.example)"
        );
        assert_eq!(
            paying.lines,
            [
                "Flat white \u{d7} 2 UGX 16,000",
                "Beans, 250 g UGX 29,000",
                "Total UGX 45,000"
            ]
        );
        assert!(!format!("{paying:?}").contains("4242"));
    }

    /// The tree Chrome 157 gave of `handrun36/site/shop.html` — a table
    /// with no headers, named *Order summary*, read by Chrome as a layout
    /// table — its total's words and price in two cells.
    fn chromes_checkout() -> Snapshot {
        let declared = |id: usize, role: &str, name: &str, children| SnapshotNode {
            role: Role::Declared(role.into()),
            ..node(id, KnownRole::Group, name, children)
        };
        let cell = |id: usize, words: &str| {
            declared(id, "LayoutTableCell", words, vec![text(id + 100, words)])
        };
        let row = |id: usize, what: &str, price: &str| {
            declared(
                id,
                "LayoutTableRow",
                "",
                vec![cell(id + 1, what), cell(id + 2, price)],
            )
        };
        Snapshot {
            root: Some(node(
                6,
                KnownRole::Document,
                "Kampala Coffee \u{2014} Checkout",
                vec![
                    node(14, KnownRole::Heading, "Checkout", Vec::new()),
                    declared(
                        16,
                        "LayoutTable",
                        "Order summary",
                        vec![
                            row(18, "Flat white \u{d7} 2", "UGX 16,000"),
                            row(21, "Beans, 250 g", "UGX 29,000"),
                            row(24, "Total", "UGX 45,000"),
                        ],
                    ),
                    node(
                        1,
                        KnownRole::Form,
                        "",
                        vec![
                            field(2, "Name", "Ada"),
                            field(4, "Your long number", ""),
                            node(33, KnownRole::Button, "Pay now", Vec::new()),
                        ],
                    ),
                ],
            )),
        }
    }

    #[test]
    fn chromes_layout_table_is_read_as_a_tables_rows() {
        let paying = of(&chromes_checkout(), BoxId::from_wire(33), "127.0.0.1:8736");
        assert_eq!(paying.amount.as_deref(), Some("UGX 45,000"));
        assert_eq!(
            paying.lines,
            [
                "Flat white \u{d7} 2 UGX 16,000",
                "Beans, 250 g UGX 29,000",
                "Total UGX 45,000"
            ]
        );
    }

    #[test]
    fn a_total_whose_price_is_on_the_next_line_takes_it() {
        let page = Snapshot {
            root: Some(node(
                1,
                KnownRole::Document,
                "",
                vec![
                    node(2, KnownRole::Paragraph, "", vec![text(3, "Order total")]),
                    node(4, KnownRole::Paragraph, "", vec![text(5, "$42.00")]),
                    node(6, KnownRole::Button, "Place it", Vec::new()),
                ],
            )),
        };
        assert_eq!(
            of(&page, BoxId::from_wire(6), "a.example")
                .amount
                .as_deref(),
            Some("$42.00")
        );
    }

    #[test]
    fn the_buttons_own_price_is_the_amount_first() {
        let mut page = checkout();
        if let Some(root) = page.root.as_mut() {
            root.children[2].children[3].name = Some("Pay $42.50".to_owned());
        }
        let paying = of(&page, BoxId::from_wire(60), "shop.example");
        assert_eq!(paying.amount.as_deref(), Some("$42.50"));
    }

    #[test]
    fn with_no_total_and_no_summary_there_is_none_and_no_price_is_guessed() {
        let page = Snapshot {
            root: Some(node(
                1,
                KnownRole::Document,
                "",
                vec![
                    text(2, "Beans UGX 29,000"),
                    node(3, KnownRole::Button, "Pay", Vec::new()),
                ],
            )),
        };
        let paying = of(&page, BoxId::from_wire(3), "127.0.0.1:8736");
        assert_eq!(paying.amount, None, "an item's price is not the total");
        assert_eq!(paying.merchant, "127.0.0.1:8736");
        assert!(paying.lines.is_empty());
    }

    #[test]
    fn a_summary_under_its_heading_runs_to_the_next_heading_and_says_how_many_more() {
        let mut items: Vec<SnapshotNode> = vec![node(
            2,
            KnownRole::Heading,
            "",
            vec![text(3, "Your basket")],
        )];
        items.extend((0..9).map(|at| {
            node(
                10 + at * 2,
                KnownRole::Paragraph,
                "",
                vec![text(11 + at * 2, &format!("Item {at}"))],
            )
        }));
        items.push(node(90, KnownRole::Heading, "Pay", vec![text(91, "Pay")]));
        items.push(text(92, "Not this"));
        let page = Snapshot {
            root: Some(node(1, KnownRole::Document, "Shop", items)),
        };
        let lines = summary(&page);
        assert_eq!(lines.len(), MOST_SUMMARY);
        assert_eq!(lines[0], "Item 0");
        assert_eq!(lines[MOST_SUMMARY - 1], "and 4 more lines on the page");
        assert!(!lines.iter().any(|line| line.contains("Not this")));
    }

    #[test]
    fn prices_are_found_written_every_way_and_nothing_else_is_one() {
        for (words, price) in [
            ("Total UGX 45,000", Some("UGX 45,000")),
            ("Pay $42.00 now", Some("$42.00")),
            ("Total: 42,00 \u{20ac}", Some("42,00 \u{20ac}")),
            ("\u{a3}1,250.99", Some("\u{a3}1,250.99")),
            ("45 000 KES due", Some("45 000 KES")),
            ("Qty 2 45,000 UGX", Some("45,000 UGX")),
            ("45,000 KES", Some("45,000 KES")),
            ("USD12", Some("USD12")),
            ("Order 1234", None),
            ("BUSD 12", None),
            ("Total", None),
            ("$", None),
        ] {
            assert_eq!(price_in(words).as_deref(), price, "{words}");
        }
        assert!(says_total("Order total"));
        assert!(says_total("Amount due"));
        assert!(!says_total("Subtotal"));
    }
}
