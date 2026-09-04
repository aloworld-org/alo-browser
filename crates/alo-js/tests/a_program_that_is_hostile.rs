/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A parser is handed a stranger's bytes, and it never stops.
//!
//! ADR 0013 § 4, and `LOOP.md`'s stage 2 clause 2: anything that reads bytes
//! from outside gets a test that feeds it malformed, truncated and adversarial
//! input, and it must **return an error rather than panicking**. A refusal is a
//! result; a renderer that aborts is a denial of service.
//!
//! # The shape rather than the list
//!
//! A list of nasty cases finds what somebody thought of. So the same thing the
//! lexer's hostile test does is done here: a nasty corpus is **cut at every
//! character boundary, from both ends**, and every cut is parsed. It is
//! deterministic, so a failure is reproducible in a way a random fuzzer's is
//! not, and a truncation is the malformed input a network actually produces.
//!
//! # And the one a parser has that a lexer does not
//!
//! Recursion. A lexer of a million open brackets makes a million tokens and no
//! stack at all; a parser of the same file is a million frames deep and is an
//! abort rather than a refusal. [`alo_js::bounds::DEEPEST_NESTING`] is the
//! ceiling, and the test below stands either side of it — because a bound that
//! is only ever tested from one side is a bound nobody has checked is
//! reachable.
//!
//! That it is reachable *here* is the point of
//! [`alo_js::bounds::STACK_FOR_A_PARSE`]. A `cargo test` thread has two
//! mebibytes, which is a quarter of what this file's deepest case needs, so
//! without a stack of the parser's own this test would be measuring the test
//! harness rather than the parser.

use alo_js::bounds::DEEPEST_NESTING;
use alo_js::{Reason, Source, module, script};

/// Programs whose shapes are the ones a parser is most likely to trip on.
const NASTY: &[&str] = &[
    "((((((((((a))))))))))",
    "a => b => c => d",
    "`${`${`${a}`}`}`",
    "class A extends B { static { #a in this } #a; }",
    "for (const [a, { b: [c] }] of d) e;",
    "a?.b?.(c)?.[d]",
    "({ a = 1 } = b)",
    "function* a() { yield* await b; }",
    "async (a = (b, c)) => { await d; }",
    "label: for (;;) { continue label; }",
    "switch (a) { case b: default: c; }",
    "try { a } catch { b } finally { c }",
    "/a[/]b/gu.test(c) / 2",
    "a = { get b() {}, set b(c) {}, *d() {}, async e() {}, ...f }",
    "new new a.b()()",
    "import(\"a\", { with: { type: \"json\" } })",
    "let a = 0755;",
    "\"\\u{110000}\"",
    "`\\unicode`",
    "#!/usr/bin/env node\na;",
];

/// Every prefix and every suffix of a piece of source text, cut only where a
/// character ends.
fn every_cut(source: &str) -> Vec<&str> {
    let mut cuts = Vec::new();
    for (at, _) in source.char_indices() {
        if let Some(prefix) = source.get(..at) {
            cuts.push(prefix);
        }
        if let Some(suffix) = source.get(at..) {
            cuts.push(suffix);
        }
    }
    cuts.push(source);
    cuts
}

#[test]
fn no_cut_of_anything_nasty_stops_the_parser() {
    let mut cuts = 0;
    for source in NASTY {
        for cut in every_cut(source) {
            // Both goals, because they are different grammars: `await a` is a
            // name and a call in one and an operator in the other.
            let _ = script(cut);
            let _ = module(cut);
            cuts += 1;
        }
    }
    // The corpus is what it is; the number is here so that a case deleted by
    // accident shows up as a failure rather than as a quieter test.
    assert_eq!(cuts, 1074);
}

#[test]
fn every_character_below_the_basic_plane_is_refused_or_read_and_never_more() {
    // The lexer's own hostile test walks these as tokens. Here each one is
    // asked to be a whole program, which is where a parser that assumed a
    // token kind it never checked would fall over.
    for code in 0u32..=0xFFFF {
        let Some(character) = char::from_u32(code) else {
            continue;
        };
        let text = character.to_string();
        let _ = script(&text);
        let _ = script(&format!("a {text} b"));
        let _ = script(&format!("a.{text}"));
    }
}

#[test]
fn a_program_at_the_ceiling_is_read_and_one_past_it_is_refused() {
    // A statement is a level of its own, so a program of *n* brackets is n + 1
    // levels deep — which is why the case that reads is two short of the
    // ceiling rather than one.
    for (depth, refused) in [(DEEPEST_NESTING - 2, false), (DEEPEST_NESTING, true)] {
        let source = format!("{}a{}", "[".repeat(depth), "]".repeat(depth));
        match script(&source) {
            Ok(_) => assert!(!refused, "{depth} deep was read"),
            Err(error) => {
                assert!(refused, "{depth} deep was refused: {error}");
                assert_eq!(
                    error.reason,
                    Reason::TooDeeplyNested {
                        most: DEEPEST_NESTING
                    }
                );
            }
        }
    }
}

#[test]
fn twenty_thousand_open_brackets_is_a_refusal_rather_than_an_abort() {
    // The case `bounds.rs` names: four bytes of somebody else's file, and a
    // stack overflow in any parser that does not count.
    for opener in ["[", "(", "{", "[{", "((("] {
        let source = opener.repeat(20_000);
        let error = script(&source).expect_err("twenty thousand of anything is too deep");
        assert_eq!(
            error.reason,
            Reason::TooDeeplyNested {
                most: DEEPEST_NESTING
            },
            "for {opener:?}"
        );
    }
}

#[test]
fn a_source_longer_than_the_ceiling_is_refused_before_anything_is_read() {
    let source = "a;".repeat(alo_js::bounds::LONGEST_SOURCE / 2 + 1);
    let error = alo_js::Parser::new(&source, Source::Script).expect_err("too long");
    assert!(matches!(error.reason, Reason::SourceTooLong { .. }));
}

#[test]
fn a_program_that_is_only_a_refusal_still_says_where() {
    let error = script("a b;").expect_err("`a b` is not a program");
    let at = alo_js::Position::of("a b;", error.at);
    assert_eq!((at.line, at.column), (1, 3));
}
