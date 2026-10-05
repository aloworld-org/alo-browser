/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 241, cut from 78: a throw nothing caught is said with where it
//! was — the script, line and column of the throw, then of each call it left.
//!
//! *A throw on a script's third line is placed there; a function one script
//! declared and another called is placed in the first and called from the
//! second; a column in a one-line script tens of kilobytes long is the column,
//! in UTF-16 code units; and a runaway recursion says thirty-two places and
//! how many more it left.*
//!
//! Every place here is counted by hand from the page's own text.

use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;

/// A small window; nothing here is about where anything is drawn.
const WINDOW: Size = Size {
    width: 200.0,
    height: 100.0,
};

/// Load a page made of these scripts, each in its own element, and answer
/// what its load said about them.
fn said(scripts: &[&str]) -> Vec<String> {
    said_about(&scripts.iter().fold(String::new(), |page, script| {
        page + "<script>" + script + "</script>"
    }))
}

/// Load this markup and answer what its load said about its scripts.
fn said_about(markup: &str) -> Vec<String> {
    let mut renderer = Renderer::new(FontDatabase::new());
    let page = Page::new(markup.to_owned(), WINDOW);
    match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded { issues, .. } => issues
            .into_iter()
            .filter(|issue| issue.starts_with("script "))
            .collect(),
        other => vec![format!("not loaded: {other:?}")],
    }
}

// --- The closing clauses ------------------------------------------------------

#[test]
fn a_throw_is_placed_on_its_line_and_at_its_column() {
    assert_eq!(
        said(&[
            "var a = 1;",
            "var b = 2;\nb += a;\n    throw new Error('e')"
        ]),
        vec!["script 2: uncaught: Error: e (at script 2, line 3, column 5)"]
    );
}

#[test]
fn a_function_one_script_declared_is_placed_in_it_and_called_from_another() {
    // `null.x` is at line 2, column 10 of the first; `f()` at line 1,
    // column 9 of the second, whose report it is.
    assert_eq!(
        said(&["function f() {\n  return null.x\n}", "var a = f()"]),
        vec![
            "script 2: uncaught: TypeError: cannot read property 'x' of null \
             (at script 1, line 2, column 10; called from script 2, line 1, column 9)"
        ]
    );
}

#[test]
fn a_column_in_a_long_one_line_script_is_the_column() {
    // Minified: one line, far longer than the distance between the marks a
    // place is counted from, so a mistake carrying a column across one shows.
    let mut script = "a=1;".repeat(5000);
    script.push_str("throw a");
    assert_eq!(
        said(&[&format!("var {script}")]),
        vec!["script 1: uncaught: 1 (at script 1, line 1, column 20005)"]
    );
}

#[test]
fn a_column_counts_utf_16_code_units_rather_than_bytes_or_characters() {
    // `é` is two bytes and one code unit; `😀` four bytes and two code
    // units. Counted from one, `throw` is byte 19, character 15 and code
    // unit 16 — three different answers, and only the last is a column.
    assert_eq!(
        said(&["var s = 'é😀'; throw s"]),
        vec!["script 1: uncaught: \"é😀\" (at script 1, line 1, column 16)"]
    );
}

#[test]
fn a_runaway_recursion_says_thirty_two_places_and_how_many_more() {
    let said = said(&["function r() {\n  r()\n}\nr()"]);
    let [one] = said.as_slice() else {
        panic!("one report: {said:?}");
    };
    let inner = "line 2, column 3";
    assert!(
        one.starts_with(
            "script 1: uncaught: RangeError: this script calls more deeply than this engine will \
             go (at script 1, line 2, column 3; called from script 1, "
        ),
        "{one}"
    );
    assert_eq!(one.matches(inner).count(), 32, "{one}");
    // Ten thousand two hundred and forty calls: the script's own, which is
    // further out than any kept, and every call of `r`.
    assert!(one.ends_with("; and 10208 calls further out)"), "{one}");
}

// --- Around them --------------------------------------------------------------

#[test]
fn every_kind_of_line_ending_ends_a_line() {
    // HTML turns a `\r\n` and a lone `\r` in markup into `\n` before the
    // script sees it; U+2028 and U+2029 reach the script, and the language
    // counts both as ending a line.
    for markup in [
        "<script>var a;\r\n\r\nthrow 1</script>",
        "<script>var a;\r\rthrow 1</script>",
        "<script>var a;\u{2028}\u{2029}throw 1</script>",
    ] {
        assert_eq!(
            said_about(markup),
            vec!["script 1: uncaught: 1 (at script 1, line 3, column 1)"],
            "{markup:?}"
        );
    }
}

#[test]
fn a_script_its_policy_refused_still_counts_so_names_agree_with_the_page() {
    let said = said_about(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"script-src 'nonce-n'\">\
         <script>var refused;</script>\
         <script nonce=\"n\">\n throw 3</script>",
    );
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(
        said.first()
            .is_some_and(|it| it.starts_with("script 1: refused"))
    );
    assert_eq!(
        said.get(1).map(String::as_str),
        Some("script 2: uncaught: 3 (at script 2, line 2, column 2)")
    );
}

#[test]
fn a_job_is_placed_in_the_function_that_was_queued() {
    assert_eq!(
        said(&[
            "function later() {\n  throw 'late'\n}",
            "queueMicrotask(later); var after = 1",
        ]),
        vec!["script 2: uncaught: \"late\" (at script 1, line 2, column 3)"]
    );
}

#[test]
fn a_rethrown_error_is_placed_where_it_was_thrown_again() {
    // The throw that escaped is the second; where the error was made is
    // `error.stack`'s, which is the rest of item 78.
    assert_eq!(
        said(&["var e = new Error('once');\ntry { throw e } catch (c) {\n  throw c\n}"]),
        vec!["script 1: uncaught: Error: once (at script 1, line 3, column 3)"]
    );
}

#[test]
fn a_throw_from_inside_a_builtin_is_placed_at_the_call_that_entered_it() {
    // `queueMicrotask` refuses a non-function from inside itself; a builtin
    // has no source, so the place is the call.
    assert_eq!(
        said(&["var a = 0;\n  queueMicrotask(5)"]),
        vec![
            "script 1: uncaught: TypeError: 5 is not a function, and only a function can be \
             queued (at script 1, line 2, column 3)"
        ]
    );
}

#[test]
fn a_script_that_did_not_parse_is_not_placed_by_this() {
    let said = said(&["var a = ;"]);
    assert!(
        said.first()
            .is_some_and(|it| it.starts_with("script 1: not a script:") && !it.contains("(at ")),
        "{said:?}"
    );
}

#[test]
fn every_prefix_of_a_page_that_throws_from_deep_places_loads() {
    // A hostile page cut short anywhere still loads, and whatever it says
    // about its scripts is said in the shape a report has.
    const PAGE: &str = "<script>function a(n) {\n  if (n) return a(n - 1);\n  throw \
                        new RangeError('é😀')\n}\nqueueMicrotask(() => a(40));\na(3)</script>";
    for at in 0..=PAGE.len() {
        let Some(prefix) = PAGE.get(..at) else {
            continue;
        };
        for issue in said_about(prefix) {
            assert!(issue.starts_with("script 1: "), "{prefix:?} said {issue:?}");
        }
    }
    let whole = said_about(PAGE);
    assert_eq!(whole.len(), 2, "{whole:?}");
    assert!(
        whole.first().is_some_and(|it| it.starts_with(
            "script 1: uncaught: RangeError: é😀 (at script 1, line 3, column 3; called from \
             script 1, line 2, column 17; "
        ) && it
            .ends_with("called from script 1, line 6, column 1)")),
        "{whole:?}"
    );
    // The job: forty-one calls of `a`, and the arrow function that made the
    // first, is forty-two — thirty-two kept and ten counted.
    assert!(
        whole
            .get(1)
            .is_some_and(|it| it.ends_with("; and 10 calls further out)")),
        "{whole:?}"
    );
}
