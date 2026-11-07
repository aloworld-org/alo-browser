/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The one key that chooses an option of a closed list on a card's page
//! without its menu (queue item 810).
//!
//! On a card the menu of a closed list is never opened for an agent: it
//! would be a window of Chromium's at the edge of a display, away from the
//! card, taking the person's keys while nobody sees what it is for — what
//! the owner's design rules out (*nothing invisible captures keys*). A
//! focused closed list still takes keys without its menu. Measured through
//! alo's CEF host (CEF 156.0.14, macOS 26.5) on a list of *Apples,
//! Apricots, Bananas* (held), *Cherries, Dates, Elderberries*: →, ←, Home,
//! End and Page Down changed nothing and opened no menu; the letter `d`
//! chose *Dates* — one `input`, one `change`; `a` chose *Apples*, and `a`
//! again *Apricots*: Chromium's type-ahead, which goes to the next option
//! whose words begin with the letter, round from the one held.
//!
//! So one key chooses an option **only when it is the one option whose
//! words begin with its first letter**: then the page hears one `input`
//! and one `change`, for it alone, wherever the list stands. An option
//! another one shares a first letter with would be reached through options
//! nobody chose, each heard as a choice (ADR 0064 § 6): it is not chosen
//! by a key at all, and the agent is told so — the person chooses it,
//! pressing the list, which opens Chromium's own menu with the card
//! expanded. A disabled option is never chosen.

/// An option of a closed list, as the one key is decided for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// Its words, as Chrome's tree names it.
    pub name: String,
    /// Whether a person can choose it.
    pub enabled: bool,
}

/// The letter that chooses `options[target]` alone, typed once into the
/// focused list — or [`None`] when no one key does.
pub fn one_key(options: &[Listed], target: usize) -> Option<char> {
    let first = |option: &Listed| {
        option
            .name
            .trim_start()
            .chars()
            .next()
            .map(|letter| letter.to_lowercase().collect::<String>())
    };
    let chosen = options.get(target).filter(|option| option.enabled)?;
    let letter = first(chosen)?;
    let sharing = options
        .iter()
        .filter(|option| first(option).as_deref() == Some(letter.as_str()))
        .count();
    let mut single = letter.chars();
    match (sharing, single.next(), single.next()) {
        (1, Some(key), None) if key.is_alphanumeric() => Some(key),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fruit() -> Vec<Listed> {
        [
            "Apples",
            "Apricots",
            "Bananas",
            "Cherries",
            " dates",
            "Elderberries",
        ]
        .into_iter()
        .map(|name| Listed {
            name: name.to_owned(),
            enabled: true,
        })
        .collect()
    }

    #[test]
    fn an_option_alone_in_its_first_letter_is_chosen_by_it() {
        assert_eq!(one_key(&fruit(), 4), Some('d'), "Dates, as measured");
        assert_eq!(one_key(&fruit(), 3), Some('c'));
        assert_eq!(one_key(&fruit(), 5), Some('e'));
    }

    #[test]
    fn one_sharing_its_letter_or_disabled_or_wordless_is_chosen_by_no_key() {
        assert_eq!(one_key(&fruit(), 0), None, "Apples and Apricots");
        assert_eq!(one_key(&fruit(), 1), None);
        let mut off = fruit();
        if let Some(cherries) = off.get_mut(3) {
            cherries.enabled = false;
        }
        assert_eq!(one_key(&off, 3), None);
        let blank = vec![Listed {
            name: "  ".to_owned(),
            enabled: true,
        }];
        assert_eq!(one_key(&blank, 0), None);
        assert_eq!(one_key(&fruit(), 9), None, "no such option");
        let symbol = vec![Listed {
            name: "— none —".to_owned(),
            enabled: true,
        }];
        assert_eq!(one_key(&symbol, 0), None);
    }
}
