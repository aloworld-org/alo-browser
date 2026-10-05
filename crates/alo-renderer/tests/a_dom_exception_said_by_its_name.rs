/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 252, found by 250: a `DOMException` a page let escape is
//! reported by its name and message.
//!
//! *Every `DOMException` a member of item 80's throws is reported with its
//! name and message, read without running any of the page's code, and every
//! other object is reported as now.*
//!
//! Each page runs through a real [`Renderer`]. What a page's `catch` reads
//! through the exception's getters is read back from the page's own engine,
//! so each report is checked against what the page itself would have seen —
//! and what a getter the page planted did is read back the same way, which
//! is how a test can tell that the report ran nothing.

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

/// Load a page whose body is this script, and answer the renderer and the
/// issues about scripts its load came back with, each without its place.
fn run(source: &str) -> (Renderer, Vec<String>) {
    let mut renderer = Renderer::new(FontDatabase::new());
    let page = Page::new(
        format!("<!DOCTYPE html><html><head></head><body><script>{source}</script></body></html>"),
        WINDOW,
    );
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
/// *Where* is item 241's and `where_a_page_threw.rs` asserts it; this file
/// is about *what* is said. The place is checked to be in the page's one
/// script, on its one line, and set aside — a report with no place, or a
/// place somewhere else, is left whole and fails the test that reads it.
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

/// Each member of item 80's that throws, made to refuse, with the
/// exception's name and message as the standard and `alo-dom` give them.
const REFUSED: [(&str, &str, &str); 8] = [
    (
        "document.documentElement.appendChild(document)",
        "HierarchyRequestError",
        "a node cannot be put inside itself or anything inside it",
    ),
    (
        "document.appendChild(document.createElement('p'))",
        "HierarchyRequestError",
        "a document can hold only one element",
    ),
    (
        "document.appendChild(document.createTextNode('t'))",
        "HierarchyRequestError",
        "a document cannot hold text",
    ),
    (
        "document.documentElement.insertBefore(document.createElement('p'), document)",
        "NotFoundError",
        "the node to insert before is not a child of this parent",
    ),
    (
        "document.documentElement.removeChild(document.createElement('p'))",
        "NotFoundError",
        "the node to remove is not a child of this parent",
    ),
    (
        "document.documentElement.replaceChild(document.createElement('p'), \
         document.createElement('i'))",
        "NotFoundError",
        "the node to replace is not a child of this parent",
    ),
    (
        "document.createElement('a b')",
        "InvalidCharacterError",
        "an element's name must be a valid element local name",
    ),
    (
        "document.documentElement.setAttribute('a b', 'x')",
        "InvalidCharacterError",
        "an attribute's name must be a valid attribute local name",
    ),
];

// --- The closing clauses ------------------------------------------------------

#[test]
fn every_refusal_a_member_throws_is_said_by_its_name_and_message() {
    for (call, name, message) in REFUSED {
        assert_eq!(
            said(call),
            format!("script 1: uncaught: {name}: {message}"),
            "{call}"
        );
    }
}

#[test]
fn what_is_said_is_what_the_pages_own_catch_reads() {
    for (call, _, _) in REFUSED {
        let (mut renderer, issues) = run(&format!(
            "try {{ {call} }} catch (e) {{ seen = e.name + ': ' + e.message; throw e }}"
        ));
        let seen = read(&mut renderer, "seen");
        assert_eq!(
            issues,
            vec![format!("script 1: uncaught: {seen}")],
            "{call}"
        );
    }
}

#[test]
fn the_exceptions_getters_are_never_called_and_never_trusted() {
    // The page takes `DOMException.prototype`'s two getters away, puts a
    // name of its own where they were, and a counting getter for `message`
    // behind it. Its `catch` now reads the page's words; the report reads
    // the exception.
    let (mut renderer, issues) = run("out = 0; \
         try { document.createElement('a b') } catch (e) { \
           const p = e.__proto__; delete p.name; delete p.message; \
           p.name = 'Spoofed'; \
           p.__proto__ = { __proto__: p.__proto__, get message() { out += 1; return 'm' } }; \
           seen = e.name + ': ' + e.message; out = 0; \
           throw e \
         }");
    assert_eq!(
        read(&mut renderer, "seen"),
        "Spoofed: m",
        "the page's own words"
    );
    assert_eq!(
        issues,
        vec![
            "script 1: uncaught: InvalidCharacterError: an element's name must be a valid \
             element local name"
        ]
    );
    assert_eq!(read(&mut renderer, "out"), "0", "the report ran the getter");
}

#[test]
fn an_object_that_inherits_from_dom_exception_prototype_is_still_an_object() {
    // Which object is a `DOMException` is asked of the cell, never of its
    // chain: one that only inherits the getters is not one.
    assert_eq!(
        said(
            "let p; try { document.createElement('a b') } catch (e) { p = e.__proto__ } \
              throw { __proto__: p }"
        ),
        "script 1: uncaught: an object"
    );
}

#[test]
fn every_other_object_is_said_as_before() {
    assert_eq!(
        said("throw { name: 'HierarchyRequestError', message: 'x' }"),
        "script 1: uncaught: an object"
    );
    assert_eq!(
        said("const e = new Error('x'); e.name = 'NotFoundError'; throw e"),
        "script 1: uncaught: NotFoundError: x"
    );
    assert_eq!(
        said("document.documentElement.firstChild.x.y"),
        "script 1: uncaught: TypeError: cannot read property 'y' of undefined"
    );
}

// --- And around them ----------------------------------------------------------

#[test]
fn a_refusal_thrown_through_a_function_is_said_with_where_it_was() {
    let mut renderer = Renderer::new(FontDatabase::new());
    let page = Page::new(
        "<!DOCTYPE html><html><body><script>\nfunction put(into) {\n  \
         into.appendChild(document)\n}\nput(document.documentElement)\n</script></body></html>"
            .to_owned(),
        WINDOW,
    );
    let FromRenderer::Loaded { issues, .. } = renderer.handle(ToRenderer::Load(Box::new(page)))
    else {
        panic!("the page did not load");
    };
    assert_eq!(
        issues,
        vec![
            "script 1: uncaught: HierarchyRequestError: a node cannot be put inside itself or \
             anything inside it (at script 1, line 3, column 3; called from script 1, line 5, \
             column 1)"
        ]
    );
}

#[test]
fn a_page_that_throws_one_and_then_another_has_each_said() {
    let mut renderer = Renderer::new(FontDatabase::new());
    let page = Page::new(
        "<!DOCTYPE html><html><body>\
         <script>document.createElement('')</script>\
         <script>document.body</script>\
         <script>document.documentElement.removeChild(document)</script>\
         </body></html>"
            .to_owned(),
        WINDOW,
    );
    let FromRenderer::Loaded { issues, .. } = renderer.handle(ToRenderer::Load(Box::new(page)))
    else {
        panic!("the page did not load");
    };
    let said: Vec<&str> = issues
        .iter()
        .filter_map(|issue| issue.split_once(" (at ").map(|(said, _)| said))
        .collect();
    assert_eq!(
        said,
        vec![
            "script 1: uncaught: InvalidCharacterError: an element's name must be a valid \
             element local name",
            "script 3: uncaught: NotFoundError: the node to remove is not a child of this parent",
        ],
        "{issues:?}"
    );
}
