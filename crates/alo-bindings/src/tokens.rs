/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A `DOMTokenList`'s token set, as the DOM standard computes it (queue
//! item 327).
//!
//! An attribute's value is read as an **ordered set** — split on ASCII
//! whitespace, each token kept the first time it appears — and written back
//! as the tokens joined by one space. A token a page hands `add`, `remove`
//! or `toggle` is **validated** first: an empty one is a `SyntaxError`, and
//! one with ASCII whitespace in it an `InvalidCharacterError`, since neither
//! could ever be read back out of the attribute as itself.
//!
//! No list here is held: the set is computed from the attribute each time it
//! is asked for, which is what the standard's *attribute change steps* keep
//! a held set equal to. A page that writes `class` with `setAttribute` sees
//! the change in `classList` at once, with nothing to keep in step.
//!
//! Nothing here touches a heap or a document; [`crate::token_list`] holds
//! the object, and [`crate::interface::dom_token_list`] its members.

/// Infra's ASCII whitespace: tab, line feed, form feed, carriage return and
/// space — and not every character Unicode calls a space.
pub(crate) const fn is_ascii_whitespace(character: char) -> bool {
    matches!(character, '\t' | '\n' | '\u{c}' | '\r' | ' ')
}

/// The ordered set parser: `value`'s tokens, each kept once, in the order of
/// its first appearance.
pub(crate) fn parse(value: &str) -> Vec<&str> {
    let mut set: Vec<&str> = Vec::new();
    for token in value.split(is_ascii_whitespace) {
        if !token.is_empty() && !set.contains(&token) {
            set.push(token);
        }
    }
    set
}

/// The ordered set serializer: the tokens joined by one space.
pub(crate) fn serialize(set: &[&str]) -> String {
    set.join(" ")
}

/// Why a token was refused, as the `DOMException` it is thrown as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Invalid {
    /// The empty string: `SyntaxError`.
    Empty,
    /// A token with ASCII whitespace in it: `InvalidCharacterError`.
    Whitespace,
}

impl Invalid {
    /// The exception's name, as the standard spells it.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Empty => "SyntaxError",
            Self::Whitespace => "InvalidCharacterError",
        }
    }

    /// Which rule refused, in words.
    pub(crate) const fn message(self) -> &'static str {
        match self {
            Self::Empty => "a token must not be the empty string",
            Self::Whitespace => "a token must not contain ASCII whitespace",
        }
    }
}

/// The DOM standard's token validation, of every token in order: the first
/// refusal, or nothing.
pub(crate) fn validate<'a>(tokens: impl IntoIterator<Item = &'a str>) -> Result<(), Invalid> {
    for token in tokens {
        if token.is_empty() {
            return Err(Invalid::Empty);
        }
        if token.contains(is_ascii_whitespace) {
            return Err(Invalid::Whitespace);
        }
    }
    Ok(())
}

/// What `toggle` did: whether the token is in the set after it, and the set
/// to write back, when one is to be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Toggled {
    /// What `toggle` answers.
    pub present: bool,
    /// The serialized set to run the update steps with, or [`None`] when the
    /// standard runs no update at all.
    pub write: Option<String>,
}

/// `add`'s set: `tokens` appended to `value`'s, each only if absent.
pub(crate) fn added(value: &str, tokens: &[String]) -> String {
    let mut set = parse(value);
    for token in tokens {
        if !set.contains(&token.as_str()) {
            set.push(token);
        }
    }
    serialize(&set)
}

/// `remove`'s set: `value`'s without any of `tokens`.
pub(crate) fn removed(value: &str, tokens: &[String]) -> String {
    let mut set = parse(value);
    set.retain(|held| !tokens.iter().any(|token| token == held));
    serialize(&set)
}

/// `toggle(token, force)`, from step 3 on: the token removed when present
/// and `force` is not `true`, added when absent and `force` is not `false`,
/// and otherwise the set left alone **with no update run** — which is why
/// `toggle("a", true)` on an element whose `class` is `"a  a"` leaves the
/// attribute exactly as it was.
pub(crate) fn toggled(value: &str, token: &str, force: Option<bool>) -> Toggled {
    let mut set = parse(value);
    if set.contains(&token) {
        if force == Some(true) {
            return Toggled {
                present: true,
                write: None,
            };
        }
        set.retain(|held| *held != token);
        return Toggled {
            present: false,
            write: Some(serialize(&set)),
        };
    }
    if force == Some(false) {
        return Toggled {
            present: false,
            write: None,
        };
    }
    set.push(token);
    Toggled {
        present: true,
        write: Some(serialize(&set)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ordered_set_parser_splits_on_ascii_whitespace_and_keeps_the_first() {
        assert_eq!(parse(""), Vec::<&str>::new());
        assert_eq!(parse(" \t\n\u{c}\r "), Vec::<&str>::new());
        assert_eq!(parse("b a\tb\n\nc a"), ["b", "a", "c"]);
        // U+00A0 and U+3000 are not ASCII whitespace: each is part of a token.
        assert_eq!(parse("a\u{a0}b c\u{3000}d"), ["a\u{a0}b", "c\u{3000}d"]);
        // Case matters: a token set is not an HTML name.
        assert_eq!(parse("A a"), ["A", "a"]);
    }

    #[test]
    fn the_serializer_joins_with_one_space() {
        assert_eq!(serialize(&parse("  a   b  a ")), "a b");
        assert_eq!(serialize(&[]), "");
    }

    #[test]
    fn validation_refuses_the_empty_token_first_and_whitespace_second() {
        assert_eq!(validate(["a", "b"]), Ok(()));
        assert_eq!(validate(["a", ""]), Err(Invalid::Empty));
        assert_eq!(validate(["a b"]), Err(Invalid::Whitespace));
        assert_eq!(validate(["\u{c}"]), Err(Invalid::Whitespace));
        // The first token refused decides: an empty one after a spaced one
        // is the spaced one's error.
        assert_eq!(validate(["a b", ""]), Err(Invalid::Whitespace));
        assert_eq!(validate(["a\u{a0}b"]), Ok(()));
        assert_eq!(Invalid::Empty.name(), "SyntaxError");
        assert_eq!(Invalid::Whitespace.name(), "InvalidCharacterError");
    }

    #[test]
    fn add_and_remove_rewrite_the_set_in_its_order() {
        let tokens = |list: &[&str]| list.iter().map(|t| (*t).to_owned()).collect::<Vec<_>>();
        assert_eq!(added("card  card", &tokens(&["rec"])), "card rec");
        assert_eq!(added("a b", &tokens(&["b", "c", "c"])), "a b c");
        assert_eq!(added("", &tokens(&[])), "");
        assert_eq!(removed("a b a c", &tokens(&["a", "z"])), "b c");
        assert_eq!(removed("a", &tokens(&["a"])), "");
    }

    #[test]
    fn toggle_writes_only_when_the_standard_runs_its_update() {
        assert_eq!(
            toggled("a  b", "a", None),
            Toggled {
                present: false,
                write: Some("b".to_owned()),
            }
        );
        assert_eq!(
            toggled("a  b", "c", None),
            Toggled {
                present: true,
                write: Some("a b c".to_owned()),
            }
        );
        assert_eq!(
            toggled("a  a", "a", Some(true)),
            Toggled {
                present: true,
                write: None,
            }
        );
        assert_eq!(
            toggled("a", "b", Some(false)),
            Toggled {
                present: false,
                write: None,
            }
        );
        assert_eq!(
            toggled("a b", "a", Some(false)),
            Toggled {
                present: false,
                write: Some("b".to_owned()),
            }
        );
        assert_eq!(
            toggled("b", "a", Some(true)),
            Toggled {
                present: true,
                write: Some("b a".to_owned()),
            }
        );
    }
}
