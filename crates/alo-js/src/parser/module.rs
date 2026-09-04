/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `import` and `export`.
//!
//! # Which file is a module is not decided here, and not decided by the text
//!
//! A file holding an `import` is not thereby a module: `<script>` runs a
//! script, and a script with an `import` in it is an error rather than a
//! promotion. What decides is how the page asked for the file, which is
//! [`crate::ast::Source`] and comes in from the caller — so an `import` in a
//! script is [`Reason::ModuleDeclarationInAScript`] and never a quiet change of
//! goal. Guessing would mean the same bytes meaning different things depending
//! on what somebody typed inside them.
//!
//! # A name here is not a name in the program
//!
//! `import { default as a }` and `export { a as "b c" }` are both legal, and
//! neither `default` nor `b c` is a name a program could use. They are strings
//! the *module* is keyed by, which is why [`crate::ast::ModuleName`] is its own
//! type and why a string one is kept as code units — a name a page can export
//! can hold anything, half a surrogate pair included.
//!
//! What this does **not** do is read an import attribute (`with { type:
//! "json" }`). It is written into the queue rather than left out silently: an
//! attribute changes how a module is *fetched*, so it is worth taking with the
//! loader that fetches it rather than a year before there is one.

use crate::ast::{
    Export, ExportSpecifier, Import, ImportSpecifier, ModuleName, Statement, StatementKind,
};
use crate::error::{Reason, SyntaxError};
use crate::punctuator::Punctuator;
use crate::token::Kind;
use crate::word::Keyword;

use super::{OPERAND, OPERATOR, Parser};

impl Parser<'_> {
    /// `import a, { b as c } from "d"`, and `import "d"`.
    pub(super) fn import_declaration(&mut self, start: usize) -> Result<Statement, SyntaxError> {
        if !self.is_module() {
            return Err(SyntaxError::new(Reason::ModuleDeclarationInAScript, start));
        }
        self.expect_keyword(OPERAND, Keyword::Import)?;
        // `import "a"` runs a module for what it does and binds nothing.
        if matches!(self.look(OPERAND)?.kind, Kind::String(_)) {
            let source = self.module_specifier()?;
            self.semicolon()?;
            return Ok(self.statement_at(
                StatementKind::Import(Import {
                    specifiers: Vec::new(),
                    source,
                }),
                start,
            ));
        }
        let mut specifiers = Vec::new();
        if self.at_name(OPERAND)? {
            specifiers.push(ImportSpecifier::Default(self.binding_name(OPERAND)?));
            if self.eat(OPERATOR, Punctuator::Comma)? {
                self.import_the_rest(&mut specifiers)?;
            }
        } else {
            self.import_the_rest(&mut specifiers)?;
        }
        self.expect_keyword(OPERATOR, Keyword::From)?;
        let source = self.module_specifier()?;
        self.semicolon()?;
        Ok(self.statement_at(StatementKind::Import(Import { specifiers, source }), start))
    }

    /// `* as a` or `{ a, b as c }`, after any default binding.
    fn import_the_rest(
        &mut self,
        specifiers: &mut Vec<ImportSpecifier>,
    ) -> Result<(), SyntaxError> {
        if self.eat(OPERAND, Punctuator::Times)? {
            self.expect_keyword(OPERATOR, Keyword::As)?;
            specifiers.push(ImportSpecifier::Namespace(self.binding_name(OPERAND)?));
            return Ok(());
        }
        self.expect(OPERAND, Punctuator::LeftBrace)?;
        while !self.at(OPERAND, Punctuator::RightBrace)? {
            let exported = self.module_name()?;
            let local = if self.eat_keyword(OPERATOR, Keyword::As)? {
                self.binding_name(OPERAND)?
            } else {
                match &exported {
                    ModuleName::Name(name) => name.clone(),
                    // `import { "a b" }` names nothing a program could use, so
                    // the `as` is not optional there.
                    ModuleName::String(_) => {
                        let at = self.start_of_next(OPERATOR)?;
                        return Err(SyntaxError::new(Reason::Expected { wanted: "as" }, at));
                    }
                }
            };
            specifiers.push(ImportSpecifier::Named { exported, local });
            if !self.eat(OPERATOR, Punctuator::Comma)? {
                break;
            }
        }
        self.expect(OPERATOR, Punctuator::RightBrace)?;
        Ok(())
    }

    /// `export …`, in all four of its shapes.
    pub(super) fn export_declaration(&mut self, start: usize) -> Result<Statement, SyntaxError> {
        if !self.is_module() {
            return Err(SyntaxError::new(Reason::ModuleDeclarationInAScript, start));
        }
        self.expect_keyword(OPERAND, Keyword::Export)?;
        if self.eat(OPERAND, Punctuator::Times)? {
            let alias = if self.eat_keyword(OPERATOR, Keyword::As)? {
                Some(self.module_name()?)
            } else {
                None
            };
            self.expect_keyword(OPERATOR, Keyword::From)?;
            let source = self.module_specifier()?;
            self.semicolon()?;
            return Ok(
                self.statement_at(StatementKind::Export(Export::All { alias, source }), start)
            );
        }
        if self.at(OPERAND, Punctuator::LeftBrace)? {
            return self.export_a_list(start);
        }
        if self.eat_keyword(OPERAND, Keyword::Default)? {
            let statement = self.exported_default()?;
            return Ok(self.statement_at(
                StatementKind::Export(Export::Default(Box::new(statement))),
                start,
            ));
        }
        let declaration = self.statement()?;
        if !matches!(
            declaration.kind,
            StatementKind::Declaration(_) | StatementKind::Function(_) | StatementKind::Class(_)
        ) {
            return Err(SyntaxError::new(
                Reason::Expected {
                    wanted: "a declaration",
                },
                declaration.start,
            ));
        }
        Ok(self.statement_at(
            StatementKind::Export(Export::Declaration(Box::new(declaration))),
            start,
        ))
    }

    /// `export { a as b } from "c"`.
    fn export_a_list(&mut self, start: usize) -> Result<Statement, SyntaxError> {
        self.expect(OPERAND, Punctuator::LeftBrace)?;
        let mut specifiers = Vec::new();
        while !self.at(OPERAND, Punctuator::RightBrace)? {
            let local = self.module_name()?;
            let exported = if self.eat_keyword(OPERATOR, Keyword::As)? {
                self.module_name()?
            } else {
                local.clone()
            };
            specifiers.push(ExportSpecifier { local, exported });
            if !self.eat(OPERATOR, Punctuator::Comma)? {
                break;
            }
        }
        self.expect(OPERATOR, Punctuator::RightBrace)?;
        let source = if self.eat_keyword(OPERATOR, Keyword::From)? {
            Some(self.module_specifier()?)
        } else {
            None
        };
        self.semicolon()?;
        Ok(self.statement_at(
            StatementKind::Export(Export::Named { specifiers, source }),
            start,
        ))
    }

    /// What follows `export default`.
    ///
    /// A function or a class here may have no name — that is the whole point of
    /// a default export — and anything else is one expression rather than a
    /// statement, so `export default a, b` is not a program.
    fn exported_default(&mut self) -> Result<Statement, SyntaxError> {
        let start = self.start_of_next(OPERAND)?;
        if self.at_keyword(OPERAND, Keyword::Function)? || self.async_function_follows()? {
            let is_async = self.eat_keyword(OPERAND, Keyword::Async)?;
            let function = self.function_expression(is_async)?;
            return Ok(self.statement_at(StatementKind::Function(Box::new(function)), start));
        }
        if self.at_keyword(OPERAND, Keyword::Class)? {
            let class = self.class(false)?;
            return Ok(self.statement_at(StatementKind::Class(Box::new(class)), start));
        }
        let value = self.value_assignment(true)?;
        self.semicolon()?;
        Ok(self.statement_at(StatementKind::Expression(value), start))
    }

    /// A name in an `import` or `export`, which may be written as a string.
    fn module_name(&mut self) -> Result<ModuleName, SyntaxError> {
        if let Kind::String(units) = &self.look(OPERAND)?.kind {
            let units = units.clone();
            self.bump(OPERAND)?;
            return Ok(ModuleName::String(units));
        }
        Ok(ModuleName::Name(self.any_name(OPERAND)?))
    }
}
