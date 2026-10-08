/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A regular expression is a stranger's program (`LOOP.md`, stage 2 § 2):
//! malformed, truncated and adversarial patterns, and inputs chosen to make
//! a match expensive, each **answered** — a pattern, a refusal, a match, no
//! match, or a halt — and never a panic, a stack overflow or a loop that does
//! not end.
//!
//! The random patterns come from a fixed generator, so a failure here is a
//! pattern somebody can print and read rather than one that went away on the
//! next run.

use alo_js::bounds;
use alo_js::regexp::{self, Halt, Refused, Wrong, search};

/// Patterns worth cutting short at every character.
const WHOLE: [&str; 12] = [
    r"(?<year>\d{4})-(?<m>\d{2})\k<year>",
    r"(?<=\$)\d+(\.\d*)?(?!x)",
    r"(?i:a)(?-s:.)(?m-s:^)[\w-]{2,5}?",
    r"\u{1F600}😀\p{Script=Greek}\P{Lu}",
    r"[\p{L}--[a-z]]&&[\q{abc|d}]",
    r"[[a-z]&&[aeiou]]",
    r"^(?:a|b|)*?c{0,}$|\b\B",
    r"\/\*[\s\S]*?\*\/",
    r"--([a-z0-9-]+)\s*:\s*([\s\S]+)$",
    r"^-?[\d.]+(rem|px|em)$",
    r"\cJ\x41\0[\b]\1(a)",
    r"(((((a)))))\5\4\3\2\1",
];

/// The flag sets tried, which are the three grammars.
const MODES: [&str; 3] = ["", "u", "v"];

#[test]
fn every_truncation_of_a_pattern_is_answered() {
    for whole in WHOLE {
        for (cut, _) in whole.char_indices() {
            let part = whole.get(..cut).unwrap_or("");
            for flags in MODES {
                // Either answer is fine; what is not fine is no answer.
                let _ = regexp::check(part, flags);
                if let Ok(program) = regexp::compile(part, flags) {
                    let input: Vec<u16> = "ab 2024-05 $10.53 /* x */ ABC".encode_utf16().collect();
                    let _ = search(&program, &input, 0, &|| false);
                }
            }
        }
    }
}

/// A small fixed generator: the same patterns every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn pick<'a>(&mut self, from: &[&'a str]) -> &'a str {
        let at = usize::try_from(self.next()).unwrap_or(0) % from.len();
        from.get(at).copied().unwrap_or("")
    }
}

/// The pieces random patterns are made of: every syntax character, every
/// escape the grammar has and some it does not.
const PIECES: [&str; 48] = [
    "a",
    "b",
    "(",
    ")",
    "[",
    "]",
    "{",
    "}",
    "|",
    "*",
    "+",
    "?",
    "^",
    "$",
    ".",
    "\\",
    "-",
    ",",
    "1",
    "0",
    "(?:",
    "(?=",
    "(?!",
    "(?<=",
    "(?<!",
    "(?<n>",
    "\\k<n>",
    "\\1",
    "\\b",
    "\\B",
    "\\d",
    "\\W",
    "\\s",
    "\\p{L}",
    "\\q{ab}",
    "\\u{1F600}",
    "\\uD83D",
    "\\x4",
    "\\c",
    "&&",
    "--",
    "{2,1}",
    "{1,}",
    "😀",
    "\\/",
    "(?i:",
    "(?m-s:",
    "[^",
];

#[test]
fn random_patterns_in_every_mode_are_answered_and_their_matches_end() {
    let mut random = Lcg(0x5eed);
    let inputs: Vec<Vec<u16>> = ["", "aab", "ab😀ba", "(a)[b]{c}", "\n\u{2028}a\r"]
        .iter()
        .map(|text| text.encode_utf16().collect())
        .collect();
    let mut compiled = 0_u32;
    for _ in 0..6_000 {
        let length = usize::try_from(random.next() % 12).unwrap_or(0);
        let pattern: String = (0..length).map(|_| random.pick(&PIECES)).collect();
        for flags in MODES {
            match regexp::compile(&pattern, flags) {
                Ok(program) => {
                    compiled = compiled.saturating_add(1);
                    for input in &inputs {
                        for from in [0, 1, 7] {
                            match search(&program, input, from, &|| false) {
                                Ok(_) | Err(Halt::Steps | Halt::Places) => {}
                                Err(other) => panic!("/{pattern}/{flags} halted as {other:?}"),
                            }
                        }
                    }
                }
                Err(Refused::Wrong(_) | Refused::Unbuilt(_)) => {}
            }
        }
    }
    assert!(
        compiled > 1_000,
        "the generator makes patterns that compile too, or this tests only the parser: {compiled}"
    );
}

#[test]
fn nesting_is_bounded_at_parse_and_what_is_inside_the_bound_runs() {
    let deep = bounds::DEEPEST_PATTERN;
    let inside = format!("{}a{}", "(".repeat(deep), ")".repeat(deep));
    let Ok(program) = regexp::compile(&inside, "") else {
        panic!("{deep} groups deep is a pattern");
    };
    let input: Vec<u16> = "a".encode_utf16().collect();
    let Ok(Some(found)) = search(&program, &input, 0, &|| false) else {
        panic!("and it matches");
    };
    assert_eq!(found.captures.len(), deep + 1);

    for (open, close, flags) in [
        ("(", ")", ""),
        ("(?=", ")", ""),
        ("(?<!", ")", ""),
        ("[", "]", "v"),
    ] {
        let past = format!("{}{}", open.repeat(deep + 1), close.repeat(deep + 1));
        match regexp::check(&past, flags) {
            Err(error) => assert_eq!(error.wrong, Wrong::TooDeep, "{open}"),
            Ok(()) => panic!("{open} nested {} deep is refused", deep + 1),
        }
    }
    // Twenty thousand is a few bytes of a page and is refused the same way,
    // rather than overflowing a stack.
    let page = format!("{}{}", "(".repeat(20_000), ")".repeat(20_000));
    assert!(regexp::check(&page, "").is_err());
}

#[test]
fn numbers_past_what_fits_are_answered() {
    for (body, input) in [
        ("a{4294967296}", "aaa"),
        ("a{0,99999999999999999999}", "aaa"),
        ("(?:a{2}){4294967295,}", "aaaa"),
        ("\\99999999999999999999(a)", "a"),
        ("(?:){4294967295}", ""),
    ] {
        let flags = "";
        let Ok(program) = regexp::compile(body, flags) else {
            // A backreference past the groups is Annex B's octal escape.
            assert!(body.starts_with('\\'), "{body} compiles");
            continue;
        };
        let units: Vec<u16> = input.encode_utf16().collect();
        match search(&program, &units, 0, &|| false) {
            Ok(_) | Err(Halt::Steps | Halt::Places) => {}
            Err(other) => panic!("{body} halted as {other:?}"),
        }
    }
}

#[test]
fn an_empty_loop_with_a_huge_least_runs_out_of_what_one_match_may_have() {
    // Four thousand million iterations that each match nothing, all needed
    // to reach the least: every one is counted, and each holds the count it
    // must restore, so whichever bound comes first ends it.
    for body in [
        "(?:(?:)|a){4294967295}b",
        "(?:){4294967295}b",
        "(?:\\b|a){4294967295}b",
    ] {
        let Ok(program) = regexp::compile(body, "") else {
            panic!("{body} compiles");
        };
        let units: Vec<u16> = "b".encode_utf16().collect();
        let halted = search(&program, &units, 0, &|| false);
        assert!(
            matches!(halted, Err(Halt::Steps | Halt::Places)),
            "{body}: {halted:?}"
        );
    }
}
