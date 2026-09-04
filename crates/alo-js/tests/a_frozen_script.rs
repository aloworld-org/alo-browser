/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A real page's own script, frozen, read from beginning to end.
//!
//! `LOOP.md`'s stage 2 clause 1: an item is opened by something real that fails
//! and closed by the same thing working, and what it is judged against is
//! **frozen, never fetched**. The script is
//! `crates/alo-corpus/scripts/alo-service-worker/script.js` — alo's own
//! offline-shell service worker, taken from `alo-workplace`, with `origin.txt`
//! beside it saying where it came from and what it does and does not cover.
//!
//! # What a real script proves that the table cannot
//!
//! The table beside this (`what_the_lexer_makes_of_it.rs`) is written by the
//! same person who wrote the lexer, on the same afternoon, and it proves the
//! lexer agrees with what that person believed the grammar to be. A script
//! somebody wrote for a different purpose entirely proves it agrees with what
//! the grammar *is*. Neither is the other, which is why both are here.
//!
//! # And the assertion that would catch a byte quietly dropped
//!
//! Tokens and their spans are not enough on their own: a lexer that skipped a
//! character it should have read would produce a perfectly tidy token stream
//! with a hole in it. So the gap between every pair of neighbouring tokens is
//! itself lexed, and must come back as nothing at all. Anything the lexer let
//! through unnoticed shows up there as a token.

use std::path::PathBuf;

use alo_js::Lexer;
use alo_js::lexer::Goal;
use alo_js::punctuator::Punctuator;
use alo_js::token::{Kind, Token};

/// Where the frozen script is, for a message somebody can act on.
///
/// Read by path rather than through `alo-corpus`, on purpose: ADR 0013 § 5 says
/// `alo-js` depends on no I/O crate, and a dependency on the corpus crate would
/// put the whole renderer behind this one file — and behind a cycle, the day
/// the renderer runs script.
fn where_the_script_is() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../alo-corpus/scripts/alo-service-worker/script.js")
}

/// The frozen script, or nothing at all if it is not there.
///
/// Empty rather than a refusal to continue, because
/// `the_script_is_frozen_where_the_test_looks_for_it` is the test that says so
/// — and every other failure in this file then has one explanation rather than
/// four copies of it.
fn frozen_script() -> String {
    std::fs::read_to_string(where_the_script_is()).unwrap_or_default()
}

/// Every token of the script, read as though an operator could come next.
///
/// One goal throughout is the right reading for this script and the test says
/// so rather than assuming it: `no_slash_in_it_means_the_goal_never_mattered`
/// is what holds it up, and it would fail the day somebody froze a script with
/// a regular expression in it — which is the day this needs a parser.
fn tokens(source: &str) -> Vec<Token> {
    let Ok(mut lexer) = Lexer::new(source) else {
        return Vec::new();
    };
    let mut tokens = Vec::new();
    while let Ok(token) = lexer.next(Goal::Division) {
        let end = token.kind == Kind::End;
        tokens.push(token);
        if end {
            break;
        }
    }
    tokens
}

/// What refusing the script says, and where, or [`None`] if it reads whole.
fn refusal(source: &str) -> Option<String> {
    let Ok(mut lexer) = Lexer::new(source) else {
        return Some("the script is longer than the bound".to_owned());
    };
    loop {
        match lexer.next(Goal::Division) {
            Ok(token) if token.kind == Kind::End => return None,
            Ok(_) => {}
            Err(error) => {
                return Some(format!(
                    "{error} at {}",
                    alo_js::Position::of(source, error.at)
                ));
            }
        }
    }
}

#[test]
fn the_script_is_frozen_where_the_test_looks_for_it() {
    assert!(
        !frozen_script().is_empty(),
        "nothing at {} — the corpus is read out of the repository rather than \
         fetched, so a missing file here is a missing commit",
        where_the_script_is().display()
    );
}

#[test]
fn the_whole_script_reads() {
    let source = frozen_script();
    assert_eq!(refusal(&source), None, "the frozen script was refused");
    let tokens = tokens(&source);
    assert_eq!(
        tokens.len(),
        671,
        "670 tokens and the end; a change to this number is a change to how \
         alo's own service worker is read, and is worth looking at"
    );
    assert_eq!(tokens.last().map(|token| &token.kind), Some(&Kind::End));
}

#[test]
fn nothing_between_two_tokens_was_anything_but_trivia() {
    let source = frozen_script();
    let tokens = tokens(&source);
    let mut reached = 0;
    for token in &tokens {
        let gap = source.get(reached..token.start).unwrap_or("!");
        let left = Lexer::new(gap)
            .and_then(|mut over_the_gap| over_the_gap.next(Goal::Division))
            .map(|found| found.kind);
        assert_eq!(
            left,
            Ok(Kind::End),
            "{gap:?} was skipped between two tokens and is not only whitespace \
             and comments"
        );
        reached = token.end;
    }
    assert_eq!(
        reached,
        source.len(),
        "the last token ends at the last byte"
    );
}

#[test]
fn no_slash_in_it_means_the_goal_never_mattered() {
    let source = frozen_script();
    for token in tokens(&source) {
        assert!(
            !matches!(
                token.kind,
                Kind::Punctuator(Punctuator::Divide | Punctuator::DivideAssign)
            ),
            "a division at {} means this script has a `/` outside a comment or \
             a string, so reading all of it with one goal is reading a \
             different program",
            alo_js::Position::of(&source, token.start)
        );
    }
}

#[test]
fn what_the_script_actually_says() {
    let source = frozen_script();
    let tokens = tokens(&source);
    let names: Vec<&str> = tokens
        .iter()
        .filter_map(|token| match &token.kind {
            Kind::Name(found) => Some(found.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        names.first().copied(),
        Some("const"),
        "the script begins with sixteen lines of comment and then a `const`"
    );
    for expected in ["addEventListener", "waitUntil", "caches", "skipWaiting"] {
        assert!(names.contains(&expected), "{expected} is in this script");
    }

    let strings: Vec<String> = tokens
        .iter()
        .filter_map(|token| match &token.kind {
            Kind::String(units) => String::from_utf16(units).ok(),
            _ => None,
        })
        .collect();
    assert!(strings.contains(&"/offline.html".to_owned()));
    assert!(
        strings.contains(&"alo öffnen".to_owned()),
        "the German string has a character outside ASCII in it, which is the \
         one thing a byte-at-a-time lexer would get wrong"
    );

    assert!(
        tokens.iter().any(|token| token.newline_before),
        "a script written by a person has lines in it, and the tokens say so"
    );
}
