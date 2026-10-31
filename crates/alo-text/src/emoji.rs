/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Which characters are meant to be shown as emoji, and which keep to the
//! character before them (ADR 0088).
//!
//! A font either has a character or it does not ([`crate::Font::has_glyph`]),
//! and for most characters that is the whole question. Emoji add a second:
//! *how is it meant to look?* `😀` is meant to be a picture; `©` is meant to
//! be a letter, though an emoji face has one too; and `❤` is a letter until
//! it is followed by U+FE0F, which asks for the picture. Unicode answers this
//! in a table — `Emoji`, `Emoji_Presentation` and `Emoji_Component`
//! (UTS #51) — which is rented (`unicode-properties`), and this is the only
//! file that names it.
//!
//! What is done with the answer is ours, and it is what a browser does: a
//! character meant to be a picture is looked for first in a face that draws
//! pictures ([`crate::Font::draws_pictures`]), and one meant to be a letter
//! first in a face that does not — each falling back to the plain chain when
//! no such face has it, so nothing that was drawn before is lost.
//!
//! # What keeps to the character before it
//!
//! An emoji is often several characters drawn as one: a base and a
//! variation selector (`❤️`), a skin tone (`👋🏽`), people joined by U+200D
//! (`👩‍💻`), a keycap (`1️⃣`), a flag's tags. Those that only modify what came
//! before — [`joins_previous`] — stay in its run whatever font has them, so
//! the shaper is handed the whole sequence in one face and can draw it as the
//! one picture the face holds for it.

use unicode_properties::emoji::{EmojiStatus, UnicodeEmoji};

/// How a character is meant to be shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presentation {
    /// As a letter: the ordinary chain, a face of outlines first.
    Text,
    /// As a picture: a face that draws pictures first.
    Emoji,
}

/// U+FE0E, which asks for the character before it as a letter.
const AS_TEXT: char = '\u{fe0e}';

/// U+FE0F, which asks for the character before it as a picture.
const AS_EMOJI: char = '\u{fe0f}';

/// How `character` is meant to be shown, when `next` follows it.
///
/// A variation selector after it decides; otherwise Unicode's
/// `Emoji_Presentation` does. A regional indicator — half a flag — is a
/// picture, as `Emoji_Presentation` says.
pub fn presentation(character: char, next: Option<char>) -> Presentation {
    match next {
        Some(AS_TEXT) => return Presentation::Text,
        Some(AS_EMOJI) if character.is_emoji_char() => return Presentation::Emoji,
        _ => {}
    }
    // Every status that says `Emoji_Presentation=Yes`; the table may grow
    // statuses, and one this does not name is shown as a letter, as before.
    let pictured = matches!(
        character.emoji_status(),
        EmojiStatus::EmojiPresentation
            | EmojiStatus::EmojiPresentationAndModifierBase
            | EmojiStatus::EmojiPresentationAndEmojiComponent
            | EmojiStatus::EmojiPresentationAndModifierAndEmojiComponent
    );
    if pictured {
        Presentation::Emoji
    } else {
        Presentation::Text
    }
}

/// Whether `character` only modifies the character before it, and so stays
/// in its run: a variation selector, the zero-width joiner, the keycap's
/// enclosing mark, a skin tone, or a tag.
pub fn joins_previous(character: char) -> bool {
    matches!(
        character,
        AS_TEXT
            | AS_EMOJI
            | '\u{200d}'
            | '\u{20e3}'
            | '\u{1f3fb}'..='\u{1f3ff}'
            | '\u{e0020}'..='\u{e007f}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_face_is_meant_as_a_picture_and_a_copyright_sign_as_a_letter() {
        for character in ['😀', '🚗', '⭐', '🇺', '👍'] {
            assert_eq!(
                presentation(character, None),
                Presentation::Emoji,
                "{character:?}"
            );
        }
        for character in ['a', '1', '#', '©', '™', '★', '❤', 'Ж', '中'] {
            assert_eq!(
                presentation(character, None),
                Presentation::Text,
                "{character:?}"
            );
        }
    }

    #[test]
    fn a_variation_selector_after_a_character_decides_how_it_is_shown() {
        assert_eq!(presentation('❤', Some(AS_EMOJI)), Presentation::Emoji);
        assert_eq!(presentation('1', Some(AS_EMOJI)), Presentation::Emoji);
        assert_eq!(presentation('😀', Some(AS_TEXT)), Presentation::Text);
        assert_eq!(
            presentation('a', Some(AS_EMOJI)),
            Presentation::Text,
            "a letter that is no emoji stays a letter whatever follows it"
        );
    }

    #[test]
    fn what_modifies_the_character_before_it_joins_its_run() {
        for character in [AS_TEXT, AS_EMOJI, '\u{200d}', '\u{20e3}', '🏽', '\u{e0067}'] {
            assert!(joins_previous(character), "{character:?}");
        }
        for character in ['a', '😀', ' ', '\u{301}'] {
            assert!(!joins_previous(character), "{character:?}");
        }
    }
}
