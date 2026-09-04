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
//!
//! # And the one nobody had looked at: the tree, rather than the parser
//!
//! The brackets above were the shape somebody thought of, and counting them
//! answered a narrower question than the one that matters. Nine other shapes
//! got past it — `!!!…a`, `new new …a`, `a**a**a…`, `a.b.b.b…`, `a[b][b]…`,
//! `a()()…`, `a?.b?.b…`, a run of tagged templates and `a+a+a…` — and they
//! split in two:
//!
//! - the ones the parser **descends into** and did not count, which overflow
//!   the parse thread's own stack; and
//! - the ones it reads in a **loop**, which cost it nothing at all and build a
//!   tree as deep as the file is long. Nothing refused those, and the first
//!   thing to walk the tree aborted the process — `Drop`, before any compiler
//!   got near it. Sixty thousand `+` is a hundred and eighty kilobytes of
//!   somebody else's file.
//!
//! Two bounds, because they are two questions:
//! [`alo_js::bounds::DEEPEST_NESTING`] is how deep this parser recurses and
//! [`alo_js::bounds::DEEPEST_EXPRESSION`] is how deep a tree it builds. The
//! table below says which one refuses each shape, and stands either side of it
//! for the reason above. What follows the table is the other half: the shapes
//! that are **wide** rather than deep — fifty thousand array elements, twenty
//! thousand properties, twenty thousand statements — which must go on parsing,
//! because a bound that counted siblings would refuse every bundle there is.

use alo_js::bounds::{DEEPEST_EXPRESSION, DEEPEST_NESTING};
use alo_js::{Reason, Source, module, script};

/// Which of the two ceilings a shape is refused by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bound {
    /// The parser descends into it, so it costs stack frames.
    Recursion,
    /// The parser reads it in a loop, so it costs only tree.
    Tree,
}

impl Bound {
    /// How deep this shape may go.
    fn most(self) -> usize {
        match self {
            Bound::Recursion => DEEPEST_NESTING,
            Bound::Tree => DEEPEST_EXPRESSION,
        }
    }

    /// The refusal a program past it gets.
    fn refusal(self) -> Reason {
        match self {
            Bound::Recursion => Reason::TooDeeplyNested {
                most: DEEPEST_NESTING,
            },
            Bound::Tree => Reason::ExpressionTooDeep {
                most: DEEPEST_EXPRESSION,
            },
        }
    }
}

/// Every shape that adds a level of tree per repetition.
///
/// `(before, repeated, after)`, and the bound that refuses it. A shape moving
/// from one column to the other is a change in how this parser is written, not
/// a number to update: it means something that used to be read in a loop now
/// recurses, or the other way round.
const DEEP: &[(&str, &str, &str, Bound)] = &[
    ("", "!", "a", Bound::Recursion),
    // A space, because `--` is one token and a decrement of a decrement is
    // refused for being an assignment to something that is not a name — which
    // would be a different test passing for a different reason.
    ("", "- ", "a", Bound::Recursion),
    ("", "typeof ", "a", Bound::Recursion),
    ("", "void ", "a", Bound::Recursion),
    ("", "new ", "a", Bound::Recursion),
    ("a", "**a", "", Bound::Recursion),
    ("a", ".b", "", Bound::Tree),
    ("a", "[b]", "", Bound::Tree),
    ("a", "()", "", Bound::Tree),
    ("a", "?.b", "", Bound::Tree),
    ("a", "`x`", "", Bound::Tree),
    ("a", "+a", "", Bound::Tree),
    ("a", "||a", "", Bound::Tree),
    ("a", "&&a", "", Bound::Tree),
    ("a", "??a", "", Bound::Tree),
];

/// One of the shapes above, repeated.
fn shape(before: &str, repeated: &str, after: &str, times: usize) -> String {
    format!("{before}{}{after}", repeated.repeat(times))
}

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

#[test]
fn every_shape_that_deepens_a_tree_is_refused_past_its_bound() {
    // Sixty thousand of each. Nine of these aborted the process before the
    // bounds existed — five inside the parser and four when the program they
    // had built was let go of — so the assertion that matters most is the one
    // the test harness makes by reaching the next line at all.
    for (before, repeated, after, bound) in DEEP {
        let source = shape(before, repeated, after, 60_000);
        let error = script(&source).expect_err("sixty thousand of anything is too deep");
        assert_eq!(error.reason, bound.refusal(), "for {repeated:?}");
    }
}

#[test]
fn every_one_of_them_is_read_just_below_its_bound_and_let_go_of() {
    // The other side, because a bound only ever tested from above is a bound
    // nobody has checked is reachable — and the `drop` is not a formality: the
    // whole of what went wrong here was a tree that parsed and then could not
    // be walked.
    for (before, repeated, after, bound) in DEEP {
        let source = shape(before, repeated, after, bound.most() - 50);
        let program = match script(&source) {
            Ok(program) => program,
            Err(error) => panic!(
                "{repeated:?} just under {} was refused: {error}",
                bound.most()
            ),
        };
        drop(program);
    }
}

#[test]
fn a_chain_inside_a_chain_inside_a_chain_is_counted_as_all_three() {
    // The shape a bound that put its count back on the way out would never
    // see: two hundred levels, each a thousand links, none of which reaches
    // the ceiling on its own and which together are two hundred thousand deep.
    let mut source = String::from("a");
    for _ in 0..200 {
        source = format!("({source}{})", "+a".repeat(1_000));
    }
    let error = script(&source).expect_err("two hundred thousand deep is too deep");
    assert_eq!(
        error.reason,
        Reason::ExpressionTooDeep {
            most: DEEPEST_EXPRESSION
        }
    );
}

#[test]
fn a_program_that_is_wide_rather_than_deep_is_still_read() {
    // The half a bound gets wrong in the other direction. Every one of these
    // is a shape a real bundle has, and each is two or three levels deep with
    // tens of thousands of siblings — so a count that added siblings up would
    // refuse the ordinary web while the deep shapes above went on aborting.
    let wide = [
        format!("[{}]", vec!["1"; 50_000].join(",")),
        format!(
            "x = {{{}}}",
            (0..20_000)
                .map(|at| format!("k{at}:1"))
                .collect::<Vec<_>>()
                .join(",")
        ),
        format!("f({})", vec!["a"; 20_000].join(",")),
        format!(
            "var {};",
            (0..5_000)
                .map(|at| format!("v{at}=1"))
                .collect::<Vec<_>>()
                .join(",")
        ),
        format!("a{}", ",a".repeat(60_000)),
        "a = b + c;".repeat(20_000),
        format!("`{}`", "${a}x".repeat(20_000)),
    ];
    for source in wide {
        let program = match script(&source) {
            Ok(program) => program,
            Err(error) => panic!("a wide program was refused: {error}"),
        };
        drop(program);
    }
}
