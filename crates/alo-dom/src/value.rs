/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a control holds, read from its markup (ADR 0051 § 1).
//!
//! HTML keeps a control's value, checkedness and selectedness apart from
//! the attributes, which only give the defaults. That split is queue item
//! 469's (`control.rs`). Until it lands **the markup is the state**, as
//! [`field`](crate::field) and [`activation`](crate::activation) have it,
//! and every rule that needs a control's state — the constraints and the
//! entry list — reads it through **one function each** here, so the split
//! changes where an answer comes from and not who asks.
//!
//! # Sanitised as Chrome sanitises it
//!
//! A value is read through HTML's *value sanitization algorithm* for its
//! state, as Chrome 154 applies it (measured, with `value` and `FormData`):
//!
//! - `text`, `search`, `tel` and `password` lose their line breaks;
//! - `url` loses its line breaks and the white space around it;
//! - `email` loses the white space around it, and with `multiple` around
//!   each address, which are joined with a bare comma (`" a@b , c@d "` is
//!   `a@b,c@d`);
//! - `date` is a valid date string or nothing ([`crate::date`]);
//! - `number` is a valid floating-point number or nothing, as written
//!   (`1e3` stays `1e3`);
//! - `color` is a valid simple colour, lowercased, or `#000000`.
//!
//! `month`, `week`, `time`, `datetime-local` and `range` are not sanitised
//! here, because their rules are not built: [`is_unheld`] says so, and a form
//! holding one with a value refuses to submit rather than send what Chrome
//! would not (ADR 0013 § 3).

use crate::document::Document;
use crate::kind::{self, Kind};
use crate::node::NodeId;

/// The `<input>` states whose value is not read here yet, by the keyword
/// that names them, when the input holds one. A `range` always holds one.
pub fn is_unheld(document: &Document, node: NodeId) -> Option<&'static str> {
    let element = document.element(node)?;
    let kind = kind::of(element)?;
    let written = element.attr("value").is_some_and(|value| !value.is_empty());
    match kind {
        Kind::Range => Some(kind.keyword()),
        Kind::Month | Kind::Week | Kind::Time | Kind::DateTimeLocal if written => {
            Some(kind.keyword())
        }
        _ => None,
    }
}

/// The value of the `<input>` or `<textarea>` `node`, sanitised for its
/// state; the empty string for any other node.
///
/// A `<textarea>`'s is its content. A button's, a hidden input's and a
/// box's are their `value` attribute as written, and a box with none is
/// `on`.
pub fn value(document: &Document, node: NodeId) -> String {
    let Some(element) = document.element(node) else {
        return String::new();
    };
    if element.name.is_html("textarea") {
        return document.text_content(node);
    }
    let Some(kind) = kind::of(element) else {
        return String::new();
    };
    let written = element.attr("value").unwrap_or_default();
    match kind {
        Kind::Text | Kind::Search | Kind::Tel | Kind::Password => without_line_breaks(written),
        Kind::Email if element.attr("multiple").is_some() => written
            .split(',')
            .map(trimmed)
            .collect::<Vec<&str>>()
            .join(","),
        Kind::Url | Kind::Email => trimmed(&without_line_breaks(written)).to_owned(),
        Kind::Date => crate::date::held_date(element)
            .unwrap_or_default()
            .to_owned(),
        Kind::Number if is_valid_number(written) => written.to_owned(),
        Kind::Number => String::new(),
        Kind::Color if is_simple_colour(written) => written.to_ascii_lowercase(),
        Kind::Color => "#000000".to_owned(),
        Kind::Checkbox | Kind::Radio if element.attr("value").is_none() => "on".to_owned(),
        Kind::Hidden
        | Kind::Month
        | Kind::Week
        | Kind::Time
        | Kind::DateTimeLocal
        | Kind::Range
        | Kind::Checkbox
        | Kind::Radio
        | Kind::File
        | Kind::Submit
        | Kind::Image
        | Kind::Reset
        | Kind::Button => written.to_owned(),
    }
}

/// Whether the box `node` is checked: its `checked` attribute, until item
/// 469 holds checkedness apart from it.
pub fn checked(document: &Document, node: NodeId) -> bool {
    document
        .element(node)
        .is_some_and(|element| element.attr("checked").is_some())
}

/// The options of the `<select>` `node`: HTML's *list of options*, its
/// `option` children and those of its `optgroup` children, in tree order.
pub fn options(document: &Document, select: NodeId) -> Vec<NodeId> {
    let is = |node: NodeId, name: &str| {
        document
            .element(node)
            .is_some_and(|element| element.name.is_html(name))
    };
    let mut options = Vec::new();
    for child in document.children(select) {
        if is(child, "option") {
            options.push(child);
        } else if is(child, "optgroup") {
            options.extend(document.children(child).filter(|node| is(*node, "option")));
        }
    }
    options
}

/// The options of the `<select>` `node` that are selected, in tree order,
/// from their `selected` attributes until item 469.
///
/// HTML's *selectedness setting algorithm*: a select that shows one option
/// and takes one has exactly one selected while it has any to select — the
/// last one marked, or else the first that is not disabled — and any other
/// select has those marked, but only the last when it takes one.
pub fn selected(document: &Document, select: NodeId) -> Vec<NodeId> {
    let Some(element) = document.element(select) else {
        return Vec::new();
    };
    let options = options(document, select);
    let marked: Vec<NodeId> = options
        .iter()
        .copied()
        .filter(|option| {
            document
                .element(*option)
                .is_some_and(|option| option.attr("selected").is_some())
        })
        .collect();
    if element.attr("multiple").is_some() {
        return marked;
    }
    if let Some(last) = marked.last() {
        return vec![*last];
    }
    if display_size(document, select) == 1 {
        return options
            .into_iter()
            .find(|option| !crate::disabled::is_disabled_node(document, *option))
            .into_iter()
            .collect();
    }
    Vec::new()
}

/// How many options the `<select>` `node` shows at once: its `size` when
/// that is a number above nought, or else four for one that takes several
/// and one for one that takes one.
pub fn display_size(document: &Document, select: NodeId) -> u32 {
    let Some(element) = document.element(select) else {
        return 1;
    };
    match element
        .attr("size")
        .and_then(|size| size.trim().parse::<u32>().ok())
    {
        Some(size) if size > 0 => size,
        _ if element.attr("multiple").is_some() => 4,
        _ => 1,
    }
}

/// The value of the `<option>` `node`: its `value` attribute, or else its
/// text with white space stripped and collapsed.
pub fn option_value(document: &Document, option: NodeId) -> String {
    if let Some(value) = document
        .element(option)
        .and_then(|element| element.attr("value"))
    {
        return value.to_owned();
    }
    document
        .text_content(option)
        .split_ascii_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

fn without_line_breaks(text: &str) -> String {
    text.chars().filter(|c| !matches!(c, '\n' | '\r')).collect()
}

fn trimmed(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_ascii_whitespace())
}

/// Whether `text` is HTML's *valid floating-point number*: an optional `-`,
/// digits with an optional fraction or a fraction alone, and an optional
/// exponent.
fn is_valid_number(text: &str) -> bool {
    let rest = text.strip_prefix('-').unwrap_or(text);
    let (mantissa, exponent) = match rest.find(['e', 'E']) {
        Some(at) => (rest.get(..at).unwrap_or_default(), rest.get(at + 1..)),
        None => (rest, None),
    };
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    let mantissa_ok = match mantissa.split_once('.') {
        Some((whole, fraction)) => (whole.is_empty() || digits(whole)) && digits(fraction),
        None => digits(mantissa),
    };
    let exponent_ok = exponent
        .is_none_or(|exponent| digits(exponent.strip_prefix(['+', '-']).unwrap_or(exponent)));
    mantissa_ok && exponent_ok
}

/// Whether `text` is HTML's *valid simple colour*: `#` and six hex digits.
fn is_simple_colour(text: &str) -> bool {
    text.len() == 7
        && text.starts_with('#')
        && text.bytes().skip(1).all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_document;

    fn named(document: &Document, name: &str) -> NodeId {
        let Some(found) = document.descendants(document.root()).find(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.attr("name") == Some(name))
        }) else {
            panic!("nothing named {name}");
        };
        found
    }

    /// Each value Chrome 154 gave the same markup, from `value` and
    /// `FormData`.
    #[test]
    fn a_value_is_sanitised_as_chrome_sanitises_it() {
        let document = parse_document(
            "<input name=plain value=' x '><input name=lines value='a\nb'>\
             <input type=email name=em value=' a@b.c '>\
             <input type=email multiple name=ems value=' a@b , c@d '>\
             <input type=url name=ur value=' https://a.b '>\
             <input type=number name=nu value=1e3><input type=number name=nu2 value=abc>\
             <input type=date name=da value=2026-13-01><input type=date name=db value=2026-10-20>\
             <input type=color name=co><input type=color name=co2 value=#ABCDEF>\
             <input type=checkbox name=cb><input type=checkbox name=cb2 value=''>\
             <input type=hidden name=hi value=hv><textarea name=ta>\nline1\nline2</textarea>",
        );
        let cases = [
            ("plain", " x "),
            ("lines", "ab"),
            ("em", "a@b.c"),
            ("ems", "a@b,c@d"),
            ("ur", "https://a.b"),
            ("nu", "1e3"),
            ("nu2", ""),
            ("da", ""),
            ("db", "2026-10-20"),
            ("co", "#000000"),
            ("co2", "#abcdef"),
            ("cb", "on"),
            ("cb2", ""),
            ("hi", "hv"),
            ("ta", "line1\nline2"),
        ];
        for (name, expected) in cases {
            assert_eq!(value(&document, named(&document, name)), expected, "{name}");
        }
    }

    #[test]
    fn a_number_is_valid_as_html_writes_one() {
        for good in ["0", "-1", "1.5", ".5", "1e3", "1E-3", "-0.25e+2"] {
            assert!(is_valid_number(good), "{good}");
        }
        for bad in [
            "", "-", "1.", "+1", "1e", "e3", "1.2.3", " 1", "0x10", "1e+",
        ] {
            assert!(!is_valid_number(bad), "{bad}");
        }
    }

    #[test]
    fn the_selected_options_are_chromes() {
        let document = parse_document(
            "<select name=sel><option>  one   two </option><option>b</option></select>\
             <select name=sel2><option disabled>x</option><option>y</option></select>\
             <select name=sel3 multiple><option selected>m1</option>\
             <option selected value=m2v>m2</option><option>m3</option></select>\
             <select name=sel4><option value=a selected>a</option>\
             <option value=b selected>b</option></select>\
             <select name=sel5></select><select name=sel6 size=3><option>q</option></select>\
             <select name=grouped><optgroup><option>g</option></optgroup></select>",
        );
        let values = |name| -> Vec<String> {
            selected(&document, named(&document, name))
                .into_iter()
                .map(|option| option_value(&document, option))
                .collect()
        };
        assert_eq!(values("sel"), ["one two"]);
        assert_eq!(values("sel2"), ["y"]);
        assert_eq!(values("sel3"), ["m1", "m2v"]);
        assert_eq!(values("sel4"), ["b"]);
        assert!(values("sel5").is_empty());
        assert!(
            values("sel6").is_empty(),
            "a list box selects nothing itself"
        );
        assert_eq!(values("grouped"), ["g"]);
    }

    #[test]
    fn the_states_whose_values_are_not_read_say_so_when_they_hold_one() {
        let document = parse_document(
            "<input type=time name=t><input type=time name=t2 value=10:00>\
             <input type=range name=r><input type=week name=w value=''>",
        );
        assert_eq!(is_unheld(&document, named(&document, "t")), None);
        assert_eq!(is_unheld(&document, named(&document, "t2")), Some("time"));
        assert_eq!(is_unheld(&document, named(&document, "r")), Some("range"));
        assert_eq!(is_unheld(&document, named(&document, "w")), None);
    }
}
