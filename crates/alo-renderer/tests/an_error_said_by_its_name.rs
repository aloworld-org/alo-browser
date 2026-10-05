/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 239, cut from 236: an error a page threw and nothing caught is
//! said by its `name` and `message`, read without running any of the page's
//! script.
//!
//! *An uncaught `new TypeError('x')` is reported `TypeError: x`; an error
//! whose `name` was reassigned is said by the new name; and a `message`
//! getter is never called.*
//!
//! Each page runs through a real [`Renderer`], and what a getter or a
//! `toString` did is read back from the page's own engine — which is how a
//! test can tell that describing the error ran nothing.

use alo_js::interpret::Trouble;
use alo_js::{Value, script};
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;

/// A small window; nothing here is about where anything is.
const WINDOW: Size = Size {
    width: 200.0,
    height: 100.0,
};

/// Load a page whose only content is this script, and answer the renderer
/// and the issues about scripts its load came back with.
fn run(source: &str) -> (Renderer, Vec<String>) {
    let mut renderer = Renderer::new(FontDatabase::new());
    let page = Page::new(format!("<script>{source}</script>"), WINDOW);
    let issues = match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded { issues, .. } => issues,
        other => vec![format!("not loaded: {other:?}")],
    };
    let issues = issues
        .into_iter()
        .filter(|issue| issue.starts_with("script "))
        .map(|issue| without_its_place(&issue))
        .collect();
    (renderer, issues)
}

/// An uncaught throw's report with where it was taken off the end.
///
/// *Where* is queue item 241's and is asserted in
/// `where_a_page_threw.rs`; this file is about *what* is said. So the place
/// is checked to be there and to be in the page's one script, on its one
/// line, and is then set aside — a report with no place, or a place
/// somewhere else, is left whole and fails whichever test reads it.
fn without_its_place(issue: &str) -> String {
    if !issue.contains("uncaught: ") {
        return issue.to_owned();
    }
    match issue.rsplit_once(" (at script 1, line 1, column ") {
        Some((said, column))
            if column
                .strip_suffix(')')
                .is_some_and(|number| number.parse::<usize>().is_ok()) =>
        {
            said.to_owned()
        }
        _ => issue.to_owned(),
    }
}

/// What one expression evaluates to in the page's engine, as text.
fn read(renderer: &mut Renderer, expression: &str) -> String {
    let Some(looping) = renderer.event_loop() else {
        return "no script ran".to_owned();
    };
    let program = match script(expression) {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    match looping.engine().evaluate(&program) {
        Ok(Value::Text(held)) => match looping.engine().objects().units(held) {
            Some(units) => String::from_utf16_lossy(units),
            None => "a string that has gone".to_owned(),
        },
        Ok(Value::Number(number)) => number.to_string(),
        Ok(other) => format!("{other:?}"),
        Err(Trouble::Escaped(escape)) => format!("! {escape}"),
        Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
    }
}

/// The one thing a page's script reported, or every one if there were more.
fn said(source: &str) -> String {
    let (_, issues) = run(source);
    match issues.as_slice() {
        [one] => one.clone(),
        many => format!("{many:?}"),
    }
}

// --- The closing clauses ------------------------------------------------------

#[test]
fn an_uncaught_type_error_is_said_by_its_name_and_message() {
    assert_eq!(
        said("throw new TypeError('x')"),
        "script 1: uncaught: TypeError: x"
    );
}

#[test]
fn an_error_whose_name_was_reassigned_is_said_by_the_new_name() {
    assert_eq!(
        said("const e = new TypeError('x'); e.name = 'NotFound'; throw e"),
        "script 1: uncaught: NotFound: x"
    );
    // Reassigned on the prototype, where every instance finds it.
    assert_eq!(
        said("RangeError.prototype.name = 'Bounds'; throw new RangeError('far')"),
        "script 1: uncaught: Bounds: far"
    );
}

#[test]
fn a_message_getter_is_never_called() {
    let (mut renderer, issues) = run("out = 0; const e = new Error('mine'); delete e.message; \
         e.__proto__ = { __proto__: Error.prototype, \
         get message() { out += 1; return 'from a getter' } }; \
         throw e");
    assert_eq!(
        issues,
        vec!["script 1: uncaught: Error (its message is a getter, which was not called)"]
    );
    assert_eq!(read(&mut renderer, "out"), "0", "the getter ran");
}

// --- And around them ----------------------------------------------------------

#[test]
fn every_one_of_the_seven_is_said_by_its_own_name() {
    for name in [
        "Error",
        "EvalError",
        "RangeError",
        "ReferenceError",
        "SyntaxError",
        "TypeError",
        "URIError",
    ] {
        assert_eq!(
            said(&format!("throw new {name}('m')")),
            format!("script 1: uncaught: {name}: m")
        );
    }
}

#[test]
fn an_error_made_without_new_is_the_same_error() {
    assert_eq!(
        said("throw TypeError('called')"),
        "script 1: uncaught: TypeError: called"
    );
}

#[test]
fn an_empty_message_or_name_is_left_out_as_to_string_leaves_it_out() {
    assert_eq!(
        said("throw new TypeError()"),
        "script 1: uncaught: TypeError"
    );
    assert_eq!(
        said("const e = new Error('only this'); e.name = ''; throw e"),
        "script 1: uncaught: only this"
    );
    assert_eq!(
        said("const e = new Error(); e.name = ''; throw e"),
        "script 1: uncaught: an error with an empty name and message"
    );
}

#[test]
fn a_name_nothing_defines_is_error() {
    assert_eq!(
        said(
            "delete TypeError.prototype.name; delete Error.prototype.name; \
             throw new TypeError('anonymous')"
        ),
        "script 1: uncaught: Error: anonymous"
    );
}

#[test]
fn a_name_getter_is_never_called_either() {
    let (mut renderer, issues) = run("out = 0; const e = new TypeError('x'); \
         e.__proto__ = { __proto__: TypeError.prototype, \
         get name() { out += 1; return 'Chosen' } }; throw e");
    assert_eq!(
        issues,
        vec!["script 1: uncaught: an error: x (its name is a getter, which was not called)"]
    );
    assert_eq!(read(&mut renderer, "out"), "0", "the getter ran");
}

#[test]
fn a_message_that_is_an_object_is_not_converted() {
    let (mut renderer, issues) = run("out = 0; const e = new Error('x'); \
         e.message = { toString() { out += 1; return 'converted' } }; throw e");
    assert_eq!(
        issues,
        vec!["script 1: uncaught: Error (its message is an object, which was not converted)"]
    );
    assert_eq!(read(&mut renderer, "out"), "0", "toString ran");
}

#[test]
fn a_primitive_message_that_is_not_a_string_is_spelled_as_to_string_would() {
    assert_eq!(
        said("const e = new Error('x'); e.message = 404; e.name = false; throw e"),
        "script 1: uncaught: false: 404"
    );
}

#[test]
fn a_page_replacing_to_string_does_not_choose_what_its_error_says() {
    let (mut renderer, issues) = run("out = 0; Error.prototype.toString = function () { \
         out += 1; return 'all is well' }; throw new Error('broken')");
    assert_eq!(issues, vec!["script 1: uncaught: Error: broken"]);
    assert_eq!(read(&mut renderer, "out"), "0", "toString ran");
}

#[test]
fn an_object_that_only_looks_like_an_error_is_still_an_object() {
    assert_eq!(
        said("throw { name: 'TypeError', message: 'x' }"),
        "script 1: uncaught: an object"
    );
}

#[test]
fn an_error_the_engine_threw_and_a_page_rethrew_is_said_the_same_way() {
    assert_eq!(
        said("try { null.x } catch (e) { throw e }"),
        "script 1: uncaught: TypeError: cannot read property 'x' of null"
    );
}

#[test]
fn an_error_thrown_from_a_job_is_said_by_its_name() {
    assert_eq!(
        said("queueMicrotask(() => { throw new RangeError('later') })"),
        "script 1: uncaught: RangeError: later"
    );
}

#[test]
fn a_long_message_is_cut_and_what_was_left_out_counted() {
    // 2^13 code units, eight times what a report repeats.
    let issue = said("let s = 'x'; for (let i = 0; i < 13; i++) { s += s } throw new Error(s)");
    let Some(rest) = issue.strip_prefix("script 1: uncaught: Error: ") else {
        panic!("{issue}");
    };
    assert_eq!(rest.chars().filter(|&it| it == 'x').count(), 1024);
    assert!(rest.ends_with("… and 7168 more code units"), "{rest}");

    let issue = said("let s = 'y'; for (let i = 0; i < 13; i++) { s += s } throw s");
    assert!(
        issue.ends_with("\"… and 7168 more code units"),
        "a thrown string is cut too: {}",
        issue.len()
    );
}
