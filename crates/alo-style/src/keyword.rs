//! The four words that mean "not a value".
//!
//! `inherit`, `initial`, `unset` and `revert` can be written for any property,
//! and none of them is a value — each says what to do instead of taking one.
//! They are handled here, in one place, because handling them per property is
//! how three of the four end up subtly different from each other.

use core::fmt;

/// A value that is an instruction rather than a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WideKeyword {
    /// Take the parent's value, whether or not the property inherits.
    Inherit,
    /// Take the property's initial value.
    Initial,
    /// `inherit` for a property that inherits, `initial` for one that does not.
    Unset,
    /// Roll back to what the previous origin said, and `unset` if no earlier
    /// origin said anything.
    Revert,
}

impl WideKeyword {
    /// The keyword a value is, if it is one.
    ///
    /// A value is only a wide keyword when it is the *whole* value: `initial`
    /// is one, and `1px initial` is an ordinary value that happens to contain
    /// the word.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        for (keyword, name) in [
            (WideKeyword::Inherit, "inherit"),
            (WideKeyword::Initial, "initial"),
            (WideKeyword::Unset, "unset"),
            (WideKeyword::Revert, "revert"),
        ] {
            if value.eq_ignore_ascii_case(name) {
                return Some(keyword);
            }
        }
        None
    }

    /// The keyword, as written.
    pub fn as_str(self) -> &'static str {
        match self {
            WideKeyword::Inherit => "inherit",
            WideKeyword::Initial => "initial",
            WideKeyword::Unset => "unset",
            WideKeyword::Revert => "revert",
        }
    }

    /// What this keyword means for a property that does or does not inherit.
    ///
    /// `revert` is not resolved here: it depends on the other origins, which
    /// is the cascade's business rather than the keyword's.
    pub fn resolve(self, property_inherits: bool) -> Resolution {
        match self {
            WideKeyword::Inherit => Resolution::FromParent,
            WideKeyword::Initial => Resolution::Initial,
            WideKeyword::Unset => {
                if property_inherits {
                    Resolution::FromParent
                } else {
                    Resolution::Initial
                }
            }
            WideKeyword::Revert => Resolution::PreviousOrigin,
        }
    }
}

impl fmt::Display for WideKeyword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a wide keyword asks the cascade to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// Take whatever the parent element ended up with.
    FromParent,
    /// Take the property's initial value.
    ///
    /// In this engine a property's initial value is its **absence**: a
    /// computed style holds only what was actually set, and whoever reads it
    /// knows the initial value for the property they are asking about. That
    /// keeps the engine from carrying a table of initial values it would have
    /// to keep right, and it makes "nobody set this" and "somebody set it to
    /// its initial value" the same thing, which is what CSS says they are.
    Initial,
    /// Take what the previous origin said, and `unset` if there is none.
    PreviousOrigin,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_keyword_is_recognised_however_it_is_written() {
        assert_eq!(WideKeyword::parse("inherit"), Some(WideKeyword::Inherit));
        assert_eq!(WideKeyword::parse("  INITIAL "), Some(WideKeyword::Initial));
        assert_eq!(WideKeyword::parse("Unset"), Some(WideKeyword::Unset));
        assert_eq!(WideKeyword::parse("revert"), Some(WideKeyword::Revert));
    }

    #[test]
    fn a_value_that_merely_contains_the_word_is_not_a_keyword() {
        assert_eq!(WideKeyword::parse("1px initial"), None);
        assert_eq!(WideKeyword::parse("inherited"), None);
        assert_eq!(WideKeyword::parse("var(--inherit)"), None);
        assert_eq!(WideKeyword::parse(""), None);
    }

    #[test]
    fn unset_is_the_one_that_depends_on_the_property() {
        assert_eq!(
            WideKeyword::Unset.resolve(true),
            Resolution::FromParent,
            "an inherited property unsets to its parent's value",
        );
        assert_eq!(WideKeyword::Unset.resolve(false), Resolution::Initial);
    }

    #[test]
    fn inherit_and_initial_do_not_depend_on_the_property() {
        for inherits in [true, false] {
            assert_eq!(
                WideKeyword::Inherit.resolve(inherits),
                Resolution::FromParent,
            );
            assert_eq!(WideKeyword::Initial.resolve(inherits), Resolution::Initial);
            assert_eq!(
                WideKeyword::Revert.resolve(inherits),
                Resolution::PreviousOrigin,
            );
        }
    }

    #[test]
    fn a_keyword_writes_itself_back_out() {
        for keyword in [
            WideKeyword::Inherit,
            WideKeyword::Initial,
            WideKeyword::Unset,
            WideKeyword::Revert,
        ] {
            assert_eq!(WideKeyword::parse(keyword.as_str()), Some(keyword));
            assert_eq!(keyword.to_string(), keyword.as_str());
        }
    }
}
