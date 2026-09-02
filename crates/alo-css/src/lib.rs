//! Style sheets for the alo browser.
//!
//! `cssparser` tokenises and `selectors` matches; the rules are ours. That is
//! ADR 0001 again: both crates parse to specification and carry none of this
//! engine's value, while what a rule *is*, which selectors exist, and what is
//! refused rather than guessed at are decisions this engine has to own.
//!
//! # The boundary
//!
//! `alo-dom` keeps `html5ever` inside one file, because an HTML parser is used
//! once. A CSS tokeniser is not: selector text, media conditions and
//! declaration values are all read from the same token stream, and pretending
//! otherwise would mean copying strings about to keep a boundary that was only
//! ever cosmetic. So the boundary here is the crate's **public API**: no type
//! from `cssparser` or `selectors` appears in it, and the files that may name
//! them are listed in `scripts/gate.sh` and checked on every run.
//!
//! # What is refused, and why it is recorded
//!
//! Nothing is dropped in silence. Every refusal becomes a [`StyleIssue`] with
//! the text that caused it and the place it was written:
//!
//! - **An unknown property is kept**, value and all, and ignored. A later
//!   stage implements it without re-parsing the sheet — which is
//!   `docs/features.md`'s rule for style, applied literally.
//! - **An unknown at-rule is kept** whole, for the same reason.
//! - **An invalid selector drops its rule**, which is what CSS says to do: a
//!   rule whose selector nobody can evaluate would match everything or
//!   nothing, and both are worse than absence.
//! - **A selector naming a pseudo-element is kept and never matches.** Stage 1
//!   produces no boxes for pseudo-elements, so there is nothing for it to
//!   match, and saying so is better than appearing to work.

pub mod declaration;
pub mod ident;
pub mod issue;
pub mod matching;
pub mod media;
pub mod parse;
pub mod selector;
pub mod state;
pub mod stylesheet;

pub use declaration::{Declaration, DeclarationBlock, Importance, PropertyName};
pub use ident::Ident;
pub use issue::{IssueKind, Location, StyleIssue};
pub use matching::{MatchContext, matches};
pub use media::{ColorScheme, MediaCondition, MediaContext, MediaQueryList};
pub use parse::parse_stylesheet;
pub use selector::{PseudoClass, PseudoElement, Selector, SelectorList, Specificity};
pub use stylesheet::{MediaRule, Rule, StyleRule, Stylesheet, UnknownAtRule};
