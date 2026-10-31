/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A search result's title and description, as Brave writes them, made
//! plain text (ADR 0083 § 4, queue item 708).
//!
//! Brave writes both as HTML text: a character that would be markup as an
//! entity — `&amp;`, `&#x27;` — and `<strong>` around the words that
//! matched. alo shows them as plain text, so:
//!
//! - `<strong>` and `</strong>`, in any case, are taken out, and the
//!   emphasis is not drawn: the design shows none.
//! - `&amp; &lt; &gt; &quot; &apos;` and numeric references — `&#39;`,
//!   `&#x27;` — are read as the characters they name. One that names a
//!   control character, or no character at all, is read as nothing.
//! - Anything else is the characters it is: `<b>` stays `<b>`, and a `&`
//!   that names nothing stays `&`.
//!
//! The result is a stranger's string like any other, which the first
//! screen's renderer puts into its document as a text node. Nothing here is
//! parsed as markup, and nothing is made of what it says.

/// The most characters of a title or a description kept: far more than two
/// lines show, so a hostile answer cannot make the first screen's document
/// without end.
pub const LONGEST: usize = 600;

/// `written`, as Brave writes a title or a description, as plain text, at
/// most [`LONGEST`] characters.
pub fn plain(written: &str) -> String {
    let mut out = String::with_capacity(written.len().min(LONGEST * 4));
    let mut rest = written;
    let mut kept = 0usize;
    while let Some(first) = rest.chars().next() {
        if kept >= LONGEST {
            break;
        }
        if first == '<' {
            if let Some(after) = strong_tag(rest) {
                rest = after;
                continue;
            }
        } else if first == '&'
            && let Some((named, after)) = entity(rest)
        {
            kept += usize::from(named.is_some());
            out.extend(named);
            rest = after;
            continue;
        }
        kept += 1;
        out.push(first);
        rest = rest.get(first.len_utf8()..).unwrap_or_default();
    }
    out
}

/// What follows `<strong>` or `</strong>` at the start of `text`, in any
/// case, if it begins with one.
fn strong_tag(text: &str) -> Option<&str> {
    ["<strong>", "</strong>"].iter().find_map(|tag| {
        let head = text.get(..tag.len())?;
        head.eq_ignore_ascii_case(tag)
            .then(|| text.get(tag.len()..))
            .flatten()
    })
}

/// The character the entity at the start of `text` names — or nothing, for
/// one naming a control character or no character — and what follows it,
/// if it begins with one this reads.
fn entity(text: &str) -> Option<(Option<char>, &str)> {
    let end = text.get(..12.min(text.len()))?.find(';')?;
    let name = text.get(1..end)?;
    let after = text.get(end + 1..)?;
    let named = match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let number = name.strip_prefix('#')?;
            let (digits, radix) = match number.strip_prefix(['x', 'X']) {
                Some(hex) => (hex, 16),
                None => (number, 10),
            };
            if digits.is_empty() || !digits.chars().all(|digit| digit.is_digit(radix)) {
                return None;
            }
            let value = u32::from_str_radix(digits, radix).ok()?;
            char::from_u32(value).filter(|named| !named.is_control())
        }
    };
    Some((named, after))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strong_is_taken_out_and_entities_read() {
        assert_eq!(
            plain("The <strong>borrow checker</strong> &amp; you"),
            "The borrow checker & you"
        );
        assert_eq!(plain("<STRONG>Rust</Strong>&#x27;s"), "Rust's");
        assert_eq!(plain("it&#39;s &quot;fine&quot; &lt;3"), "it's \"fine\" <3");
        assert_eq!(plain("&apos;a&gt;"), "'a>");
    }

    #[test]
    fn other_markup_and_unknown_entities_stay_as_written() {
        assert_eq!(plain("<b>Fish</b> &amp chips"), "<b>Fish</b> &amp chips");
        assert_eq!(plain("&nbsp;&copy; a & b"), "&nbsp;&copy; a & b");
        assert_eq!(plain("<script>x()</script>"), "<script>x()</script>");
        assert_eq!(plain("<strong"), "<strong");
        assert_eq!(plain("&#;&#x;"), "&#;&#x;");
    }

    #[test]
    fn a_reference_to_nothing_or_a_control_character_is_nothing() {
        assert_eq!(plain("a&#0;b&#x7;c&#xD800;d&#99999999;e"), "abcde");
        assert_eq!(plain("&#x1F980;"), "\u{1f980}");
    }

    #[test]
    fn what_is_kept_is_bounded_and_never_cut_inside_a_character() {
        let long = "é".repeat(LONGEST * 3);
        assert_eq!(plain(&long).chars().count(), LONGEST);
        assert_eq!(plain(""), "");
        assert_eq!(plain("&"), "&");
        assert_eq!(plain("é&"), "é&");
    }
}
