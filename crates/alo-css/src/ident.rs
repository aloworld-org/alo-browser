/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A CSS identifier: a name or a string that appears inside a selector.
//!
//! One type serves for element names, namespace URIs, class and id names, and
//! attribute values, because the selector machinery wants a single string type
//! with a cheap hash and it is the same string in every case.
//!
//! The hash is computed once, when the identifier is made, because selector
//! matching asks for it inside its innermost loop and recomputing it there
//! would be paying for the same answer on every element of every document.

use core::borrow::Borrow;
use core::fmt;
use core::hash::{Hash, Hasher};
use precomputed_hash::PrecomputedHash;
use std::collections::hash_map::DefaultHasher;

/// A name or string in a selector, with its hash alongside it.
///
/// Equality, ordering and hashing are by text alone: the stored hash is an
/// optimisation and never part of identity.
#[derive(Clone)]
pub struct Ident {
    text: Box<str>,
    hash: u32,
}

impl Ident {
    /// The identifier, as text.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Whether this identifier is `other`, ignoring ASCII case.
    ///
    /// HTML element names, and attribute names in HTML documents, are matched
    /// this way; class names and ids are not, which is why this is a method
    /// rather than the behaviour of `==`.
    pub fn eq_ignore_ascii_case(&self, other: &str) -> bool {
        self.text.eq_ignore_ascii_case(other)
    }
}

fn hash_of(text: &str) -> u32 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    // The selector machinery wants 32 bits; the top half of a 64-bit hash is
    // as good as the bottom for this and costs nothing to take.
    #[allow(clippy::cast_possible_truncation)]
    let truncated = hasher.finish() as u32;
    truncated
}

impl From<&str> for Ident {
    fn from(text: &str) -> Self {
        Self {
            text: text.into(),
            hash: hash_of(text),
        }
    }
}

impl Default for Ident {
    /// The empty identifier. The selector machinery needs one for "no
    /// namespace", which is written as the empty string.
    fn default() -> Self {
        Self::from("")
    }
}

impl PartialEq for Ident {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for Ident {}

impl Hash for Ident {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hashing the text rather than the stored hash is what keeps the
        // `Borrow<str>` contract: an `Ident` and its `&str` hash the same.
        self.text.hash(state);
    }
}

impl AsRef<str> for Ident {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

impl Borrow<str> for Ident {
    fn borrow(&self) -> &str {
        &self.text
    }
}

impl PrecomputedHash for Ident {
    fn precomputed_hash(&self) -> u32 {
        self.hash
    }
}

impl fmt::Debug for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.text, f)
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl cssparser::ToCss for Ident {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result
    where
        W: fmt::Write,
    {
        cssparser::serialize_identifier(&self.text, dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssparser::ToCss;

    #[test]
    fn identity_is_the_text_and_nothing_else() {
        assert_eq!(Ident::from("div"), Ident::from("div"));
        assert_ne!(Ident::from("div"), Ident::from("span"));
        assert_ne!(
            Ident::from("div"),
            Ident::from("DIV"),
            "equality is case sensitive; matching decides when to fold case",
        );
        assert!(Ident::from("DIV").eq_ignore_ascii_case("div"));
    }

    #[test]
    fn the_stored_hash_agrees_with_the_text() {
        let ident = Ident::from("lead");
        assert_eq!(ident.precomputed_hash(), hash_of("lead"));
        assert_eq!(
            ident.precomputed_hash(),
            Ident::from("lead").precomputed_hash(),
            "the same text hashes the same every time",
        );
    }

    #[test]
    fn borrowing_as_a_str_hashes_the_same_as_the_str() {
        let mut from_ident = DefaultHasher::new();
        Ident::from("x").hash(&mut from_ident);
        let mut from_str = DefaultHasher::new();
        "x".hash(&mut from_str);
        assert_eq!(from_ident.finish(), from_str.finish());
        assert_eq!(Borrow::<str>::borrow(&Ident::from("x")), "x");
    }

    #[test]
    fn the_default_is_the_empty_string_which_is_how_no_namespace_is_written() {
        assert_eq!(Ident::default().as_str(), "");
    }

    #[test]
    fn writing_one_back_out_escapes_what_has_to_be_escaped() {
        assert_eq!(Ident::from("lead").to_css_string(), "lead");
        assert_eq!(Ident::from("a b").to_css_string(), "a\\ b");
    }
}
