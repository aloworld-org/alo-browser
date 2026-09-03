/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a Content Security Policy stops, named by the attack rather than by
//! the header.
//!
//! The same rule items 61 and 62's tests were written under, and it matters
//! more here than anywhere: a file of `script_src_is_parsed` tests passes
//! against an implementation that drops a directive it cannot read — which is
//! the one thing a policy must never do, because the page that wrote it is
//! usually protecting itself from a bug it has not found yet.

use alo_net::csp::{Content, Inline, Policies};
use alo_net::{Headers, Purpose, Request};
use alo_url::Origin;

/// The page every test below is on.
const PAGE: &str = "https://shop.example.com/checkout";

fn url(text: &str) -> alo_url::Url {
    alo_url::parse(text).unwrap_or_else(|_| alo_url::Url {
        scheme: "about".to_owned(),
        host: None,
        port: None,
        path: "not-a-url".to_owned(),
        query: None,
        fragment: None,
        serialised: "about:not-a-url".to_owned(),
    })
}

/// A request the page made, as the page.
fn wants(target: &str, purpose: Purpose) -> Request {
    Request::get(url(target))
        .for_purpose(purpose)
        .asked_by(Origin::of(&url(PAGE)))
}

fn enforcing(value: &str) -> Policies {
    let mut headers = Headers::new();
    headers.add("Content-Security-Policy", value);
    Policies::stated_by(&headers)
}

/// The whole point of the feature: the page's escaping failed somewhere, a
/// `<script src>` an attacker chose is now in the markup, and it does not load.
#[test]
fn a_script_tag_an_attacker_injected_does_not_load() {
    let policies = enforcing("default-src 'self'; script-src 'self' https://cdn.example.com");

    for allowed in [
        "https://shop.example.com/app.js",
        "https://cdn.example.com/vendor.js",
    ] {
        assert!(
            policies
                .allows(&wants(allowed, Purpose::Script), None)
                .is_ok(),
            "{allowed} is what the page actually ships",
        );
    }

    let refused = policies
        .allows(&wants("https://evil.test/steal.js", Purpose::Script), None)
        .expect_err("the injected script loaded");
    let said = refused.to_string();
    assert!(said.contains("script-src"), "{said}");
    assert!(said.contains("evil.test"), "{said}");
}

/// The rule that matters more than any single directive. A source expression
/// from a version of the specification this engine has not read must not take
/// the directive holding it down with it.
#[test]
fn a_word_this_engine_cannot_read_never_widens_the_sentence_it_is_in() {
    let policies = enforcing("script-src 'self' 'trusted-types-eval-from-2027'");

    assert!(
        policies
            .allows(&wants("https://evil.test/steal.js", Purpose::Script), None)
            .is_err(),
        "the directive was discarded, so scripts fell through to nothing at all",
    );
    assert!(
        policies
            .allows(
                &wants("https://shop.example.com/app.js", Purpose::Script),
                None
            )
            .is_ok(),
        "and the half we could read still has to work",
    );

    // And the author is told which word, because a page that stopped working
    // deserves better than a bare refusal.
    let said = policies
        .allows(&wants("https://evil.test/steal.js", Purpose::Script), None)
        .expect_err("allowed")
        .to_string();
    assert!(said.contains("'trusted-types-eval-from-2027'"), "{said}");
}

/// A directive made entirely of words we cannot read allows nothing, rather
/// than being an absent directive that falls back to something wider.
#[test]
fn a_sentence_made_only_of_words_we_cannot_read_allows_nothing() {
    let policies = enforcing("default-src *; script-src 'from-the-future'");
    assert!(
        policies
            .allows(&wants("https://evil.test/steal.js", Purpose::Script), None)
            .is_err(),
        "script-src vanished and default-src * let everything in",
    );
}

/// A reflected header, a misconfigured proxy: anybody who can append to the
/// header would otherwise restate a directive and widen it.
#[test]
fn appending_to_the_header_cannot_widen_a_policy() {
    let appended = enforcing("script-src 'self'; script-src https://evil.test");
    assert!(
        appended
            .allows(&wants("https://evil.test/steal.js", Purpose::Script), None)
            .is_err(),
        "the second script-src replaced the first",
    );

    // And a whole second policy, which is the other way to append one.
    let mut headers = Headers::new();
    headers.add("Content-Security-Policy", "script-src 'self'");
    headers.add("Content-Security-Policy", "script-src https://evil.test");
    let two = Policies::stated_by(&headers);
    assert!(
        two.allows(&wants("https://evil.test/steal.js", Purpose::Script), None)
            .is_err(),
        "two policies are an intersection, never a union",
    );
    assert!(
        two.allows(
            &wants("https://shop.example.com/app.js", Purpose::Script),
            None
        )
        .is_err(),
        "and the first policy's own script is refused by the second, which is \
         what an intersection means",
    );
}

/// A site deploying a policy watches for a week before it enforces. Enforcing
/// the watching header would break exactly the sites being careful.
#[test]
fn a_policy_a_site_is_only_watching_blocks_nothing() {
    let mut headers = Headers::new();
    headers.add(
        "Content-Security-Policy-Report-Only",
        "default-src 'none'; script-src 'none'",
    );
    let policies = Policies::stated_by(&headers);
    let request = wants("https://evil.test/steal.js", Purpose::Script);

    assert!(policies.allows(&request, None).is_ok());
    assert_eq!(
        policies.violations(&request, None).len(),
        1,
        "and it noticed, which is what gets reported",
    );
}

/// Both headers at once, which is how a site tightens a policy: one enforced,
/// one being tried out. Only the enforced one may block.
#[test]
fn enforcing_one_policy_while_watching_a_stricter_one() {
    let mut headers = Headers::new();
    headers.add(
        "Content-Security-Policy",
        "script-src 'self' 'unsafe-inline'",
    );
    headers.add("Content-Security-Policy-Report-Only", "script-src 'self'");
    let policies = Policies::stated_by(&headers);

    assert!(
        policies
            .allows_inline(Inline::Script, None, Content::element("go()"))
            .is_ok(),
        "the enforced policy still allows what it allows",
    );
    assert_eq!(policies.len(), 2);
}

/// A nonce is a secret the page minted for this response. Guessing it, or
/// getting its case wrong, is not knowing it.
#[test]
fn a_nonce_an_attacker_did_not_see_does_not_get_a_script_in() {
    let policies = enforcing("script-src 'nonce-Kj9fL2mQ'");
    let request = wants("https://cdn.example.com/vendor.js", Purpose::Script);

    assert!(policies.allows(&request, Some("Kj9fL2mQ")).is_ok());
    for wrong in ["kj9fl2mq", "Kj9fL2mQx", "", "Kj9fL2m"] {
        assert!(
            policies.allows(&request, Some(wrong)).is_err(),
            "{wrong:?} got in",
        );
    }
    assert!(
        policies.allows(&request, None).is_err(),
        "an injected tag has no nonce at all, which is the case that matters",
    );
}

/// `'unsafe-inline'` is left in a policy for browsers that predate nonces. A
/// browser that honoured both would undo the protection the nonce was added
/// for, on every site that did the recommended thing.
#[test]
fn the_backwards_compatible_keyword_does_not_undo_the_nonce_beside_it() {
    let policies = enforcing("script-src 'unsafe-inline' 'nonce-Kj9fL2mQ'");

    assert!(
        policies
            .allows_inline(Inline::Script, None, Content::element("steal()"))
            .is_err(),
        "an injected inline script ran",
    );
    assert!(
        policies
            .allows_inline(Inline::Script, Some("Kj9fL2mQ"), Content::element("go()"))
            .is_ok(),
        "and the page's own inline script still runs",
    );
}

/// A subdomain wildcard names subdomains. An attacker who can serve
/// `example.com.evil.test` — or who took over nothing at all — must not be
/// inside it.
#[test]
fn a_wildcard_over_subdomains_is_not_a_wildcard_over_lookalikes() {
    let policies = enforcing("script-src https://*.example.com");

    assert!(
        policies
            .allows(
                &wants("https://cdn.example.com/vendor.js", Purpose::Script),
                None
            )
            .is_ok()
    );
    for lookalike in [
        "https://example.com.evil.test/steal.js",
        "https://notexample.com/steal.js",
        "https://example.com/app.js",
    ] {
        assert!(
            policies
                .allows(&wants(lookalike, Purpose::Script), None)
                .is_err(),
            "{lookalike} was read as a subdomain",
        );
    }
}

/// A policy naming `https` must never be satisfied by plain HTTP: the whole
/// value of naming the scheme is that somebody on the network cannot answer.
#[test]
fn a_policy_that_named_tls_is_not_satisfied_without_it() {
    let policies = enforcing("script-src https://cdn.example.com");
    assert!(
        policies
            .allows(
                &wants("http://cdn.example.com/vendor.js", Purpose::Script),
                None
            )
            .is_err(),
        "a downgrade satisfied a policy that named https",
    );
}

/// `'strict-dynamic'` is an author saying *ignore every host I listed*. A
/// browser that read the keyword and kept honouring the hosts would leave the
/// compromised CDN in the list as a way in.
#[test]
fn strict_dynamic_means_the_listed_cdn_is_not_a_way_in() {
    let policies =
        enforcing("script-src 'strict-dynamic' 'nonce-Kj9fL2mQ' https://cdn.example.com 'self'");

    assert!(
        policies
            .allows(
                &wants("https://cdn.example.com/vendor.js", Purpose::Script),
                None
            )
            .is_err(),
        "the host the author told us to ignore let a script in",
    );
    assert!(
        policies
            .allows(
                &wants("https://cdn.example.com/vendor.js", Purpose::Script),
                Some("Kj9fL2mQ")
            )
            .is_ok(),
    );
}

/// A policy full of directives this engine does not act on is a policy that
/// protects less than it says. That is a thing to print, not a thing to leave
/// somebody to discover.
#[test]
fn a_page_protected_in_four_respects_and_not_a_fifth_says_which() {
    let policies = enforcing(
        "default-src 'self'; frame-ancestors 'none'; base-uri 'none'; \
         form-action 'self'; report-uri /csp",
    );

    assert_eq!(
        policies.not_enforced(),
        vec![
            "base-uri".to_owned(),
            "form-action".to_owned(),
            "frame-ancestors".to_owned(),
        ],
        "a gap nobody prints is a false sense of security",
    );
    assert!(
        policies
            .allows(&wants("https://evil.test/steal.js", Purpose::Script), None)
            .is_err(),
        "and the four it does act on still act",
    );
}

/// Every purpose this engine has, under one `default-src`.
#[test]
fn default_src_reaches_every_kind_of_load_a_page_makes() {
    let policies = enforcing("default-src 'self'");
    for purpose in [
        Purpose::Script,
        Purpose::Style,
        Purpose::Image,
        Purpose::Fetch,
    ] {
        assert!(
            policies
                .allows(&wants("https://evil.test/thing", purpose.clone()), None)
                .is_err(),
            "a {purpose} from another origin was allowed",
        );
        assert!(
            policies
                .allows(
                    &wants("https://shop.example.com/thing", purpose.clone()),
                    None
                )
                .is_ok(),
            "a {purpose} from the page's own origin was refused",
        );
    }
}

/// Whatever a server sends, a policy is read, and reading it is never worse
/// than refusing more than it asked for. `LOOP.md`'s hostile-input clause: the
/// bytes here came from a stranger.
#[test]
fn a_policy_no_server_should_have_sent_is_read_rather_than_believed() {
    for value in [
        "",
        ";",
        ",;,;,",
        "default-src",
        "default-src 'none' 'none' 'none'",
        "default-src 'self",
        "default-src self'",
        "default-src ''''''",
        "default-src https://",
        "default-src https://:::::",
        "default-src *.*.*",
        "default-src 'nonce-'",
        "default-src 'sha256-'",
        "default-src 'sha999-YWJj'",
        "default-src \u{0}\u{1}\u{2}",
        "default-src \u{feff}'self'",
        "script-src 'self'; script-src",
        &format!("default-src {}", "a".repeat(4000)),
        &format!("default-src {}", "https://a.test ".repeat(400)),
    ] {
        let policies = enforcing(value);
        // The only outcome that would be a bug is a policy that was stated and
        // then let a stranger's script in.
        if !policies.is_empty() {
            assert!(
                policies
                    .allows(&wants("https://evil.test/steal.js", Purpose::Script), None)
                    .is_err(),
                "{value:?} was read as a policy that permits a stranger's script",
            );
        }
        let _ = policies.not_enforced();
        let _ = policies.allows_inline(Inline::Style, None, Content::element("a { color: red }"));
    }
}
