/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The table: source text on the left, the tokens it makes on the right.
//!
//! ADR 0013 § 9 asks for *tables of small programs with the values the
//! specification says* rather than prose, and a lexer is the easiest place in
//! the engine to keep that promise: a token stream is a list of short strings.
//! Every case here is one line of source and the tokens it must produce, so a
//! change to the grammar shows up as a diff somebody can read.
//!
//! The frozen page's own script is the other half and lives in
//! `a_frozen_script.rs`. A table proves the lexer meets the grammar; a real
//! script proves it meets real code, and neither is the other.

use alo_js::lexer::Goal;
use alo_js::punctuator::{self, Punctuator};
use alo_js::token::Kind;
use alo_js::word::{self, Status};
use alo_js::{Lexer, Reason};

/// Every token of `source`, read as though an operator could come next.
fn division(source: &str) -> Vec<String> {
    read(source, Goal::Division)
}

/// Every token of `source`, read as though an operand could come next.
fn regular_expression(source: &str) -> Vec<String> {
    read(source, Goal::RegularExpression)
}

/// Every token of `source`, sketched, with one goal used throughout.
///
/// One goal throughout is only right for source with no `/` in it, which is
/// most of this table; the cases where the goal is the point drive the lexer by
/// hand.
fn read(source: &str, goal: Goal) -> Vec<String> {
    let Ok(mut lexer) = Lexer::new(source) else {
        return vec!["source too long".to_owned()];
    };
    let mut sketches = Vec::new();
    loop {
        match lexer.next(goal) {
            Ok(token) if token.kind == Kind::End => return sketches,
            Ok(token) => sketches.push(sketch(&token.kind)),
            // A refusal is sketched rather than raised, so a case that starts
            // failing shows what the lexer said in the diff instead of a
            // panic somebody has to reproduce.
            Err(error) => {
                sketches.push(format!("refused {:?}", error.reason));
                return sketches;
            }
        }
    }
}

/// What refusing `source` says, or `"no refusal"`.
fn refusal(source: &str, goal: Goal) -> String {
    let Ok(mut lexer) = Lexer::new(source) else {
        return "source too long".to_owned();
    };
    loop {
        match lexer.next(goal) {
            Ok(token) if token.kind == Kind::End => return "no refusal".to_owned(),
            Ok(_) => {}
            Err(error) => return format!("{:?}", error.reason),
        }
    }
}

/// One token as a short string, so that a table reads as a table.
fn sketch(kind: &Kind) -> String {
    match kind {
        Kind::Name(found) if found.escaped => format!("escaped name {}", found.name),
        Kind::Name(found) => format!("name {}", found.name),
        Kind::PrivateName(found) => format!("private #{}", found.name),
        Kind::Number(value) => format!("number {value:?}"),
        Kind::BigInt { digits, radix } => format!("bigint {digits} base {}", radix.value()),
        Kind::String(units) => format!("string {}", text(units)),
        Kind::Template(piece) => format!(
            "template {:?} raw {:?} cooked {}",
            piece.part,
            piece.raw,
            piece.cooked.as_deref().map_or("-".to_owned(), text)
        ),
        Kind::RegularExpression(literal) => format!("regexp /{}/{}", literal.body, literal.flags),
        Kind::Punctuator(spelling) => format!("punct {}", spelling.as_str()),
        Kind::End => "end".to_owned(),
        _ => "a kind this test does not know".to_owned(),
    }
}

/// Code units as text, or as their numbers when they are not text at all.
///
/// The fallback is the point: `'\uD800'` is a legal string of one code unit
/// that is half a surrogate pair, and a test that could only show valid text
/// could not tell it from a refusal.
fn text(units: &[u16]) -> String {
    String::from_utf16(units).map_or_else(
        |_| {
            let numbers: Vec<String> = units.iter().map(|unit| format!("U+{unit:04X}")).collect();
            format!("[{}]", numbers.join(" "))
        },
        |readable| format!("{readable:?}"),
    )
}

#[test]
fn punctuation_takes_the_longest_spelling() {
    assert_eq!(
        division("a >>>= b"),
        ["name a", "punct >>>=", "name b"],
        "the longest spelling wins, or `a >>>= b` is `a >>>` and `= b`"
    );
    assert_eq!(division("a ?? b"), ["name a", "punct ??", "name b"]);
    assert_eq!(division("x ??= 1"), ["name x", "punct ??=", "number 1.0"]);
    assert_eq!(division("...rest"), ["punct ...", "name rest"]);
    assert_eq!(division("x ** 2"), ["name x", "punct **", "number 2.0"]);
    assert_eq!(division("a === b"), ["name a", "punct ===", "name b"]);
    assert_eq!(
        division("() => {}"),
        ["punct (", "punct )", "punct =>", "punct {", "punct }"]
    );
}

#[test]
fn no_punctuator_is_shadowed_by_a_shorter_one() {
    for (earlier, spelling) in punctuator::IN_ANY_GOAL.iter().enumerate() {
        for later in punctuator::IN_ANY_GOAL.iter().skip(earlier + 1) {
            assert!(
                !later.as_str().starts_with(spelling.as_str()),
                "{} comes before {}, so {} can never be read",
                spelling.as_str(),
                later.as_str(),
                later.as_str()
            );
        }
    }
}

#[test]
fn every_punctuator_can_be_read() {
    let goal_free = punctuator::IN_ANY_GOAL.iter().copied();
    let decided = [
        Punctuator::Divide,
        Punctuator::DivideAssign,
        Punctuator::Question,
        Punctuator::OptionalChain,
    ];
    for spelling in goal_free.chain(decided) {
        assert_eq!(
            division(spelling.as_str()),
            [format!("punct {}", spelling.as_str())],
            "{} is in the table and cannot be read back",
            spelling.as_str()
        );
    }
}

#[test]
fn a_question_mark_before_a_digit_is_not_an_optional_chain() {
    assert_eq!(division("a?.b"), ["name a", "punct ?.", "name b"]);
    assert_eq!(
        division("a?.5:b"),
        ["name a", "punct ?", "number 0.5", "punct :", "name b"],
        "`a?.5:b` is a conditional whose consequent is `.5`"
    );
}

#[test]
fn numbers_in_every_base_the_language_has() {
    assert_eq!(division("0"), ["number 0.0"]);
    assert_eq!(division("1_000_000"), ["number 1000000.0"]);
    assert_eq!(division("0x1f"), ["number 31.0"]);
    assert_eq!(division("0b1010"), ["number 10.0"]);
    assert_eq!(division("0o755"), ["number 493.0"]);
    assert_eq!(division("1e3"), ["number 1000.0"]);
    assert_eq!(division(".5"), ["number 0.5"]);
    assert_eq!(division("1."), ["number 1.0"]);
    assert_eq!(division("1.5e-3"), ["number 0.0015"]);
    assert_eq!(
        division("1e400"),
        ["number inf"],
        "a literal too large for a double is infinity rather than a refusal"
    );
}

#[test]
fn a_literal_is_rounded_once_rather_than_at_every_digit() {
    // Both of these have fifty-four significant bits, so the answer is decided
    // by the guard bit and the parity of the significand — which is exactly
    // what an accumulate-as-you-go loop gets wrong.
    assert_eq!(
        division("0x20000000000001"),
        ["number 9007199254740992.0"],
        "2^53 + 1 rounds down: the guard bit is set and the significand is even"
    );
    assert_eq!(
        division("0x20000000000003"),
        ["number 9007199254740996.0"],
        "2^53 + 3 rounds up: the guard bit is set and the significand is odd"
    );
    assert_eq!(division("9007199254740993"), ["number 9007199254740992.0"]);
}

#[test]
fn a_bigint_keeps_its_digits() {
    assert_eq!(division("123n"), ["bigint 123 base 10"]);
    assert_eq!(division("0x10n"), ["bigint 10 base 16"]);
    assert_eq!(division("0n"), ["bigint 0 base 10"]);
    assert_eq!(division("1_0n"), ["bigint 10 base 10"]);
}

#[test]
fn strings_are_code_units_and_not_characters() {
    assert_eq!(division(r"'a\nb'"), [r#"string "a\nb""#]);
    assert_eq!(division(r"'\x41'"), [r#"string "A""#]);
    assert_eq!(division(r"'\''"), [r#"string "'""#]);
    assert_eq!(division("\"\\u{1F600}\""), [r#"string "😀""#]);
    assert_eq!(
        division(r"'\uD800'"),
        ["string [U+D800]"],
        "a lone surrogate is a legal string of one code unit and no character"
    );
    assert_eq!(
        division("'a\\\nb'"),
        [r#"string "ab""#],
        "a backslash before a line ending continues the line and adds nothing"
    );
    assert_eq!(
        division("'\u{2028}'"),
        [r#"string "\u{2028}""#],
        "the two Unicode separators have been allowed in a string since ES2019"
    );
}

#[test]
fn names_may_be_written_with_escapes_and_are_then_never_keywords() {
    assert_eq!(division("$_x1"), ["name $_x1"]);
    assert_eq!(division("é"), ["name é"]);
    assert_eq!(division(r"\u0041bc"), ["escaped name Abc"]);
    assert_eq!(
        division(r"\u0069\u0066"),
        ["escaped name if"],
        "a reserved word written with an escape is a name; the parser refuses it"
    );
    assert_eq!(division("#count"), ["private #count"]);
}

#[test]
fn the_keyword_table_says_how_reserved_a_word_is() {
    let status = |name: &str| word::keyword(name).map(word::Keyword::status);
    assert_eq!(status("if"), Some(Status::Reserved));
    assert_eq!(status("with"), Some(Status::Reserved));
    assert_eq!(status("await"), Some(Status::ReservedWhereItMeansSomething));
    assert_eq!(status("yield"), Some(Status::ReservedWhereItMeansSomething));
    assert_eq!(status("let"), Some(Status::ReservedInStrictCode));
    assert_eq!(status("static"), Some(Status::ReservedInStrictCode));
    assert_eq!(status("of"), Some(Status::Contextual));
    assert_eq!(status("async"), Some(Status::Contextual));
    assert_eq!(status("banana"), None);
}

#[test]
fn a_slash_is_whatever_the_caller_asked_for() {
    // The same eight characters, read two ways, meaning two different programs.
    assert_eq!(
        division("a /b/ g"),
        ["name a", "punct /", "name b", "punct /", "name g"]
    );

    let mut lexer = Lexer::new("a /b/ g").expect("a lexer");
    let first = lexer.next(Goal::Division).expect("a name");
    assert_eq!(sketch(&first.kind), "name a");
    let second = lexer.next(Goal::RegularExpression).expect("a literal");
    assert_eq!(sketch(&second.kind), "regexp /b/");
    let third = lexer.next(Goal::Division).expect("a name");
    assert_eq!(sketch(&third.kind), "name g");
}

#[test]
fn a_regular_expression_ends_where_the_class_says() {
    assert_eq!(
        regular_expression("/[/]/"),
        ["regexp /[/]/"],
        "a slash inside a character class is an ordinary character"
    );
    assert_eq!(regular_expression(r"/\//"), [r"regexp /\//"]);
    assert_eq!(regular_expression("/ab+c/gi"), ["regexp /ab+c/gi"]);
    assert_eq!(
        regular_expression("/x/"),
        ["regexp /x/"],
        "no flags is not the same as a missing flag"
    );
}

#[test]
fn a_comment_is_never_a_regular_expression() {
    assert_eq!(
        regular_expression("// not a literal\n1"),
        ["number 1.0"],
        "trivia is skipped before the goal is consulted, which is why a pattern \
         can never begin with a slash"
    );
    assert_eq!(regular_expression("/* also not */ 1"), ["number 1.0"]);
}

#[test]
fn a_template_is_read_in_pieces_the_parser_asks_for() {
    assert_eq!(
        division("`plain`"),
        [r#"template Whole raw "plain" cooked "plain""#]
    );

    let mut lexer = Lexer::new("`a${b}c`").expect("a lexer");
    let head = lexer.next(Goal::Division).expect("a head");
    assert_eq!(sketch(&head.kind), r#"template Head raw "a" cooked "a""#);
    let inside = lexer.next(Goal::RegularExpression).expect("a name");
    assert_eq!(sketch(&inside.kind), "name b");
    let tail = lexer
        .next(Goal::TemplateContinuation)
        .expect("a tail rather than a closing brace");
    assert_eq!(sketch(&tail.kind), r#"template Tail raw "c" cooked "c""#);
}

#[test]
fn a_template_keeps_what_the_author_wrote_as_well_as_what_it_means() {
    assert_eq!(
        division(r"`a\nb`"),
        [r#"template Whole raw "a\\nb" cooked "a\nb""#],
        "the raw value is the backslash and the n, which is what a tag is handed"
    );
    assert_eq!(
        division("`a\r\nb`"),
        [r#"template Whole raw "a\nb" cooked "a\nb""#],
        "a file saved on Windows is the same program as one saved anywhere else"
    );
    assert_eq!(
        division(r"`\unicode`"),
        [r#"template Whole raw "\\unicode" cooked -"#],
        "an escape nobody can read is undefined when cooked, not a refusal — \
         only a tagged template may have one, and the parser decides that"
    );
    assert_eq!(
        division(r"`\``"),
        [r#"template Whole raw "\\`" cooked "`""#],
        "an escaped backtick does not end the template"
    );
}

#[test]
fn a_line_ending_is_remembered_and_everything_else_about_trivia_is_not() {
    let ends_a_line = |source: &str| {
        let mut lexer = Lexer::new(source).expect("a lexer");
        lexer.next(Goal::Division).expect("the first token");
        lexer
            .next(Goal::Division)
            .expect("the second token")
            .newline_before
    };
    assert!(ends_a_line("a\nb"));
    assert!(!ends_a_line("a b"));
    assert!(ends_a_line("a // note\nb"));
    assert!(
        ends_a_line("a /*\n*/ b"),
        "a block comment holding a line ending is one, which is why they are \
         looked into rather than jumped over"
    );
    assert!(!ends_a_line("a /* note */ b"));
}

#[test]
fn a_hashbang_is_a_comment_once() {
    assert_eq!(division("#!/usr/bin/env node\nx"), ["name x"]);
    assert_eq!(
        refusal("x\n#!/usr/bin/env node", Goal::Division),
        "PrivateNameWithoutAName",
        "the rule holds at offset zero and nowhere else"
    );
}

#[test]
fn html_like_comments_are_punctuation_rather_than_comments() {
    assert_eq!(
        division("a <!--b"),
        ["name a", "punct <", "punct !", "punct --", "name b"],
        "Annex B is refused by not being implemented; refusing the characters \
         would break ordinary modern code that means `a < !(--b)`"
    );
}

#[test]
fn a_token_knows_where_it_was() {
    let mut lexer = Lexer::new("  foo  ").expect("a lexer");
    let token = lexer.next(Goal::Division).expect("a name");
    assert_eq!((token.start, token.end), (2, 5));
    let end = lexer.next(Goal::Division).expect("the end");
    assert_eq!(end.kind, Kind::End);
    assert_eq!((end.start, end.end), (7, 7));
}

#[test]
fn what_is_refused_and_by_what_name() {
    let table = [
        (r"'\1'", "LegacyOctalEscape"),
        (r"'\01'", "LegacyOctalEscape"),
        (r"'\8'", "NonOctalDecimalEscape"),
        ("0755", "LegacyOctalLiteral"),
        ("08", "LegacyOctalLiteral"),
        ("0_1", "MisplacedNumericSeparator"),
        ("1_", "MisplacedNumericSeparator"),
        ("1__0", "MisplacedNumericSeparator"),
        ("_1", "no refusal"),
        ("3in", "NumberFollowedByName"),
        ("1.5n", "BigIntIsNotAnInteger"),
        ("1e3n", "BigIntIsNotAnInteger"),
        ("0x", "MissingDigits"),
        ("1e", "MissingDigits"),
        (r"'\u{110000}'", "CodePointOutOfRange"),
        (r"'\u12'", "BadUnicodeEscape"),
        (r"'\x1'", "BadHexEscape"),
        (r"'\u{}'", "BadUnicodeEscape"),
        (r"\u0030x", "EscapeIsNotANameCharacter"),
        ("'a\nb'", "LineTerminatorInString"),
        ("'unclosed", "UnterminatedString"),
        ("`unclosed", "UnterminatedTemplate"),
        ("/* unclosed", "UnterminatedComment"),
        ("#", "PrivateNameWithoutAName"),
        ("@", "UnexpectedCharacter('@')"),
    ];
    for (source, expected) in table {
        assert_eq!(
            refusal(source, Goal::Division),
            expected,
            "reading {source:?}"
        );
    }
}

#[test]
fn what_a_regular_expression_refuses() {
    let table = [
        ("/x/z", "UnknownRegularExpressionFlag('z')"),
        ("/x/gg", "RepeatedRegularExpressionFlag('g')"),
        ("/x/uv", "BothUnicodeModes"),
        ("/x", "UnterminatedRegularExpression"),
        ("/x\n/", "LineTerminatorInRegularExpression"),
        ("/[x/", "UnterminatedRegularExpression"),
        ("/x/dgimsy", "no refusal"),
    ];
    for (source, expected) in table {
        assert_eq!(
            refusal(source, Goal::RegularExpression),
            expected,
            "reading {source:?}"
        );
    }
}

#[test]
fn a_template_continuation_asked_for_anywhere_else_is_refused() {
    let mut lexer = Lexer::new("a").expect("a lexer");
    let error = lexer
        .next(Goal::TemplateContinuation)
        .expect_err("a name is not a template continuation");
    assert_eq!(error.reason, Reason::NotATemplateContinuation);
}

#[test]
fn a_refusal_says_where_it_is_in_words() {
    let mut lexer = Lexer::new("let a = 1;\nlet b = 0755;").expect("a lexer");
    let error = loop {
        match lexer.next(Goal::Division) {
            Ok(token) if token.kind == Kind::End => panic!("the legacy octal was read"),
            Ok(_) => {}
            Err(error) => break error,
        }
    };
    let position = alo_js::Position::of("let a = 1;\nlet b = 0755;", error.at);
    assert_eq!((position.line, position.column), (2, 9));
    assert_eq!(position.to_string(), "line 2, column 9");
}
