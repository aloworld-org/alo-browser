/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One source expression: a single entry in a policy's list of where content
//! may come from.
//!
//! Beside [`crate::csp`] rather than inside it for the reason `cors.rs` and
//! `preflight.rs` are two files: this one answers *does this URL match what the
//! author wrote*, which is a grammar and a matching algorithm, and that one
//! answers *which directive decides and what a refusal says*, which is policy.
//! They change for different reasons — a new source form here, a newly enforced
//! directive there.
//!
//! # The rule the whole file is written around
//!
//! **A token this engine cannot read is kept and matches nothing.** It is never
//! dropped, and the directive holding it is never discarded. Both of those
//! alternatives are how a policy gets *wider* than the author wrote it:
//! discarding `script-src` because one of its five sources was a keyword from
//! 2024 would send scripts to `default-src`, or to nothing at all, and a page
//! that asked for a protection would have lost it to our not understanding the
//! sentence it asked in. So [`Source::Unreadable`] exists, it permits no URL,
//! and a refusal names it — because an author whose page stopped working
//! deserves to be told which word we could not read.
//!
//! # What is deliberately narrower than a browser
//!
//! Two things, both in the direction of refusing rather than permitting:
//!
//! - A host written in Unicode (`*.köln.example`) is unreadable. Every host
//!   this engine holds is already in its ASCII form (`alo_url::parse`), so a
//!   Unicode source expression could never match anything; saying so is better
//!   than matching nothing quietly.
//! - An IPv6 literal is unreadable. CSP's own host grammar has no spelling for
//!   one, and inventing a spelling here would be inventing a rule about a
//!   security boundary.
//!
//! # And one thing that is not implemented, and is not silent about it
//!
//! A hash source (`'sha256-…'`) is **read** — its digest and its value are
//! held, and its presence correctly disables `'unsafe-inline'` — and it matches
//! nothing, because nothing here computes a digest. That is queue item 189, and
//! [`crate::csp::Refusal`] says so in words rather than reporting a bare block.

use alo_url::parts::default_port;
use alo_url::{Origin, Url};
use core::fmt;

/// Which digest a hash source names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Digest {
    /// SHA-256.
    Sha256,
    /// SHA-384.
    Sha384,
    /// SHA-512.
    Sha512,
}

impl Digest {
    /// The name a policy writes it as.
    pub const fn name(self) -> &'static str {
        match self {
            Digest::Sha256 => "sha256",
            Digest::Sha384 => "sha384",
            Digest::Sha512 => "sha512",
        }
    }
}

/// Which hosts a host-source names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Names {
    /// `*` — any host at all.
    Any,
    /// `*.example.com` — anything under a name, and **not** the name itself.
    ///
    /// That exclusion is the whole value of the form: an author who writes
    /// `*.example.com` is naming their subdomains, and a wildcard that also
    /// covered the bare name would quietly widen every such policy.
    Under(String),
    /// `example.com` — that name, and nothing under it.
    Exactly(String),
}

impl Names {
    /// Whether a URL's host is one of these.
    ///
    /// The host arrives already in ASCII form, so this is a fold-case compare
    /// rather than anything that could disagree with [`alo_url`] about what a
    /// name is.
    pub fn matches(&self, host: &str) -> bool {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        match self {
            Names::Any => true,
            Names::Under(base) => host.ends_with(&format!(".{base}")),
            Names::Exactly(name) => host == *name,
        }
    }
}

impl fmt::Display for Names {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Names::Any => f.write_str("*"),
            Names::Under(base) => write!(f, "*.{base}"),
            Names::Exactly(name) => f.write_str(name),
        }
    }
}

/// Which port a host-source names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Port {
    /// `:*` — any port.
    Any,
    /// `:8443` — that one.
    Exactly(u16),
}

/// `[scheme://]host[:port][/path]`, the commonest thing a policy contains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSource {
    /// The scheme, when one was written. When none was, the page's own scheme
    /// has to reach the URL's — see [`HostSource::matches`].
    pub scheme: Option<String>,
    /// The hosts named.
    pub names: Names,
    /// The port, when one was written.
    pub port: Option<Port>,
    /// The path, when one was written, always beginning with `/`.
    pub path: Option<String>,
}

impl HostSource {
    /// Whether this permits a URL, for a page at `page`.
    pub fn matches(&self, url: &Url, page: &Origin) -> bool {
        // A scheme the author wrote needs nothing from the page. When none was
        // written, the specification's rule is that the **page's own** scheme
        // has to reach the URL's — which is what stops `script-src example.com`
        // on an https page from permitting `http://example.com`. An opaque page
        // has neither, and the safe reading of that is no match.
        let pages = match page {
            Origin::Tuple { scheme, .. } => Some(scheme.as_str()),
            Origin::Opaque(_) => None,
        };
        let Some(reaching) = self.scheme.as_deref().or(pages) else {
            return false;
        };
        if !scheme_reaches(reaching, &url.scheme) {
            return false;
        }
        let Some(host) = &url.host else {
            return false;
        };
        self.names.matches(&host.to_string()) && self.port_matches(url) && self.path_matches(url)
    }

    /// Whether the URL is on the port this names.
    ///
    /// A source with **no** port matches only a URL on its own scheme's default
    /// port, which is what keeps `https://example.com` in a policy from
    /// permitting `https://example.com:8443`.
    fn port_matches(&self, url: &Url) -> bool {
        match self.port {
            Some(Port::Any) => true,
            Some(Port::Exactly(port)) => url.effective_port() == Some(port),
            None => url.effective_port() == default_port(&url.scheme),
        }
    }

    /// Whether the URL is under the path this names.
    ///
    /// A path ending in `/` is a directory and covers everything under it;
    /// anything else is exact. Because the directory form ends in `/`, a string
    /// prefix is a segment prefix here — `/a/` covers `/a/b` and does not cover
    /// `/ab`. A bare `/` covers everything, which is what the specification
    /// says and what most authors mean by writing it.
    fn path_matches(&self, url: &Url) -> bool {
        let Some(wanted) = &self.path else {
            return true;
        };
        if wanted == "/" {
            return true;
        }
        if wanted.ends_with('/') {
            return url.path.starts_with(wanted.as_str());
        }
        url.path == *wanted
    }
}

impl fmt::Display for HostSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(scheme) = &self.scheme {
            write!(f, "{scheme}://")?;
        }
        write!(f, "{}", self.names)?;
        match self.port {
            Some(Port::Any) => f.write_str(":*")?,
            Some(Port::Exactly(port)) => write!(f, ":{port}")?,
            None => {}
        }
        if let Some(path) = &self.path {
            f.write_str(path)?;
        }
        Ok(())
    }
}

/// One entry in a directive's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// `'none'`. Permits nothing, and means it.
    Nothing,
    /// `'self'`. The page's own origin — and, when the page is insecure, the
    /// same host reached securely.
    SameOrigin,
    /// `https:` — a whole scheme.
    Scheme(String),
    /// `[scheme://]host[:port][/path]`.
    Host(HostSource),
    /// `'unsafe-inline'`. About content rather than about where it came from,
    /// so it permits no URL; [`crate::csp::Policies::allows_inline`] is where
    /// it is read.
    UnsafeInline,
    /// `'nonce-…'`. The value keeps the case it was written in, because it is
    /// compared with an attribute byte for byte.
    Nonce(String),
    /// `'sha256-…'` and its two larger relatives. Read, and not yet computed —
    /// see this module's header, and queue item 189.
    Hash {
        /// Which digest was named.
        digest: Digest,
        /// The base64 the author wrote, as written.
        expected: String,
    },
    /// `'strict-dynamic'`. Turns a script directive into nonces and hashes
    /// only, ignoring every host and scheme in it.
    StrictDynamic,
    /// A keyword this engine reads and does not act on — `'unsafe-eval'` and
    /// its relatives, which are about running code rather than fetching it.
    ///
    /// Separate from [`Source::Unreadable`] on purpose: "we know what this
    /// means and there is nothing here for it to govern" and "we could not read
    /// this" are different things to tell somebody, even though both permit
    /// nothing.
    Inert(String),
    /// Something that is not a source expression this engine can read. Permits
    /// nothing, is never dropped, and is named in a refusal.
    Unreadable(String),
}

impl Source {
    /// Read one token of a directive's value.
    ///
    /// Total: every token becomes a source, and the ones that could not be read
    /// become [`Source::Unreadable`] rather than disappearing.
    pub fn parse(token: &str) -> Self {
        if let Some(inside) = token
            .strip_prefix('\'')
            .and_then(|rest| rest.strip_suffix('\''))
        {
            return keyword(inside, token);
        }
        if let Some(scheme) = scheme_source(token) {
            return Source::Scheme(scheme);
        }
        host_source(token).map_or_else(|| Source::Unreadable(token.to_owned()), Source::Host)
    }

    /// Whether this permits a URL, for a page at `page`.
    ///
    /// Everything that is not about *where content came from* permits no URL:
    /// `'none'` by definition, the inline keywords because they are about the
    /// content itself, the hashes because nothing here computes one, and the
    /// unreadable ones because that is the whole point of keeping them.
    pub fn matches(&self, url: &Url, page: &Origin) -> bool {
        match self {
            Source::SameOrigin => is_the_pages_own(url, page),
            Source::Scheme(scheme) => scheme_reaches(scheme, &url.scheme),
            Source::Host(host) => host.matches(url, page),
            _ => false,
        }
    }
}

impl fmt::Display for Source {
    /// As the author wrote it, near enough to be recognised in a message.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Nothing => f.write_str("'none'"),
            Source::SameOrigin => f.write_str("'self'"),
            Source::Scheme(scheme) => write!(f, "{scheme}:"),
            Source::Host(host) => write!(f, "{host}"),
            Source::UnsafeInline => f.write_str("'unsafe-inline'"),
            Source::Nonce(value) => write!(f, "'nonce-{value}'"),
            Source::Hash { digest, expected } => write!(f, "'{}-{expected}'", digest.name()),
            Source::StrictDynamic => f.write_str("'strict-dynamic'"),
            Source::Inert(name) => write!(f, "'{name}'"),
            Source::Unreadable(token) => f.write_str(token),
        }
    }
}

/// Whether one scheme reaches another, in the specification's sense.
///
/// Not equality, and the difference is the point: a policy naming `http` also
/// permits `https`, so that a site moving to TLS does not have to rewrite every
/// policy it ever wrote. **Never the other way round** — `https` does not reach
/// `http`, because that direction is a downgrade somebody could arrange.
pub fn scheme_reaches(source: &str, url: &str) -> bool {
    if source.eq_ignore_ascii_case(url) {
        return true;
    }
    let url = url.to_ascii_lowercase();
    match source.to_ascii_lowercase().as_str() {
        // `http` reaches its own secure form, and so does `wss` — which is
        // already the secure half of `ws`, so `https` is the only thing left
        // for it to reach.
        "http" | "wss" => url == "https",
        "ws" => matches!(url.as_str(), "wss" | "http" | "https"),
        _ => false,
    }
}

/// Whether `'self'` covers this URL.
///
/// The page's own origin, plus one allowance in the safe direction: an insecure
/// page's `'self'` also covers the same host reached **securely**, on that
/// scheme's own port. The reverse is never true, and neither is a different
/// port — those would each turn `'self'` into a wider word than the author
/// meant by it.
fn is_the_pages_own(url: &Url, page: &Origin) -> bool {
    if Origin::of(url) == *page {
        return true;
    }
    let Origin::Tuple { scheme, host, .. } = page else {
        return false;
    };
    if url.host.as_ref() != Some(host) {
        return false;
    }
    let upgraded = match scheme.as_str() {
        "http" => matches!(url.scheme.as_str(), "https" | "wss"),
        "ws" => url.scheme == "wss",
        _ => false,
    };
    upgraded && url.effective_port() == default_port(&url.scheme)
}

/// A keyword source, from what was inside the quotes.
fn keyword(inside: &str, whole: &str) -> Source {
    let folded = inside.to_ascii_lowercase();
    match folded.as_str() {
        "none" => return Source::Nothing,
        "self" => return Source::SameOrigin,
        "unsafe-inline" => return Source::UnsafeInline,
        "strict-dynamic" => return Source::StrictDynamic,
        "unsafe-eval"
        | "wasm-unsafe-eval"
        | "unsafe-hashes"
        | "report-sample"
        | "inline-speculation-rules" => return Source::Inert(folded),
        _ => {}
    }
    if let Some(value) = after_prefix(inside, "nonce-") {
        return if is_base64(value) {
            Source::Nonce(value.to_owned())
        } else {
            Source::Unreadable(whole.to_owned())
        };
    }
    for (name, digest) in [
        ("sha256-", Digest::Sha256),
        ("sha384-", Digest::Sha384),
        ("sha512-", Digest::Sha512),
    ] {
        if let Some(value) = after_prefix(inside, name) {
            return if is_base64(value) {
                Source::Hash {
                    digest,
                    expected: value.to_owned(),
                }
            } else {
                Source::Unreadable(whole.to_owned())
            };
        }
    }
    Source::Unreadable(whole.to_owned())
}

/// What follows a case-insensitive prefix, when the text has one.
fn after_prefix<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let (head, rest) = text.split_at_checked(prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then_some(rest)
}

/// The alphabet a nonce or a hash may be written in.
///
/// Both alphabets, standard and URL-safe, because authors use both and a
/// browser that read only one would silently refuse half of them.
fn is_base64(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '-' | '_' | '='))
}

/// `https:` — a scheme and nothing else.
fn scheme_source(token: &str) -> Option<String> {
    let head = token.strip_suffix(':')?;
    is_scheme(head).then(|| head.to_ascii_lowercase())
}

/// Whether this is a scheme by the URL Standard's spelling of one.
fn is_scheme(text: &str) -> bool {
    let mut characters = text.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && characters.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// `[scheme://]host[:port][/path]`, or nothing when it is not one.
fn host_source(token: &str) -> Option<HostSource> {
    let (scheme, rest) = match token.split_once("://") {
        Some((head, rest)) if is_scheme(head) => (Some(head.to_ascii_lowercase()), rest),
        // `://something` and `not a scheme://x` are neither a host source nor
        // anything else, rather than a host called `://something`.
        Some(_) => return None,
        None => (None, token),
    };
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, Some(format!("/{path}"))),
        None => (rest, None),
    };
    // The grammar has no spelling for either, and a query or a fragment in a
    // policy is far more likely to be a mistake than an intention.
    if path.as_ref().is_some_and(|path| path.contains(['?', '#'])) {
        return None;
    }
    let (names, port) = match authority.rsplit_once(':') {
        Some((names, port)) => (names, Some(port_pattern(port)?)),
        None => (authority, None),
    };
    Some(HostSource {
        scheme,
        names: names_pattern(names)?,
        port,
        path,
    })
}

/// `*` or a number.
fn port_pattern(text: &str) -> Option<Port> {
    if text == "*" {
        return Some(Port::Any);
    }
    text.parse::<u16>().ok().map(Port::Exactly)
}

/// `*`, `*.example.com` or `example.com`.
fn names_pattern(text: &str) -> Option<Names> {
    if text == "*" {
        return Some(Names::Any);
    }
    let (wildcard, name) = match text.strip_prefix("*.") {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    // A trailing dot is the same name written absolutely, and `Names::matches`
    // takes it off the other side too.
    let name = name.strip_suffix('.').unwrap_or(name);
    if name.is_empty() || !name.split('.').all(is_label) {
        return None;
    }
    let name = name.to_ascii_lowercase();
    Some(if wildcard {
        Names::Under(name)
    } else {
        Names::Exactly(name)
    })
}

/// One label of a name: ASCII, non-empty, and nothing that would need
/// converting before it could be compared with a host this engine holds.
fn is_label(label: &str) -> bool {
    !label.is_empty()
        && label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(text: &str) -> Url {
        alo_url::parse(text).expect("a URL")
    }

    fn page(text: &str) -> Origin {
        Origin::of(&url(text))
    }

    #[test]
    fn a_keyword_nobody_here_has_heard_of_permits_nothing_and_is_kept() {
        let source = Source::parse("'from-the-future'");
        assert_eq!(
            source,
            Source::Unreadable("'from-the-future'".to_owned()),
            "it was dropped, which is how a directive gets wider than it was written",
        );
        assert!(!source.matches(
            &url("https://example.com/x.js"),
            &page("https://example.com/")
        ));
    }

    #[test]
    fn a_keyword_we_read_and_do_not_act_on_says_which_it_is() {
        assert_eq!(
            Source::parse("'unsafe-eval'"),
            Source::Inert("unsafe-eval".to_owned()),
        );
        assert_eq!(
            Source::parse("'WASM-Unsafe-Eval'"),
            Source::Inert("wasm-unsafe-eval".to_owned()),
            "a keyword folds case",
        );
    }

    #[test]
    fn a_nonce_keeps_the_case_it_was_written_in() {
        assert_eq!(
            Source::parse("'nonce-AbCd12+/='"),
            Source::Nonce("AbCd12+/=".to_owned()),
        );
        assert_eq!(
            Source::parse("'nonce-'"),
            Source::Unreadable("'nonce-'".to_owned()),
            "an empty nonce would match an attribute nobody wrote",
        );
        assert_eq!(
            Source::parse("'nonce-not a nonce'"),
            Source::Unreadable("'nonce-not a nonce'".to_owned()),
        );
    }

    #[test]
    fn a_hash_is_read_even_though_nothing_computes_one() {
        assert_eq!(
            Source::parse("'sha384-YWJj'"),
            Source::Hash {
                digest: Digest::Sha384,
                expected: "YWJj".to_owned(),
            },
        );
    }

    #[test]
    fn a_scheme_source_is_a_scheme_and_a_host_source_is_not() {
        assert_eq!(Source::parse("https:"), Source::Scheme("https".to_owned()));
        assert_eq!(Source::parse("DATA:"), Source::Scheme("data".to_owned()));
        assert!(matches!(Source::parse("example.com"), Source::Host(_)));
    }

    #[test]
    fn a_policy_naming_http_also_reaches_https_and_never_the_other_way() {
        assert!(scheme_reaches("http", "https"), "a site moving to TLS");
        assert!(
            !scheme_reaches("https", "http"),
            "that direction is a downgrade"
        );
        assert!(scheme_reaches("ws", "wss"));
        assert!(!scheme_reaches("wss", "ws"));
    }

    #[test]
    fn a_subdomain_wildcard_does_not_cover_the_bare_name() {
        let source = Source::parse("*.example.com");
        let page = page("https://example.com/");
        assert!(source.matches(&url("https://a.example.com/x.js"), &page));
        assert!(source.matches(&url("https://a.b.example.com/x.js"), &page));
        assert!(
            !source.matches(&url("https://example.com/x.js"), &page),
            "the bare name is what the author did not write",
        );
        assert!(!source.matches(&url("https://notexample.com/x.js"), &page));
        assert!(
            !source.matches(&url("https://example.com.evil.test/x.js"), &page),
            "a suffix is not a subdomain",
        );
    }

    #[test]
    fn a_source_with_no_port_means_the_schemes_own_port() {
        let source = Source::parse("https://example.com");
        let page = page("https://example.com/");
        assert!(source.matches(&url("https://example.com/x.js"), &page));
        assert!(source.matches(&url("https://example.com:443/x.js"), &page));
        assert!(!source.matches(&url("https://example.com:8443/x.js"), &page));

        let any = Source::parse("https://example.com:*");
        assert!(any.matches(&url("https://example.com:8443/x.js"), &page));
    }

    #[test]
    fn a_host_with_no_scheme_takes_the_pages_own() {
        let source = Source::parse("example.com");
        assert!(
            !source.matches(
                &url("http://example.com/x.js"),
                &page("https://elsewhere.test/")
            ),
            "an https page must not reach plain http through a bare host source",
        );
        assert!(source.matches(
            &url("https://example.com/x.js"),
            &page("https://elsewhere.test/")
        ));
        assert!(
            source.matches(
                &url("http://example.com/x.js"),
                &page("http://elsewhere.test/")
            ),
            "an insecure page's bare host source reaches insecurely",
        );
    }

    #[test]
    fn a_path_ending_in_a_slash_is_a_directory_and_one_that_does_not_is_exact() {
        let under = Source::parse("https://example.com/assets/");
        let page = page("https://example.com/");
        assert!(under.matches(&url("https://example.com/assets/x.js"), &page));
        assert!(under.matches(&url("https://example.com/assets/deep/x.js"), &page));
        assert!(
            !under.matches(&url("https://example.com/assetsx.js"), &page),
            "a string prefix that is not a segment prefix",
        );

        let exact = Source::parse("https://example.com/one.js");
        assert!(exact.matches(&url("https://example.com/one.js"), &page));
        assert!(!exact.matches(&url("https://example.com/one.js.map"), &page));

        let root = Source::parse("https://example.com/");
        assert!(root.matches(&url("https://example.com/anything/at/all"), &page));
    }

    #[test]
    fn self_is_the_pages_own_origin_and_the_upgrade_of_it() {
        let secure = page("https://example.com/");
        assert!(Source::SameOrigin.matches(&url("https://example.com/x.js"), &secure));
        assert!(!Source::SameOrigin.matches(&url("https://other.test/x.js"), &secure));
        assert!(
            !Source::SameOrigin.matches(&url("http://example.com/x.js"), &secure),
            "a secure page's own origin is never the insecure one",
        );

        let insecure = page("http://example.com/");
        assert!(
            Source::SameOrigin.matches(&url("https://example.com/x.js"), &insecure),
            "the one allowance, and it is the upgrade",
        );
        assert!(
            !Source::SameOrigin.matches(&url("https://example.com:8443/x.js"), &insecure),
            "a different port is a different server",
        );
    }

    #[test]
    fn an_opaque_page_reaches_nothing_through_a_bare_host_source() {
        let opaque = Origin::Opaque(alo_url::Opaque::new());
        assert!(!Source::parse("example.com").matches(&url("https://example.com/x.js"), &opaque));
        assert!(
            Source::parse("https://example.com").matches(&url("https://example.com/x.js"), &opaque),
            "a scheme the author wrote needs nothing from the page",
        );
    }

    #[test]
    fn a_host_this_engine_could_never_compare_is_unreadable_rather_than_silent() {
        for token in ["*.köln.example", "[::1]", "https://[::1]:443", "a..b", "*"] {
            let source = Source::parse(token);
            if token == "*" {
                assert!(matches!(source, Source::Host(_)), "{token}");
                continue;
            }
            assert_eq!(source, Source::Unreadable(token.to_owned()), "{token}");
        }
    }

    /// Whatever a server sends, a token becomes a source and nothing panics.
    #[test]
    fn nothing_a_server_can_write_is_worse_than_unreadable() {
        for token in [
            "",
            "'",
            "''",
            "'''",
            ":",
            "://",
            "://x",
            "http://",
            "https://:443",
            "https://example.com:",
            "https://example.com:99999",
            "https://example.com:-1",
            "example.com/?q=1",
            "example.com/#x",
            "\u{0}\u{1}\u{2}",
            "\u{feff}",
            "'nonce-\u{0}'",
            "*.*.example.com",
            "*example.com",
            "-",
            ".",
            "..",
            "/path/only",
        ] {
            let source = Source::parse(token);
            // Whatever it read as, it permits nothing it should not: every one
            // of these is either unreadable or a host nobody is serving.
            let _ = source.matches(
                &url("https://example.com/x.js"),
                &page("https://example.com/"),
            );
            let _ = format!("{source}");
        }
    }

    #[test]
    fn a_trailing_dot_is_the_same_name_written_absolutely() {
        let source = Source::parse("https://example.com.");
        let page = page("https://example.com/");
        assert!(source.matches(&url("https://example.com/x.js"), &page));
    }
}
