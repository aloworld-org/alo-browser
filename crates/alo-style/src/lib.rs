//! The cascade: which declaration wins, what a child inherits, and what
//! `var()` resolves to.
//!
//! `docs/decisions/0001` calls this stage 1's first hard requirement rather
//! than decoration, and the reason is specific: `alo-workplace`'s design
//! system is custom properties throughout, so an engine that cannot resolve
//! `var()` renders nothing of alo at all — not badly, nothing.
//!
//! # The order things happen in
//!
//! For each element, in document order:
//!
//! 1. **Gather.** Every declaration whose selector matched, from every sheet,
//!    with the specificity of the selector that actually matched (ADR: a rule
//!    written `h1, #title` contributes at two different specificities).
//! 2. **Cascade.** Origin, then `!important`, then specificity, then order.
//!    [`cascade`] does this and nothing else.
//! 3. **Inherit.** What the parent ended up with, for the properties that
//!    inherit — [`inheritance`] is the table, because CSS is a table.
//! 4. **Resolve.** Custom properties first, as a group, because one may use
//!    another declared beside it; then `var()` in everything else. A cycle is
//!    refused rather than looped.
//!
//! Document order is not an optimisation here. A child's `var(--surface)`
//! resolves against the map its parent ended up with, so the parent has to be
//! finished first, and any other order would compute the same element twice
//! with different answers.
//!
//! # What comes out, and what does not
//!
//! **Specified values, as text.** `16px` is still four characters. A property
//! that is absent is at its initial value — CSS says "nobody set this" and
//! "somebody set this to its initial value" are the same state, so this engine
//! carries no table of initial values to keep right.
//!
//! Turning text into numbers — lengths in pixels, colours in channels — is
//! **queue item 12**, and it belongs with the code that knows which unit each
//! property wants. Layout will ask for `width` and parse a length; paint will
//! ask for `color` and parse a colour. Neither wants a general answer, and a
//! general answer is what would have to be wrong somewhere.

pub mod cascade;
pub mod computed;
pub mod inheritance;
pub mod keyword;
pub mod metrics;
pub mod origin;
pub mod user_agent;
pub mod variables;

pub use cascade::{Applicable, Contender, SourcedSheet};
pub use computed::{ComputedStyle, StyleTree, resolve};
pub use inheritance::inherits;
pub use keyword::{Resolution, WideKeyword};
pub use metrics::{DEFAULT_FONT_SIZE, resolve_font_size, resolve_line_height};
pub use origin::{CascadeLevel, Origin};
pub use user_agent::USER_AGENT_STYLE_SHEET;
pub use variables::{Resolved, Variables, referenced_variables, substitute};
