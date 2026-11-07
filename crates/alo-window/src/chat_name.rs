/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a chat is called, from what was said in it (queue items 758 and
//! 859, ADR 0092 § 5 and its amendment of 7 November 2026).
//!
//! A chat with no turns is *New chat* (`alo-renderer`'s `panel::NEW_CHAT`).
//! Once it holds one it is named by **the first words said in it**: the
//! first turn's words — a search's, an address typed, or a site's title
//! pressed — their white space run together, as many whole words as fit in
//! [`LONGEST_NAME`] characters, and `…` after them when any were left out.
//! One word longer than that on its own is cut at a character. Named once,
//! as the first turn is made, and never again: a later turn does not rename
//! it, and the name is kept with the chat (ADR 0092). Only the first turn
//! names it, so a name never joins the words of two turns.
//!
//! # An address reads as its host (queue item 859)
//!
//! A word that is an address — what the one box would go to
//! (`alo-renderer`'s `typed::address`) — stands in the name as **its host**
//! (`https://en.wikipedia.org/wiki/Kampala` is *en.wikipedia.org*), its
//! port kept when one is written: the scheme and the path are what make
//! every name start *https://* and every rail's initials *H*, and the path
//! is kept whole in the turn itself, a press away. The same host twice in
//! a row is said once.
//!
//! **Two addresses glued** — an `http://` or `https://` straight after a
//! letter or digit, as in `https://en.wikipedia.orghttps://en.wikipedia.org`
//! — are two words: a scheme never follows a letter inside one address,
//! while one after `/`, `=` or `?` (an address carried in another's path or
//! query) is left inside it. That is how batch 16's hand run came to a
//! chat named `https://en.wikipedia.orghttp…`: its one turn said the
//! address twice with no space between, because the one box has no ⌘A
//! (queue item 861) and the second typing went on after the first.
//!
//! # Names kept by the rule before
//!
//! A chat kept before 859 holds the name 758's rule made, which may be a
//! whole address or two glued. Read back ([`read_back`]), a name that is
//! exactly what that rule made of the first turn kept is alo's, and is
//! named again by today's rule; any other name — one a person or, one day,
//! a model gave — is kept as it is.
//!
//! No model names it: which model alo thinks with is not decided (ADR
//! 0081), and an agent's name for a chat waits on the agent's record (queue
//! item 704). A rule over the person's own words is what can be checked
//! today, and says nothing they did not say.

use alo_renderer::typed::address;

/// The most characters of a person's words a chat's name holds, before the
/// `…` that says more was said.
///
/// More than the panel's row shows — 13.5 px text in a row about 190 wide
/// holds some thirty — which the panel's own sheet cuts where the row ends;
/// so *Search chats* finds a chat by more of its words than the row draws.
pub const LONGEST_NAME: usize = 48;

/// What a chat whose first turn said `said` is called: its first words,
/// each address as its host, or nothing — *New chat* — when it said only
/// white space.
pub fn of(said: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    let mut last_was_host = false;
    for word in said.split_whitespace().flat_map(unglued) {
        if let Some(url) = address(word) {
            let host = alo_url::reading::host(&url);
            if !(last_was_host && words.last() == Some(&host)) {
                words.push(host);
            }
            last_was_host = true;
        } else {
            words.push(word.to_owned());
            last_was_host = false;
        }
    }
    fitted(words.iter().map(String::as_str))
}

/// The name a chat kept as `stored`, whose first turn kept said
/// `first_said`, is read back with: named again by [`of`] when `stored` is
/// exactly what 758's rule made of that turn — alo's own name — and kept as
/// it is otherwise.
pub fn read_back(stored: &str, first_said: Option<&str>) -> String {
    match first_said {
        Some(said) if stored == before_859(said) => of(said),
        _ => stored.to_owned(),
    }
}

/// What 758's rule named a chat whose first turn said `said`: its words as
/// they were, addresses whole and glued ones glued.
fn before_859(said: &str) -> String {
    fitted(said.split_whitespace())
}

/// `words`, a space between each, as many whole as fit in [`LONGEST_NAME`]
/// characters and `…` when any were left out; a first word longer than
/// that on its own cut at a character.
fn fitted<'a>(words: impl Iterator<Item = &'a str>) -> String {
    let mut name = String::new();
    let mut held = 0;
    for word in words {
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

/// `word` cut before each `http://` or `https://` that follows a letter or
/// digit: two addresses typed with nothing between them.
fn unglued(word: &str) -> impl Iterator<Item = &str> {
    // ASCII lowering keeps every byte where it was.
    let lowered = word.to_ascii_lowercase();
    let mut cuts: Vec<usize> = ["http://", "https://"]
        .iter()
        .flat_map(|scheme| lowered.match_indices(scheme).map(|(at, _)| at))
        .filter(|&at| {
            word.get(..at)
                .and_then(|before| before.chars().next_back())
                .is_some_and(char::is_alphanumeric)
        })
        .collect();
    cuts.sort_unstable();
    let mut pieces = Vec::with_capacity(cuts.len() + 1);
    let mut from = 0;
    for at in cuts {
        if let Some(piece) = word.get(from..at) {
            pieces.push(piece);
        }
        from = at;
    }
    if let Some(rest) = word.get(from..) {
        pieces.push(rest);
    }
    pieces.into_iter()
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
        // A long host is a long word.
        let host = format!("{}.example", "a".repeat(LONGEST_NAME));
        let named = of(&format!("https://{host}/p"));
        assert_eq!(named.chars().count(), LONGEST_NAME + 1);
        assert!(named.starts_with("aaaa"), "{named}");
    }

    #[test]
    fn an_address_reads_as_its_host() {
        assert_eq!(of("https://en.wikipedia.org"), "en.wikipedia.org");
        assert_eq!(
            of("https://en.wikipedia.org/wiki/Kampala"),
            "en.wikipedia.org"
        );
        assert_eq!(of("www.bbc.com/news"), "www.bbc.com");
        assert_eq!(of("HTTP://Example.COM/A?b=c#d"), "example.com");
        // A port written is kept: two local servers are two names.
        assert_eq!(of("http://127.0.0.1:8736/shop.html"), "127.0.0.1:8736");
        // Among other words, only the address changes.
        assert_eq!(
            of("compare https://www.bbc.com/news and bbc.co.uk"),
            "compare www.bbc.com and bbc.co.uk"
        );
        // Not addresses: kept as they were.
        assert_eq!(of("me@example.com"), "me@example.com");
        assert_eq!(of("e.g. 3.14"), "e.g. 3.14");
        assert_eq!(of("mailto:me@example.com"), "mailto:me@example.com");
    }

    #[test]
    fn two_addresses_glued_are_two_words_never_one() {
        // Batch 16's hand run: the one turn said the address twice.
        assert_eq!(
            of("https://en.wikipedia.orghttps://en.wikipedia.org"),
            "en.wikipedia.org",
            "the same host twice in a row is said once"
        );
        assert_eq!(
            of("https://en.wikipedia.org/wiki/Kampalahttps://www.bbc.com/news"),
            "en.wikipedia.org www.bbc.com"
        );
        assert_eq!(
            of("en.wikipedia.orgHTTP://www.bbc.com x"),
            "en.wikipedia.org www.bbc.com x"
        );
        // An address carried inside another's path or query is not cut.
        assert_eq!(
            of("https://web.archive.org/web/2020/https://example.com/"),
            "web.archive.org"
        );
        assert_eq!(of("https://a.example/?to=https://b.example/"), "a.example");
        // A host said again later, not right after, is said again.
        assert_eq!(of("a.example b a.example"), "a.example b a.example");
        // A person's word said twice is theirs, kept.
        assert_eq!(of("very very"), "very very");
    }

    #[test]
    fn no_name_ever_joins_two_addresses_without_a_space() {
        let addresses = [
            "https://en.wikipedia.org/wiki/Kampala",
            "http://www.bbc.com/news",
            "example.com",
            "https://127.0.0.1:8736/shop.html",
        ];
        for first in addresses {
            for second in addresses {
                for between in ["", " ", "\n"] {
                    // Glued with no scheme between, two hosts are one host
                    // to anybody: `example.comexample.com`.
                    if between.is_empty() && !second.contains("://") {
                        continue;
                    }
                    let named = of(&format!("{first}{between}{second}"));
                    for word in named.split(' ') {
                        let word = word.trim_end_matches('\u{2026}');
                        assert!(!word.contains("://"), "{first}|{second}: {named}");
                        assert!(
                            !word.contains(".orgh")
                                && !word.contains(".comh")
                                && !word.contains(".comw")
                                && !word.contains(".come"),
                            "{first}|{second}: {named}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_name_kept_by_the_rule_before_is_named_again_and_any_other_kept() {
        let said = "https://en.wikipedia.orghttps://en.wikipedia.org";
        // What 758's rule made of it: the words whole, cut at 48.
        let before = before_859(said);
        assert_eq!(before, "https://en.wikipedia.orghttps://en.wikipedia.org");
        assert_eq!(read_back(&before, Some(said)), "en.wikipedia.org");
        let long = "https://en.wikipedia.org/wiki/Kampala_Capital_City_Authority";
        assert_eq!(read_back(&before_859(long), Some(long)), "en.wikipedia.org");
        // A person's words named the same either way.
        assert_eq!(
            read_back("ev chargers near me", Some("ev chargers near me")),
            "ev chargers near me"
        );
        // A name that is not the rule's of the first turn is kept.
        assert_eq!(read_back("My trip", Some(said)), "My trip");
        assert_eq!(
            read_back("https://example.com", Some("something else")),
            "https://example.com"
        );
        assert_eq!(read_back("kept", None), "kept");
    }
}
