/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `application/x-www-form-urlencoded`: the URL Standard's serializer, which
//! is how a form sent by `GET` writes its query and how one sent by `POST`
//! writes its body (ADR 0051 § 5, queue item 470), and its parser, which is
//! how `new URLSearchParams("a=b&c=d")` reads a string (queue item 472).
//!
//! Names and values are UTF-8, which is every document here. A byte that is
//! an ASCII letter or digit, or one of `*`, `-`, `.` and `_`, is written as it
//! is; a space is `+`; every other byte is `%` and two upper-case hex digits.
//! So `hello world&=é` is `hello+world%26%3D%C3%A9`, which is what Chrome 154
//! sent for it (ADR 0051, *Measured*).
//!
//! Nothing here normalises a line break. HTML does that to the entry list
//! before it is encoded, and it is the encoder's caller that holds the list.
//!
//! The parser splits on `&`, skips what is empty, splits each piece at its
//! first `=`, turns `+` into a space, percent-decodes, and reads the bytes as
//! UTF-8 with each broken sequence a U+FFFD. A `%` not followed by two hex
//! digits is kept as it is. Chrome 154 reads `a=b+c&d=%zz&&e&=f` as `a`
//! holding `b c`, `d` holding `%zz`, `e` holding nothing, and the empty name
//! holding `f`, which is what this does.

/// The pairs, in order, as one `name=value&name=value` string.
pub fn urlencoded<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> String {
    let mut written = String::new();
    for (name, value) in pairs {
        if !written.is_empty() {
            written.push('&');
        }
        write_bytes(&mut written, name);
        written.push('=');
        write_bytes(&mut written, value);
    }
    written
}

/// The pairs `input` holds, in order: the URL Standard's
/// `application/x-www-form-urlencoded` parser.
pub fn parse(input: &[u8]) -> Vec<(String, String)> {
    input
        .split(|byte| *byte == b'&')
        .filter(|piece| !piece.is_empty())
        .map(|piece| {
            let (name, value) = match piece.iter().position(|byte| *byte == b'=') {
                Some(at) => (
                    piece.get(..at).unwrap_or_default(),
                    piece.get(at.saturating_add(1)..).unwrap_or_default(),
                ),
                None => (piece, &[][..]),
            };
            (decoded(name), decoded(value))
        })
        .collect()
}

/// One name or value as the parser reads it: `+` as a space, then
/// percent-decoded, then UTF-8 with each broken sequence a U+FFFD.
fn decoded(bytes: &[u8]) -> String {
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let escaped = (byte == b'%')
            .then(|| {
                Some((
                    hex(*bytes.get(at.saturating_add(1))?)?,
                    hex(*bytes.get(at.saturating_add(2))?)?,
                ))
            })
            .flatten();
        match (byte, escaped) {
            (_, Some((high, low))) => {
                out.push(high << 4 | low);
                at = at.saturating_add(3);
            }
            (b'+', None) => {
                out.push(b' ');
                at = at.saturating_add(1);
            }
            (_, None) => {
                out.push(byte);
                at = at.saturating_add(1);
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The value of one ASCII hex digit.
fn hex(digit: u8) -> Option<u8> {
    char::from(digit)
        .to_digit(16)
        .and_then(|value| u8::try_from(value).ok())
}

/// `text`'s UTF-8 bytes, as the serializer writes them.
fn write_bytes(into: &mut String, text: &str) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                into.push(char::from(byte));
            }
            b' ' => into.push('+'),
            _ => {
                into.push('%');
                for nibble in [byte >> 4, byte & 0x0F] {
                    if let Some(digit) = HEX.get(usize::from(nibble)) {
                        into.push(char::from(*digit));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_chrome_sent_is_what_is_written() {
        assert_eq!(
            urlencoded([("q", "hello world&=é")]),
            "q=hello+world%26%3D%C3%A9"
        );
        assert_eq!(
            urlencoded([
                ("custname", "Ada Lovelace"),
                ("custtel", ""),
                ("custemail", ""),
                ("delivery", ""),
                ("comments", ""),
            ]),
            "custname=Ada+Lovelace&custtel=&custemail=&delivery=&comments="
        );
        assert_eq!(urlencoded([("date", "2026-10-20")]), "date=2026-10-20");
    }

    #[test]
    fn a_line_break_and_every_reserved_byte_are_escaped() {
        assert_eq!(urlencoded([("c", "a\r\nb")]), "c=a%0D%0Ab");
        assert_eq!(
            urlencoded([("+%?#/~!'()", "")]),
            "%2B%25%3F%23%2F%7E%21%27%28%29="
        );
        assert_eq!(urlencoded([("*-._", "AZaz09")]), "*-._=AZaz09");
        assert_eq!(urlencoded([("", "")]), "=");
    }

    #[test]
    fn nothing_to_send_is_nothing_written() {
        assert_eq!(urlencoded([]), "");
        assert_eq!(urlencoded([("a", "1"), ("a", "2")]), "a=1&a=2");
    }

    #[test]
    fn what_chrome_read_is_what_is_parsed() {
        // Chrome 154's `new URLSearchParams(…)` of each, written back out.
        let read = parse(b"a=b+c&d=%zz&&e&=f&g=%C3%A9%2B");
        assert_eq!(
            read,
            [
                ("a".to_owned(), "b c".to_owned()),
                ("d".to_owned(), "%zz".to_owned()),
                ("e".to_owned(), String::new()),
                (String::new(), "f".to_owned()),
                ("g".to_owned(), "é+".to_owned()),
            ]
        );
        let pairs: Vec<(&str, &str)> = read.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
        assert_eq!(urlencoded(pairs), "a=b+c&d=%25zz&e=&=f&g=%C3%A9%2B");
        assert_eq!(
            parse(b"a=%F0%9F%98%80&b=%FF"),
            [
                ("a".to_owned(), "😀".to_owned()),
                ("b".to_owned(), "\u{FFFD}".to_owned())
            ]
        );
    }

    #[test]
    fn a_broken_escape_is_kept_and_nothing_is_nothing() {
        assert_eq!(parse(b""), []);
        assert_eq!(parse(b"&&"), []);
        assert_eq!(
            parse(b"%=%4=%4g==="),
            [("%".to_owned(), "%4=%4g===".to_owned())]
        );
        assert_eq!(parse(b"%41%2b=1=2"), [("A+".to_owned(), "1=2".to_owned())]);
        assert_eq!(parse(b"\xFF"), [("\u{FFFD}".to_owned(), String::new())]);
    }

    #[test]
    fn a_character_outside_the_basic_plane_is_its_four_bytes() {
        assert_eq!(urlencoded([("e", "😀")]), "e=%F0%9F%98%80");
        assert_eq!(urlencoded([("nul", "\u{0}")]), "nul=%00");
    }
}
