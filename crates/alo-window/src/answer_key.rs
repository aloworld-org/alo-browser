/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `OpenAI`'s API key, as an older alo kept it (ADR 0081, ADR 0091, ADR
//! 0095 § 2; queue item 773).
//!
//! Since ADR 0106 § 4 the key `OpenAI` is asked with is
//! [`crate::service_keys`]'s to find — the person's own given in
//! *Settings* (`alo-ai-openai`), the one a release carries in its answer
//! slot, then this older item — so nothing set up before breaks. It is
//! held as a [`crate::search_key::Key`], whose `Debug` never says it, and
//! handed to the one request header that carries it. It is never in the
//! repository, a program, a log or a sentence.

/// The keychain item's service an older alo read `OpenAI`'s key from,
/// still read after a carried key ([`crate::services::Keyed::older`]).
pub const SERVICE: &str = "alo-openai";
