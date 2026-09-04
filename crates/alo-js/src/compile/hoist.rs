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
//! Nothing here walks into a function, because there are none yet (queue item
//! 209). When there are, this is the file that gains the rule — a `var` stops
//! at a function boundary and a `let` never crossed one.

use crate::ast::{
    Declaration, DeclarationKind, ForInit, ForTarget, Pattern, Statement, StatementKind,
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

#[cfg(test)]
mod tests {
    use super::{lexical, vars};
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
    fn a_var_in_a_for_header_is_still_a_var() {
        let body = program("for (var i = 0; i < 1; i++) {}");
        let mut names = Vec::new();
        vars(&body, &mut names);
        assert_eq!(names, vec!["i".to_owned()]);
    }
}
