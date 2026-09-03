/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Content Security Policy: what a page has said it is willing to load.
//!
//! Every other rule in this crate is the *browser* protecting a person from a
//! site. This one is a **site protecting itself** — usually from an injection
//! it has not found yet. `script-src 'self'` is a page saying *if a script
//! from anywhere else ever appears in me, something has gone wrong and you
//! should refuse it*, and the whole value of that sentence is that it holds on
//! the day the page is wrong about its own escaping.
//!
//! # The rule that matters more than any single directive
//!
//! **A directive this engine cannot parse makes the policy more restrictive,
//! never less.** A page that asked for a protection must not lose it to our not
//! understanding the sentence it asked in, and there are three separate ways
//! that could happen. Each is closed here rather than left to care:
//!
//! - A **source expression** we cannot read is kept and permits nothing
//!   ([`crate::csp_source::Source::Unreadable`]), rather than being dropped
//!   from its list.
//! - A **directive** with an unreadable source is kept whole. Discarding it
//!   would send its requests to `default-src`, or to nothing, and either is
//!   wider than what was written.
//! - A **directive name** we do not act on grants nothing — it is never a
//!   reason to permit a load. What it *is* is a gap, so
//!   [`Policies::not_enforced`] names every one of them: a policy that thinks
//!   it is protected and is not should be readable rather than believed.
//!
//! # A repeated directive keeps the first
//!
//! The specification says to ignore a repeat, and the security reason to follow
//! it is worth writing down: an attacker who can *append* to a header — a
//! reflected value, a misconfigured proxy — can otherwise widen a policy by
//! restating one of its directives. First wins, so appending achieves nothing.
//!
//! # What is here and what is cut
//!
//! Cut on starting, per `LOOP.md`, and each is a queue item rather than a
//! remark:
//!
//! - **Reporting** — `report-uri`, `report-to`, and sending a violation
//!   anywhere — is **queue item 188**. What is here is [`Disposition`], so that
//!   a `Content-Security-Policy-Report-Only` header can never be *enforced* by
//!   a caller that did not notice which header it came from, and
//!   [`Policies::objections`], which is the list such a report would be made
//!   from.
//! - **Computing a hash** is **queue item 189**. A hash source is read, its
//!   presence correctly disables `'unsafe-inline'`, and a refusal says in words
//!   that the policy would have allowed the content by hash — a named
//!   substitution rather than a silent block.
//!
//! # And one gap that is a design decision rather than a cut
//!
//! **A document load is not governed here**, so `frame-src` and `child-src` are
//! among the directives [`Policies::not_enforced`] names. CSP governs a *nested*
//! document and deliberately does not govern a top-level navigation — clicking
//! a link off a site with `default-src 'self'` must still work — and this engine
//! cannot yet tell one from the other: [`crate::request::Purpose::Document`]
//! with an initiator is a link click and an `<iframe>` alike. Guessing would
//! either break every link on a protected page or protect nothing, so it is
//! stated instead, and queue item 86 is where a nested document becomes a thing
//! with a name.

use crate::csp_source::Source;
use crate::headers::Headers;
use crate::request::{Purpose, Request};
use alo_url::{Origin, Url};
use core::fmt;

/// Whether a policy is enforced or only watched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// `Content-Security-Policy`: a load it forbids does not happen.
    Enforce,
    /// `Content-Security-Policy-Report-Only`: a load it forbids happens anyway,
    /// and the author is told. This is how a policy is deployed at all — a site
    /// watches for a week before it enforces — so treating one as enforced
    /// would break the sites being most careful.
    Report,
}

/// Which kind of inline content is being asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inline {
    /// A `<script>` with its code in the page.
    Script,
    /// A `<style>` element, or a `style` attribute.
    Style,
}

impl fmt::Display for Inline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Inline::Script => "inline script",
            Inline::Style => "inline style",
        })
    }
}

/// The directives this engine acts on, and everything else.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Name {
    /// `default-src`, consulted when the specific one is absent.
    Default,
    /// `script-src`.
    Script,
    /// `style-src`.
    Style,
    /// `img-src`.
    Image,
    /// `connect-src`.
    Connect,
    /// A directive this engine does not act on. Held rather than discarded, so
    /// that [`Policies::not_enforced`] can name it.
    Other,
}

impl Name {
    /// Which directive a name is.
    fn of(written: &str) -> Self {
        match written {
            "default-src" => Name::Default,
            "script-src" => Name::Script,
            "style-src" => Name::Style,
            "img-src" => Name::Image,
            "connect-src" => Name::Connect,
            _ => Name::Other,
        }
    }

    /// Which directive governs a request, when one does.
    ///
    /// [`None`] for a document, and this module's header says why that is a
    /// decision rather than an omission.
    fn governing(purpose: &Purpose) -> Option<Self> {
        match purpose {
            Purpose::Script => Some(Name::Script),
            Purpose::Style => Some(Name::Style),
            Purpose::Image => Some(Name::Image),
            Purpose::Fetch => Some(Name::Connect),
            Purpose::Document => None,
        }
    }
}

/// One directive: a name, and the list of where its content may come from.
#[derive(Debug, Clone)]
struct Directive {
    /// Which directive this is.
    name: Name,
    /// The name as the author wrote it, lowercased — what a message quotes.
    written: String,
    /// Its sources, in order, including the ones nobody could read.
    sources: Vec<Source>,
}

impl Directive {
    /// Whether a nonce on the element that asked matches one this names.
    ///
    /// Byte for byte: a nonce is a secret the page generated for this response,
    /// and folding its case would halve it.
    fn names_nonce(&self, nonce: &str) -> bool {
        self.sources
            .iter()
            .any(|source| matches!(source, Source::Nonce(value) if value == nonce))
    }

    /// Whether any source here is a nonce or a hash.
    ///
    /// The question `'unsafe-inline'` is answered by: a directive that names
    /// even one nonce or hash is a directive whose author has moved past
    /// blanket inline content, and the specification has `'unsafe-inline'`
    /// ignored in that case. Sites rely on it — the keyword is left in for
    /// browsers that do not understand nonces, and honouring it would undo the
    /// protection the nonce was added for.
    fn names_a_secret(&self) -> bool {
        self.sources
            .iter()
            .any(|source| matches!(source, Source::Nonce(_) | Source::Hash { .. }))
    }

    /// Whether `'strict-dynamic'` is in this directive.
    fn is_strict(&self) -> bool {
        self.sources
            .iter()
            .any(|source| matches!(source, Source::StrictDynamic))
    }

    /// Whether `'unsafe-inline'` is in this directive.
    fn says_unsafe_inline(&self) -> bool {
        self.sources
            .iter()
            .any(|source| matches!(source, Source::UnsafeInline))
    }

    /// Whether this permits a load, given the nonce on whatever asked.
    ///
    /// `strict` is whether `'strict-dynamic'` applies, which is true only for
    /// scripts: the keyword is defined for script directives, and applying it
    /// to `img-src 'strict-dynamic' https://cdn` would refuse pictures the
    /// author plainly allowed.
    fn permits(&self, url: &Url, page: &Origin, nonce: Option<&str>, strict: bool) -> bool {
        if nonce.is_some_and(|nonce| self.names_nonce(nonce)) {
            return true;
        }
        if strict && self.is_strict() {
            // Every host and scheme in the list is ignored, by the author's own
            // instruction. Only a nonce or a hash gets in, and the nonce has
            // already been asked about.
            return false;
        }
        self.sources.iter().any(|source| source.matches(url, page))
    }

    /// The sources, as written, for a message.
    fn as_written(&self) -> Vec<String> {
        self.sources.iter().map(ToString::to_string).collect()
    }

    /// The sources nobody here could read, as written.
    fn unreadable(&self) -> Vec<String> {
        self.sources
            .iter()
            .filter_map(|source| match source {
                Source::Unreadable(token) => Some(token.clone()),
                _ => None,
            })
            .collect()
    }
}

/// One policy: everything between two commas of one header.
///
/// Not public, and that is a rule rather than tidiness: **no caller may check
/// one policy on its own.** Checking one is how a report-only policy comes to
/// block something, and how the second of two policies comes to be forgotten.
/// [`Policies`] is the only way to ask.
#[derive(Debug, Clone)]
struct Policy {
    /// Enforced, or only watched.
    disposition: Disposition,
    /// Its directives, in order, first of each name.
    directives: Vec<Directive>,
}

impl Policy {
    /// Every policy one header value states.
    ///
    /// A comma separates two policies inside one header, which is a genuine
    /// part of the syntax rather than a curiosity: a proxy that appends its own
    /// policy to an existing header is how many of them are deployed, and each
    /// one is enforced separately.
    fn parse(value: &str, disposition: Disposition) -> Vec<Self> {
        value
            .split(',')
            .map(|one| Self::one(one, disposition))
            .filter(|policy| !policy.directives.is_empty())
            .collect()
    }

    /// One policy: directives separated by `;`, each a name and its sources.
    fn one(text: &str, disposition: Disposition) -> Self {
        let mut directives: Vec<Directive> = Vec::new();
        for piece in text.split(';') {
            let mut words = piece.split_ascii_whitespace();
            let Some(name) = words.next() else {
                continue;
            };
            let written = name.to_ascii_lowercase();
            if directives.iter().any(|held| held.written == written) {
                // The first wins. See this module's header: otherwise anybody
                // who can append to the header can widen the policy.
                continue;
            }
            directives.push(Directive {
                name: Name::of(&written),
                written,
                sources: words.map(Source::parse).collect(),
            });
        }
        Self {
            disposition,
            directives,
        }
    }

    /// Which directive decides for a name: the one asked for, else
    /// `default-src`, else none at all.
    fn deciding(&self, wanted: &Name) -> Option<&Directive> {
        self.directives
            .iter()
            .find(|held| held.name == *wanted)
            .or_else(|| {
                self.directives
                    .iter()
                    .find(|held| held.name == Name::Default)
            })
    }

    /// Whether this policy permits a request.
    ///
    /// # Errors
    ///
    /// [`Refusal::Fetch`], naming the directive that decided and what it does
    /// allow.
    fn allows(&self, request: &Request, nonce: Option<&str>) -> Result<(), Refusal> {
        // A load nobody's page asked for is not governed by anybody's policy:
        // this is the person going somewhere, and a policy is a thing a page
        // says about its own contents.
        let Some(page) = &request.initiator else {
            return Ok(());
        };
        let Some(wanted) = Name::governing(&request.purpose) else {
            return Ok(());
        };
        let strict = wanted == Name::Script;
        let Some(directive) = self.deciding(&wanted) else {
            return Ok(());
        };
        if directive.permits(&request.url, page, nonce, strict) {
            return Ok(());
        }
        Err(Refusal::Fetch {
            purpose: request.purpose.to_string(),
            url: request.url.to_string(),
            directive: directive.written.clone(),
            allows: directive.as_written(),
            unreadable: directive.unreadable(),
        })
    }

    /// Whether this policy permits inline content.
    ///
    /// # Errors
    ///
    /// [`Refusal::Inline`], saying whether a hash would have allowed it.
    fn allows_inline(&self, kind: Inline, nonce: Option<&str>) -> Result<(), Refusal> {
        let wanted = match kind {
            Inline::Script => Name::Script,
            Inline::Style => Name::Style,
        };
        let strict = matches!(kind, Inline::Script);
        let Some(directive) = self.deciding(&wanted) else {
            return Ok(());
        };
        if nonce.is_some_and(|nonce| directive.names_nonce(nonce)) {
            return Ok(());
        }
        if directive.says_unsafe_inline()
            && !directive.names_a_secret()
            && !(strict && directive.is_strict())
        {
            return Ok(());
        }
        Err(Refusal::Inline {
            kind,
            directive: directive.written.clone(),
            allows: directive.as_written(),
            only_by_hash: directive
                .sources
                .iter()
                .any(|source| matches!(source, Source::Hash { .. })),
        })
    }
}

/// Every policy a response stated, all of which have to agree.
///
/// A list rather than a merged policy, and that is the security property: two
/// policies are an **intersection**, so a second header can only ever narrow
/// what the first allowed. A site adding a policy never has to check that it
/// did not accidentally widen one.
#[derive(Debug, Clone, Default)]
pub struct Policies {
    held: Vec<Policy>,
}

impl Policies {
    /// No policy at all, which permits everything.
    pub fn none() -> Self {
        Self::default()
    }

    /// The policies a response's headers state.
    ///
    /// Both headers, because both have to be read to be told apart: a
    /// report-only policy that arrived unrecognised and was enforced would
    /// break exactly the sites deploying a policy carefully.
    pub fn stated_by(headers: &Headers) -> Self {
        let mut held = Vec::new();
        for value in headers.all("Content-Security-Policy") {
            held.extend(Policy::parse(value, Disposition::Enforce));
        }
        for value in headers.all("Content-Security-Policy-Report-Only") {
            held.extend(Policy::parse(value, Disposition::Report));
        }
        Self { held }
    }

    /// Whether every enforced policy permits this request.
    ///
    /// `nonce` is the one on the element that asked — a `<script nonce=…>`, a
    /// `<link nonce=…>`. It is a parameter rather than a field of
    /// [`Request`] because it is a property of the markup rather than of the
    /// load: nothing on the wire carries it, and a request replayed from a
    /// cache has no element behind it at all.
    ///
    /// # Errors
    ///
    /// The first [`Refusal`], which is the one to show somebody.
    pub fn allows(&self, request: &Request, nonce: Option<&str>) -> Result<(), Refusal> {
        for policy in &self.held {
            if policy.disposition == Disposition::Enforce {
                policy.allows(request, nonce)?;
            }
        }
        Ok(())
    }

    /// Whether every enforced policy permits this inline content.
    ///
    /// # Errors
    ///
    /// The first [`Refusal`].
    pub fn allows_inline(&self, kind: Inline, nonce: Option<&str>) -> Result<(), Refusal> {
        for policy in &self.held {
            if policy.disposition == Disposition::Enforce {
                policy.allows_inline(kind, nonce)?;
            }
        }
        Ok(())
    }

    /// What **every** policy objected to, the report-only ones included.
    ///
    /// This is the list a violation report would be made from, and it is here
    /// rather than in item 188 because it is what gives [`Disposition::Report`]
    /// a meaning today: a report-only policy that objects to something is
    /// visible, and it still does not block it.
    pub fn objections(&self, request: &Request, nonce: Option<&str>) -> Vec<Refusal> {
        self.held
            .iter()
            .filter_map(|policy| policy.allows(request, nonce).err())
            .collect()
    }

    /// The directives these policies contain that this engine does not act on.
    ///
    /// Sorted and deduplicated, and it exists because the honest answer to "is
    /// this page protected" is sometimes "in these four respects, and not in
    /// that one". A `frame-ancestors` nobody enforces is a gap; a gap nobody
    /// prints is a false sense of security.
    pub fn not_enforced(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .held
            .iter()
            .flat_map(|policy| policy.directives.iter())
            .filter(|directive| directive.name == Name::Other)
            .map(|directive| directive.written.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// How many policies were stated. Zero means nothing was said.
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether nothing was said at all.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }
}

/// Why a policy refused something.
///
/// Named cases with the directive in them, for the reason [`crate::cors`] gives
/// for the same shape: somebody looking at a blocked script needs to know
/// **which** sentence of their own policy stopped it, and browsers are
/// notorious for answering that badly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A load the policy does not permit.
    Fetch {
        /// What it was for.
        purpose: String,
        /// What was asked for.
        url: String,
        /// The directive that decided, as written — which may be `default-src`
        /// rather than the specific one, and saying which is half the answer.
        directive: String,
        /// What that directive does allow, as written.
        allows: Vec<String>,
        /// The sources in it this engine could not read.
        unreadable: Vec<String>,
    },
    /// Inline content the policy does not permit.
    Inline {
        /// Which kind.
        kind: Inline,
        /// The directive that decided, as written.
        directive: String,
        /// What that directive does allow, as written.
        allows: Vec<String>,
        /// Whether a hash in that directive might have allowed it. This engine
        /// computes none yet (queue item 189), so a policy that allows inline
        /// content only by hash blocks it here — and says so rather than
        /// reporting a bare refusal.
        only_by_hash: bool,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refusal::Fetch {
                purpose,
                url,
                directive,
                allows,
                unreadable,
            } => {
                write!(
                    f,
                    "this page's content security policy does not allow a {purpose} from {url}: \
                     {directive} allows {}",
                    if allows.is_empty() {
                        "nothing".to_owned()
                    } else {
                        allows.join(" ")
                    }
                )?;
                if !unreadable.is_empty() {
                    write!(
                        f,
                        ". This engine could not read {} of that directive, and kept it as \
                         something that matches nothing — a policy is never widened by our not \
                         understanding a word of it",
                        unreadable.join(" and ")
                    )?;
                }
                Ok(())
            }
            Refusal::Inline {
                kind,
                directive,
                allows,
                only_by_hash,
            } => {
                write!(
                    f,
                    "this page's content security policy does not allow {kind}: {directive} \
                     allows {}",
                    if allows.is_empty() {
                        "nothing".to_owned()
                    } else {
                        allows.join(" ")
                    }
                )?;
                if *only_by_hash {
                    f.write_str(
                        ". That directive would allow it by hash, and this engine does not \
                         compute one yet (queue item 189), so it is refused rather than run",
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        alo_url::parse(text).expect("a URL")
    }

    fn asking(target: &str, purpose: Purpose) -> Request {
        Request::get(url(target))
            .for_purpose(purpose)
            .asked_by(Origin::of(&url("https://example.com/page")))
    }

    fn enforcing(value: &str) -> Policies {
        let mut headers = Headers::new();
        headers.add("Content-Security-Policy", value);
        Policies::stated_by(&headers)
    }

    #[test]
    fn a_policy_that_would_block_an_injected_script_blocks_it() {
        let policies = enforcing("default-src 'self'");
        assert!(
            policies
                .allows(&asking("https://example.com/own.js", Purpose::Script), None)
                .is_ok()
        );
        let refused = policies
            .allows(&asking("https://evil.test/steal.js", Purpose::Script), None)
            .expect_err("an injected script was allowed");
        let said = refused.to_string();
        assert!(said.contains("default-src"), "{said}");
        assert!(said.contains("'self'"), "{said}");
    }

    #[test]
    fn the_specific_directive_beats_default_src() {
        let policies = enforcing("default-src 'none'; img-src https://pictures.test");
        assert!(
            policies
                .allows(&asking("https://pictures.test/a.png", Purpose::Image), None)
                .is_ok()
        );
        assert!(
            policies
                .allows(&asking("https://pictures.test/a.js", Purpose::Script), None)
                .is_err(),
            "a script fell back to default-src and should have been refused",
        );
    }

    #[test]
    fn a_directive_nobody_here_can_read_still_refuses() {
        // One source is from the future. The directive is kept whole, so the
        // other source still decides and the future one permits nothing.
        let policies = enforcing("script-src 'self' 'from-the-future'");
        let refused = policies
            .allows(&asking("https://cdn.test/x.js", Purpose::Script), None)
            .expect_err("an unreadable source widened the directive");
        let said = refused.to_string();
        assert!(said.contains("'from-the-future'"), "{said}");
        assert!(
            policies
                .allows(&asking("https://example.com/x.js", Purpose::Script), None)
                .is_ok(),
            "and the readable half still works",
        );
    }

    #[test]
    fn a_directive_whose_every_source_is_unreadable_allows_nothing() {
        let policies = enforcing("script-src 'from-the-future'");
        assert!(
            policies
                .allows(&asking("https://example.com/x.js", Purpose::Script), None)
                .is_err(),
            "it fell through to default-src, or to nothing",
        );
    }

    #[test]
    fn a_repeated_directive_keeps_the_first() {
        let policies = enforcing("script-src 'self'; script-src https://evil.test");
        assert!(
            policies
                .allows(&asking("https://evil.test/x.js", Purpose::Script), None)
                .is_err(),
            "appending to the header widened the policy",
        );
    }

    #[test]
    fn two_policies_are_an_intersection() {
        let mut headers = Headers::new();
        headers.add(
            "Content-Security-Policy",
            "script-src 'self' https://a.test",
        );
        headers.add(
            "Content-Security-Policy",
            "script-src 'self' https://b.test",
        );
        let policies = Policies::stated_by(&headers);
        assert_eq!(policies.len(), 2);
        assert!(
            policies
                .allows(&asking("https://example.com/x.js", Purpose::Script), None)
                .is_ok(),
            "both allow 'self'",
        );
        for one in ["https://a.test/x.js", "https://b.test/x.js"] {
            assert!(
                policies
                    .allows(&asking(one, Purpose::Script), None)
                    .is_err(),
                "{one} was allowed by one policy and refused by the other",
            );
        }
    }

    #[test]
    fn two_policies_in_one_header_are_two_policies() {
        let policies = enforcing("script-src 'self', script-src https://b.test");
        assert_eq!(policies.len(), 2);
        assert!(
            policies
                .allows(&asking("https://example.com/x.js", Purpose::Script), None)
                .is_err(),
        );
    }

    #[test]
    fn a_report_only_policy_blocks_nothing_and_is_still_visible() {
        let mut headers = Headers::new();
        headers.add("Content-Security-Policy-Report-Only", "default-src 'none'");
        let policies = Policies::stated_by(&headers);
        let request = asking("https://example.com/x.js", Purpose::Script);
        assert!(
            policies.allows(&request, None).is_ok(),
            "a policy being watched was enforced, which breaks careful sites",
        );
        assert_eq!(
            policies.objections(&request, None).len(),
            1,
            "and it objected, which is what item 188 will report",
        );
    }

    #[test]
    fn a_nonce_on_the_element_that_asked_gets_it_in() {
        let policies = enforcing("script-src 'nonce-r4nd0m'");
        let request = asking("https://cdn.test/x.js", Purpose::Script);
        assert!(policies.allows(&request, Some("r4nd0m")).is_ok());
        assert!(policies.allows(&request, Some("R4ND0M")).is_err(), "case");
        assert!(policies.allows(&request, None).is_err());
    }

    #[test]
    fn strict_dynamic_ignores_every_host_in_a_script_directive() {
        let policies = enforcing("script-src 'strict-dynamic' 'nonce-abc' https://cdn.test 'self'");
        assert!(
            policies
                .allows(&asking("https://cdn.test/x.js", Purpose::Script), None)
                .is_err(),
            "the author said to ignore hosts and the host got in anyway",
        );
        assert!(
            policies
                .allows(&asking("https://example.com/x.js", Purpose::Script), None)
                .is_err(),
            "'self' is ignored too",
        );
        assert!(
            policies
                .allows(
                    &asking("https://cdn.test/x.js", Purpose::Script),
                    Some("abc")
                )
                .is_ok(),
        );
    }

    /// The keyword is defined for scripts. Applying it elsewhere would refuse
    /// pictures an author plainly allowed.
    #[test]
    fn strict_dynamic_does_not_reach_a_picture() {
        let policies = enforcing("img-src 'strict-dynamic' https://pictures.test");
        assert!(
            policies
                .allows(&asking("https://pictures.test/a.png", Purpose::Image), None)
                .is_ok()
        );
    }

    #[test]
    fn unsafe_inline_is_ignored_once_a_nonce_is_present() {
        let with_only_the_keyword = enforcing("script-src 'unsafe-inline'");
        assert!(
            with_only_the_keyword
                .allows_inline(Inline::Script, None)
                .is_ok()
        );

        let with_a_nonce = enforcing("script-src 'unsafe-inline' 'nonce-abc'");
        assert!(
            with_a_nonce.allows_inline(Inline::Script, None).is_err(),
            "the keyword is left in for old browsers and must not undo the nonce",
        );
        assert!(
            with_a_nonce
                .allows_inline(Inline::Script, Some("abc"))
                .is_ok()
        );
    }

    #[test]
    fn inline_content_a_hash_would_have_allowed_says_so() {
        let policies = enforcing("script-src 'sha256-YWJj'");
        let refused = policies
            .allows_inline(Inline::Script, None)
            .expect_err("a hash was computed, which nothing here does");
        let said = refused.to_string();
        assert!(said.contains("hash"), "{said}");
        assert!(said.contains("189"), "{said}");
    }

    #[test]
    fn a_style_directive_governs_inline_style_and_a_script_one_does_not() {
        let policies = enforcing("script-src 'unsafe-inline'; style-src 'self'");
        assert!(policies.allows_inline(Inline::Script, None).is_ok());
        assert!(
            policies.allows_inline(Inline::Style, None).is_err(),
            "'self' does not allow inline style",
        );
    }

    #[test]
    fn a_directive_this_engine_does_not_act_on_is_named_rather_than_believed() {
        let policies = enforcing("default-src 'self'; frame-ancestors 'none'; report-uri /r");
        assert_eq!(
            policies.not_enforced(),
            vec!["frame-ancestors".to_owned(), "report-uri".to_owned()],
        );
    }

    #[test]
    fn a_document_load_is_not_governed_and_the_gap_is_named() {
        let policies = enforcing("default-src 'none'; frame-src 'none'");
        assert!(
            policies
                .allows(
                    &asking("https://anywhere.test/page", Purpose::Document),
                    None
                )
                .is_ok(),
            "a link off a protected page must still work",
        );
        assert_eq!(policies.not_enforced(), vec!["frame-src".to_owned()]);
    }

    #[test]
    fn the_first_load_of_a_window_is_governed_by_nobodys_policy() {
        let policies = enforcing("default-src 'none'");
        let typed = Request::get(url("https://anywhere.test/")).for_purpose(Purpose::Script);
        assert!(policies.allows(&typed, None).is_ok());
    }

    #[test]
    fn nothing_said_permits_everything() {
        let policies = Policies::none();
        assert!(policies.is_empty());
        assert!(
            policies
                .allows(&asking("https://evil.test/x.js", Purpose::Script), None)
                .is_ok()
        );
        assert!(policies.allows_inline(Inline::Script, None).is_ok());
    }

    /// Whatever a server sends, a policy is read and nothing panics.
    #[test]
    fn nothing_a_server_can_write_is_worse_than_a_narrow_policy() {
        for value in [
            "",
            ";",
            ";;;;;;",
            ",",
            ",,,,",
            "   ",
            "script-src",
            "script-src;",
            "script-src ;;; style-src",
            "SCRIPT-SRC 'SELF'",
            "\u{0}\u{1}",
            "script-src \u{0}",
            "script-src 'self'; script-src 'self'; script-src 'self'",
            "default-src 'none'; ; ; default-src *",
            "script-src *",
            "script-src * 'unsafe-inline'",
        ] {
            let policies = enforcing(value);
            let _ = policies.allows(&asking("https://evil.test/x.js", Purpose::Script), None);
            let _ = policies.allows_inline(Inline::Script, None);
            let _ = policies.not_enforced();
        }
    }

    /// A header of the biggest size `crate::http` will read, in the shape most
    /// likely to be quadratic: every directive a repeat of the one before it.
    #[test]
    fn the_largest_header_a_response_may_carry_is_read_without_trouble() {
        let value = "script-src 'self';".repeat(8 * 1024 / 18);
        let policies = enforcing(&value);
        assert_eq!(policies.len(), 1);
        assert!(
            policies
                .allows(&asking("https://evil.test/x.js", Purpose::Script), None)
                .is_err()
        );
    }

    #[test]
    fn a_case_folded_directive_name_is_the_same_directive() {
        let policies = enforcing("Script-Src 'self'");
        assert!(
            policies
                .allows(&asking("https://evil.test/x.js", Purpose::Script), None)
                .is_err()
        );
        assert!(
            policies.not_enforced().is_empty(),
            "it was read as a directive nobody acts on",
        );
    }

    #[test]
    fn a_star_permits_a_host_and_not_a_scheme_with_no_host() {
        let policies = enforcing("img-src *");
        assert!(
            policies
                .allows(&asking("https://anywhere.test/a.png", Purpose::Image), None)
                .is_ok()
        );
        assert!(
            policies
                .allows(&asking("data:image/png;base64,AAAA", Purpose::Image), None)
                .is_err(),
            "`*` names hosts, and no host source reaches a scheme the page's own cannot",
        );
    }
}
