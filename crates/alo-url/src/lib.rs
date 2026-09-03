//! URLs, and the origins every security decision is made against.
//!
//! First in stage 2, and not for tidiness. The same-origin policy, CORS,
//! cookies, CSP and the process split all ask the same question — *which origin
//! is this?* — and every one of them is wrong if the answer is. `ROADMAP.md`
//! puts it first for that reason.
//!
//! # What is rented and where
//!
//! **Parsing is `url`'s**, and [`parse`] is the only file that names it.
//! ADR 0001: a parser that implements a specification and carries none of our
//! value is prior art to take, exactly as `html5ever` and `cssparser` were.
//! WHATWG's URL Standard is a state machine with two decades of
//! interoperability in it, and IDNA — which decides whether `аpple.com` in
//! Cyrillic is the same host as `apple.com` — is a Unicode specification with a
//! table. Writing either would be spending effort on the part of a browser
//! nobody would notice us doing well.
//!
//! **The types are ours.** `url::Url` is a string with indices into it; our
//! [`Url`] is the parts, and our [`Origin`] is a value other code compares. The
//! rented crate parses; we hold what came out — the same shape as
//! `alo-dom`'s "`html5ever` parses; we hold the tree".
//!
//! # The one rule worth reading twice
//!
//! **An opaque origin is equal to itself and to nothing else.** A `data:` URL,
//! a sandboxed frame, anything with no host to speak of — each gets its own
//! identity, and two of them are never the same origin however alike they look.
//! Getting that backwards is a same-origin bypass, which is why it is a type
//! here rather than a convention.

pub mod origin;
pub mod parse;
pub mod parts;

pub use origin::{Opaque, Origin};
pub use parse::{ParseError, join, parse};
pub use parts::{Host, Url};
