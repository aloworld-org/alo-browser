/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Inline content a policy allows by naming its digest, named by the attack
//! rather than by the header — the rule `a_policy_a_page_states.rs` is written
//! under, for the same reason.
//!
//! A hash is the sentence a site writes when its inline content never changes:
//! *this exact stylesheet, and nothing else*. It protects against the injection
//! a nonce protects against, and without needing anything generated per
//! response — which is why a site that serves the same HTML from a cache to a
//! million people can use it and cannot use a nonce.
//!
//! Every digest here was produced by `python3 -c 'import hashlib, base64'`
//! rather than by the crate that computes one in `alo_net::digest`, because a
//! vector produced by the thing under test is a vector that agrees with itself.

use alo_net::Headers;
use alo_net::csp::{Content, Inline, Policies};

/// The page's own inline stylesheet, exactly as it is in the markup.
const STYLE: &str = ".banner { display: none }";

/// Its SHA-256, in the standard alphabet.
const STYLE_SHA256: &str = "'sha256-Hb4Z0rjzh3AGKq/nNI5skfAzg2jXOswTVhbbhkzcoXA='";

/// The same digest in the URL-safe alphabet: `/` written as `_`.
const STYLE_SHA256_URL_SAFE: &str = "'sha256-Hb4Z0rjzh3AGKq_nNI5skfAzg2jXOswTVhbbhkzcoXA='";

/// And its SHA-512, which has two `/` in it — so a value with one of them
/// swapped is a genuine mixture of the two alphabets rather than a value that
/// is simply the wrong length.
const STYLE_SHA512: &str = concat!(
    "yoycc33SUKl5WlTIeXskKRfhCy8wq7VUoa/9zL5tbZuRcvbhaa/",
    "VljzODSmZnt7RNfLwVN5565an1vWyOtnXlQ==",
);

/// A `style` attribute the page wrote — content with no element of its own,
/// which is what `'unsafe-hashes'` is about.
const ATTRIBUTE: &str = "display: none";

/// Its SHA-256.
const ATTRIBUTE_SHA256: &str = "'sha256-ZdHxw9eWtnxUb3mk6tBS+gIiVUPE3pGM470keHPDFlE='";

fn enforcing(value: &str) -> Policies {
    let mut headers = Headers::new();
    headers.add("Content-Security-Policy", value);
    Policies::stated_by(&headers)
}

/// The whole point: the page's own `<style>` runs, and the one an attacker got
/// into the markup does not — with no `'unsafe-inline'` anywhere, which is the
/// keyword that would have let both in.
#[test]
fn the_stylesheet_the_policy_names_is_the_only_one_that_applies() {
    let policies = enforcing(&format!("style-src {STYLE_SHA256}"));

    assert!(
        policies
            .allows_inline(Inline::Style, None, Content::element(STYLE))
            .is_ok(),
        "the page's own stylesheet was refused by its own policy",
    );

    for injected in [
        "body { background: url(https://evil.test/?c=) }",
        // The same rule, one space longer. A digest is over bytes, and an
        // attacker who can add a byte cannot keep the digest.
        ".banner { display: none } ",
        "",
    ] {
        assert!(
            policies
                .allows_inline(Inline::Style, None, Content::element(injected))
                .is_err(),
            "{injected:?} applied under a policy that names one stylesheet",
        );
    }
}

/// Both alphabets, because authors use both — a digest is often copied out of
/// a browser's console, and browsers print both. Reading only one would refuse
/// half the policies that were written correctly.
#[test]
fn a_digest_in_either_alphabet_is_the_same_digest() {
    for spelling in [STYLE_SHA256, STYLE_SHA256_URL_SAFE] {
        let policies = enforcing(&format!("style-src {spelling}"));
        assert!(
            policies
                .allows_inline(Inline::Style, None, Content::element(STYLE))
                .is_ok(),
            "{spelling} did not allow the content it is the digest of",
        );
    }
}

/// A policy may name more than one, which is what a page with several inline
/// blocks does. Each one gets in on its own digest and on nobody else's.
#[test]
fn one_of_several_hashes_is_enough_and_only_for_its_own_content() {
    let other = "console.log(1)";
    let policies = enforcing(concat!(
        "script-src 'sha256-bhHHL3z2vDgxUt0W3dWQOrprscmda2Y5pLsLg4GF+pI=' ",
        "'sha384-HT2E9NfWiuQ/w1PRai+hTyqW16NIoCGA/m8VQDUopfAtcz6YQjtsMmQd5uRbVDpW'",
    ));
    assert!(
        policies
            .allows_inline(Inline::Script, None, Content::element("alert(1)"))
            .is_ok(),
        "the SHA-256 of it is in the policy twice over",
    );
    assert!(
        policies
            .allows_inline(Inline::Script, None, Content::element(other))
            .is_err(),
        "{other:?} ran under a policy naming two digests, neither of them its own",
    );
}

/// The specification's rule, and the reason for it: a directive that names a
/// hash has an author who has moved past blanket inline content, so the keyword
/// left in for older browsers must not undo it.
#[test]
fn the_backwards_compatible_keyword_does_not_undo_the_hash_beside_it() {
    let policies = enforcing(&format!("style-src 'unsafe-inline' {STYLE_SHA256}"));
    assert!(
        policies
            .allows_inline(
                Inline::Style,
                None,
                Content::element("body { background: red }")
            )
            .is_err(),
        "an injected stylesheet applied",
    );
    assert!(
        policies
            .allows_inline(Inline::Style, None, Content::element(STYLE))
            .is_ok(),
        "and the page's own still applies",
    );
}

/// A policy the author got wrong — the digest of a stylesheet they have since
/// edited — is content that does not apply, and a message that says which of
/// the two kinds of wrong it is. The alternative is an author reading
/// "refused" and looking for the wrong bug.
#[test]
fn a_refusal_says_whether_a_hash_was_the_reason() {
    let policies = enforcing(&format!("style-src {STYLE_SHA256}"));

    let stale = policies
        .allows_inline(
            Inline::Style,
            None,
            Content::element("\n  .banner { display: none }\n"),
        )
        .expect_err("a stylesheet the policy does not name applied");
    let said = stale.to_string();
    assert!(said.contains("byte for byte"), "{said}");

    // A `style` attribute has no element of its own, so a hash reaches it only
    // under `'unsafe-hashes'`. This policy has none, so the refusal is the
    // third kind: not *your digest is wrong* but *no digest applies here*.
    let attribute = policies
        .allows_inline(Inline::Style, None, Content::attribute(STYLE))
        .expect_err("a hash reached a style attribute under a policy that did not allow it to");
    let said = attribute.to_string();
    assert!(said.contains("'unsafe-hashes'"), "{said}");
    assert!(said.contains("a style attribute"), "{said}");
    assert!(
        !said.contains("byte for byte"),
        "it was reported as a digest that did not match, and this one did: {said}",
    );

    // And a policy with no hash in it says nothing about hashes at all.
    let plain = enforcing("style-src 'self'")
        .allows_inline(Inline::Style, None, Content::element(STYLE))
        .expect_err("'self' allowed inline style");
    let said = plain.to_string();
    assert!(!said.contains("hash"), "{said}");
}

/// The item's own condition, and the half that matters is the second one: the
/// digest is the same in both policies and the keyword is the whole difference.
/// `'unsafe-hashes'` exists so that allowing a `style` attribute by its digest
/// is something an author asked for in words — a `style` attribute is the shape
/// an injection most often takes, and inheriting the permission from a hash
/// written for a `<style>` element would be this engine widening a policy.
#[test]
fn a_style_attribute_needs_the_keyword_as_well_as_the_digest() {
    let without = enforcing(&format!("style-src {ATTRIBUTE_SHA256}"));
    assert!(
        without
            .allows_inline(Inline::Style, None, Content::attribute(ATTRIBUTE))
            .is_err(),
        "a style attribute applied by its digest under a policy that never said so",
    );

    let with = enforcing(&format!("style-src 'unsafe-hashes' {ATTRIBUTE_SHA256}"));
    assert!(
        with.allows_inline(Inline::Style, None, Content::attribute(ATTRIBUTE))
            .is_ok(),
        "the author allowed this attribute deliberately and it was refused anyway",
    );
}

/// The keyword is a permission to *hash*, never a permission. On its own it
/// allows nothing, and beside a hash it allows that one attribute and no other
/// — which is what stops it being `'unsafe-inline'` spelt differently.
#[test]
fn the_keyword_on_its_own_allows_no_attribute_at_all() {
    assert!(
        enforcing("style-src 'unsafe-hashes'")
            .allows_inline(Inline::Style, None, Content::attribute(ATTRIBUTE))
            .is_err(),
        "the keyword allowed an attribute with no digest beside it",
    );

    let policies = enforcing(&format!("style-src 'unsafe-hashes' {ATTRIBUTE_SHA256}"));
    for injected in ["color: red", "display: none ", ""] {
        assert!(
            policies
                .allows_inline(Inline::Style, None, Content::attribute(injected))
                .is_err(),
            "{injected:?} applied under a policy naming one attribute's digest",
        );
    }
}

/// `'unsafe-hashes'` is a source expression, so the list that decides is the
/// **deciding directive's own**. One written into `default-src` while
/// `style-src` decides is not this policy allowing an attribute, and reading it
/// as though it were would let a keyword in one sentence widen another — the
/// same rule the repeated-directive one is written for.
#[test]
fn the_keyword_does_not_reach_out_of_the_directive_it_was_written_in() {
    let elsewhere = enforcing(&format!(
        "default-src 'unsafe-hashes'; style-src {ATTRIBUTE_SHA256}"
    ));
    assert!(
        elsewhere
            .allows_inline(Inline::Style, None, Content::attribute(ATTRIBUTE))
            .is_err(),
        "a keyword in default-src allowed an attribute that style-src decided about",
    );

    // And two policies are an intersection, so a second header cannot add the
    // keyword to the first one's hash.
    let mut headers = Headers::new();
    headers.add(
        "Content-Security-Policy",
        format!("style-src {ATTRIBUTE_SHA256}"),
    );
    headers.add("Content-Security-Policy", "style-src 'unsafe-hashes'");
    assert!(
        Policies::stated_by(&headers)
            .allows_inline(Inline::Style, None, Content::attribute(ATTRIBUTE))
            .is_err(),
        "a second policy widened the first one",
    );
}

/// The keyword changes nothing for content that has an element of its own: a
/// `<style>` was already allowed by its digest, and adding the keyword neither
/// grants anything more nor takes the hash away.
#[test]
fn the_keyword_changes_nothing_for_a_style_element() {
    for policy in [
        format!("style-src {STYLE_SHA256}"),
        format!("style-src 'unsafe-hashes' {STYLE_SHA256}"),
    ] {
        let policies = enforcing(&policy);
        assert!(
            policies
                .allows_inline(Inline::Style, None, Content::element(STYLE))
                .is_ok(),
            "{policy} refused the stylesheet it names",
        );
        assert!(
            policies
                .allows_inline(Inline::Style, None, Content::element("body { color: red }"))
                .is_err(),
            "{policy} allowed a stylesheet it does not name",
        );
    }
}

/// A policy is a stranger's bytes, and a hash source is the part of one most
/// likely to have been mangled by somebody's build step. Each row says whether
/// the stylesheet applies under it and why — a table rather than a list of
/// things that must fail, because four of these are legal and saying which is
/// the point.
#[test]
fn every_spelling_of_a_hash_source_a_server_may_send() {
    let digest = "Hb4Z0rjzh3AGKq/nNI5skfAzg2jXOswTVhbbhkzcoXA=";
    for (source, applies, why) in [
        (format!("'sha256-{digest}'"), true, "the digest, as written"),
        (
            format!("'SHA256-{digest}'"),
            true,
            "a keyword folds case, and the name of a digest is a keyword",
        ),
        (
            "'sha256-Hb4Z0rjzh3AGKq/nNI5skfAzg2jXOswTVhbbhkzcoXA'".to_owned(),
            true,
            "the same bytes with the padding left off, which some tools do",
        ),
        (
            format!("'sha512-{STYLE_SHA512}'"),
            true,
            "the same stylesheet, named by a longer digest",
        ),
        (
            format!("'sha512-{}'", STYLE_SHA512.replacen('/', "_", 1)),
            false,
            "both alphabets in one value, which no encoder produces",
        ),
        (
            "'sha256-Hb4Z0rjzh3AGKq/nNI5skfAzg2jXOswTVhbbhkzcoXB='".to_owned(),
            false,
            "the same thirty-two bytes with rubbish in the bits that stand for none \
             of them — a second spelling of one permission",
        ),
        (
            "'sha256-Hb4Z0rjzh3AGKq/nNI5skfAzg2jXOswTVhbbhkzcoXA=='".to_owned(),
            false,
            "padded to no multiple of four",
        ),
        (
            format!("'sha512-{digest}'"),
            false,
            "a SHA-256 value labelled as a SHA-512, which is the wrong length",
        ),
        (
            format!("'sha1-{digest}'"),
            false,
            "an algorithm CSP does not have, and one nobody should trust",
        ),
        (
            format!("'sha256 {digest}'"),
            false,
            "a space where the hyphen goes: not a hash source at all",
        ),
        ("'sha256-'".to_owned(), false, "nothing after the name"),
        ("'sha256-=='".to_owned(), false, "padding and no value"),
        ("'sha256-\u{0}'".to_owned(), false, "a control character"),
        ("'sha256-héllo'".to_owned(), false, "not ASCII"),
    ] {
        let policies = enforcing(&format!("style-src {source}"));
        let allowed = policies
            .allows_inline(Inline::Style, None, Content::element(STYLE))
            .is_ok();
        assert_eq!(allowed, applies, "{source} — {why}");
    }
}
