/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 356's closing conditions, as programs and what they evaluate to
//! (ADR 0036).
//!
//! *A realm with no clock throws a `TypeError` for `Date.now()` and still
//! answers `new Date(0).getTime()`; a fixed clock makes the same answer in
//! every run; and getters and setters agree with ECMA-262's worked values at
//! the range's ends (±8.64 × 10¹⁵) and one past them.*
//!
//! Every program runs twice, once with the collector running at every
//! allocation ([`Heap::stress`]): a date builtin converts its arguments by
//! running the page's script, and keeps each number in a slot across those
//! calls (ADR 0031), which is checked here rather than believed.
//!
//! [`Heap::stress`]: alo_js::Heap::stress

use std::rc::Rc;

use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::{Fixed, numeric, script};

/// The instant every clocked realm here is stopped at:
/// 2026-10-09T13:14:15.678Z, a Friday.
const NOW: f64 = 1_791_551_655_678.0;

/// Run a program in a realm told the time by a fixed clock, both ways.
fn timed(source: &str) -> String {
    both(source, true)
}

/// Run a program in a realm given no clock, both ways.
fn untimed(source: &str) -> String {
    both(source, false)
}

/// Run a program both ways, and answer what it produced.
fn both(source: &str, clocked: bool) -> String {
    let ordinary = run(source, clocked, false);
    let stressed = run(source, clocked, true);
    assert_eq!(
        ordinary, stressed,
        "{source} answered differently when the collector ran at every allocation"
    );
    ordinary
}

/// One run.
fn run(source: &str, clocked: bool, stress: bool) -> String {
    let program = match script(source) {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    let made = if clocked {
        Engine::with_clock(Rc::new(Fixed::at(NOW)))
    } else {
        Engine::new()
    };
    let mut engine = match made {
        Ok(engine) => engine,
        Err(why) => return format!("no engine: {why}"),
    };
    engine.objects().heap_mut().stress(stress);
    match engine.evaluate(&program) {
        Ok(value) => show(&mut engine, value),
        Err(Trouble::Escaped(escape)) => format!("! {escape}"),
        Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
    }
}

/// A value, written out so that a failing test says what it got.
fn show(engine: &mut Engine, value: Value) -> String {
    match value {
        Value::Undefined => "undefined".to_owned(),
        Value::Null => "null".to_owned(),
        Value::Bool(true) => "true".to_owned(),
        Value::Bool(false) => "false".to_owned(),
        Value::Number(number) => numeric::text_of(number),
        Value::Text(held) => match engine.objects().units(held) {
            Some(units) => format!("\"{}\"", String::from_utf16_lossy(units)),
            None => "\"?\"".to_owned(),
        },
        Value::Symbol(_) => "a symbol".to_owned(),
        Value::Object(_) => "an object".to_owned(),
    }
}

/// Check a table of programs, run in a clocked realm, against what each
/// evaluates to.
fn table(cases: &[(&str, &str)]) {
    for (source, expected) in cases {
        assert_eq!(&timed(source), expected, "{source}");
    }
}

/// What a date as text is refused with until item 357.
const AS_TEXT: &str = "! a date as text — Date.parse, new Date(string), Date() and the toString family — is queue item 357";

#[test]
fn a_realm_with_no_clock_refuses_to_say_what_time_it_is_and_still_does_arithmetic() {
    let refused = |at: usize| {
        format!(
            "! TypeError: this realm was given no clock, so it cannot say what time it is (at byte {at})"
        )
    };
    assert_eq!(untimed("Date.now()"), refused(0));
    assert_eq!(untimed("new Date()"), refused(0));
    // `Date()` reads the clock before it would write it as text, so a realm
    // with none answers it with the same error rather than item 357's.
    assert_eq!(untimed("Date()"), refused(0));
    // A page's own `catch` survives it, because it is the language's error.
    assert_eq!(
        untimed("try { Date.now(); } catch (e) { e.name + ': ' + e.message }"),
        "\"TypeError: this realm was given no clock, so it cannot say what time it is\""
    );
    // Nothing that needs no clock is refused.
    assert_eq!(untimed("new Date(0).getTime()"), "0");
    assert_eq!(untimed("Date.UTC(2026, 9, 9)"), "1791504000000");
    assert_eq!(
        untimed("new Date(2026, 9, 9, 13, 14, 15, 678).toISOString()"),
        "\"2026-10-09T13:14:15.678Z\""
    );
}

#[test]
fn a_fixed_clock_answers_the_same_instant_in_every_run() {
    table(&[
        ("Date.now()", "1791551655678"),
        ("new Date().getTime()", "1791551655678"),
        ("Date.now() === Date.now()", "true"),
        ("new Date().toISOString()", "\"2026-10-09T13:14:15.678Z\""),
    ]);
    // A second engine, made afresh, is stopped at the same instant.
    assert_eq!(timed("Date.now()"), timed("Date.now()"));
    // And it is still the realm's clock that `Date()` reads before refusing.
    assert_eq!(timed("Date()"), AS_TEXT);
}

#[test]
fn every_getter_reads_its_field_and_local_time_is_utc() {
    table(&[
        ("new Date().getFullYear()", "2026"),
        ("new Date().getMonth()", "9"),
        ("new Date().getDate()", "9"),
        ("new Date().getDay()", "5"),
        ("new Date().getHours()", "13"),
        ("new Date().getMinutes()", "14"),
        ("new Date().getSeconds()", "15"),
        ("new Date().getMilliseconds()", "678"),
        ("new Date().getUTCFullYear()", "2026"),
        ("new Date().getUTCMonth()", "9"),
        ("new Date().getUTCDate()", "9"),
        ("new Date().getUTCDay()", "5"),
        ("new Date().getUTCHours()", "13"),
        ("new Date().getUTCMinutes()", "14"),
        ("new Date().getUTCSeconds()", "15"),
        ("new Date().getUTCMilliseconds()", "678"),
        ("new Date().getTimezoneOffset()", "0"),
        ("new Date().valueOf()", "1791551655678"),
        // An Invalid Date answers `NaN` from every getter.
        ("new Date(NaN).getHours()", "NaN"),
        ("new Date(NaN).getUTCDay()", "NaN"),
        ("new Date(NaN).getTimezoneOffset()", "NaN"),
        ("new Date(NaN).getTime()", "NaN"),
    ]);
}

#[test]
fn the_ends_of_the_range_are_the_specifications_and_one_past_them_is_invalid() {
    table(&[
        ("new Date(8.64e15).getTime()", "8640000000000000"),
        (
            "new Date(8.64e15).toISOString()",
            "\"+275760-09-13T00:00:00.000Z\"",
        ),
        ("new Date(8.64e15).getUTCDay()", "6"),
        ("new Date(8.64e15 + 1).getTime()", "NaN"),
        ("new Date(-8.64e15).getTime()", "-8640000000000000"),
        (
            "new Date(-8.64e15).toISOString()",
            "\"-271821-04-20T00:00:00.000Z\"",
        ),
        ("new Date(-8.64e15).getFullYear()", "-271821"),
        ("new Date(-8.64e15).getMonth()", "3"),
        ("new Date(-8.64e15).getDate()", "20"),
        ("new Date(-8.64e15 - 1).getTime()", "NaN"),
        ("Date.UTC(275760, 8, 13)", "8640000000000000"),
        ("Date.UTC(275760, 8, 13, 0, 0, 0, 1)", "NaN"),
        ("Date.UTC(-271821, 3, 20)", "-8640000000000000"),
        ("Date.UTC(-271821, 3, 19, 23, 59, 59, 999)", "NaN"),
        // A setter that stays at an end keeps it, and one that steps past it
        // makes an Invalid Date.
        (
            "var d = new Date(8.64e15); d.setUTCMilliseconds(0)",
            "8640000000000000",
        ),
        ("var d = new Date(8.64e15); d.setUTCMilliseconds(1)", "NaN"),
        (
            "var d = new Date(8.64e15); d.setUTCMilliseconds(1); d.getTime()",
            "NaN",
        ),
        (
            "var d = new Date(-8.64e15); d.setUTCMilliseconds(-1)",
            "NaN",
        ),
        ("var d = new Date(-8.64e15); d.setUTCDate(19)", "NaN"),
        (
            "var d = new Date(-8.64e15); d.setUTCHours(1)",
            "-8639999996400000",
        ),
        ("new Date(0).setTime(8.64e15)", "8640000000000000"),
        ("new Date(0).setTime(8.64e15 + 1)", "NaN"),
        ("new Date(0).setTime(-8.64e15 - 1)", "NaN"),
    ]);
}

#[test]
fn the_constructor_reads_numbers_dates_and_fields_but_not_text() {
    table(&[
        ("new Date(2026, 9, 9).getTime()", "1791504000000"),
        ("new Date(2026, 9).getDate()", "1"),
        ("new Date(2026, 0, 1, 25).getDate()", "2"),
        ("new Date(2026, 12, 1).getFullYear()", "2027"),
        // A year from 0 to 99 is in the twentieth century.
        ("new Date(99, 0).getFullYear()", "1999"),
        ("new Date(0, 0).getFullYear()", "1900"),
        ("new Date(100, 0).getFullYear()", "100"),
        ("new Date(-1, 0).getFullYear()", "-1"),
        ("new Date(99.9, 0).getFullYear()", "1999"),
        ("Date.UTC(99)", "915148800000"),
        // One argument is a time value, another date's, or a number.
        ("new Date(new Date(5)).getTime()", "5"),
        ("new Date(1.9).getTime()", "1"),
        ("new Date(-1.9).getTime()", "-1"),
        ("new Date(true).getTime()", "1"),
        ("new Date(null).getTime()", "0"),
        ("new Date(undefined).getTime()", "NaN"),
        ("new Date(NaN, 0).getTime()", "NaN"),
        ("new Date(Infinity).getTime()", "NaN"),
        (
            "new Date({ valueOf: function () { return 7; } }).getTime()",
            "7",
        ),
        // With no hint, an object's `valueOf` is asked first.
        (
            "new Date({ valueOf: function () { return 8; }, toString: function () { return 'x'; } }).getTime()",
            "8",
        ),
        // `new Date()` makes a date; it is a `Date`'s own.
        ("new Date(0).constructor === Date", "true"),
        ("new Date(0).__proto__ === Date.prototype", "true"),
        ("Date.prototype.__proto__ === ({}).__proto__", "true"),
        ("({}).toString.call(new Date(0))", "\"[object Date]\""),
        ("({}).toString.call(Date.prototype)", "\"[object Object]\""),
        ("typeof Date", "\"function\""),
        ("typeof new Date(0)", "\"object\""),
        // Text is item 357's.
        ("new Date('2026-10-09')", AS_TEXT),
        (
            "new Date({ valueOf: function () { return '2026'; } })",
            AS_TEXT,
        ),
        ("Date.parse('2026-10-09')", AS_TEXT),
    ]);
}

#[test]
fn arguments_are_converted_once_each_and_in_order() {
    table(&[
        (
            "var log = ''; function n(v) { return { valueOf: function () { log += v; return v; } }; } \
             new Date(n(1), n(2), n(3), n(4), n(5), n(6), n(7)); log",
            "\"1234567\"",
        ),
        (
            "var log = ''; function n(v) { return { valueOf: function () { log += v; return v; } }; } \
             Date.UTC(n(2026), n(9), n(9)) + log",
            "\"1791504000000202699\"",
        ),
        (
            "var log = ''; function n(v) { return { valueOf: function () { log += v; return v; } }; } \
             new Date(0).setUTCHours(n(1), n(2), n(3), n(4)); log",
            "\"1234\"",
        ),
        // Past the seventh, nothing is converted.
        (
            "var log = ''; function n(v) { return { valueOf: function () { log += v; return v; } }; } \
             new Date(n(1), n(2), n(3), n(4), n(5), n(6), n(7), n(8)); log",
            "\"1234567\"",
        ),
        // A conversion that throws ends the constructor where it was.
        (
            "var log = ''; try { new Date(1, { valueOf: function () { throw 'no'; } }, \
             { valueOf: function () { log += 'late'; return 1; } }); } catch (e) { log += e; } log",
            "\"no\"",
        ),
    ]);
}

#[test]
fn a_setter_works_from_the_value_it_read_first() {
    table(&[
        (
            "var d = new Date(0); d.setUTCHours(5, 6, 7, 8); d.toISOString()",
            "\"1970-01-01T05:06:07.008Z\"",
        ),
        (
            "var d = new Date(0); d.setHours(5, 6, 7, 8); d.toISOString()",
            "\"1970-01-01T05:06:07.008Z\"",
        ),
        (
            "var d = new Date(0); d.setUTCMinutes(90); d.toISOString()",
            "\"1970-01-01T01:30:00.000Z\"",
        ),
        (
            "var d = new Date(0); d.setSeconds(61, 5); d.toISOString()",
            "\"1970-01-01T00:01:01.005Z\"",
        ),
        (
            "var d = new Date(0); d.setMilliseconds(-1); d.toISOString()",
            "\"1969-12-31T23:59:59.999Z\"",
        ),
        (
            "var d = new Date(0); d.setUTCDate(32); d.toISOString()",
            "\"1970-02-01T00:00:00.000Z\"",
        ),
        (
            "var d = new Date(0); d.setMonth(13, 2); d.toISOString()",
            "\"1971-02-02T00:00:00.000Z\"",
        ),
        (
            "var d = new Date(Date.UTC(2024, 1, 29)); d.setUTCFullYear(2025); d.toISOString()",
            "\"2025-03-01T00:00:00.000Z\"",
        ),
        (
            "var d = new Date(0); d.setFullYear(2026, 9, 9); d.getTime()",
            "1791504000000",
        ),
        // `setFullYear` starts an Invalid Date from the epoch; every other
        // setter leaves it invalid.
        (
            "var d = new Date(NaN); d.setFullYear(2026); d.toISOString()",
            "\"2026-01-01T00:00:00.000Z\"",
        ),
        (
            "var d = new Date(NaN); d.setUTCFullYear(2026, 1); d.toISOString()",
            "\"2026-02-01T00:00:00.000Z\"",
        ),
        ("var d = new Date(NaN); d.setHours(1)", "NaN"),
        (
            "var d = new Date(NaN); d.setUTCMonth(1); d.getTime()",
            "NaN",
        ),
        // No argument is `NaN`, which makes an Invalid Date.
        ("var d = new Date(0); d.setHours(); d.getTime()", "NaN"),
        ("var d = new Date(0); d.setTime(); d.getTime()", "NaN"),
        ("new Date(0).setTime(1.9)", "1"),
        ("new Date(0).setTime('5')", "5"),
        // The time value is read before any argument runs script, and the
        // answer is made from it…
        (
            "var d = new Date(0); \
             d.setUTCMinutes({ valueOf: function () { d.setTime(86400000); return 30; } }); \
             d.getTime()",
            "1800000",
        ),
        // …and an Invalid Date read first answers `NaN` without writing over
        // what the argument's `valueOf` set.
        (
            "var d = new Date(NaN); \
             var r = d.setHours({ valueOf: function () { d.setTime(7); return 1; } }); \
             r + ' ' + d.getTime()",
            "\"NaN 7\"",
        ),
    ]);
}

#[test]
fn a_method_called_on_something_that_is_not_a_date_is_a_type_error() {
    table(&[
        (
            "try { Date.prototype.getTime.call({}); } catch (e) { e.name + ': ' + e.message }",
            "\"TypeError: Date.prototype.getTime was called on something that is not a Date\"",
        ),
        (
            "try { Date.prototype.valueOf.call({}); } catch (e) { e.message }",
            "\"Date.prototype.valueOf was called on something that is not a Date\"",
        ),
        (
            "try { Date.prototype.getHours(); } catch (e) { e.message }",
            "\"Date.prototype.getHours was called on something that is not a Date\"",
        ),
        (
            "try { Date.prototype.setUTCHours.call(1, 2); } catch (e) { e.message }",
            "\"Date.prototype.setUTCHours was called on something that is not a Date\"",
        ),
        (
            "try { Date.prototype.toISOString.call({ __proto__: Date.prototype }); } catch (e) { e.message }",
            "\"Date.prototype.toISOString was called on something that is not a Date\"",
        ),
        // The brand is checked before any argument runs script.
        (
            "var log = ''; try { Date.prototype.setTime.call({}, { valueOf: function () { log += 'ran'; } }); } \
             catch (e) { log += e.name; } log",
            "\"TypeError\"",
        ),
        (
            "try { Date.prototype.toString.call({}); } catch (e) { e.name }",
            "\"TypeError\"",
        ),
    ]);
}

#[test]
fn an_invalid_date_has_no_iso_string() {
    table(&[
        (
            "try { new Date(NaN).toISOString(); } catch (e) { e.name + ': ' + e.message }",
            "\"RangeError: an Invalid Date has no ISO string\"",
        ),
        (
            "new Date(Date.UTC(-1, 0, 1)).toISOString()",
            "\"-000001-01-01T00:00:00.000Z\"",
        ),
        (
            "new Date(Date.UTC(10000, 0, 1)).toISOString()",
            "\"+010000-01-01T00:00:00.000Z\"",
        ),
        ("new Date(0).toJSON()", "\"1970-01-01T00:00:00.000Z\""),
        ("new Date(NaN).toJSON()", "null"),
        // `toJSON` is generic: anything with a finite number and a
        // `toISOString` of its own.
        (
            "Date.prototype.toJSON.call({ valueOf: function () { return 1; }, toISOString: function () { return 'mine'; } })",
            "\"mine\"",
        ),
        (
            "Date.prototype.toJSON.call({ valueOf: function () { return Infinity; } })",
            "null",
        ),
        (
            "try { Date.prototype.toJSON.call({ valueOf: function () { return 1; }, toISOString: 1 }); } \
             catch (e) { e.name }",
            "\"TypeError\"",
        ),
        (
            "try { Date.prototype.toJSON.call(null); } catch (e) { e.name }",
            "\"TypeError\"",
        ),
    ]);
}

#[test]
fn a_date_is_text_with_no_hint_and_a_number_with_one() {
    table(&[
        // A number is asked for: `valueOf`, through `Symbol.toPrimitive`.
        ("new Date(5) - 0", "5"),
        ("new Date(5) * 2", "10"),
        ("new Date(5) < new Date(6)", "true"),
        ("+new Date(9)", "9"),
        // No hint is a string, which is `toString` — item 357's — rather than
        // the number `valueOf` would have answered.
        ("new Date(0) + ''", AS_TEXT),
        ("new Date(0) + 1", AS_TEXT),
        ("`${new Date(0)}`", AS_TEXT),
        ("new Date(0) == 0", AS_TEXT),
        // A date's own `toString` is asked first with no hint, and its own
        // `valueOf` first with a number.
        (
            "var d = new Date(0); d.toString = function () { return 'when'; }; d + 1",
            "\"when1\"",
        ),
        (
            "var d = new Date(0); d.valueOf = function () { return 2; }; d * 3",
            "6",
        ),
        // A `toString` that answers an object moves on to `valueOf`, as
        // `OrdinaryToPrimitive` does.
        (
            "var d = new Date(4); d.toString = function () { return {}; }; d + ''",
            "\"4\"",
        ),
        (
            "var d = new Date(4); d.toString = function () { return {}; }; d.valueOf = function () { return {}; }; \
             try { d + ''; } catch (e) { e.name }",
            "\"TypeError\"",
        ),
        // An object that inherits from `Date.prototype` without being a date
        // reaches its `Symbol.toPrimitive`, which asks `toString` first, which
        // is the language's `TypeError` for something that is not a date.
        (
            "try { ({ __proto__: Date.prototype }) + ''; } catch (e) { e.name }",
            "\"TypeError\"",
        ),
        // Objects with no `Symbol.toPrimitive` are converted as before.
        ("({ valueOf: function () { return 1; } }) + ''", "\"1\""),
        ("({}) + ''", "\"[object Object]\""),
    ]);
}

#[test]
fn what_is_refused_is_refused_by_name_and_what_is_absent_is_absent() {
    table(&[
        ("new Date(0).toString()", AS_TEXT),
        ("new Date(0).toDateString()", AS_TEXT),
        ("new Date(0).toTimeString()", AS_TEXT),
        ("new Date(0).toUTCString()", AS_TEXT),
        // Annex B's are absent, and the `Intl` ones wait for it.
        ("typeof new Date(0).getYear", "\"undefined\""),
        ("typeof new Date(0).setYear", "\"undefined\""),
        ("typeof new Date(0).toGMTString", "\"undefined\""),
        ("typeof new Date(0).toLocaleDateString", "\"undefined\""),
        ("typeof new Date(0).toLocaleTimeString", "\"undefined\""),
    ]);
}

#[test]
fn hostile_numbers_make_an_invalid_date_and_never_a_panic() {
    table(&[
        ("Date.UTC(1e300, 1e300, 1e300)", "NaN"),
        (
            "Date.UTC(1.7976931348623157e308, 1.7976931348623157e308)",
            "NaN",
        ),
        ("new Date(2026, 0, 1e300).getTime()", "NaN"),
        ("new Date(-1e300).getTime()", "NaN"),
        ("Date.UTC(2026, 0, 1, 0, 0, 0, -Infinity)", "NaN"),
        ("Date.UTC(0, 1e15)", "NaN"),
        (
            "var d = new Date(0); d.setUTCFullYear(1e300); d.getTime()",
            "NaN",
        ),
        (
            "var d = new Date(0); d.setUTCDate(-1e20); d.getTime()",
            "NaN",
        ),
        ("Date.UTC()", "NaN"),
        ("Date.UTC(2026)", "1767225600000"),
    ]);
}
