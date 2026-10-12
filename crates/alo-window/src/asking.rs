/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What an agent outside this browser may ask, and what it can be answered
//! (ADR 0055).
//!
//! Typed, and holding no JSON: [`crate::protocol`] is the one file that reads
//! and writes the bytes, and everything here is what those bytes mean. The
//! contract a caller builds against is `docs/contracts/agent-socket.md`.
//!
//! # Every answer is a kind
//!
//! ADR 0055 § 4. A refusal crosses as a tag and its own fields, never as a
//! sentence a caller has to parse: `verb.rs`'s twelve as they are, and the
//! ones only this interface can meet — a line too long, a tab nobody has
//! open — as [`Refused`]'s own. The English goes beside the kind, for a
//! person reading, and a caller is never asked to read it.
//!
//! # Every bound is ours
//!
//! ADR 0055 § 3, and ADR 0013's rule that *a limit somebody else chooses is
//! not a limit*. A line's length, a name's, an address's, how deeply a
//! request may nest and how deeply a tree may be answered are all here, and
//! each is refused rather than truncated.

use alo_agent::{Outcome, Refusal, Target, Verb};
use alo_renderer::Snapshot;
use core::fmt;

/// The version of the protocol this browser speaks. A request names the one
/// it was written for, and is refused if this browser does not speak it.
pub const VERSION: u64 = 1;

/// The longest line a caller may send, in bytes, its newline included.
pub const MOST_LINE: usize = 64 * 1024;

/// The deepest a request may nest. The deepest a valid one does is two — a
/// target inside the request — so this is room, not a guess.
pub const MOST_NESTING: usize = 4;

/// The longest name a target may carry, and the longest echo a caller may
/// ask to have handed back, in characters.
pub const MOST_NAME: usize = 1024;

/// The longest address a tab may be named by, in characters.
pub const MOST_ADDRESS: usize = 8 * 1024;

/// The deepest tree answered. A page deeper than this is refused rather than
/// cut off: an agent handed half a tree would act on what it believes is the
/// whole of it.
pub const MOST_DEPTH: usize = 256;

/// How many agents may be connected at once.
pub const MOST_CALLERS: usize = 8;

/// What a caller asked to have handed back with its answer, so it can tell
/// its answers apart. Never read by this browser, and never recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Echo {
    /// A whole number.
    Number(u64),
    /// Some text, at most [`MOST_NAME`] characters.
    Text(String),
}

/// One request, as a caller made it.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// What to hand back with the answer, if anything.
    pub echo: Option<Echo>,
    /// What is asked — or why what was sent is not a request.
    pub ask: Result<Ask, Refused>,
}

/// What an agent may ask.
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    /// The address of every open tab, in the order they were opened.
    Tabs,
    /// The tree of the page in the tab at `tab`.
    Read {
        /// The tab's address, as the caller wrote it.
        tab: String,
    },
    /// A verb on something in the page in the tab at `tab`, named rather
    /// than pointed at (ADR 0002).
    Act {
        /// The tab's address, as the caller wrote it.
        tab: String,
        /// What to act on.
        target: Target,
        /// What to do.
        verb: Verb,
    },
}

impl Ask {
    /// The tab it names, if it names one.
    pub fn tab(&self) -> Option<&str> {
        match self {
            Ask::Tabs => None,
            Ask::Read { tab } | Ask::Act { tab, .. } => Some(tab),
        }
    }
}

/// What a caller is answered.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// The address of every open tab.
    Tabs(Vec<String>),
    /// The tree, as it was when it was read.
    Tree {
        /// The tab, as the caller named it.
        tab: String,
        /// The tree.
        snapshot: Box<Snapshot>,
    },
    /// A verb ran, and this is what it did.
    Done {
        /// The tab, as the caller named it.
        tab: String,
        /// What it did, as the page left it.
        outcome: Outcome,
        /// What the page's script said while it ran, as the renderer said it
        /// (`FromRenderer::Acted`).
        issues: Vec<String>,
    },
    /// It was refused, and this is why.
    Refused(Refused),
}

/// Why a request was refused, as a kind with its own fields.
///
/// Refusing is a result (ADR 0002), here as in `verb.rs`.
#[derive(Debug, Clone, PartialEq)]
pub enum Refused {
    /// The line was longer than [`MOST_LINE`]. The connection is closed
    /// after it, because where the next line begins cannot be found without
    /// reading as much as the caller cares to send.
    LineTooLong {
        /// The longest line taken.
        most: usize,
    },
    /// The line is not JSON.
    NotJson,
    /// The line nests deeper than [`MOST_NESTING`].
    TooDeep {
        /// The deepest a line may nest.
        most: usize,
    },
    /// The line is JSON, and not an object.
    NotAnObject,
    /// The request names a version this browser does not speak, or none.
    UnknownVersion {
        /// The version it speaks.
        speaks: u64,
    },
    /// A field the request needs is not there.
    MissingField {
        /// Which.
        field: &'static str,
    },
    /// A field this version does not have. Refused rather than ignored, so
    /// that a misspelt field is said rather than silently doing nothing.
    UnknownField {
        /// Which, as the caller wrote it, at most [`MOST_NAME`] characters.
        field: String,
    },
    /// A field holds what it cannot: a number where text goes, an ask or a
    /// verb this version does not have.
    BadField {
        /// Which.
        field: &'static str,
    },
    /// A field holds more than it may.
    FieldTooLong {
        /// Which.
        field: &'static str,
        /// The most it may hold, in characters.
        most: usize,
    },
    /// The tab is named by something that is not an address.
    NotAnAddress {
        /// What it was named by.
        tab: String,
    },
    /// No open tab is at that address.
    NoSuchTab {
        /// The address asked for.
        tab: String,
    },
    /// More than one tab is at that address (ADR 0055 § 5), and one is not
    /// picked: acting in the wrong tab is worse than acting in none.
    Ambiguous {
        /// The address asked for.
        tab: String,
        /// How many tabs are there.
        count: usize,
    },
    /// The tab is open and nothing is loaded in it.
    NothingLoaded {
        /// The tab.
        tab: String,
    },
    /// The tab's renderer is gone, or could not answer.
    TabGone {
        /// The tab.
        tab: String,
        /// What happened to it, in words, for a person.
        said: String,
    },
    /// The page's tree is deeper than [`MOST_DEPTH`].
    TreeTooDeep {
        /// The tab.
        tab: String,
        /// The deepest tree answered.
        most: usize,
    },
    /// [`MOST_CALLERS`] agents are connected already. Said once, and the
    /// connection closed.
    TooManyCallers {
        /// How many may be.
        most: usize,
    },
    /// The page refused the verb, with one of `verb.rs`'s refusals as it is.
    Verb {
        /// The tab.
        tab: String,
        /// Why.
        refusal: Refusal,
    },
}

impl Refused {
    /// Its kind, as it crosses: `verb.rs`'s own name for a refusal of the
    /// page's, and this interface's for its own.
    pub fn kind(&self) -> &'static str {
        match self {
            Refused::LineTooLong { .. } => "LineTooLong",
            Refused::NotJson => "NotJson",
            Refused::TooDeep { .. } => "TooDeep",
            Refused::NotAnObject => "NotAnObject",
            Refused::UnknownVersion { .. } => "UnknownVersion",
            Refused::MissingField { .. } => "MissingField",
            Refused::UnknownField { .. } => "UnknownField",
            Refused::BadField { .. } => "BadField",
            Refused::FieldTooLong { .. } => "FieldTooLong",
            Refused::NotAnAddress { .. } => "NotAnAddress",
            Refused::NoSuchTab { .. } => "NoSuchTab",
            Refused::Ambiguous { .. } => "Ambiguous",
            Refused::NothingLoaded { .. } => "NothingLoaded",
            Refused::TabGone { .. } => "TabGone",
            Refused::TreeTooDeep { .. } => "TreeTooDeep",
            Refused::TooManyCallers { .. } => "TooManyCallers",
            Refused::Verb { refusal, .. } => refusal_kind(refusal),
        }
    }
}

/// The kind of one of `verb.rs`'s refusals: its own name there.
pub fn refusal_kind(refusal: &Refusal) -> &'static str {
    match refusal {
        Refusal::NotFound { .. } => "NotFound",
        Refusal::Ambiguous { .. } => "Ambiguous",
        Refusal::NotOperable { .. } => "NotOperable",
        Refusal::Disabled { .. } => "Disabled",
        Refusal::NotAField { .. } => "NotAField",
        Refusal::ReadOnly { .. } => "ReadOnly",
        Refusal::NotADate { .. } => "NotADate",
        Refusal::Unbuilt { .. } => "Unbuilt",
        Refusal::DoesNotScroll { .. } => "DoesNotScroll",
        Refusal::TooLong { .. } => "TooLong",
        Refusal::Unchecked { .. } => "Unchecked",
        Refusal::NotSubmittable { .. } => "NotSubmittable",
    }
}

/// The kind of an outcome, as it crosses: its own name in `verb.rs`.
pub fn outcome_kind(outcome: &Outcome) -> &'static str {
    match outcome {
        Outcome::Activated { .. } => "Activated",
        Outcome::Followed { .. } => "Followed",
        Outcome::TextPut { .. } => "TextPut",
        Outcome::TextCanceled { .. } => "TextCanceled",
        Outcome::Scrolled { .. } => "Scrolled",
        Outcome::Submitted { .. } => "Submitted",
        Outcome::NotSubmitted { .. } => "NotSubmitted",
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refused::LineTooLong { most } => {
                write!(
                    f,
                    "a line may be at most {most} bytes, and this one was longer"
                )
            }
            Refused::NotJson => f.write_str("the line is not JSON"),
            Refused::TooDeep { most } => {
                write!(
                    f,
                    "a request may nest at most {most} deep, and this one nests deeper"
                )
            }
            Refused::NotAnObject => f.write_str("a request is a JSON object"),
            Refused::UnknownVersion { speaks } => {
                write!(
                    f,
                    "this browser speaks version {speaks} and the request names another, or none"
                )
            }
            Refused::MissingField { field } => write!(f, "the request has no {field:?}"),
            Refused::UnknownField { field } => {
                write!(f, "{field:?} is not a field of this version")
            }
            Refused::BadField { field } => {
                write!(f, "{field:?} holds something it cannot")
            }
            Refused::FieldTooLong { field, most } => {
                write!(f, "{field:?} may hold at most {most} characters")
            }
            Refused::NotAnAddress { tab } => write!(f, "{tab:?} is not an address"),
            Refused::NoSuchTab { tab } => write!(f, "no tab is open at {tab}"),
            Refused::Ambiguous { tab, count } => {
                write!(
                    f,
                    "{count} tabs are open at {tab}, and one is not picked for you"
                )
            }
            Refused::NothingLoaded { tab } => write!(f, "nothing is loaded in the tab at {tab}"),
            Refused::TabGone { tab, said } => write!(f, "the tab at {tab} cannot answer: {said}"),
            Refused::TreeTooDeep { tab, most } => write!(
                f,
                "the page at {tab} is more than {most} boxes deep, and half a tree is not answered"
            ),
            Refused::TooManyCallers { most } => {
                write!(f, "{most} agents are connected already")
            }
            Refused::Verb { refusal, .. } => write!(f, "{refusal}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_box::BoxId;

    #[test]
    fn a_refusal_of_the_pages_crosses_under_its_own_name() {
        let refused = Refused::Verb {
            tab: "https://example.com/".to_owned(),
            refusal: Refusal::Disabled {
                node: BoxId::from_index_for_tests(3),
            },
        };
        assert_eq!(refused.kind(), "Disabled");
        assert_eq!(refused.to_string(), "box#3 says it is disabled");
    }

    #[test]
    fn two_ambiguities_share_a_kind_and_not_their_fields() {
        let tabs = Refused::Ambiguous {
            tab: "https://example.com/".to_owned(),
            count: 2,
        };
        let names = Refused::Verb {
            tab: "https://example.com/".to_owned(),
            refusal: Refusal::Ambiguous {
                target: Target::Named("Save".to_owned()),
                candidates: Vec::new(),
            },
        };
        assert_eq!(tabs.kind(), names.kind());
        assert_ne!(tabs, names);
    }

    #[test]
    fn only_an_ask_about_a_page_names_a_tab() {
        assert_eq!(Ask::Tabs.tab(), None);
        assert_eq!(
            Ask::Read {
                tab: "about:blank".to_owned()
            }
            .tab(),
            Some("about:blank")
        );
    }
}
