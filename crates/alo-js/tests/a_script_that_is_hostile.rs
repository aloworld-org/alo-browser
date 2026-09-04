/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Source text that is trying to stop us, and does not.
//!
//! `LOOP.md`'s stage 2 clause 2 and ADR 0013 § 4: a script is a stranger's
//! bytes that we then execute, so anything that reads them gets a test that
//! feeds it malformed, truncated and adversarial input and a guarantee that it
//! **returns an error rather than panicking**. A refusal is a result; a crash
//! in a renderer is a denial of service, and this crate is the largest piece of
//! attack surface in the browser.
//!
//! # Why the shape is "every prefix" rather than a list
//!
//! A lexer's bugs are almost all at the end of the source: a `\u{` with the
//! file finishing inside it, a `/*` that never closes, a `\` as the last byte.
//! A list of cases finds the ones somebody thought of. Cutting a nasty corpus
//! at **every character boundary** finds the ones nobody did, and it is
//! deterministic — the same cuts every run, so a failure is reproducible in a
//! way a random fuzzer's is not.

use alo_js::lexer::Goal;
use alo_js::token::Kind;
use alo_js::{Lexer, Reason};

/// Source with something awkward in every one of them.
const NASTY: &[&str] = &[
    r"'\u{10FFFF}\u{110000}𐀀\x4'",
    r"`a${b}c${`nested ${d}`}e`",
    r"/[/]\/(?<n>x)/dgimsy",
    "0x1fffffffffffffffffffffffffffffff 0b1_0 0o7_7 1_000.5e-1_0 9n",
    "#a.#b class{#c(){}}",
    "a?.5:b a?.b a??=c a>>>=d ...e =>f",
    "\u{feff}\u{2028}\u{2029}\u{00a0}\u{3000}x",
    "/* /* not nested */ // \n `\r\n\r`",
    r"if \u{61}bc $_ é ⅷ",
    "#!/usr/bin/env node\n1e400 .5 1. 0.0",
    "'\\\r\n' '\\\r' '\\\u{2028}'",
    "((((((((((((((((((((((((((((((((((((((((",
    "'''''''''''''''''''''''''''''''''''''''",
    "\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\\",
    "````````````````````````````````````````",
    "${${${${${${${${${${${${${${${${${${${${",
    "\0\u{1}\u{7f}\u{fffd}",
];

/// Read `source` to the end or to a refusal, answering how many tokens it made.
///
/// The point is that it **returns**: a panic is the failure this whole file
/// exists to rule out, and a loop that did not end would be one too.
fn read_to_the_end(source: &str, goal: Goal) -> usize {
    let Ok(mut lexer) = Lexer::new(source) else {
        return 0;
    };
    let mut tokens = 0;
    let mut reached = lexer.offset();
    loop {
        match lexer.next(goal) {
            Ok(token) if token.kind == Kind::End => return tokens,
            Ok(_) => {
                tokens += 1;
                assert!(
                    lexer.offset() > reached,
                    "the lexer made no progress on {source:?} at {reached}: a \
                     token that consumed nothing loops for ever on a page"
                );
                reached = lexer.offset();
            }
            Err(_) => return tokens,
        }
    }
}

#[test]
fn every_cut_of_every_nasty_source_returns() {
    for source in NASTY {
        for cut in 0..=source.len() {
            let Some(prefix) = source.get(..cut) else {
                continue; // Not a character boundary.
            };
            for goal in [Goal::Division, Goal::RegularExpression] {
                read_to_the_end(prefix, goal);
            }
        }
    }
}

#[test]
fn every_cut_of_every_nasty_source_returns_from_the_far_end_too() {
    // The other half of the same idea: a source that *begins* in the middle of
    // something is what a caller gets when it restarts after a refusal.
    for source in NASTY {
        for cut in 0..=source.len() {
            let Some(suffix) = source.get(cut..) else {
                continue;
            };
            for goal in [Goal::Division, Goal::RegularExpression] {
                read_to_the_end(suffix, goal);
            }
        }
    }
}

#[test]
fn every_single_character_is_either_a_token_or_a_refusal() {
    // Every code point up to the end of the Basic Multilingual Plane, one at a
    // time. Most are refusals; what matters is that none of them is a stop.
    for code in 0u32..=0xFFFF {
        let Some(c) = char::from_u32(code) else {
            continue; // A surrogate, which cannot be in Rust source text.
        };
        let source = c.to_string();
        read_to_the_end(&source, Goal::Division);
        read_to_the_end(&source, Goal::RegularExpression);
    }
}

#[test]
fn a_source_longer_than_the_bound_is_refused_before_anything_is_read() {
    let too_long = "a".repeat(alo_js::bounds::LONGEST_SOURCE + 1);
    let error = Lexer::new(&too_long).expect_err("past the bound");
    assert_eq!(
        error.reason,
        Reason::SourceTooLong {
            bytes: alo_js::bounds::LONGEST_SOURCE + 1,
            most: alo_js::bounds::LONGEST_SOURCE,
        }
    );
    assert_eq!(error.at, 0);
}

#[test]
fn a_million_open_brackets_is_a_token_stream_rather_than_a_stack() {
    // The lexer has no nesting and therefore no depth bound: a script of a
    // million open brackets is a million tokens and no recursion. That is why
    // `bounds` has one constant rather than two — the depth bound belongs to
    // the parser, which is the thing that recurses.
    let brackets = "(".repeat(1_000_000);
    assert_eq!(read_to_the_end(&brackets, Goal::Division), 1_000_000);
}

#[test]
fn a_deeply_nested_escape_is_a_refusal_rather_than_an_overflow() {
    // Leading zeros in a `\u{…}` are legal and unbounded, so a value that would
    // overflow a `u32` on the way to being refused is the arithmetic the lints
    // do not catch.
    let zeros = "0".repeat(100_000);
    let source = format!("'\\u{{{zeros}110000}}'");
    let mut lexer = Lexer::new(&source).expect("a lexer");
    let error = lexer
        .next(Goal::Division)
        .expect_err("a code point above the last one");
    assert_eq!(error.reason, Reason::CodePointOutOfRange);

    let legal = format!("'\\u{{{zeros}61}}'");
    let mut lexer = Lexer::new(&legal).expect("a lexer");
    let token = lexer.next(Goal::Division).expect("a string");
    assert_eq!(
        token.kind,
        Kind::String(vec![u16::from(b'a')]),
        "a hundred thousand leading zeros still spell the letter a"
    );
}

#[test]
fn a_number_with_a_hostile_exponent_is_a_number() {
    for source in [
        "1e999999999",
        "1e-999999999",
        "0.0e0",
        "1e0000000000000000001",
    ] {
        let mut lexer = Lexer::new(source).expect("a lexer");
        let token = lexer.next(Goal::Division).expect("a number");
        assert!(
            matches!(token.kind, Kind::Number(_)),
            "{source} should be a number"
        );
    }
}

#[test]
fn a_refusal_leaves_the_lexer_somewhere_a_caller_can_use() {
    // ADR 0013 § 4 does not ask for recovery and this does not provide one, but
    // the offset a refusal happened at has to be inside the source: an error
    // pointing past the end is a message that cannot be shown.
    for source in NASTY {
        let Ok(mut lexer) = Lexer::new(source) else {
            continue;
        };
        loop {
            match lexer.next(Goal::Division) {
                Ok(token) if token.kind == Kind::End => break,
                Ok(_) => {}
                Err(error) => {
                    assert!(
                        error.at <= source.len(),
                        "{source:?} refused at {} of {}",
                        error.at,
                        source.len()
                    );
                    let position = alo_js::Position::of(source, error.at);
                    assert!(position.line >= 1 && position.column >= 1);
                    break;
                }
            }
        }
    }
}
