//! Where a declaration came from, and the order that gives it.
//!
//! CSS resolves a conflict by asking, in order: which origin, whether it is
//! `!important`, how specific the selector was, and which came last. The first
//! of those is here; it is a separate thing from specificity, and conflating
//! them is a bug that only shows up once there is more than one style sheet.

use alo_css::Importance;
use core::fmt;

/// Who wrote a declaration.
///
/// There is no user origin yet: nothing in stage 1 lets a person supply their
/// own style sheet, and inventing an origin nobody can fill is how a cascade
/// grows a branch that is never tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    /// The engine's own style sheet — what an element looks like before
    /// anybody says otherwise.
    UserAgent,
    /// The document's style sheets.
    Author,
}

impl Origin {
    /// The name, for a diagnostic.
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::UserAgent => "user agent",
            Origin::Author => "author",
        }
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How much an origin and an importance weigh together, as one comparable
/// number.
///
/// The order is the specification's, and the reason `!important` reverses the
/// origins is worth remembering: an important user-agent rule exists so that
/// an engine can insist on something a page must not override, and a page that
/// could shout it down would make that guarantee worthless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CascadeLevel(u8);

impl CascadeLevel {
    /// The level a declaration sits at.
    pub fn of(origin: Origin, importance: Importance) -> Self {
        Self(match (origin, importance) {
            (Origin::UserAgent, Importance::Normal) => 0,
            (Origin::Author, Importance::Normal) => 1,
            (Origin::Author, Importance::Important) => 2,
            (Origin::UserAgent, Importance::Important) => 3,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(origin: Origin, importance: Importance) -> CascadeLevel {
        CascadeLevel::of(origin, importance)
    }

    #[test]
    fn an_author_rule_beats_the_engines_own() {
        assert!(
            level(Origin::Author, Importance::Normal)
                > level(Origin::UserAgent, Importance::Normal),
        );
    }

    #[test]
    fn important_reverses_the_origins() {
        assert!(
            level(Origin::UserAgent, Importance::Important)
                > level(Origin::Author, Importance::Important),
            "an engine that could be shouted down could not insist on anything",
        );
        assert!(
            level(Origin::Author, Importance::Important)
                > level(Origin::Author, Importance::Normal),
        );
    }

    #[test]
    fn the_whole_order_is_the_one_the_specification_gives() {
        let mut levels = [
            level(Origin::Author, Importance::Important),
            level(Origin::UserAgent, Importance::Normal),
            level(Origin::UserAgent, Importance::Important),
            level(Origin::Author, Importance::Normal),
        ];
        levels.sort_unstable();
        assert_eq!(
            levels,
            [
                level(Origin::UserAgent, Importance::Normal),
                level(Origin::Author, Importance::Normal),
                level(Origin::Author, Importance::Important),
                level(Origin::UserAgent, Importance::Important),
            ],
        );
    }

    #[test]
    fn an_origin_says_which_it_is() {
        assert_eq!(Origin::Author.to_string(), "author");
        assert_eq!(Origin::UserAgent.to_string(), "user agent");
    }
}
