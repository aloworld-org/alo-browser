/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Item 205: plain function headers are checked under the body's strictness.
//! These are early-error regressions, not a claim that a new web page runs.
//! The contract is ECMAScript's Function Definitions / Early Errors and
//! Identifiers / Early Errors; the queue retains unsupported forms separately.
//!
//! <https://tc39.es/ecma262/multipage/ecmascript-language-functions-and-classes.html#sec-function-definitions-static-semantics-early-errors>
//! <https://tc39.es/ecma262/multipage/ecmascript-language-expressions.html#sec-identifiers-static-semantics-early-errors>

use std::fmt::Write;

use alo_js::compile::{Refusal, What, compile};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::script;

fn early_error(source: &str) -> Result<(), String> {
    let program = script(source).map_err(|why| format!("{source}: {why}"))?;
    match compile(&program) {
        Err(Refusal::NotAProgram { at, why }) if at < source.len() && !why.is_empty() => Ok(()),
        other => Err(format!("{source} needs an early error, got {other:?}")),
    }
}

#[test]
fn a_body_directive_rechecks_every_strict_reserved_parameter() {
    for name in [
        "implements",
        "interface",
        "let",
        "package",
        "private",
        "protected",
        "public",
        "static",
        "yield",
        "eval",
        "arguments",
        r"\u0065val",
    ] {
        for source in [
            format!("function f({name}) {{ 'use strict'; }}"),
            format!("(function ({name}) {{ 'use strict'; }})"),
            format!("({name}) => {{ 'use strict'; }}"),
            format!("({{ method({name}) {{ 'use strict'; }} }})"),
            format!("({{ set value({name}) {{ 'use strict'; }} }})"),
        ] {
            early_error(&source).unwrap();
        }
    }
}

#[test]
fn a_function_name_is_a_binding_but_a_method_name_is_a_property() {
    for name in ["eval", "arguments", "implements", "yield", r"\u0065val"] {
        early_error(&format!("function {name}() {{ 'use strict'; }}")).unwrap();
        early_error(&format!("(function {name}() {{ 'use strict'; }})")).unwrap();
    }
    let program = script("({ eval() { 'use strict'; return 42; } }).eval() ").unwrap();
    let mut engine = Engine::new().unwrap();
    assert_eq!(engine.evaluate(&program), Ok(Value::Number(42.0)));
}

#[test]
fn duplicate_strict_parameters_and_arrow_parameters_are_early_errors() {
    for source in [
        "function f(a, a) { 'use strict'; }",
        "'use strict'; function f(a, a) {}",
        "(function (a, a) { 'use strict'; })",
        "(a, a) => { 'use strict'; }",
        "(a, a) => a",
        "({ method(a, a) { 'use strict'; } })",
        r"function f(a, \u0061) { 'use strict'; }",
    ] {
        early_error(source).unwrap();
    }
}

#[test]
fn inherited_strictness_checks_eval_and_arguments_too() {
    for source in [
        "'use strict'; function f(eval) {}",
        "function outer() { 'use strict'; return function (arguments) {}; }",
        "'use strict'; (eval) => 1",
    ] {
        early_error(source).unwrap();
    }
}

#[test]
fn a_function_body_cannot_redeclare_its_parameter_lexically() {
    for source in [
        "function f(a) { let a; }",
        "function f(a) { const a = 1; }",
        "function f() { let a; let a; }",
        "function f() { let a; { var a; } }",
        "outer: while (false) { function f() { break outer; } }",
    ] {
        early_error(source).unwrap();
    }
}

#[test]
fn legal_shadowing_and_non_directives_keep_their_meaning() {
    for source in [
        "function f(a) { 'use strict'; var a; return a; } f(42)",
        "let a = 7; function f() { let a = 42; return a; } f()",
        "function f(a) { { let a = 7; } return a; } f(42)",
        "function strict() { 'use strict'; } function f(eval) { return eval; } f(42)",
        r"function f(eval) { 'use\u0020strict'; return eval; } f(42)",
        "function f(eval) { ('use strict'); return eval; } f(42)",
        "function f(eval) { ('other'); 'use strict'; return eval; } f(42)",
        "function f(eval) { /* before */ ('use strict'); return eval; } f(42)",
        "function f(eval) { 0; 'use strict'; return eval; } f(42)",
        "function f(await) { 'use strict'; return await; } f(42)",
    ] {
        for stress in [false, true] {
            let program = script(source).unwrap();
            let mut engine = Engine::new().unwrap();
            engine.objects().heap_mut().stress(stress);
            assert_eq!(
                engine.evaluate(&program),
                Ok(Value::Number(42.0)),
                "{source}"
            );
        }
    }
    let program = script("function f(a, a) {}").unwrap();
    assert!(
        matches!(
            compile(&program),
            Err(Refusal::NotBuiltYet {
                what: What::AParameterForm,
                ..
            })
        ),
        "sloppy duplicate semantics remain explicitly unbuilt"
    );
}

#[test]
fn an_early_error_runs_no_statement_and_the_engine_remains_usable() {
    for stress in [false, true] {
        let mut engine = Engine::new().unwrap();
        engine.objects().heap_mut().stress(stress);
        engine
            .evaluate(&script("var marker = 0;").unwrap())
            .unwrap();
        let source = "marker = 1; (function (eval) { 'use strict'; });";
        assert!(matches!(
            engine.evaluate(&script(source).unwrap()),
            Err(Trouble::NotCompiled(Refusal::NotAProgram { .. }))
        ));
        assert_eq!(
            engine.evaluate(&script("marker").unwrap()),
            Ok(Value::Number(0.0))
        );
        assert_eq!(
            engine.evaluate(&script("marker = 42").unwrap()),
            Ok(Value::Number(42.0))
        );
    }
}

#[test]
fn malformed_and_truncated_headers_return_without_panicking() {
    for source in [
        r"function f(\u0065val) { 'use strict'; }",
        "(α, α) => { 'use strict'; }",
        "function f(a,,b) { 'use strict'; }",
        "function f(\\u{FFFFFF}) { 'use strict'; }",
    ] {
        for end in 0..=source.len() {
            if let Some(prefix) = source.get(..end) {
                if let Ok(program) = script(prefix) {
                    let _ = compile(&program);
                }
            }
        }
    }
    let mut many = String::from("function f(");
    for n in 0..1000 {
        write!(many, "p{n},").unwrap();
    }
    many.push_str("eval) { 'use strict'; }");
    early_error(&many).unwrap();
}
