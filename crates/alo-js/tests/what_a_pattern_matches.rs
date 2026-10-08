/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 74's closing condition, as tables: a regular expression
//! literal matches what the specification says it matches, and a page sees
//! its `exec`, `test` and `lastIndex` behave as the language says.
//!
//! The cases with captures are the specification's own examples where it has
//! them — the notes under `RepeatMatcher`, lookahead and backreferences in
//! ECMA-262 § 22.2.2 — and otherwise derived from its algorithm step by step,
//! each saying in its row which rule it is about: lazy against greedy,
//! alternation's order, captures reset in a repeat, backreferences, lookahead
//! and lookbehind.
//!
//! Every program runs twice, once with the collector running at every
//! allocation, as `what_a_program_evaluates_to.rs` does: a match array is a
//! dozen allocations, and a capture held only in a Rust local across one of
//! them is the bug that run exists to find.

use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{Escape, numeric, script};

/// `show(m)`: a match array as `index:whole,group1,…`, with `_` for a group
/// that took no part, and `null` for no match.
const SHOW: &str = "function show(m) {
  if (m === null) return 'null';
  let s = m.index + ':';
  for (let i = 0; i < m.length; i++) {
    s = s + (i > 0 ? ',' : '') + (m[i] === undefined ? '_' : m[i]);
  }
  return s;
}
";

/// Run a program in a fresh engine, both ways, and answer what it produced.
fn value(source: &str) -> String {
    let ordinary = run(source, false);
    let stressed = run(source, true);
    assert_eq!(
        ordinary, stressed,
        "{source} answered differently when the collector ran at every allocation"
    );
    ordinary
}

/// One run, with `show` defined first.
fn run(source: &str, stress: bool) -> String {
    let whole = format!("{SHOW}{source}");
    let program = match script(&whole) {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    let mut engine = match Engine::new() {
        Ok(engine) => engine,
        Err(why) => return format!("no engine: {why}"),
    };
    engine.objects().heap_mut().stress(stress);
    match engine.evaluate(&program) {
        Ok(value) => shown(&mut engine, value),
        Err(Trouble::Escaped(escape)) => format!("! {escape}"),
        Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
    }
}

/// A value, written out.
fn shown(engine: &mut Engine, value: Value) -> String {
    match value {
        Value::Undefined => "undefined".to_owned(),
        Value::Null => "null".to_owned(),
        Value::Bool(is) => is.to_string(),
        Value::Number(number) => numeric::text_of(number),
        Value::Text(held) => match engine.objects().units(held) {
            Some(units) => String::from_utf16_lossy(units),
            None => "?".to_owned(),
        },
        Value::Symbol(_) => "a symbol".to_owned(),
        Value::Object(_) => "an object".to_owned(),
    }
}

/// Check a table of programs against what each evaluates to.
fn table(cases: &[(&str, &str)]) {
    for (source, expected) in cases {
        assert_eq!(&value(source), expected, "{source}");
    }
}

#[test]
fn the_specifications_own_examples_capture_what_it_says() {
    table(&[
        // Alternatives are tried in order, and the first that lets the rest
        // match wins — not the longest.
        (r#"show(/a|ab/.exec("abc"))"#, "0:a"),
        (
            r#"show(/((a)|(ab))((c)|(bc))/.exec("abc"))"#,
            "0:abc,a,a,_,bc,_,bc",
        ),
        // Greedy and lazy bounded repeats.
        (r#"show(/a[a-z]{2,4}/.exec("abcdefghi"))"#, "0:abcde"),
        (r#"show(/a[a-z]{2,4}?/.exec("abcdefghi"))"#, "0:abc"),
        // A repeat keeps the last iteration's choice of alternative.
        (r#"show(/(aa|aabaac|ba|b|c)*/.exec("aabaac"))"#, "0:aaba,ba"),
        // Captures inside a repeat are reset at each iteration: the last
        // iteration matched `ac`, so `(b+)` is undefined although an earlier
        // one matched `bbb`.
        (
            r#"show(/(z)((a+)?(b+)?(c))*/.exec("zaacbbbcac"))"#,
            "0:zaacbbbcac,z,ac,a,_,c",
        ),
        // The empty check: an iteration that matched nothing ends the repeat.
        (r#"show(/(a*)*/.exec("b"))"#, "0:,_"),
        (r#"show(/(a*)b\1+/.exec("baaaac"))"#, "0:b,"),
        // A lookahead's captures are kept; it consumes nothing.
        (r#"show(/(?=(a+))/.exec("baaabac"))"#, "1:,aaa"),
        (r#"show(/(?=(a+))a*b\1/.exec("baaabac"))"#, "3:aba,a"),
        // A negative lookahead's captures are always undefined after it, and
        // a backreference to one matches the empty string.
        (
            r#"show(/(.*?)a(?!(a+)b\2c)\2(.*)/.exec("baaabaac"))"#,
            "0:baaabaac,ba,_,abaac",
        ),
        (r#"show(/(a|ab)(c|bcd)(d*)/.exec("abcd"))"#, "0:abcd,a,bcd,"),
    ]);
}

#[test]
fn lookbehind_reads_right_to_left() {
    table(&[
        (
            r#"show(/(?<=\$)\d+(\.\d*)?/.exec("cost $10.53"))"#,
            "6:10.53,.53",
        ),
        // At 1 `10` is after `$`; at 2 `0` is after `1`, which is not.
        (r#"show(/(?<!\$)\d+/.exec("$10 20"))"#, "2:0"),
        // Right to left, so the second group is the greedy one.
        (r#"show(/(?<=(\d+)(\d+))$/.exec("1053"))"#, "4:,1,053"),
        // Right to left, so the group is matched before the backreference
        // written in front of it.
        (r#"show(/(?<=\1(a))b/.exec("aab"))"#, "2:b,a"),
        (r#"/(?<=a)b/.test("ab")"#, "true"),
        (r#"/(?<=a)b/.test("cb")"#, "false"),
        (r#"show(/(?<!(a))b/.exec("cb"))"#, "1:b,_"),
    ]);
}

#[test]
fn repeats_alternatives_and_backreferences() {
    table(&[
        (r#"show(/(a+?)(a*)/.exec("aaa"))"#, "0:aaa,a,aa"),
        (r#"show(/a+?/.exec("aaa"))"#, "0:a"),
        (r#"show(/a{2,}?/.exec("aaaa"))"#, "0:aa"),
        (r#"show(/a*ab/.exec("aaab"))"#, "0:aaab"),
        (r#"show(/.*b/.exec("abcb"))"#, "0:abcb"),
        (r#"show(/.*?b/.exec("abcb"))"#, "0:ab"),
        (r#"show(/(?:a{2})+/.exec("aaaaa"))"#, "0:aaaa"),
        (r#"show(/(?:ab){2,3}?c/.exec("abababc"))"#, "0:abababc"),
        (r#"show(/x{0}y/.exec("xy"))"#, "1:y"),
        (r#"show(/(a){0}b/.exec("ab"))"#, "1:b,_"),
        (r#"show(/(?:)*/.exec("x"))"#, "0:"),
        // A backreference to a group that took no part matches nothing, and
        // so does one to a group that comes later.
        (r#"show(/(a)?b\1/.exec("b"))"#, "0:b,_"),
        (r#"show(/\1(a)/.exec("aa"))"#, "0:a,a"),
        (
            r#"show(/(?<q>['"]).*?\k<q>/.exec("say 'hi' now"))"#,
            "4:'hi','",
        ),
        (r#"show(/(a)|b/.exec("b"))"#, "0:b,_"),
    ]);
}

#[test]
fn classes_assertions_and_the_dot() {
    table(&[
        (r#"show(/[^a-c]+/.exec("abcdef"))"#, "3:def"),
        (r#"show(/[\d-]+/.exec("a1-2b"))"#, "1:1-2"),
        (r#"show(/[\D]/.exec("12a"))"#, "2:a"),
        (
            r#"/\s+/.exec("a \t\n\u2028\ufeffb")[0] === " \t\n\u2028\ufeff""#,
            "true",
        ),
        (r#"show(/\w+/.exec("--a_1--"))"#, "2:a_1"),
        (r#"show(/[\b]/.exec("a\bb")) === "1:\b""#, "true"),
        (r#"show(/\bfoo\b/.exec("a foo b"))"#, "2:foo"),
        (r#"show(/\Boo/.exec("foo"))"#, "1:oo"),
        (r#"show(/^b/m.exec("a\nb"))"#, "2:b"),
        (r#"show(/^b/.exec("a\nb"))"#, "null"),
        (r#"show(/a$/m.exec("a\nb"))"#, "0:a"),
        (r#"/a$/.test("a\nb")"#, "false"),
        (r#"/a.b/.test("a\nb")"#, "false"),
        (r#"/a.b/s.test("a\nb")"#, "true"),
        (r#"/a.b/.test("a b")"#, "false"),
        // Modifiers change `m` and `s` for the group only.
        (r#"/(?s:a.b)c/.test("a\nbc")"#, "true"),
        (r#"/(?m:^b)/.test("a\nb")"#, "true"),
        (r#"/a(?-s:.)/s.test("a\n")"#, "false"),
        (r#"/\cJ/.test("\n")"#, "true"),
        (r#"/\x41B\0/.test("AB\0")"#, "true"),
        (r#"show(/\//.exec("a/b"))"#, "1:/"),
    ]);
}

#[test]
fn with_u_a_surrogate_pair_is_one_character_and_without_it_two() {
    table(&[
        (r#"/^.$/u.test("😀")"#, "true"),
        (r#"/^.$/.test("😀")"#, "false"),
        (r#"/^..$/.test("😀")"#, "true"),
        (r#"/\u{1F600}/u.test("😀")"#, "true"),
        (r#"/😀/u.test("😀")"#, "true"),
        (r#"/\udf06/u.test("𝌆")"#, "false"),
        (r#"/\udf06/.exec("𝌆").index"#, "1"),
        (r#"/[😀]/u.exec("x😀")[0] === "😀""#, "true"),
        (r#"/[😀]/.exec("x😀")[0] === "\ud83d""#, "true"),
        (r#"/[^a]/u.exec("😀")[0] === "😀""#, "true"),
        (r#"/😀+/.exec("😀\ude00")[0] === "😀\ude00""#, "true"),
        // `lastIndex` inside a pair names the pair; the match is still
        // reported at `lastIndex`, which is the specification's.
        (
            r#"let r = /./gu; r.lastIndex = 1; let m = r.exec("😀"); m.index + ":" + (m[0] === "\ude00") + ":" + r.lastIndex"#,
            "1:true:2",
        ),
        (
            r#"let r = /\S/gu; let s = ""; let m; while ((m = r.exec("😀a")) !== null) { s = s + m.index + ","; } s"#,
            "0,2,",
        ),
    ]);
}

#[test]
fn named_groups_make_a_groups_object() {
    table(&[
        (
            r#"let m = /(?<y>\d{4})-(?<m>\d{2})/.exec("on 2024-05"); m.groups.y + "/" + m.groups.m + "@" + m.index"#,
            "2024/05@3",
        ),
        (
            r#"let m = /(?<a>x)|(?<a>y)/.exec("y"); show(m) + "|" + m.groups.a"#,
            "0:y,_,y|y",
        ),
        (
            r#"let m = /(?<a>x)|(?<a>y)/.exec("x"); show(m) + "|" + m.groups.a"#,
            "0:x,x,_|x",
        ),
        (r#"/(a)/.exec("a").groups"#, "undefined"),
        (r#"/(?<a>.)/.exec("a").groups.__proto__"#, "undefined"),
        (
            r#"let m = /(?<a>.)(?<b>z)?/.exec("q"); m.groups.b"#,
            "undefined",
        ),
        (r#"/(?<ab>.)/.exec("q").groups.ab"#, "q"),
    ]);
}

#[test]
fn last_index_moves_with_g_and_y_and_not_otherwise() {
    table(&[
        (
            r#"let r = /a/g; let s = ""; let m; while ((m = r.exec("banana")) !== null) { s = s + m.index + ","; } s + r.lastIndex"#,
            "1,3,5,0",
        ),
        (
            r#"let r = /a/y; r.lastIndex = 1; "" + r.test("ba") + r.lastIndex + r.test("ba") + r.lastIndex"#,
            "true2false0",
        ),
        (r#"let r = /a/y; r.test("ba")"#, "false"),
        (
            r#"let r = /a/; r.lastIndex = 5; r.exec("ba").index + ":" + r.lastIndex"#,
            "1:5",
        ),
        (
            r#"let r = /a/g; r.lastIndex = 9; "" + r.exec("aa") + r.lastIndex"#,
            "null0",
        ),
        (r#"let r = /a/g; r.lastIndex = -4; r.exec("ba").index"#, "1"),
        (
            r#"let r = /a/g; r.lastIndex = "1"; r.exec("aa").index"#,
            "1",
        ),
        (
            r#"let r = /a/g; r.lastIndex = { valueOf() { return 1; } }; r.exec("aa").index"#,
            "1",
        ),
        (r#"let r = /(?:)/g; r.exec("ab"); r.lastIndex"#, "0"),
        (r"/a/.lastIndex", "0"),
        (r"let r = /a/; delete r.lastIndex", "false"),
        (r"let r = /a/; r.lastIndex = 7; r.lastIndex", "7"),
    ]);
}

#[test]
fn a_literal_makes_a_new_object_each_time_and_it_is_a_regexp() {
    table(&[
        (r"function f() { return /a/g; } f() === f()", "false"),
        (
            r"function f() { return /a/g; } let a = f(); a.lastIndex = 3; f().lastIndex",
            "0",
        ),
        (r"typeof /a/", "object"),
        (r"({}).toString.call(/a/)", "[object RegExp]"),
        (r"/a/.__proto__ === /b/.__proto__", "true"),
        (r"/a/.exec === /b/.exec", "true"),
        (r"/a/.__proto__.__proto__ === ({}).__proto__", "true"),
        (r#"show(/b/.exec({ toString() { return "abc"; } }))"#, "1:b"),
        (r"show(/1/.exec(31))", "1:1"),
        (r"show(/undefined/.exec())", "0:undefined"),
        (
            r#"show(/a/.exec("a")) + typeof /a/.exec("a").input"#,
            "0:astring",
        ),
    ]);
}

#[test]
fn test_calls_whatever_exec_the_object_has() {
    table(&[
        (
            r#"let r = /a/; r.exec = function () { return {}; }; r.test("zzz")"#,
            "true",
        ),
        (
            r#"let r = /a/; r.exec = function () { return null; }; r.test("a")"#,
            "false",
        ),
        (
            r"let r = /a/; let seen; r.exec = function (s) { seen = this === r && s; return null; }; r.test(5); seen",
            "5",
        ),
        (
            r#"let r = /a/; r.exec = function () { return 1; }; try { r.test("a") } catch (e) { e.name }"#,
            "TypeError",
        ),
        (
            r#"let o = { get exec() { return function () { return {}; }; } }; /a/.test.call(o, "q")"#,
            "true",
        ),
        (
            r#"/a/.test.call({ exec() { return null; } }, "a")"#,
            "false",
        ),
        (
            r#"try { /a/.test.call({}, "a") } catch (e) { e.name }"#,
            "TypeError",
        ),
        (
            r#"try { /a/.test.call(1, "a") } catch (e) { e.name }"#,
            "TypeError",
        ),
        (
            r#"try { /a/.exec.call({}, "a") } catch (e) { e.name }"#,
            "TypeError",
        ),
    ]);
}

#[test]
fn two_calls_after_a_converted_string_are_refused_by_name() {
    // The converted string is in the one slot a second call's answer would
    // be written to (item 221).
    for source in [
        r#"let o = { get exec() { return null; } }; /a/.test.call(o, { toString() { return "a"; } })"#,
        r#"let r = /a/g; r.lastIndex = { valueOf() { return 0; } }; r.exec({ toString() { return "a"; } })"#,
    ] {
        let answered = value(source);
        assert!(
            answered.starts_with('!') && answered.contains("221"),
            "{source}: {answered}"
        );
    }
}

#[test]
fn a_catastrophic_backtrack_is_a_range_error_a_page_catches() {
    // `/(a+)+$/` against thirty `a`s and a `b` is about a thousand million
    // steps in a backtracker. The budget stops it, the page's `catch` sees a
    // `RangeError`, and the engine goes on to match the next pattern.
    let program = r#"let s = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaab";
let caught;
try { /(a+)+$/.test(s); caught = "finished"; } catch (e) { caught = e.name + "|" + e.message; }
caught + "|" + /a+b$/.test(s)"#;
    let answered = value(program);
    assert!(answered.starts_with("RangeError|"), "{answered}");
    assert!(answered.contains("steps of work"), "{answered}");
    assert!(
        answered.ends_with("|true"),
        "and the engine goes on: {answered}"
    );
}

#[test]
fn a_match_that_holds_too_many_places_is_a_range_error_too() {
    // A repeated group holds a few places per iteration, so a million of them
    // is past the ceiling; a repeated character holds one however long it
    // runs, so `.*` over the same string is not.
    let program = r#"let s = "a";
for (let i = 0; i < 20; i++) { s = s + s; }
let caught;
try { /(?:a|b)*c/.test(s); caught = "finished"; } catch (e) { caught = e.name + "|" + e.message; }
caught + "|" + /^.*$/.test(s) + "," + /^a*$/.test(s)"#;
    let answered = value(program);
    assert!(answered.starts_with("RangeError|"), "{answered}");
    assert!(answered.contains("places to come back to"), "{answered}");
    assert!(answered.ends_with("|true,true"), "{answered}");
}

#[test]
fn the_embedders_stop_ends_a_match() {
    // Nothing in this program is a backward jump or a call into a script's
    // function, which are where the interpreter asks; the only place the stop
    // can be seen is inside the matcher.
    let Ok(program) = script(r#"/(a+)+$/.test("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaab")"#) else {
        panic!("a program");
    };
    let Ok(mut engine) = Engine::new() else {
        panic!("an engine");
    };
    engine.stop().ask();
    assert_eq!(
        engine.evaluate(&program),
        Err(Trouble::Escaped(Escape::Interrupted))
    );
    engine.stop().clear();
    let Ok(again) = script(r#"/a/.test("a")"#) else {
        panic!("a program");
    };
    assert_eq!(engine.evaluate(&again), Ok(Value::Bool(true)));
}

#[test]
fn a_pattern_that_is_not_one_is_an_early_error_and_nothing_runs() {
    let Err(error) = script("let x = 1; /(/") else {
        panic!("an unclosed group is not a pattern");
    };
    assert_eq!(
        error.reason,
        alo_js::Reason::Pattern(alo_js::regexp::Wrong::UnclosedGroup)
    );
    assert_eq!(error.at, 12, "at the `(`");
    let Err(legacy) = script(r"/\1/") else {
        panic!("an octal escape is Annex B's");
    };
    assert!(legacy.to_string().contains("Annex B"), "{legacy}");
}

#[test]
fn what_is_not_built_says_which_item_builds_it() {
    for (source, item) in [
        ("/a/i", "322"),
        ("/(?i:a)/", "322"),
        (r"/\p{Lu}/u", "322"),
        ("/[a--b]/v", "324"),
        ("/a/d", "324"),
    ] {
        let answered = value(source);
        assert!(
            answered.starts_with('?') && answered.contains(item),
            "{source} should name queue item {item}: {answered}"
        );
    }
}
