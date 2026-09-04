/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a piece of program declares before any of it runs.
//!
//! Two questions, and they have different answers on purpose.
//!
//! **`VarDeclaredNames`** walks *through* blocks, loops and branches, because a
//! `var` inside them is not theirs: `if (false) { var a; }` still declares `a`,
//! and `typeof a` before the `if` is `"undefined"` rather than a
//! `ReferenceError`. That is the rule people mean when they say `var` is
//! hoisted, and it is why this walk is recursive.
//!
//! **`LexicallyScopedDeclarations`** does not walk at all. A `let` in a nested
//! block belongs to that block, so the list for a statement list is its
//! *direct* children only — and it is what the compiler needs at the moment it
//! enters a block, so that a name read before its declaration is a dead zone
//! rather than a name nobody has heard of.
//!
//! **Nothing here walks into a function**, and that is the rule rather than an
//! omission: a `var` stops at a function boundary and a `let` never crossed
//! one. [`StatementKind::Function`] is a leaf to both walks for exactly that
//! reason.
//!
//! # And function declarations, which are neither and are both
//!
//! `function a() {}` declares a name, and *which* kind depends only on where it
//! is written: at the top level of a script or a function body it is
//! var-scoped, and inside a block it is that block's. So [`functions`] is a
//! third list rather than an entry in either of the other two, and the caller —
//! which knows which of the two places it is standing in — decides. Neither
//! [`vars`] nor [`lexical`] reports one, so nothing can declare it twice by
//! reading both.
//!
//! Annex B's *"a function in a block is also a `var` in sloppy code"* is not
//! here and is not owed: law 1 refuses thirty years of compatibility, and this
//! is one of the rules that buys.

use crate::ast::{
    Declaration, DeclarationKind, ForInit, ForTarget, Function, Pattern, Statement, StatementKind,
};

/// One name a statement list declares lexically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    /// The name.
    pub name: String,
    /// Whether it may be assigned to again.
    pub mutable: bool,
    /// Where in the source it was declared, for a message about it.
    pub at: usize,
}

/// Every name these statements declare with `var`, in the order they are met.
///
/// Recursive: a `var` is the enclosing function's — or, here, the script's —
/// however deeply it is written.
pub fn vars(statements: &[Statement], into: &mut Vec<String>) {
    for statement in statements {
        one(statement, into);
    }
}

/// The `var` names one statement declares, including the ones inside it.
fn one(statement: &Statement, into: &mut Vec<String>) {
    match &statement.kind {
        StatementKind::Declaration(declaration) => from_declaration(declaration, into),
        StatementKind::Block(body) => vars(body, into),
        StatementKind::If {
            consequent,
            alternate,
            ..
        } => {
            one(consequent, into);
            if let Some(alternate) = alternate {
                one(alternate, into);
            }
        }
        StatementKind::For { init, body, .. } => {
            if let Some(ForInit::Declaration(declaration)) = init {
                from_declaration(declaration, into);
            }
            one(body, into);
        }
        StatementKind::ForIn { left, body, .. } | StatementKind::ForOf { left, body, .. } => {
            if let ForTarget::Declaration(declaration) = left {
                from_declaration(declaration, into);
            }
            one(body, into);
        }
        StatementKind::While { body, .. }
        | StatementKind::DoWhile { body, .. }
        | StatementKind::Labelled { body, .. } => one(body, into),
        StatementKind::Switch { cases, .. } => {
            for case in cases {
                vars(&case.body, into);
            }
        }
        StatementKind::Try {
            block,
            handler,
            finalizer,
        } => {
            vars(block, into);
            if let Some(handler) = handler {
                vars(&handler.body, into);
            }
            if let Some(finalizer) = finalizer {
                vars(finalizer, into);
            }
        }
        // Everything else declares no `var`: an expression cannot, a function
        // body is its own (queue item 209), and a module's declarations are
        // lexical.
        _ => {}
    }
}

/// The names in a `var` declaration, and nothing from a `let` or a `const`.
fn from_declaration(declaration: &Declaration, into: &mut Vec<String>) {
    if declaration.kind != DeclarationKind::Var {
        return;
    }
    for declarator in &declaration.declarators {
        if let Pattern::Name(name) = &declarator.pattern {
            if !into.contains(name) {
                into.push(name.clone());
            }
        }
        // A destructuring pattern declares names too, and it is queue item 211:
        // the compiler refuses one rather than reaching here, so there is
        // nothing to collect and nothing to half-collect.
    }
}

/// The `let` and `const` names these statements declare **themselves**.
///
/// Not recursive, for the reason in this file's header: a `let` in a nested
/// block is that block's.
pub fn lexical(statements: &[Statement]) -> Vec<Declared> {
    let mut declared = Vec::new();
    for statement in statements {
        let StatementKind::Declaration(declaration) = &statement.kind else {
            continue;
        };
        let mutable = match declaration.kind {
            DeclarationKind::Var => continue,
            DeclarationKind::Let => true,
            DeclarationKind::Const => false,
        };
        for declarator in &declaration.declarators {
            if let Pattern::Name(name) = &declarator.pattern {
                declared.push(Declared {
                    name: name.clone(),
                    mutable,
                    at: statement.start,
                });
            }
        }
    }
    declared
}

/// The functions these statements declare **themselves**, in the order they are
/// written.
///
/// Not recursive, for [`lexical`]'s reason: a function declared in a nested
/// block is that block's. Its name is [`Function::name`], and one without a name
/// is not a declaration at all — the parser only builds a nameless [`Function`]
/// for an expression — so it is left out rather than given a place nothing could
/// read.
pub fn functions(statements: &[Statement]) -> Vec<&Function> {
    let mut declared = Vec::new();
    for statement in statements {
        if let StatementKind::Function(function) = &statement.kind {
            if function.name.is_some() {
                declared.push(function.as_ref());
            }
        }
    }
    declared
}

#[cfg(test)]
mod tests {
    use super::{functions, lexical, vars};
    use crate::ast::Source;
    use crate::parser::script;

    fn program(source: &str) -> Vec<crate::ast::Statement> {
        match script(source) {
            Ok(program) => {
                assert_eq!(program.source, Source::Script);
                program.body
            }
            Err(why) => panic!("{source} does not parse: {why}"),
        }
    }

    #[test]
    fn a_var_is_declared_however_deeply_it_is_written() {
        let body = program("if (false) { while (a) { var deep = 1; } } var flat;");
        let mut names = Vec::new();
        vars(&body, &mut names);
        assert_eq!(names, vec!["deep".to_owned(), "flat".to_owned()]);
    }

    #[test]
    fn a_let_in_a_nested_block_is_not_this_lists() {
        let body = program("let mine = 1; { let theirs = 2; } const also = 3;");
        let declared = lexical(&body);
        let names: Vec<&str> = declared.iter().map(|one| one.name.as_str()).collect();
        assert_eq!(names, vec!["mine", "also"]);
        assert!(declared.first().is_some_and(|one| one.mutable));
        assert!(declared.last().is_some_and(|one| !one.mutable));
    }

    #[test]
    fn a_function_declaration_is_its_own_list_and_stops_the_var_walk() {
        let body = program("function a() { var inside; } { function b() {} } var outside;");
        let mut names = Vec::new();
        vars(&body, &mut names);
        assert_eq!(
            names,
            vec!["outside".to_owned()],
            "a function body's `var` is the function's, and a declaration is neither list's"
        );
        assert!(lexical(&body).is_empty());
        let declared: Vec<&str> = functions(&body)
            .iter()
            .filter_map(|function| function.name.as_deref())
            .collect();
        assert_eq!(declared, vec!["a"], "and one in a block is that block's");
    }

    #[test]
    fn a_var_in_a_for_header_is_still_a_var() {
        let body = program("for (var i = 0; i < 1; i++) {}");
        let mut names = Vec::new();
        vars(&body, &mut names);
        assert_eq!(names, vec!["i".to_owned()]);
    }
}
