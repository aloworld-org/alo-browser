/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a chat is called, from what was said in it (queue item 758, ADR
//! 0092 § 5).
//!
//! A chat with no turns is *New chat* (`alo-renderer`'s `panel::NEW_CHAT`).
//! Once it holds one it is named by **the first words said in it**: the
//! first turn's words — a search's, an address typed, or a site's title
//! pressed — their white space run together, as many whole words as fit in
//! [`LONGEST_NAME`] characters, and `…` after them when any were left out.
//! One word longer than that on its own is cut at a character. Named once,
//! as the first turn is made, and never again: a later turn does not rename
//! it, and the name is kept with the chat (ADR 0092).
//!
//! No model names it: which model alo thinks with is not decided (ADR
//! 0081), and an agent's name for a chat waits on the agent's record (queue
//! item 704). A rule over the person's own words is what can be checked
//! today, and says nothing they did not say.

/// The most characters of a person's words a chat's name holds, before the
/// `…` that says more was said.
///
/// More than the panel's row shows — 13.5 px text in a row about 190 wide
/// holds some thirty — which the panel's own sheet cuts where the row ends;
/// so *Search chats* finds a chat by more of its words than the row draws.
pub const LONGEST_NAME: usize = 48;

/// What a chat whose first turn said `said` is called: its first words, or
/// nothing — *New chat* — when it said only white space.
pub fn of(said: &str) -> String {
    let mut name = String::new();
    let mut held = 0;
    for word in said.split_whitespace() {
        let length = word.chars().count();
        let gap = usize::from(held > 0);
        if held + gap + length <= LONGEST_NAME {
            if gap > 0 {
                name.push(' ');
            }
            name.push_str(word);
            held += gap + length;
            continue;
        }
        if held == 0 {
            name.extend(word.chars().take(LONGEST_NAME));
        }
        name.push('\u{2026}');
        break;
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chat_is_named_by_its_first_words_their_spaces_run_together() {
        assert_eq!(of("ev chargers near me"), "ev chargers near me");
        assert_eq!(of("  ev \n chargers\tnear  me "), "ev chargers near me");
        assert_eq!(of("example.com"), "example.com");
        assert_eq!(of(""), "");
        assert_eq!(of(" \n\t"), "", "only white space: New chat");
        // Markup is a person's characters, kept as they are.
        assert_eq!(of("<b>bold</b> x"), "<b>bold</b> x");
    }

    #[test]
    fn as_many_whole_words_as_fit_and_an_ellipsis_for_the_rest() {
        // 48 exactly: kept whole, no ellipsis.
        let exactly = "aaaaaaaaaa bbbbbbbbbb cccccccccc dddddddddd eeee";
        assert_eq!(exactly.chars().count(), LONGEST_NAME);
        assert_eq!(of(exactly), exactly);
        // One more word: it is left out, and said so.
        assert_eq!(of(&format!("{exactly} f")), format!("{exactly}\u{2026}"));
        // A word that would cross the end is left out whole.
        assert_eq!(
            of("aaaaaaaaaa bbbbbbbbbb cccccccccc dddddddddd eeeeeeeeee"),
            "aaaaaaaaaa bbbbbbbbbb cccccccccc dddddddddd\u{2026}"
        );
        // Counted in characters, not bytes.
        let wide = "é".repeat(LONGEST_NAME);
        assert_eq!(of(&format!("{wide} more")), format!("{wide}\u{2026}"));
    }

    #[test]
    fn one_word_too_long_on_its_own_is_cut_at_a_character() {
        let long = "x".repeat(LONGEST_NAME + 20);
        assert_eq!(of(&long), format!("{}\u{2026}", "x".repeat(LONGEST_NAME)));
        let address = format!("https://example.com/{}", "p/".repeat(40));
        let named = of(&address);
        assert_eq!(named.chars().count(), LONGEST_NAME + 1);
        assert!(named.starts_with("https://example.com/p/p/"), "{named}");
    }
}
