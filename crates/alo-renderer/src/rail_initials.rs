/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The letters the rail draws for a chat (queue items 760 and 860).
//!
//! The first letter or digit of each of the first two words of the chat's
//! name as the panel shows it, as capitals — *Weekend in Lisbon* `WI`,
//! *rust* `R`, *New chat* `NC` — and a middle dot for a name with no letter
//! at all.
//!
//! # A host's letter is its site's name's (queue item 860)
//!
//! A chat opened from an address is named by its host (queue item 859:
//! *en.wikipedia.org*), and a host's first letter is mostly not its
//! site's: every *www.* chat would be `W`, every Wikipedia one `E` in
//! English and `F` in French, and a name kept before 859 — a whole
//! address — `H`. So a word that is a host, or an address with a scheme,
//! gives **the first letter of the host's first label that is not a
//! prefix**, where a prefix is a label before at least two more that is
//!
//! - **`www`**, or `www` and digits (`www2`);
//! - **`m`**, **`mobile`** or **`amp`**: a site's pages for phones;
//! - **two letters**, or two letters, a hyphen and two to four more
//!   (`en`, `fr`, `zh-hans`, `pt-br`): a language or a country.
//!
//! *en.wikipedia.org* is `W`, *www.bbc.com* `B`, *en.m.wikipedia.org* `W`,
//! *bbc.co.uk* `B`. A host of two labels keeps its first (*go.dev* `G`), so
//! nothing is skipped down to a bare suffix, and a first label of anything
//! else is the site's own (*news.bbc.co.uk* `N`). **Its limit:** a site
//! whose own name is two letters before a domain of two labels — *my.*,
//! *hp.* — is read past (`my.site.example` is `S`). The letters are drawn
//! for a person only and hidden from an agent, which reads the name.

/// The rail's letters for a chat whose name, as the panel shows it, is
/// `label`.
pub fn of(label: &str) -> String {
    let letters: String = label
        .split_whitespace()
        .filter_map(letter)
        .take(2)
        .flat_map(char::to_uppercase)
        .collect();
    if letters.is_empty() {
        "\u{b7}".to_owned()
    } else {
        letters
    }
}

/// The letter `word` gives: its site's name's first, if it is a host or an
/// address, and otherwise its own first letter or digit.
fn letter(word: &str) -> Option<char> {
    let named = host(word)
        .map(site_name)
        .and_then(|name| name.chars().find(|c| c.is_alphanumeric()));
    named.or_else(|| word.chars().find(|c| c.is_alphanumeric()))
}

/// The host `word` names: what follows its scheme, if it has one, up to
/// the path, query, fragment or port, or the word itself — each only if it
/// is a host of names (not an IP address).
fn host(word: &str) -> Option<&str> {
    // A name cut with `…` may end in the middle of its host.
    let word = word.trim_end_matches('\u{2026}');
    let after = word.split_once("://").map_or(word, |(_, rest)| rest);
    let authority = after.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    let host = host.split(':').next()?;
    is_named_host(host).then_some(host)
}

/// Whether `host` is labels of letters, digits and hyphens, at least two,
/// the last beginning with a letter.
fn is_named_host(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty() && label.chars().all(|c| c.is_alphanumeric() || c == '-')
        })
        && labels
            .last()
            .and_then(|last| last.chars().next())
            .is_some_and(char::is_alphabetic)
}

/// The first label of `host` that is not a prefix, a prefix being skipped
/// only while at least two labels stand after it.
fn site_name(host: &str) -> &str {
    let labels: Vec<&str> = host.split('.').collect();
    let mut first = 0;
    while labels.len().saturating_sub(first) > 2 && labels.get(first).is_some_and(|l| prefix(l)) {
        first += 1;
    }
    labels.get(first).copied().unwrap_or(host)
}

/// Whether `label` is a prefix a site's name stands after.
fn prefix(label: &str) -> bool {
    let label = label.to_ascii_lowercase();
    let two_letters = |part: &str| part.len() == 2 && part.chars().all(|c| c.is_ascii_lowercase());
    let www = label
        .strip_prefix("www")
        .is_some_and(|rest| rest.chars().all(|c| c.is_ascii_digit()));
    let language = match label.split_once('-') {
        None => two_letters(&label),
        Some((language, region)) => {
            two_letters(language)
                && (2..=4).contains(&region.len())
                && region.chars().all(|c| c.is_ascii_lowercase())
        }
    };
    www || language || matches!(label.as_str(), "m" | "mobile" | "amp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_names_letters_are_its_first_two_words_first_letters() {
        assert_eq!(of("Weekend in Lisbon"), "WI");
        assert_eq!(of("rust"), "R");
        assert_eq!(of("  “quoted” 3 cats"), "Q3");
        assert_eq!(of("éclair recipes"), "ÉR");
        assert_eq!(of("— …"), "\u{b7}");
        // A letter whose capital is two letters is both.
        assert_eq!(of("ßeta"), "SS");
        // Not hosts: their own first letters.
        assert_eq!(of("e.g. 3.14"), "E3");
        assert_eq!(of("127.0.0.1:8736"), "1");
    }

    #[test]
    fn a_hosts_letter_is_its_sites_name_past_www_phone_and_language_prefixes() {
        assert_eq!(of("en.wikipedia.org"), "W");
        assert_eq!(of("fr.wikipedia.org"), "W");
        assert_eq!(of("en.m.wikipedia.org"), "W");
        assert_eq!(of("zh-hans.example.org"), "E");
        assert_eq!(of("www.bbc.com"), "B");
        assert_eq!(of("www2.example.com"), "E");
        assert_eq!(of("m.youtube.com"), "Y");
        assert_eq!(of("WWW.BBC.CO.UK"), "B");
        // Two labels keep the first; a label of the site's own is kept.
        assert_eq!(of("wikipedia.org"), "W");
        assert_eq!(of("go.dev"), "G");
        assert_eq!(of("en.org"), "E");
        assert_eq!(of("news.bbc.co.uk"), "N");
        // Its limit, as the module says.
        assert_eq!(of("my.site.example"), "S");
        // Two hosts: two letters, so two chats of two sites differ.
        assert_eq!(of("en.wikipedia.org www.bbc.com"), "WB");
        assert_eq!(of("compare www.bbc.com and bbc.co.uk"), "CB");
    }

    #[test]
    fn a_whole_address_kept_before_859_gives_its_sites_letter_not_h() {
        assert_eq!(of("https://www.theguardian.com/international"), "T");
        assert_eq!(of("https://en.wikipedia.orghttp\u{2026}"), "W");
        assert_eq!(of("http://user@www.example.com:8080/a"), "E");
        assert_eq!(of("https://127.0.0.1:8736/shop.html"), "H");
    }
}
