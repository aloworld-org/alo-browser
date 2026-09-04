/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Two real pages' own scripts, frozen, read all the way to a tree.
//!
//! `LOOP.md`'s stage 2 clause 1: an item is opened by something real that
//! fails and closed by the same thing working, and what it is judged against is
//! **frozen, never fetched**. Both live in `crates/alo-corpus/scripts/`, each
//! with an `origin.txt` beside it saying where it came from, what it covers and
//! what it does not.
//!
//! # Why two, and why the second one is the one that matters here
//!
//! `alo-service-worker/script.js` is what queue item 70's lexer closed on, and
//! its own `origin.txt` records that it holds no regular expression at all — so
//! it never asks the one question [`alo_js::Goal`] exists to answer. Item 204
//! said freezing a second was part of the item, and
//! `alo-theme-generator/script.mjs` is it: six patterns, in four different
//! positions, and **no division anywhere in the file**.
//!
//! That last part is asserted rather than assumed. A parser that read a pattern
//! as arithmetic would produce a perfectly reasonable tree — `/^(bg|text)/`
//! divides, then `^`, then `(bg|text)`, then divides again — and every other
//! assertion here would still pass. So the test walks the tree for a division
//! and requires there to be none, which is the same shape of assertion as the
//! lexer's "the gap between two tokens must lex as nothing".

use std::path::{Path, PathBuf};

use alo_js::ast::{
    Argument, ArrayElement, Binary, Body, Class, ClassMember, Declaration, Export, Expression,
    ExpressionKind, ForInit, ForTarget, Function, Key, Member, Pattern, Property, Statement,
    StatementKind,
};
use alo_js::{Program, Source};

/// Where the frozen scripts are.
///
/// Read by path rather than through `alo-corpus`, because ADR 0013 § 5 gives
/// this crate no dependencies — a route through the corpus would put the whole
/// renderer behind a parser, and behind a dependency cycle the day the renderer
/// runs script.
fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("alo-corpus")
        .join("scripts")
}

/// One frozen script, parsed — or a sentence saying where it stopped, which
/// the test it was called from turns into the failure.
fn parse(name: &str, file: &str, source: Source) -> Result<Program, String> {
    let path = corpus().join(name).join(file);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("{} could not be read: {error}", path.display()))?;
    let parsed = match source {
        Source::Script => alo_js::script(&text),
        Source::Module => alo_js::module(&text),
    };
    parsed.map_err(|error| {
        let at = alo_js::Position::of(&text, error.at);
        let line = text.lines().nth(at.line.saturating_sub(1)).unwrap_or("");
        format!("{file} was refused at {at}: {error}\n  {line}")
    })
}

#[test]
fn alos_service_worker_parses() {
    let program = parse("alo-service-worker", "script.js", Source::Script)
        .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(program.source, Source::Script);
    assert!(
        !program.strict,
        "it has no `use strict` and is not a module"
    );
    // Three `const`s and seven `self.addEventListener(…)`, in the order the
    // file writes them — a count that changes if a statement is quietly
    // swallowed by the one before it.
    assert_eq!(program.body.len(), 10);
    let expressions = program
        .body
        .iter()
        .filter(|statement| matches!(statement.kind, StatementKind::Expression(_)))
        .count();
    assert_eq!(expressions, 5);
    // Every one of its arrow functions really is one, rather than a
    // parenthesised expression that happened to parse.
    let mut arrows = 0;
    let mut plain = 0;
    for function in functions(&program) {
        if function.is_arrow {
            arrows += 1;
        } else {
            plain += 1;
        }
    }
    assert_eq!((arrows, plain), (16, 1));
}

#[test]
fn alos_theme_generator_parses_as_the_module_it_is() {
    let program = parse("alo-theme-generator", "script.mjs", Source::Module)
        .unwrap_or_else(|why| panic!("{why}"));
    assert_eq!(program.source, Source::Module);
    // A module is strict code without saying so, which is one of the two ways
    // the same bytes mean different things under the two goals.
    assert!(program.strict);
    let imports = program
        .body
        .iter()
        .filter(|statement| matches!(statement.kind, StatementKind::Import(_)))
        .count();
    assert_eq!(imports, 3);
}

#[test]
fn every_slash_in_the_generator_opens_a_pattern_and_none_of_them_divides() {
    let program = parse("alo-theme-generator", "script.mjs", Source::Module)
        .unwrap_or_else(|why| panic!("{why}"));
    let mut patterns = Vec::new();
    let mut divisions = 0;
    walk(&program, &mut |expression| match &expression.kind {
        ExpressionKind::RegularExpression(literal) => patterns.push(literal.flags.clone()),
        ExpressionKind::Binary {
            operator: Binary::Divide,
            ..
        } => divisions += 1,
        _ => {}
    });
    // The assertion the whole second script exists for: a `/` read as division
    // would be a different program that parses perfectly well.
    assert_eq!(divisions, 0, "no `/` in this file is arithmetic");
    assert_eq!(patterns.len(), 6);
    assert_eq!(
        patterns
            .iter()
            .filter(|flags| flags.as_str() == "g")
            .count(),
        2
    );
}

#[test]
fn the_generators_templates_keep_what_the_author_wrote() {
    let program = parse("alo-theme-generator", "script.mjs", Source::Module)
        .unwrap_or_else(|why| panic!("{why}"));
    let mut with_substitutions = 0;
    let mut escaped_backticks = 0;
    walk(&program, &mut |expression| {
        if let ExpressionKind::Template(template) = &expression.kind {
            if !template.expressions.is_empty() {
                with_substitutions += 1;
            }
            escaped_backticks += template
                .pieces
                .iter()
                .filter(|piece| piece.raw.contains("\\`"))
                .count();
        }
    });
    assert_eq!(with_substitutions, 6);
    // The multi-line template that writes CSS holding backticks — the piece
    // that makes the `}` of a substitution a real question rather than a
    // theoretical one.
    assert!(escaped_backticks > 0);
}

// --- Walking the tree -------------------------------------------------------
//
// Written out rather than derived, because a visitor that missed a branch would
// make every assertion above weaker without failing: an unvisited subtree holds
// no divisions either. The last arm of each match is a wildcard because the
// tree's enums are `#[non_exhaustive]` — which is exactly the hole just named,
// so a variant added later is worth coming back here for.

/// Every function in a program, arrows included.
fn functions(program: &Program) -> Vec<Function> {
    let mut found = Vec::new();
    walk(program, &mut |expression| {
        if let ExpressionKind::Function(function) = &expression.kind {
            found.push((**function).clone());
        }
    });
    for statement in &program.body {
        collect_statement_functions(statement, &mut found);
    }
    found
}

fn collect_statement_functions(statement: &Statement, found: &mut Vec<Function>) {
    if let StatementKind::Function(function) = &statement.kind {
        found.push((**function).clone());
    }
}

/// Call `visit` on every expression in the program, in no particular order.
fn walk(program: &Program, visit: &mut impl FnMut(&Expression)) {
    for statement in &program.body {
        walk_statement(statement, visit);
    }
}

fn walk_statements(body: &[Statement], visit: &mut impl FnMut(&Expression)) {
    for statement in body {
        walk_statement(statement, visit);
    }
}

fn walk_statement(statement: &Statement, visit: &mut impl FnMut(&Expression)) {
    match &statement.kind {
        StatementKind::Expression(value) | StatementKind::Throw(value) => {
            walk_expression(value, visit);
        }
        StatementKind::Block(body) => walk_statements(body, visit),
        StatementKind::Declaration(declaration) => walk_declaration(declaration, visit),
        StatementKind::Function(function) => walk_function(function, visit),
        StatementKind::Class(class) => walk_class(class, visit),
        StatementKind::If {
            test,
            consequent,
            alternate,
        } => {
            walk_expression(test, visit);
            walk_statement(consequent, visit);
            if let Some(alternate) = alternate {
                walk_statement(alternate, visit);
            }
        }
        StatementKind::For {
            init,
            test,
            update,
            body,
        } => {
            match init {
                Some(ForInit::Declaration(declaration)) => walk_declaration(declaration, visit),
                Some(ForInit::Expression(value)) => walk_expression(value, visit),
                None => {}
            }
            for value in [test, update].into_iter().flatten() {
                walk_expression(value, visit);
            }
            walk_statement(body, visit);
        }
        StatementKind::ForIn { left, right, body }
        | StatementKind::ForOf {
            left, right, body, ..
        } => {
            match left {
                ForTarget::Declaration(declaration) => walk_declaration(declaration, visit),
                ForTarget::Target(target) => walk_pattern(target, visit),
            }
            walk_expression(right, visit);
            walk_statement(body, visit);
        }
        StatementKind::While { test, body } | StatementKind::DoWhile { body, test } => {
            walk_expression(test, visit);
            walk_statement(body, visit);
        }
        StatementKind::Switch {
            discriminant,
            cases,
        } => {
            walk_expression(discriminant, visit);
            for case in cases {
                if let Some(test) = &case.test {
                    walk_expression(test, visit);
                }
                walk_statements(&case.body, visit);
            }
        }
        StatementKind::Return(Some(value)) => walk_expression(value, visit),
        StatementKind::Try {
            block,
            handler,
            finalizer,
        } => {
            walk_statements(block, visit);
            if let Some(handler) = handler {
                if let Some(parameter) = &handler.parameter {
                    walk_pattern(parameter, visit);
                }
                walk_statements(&handler.body, visit);
            }
            if let Some(finalizer) = finalizer {
                walk_statements(finalizer, visit);
            }
        }
        StatementKind::Labelled { body, .. } => walk_statement(body, visit),
        StatementKind::Export(export) => match export {
            Export::Declaration(declaration) => walk_statement(declaration, visit),
            Export::Default(value) => walk_statement(value, visit),
            Export::Named { .. } | Export::All { .. } => {}
        },
        _ => {}
    }
}

fn walk_class(class: &Class, visit: &mut impl FnMut(&Expression)) {
    if let Some(heritage) = &class.heritage {
        walk_expression(heritage, visit);
    }
    for member in &class.members {
        match member {
            ClassMember::Method(method) => {
                walk_key(&method.key, visit);
                walk_function(&method.function, visit);
            }
            ClassMember::Field { key, value, .. } => {
                walk_key(key, visit);
                if let Some(value) = value {
                    walk_expression(value, visit);
                }
            }
            ClassMember::StaticBlock(body) => walk_statements(body, visit),
        }
    }
}

fn walk_declaration(declaration: &Declaration, visit: &mut impl FnMut(&Expression)) {
    for declarator in &declaration.declarators {
        walk_pattern(&declarator.pattern, visit);
        if let Some(init) = &declarator.init {
            walk_expression(init, visit);
        }
    }
}

fn walk_pattern(pattern: &Pattern, visit: &mut impl FnMut(&Expression)) {
    match pattern {
        Pattern::Name(_) => {}
        Pattern::Member(member) => walk_expression(member, visit),
        Pattern::Array { elements, rest } => {
            for element in elements.iter().flatten() {
                walk_pattern(&element.pattern, visit);
                if let Some(default) = &element.default {
                    walk_expression(default, visit);
                }
            }
            if let Some(rest) = rest {
                walk_pattern(rest, visit);
            }
        }
        Pattern::Object { properties, rest } => {
            for property in properties {
                walk_key(&property.key, visit);
                walk_pattern(&property.value.pattern, visit);
                if let Some(default) = &property.value.default {
                    walk_expression(default, visit);
                }
            }
            if let Some(rest) = rest {
                walk_pattern(rest, visit);
            }
        }
    }
}

fn walk_key(key: &Key, visit: &mut impl FnMut(&Expression)) {
    if let Key::Computed(value) = key {
        walk_expression(value, visit);
    }
}

fn walk_function(function: &Function, visit: &mut impl FnMut(&Expression)) {
    for parameter in &function.parameters {
        walk_pattern(&parameter.pattern, visit);
        if let Some(default) = &parameter.default {
            walk_expression(default, visit);
        }
    }
    if let Some(rest) = &function.rest {
        walk_pattern(rest, visit);
    }
    match &function.body {
        Body::Block(body) => walk_statements(body, visit),
        Body::Expression(value) => walk_expression(value, visit),
    }
}

fn walk_arguments(arguments: &[Argument], visit: &mut impl FnMut(&Expression)) {
    for argument in arguments {
        match argument {
            Argument::Item(value) | Argument::Spread(value) => walk_expression(value, visit),
        }
    }
}

/// The three expressions that hold a list of their own parts.
fn walk_a_literal(expression: &Expression, visit: &mut impl FnMut(&Expression)) -> bool {
    match &expression.kind {
        ExpressionKind::Template(template) => {
            for value in &template.expressions {
                walk_expression(value, visit);
            }
        }
        ExpressionKind::TaggedTemplate { tag, template } => {
            walk_expression(tag, visit);
            for value in &template.expressions {
                walk_expression(value, visit);
            }
        }
        ExpressionKind::Array(elements) => {
            for element in elements {
                match element {
                    ArrayElement::Hole => {}
                    ArrayElement::Item(value) | ArrayElement::Spread(value) => {
                        walk_expression(value, visit);
                    }
                }
            }
        }
        ExpressionKind::Object(properties) => {
            for property in properties {
                match property {
                    Property::Named { key, value, .. } => {
                        walk_key(key, visit);
                        walk_expression(value, visit);
                    }
                    Property::Method(method) => {
                        walk_key(&method.key, visit);
                        walk_function(&method.function, visit);
                    }
                    Property::Spread(value) => walk_expression(value, visit),
                }
            }
        }
        _ => return false,
    }
    true
}

fn walk_expression(expression: &Expression, visit: &mut impl FnMut(&Expression)) {
    visit(expression);
    if walk_a_literal(expression, visit) {
        return;
    }
    match &expression.kind {
        ExpressionKind::Function(function) => walk_function(function, visit),
        ExpressionKind::Class(class) => walk_class(class, visit),
        ExpressionKind::Unary { argument, .. } | ExpressionKind::Update { argument, .. } => {
            walk_expression(argument, visit);
        }
        ExpressionKind::Binary { left, right, .. }
        | ExpressionKind::Logical { left, right, .. } => {
            walk_expression(left, visit);
            walk_expression(right, visit);
        }
        ExpressionKind::Assignment { target, value, .. } => {
            walk_pattern(target, visit);
            walk_expression(value, visit);
        }
        ExpressionKind::Conditional {
            test,
            consequent,
            alternate,
        } => {
            walk_expression(test, visit);
            walk_expression(consequent, visit);
            walk_expression(alternate, visit);
        }
        ExpressionKind::Member { object, member, .. } => {
            walk_expression(object, visit);
            if let Member::Computed(index) = member {
                walk_expression(index, visit);
            }
        }
        ExpressionKind::Call {
            callee, arguments, ..
        }
        | ExpressionKind::New { callee, arguments } => {
            walk_expression(callee, visit);
            walk_arguments(arguments, visit);
        }
        ExpressionKind::Chain(inner) | ExpressionKind::Await(inner) => {
            walk_expression(inner, visit);
        }
        ExpressionKind::Sequence(all) => {
            for value in all {
                walk_expression(value, visit);
            }
        }
        ExpressionKind::Yield {
            argument: Some(argument),
            ..
        } => walk_expression(argument, visit),
        ExpressionKind::ImportCall { specifier, options } => {
            walk_expression(specifier, visit);
            if let Some(options) = options {
                walk_expression(options, visit);
            }
        }
        _ => {}
    }
}
