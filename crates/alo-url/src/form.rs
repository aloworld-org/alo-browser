/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `application/x-www-form-urlencoded`: the URL Standard's serializer, which
//! is how a form sent by `GET` writes its query and how one sent by `POST`
//! writes its body (ADR 0051 § 5, queue item 470).
//!
//! Names and values are UTF-8, which is every document here. A byte that is
//! an ASCII letter or digit, or one of `*`, `-`, `.` and `_`, is written as it
//! is; a space is `+`; every other byte is `%` and two upper-case hex digits.
//! So `hello world&=é` is `hello+world%26%3D%C3%A9`, which is what Chrome 154
//! sent for it (ADR 0051, *Measured*).
//!
//! Nothing here normalises a line break. HTML does that to the entry list
//! before it is encoded, and it is the encoder's caller that holds the list.

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
    fn a_character_outside_the_basic_plane_is_its_four_bytes() {
        assert_eq!(urlencoded([("e", "😀")]), "e=%F0%9F%98%80");
        assert_eq!(urlencoded([("nul", "\u{0}")]), "nul=%00");
    }
}
