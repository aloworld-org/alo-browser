/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Telling an author that their own policy stopped something.
//!
//! [`crate::csp`] decides; this says what happened, to whoever the policy named.
//! The two are separate files because they have different reasons to change: a
//! newly enforced directive changes the deciding, and a new report field or a
//! new place to send one changes this.
//!
//! # Why reporting is the half that makes a policy deployable
//!
//! Nobody writes `script-src 'self'` on a large site and turns it on. They send
//! `Content-Security-Policy-Report-Only` for a week, read what it *would* have
//! blocked, and find the four scripts their own marketing department added
//! through a tag manager. A browser that enforced without reporting would leave
//! them no way to do that, and a browser that reported nothing would make
//! [`Disposition::Report`] a header with no effect at all.
//!
//! So both dispositions report, and that is the first thing the tests here
//! assert.
//!
//! # The rule that decides what a report may say
//!
//! **A report is posted to a server the page chose, so a report that named a
//! cross-origin URL in full would be a way to read one.** A page cannot see
//! where a redirect ended, or what is in the query of a URL another origin
//! handed it — and if a blocked load's whole URL appeared in a report, a page
//! could learn both by writing a narrow policy and reading its own reports.
//!
//! [`stripped`] is the answer, and it is the specification's: anything not on a
//! network scheme is reduced to the scheme alone, anything cross-origin to the
//! page is reduced to its origin, and the page's own URLs lose their fragment
//! and any credentials written into them. The last of those is not decoration
//! here — [`alo_url::Url::serialised`] keeps `user:password@`, so a report
//! written from that string rather than from the parts would post somebody's
//! password to a log collector.
//!
//! # What is not in a report, and why
//!
//! `script-sample`, `source-file`, `line-number` and `column-number` are
//! **omitted rather than filled with zero**. This engine has no line numbers at
//! the point a policy decides — the request being refused knows what it is for
//! and not which line of markup asked — and a `"line-number": 0` in a report is
//! a wrong answer that reads like a right one. A field nobody sent is a field an
//! author can see is missing.
//!
//! `user_agent` is omitted from the Reporting API's envelope for a different
//! reason: this engine sends no `User-Agent` header anywhere, and a report is
//! not the place to invent the first fingerprint it ever emits.

use crate::csp::{Disposition, Refusal};
use crate::headers::Headers;
use crate::request::{Purpose, Request};
use alo_url::{Origin, Url};
use core::fmt;
use core::fmt::Write as _;

/// What a policy said about where to be told.
///
/// Parsed by [`crate::csp`] out of `report-uri` and `report-to`, and held as
/// written: resolving a reference needs the page's URL, which a policy does not
/// have and a [`Page`] does.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Told {
    /// `report-to`: the name of a group defined by `Reporting-Endpoints`.
    pub group: Option<String>,
    /// `report-uri`: URL references, in the order they were written.
    pub uris: Vec<String>,
}

impl Told {
    /// Whether the policy asked to be told anything at all.
    pub fn is_silent(&self) -> bool {
        self.group.is_none() && self.uris.is_empty()
    }
}

/// What a policy refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocked {
    /// A load, of this URL.
    At(Url),
    /// Content written into the page itself, which has no URL. The
    /// specification's word for it in a report is the literal `inline`.
    Inline,
}

/// The page a policy is protecting, and what a report about it needs to say.
///
/// A policy knows what it forbade; only the page knows where it was, what
/// referred to it, and what its own response status was. Held together rather
/// than passed as four arguments, because every one of them is a fact about the
/// same document.
#[derive(Debug, Clone)]
pub struct Page {
    /// The document's URL.
    pub url: Url,
    /// What `Referer` this document was loaded with, if any — already decided
    /// by [`crate::referrer`], and never widened here.
    pub referrer: String,
    /// The status the document itself was answered with.
    pub status: u16,
    /// The reporting endpoints its response defined, which is what a
    /// `report-to` group name means anything against.
    pub endpoints: Endpoints,
}

impl Page {
    /// A page at a URL, answered `200`, referred by nobody, defining no
    /// endpoints.
    pub fn at(url: Url) -> Self {
        Self {
            url,
            referrer: String::new(),
            status: 200,
            endpoints: Endpoints::default(),
        }
    }

    /// The same page, with the referrer it was loaded with.
    #[must_use]
    pub fn came_from(mut self, referrer: &str) -> Self {
        referrer.clone_into(&mut self.referrer);
        self
    }

    /// The same page, answered with this status.
    #[must_use]
    pub fn answered(mut self, status: u16) -> Self {
        self.status = status;
        self
    }

    /// The same page, with the endpoints its headers defined.
    #[must_use]
    pub fn reporting_to(mut self, endpoints: Endpoints) -> Self {
        self.endpoints = endpoints;
        self
    }

    /// This page's own origin, which every stripping decision is made against.
    fn origin(&self) -> Origin {
        Origin::of(&self.url)
    }
}

/// The groups a `Reporting-Endpoints` header defined.
///
/// `Reporting-Endpoints: csp="https://collector.test/reports", other="…"` — a
/// structured-fields dictionary, so a comma separates two entries and a comma
/// *inside* a quoted URL does not. Split naively and
/// `https://collector.test/a,b` becomes two broken endpoints, which is why this
/// is a scanner rather than a `split(',')`.
///
/// The older `Report-To` header — a JSON document with groups, endpoints and
/// weights in it — is **not read**, and a `report-to` naming a group only it
/// defined is named in [`Posting::unusable`] rather than silently dropped. It
/// is deprecated in favour of this one, and reading two spellings of the same
/// thing is two chances to disagree about where somebody's reports go.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Endpoints {
    /// Group name to URL, first definition of each name.
    named: Vec<(String, String)>,
}

impl Endpoints {
    /// The groups a response's headers define.
    pub fn stated_by(headers: &Headers) -> Self {
        let mut named: Vec<(String, String)> = Vec::new();
        for value in headers.all("Reporting-Endpoints") {
            for entry in entries(value) {
                let Some((name, url)) = one_endpoint(&entry) else {
                    continue;
                };
                // First wins, for the reason a repeated directive keeps the
                // first: anybody who can append to a header must not be able to
                // redirect somebody else's reports by restating a group.
                if named.iter().any(|(held, _)| *held == name) {
                    continue;
                }
                named.push((name, url));
            }
        }
        Self { named }
    }

    /// Where a group name points, when it was defined.
    pub fn named(&self, group: &str) -> Option<&str> {
        self.named
            .iter()
            .find(|(name, _)| name == group)
            .map(|(_, url)| url.as_str())
    }

    /// How many groups were defined.
    pub fn len(&self) -> usize {
        self.named.len()
    }

    /// Whether the header defined nothing.
    pub fn is_empty(&self) -> bool {
        self.named.is_empty()
    }
}

/// Split a dictionary on the commas that are not inside a quoted string.
fn entries(value: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut held = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for letter in value.chars() {
        if escaped {
            held.push(letter);
            escaped = false;
            continue;
        }
        match letter {
            '\\' if quoted => escaped = true,
            '"' => {
                quoted = !quoted;
                held.push(letter);
            }
            ',' if !quoted => {
                found.push(std::mem::take(&mut held));
            }
            _ => held.push(letter),
        }
    }
    found.push(held);
    found
}

/// One `name="url"` entry, with the quotes taken off.
fn one_endpoint(entry: &str) -> Option<(String, String)> {
    let (name, value) = entry.split_once('=')?;
    let name = name.trim();
    let value = value.trim();
    if name.is_empty() {
        return None;
    }
    let url = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))?;
    if url.is_empty() {
        return None;
    }
    Some((
        name.to_owned(),
        url.replace("\\\"", "\"").replace("\\\\", "\\"),
    ))
}

/// One violation of one policy: everything a report says, before it is written.
///
/// Built by [`crate::csp::Policies::violations`], which is the only thing that
/// can build one — a violation nobody's policy objected to is not a thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Whether the policy that objected was enforcing or only watching. Both
    /// report; only one of them blocked anything.
    pub disposition: Disposition,
    /// The policy, as its author wrote it. The report's `original-policy`, and
    /// the reason a report needs no field for *which* directive decided: an
    /// author reading `default-src 'none'` beside an effective directive of
    /// `script-src` can see how it was reached.
    pub policy: String,
    /// The directive that governs what was asked for — `script-src` for a
    /// script, whichever directive actually decided. The specification's
    /// *effective directive*, and its `violated-directive` is a historical
    /// alias for the same value.
    pub directive: String,
    /// What was refused.
    pub blocked: Blocked,
    /// Where this policy asked to be told.
    pub told: Told,
    /// The refusal in words — what a person reads, and what no report format
    /// has a field for.
    pub refusal: Refusal,
}

impl Violation {
    /// What `blocked-uri` says: a stripped URL, or the literal `inline`.
    fn blocked_as_reported(&self, about: &Page) -> String {
        match &self.blocked {
            Blocked::At(url) => stripped(url, &about.origin()),
            Blocked::Inline => "inline".to_owned(),
        }
    }

    /// The `application/csp-report` document `report-uri` asks for.
    pub fn as_csp_report(&self, about: &Page) -> String {
        let said = pairs(&[
            ("document-uri", &stripped(&about.url, &about.origin())),
            ("referrer", &about.referrer),
            ("blocked-uri", &self.blocked_as_reported(about)),
            ("effective-directive", &self.directive),
            ("violated-directive", &self.directive),
            ("original-policy", &self.policy),
            ("disposition", &self.disposition.to_string()),
        ]);
        format!(
            "{{\"csp-report\":{{{said},\"status-code\":{}}}}}",
            about.status
        )
    }

    /// The `application/reports+json` document the Reporting API asks for.
    ///
    /// A list, because that format exists to carry several reports in one post.
    /// This engine sends one at a time and says so rather than pretending to
    /// batch: `age` is `0` for the same reason, since nothing here queues a
    /// report between making it and posting it.
    pub fn as_report(&self, about: &Page) -> String {
        let document = stripped(&about.url, &about.origin());
        let said = pairs(&[
            ("documentURL", &document),
            ("referrer", &about.referrer),
            ("blockedURL", &self.blocked_as_reported(about)),
            ("effectiveDirective", &self.directive),
            ("originalPolicy", &self.policy),
            ("disposition", &self.disposition.to_string()),
        ]);
        format!(
            "[{{\"age\":0,\"type\":\"csp-violation\",\"url\":{},\"body\":{{{said},\"statusCode\":{}}}}}]",
            quoted(&document),
            about.status,
        )
    }

    /// The posts this violation asks for, and the endpoints nobody could use.
    ///
    /// **`report-to` wins when it resolves.** The specification says a policy
    /// carrying both is reported through the newer one only, and the reason is
    /// that a site migrating between them writes both and does not want two
    /// copies of every violation. When the group it names was never defined,
    /// nothing is sent and [`Posting::unusable`] says so — falling back to
    /// `report-uri` would be this engine deciding that an author who wrote a
    /// group name meant something else.
    pub fn posts(&self, about: &Page) -> Posting {
        let mut posting = Posting::default();
        if let Some(group) = &self.told.group {
            match about.endpoints.named(group) {
                Some(reference) => posting.add(self, about, reference, Format::Reports),
                None => posting.unusable.push(format!(
                    "report-to names the group {group:?}, and no Reporting-Endpoints header on \
                     this page defines it, so this violation is reported nowhere"
                )),
            }
            return posting;
        }
        for reference in &self.told.uris {
            posting.add(self, about, reference, Format::CspReport);
        }
        posting
    }
}

/// Which of the two documents a report is written as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// `report-uri`'s: `application/csp-report`.
    CspReport,
    /// `report-to`'s: `application/reports+json`.
    Reports,
}

impl Format {
    /// What the post says it is sending.
    fn media_type(self) -> &'static str {
        match self {
            Format::CspReport => "application/csp-report",
            Format::Reports => "application/reports+json",
        }
    }
}

/// What a violation turned into: the posts to make, and what could not be.
///
/// Both halves, because an endpoint nobody could use is exactly the thing an
/// author needs told — a report that goes nowhere is indistinguishable from a
/// policy that was never violated, which is the worst way to be wrong about
/// whether a site is protected.
#[derive(Debug, Clone, Default)]
pub struct Posting {
    /// The requests to send. Fire and forget: see [`Posting::posts`]'s own
    /// note, and [`crate::pool::Pool::report`], which never fails a load with
    /// one.
    pub posts: Vec<Request>,
    /// The endpoints that could not be used, each said in words.
    pub unusable: Vec<String>,
}

impl Posting {
    /// Resolve one endpoint and add the post for it, or say why not.
    fn add(&mut self, violation: &Violation, about: &Page, reference: &str, format: Format) {
        let url = match endpoint(reference, about) {
            Ok(url) => url,
            Err(why) => {
                self.unusable.push(why);
                return;
            }
        };
        let body = match format {
            Format::CspReport => violation.as_csp_report(about),
            Format::Reports => violation.as_report(about),
        };
        let mut post = Request::sending(url, "POST", body.into_bytes())
            .for_purpose(Purpose::Report)
            .asked_by(about.origin());
        post.headers.add("Content-Type", format.media_type());
        self.posts.push(post);
    }
}

/// Where one `report-uri` reference or one endpoint URL actually points.
///
/// # Errors
///
/// A sentence, when it is not a URL or not somewhere a report may be posted.
fn endpoint(reference: &str, about: &Page) -> Result<Url, String> {
    let url = alo_url::join(&about.url, reference).map_err(|why| {
        format!("a reporting endpoint of {reference:?} is not a URL this engine can resolve: {why}")
    })?;
    // A report is a `POST` of a document describing the page. `javascript:`,
    // `data:` and `file:` are not places to post one, and an engine that tried
    // would be turning a reporting directive into a way to reach the disk.
    if !matches!(url.scheme.as_str(), "http" | "https") {
        return Err(format!(
            "a reporting endpoint of {reference:?} is {}, and a violation report is only ever \
             posted over http or https",
            url.scheme
        ));
    }
    Ok(url)
}

/// A URL as a report may say it: the specification's *strip URL for use in
/// reports*.
///
/// Three answers, in this order, and each is a leak closed rather than a
/// tidying:
///
/// - **Not a network scheme** — `data:`, `blob:`, `about:` — is reduced to the
///   scheme alone. The body of a `data:` URL is the content itself, and a
///   report carrying it would post whatever a page was refused.
/// - **Cross-origin to the page** is reduced to its origin, because the page
///   is the thing that reads the report and a full URL from another origin is
///   something the page could not otherwise see. `/reset?token=…` is the
///   example that matters.
/// - **The page's own** keeps its path and query, and loses its fragment and
///   any credentials written into it. A fragment is never sent anywhere
///   (see [`alo_url::Url::fragment`]), and `https://user:password@…` is a
///   password a log collector must not be handed.
pub fn stripped(url: &Url, seen_by: &Origin) -> String {
    if !matches!(url.scheme.as_str(), "http" | "https" | "ftp" | "ws" | "wss") {
        return url.scheme.clone();
    }
    let its_own = Origin::of(url);
    if !its_own.is_same_origin(seen_by) {
        return its_own.to_string();
    }
    from_the_parts(url)
}

/// A URL written back out from its parts rather than from what was parsed.
///
/// [`alo_url::Url::serialised`] is what the parser produced, credentials and
/// fragment included. Everything a report says is built here instead, so
/// neither can reach one by being forgotten.
fn from_the_parts(url: &Url) -> String {
    // Writing to a `String` cannot fail.
    let mut written = String::new();
    let _ = write!(written, "{}://", url.scheme);
    if let Some(host) = &url.host {
        let _ = write!(written, "{host}");
    }
    if let Some(port) = url.port {
        let _ = write!(written, ":{port}");
    }
    written.push_str(&url.path);
    if let Some(query) = &url.query {
        let _ = write!(written, "?{query}");
    }
    written
}

/// A run of `"name":"value"` pairs, comma separated.
fn pairs(fields: &[(&str, &String)]) -> String {
    fields
        .iter()
        .map(|(name, value)| format!("{}:{}", quoted(name), quoted(value)))
        .collect::<Vec<_>>()
        .join(",")
}

/// One JSON string.
///
/// Written here rather than rented: the shape of a report is fixed and small,
/// and ADR 0001 rents physics rather than eight lines of escaping. What it must
/// get right is that **a value came from a stranger** — a policy header, a URL,
/// a group name — so every control character is escaped rather than passed
/// through, and a collector reading this can never be handed a broken document
/// by a server that put a newline in its own policy.
fn quoted(text: &str) -> String {
    let mut written = String::with_capacity(text.len() + 2);
    written.push('"');
    for letter in text.chars() {
        match letter {
            '"' => written.push_str("\\\""),
            '\\' => written.push_str("\\\\"),
            '\n' => written.push_str("\\n"),
            '\r' => written.push_str("\\r"),
            '\t' => written.push_str("\\t"),
            control if control < ' ' || control == '\u{7f}' => {
                let _ = write!(written, "\\u{:04x}", control as u32);
            }
            other => written.push(other),
        }
    }
    written.push('"');
    written
}

impl fmt::Display for Violation {
    /// The refusal, with which policy objected in front of it — because a
    /// report-only violation reads as a block otherwise, and it blocked
    /// nothing.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.disposition {
            Disposition::Enforce => write!(f, "{}", self.refusal),
            Disposition::Report => write!(
                f,
                "a policy being watched rather than enforced objected, and nothing was blocked: {}",
                self.refusal
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        alo_url::parse(text).expect("a URL")
    }

    fn page() -> Page {
        Page::at(url("https://shop.example.com/checkout?step=2"))
    }

    #[test]
    fn a_page_reads_its_own_url_whole_and_anybody_elses_as_an_origin() {
        let here = Origin::of(&url("https://shop.example.com/checkout"));
        assert_eq!(
            stripped(&url("https://shop.example.com/a/b?c=d"), &here),
            "https://shop.example.com/a/b?c=d",
        );
        assert_eq!(
            stripped(&url("https://evil.test/steal.js?token=s3cret"), &here),
            "https://evil.test",
        );
    }

    #[test]
    fn a_fragment_and_a_password_never_reach_a_report() {
        let here = Origin::of(&url("https://shop.example.com/"));
        assert_eq!(
            stripped(&url("https://shop.example.com/a#the-fragment"), &here),
            "https://shop.example.com/a",
        );
        let with_credentials = url("https://someone:hunter2@shop.example.com/a");
        assert!(
            with_credentials.serialised.contains("hunter2"),
            "the parser kept them, which is what this rule is for",
        );
        assert_eq!(
            stripped(&with_credentials, &here),
            "https://shop.example.com/a",
        );
    }

    #[test]
    fn a_scheme_with_no_origin_is_reported_as_the_scheme_alone() {
        let here = Origin::of(&url("https://shop.example.com/"));
        assert_eq!(stripped(&url("data:text/html,<b>hi"), &here), "data");
        assert_eq!(stripped(&url("about:blank"), &here), "about");
    }

    /// A port that is not the scheme's own is part of the origin, so a report
    /// naming one must keep it.
    #[test]
    fn a_port_survives_both_answers() {
        let here = Origin::of(&url("https://shop.example.com:8443/"));
        assert_eq!(
            stripped(&url("https://shop.example.com:8443/a"), &here),
            "https://shop.example.com:8443/a",
        );
        assert_eq!(
            stripped(&url("https://shop.example.com/a"), &here),
            "https://shop.example.com",
            "a different port is a different origin",
        );
    }

    #[test]
    fn a_group_is_defined_by_name_and_a_comma_in_a_url_is_not_a_separator() {
        let mut headers = Headers::new();
        headers.add(
            "Reporting-Endpoints",
            "csp=\"https://collector.test/r?a=1,2\", other=\"https://elsewhere.test/x\"",
        );
        let endpoints = Endpoints::stated_by(&headers);
        assert_eq!(endpoints.len(), 2);
        assert_eq!(
            endpoints.named("csp"),
            Some("https://collector.test/r?a=1,2"),
        );
        assert_eq!(endpoints.named("nobody"), None);
    }

    #[test]
    fn a_repeated_group_keeps_the_first() {
        let mut headers = Headers::new();
        headers.add("Reporting-Endpoints", "csp=\"https://first.test/r\"");
        headers.add("Reporting-Endpoints", "csp=\"https://second.test/r\"");
        let endpoints = Endpoints::stated_by(&headers);
        assert_eq!(endpoints.named("csp"), Some("https://first.test/r"));
    }

    /// Whatever a server writes, a group list is read and nothing panics.
    #[test]
    fn nothing_a_server_can_write_defines_a_group_by_accident() {
        for value in [
            "",
            ",",
            ",,,,",
            "=",
            "csp=",
            "csp=\"\"",
            "csp=unquoted",
            "=\"https://x.test/\"",
            "\"only-a-string\"",
            "csp=\"https://x.test/\\\"quoted\"",
            "\u{0}=\u{1}",
        ] {
            let mut headers = Headers::new();
            headers.add("Reporting-Endpoints", value);
            let endpoints = Endpoints::stated_by(&headers);
            assert!(
                endpoints
                    .named("csp")
                    .is_none_or(|url| url.starts_with("https://")),
                "{value:?} defined csp as something unusable",
            );
        }
    }

    #[test]
    fn a_control_character_a_server_wrote_cannot_break_the_document() {
        assert_eq!(quoted("a\nb"), "\"a\\nb\"");
        assert_eq!(quoted("a\"b"), "\"a\\\"b\"");
        assert_eq!(quoted("a\\b"), "\"a\\\\b\"");
        assert_eq!(quoted("a\u{1}b"), "\"a\\u0001b\"");
        assert_eq!(quoted("héllo"), "\"héllo\"");
    }

    fn violation(told: Told) -> Violation {
        Violation {
            disposition: Disposition::Enforce,
            policy: "script-src 'self'".to_owned(),
            directive: "script-src".to_owned(),
            blocked: Blocked::At(url("https://evil.test/steal.js?token=s3cret")),
            told,
            refusal: Refusal::Inline {
                kind: crate::csp::Inline::Script,
                placement: crate::csp::Placement::Element,
                directive: "script-src".to_owned(),
                allows: vec!["'self'".to_owned()],
                by_hash: crate::csp::ByHash::NotNamed,
            },
        }
    }

    #[test]
    fn a_report_uri_is_resolved_against_the_page() {
        let posting = violation(Told {
            group: None,
            uris: vec!["/report".to_owned()],
        })
        .posts(&page());
        assert_eq!(posting.posts.len(), 1);
        let post = posting.posts.first().expect("a post");
        assert_eq!(post.url.to_string(), "https://shop.example.com/report");
        assert_eq!(post.method, "POST");
        assert_eq!(
            post.headers.get("Content-Type"),
            Some("application/csp-report"),
        );
        assert_eq!(post.purpose, Purpose::Report);
    }

    #[test]
    fn an_endpoint_that_is_not_somewhere_to_post_is_named_rather_than_used() {
        for reference in ["javascript:alert(1)", "data:text/plain,x", "file:///etc"] {
            let posting = violation(Told {
                group: None,
                uris: vec![reference.to_owned()],
            })
            .posts(&page());
            assert!(posting.posts.is_empty(), "{reference} was posted to");
            assert_eq!(posting.unusable.len(), 1, "{reference} was dropped quietly");
        }
    }

    #[test]
    fn report_to_wins_when_it_resolves_and_says_so_when_it_does_not() {
        let mut headers = Headers::new();
        headers.add("Reporting-Endpoints", "csp=\"https://collector.test/r\"");
        let about = page().reporting_to(Endpoints::stated_by(&headers));

        let both = violation(Told {
            group: Some("csp".to_owned()),
            uris: vec!["/report".to_owned()],
        })
        .posts(&about);
        assert_eq!(both.posts.len(), 1, "a violation was reported twice");
        let post = both.posts.first().expect("a post");
        assert_eq!(post.url.to_string(), "https://collector.test/r");
        assert_eq!(
            post.headers.get("Content-Type"),
            Some("application/reports+json"),
        );

        let undefined = violation(Told {
            group: Some("nobody".to_owned()),
            uris: vec!["/report".to_owned()],
        })
        .posts(&about);
        assert!(undefined.posts.is_empty());
        let said = undefined.unusable.join(" ");
        assert!(said.contains("nobody"), "{said}");
        assert!(said.contains("Reporting-Endpoints"), "{said}");
    }

    #[test]
    fn a_policy_that_asked_to_be_told_nothing_posts_nothing() {
        let told = Told::default();
        assert!(told.is_silent());
        let posting = violation(told).posts(&page());
        assert!(posting.posts.is_empty());
        assert!(posting.unusable.is_empty());
    }
}
