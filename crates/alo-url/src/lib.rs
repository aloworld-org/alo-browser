/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

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
//! **The public suffix list is `psl`'s**, and [`site`] is the only file that
//! names it. It is data rather than an algorithm — which names anybody may
//! register under, decided by registries and not derivable from any rule of
//! syntax — and it is what turns a host into the *site* that cookies, the cache
//! and a renderer process are each divided by. It is a **snapshot**, so it
//! ages; [`snapshot`] is what says how old it is and complains when the answer
//! stops being one anybody should decide a boundary with.
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
pub mod site;
pub mod snapshot;

pub use origin::{Opaque, Origin};
pub use parse::{ParseError, join, parse};
pub use parts::{Host, Url};
