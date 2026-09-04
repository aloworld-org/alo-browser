/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The grammar, settled in a table.
//!
//! A frozen page's own script proves the parser meets real code; this proves it
//! meets the grammar, and the two are not the same thing. Every case here is
//! either a decision the specification makes that a parser can get wrong
//! quietly — `a ?? b || c`, `-a ** b`, `(a, b)` against `(a, b) => c` — or a
//! refusal ADR 0013 asks for by name.
//!
//! # Why the answers are written as source text
//!
//! Asserting on a tree means writing the tree out, and a tree written out in
//! Rust is longer than the program it stands for and is read by nobody. So
//! [`sketch`] prints the tree **back** as something close to source, with every
//! grouping made explicit — `(a + (b * c))` rather than `a + b * c`. A wrong
//! precedence, a wrong associativity and a missing node all change the
//! parentheses, which is exactly what a reader can check by eye.
//!
//! It is a printer rather than a `Debug`, because `Debug` output changes when a
//! field is added and every case in the table would then have to be rewritten
//! by somebody who is not reading them.

use alo_js::ast::{
    Argument, ArrayElement, Body, Class, ClassMember, Declaration, Element, Export,
    ExportSpecifier, Expression, ExpressionKind, ForInit, ForTarget, Function, Import,
    ImportSpecifier, Key, Member, MethodKind, ModuleName, Pattern, PatternProperty, Program,
    Property, Statement, StatementKind, Template,
};
use alo_js::{Reason, module, script};

// --- The table --------------------------------------------------------------

/// One case: source in, sketch out.
fn reads(source: &str, expected: &str) {
    let read = script(source).map(|program| sketch(&program));
    assert_eq!(
        read.as_deref().map_err(ToString::to_string),
        Ok(expected),
        "for {source:?}"
    );
}

/// One case, read as a module rather than as a script.
fn reads_as_a_module(source: &str, expected: &str) {
    let read = module(source).map(|program| sketch(&program));
    assert_eq!(
        read.as_deref().map_err(ToString::to_string),
        Ok(expected),
        "for {source:?}"
    );
}

/// One case that is not a program, and the refusal it earns.
fn refuses(source: &str, reason: &Reason) {
    let read = script(source).map(|program| sketch(&program));
    assert_eq!(
        read.as_ref().err().map(|error| &error.reason),
        Some(reason),
        "for {source:?}, which was read as {:?}",
        read.as_ref().ok()
    );
}

#[test]
fn precedence_is_a_number_and_the_table_says_which() {
    reads("a + b * c;", "(a + (b * c));");
    reads("a * b + c;", "((a * b) + c);");
    reads("a - b - c;", "((a - b) - c);");
    // `**` is the one right-associative operator in the language.
    reads("a ** b ** c;", "(a ** (b ** c));");
    reads("a | b ^ c & d;", "(a | (b ^ (c & d)));");
    reads("a == b < c;", "(a == (b < c));");
    reads("a in b;", "(a in b);");
    reads("a instanceof b;", "(a instanceof b);");
}

#[test]
fn a_unary_operator_on_the_left_of_a_power_is_refused() {
    // `(-a) ** b` and `-(a ** b)` are different numbers, so neither reading is
    // given.
    refuses("-a ** b;", &Reason::PowerAfterAUnary);
    refuses("typeof a ** b;", &Reason::PowerAfterAUnary);
    // Written down, both readings are ordinary.
    reads("(-a) ** b;", "(-(a) ** b);");
    reads("-(a ** b);", "-((a ** b));");
    // An update expression is not a unary one, and is allowed.
    reads("a++ ** b;", "((a)++ ** b);");
}

#[test]
fn coalescing_may_not_be_mixed_with_and_or_or() {
    reads("a ?? b;", "(a ?? b);");
    reads("(a || b) ?? c;", "((a || b) ?? c);");
    reads("a ?? (b || c);", "(a ?? (b || c));");
    refuses("a ?? b || c;", &Reason::CoalesceMixedWithAndOr);
    refuses("a || b ?? c;", &Reason::CoalesceMixedWithAndOr);
    refuses("a && b ?? c;", &Reason::CoalesceMixedWithAndOr);
}

#[test]
fn an_arrow_function_is_decided_after_the_parenthesis_it_began_with() {
    reads("(a, b);", "a, b;");
    reads("(a, b) => c;", "((a, b) => c);");
    reads("() => a;", "(() => a);");
    reads("a => b => c;", "((a) => ((b) => c));");
    reads("(a = 1) => a;", "((a = 1) => a);");
    reads("({ a }) => a;", "(({a}) => a);");
    reads("(a, ...b) => a;", "((a, ...b) => a);");
    reads("(a) => { return a; };", "((a) => { return a; });");
    // The one that is quadratic without the memory in `parser.rs`.
    reads("((((a))));", "a;");
    // `=>` may not begin a line.
    refuses("(a)\n=> a;", &Reason::ArrowOnANewLine);
}

#[test]
fn async_is_a_name_until_what_follows_says_otherwise() {
    reads("async;", "async;");
    reads("async(a);", "async(a);");
    reads("async a => a;", "(async (a) => a);");
    reads("async (a) => a;", "(async (a) => a);");
    reads("async => async;", "((async) => async);");
    reads("async function a() {}", "async function a() {}");
    // A line ending after `async` makes it a name and `function` a statement.
    reads("async\nfunction a() {}", "async; function a() {}");
}

#[test]
fn a_slash_is_division_or_a_pattern_and_the_parser_is_what_decides() {
    reads("a / b / g;", "((a / b) / g);");
    reads("a = /b/g;", "(a = /b/g);");
    reads("f(/a/);", "f(/a/);");
    reads("[/a/];", "[/a/];");
    reads("typeof /a/;", "typeof (/a/);");
    // The case every syntax highlighter gets wrong, both ways round.
    reads(
        "function f() { return /a/g; }",
        "function f() { return /a/g; }",
    );
    reads("a\n/b/g;", "((a / b) / g);");
}

#[test]
fn a_template_asks_for_its_own_closing_brace() {
    reads("`a`;", "`a`;");
    reads("`a${b}c`;", "`a${b}c`;");
    reads("`${ a }`;", "`${a}`;");
    // The proof that the `}` was asked for as a template continuation: if it
    // had been read as an operator, `/y/` would be two divisions and the tail
    // would not be text at all.
    reads("`${x}/y/`;", "`${x}/y/`;");
    reads("`${ { a: 1 } }`;", "`${{a: 1}}`;");
    reads("tag`a${b}`;", "tag`a${b}`;");
    // A block's `}` is not a template continuation, which is the same rule
    // read from the other side.
    reads("{ a; } `b`;", "{ a; } `b`;");
}

#[test]
fn an_escape_never_spells_a_keyword() {
    refuses(
        "\\u0069f (a) {}",
        &Reason::KeywordWrittenWithAnEscape("if".to_owned()),
    );
    refuses(
        "var \\u0076ar = 1;",
        &Reason::KeywordWrittenWithAnEscape("var".to_owned()),
    );
    // A property is an `IdentifierName`, where an escape is ordinary and a
    // keyword is an ordinary name.
    reads("a.\\u0069f;", "a.if;");
    reads("a.if;", "a.if;");
    reads("({ if: 1 });", "{if: 1};");
}

#[test]
fn with_is_refused_by_name() {
    // ADR 0013 § 3. A refusal that names it beats `with` being read as an
    // undeclared name and failing somewhere else.
    refuses("with (a) { b; }", &Reason::WithIsRefused);
}

#[test]
fn a_semicolon_is_inserted_where_the_three_rules_say() {
    reads("a\nb;", "a; b;");
    reads("function f() { return\n1; }", "function f() { return; 1; }");
    reads("function f() { return 1; }", "function f() { return 1; }");
    reads("{ a }", "{ a; }");
    reads("a", "a;");
    // A line ending before `++` ends the statement: two statements, not one.
    reads("a\n++b;", "a; (++b);");
    reads("a++\nb;", "(a)++; b;");
    // `do … while` may be followed straight away by another statement.
    reads("do a; while (b) c;", "do a; while (b); c;");
    // And where none of the three rules applies, the semicolon is not
    // invented.
    refuses("a b;", &Reason::Expected { wanted: ";" });
}

#[test]
fn what_is_written_to_is_read_as_a_pattern_afterwards() {
    reads("[a, b] = c;", "([a, b] = c);");
    reads("[a, , b] = c;", "([a, , b] = c);");
    reads("[a, ...b] = c;", "([a, ...b] = c);");
    reads("({ a, b: c } = d);", "({a, b: c} = d);");
    reads("({ a = 1 } = b);", "({a = 1} = b);");
    reads("[{ a = 1 }] = b;", "([{a = 1}] = b);");
    reads("[a.b] = c;", "([a.b] = c);");
    reads("[a[b]] = c;", "([a[b]] = c);");
    // A shorthand with a default is only ever a pattern, so where it cannot
    // become one the refusal that was kept is raised.
    refuses("f({ a = 1 });", &Reason::NotAnAssignmentTarget);
    refuses("({ a = 1 });", &Reason::NotAnAssignmentTarget);
    // And things that simply cannot be written to.
    refuses("1 = a;", &Reason::NotAnAssignmentTarget);
    refuses("f() = a;", &Reason::NotAnAssignmentTarget);
    refuses("[a] += b;", &Reason::PatternNeedsAPlainAssignment);
}

#[test]
fn a_declaration_binds_names_and_needs_a_value_where_it_binds_nothing() {
    reads("var a;", "var a;");
    reads("let a, b = 1;", "let a, b = 1;");
    reads("const a = 1;", "const a = 1;");
    reads("let { a, b: [c] } = d;", "let {a, b: [c]} = d;");
    refuses("const a;", &Reason::ConstWithoutAValue);
    refuses("let [a];", &Reason::ConstWithoutAValue);
    refuses("let a.b = 1;", &Reason::Expected { wanted: ";" });
}

#[test]
fn let_is_a_name_wherever_it_does_not_declare() {
    reads("let = 1;", "(let = 1);");
    reads("let;", "let;");
    reads("let[a] = b;", "let [a] = b;");
    reads("let a = 1;", "let a = 1;");
    // In strict code it is reserved, and the directive is what makes it so.
    refuses(
        "\"use strict\"; let = 1;",
        &Reason::ReservedWordAsAName("let".to_owned()),
    );
    refuses(
        "\"use strict\"; var implements = 1;",
        &Reason::ReservedWordAsAName("implements".to_owned()),
    );
    // A directive is what was *written*: an escape in it is a string and not a
    // directive.
    reads(
        "\"use\\u0020strict\"; let = 1;",
        "\"use strict\"; (let = 1);",
    );
}

#[test]
fn a_for_header_is_four_statements_wearing_one_keyword() {
    reads("for (;;) a;", "for (;;) a;");
    reads(
        "for (let a = 0; a < 1; a++) b;",
        "for (let a = 0;(a < 1);(a)++) b;",
    );
    reads("for (a in b) c;", "for (a in b) c;");
    reads("for (const a of b) c;", "for (const a of b) c;");
    reads("for ([a, b] of c) d;", "for ([a, b] of c) d;");
    reads("for (var a = 1 in b) c;", "for (var a = 1 in b) c;");
    // `in` is not an operator in the first clause, which is what stops
    // `for (a in b; ;)` from being a program.
    refuses("for (a in b; ;) c;", &Reason::Expected { wanted: ")" });
    reads("for (const a in b) c;", "for (const a in b) c;");
}

#[test]
fn a_declaration_may_not_be_a_body_without_braces() {
    refuses(
        "if (a) let b = 1;",
        &Reason::DeclarationWhereAStatementIsWanted,
    );
    refuses(
        "while (a) function b() {}",
        &Reason::DeclarationWhereAStatementIsWanted,
    );
    reads("if (a) { let b = 1; }", "if (a) { let b = 1; }");
}

#[test]
fn an_optional_chain_is_the_whole_chain() {
    reads("a?.b;", "chain(a?.b);");
    reads("a?.b.c;", "chain(a?.b.c);");
    reads("a?.(b);", "chain(a?.(b));");
    reads("a?.[b];", "chain(a?.[b]);");
    reads("a?.b ?? c;", "(chain(a?.b) ?? c);");
    refuses("new a?.b();", &Reason::OptionalChainInNew);
    refuses("a?.b`c`;", &Reason::TaggedTemplateInAnOptionalChain);
}

#[test]
fn new_takes_a_member_and_never_a_call() {
    reads("new a;", "(new a());");
    reads("new a();", "(new a());");
    reads("new a.b(c);", "(new a.b(c));");
    reads("new a().b;", "(new a()).b;");
    reads("new new a();", "(new (new a())());");
    reads(
        "function f() { new.target; }",
        "function f() { new.target; }",
    );
    refuses("new.target;", &Reason::NewTargetOutsideAFunction);
    refuses(
        "function f() { new.\\u0074arget; }",
        &Reason::KeywordWrittenWithAnEscape("target".to_owned()),
    );
}

#[test]
fn a_class_has_one_constructor_and_it_is_a_plain_method() {
    reads(
        "class A { constructor() {} }",
        "class A { constructor() {} }",
    );
    reads(
        "class A extends B { constructor() { super(); } }",
        "class A extends B { constructor() { super(); } }",
    );
    reads("class A { static a = 1; }", "class A { static a = 1; }");
    reads("class A { static { a; } }", "class A { static { a; } }");
    reads(
        "class A { #a = 1; b() { return this.#a; } }",
        "class A { #a = 1; b() { return this.#a; } }",
    );
    reads(
        "class A { get a() {} set a(b) {} }",
        "class A { get a() {} set a(b) {} }",
    );
    reads(
        "class A { *a() {} async b() {} }",
        "class A { *a() {} async b() {} }",
    );
    reads("class A { static() {} }", "class A { static() {} }");
    reads("(class {});", "class {};");
    refuses(
        "class A { constructor() {} constructor() {} }",
        &Reason::ConstructorIsNotThat,
    );
    refuses(
        "class A { get constructor() {} }",
        &Reason::ConstructorIsNotThat,
    );
    refuses(
        "class A { *constructor() {} }",
        &Reason::ConstructorIsNotThat,
    );
    // `super()` belongs to a derived constructor and nowhere else.
    refuses(
        "class A { constructor() { super(); } }",
        &Reason::SuperWhereThereIsNone,
    );
    refuses("a.#b;", &Reason::PrivateNameOutsideAClass);
    // A class body is strict however sloppy the file around it is.
    refuses(
        "class A { m() { let implements = 1; } }",
        &Reason::ReservedWordAsAName("implements".to_owned()),
    );
}

#[test]
fn a_private_name_is_an_expression_only_where_in_makes_it_one() {
    reads(
        "class A { static a(b) { return #c in b; } #c; }",
        "class A { static a(b) { return (#c in b); } #c; }",
    );
}

#[test]
fn yield_and_await_are_names_outside_what_makes_them_keywords() {
    reads("var await = 1;", "var await = 1;");
    reads("var yield = 1;", "var yield = 1;");
    reads(
        "function* a() { yield 1; yield* b; yield; }",
        "function* a() { (yield 1); (yield* b); (yield); }",
    );
    reads(
        "async function a() { await b; }",
        "async function a() { (await b); }",
    );
    // A line ending after `yield` ends it.
    reads(
        "function* a() { yield\n1; }",
        "function* a() { (yield); 1; }",
    );
    // In a module `await` is reserved at the top level, and top-level `await`
    // is what makes it so.
    reads_as_a_module("await a;", "(await a);");
    refuses("await a;", &Reason::Expected { wanted: ";" });
}

#[test]
fn a_module_declaration_belongs_to_a_module() {
    reads_as_a_module("import a from \"b\";", "import a from \"b\";");
    reads_as_a_module("import * as a from \"b\";", "import * as a from \"b\";");
    reads_as_a_module(
        "import a, { b as c, default as d } from \"e\";",
        "import a, {b as c, default as d} from \"e\";",
    );
    reads_as_a_module("import \"a\";", "import \"a\";");
    reads_as_a_module("export const a = 1;", "export const a = 1;");
    reads_as_a_module("export default a;", "export default a;");
    reads_as_a_module(
        "export default function () {}",
        "export default function() {}",
    );
    reads_as_a_module("export { a as b };", "export {a as b};");
    reads_as_a_module("export { a } from \"b\";", "export {a as a} from \"b\";");
    reads_as_a_module("export * as a from \"b\";", "export * as a from \"b\";");
    reads_as_a_module("import.meta.a;", "import.meta.a;");
    // `import()` is an expression and is a script's too.
    reads("import(\"a\");", "import(\"a\");");
    refuses("import a from \"b\";", &Reason::ModuleDeclarationInAScript);
    refuses("export const a = 1;", &Reason::ModuleDeclarationInAScript);
    refuses("import.meta;", &Reason::ImportMetaInAScript);
}

#[test]
fn a_label_is_a_name_before_a_colon() {
    reads("a: for (;;) break a;", "a: for (;;) break a;");
    reads("a: b;", "a: b;");
    reads("for (;;) continue;", "for (;;) continue;");
    refuses("break;", &Reason::NothingToLeave);
    refuses("continue;", &Reason::NothingToLeave);
    refuses("return;", &Reason::ReturnOutsideAFunction);
}

#[test]
fn every_node_says_where_it_was() {
    let program = script("  a + b;").expect("a program");
    let Some(Statement { start, end, kind }) = program.body.first().cloned() else {
        panic!("one statement");
    };
    assert_eq!((start, end), (2, 8));
    let StatementKind::Expression(expression) = kind else {
        panic!("an expression statement");
    };
    // The expression ends before the semicolon, and the statement ends after
    // it — which is the difference a stack trace and `toString` both need.
    assert_eq!((expression.start, expression.end), (2, 7));
}

// --- The printer ------------------------------------------------------------

/// A parsed program, written back out as something close to source.
fn sketch(program: &Program) -> String {
    statements(&program.body)
}

fn statements(body: &[Statement]) -> String {
    body.iter().map(statement).collect::<Vec<_>>().join(" ")
}

fn block(body: &[Statement]) -> String {
    if body.is_empty() {
        return "{}".to_owned();
    }
    format!("{{ {} }}", statements(body))
}

fn statement(node: &Statement) -> String {
    if let Some(text) = a_loop(node) {
        return text;
    }
    match &node.kind {
        StatementKind::Expression(value) => format!("{};", expression(value)),
        StatementKind::Block(body) => block(body),
        StatementKind::Empty => ";".to_owned(),
        StatementKind::Declaration(declaration) => format!("{};", declaration_text(declaration)),
        StatementKind::Function(function) => function_text(function),
        StatementKind::Class(class) => class_text(class),
        StatementKind::If {
            test,
            consequent,
            alternate,
        } => {
            let head = format!("if ({}) {}", expression(test), statement(consequent));
            match alternate {
                Some(alternate) => format!("{head} else {}", statement(alternate)),
                None => head,
            }
        }
        StatementKind::Switch {
            discriminant,
            cases,
        } => {
            let cases = cases
                .iter()
                .map(|case| {
                    let head = match &case.test {
                        Some(test) => format!("case {}:", expression(test)),
                        None => "default:".to_owned(),
                    };
                    if case.body.is_empty() {
                        head
                    } else {
                        format!("{head} {}", statements(&case.body))
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");
            format!("switch ({}) {{ {cases} }}", expression(discriminant))
        }
        StatementKind::Continue(label) => match label {
            Some(label) => format!("continue {label};"),
            None => "continue;".to_owned(),
        },
        StatementKind::Break(label) => match label {
            Some(label) => format!("break {label};"),
            None => "break;".to_owned(),
        },
        StatementKind::Return(value) => match value {
            Some(value) => format!("return {};", expression(value)),
            None => "return;".to_owned(),
        },
        StatementKind::Throw(value) => format!("throw {};", expression(value)),
        StatementKind::Try {
            block: guarded,
            handler,
            finalizer,
        } => {
            let caught = match handler {
                Some(handler) => match &handler.parameter {
                    Some(parameter) => {
                        format!(" catch ({}) {}", pattern(parameter), block(&handler.body))
                    }
                    None => format!(" catch {}", block(&handler.body)),
                },
                None => String::new(),
            };
            let last = match finalizer {
                Some(finalizer) => format!(" finally {}", block(finalizer)),
                None => String::new(),
            };
            format!("try {}{caught}{last}", block(guarded))
        }
        StatementKind::Labelled { label, body } => format!("{label}: {}", statement(body)),
        StatementKind::Debugger => "debugger;".to_owned(),
        StatementKind::Import(import) => import_text(import),
        StatementKind::Export(export) => export_text(export),
        _ => "?".to_owned(),
    }
}

/// The four statements that loop, which are half of what a statement can be.
fn a_loop(node: &Statement) -> Option<String> {
    Some(match &node.kind {
        StatementKind::For {
            init,
            test,
            update,
            body,
        } => {
            let init = match init {
                Some(ForInit::Declaration(declaration)) => declaration_text(declaration),
                Some(ForInit::Expression(value)) => expression(value),
                None => String::new(),
            };
            let test = test.as_ref().map(expression).unwrap_or_default();
            let update = update.as_ref().map(expression).unwrap_or_default();
            format!("for ({init};{test};{update}) {}", statement(body))
        }
        StatementKind::ForIn { left, right, body } => format!(
            "for ({} in {}) {}",
            for_target(left),
            expression(right),
            statement(body)
        ),
        StatementKind::ForOf {
            left,
            right,
            is_await,
            body,
        } => format!(
            "for {}({} of {}) {}",
            if *is_await { "await " } else { "" },
            for_target(left),
            expression(right),
            statement(body)
        ),
        StatementKind::While { test, body } => {
            format!("while ({}) {}", expression(test), statement(body))
        }
        StatementKind::DoWhile { body, test } => {
            format!("do {} while ({});", statement(body), expression(test))
        }
        _ => return None,
    })
}

fn for_target(target: &ForTarget) -> String {
    match target {
        ForTarget::Declaration(declaration) => declaration_text(declaration),
        ForTarget::Target(target) => pattern(target),
    }
}

fn declaration_text(declaration: &Declaration) -> String {
    let word = match declaration.kind {
        alo_js::ast::DeclarationKind::Var => "var",
        alo_js::ast::DeclarationKind::Let => "let",
        alo_js::ast::DeclarationKind::Const => "const",
    };
    let parts = declaration
        .declarators
        .iter()
        .map(|declarator| match &declarator.init {
            Some(init) => format!("{} = {}", pattern(&declarator.pattern), expression(init)),
            None => pattern(&declarator.pattern),
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{word} {parts}")
}

fn import_text(import: &Import) -> String {
    if import.specifiers.is_empty() {
        return format!("import {};", text_of(&import.source));
    }
    let mut plain = Vec::new();
    let mut named = Vec::new();
    for specifier in &import.specifiers {
        match specifier {
            ImportSpecifier::Default(name) => plain.push(name.clone()),
            ImportSpecifier::Namespace(name) => plain.push(format!("* as {name}")),
            ImportSpecifier::Named { exported, local } => {
                named.push(format!("{} as {local}", module_name(exported)));
            }
        }
    }
    if !named.is_empty() {
        plain.push(format!("{{{}}}", named.join(", ")));
    }
    format!(
        "import {} from {};",
        plain.join(", "),
        text_of(&import.source)
    )
}

fn export_text(export: &Export) -> String {
    match export {
        Export::Declaration(declaration) => format!("export {}", statement(declaration)),
        Export::Default(value) => format!("export default {}", statement(value)),
        Export::Named { specifiers, source } => {
            let parts = specifiers
                .iter()
                .map(|ExportSpecifier { local, exported }| {
                    format!("{} as {}", module_name(local), module_name(exported))
                })
                .collect::<Vec<_>>()
                .join(", ");
            match source {
                Some(source) => format!("export {{{parts}}} from {};", text_of(source)),
                None => format!("export {{{parts}}};"),
            }
        }
        Export::All { alias, source } => match alias {
            Some(alias) => format!(
                "export * as {} from {};",
                module_name(alias),
                text_of(source)
            ),
            None => format!("export * from {};", text_of(source)),
        },
    }
}

fn module_name(name: &ModuleName) -> String {
    match name {
        ModuleName::Name(name) => name.clone(),
        ModuleName::String(units) => text_of(units),
    }
}

fn pattern(node: &Pattern) -> String {
    match node {
        Pattern::Name(name) => name.clone(),
        Pattern::Member(member) => expression(member),
        Pattern::Array { elements, rest } => {
            let mut parts = elements
                .iter()
                .map(|element| match element {
                    Some(element) => element_text(element),
                    None => String::new(),
                })
                .collect::<Vec<_>>();
            if let Some(rest) = rest {
                parts.push(format!("...{}", pattern(rest)));
            }
            format!("[{}]", parts.join(", "))
        }
        Pattern::Object { properties, rest } => {
            let mut parts = properties
                .iter()
                .map(
                    |PatternProperty {
                         key,
                         value,
                         shorthand,
                     }| {
                        if *shorthand {
                            element_text(value)
                        } else {
                            format!("{}: {}", key_text(key), element_text(value))
                        }
                    },
                )
                .collect::<Vec<_>>();
            if let Some(rest) = rest {
                parts.push(format!("...{}", pattern(rest)));
            }
            format!("{{{}}}", parts.join(", "))
        }
    }
}

fn element_text(element: &Element) -> String {
    match &element.default {
        Some(default) => format!("{} = {}", pattern(&element.pattern), expression(default)),
        None => pattern(&element.pattern),
    }
}

fn key_text(key: &Key) -> String {
    match key {
        Key::Name(name) => name.clone(),
        Key::String(units) => text_of(units),
        Key::Number(value) => number(*value),
        Key::Computed(value) => format!("[{}]", expression(value)),
        Key::Private(name) => format!("#{name}"),
    }
}

fn function_text(function: &Function) -> String {
    let head = format!(
        "{}function{} {}",
        if function.kind.is_async() {
            "async "
        } else {
            ""
        },
        if function.kind.is_generator() {
            "*"
        } else {
            ""
        },
        function.name.clone().unwrap_or_default()
    );
    format!(
        "{}({}) {}",
        head.trim_end(),
        parameters(function),
        body(function)
    )
}

fn arrow_text(function: &Function) -> String {
    format!(
        "({}({}) => {})",
        if function.kind.is_async() {
            "async "
        } else {
            ""
        },
        parameters(function),
        body(function)
    )
}

fn parameters(function: &Function) -> String {
    let mut parts = function
        .parameters
        .iter()
        .map(element_text)
        .collect::<Vec<_>>();
    if let Some(rest) = &function.rest {
        parts.push(format!("...{}", pattern(rest)));
    }
    parts.join(", ")
}

fn body(function: &Function) -> String {
    match &function.body {
        Body::Block(statements) => block(statements),
        Body::Expression(value) => expression(value),
    }
}

fn class_text(class: &Class) -> String {
    let name = match &class.name {
        Some(name) => format!(" {name}"),
        None => String::new(),
    };
    let heritage = match &class.heritage {
        Some(heritage) => format!(" extends {}", expression(heritage)),
        None => String::new(),
    };
    let members = class
        .members
        .iter()
        .map(|member| match member {
            ClassMember::Method(method) => {
                let word = match method.kind {
                    MethodKind::Get => "get ",
                    MethodKind::Set => "set ",
                    MethodKind::Method | MethodKind::Constructor => "",
                };
                format!(
                    "{}{}{}{}{}({}) {}",
                    if method.is_static { "static " } else { "" },
                    if method.function.kind.is_async() {
                        "async "
                    } else {
                        ""
                    },
                    if method.function.kind.is_generator() {
                        "*"
                    } else {
                        ""
                    },
                    word,
                    key_text(&method.key),
                    parameters(&method.function),
                    body(&method.function)
                )
            }
            ClassMember::Field {
                key,
                value,
                is_static,
            } => {
                let start = format!(
                    "{}{}",
                    if *is_static { "static " } else { "" },
                    key_text(key)
                );
                match value {
                    Some(value) => format!("{start} = {};", expression(value)),
                    None => format!("{start};"),
                }
            }
            ClassMember::StaticBlock(body) => format!("static {}", block(body)),
        })
        .collect::<Vec<_>>()
        .join(" ");
    if members.is_empty() {
        return format!("class{name}{heritage} {{}}");
    }
    format!("class{name}{heritage} {{ {members} }}")
}

fn template_text(template: &Template) -> String {
    let mut parts = Vec::new();
    for (index, piece) in template.pieces.iter().enumerate() {
        parts.push(piece.raw.clone());
        if let Some(value) = template.expressions.get(index) {
            parts.push(format!("${{{}}}", expression(value)));
        }
    }
    format!("`{}`", parts.join(""))
}

/// The expressions that are a value written down, rather than something done
/// to one.
fn a_literal(node: &Expression) -> Option<String> {
    Some(match &node.kind {
        ExpressionKind::Name(name) => name.clone(),
        ExpressionKind::PrivateName(name) => format!("#{name}"),
        ExpressionKind::This => "this".to_owned(),
        ExpressionKind::Super => "super".to_owned(),
        ExpressionKind::Null => "null".to_owned(),
        ExpressionKind::Boolean(value) => value.to_string(),
        ExpressionKind::Number(value) => number(*value),
        ExpressionKind::BigInt { digits, .. } => format!("{digits}n"),
        ExpressionKind::String(units) => text_of(units),
        ExpressionKind::RegularExpression(literal) => {
            format!("/{}/{}", literal.body, literal.flags)
        }
        ExpressionKind::Template(template) => template_text(template),
        ExpressionKind::TaggedTemplate { tag, template } => {
            format!("{}{}", expression(tag), template_text(template))
        }
        ExpressionKind::Array(elements) => {
            let parts = elements
                .iter()
                .map(|element| match element {
                    ArrayElement::Hole => String::new(),
                    ArrayElement::Item(value) => expression(value),
                    ArrayElement::Spread(value) => format!("...{}", expression(value)),
                })
                .collect::<Vec<_>>();
            format!("[{}]", parts.join(", "))
        }
        ExpressionKind::Object(properties) => {
            let parts = properties.iter().map(object_property).collect::<Vec<_>>();
            format!("{{{}}}", parts.join(", "))
        }
        ExpressionKind::Function(function) => {
            if function.is_arrow {
                arrow_text(function)
            } else {
                function_text(function)
            }
        }
        ExpressionKind::Class(class) => class_text(class),
        ExpressionKind::NewTarget => "new.target".to_owned(),
        ExpressionKind::ImportMeta => "import.meta".to_owned(),
        _ => return None,
    })
}

/// One property of an object literal, written back out.
fn object_property(property: &Property) -> String {
    match property {
        Property::Named {
            key,
            value,
            shorthand,
        } => {
            if *shorthand {
                match &value.kind {
                    ExpressionKind::Assignment { target, value, .. } => {
                        format!("{} = {}", pattern(target), expression(value))
                    }
                    _ => key_text(key),
                }
            } else {
                format!("{}: {}", key_text(key), expression(value))
            }
        }
        Property::Method(method) => format!(
            "{}{}{}{}({}) {}",
            if method.function.kind.is_async() {
                "async "
            } else {
                ""
            },
            if method.function.kind.is_generator() {
                "*"
            } else {
                ""
            },
            match method.kind {
                MethodKind::Get => "get ",
                MethodKind::Set => "set ",
                _ => "",
            },
            key_text(&method.key),
            parameters(&method.function),
            body(&method.function)
        ),
        Property::Spread(value) => format!("...{}", expression(value)),
    }
}

/// The expressions that reach into or call something else.
fn a_reach(node: &Expression) -> Option<String> {
    Some(match &node.kind {
        ExpressionKind::Member {
            object,
            member,
            optional,
        } => {
            let dot = if *optional { "?." } else { "." };
            match member {
                Member::Name(name) => format!("{}{dot}{name}", expression(object)),
                Member::Private(name) => format!("{}{dot}#{name}", expression(object)),
                Member::Computed(index) => {
                    let open = if *optional { "?.[" } else { "[" };
                    format!("{}{open}{}]", expression(object), expression(index))
                }
            }
        }
        ExpressionKind::Call {
            callee,
            arguments,
            optional,
        } => format!(
            "{}{}({})",
            expression(callee),
            if *optional { "?." } else { "" },
            arguments_text(arguments)
        ),
        ExpressionKind::New { callee, arguments } => format!(
            "(new {}({}))",
            expression(callee),
            arguments_text(arguments)
        ),
        ExpressionKind::Chain(inner) => format!("chain({})", expression(inner)),
        ExpressionKind::Sequence(all) => all.iter().map(expression).collect::<Vec<_>>().join(", "),
        ExpressionKind::Yield { argument, delegate } => {
            let star = if *delegate { "*" } else { "" };
            match argument {
                Some(argument) => format!("(yield{star} {})", expression(argument)),
                None => "(yield)".to_owned(),
            }
        }
        ExpressionKind::Await(value) => format!("(await {})", expression(value)),
        ExpressionKind::ImportCall { specifier, options } => match options {
            Some(options) => format!("import({}, {})", expression(specifier), expression(options)),
            None => format!("import({})", expression(specifier)),
        },
        _ => return None,
    })
}

fn expression(node: &Expression) -> String {
    if let Some(text) = a_literal(node).or_else(|| a_reach(node)) {
        return text;
    }
    match &node.kind {
        ExpressionKind::Unary { operator, argument } => {
            let word = match operator {
                alo_js::ast::Unary::Delete => "delete ",
                alo_js::ast::Unary::Void => "void ",
                alo_js::ast::Unary::TypeOf => "typeof ",
                alo_js::ast::Unary::Plus => "+",
                alo_js::ast::Unary::Minus => "-",
                alo_js::ast::Unary::BitNot => "~",
                alo_js::ast::Unary::Not => "!",
            };
            format!("{word}({})", expression(argument))
        }
        ExpressionKind::Update {
            increment,
            prefix,
            argument,
        } => {
            let word = if *increment { "++" } else { "--" };
            if *prefix {
                format!("({word}{})", expression(argument))
            } else {
                format!("({}){word}", expression(argument))
            }
        }
        ExpressionKind::Binary {
            operator,
            left,
            right,
        } => format!(
            "({} {} {})",
            expression(left),
            binary_word(*operator),
            expression(right)
        ),
        ExpressionKind::Logical {
            operator,
            left,
            right,
        } => {
            let word = match operator {
                alo_js::ast::Logical::And => "&&",
                alo_js::ast::Logical::Or => "||",
                alo_js::ast::Logical::Coalesce => "??",
            };
            format!("({} {word} {})", expression(left), expression(right))
        }
        ExpressionKind::Assignment {
            operator,
            target,
            value,
        } => format!(
            "({} {} {})",
            pattern(target),
            assign_word(*operator),
            expression(value)
        ),
        ExpressionKind::Conditional {
            test,
            consequent,
            alternate,
        } => format!(
            "({} ? {} : {})",
            expression(test),
            expression(consequent),
            expression(alternate)
        ),
        _ => "?".to_owned(),
    }
}

fn arguments_text(arguments: &[Argument]) -> String {
    arguments
        .iter()
        .map(|argument| match argument {
            Argument::Item(value) => expression(value),
            Argument::Spread(value) => format!("...{}", expression(value)),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn binary_word(operator: alo_js::ast::Binary) -> &'static str {
    use alo_js::ast::Binary;
    match operator {
        Binary::Add => "+",
        Binary::Subtract => "-",
        Binary::Multiply => "*",
        Binary::Divide => "/",
        Binary::Remainder => "%",
        Binary::Power => "**",
        Binary::ShiftLeft => "<<",
        Binary::ShiftRight => ">>",
        Binary::ShiftRightUnsigned => ">>>",
        Binary::Less => "<",
        Binary::Greater => ">",
        Binary::LessOrEqual => "<=",
        Binary::GreaterOrEqual => ">=",
        Binary::InstanceOf => "instanceof",
        Binary::In => "in",
        Binary::Equal => "==",
        Binary::NotEqual => "!=",
        Binary::StrictlyEqual => "===",
        Binary::StrictlyNotEqual => "!==",
        Binary::BitAnd => "&",
        Binary::BitOr => "|",
        Binary::BitXor => "^",
        _ => "?",
    }
}

fn assign_word(operator: alo_js::ast::Assign) -> &'static str {
    use alo_js::ast::Assign;
    match operator {
        Assign::Assign => "=",
        Assign::Add => "+=",
        Assign::Subtract => "-=",
        Assign::Multiply => "*=",
        Assign::Divide => "/=",
        Assign::Remainder => "%=",
        Assign::Power => "**=",
        Assign::ShiftLeft => "<<=",
        Assign::ShiftRight => ">>=",
        Assign::ShiftRightUnsigned => ">>>=",
        Assign::BitAnd => "&=",
        Assign::BitOr => "|=",
        Assign::BitXor => "^=",
        Assign::And => "&&=",
        Assign::Or => "||=",
        Assign::Coalesce => "??=",
        _ => "?",
    }
}

/// A string, as text a person can read in a failure.
fn text_of(units: &[u16]) -> String {
    match String::from_utf16(units) {
        Ok(text) => format!("{text:?}"),
        Err(_) => format!("{units:?}"),
    }
}

/// A number, without the `.0` an integer would otherwise carry.
fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e21 {
        return format!("{value:.0}");
    }
    format!("{value}")
}
