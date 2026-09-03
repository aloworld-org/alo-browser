/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A font's own name, when the font wrote it down before Unicode existed.
//!
//! **This is the only file in this crate that names `encoding_rs`.** Which byte
//! means which character in Mac OS Roman is a table the industry settled
//! decades ago, and ADR 0001 rents that kind of thing — `alo-net`'s `encoding`
//! module rents the same crate for the same reason, one layer away and about a
//! page's bytes rather than a font's.
//!
//! # Why a font needs this at all
//!
//! A `name` table holds the same name several times over, once per platform
//! that was ever expected to read it. Nearly every font carries a Windows
//! record in UTF-16, which is Unicode and needs nothing from here. Several of
//! the fonts macOS ships — Apple Braille among them — carry **only** the
//! Macintosh records, and a font whose name nobody can read is a font a page
//! cannot ask for: [`crate::family_in`] answered [`None`], the browser process
//! reported the family as absent, and a machine that had the font said it did
//! not.
//!
//! # Two encodings, and why not the other twenty
//!
//! The Macintosh platform defines an encoding for most writing systems it ever
//! supported: Japanese, Traditional Chinese, Korean, Arabic, Hebrew, Greek and
//! a dozen more. Only two of them are read here, and the rule is the same one
//! that put this file in the engine:
//!
//! **Read the ones somebody else's table defines exactly; guess at none of
//! them.** `macintosh` and `x-mac-cyrillic` are encodings the WHATWG standard
//! names, so the rented tables *are* Apple's for those two. The rest have no
//! such table: Mac OS Japanese is close to Shift JIS and is not Shift JIS, and
//! decoding one as the other would put a character Apple never wrote into the
//! name of somebody's font. A family read wrongly is worse than one not read —
//! it is a name a page can match by accident.
//!
//! So a record in any other Macintosh encoding answers [`None`], which is
//! exactly what every Macintosh record did before this file existed. Such a
//! font is reported as unnameable and a page asking for it gets a substitution
//! it can see, which is the safe direction and is where the honest answer is.

/// Mac OS Roman, and what nearly every Macintosh name record is written in.
const ROMAN: u16 = 0;

/// Mac OS Cyrillic.
const CYRILLIC: u16 = 7;

/// A Macintosh name record's bytes as text, or [`None`] for an encoding this
/// engine will not guess at.
///
/// `encoding_id` is the record's own, from the `name` table.
pub fn text(encoding_id: u16, bytes: &[u8]) -> Option<String> {
    let encoding = match encoding_id {
        ROMAN => encoding_rs::MACINTOSH,
        CYRILLIC => encoding_rs::X_MAC_CYRILLIC,
        _ => return None,
    };
    // Without BOM handling: a font name has no byte order mark, and the three
    // bytes that would spell one are three ordinary characters in both of these
    // encodings.
    let (text, had_errors) = encoding.decode_without_bom_handling(bytes);
    if had_errors {
        // Unreachable for a single-byte table, where every one of the 256 bytes
        // stands for a character. It is checked rather than assumed because the
        // list above may one day gain an encoding where it is not true, and a
        // name half-read is a name that matches something by accident.
        return None;
    }
    Some(text.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_in_mac_os_roman_is_read_as_mac_os_roman() {
        // 0xD5 is a right single quote in Mac OS Roman and `Õ` in Latin-1, so a
        // decoder that had quietly fallen back to Latin-1 fails here. That is
        // the whole point of the byte: the two encodings agree below 0x80 and a
        // test written in ASCII would pass either way.
        assert_eq!(
            text(ROMAN, b"Apple\xD5s Font").as_deref(),
            Some("Apple\u{2019}s Font"),
        );
        assert_eq!(
            text(ROMAN, b"Hiragino Mincho \xA5 \x8C").as_deref(),
            Some("Hiragino Mincho \u{2022} \u{e5}"),
            "a bullet and an a-ring, neither of which is at that byte in Latin-1",
        );
        assert_eq!(
            text(ROMAN, b"Helvetica").as_deref(),
            Some("Helvetica"),
            "and the ASCII a real family name is nearly always all of",
        );
    }

    #[test]
    fn a_name_in_mac_os_cyrillic_is_read_as_mac_os_cyrillic() {
        assert_eq!(
            text(CYRILLIC, b"\x80\x81\xE0").as_deref(),
            Some("\u{410}\u{411}\u{430}"),
        );
    }

    #[test]
    fn an_encoding_this_engine_will_not_guess_at_is_not_guessed_at() {
        // Japanese, Traditional Chinese, Arabic, Hebrew, Greek, and Simplified
        // Chinese. Each has a table of Apple's that nobody else's crate holds,
        // so each answers nothing rather than being read as its near relative.
        for encoding_id in [1, 2, 4, 5, 6, 25] {
            assert_eq!(
                text(encoding_id, b"a name"),
                None,
                "encoding {encoding_id} was decoded by guessing",
            );
        }
    }

    #[test]
    fn every_byte_a_font_could_carry_is_read_rather_than_refused() {
        // The bytes come out of a file somebody else wrote, so the answer for
        // any of them must be an answer. A single-byte table has a character
        // for all 256, which is what makes the `had_errors` branch above
        // unreachable today rather than merely unlikely.
        let all: Vec<u8> = (0..=255).collect();
        let roman = text(ROMAN, &all).expect("every byte is a character in Mac OS Roman");
        assert_eq!(roman.chars().count(), 256);
        let cyrillic = text(CYRILLIC, &all).expect("and in Mac OS Cyrillic");
        assert_eq!(cyrillic.chars().count(), 256);
    }

    #[test]
    fn a_record_with_no_bytes_in_it_is_no_name_rather_than_no_answer() {
        // Empty is a real record — the standard permits one — and it is the
        // caller that decides an empty name is not a family.
        assert_eq!(text(ROMAN, b"").as_deref(), Some(""));
    }
}
