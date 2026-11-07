/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the caret may stand in a field's words, and where a word begins
//! and ends (queue item 861): what an arrow, ⌥ with an arrow, ⌥-Backspace
//! and a double press move by.
//!
//! Every place is a count of characters — Unicode scalar values — from the
//! words' start, as the field holds them ([`crate::text_field`]).
//!
//! # A letter with its marks is one place
//!
//! An arrow steps over a **letter and everything that hangs on it**: a
//! combining mark (`e` and U+0301 are `é`), a variation selector, an emoji's
//! skin tone and tags, and a sequence joined by U+200D (a family of four is
//! one), a pair of regional indicators (a flag) — so the caret is never
//! put inside what is drawn as one. Not every script's marks are known
//! here: those of Latin, Greek, Cyrillic and the combining blocks, the
//! selectors and the joiners are; a Devanagari vowel sign is not yet, and
//! is stepped over alone (queue item 864).
//!
//! # A word is what Unicode says one is, nearly
//!
//! A word is a run of letters, digits and `_`, which go on across one `.`,
//! `:`, `'`, `’` or `·` standing between two of them — as Unicode's word
//! boundaries (UAX #29) join `e.g` and `don't`, and macOS's own fields do:
//! `en.wikipedia.org` is one word and `/` is not in one. Digits go on across
//! `,` and `;` between two digits (`3,000`). Everything else — spaces and
//! other punctuation — stands between words, and ⌥→ passes over it to the
//! next word's end, as macOS's does.

/// Whether `c` hangs on the character before it rather than standing alone.
fn hangs_on(c: char) -> bool {
    matches!(
        u32::from(c),
        0x0300..=0x036F   // combining diacritical marks
        | 0x0483..=0x0489 // Cyrillic's
        | 0x1AB0..=0x1AFF // their extension
        | 0x1DC0..=0x1DFF // and supplement
        | 0x20D0..=0x20FF // for symbols
        | 0xFE20..=0xFE2F // half marks
        | 0xFE00..=0xFE0F // variation selectors
        | 0xE0100..=0xE01EF
        | 0x1F3FB..=0x1F3FF // an emoji's skin tones
        | 0xE0020..=0xE007F // its tags
        | 0x200D // the joiner itself
        | 0x3099..=0x309A // kana's voicing marks
    )
}

/// Whether `c` is a regional indicator, two of which are a flag.
fn regional(c: char) -> bool {
    matches!(u32::from(c), 0x1F1E6..=0x1F1FF)
}

/// The place after the one at `at` in `chars`: past a letter and all that
/// hangs on it; the end stays the end.
pub fn after(chars: &[char], at: usize) -> usize {
    let Some(first) = chars.get(at) else {
        return chars.len().min(at);
    };
    let mut next = at + 1;
    if regional(*first) && chars.get(next).copied().is_some_and(regional) {
        next += 1;
    }
    while let Some(c) = chars.get(next) {
        if *c == '\u{200D}' {
            // A joiner takes the character after it too.
            next = (next + 2).min(chars.len());
        } else if hangs_on(*c) {
            next += 1;
        } else {
            break;
        }
    }
    next
}

/// The place before the one at `at` in `chars`: back past a letter and all
/// that hangs on it; the start stays the start.
pub fn before(chars: &[char], at: usize) -> usize {
    let at = at.min(chars.len());
    // Walk forward from the start, so a letter's marks and a flag's pair
    // are counted the way [`after`] counts them.
    let mut place = 0;
    let mut last = 0;
    while place < at {
        last = place;
        place = after(chars, place);
    }
    last
}

/// What a character is, as words are told apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A letter, a digit or `_`: in a word.
    Word,
    /// White space.
    Space,
    /// Anything else.
    Other,
}

fn kind(c: char) -> Kind {
    if c.is_alphanumeric() || c == '_' || hangs_on(c) {
        Kind::Word
    } else if c.is_whitespace() {
        Kind::Space
    } else {
        Kind::Other
    }
}

/// Whether the character at `at` is in a word: a word's own, or one of the
/// few that join two of a word's between them.
fn in_a_word(chars: &[char], at: usize) -> bool {
    let Some(c) = chars.get(at) else {
        return false;
    };
    if kind(*c) == Kind::Word {
        return true;
    }
    let (Some(previous), Some(next)) = (
        at.checked_sub(1).and_then(|back| chars.get(back)),
        chars.get(at + 1),
    ) else {
        return false;
    };
    let joins_letters = matches!(c, '.' | ':' | '\'' | '\u{2019}' | '\u{00B7}');
    let joins_digits = matches!(c, ',' | ';' | '.');
    (joins_letters && kind(*previous) == Kind::Word && kind(*next) == Kind::Word)
        || (joins_digits && previous.is_numeric() && next.is_numeric())
}

/// Where the word at or after `at` ends: what ⌥→ moves to.
pub fn word_end(chars: &[char], at: usize) -> usize {
    let mut place = at.min(chars.len());
    while place < chars.len() && !in_a_word(chars, place) {
        place += 1;
    }
    while place < chars.len() && in_a_word(chars, place) {
        place += 1;
    }
    place
}

/// Where the word before `at` begins: what ⌥← moves to and ⌥-Backspace
/// erases back to.
pub fn word_start(chars: &[char], at: usize) -> usize {
    let mut place = at.min(chars.len());
    while place > 0 && !in_a_word(chars, place - 1) {
        place -= 1;
    }
    while place > 0 && in_a_word(chars, place - 1) {
        place -= 1;
    }
    place
}

/// The word a double press at `at` selects, as a start and an end: the
/// word the character at `at` is in; the run of white space it is in; or
/// that one character, if it is punctuation. At the words' end, the
/// character before it is pressed.
pub fn word_at(chars: &[char], at: usize) -> (usize, usize) {
    if chars.is_empty() {
        return (0, 0);
    }
    let at = at.min(chars.len() - 1);
    if in_a_word(chars, at) {
        let mut start = at;
        while start > 0 && in_a_word(chars, start - 1) {
            start -= 1;
        }
        let mut end = at;
        while end < chars.len() && in_a_word(chars, end) {
            end += 1;
        }
        return (start, end);
    }
    let pressed = chars.get(at).copied().map(kind);
    if pressed == Some(Kind::Space) {
        let mut start = at;
        while start > 0 && chars.get(start - 1).copied().map(kind) == Some(Kind::Space) {
            start -= 1;
        }
        let mut end = at;
        while end < chars.len() && chars.get(end).copied().map(kind) == Some(Kind::Space) {
            end += 1;
        }
        return (start, end);
    }
    (at, after(chars, at))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    #[test]
    fn an_arrow_steps_over_a_letter_and_what_hangs_on_it() {
        let text = chars("ae\u{301}b");
        assert_eq!(after(&text, 0), 1);
        assert_eq!(after(&text, 1), 3, "e and its acute");
        assert_eq!(before(&text, 3), 1);
        assert_eq!(before(&text, 4), 3);
        assert_eq!(after(&text, 4), 4, "the end stays");
        assert_eq!(before(&text, 0), 0, "the start stays");
        // A family joined by U+200D, a skin tone, a flag.
        let family = chars("a\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}b");
        assert_eq!(after(&family, 1), 6);
        assert_eq!(before(&family, 6), 1);
        let toned = chars("\u{1F44D}\u{1F3FD}!");
        assert_eq!(after(&toned, 0), 2);
        let flag = chars("\u{1F1FA}\u{1F1EC}\u{1F1F5}\u{1F1F9}");
        assert_eq!(after(&flag, 0), 2, "UG is one flag");
        assert_eq!(after(&flag, 2), 4, "PT the next");
        assert_eq!(before(&flag, 4), 2);
    }

    #[test]
    fn option_arrows_move_by_words_as_macos_does() {
        let address = chars("https://en.wikipedia.org/wiki/Kampala");
        // From the start, ⌥→ ends `https`, then the host, whole.
        assert_eq!(word_end(&address, 0), 5);
        assert_eq!(word_end(&address, 5), 24, "en.wikipedia.org is one word");
        assert_eq!(word_end(&address, 24), 29);
        // From the end, ⌥← begins `Kampala`, then `wiki`, then the host.
        assert_eq!(word_start(&address, 37), 30);
        assert_eq!(word_start(&address, 30), 25);
        assert_eq!(word_start(&address, 25), 8);
        let said = chars("don't stop  now.");
        assert_eq!(word_end(&said, 0), 5, "don't is one word");
        assert_eq!(word_end(&said, 5), 10);
        assert_eq!(word_start(&said, 16), 12, "past the full stop");
        assert_eq!(word_start(&said, 0), 0);
        assert_eq!(
            word_end(&chars("UGX 45,000 now"), 3),
            10,
            "digits join across a comma"
        );
    }

    #[test]
    fn a_double_press_selects_a_word_a_run_of_spaces_or_one_mark() {
        let said = chars("open  en.wikipedia.org/wiki now");
        assert_eq!(word_at(&said, 1), (0, 4));
        assert_eq!(word_at(&said, 4), (4, 6), "the two spaces");
        assert_eq!(word_at(&said, 10), (6, 22), "the host");
        assert_eq!(word_at(&said, 22), (22, 23), "the slash alone");
        assert_eq!(word_at(&said, 31), (28, 31), "at the end, the last word");
        assert_eq!(word_at(&[], 0), (0, 0));
    }
}
