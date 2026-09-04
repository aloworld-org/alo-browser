/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Tokens into a tree: the cursor over the lexer, and the rules that need to
//! see the token stream rather than any one token.
//!
//! The grammar itself is in the files beside this one — [`expression`],
//! [`statement`], [`binding`], [`class`] and [`module`] — because a parser that
//! is one file is a file with a reason to change for every production. What is
//! here is what all of them share.
//!
//! # Asking for a token means saying what may be there
//!
//! [`crate::Lexer::next`] takes a [`Goal`] every call and guesses at nothing
//! (queue item 70), so this cursor cannot peek without deciding. It does not
//! have to guess either: **the parser always knows whether an operand or an
//! operator may come next**, which is the whole of what the goal asks. The two
//! are named [`OPERAND`] and [`OPERATOR`] here so that a call site reads as the
//! claim it is making.
//!
//! One token of lookahead is kept, *with the goal it was read under*. Asking
//! again under a different goal re-reads it from the source rather than
//! answering with the token that was already there — which is what makes
//! `` `${x}/y/` `` work: the substitution's expression ends by peeking at `}`
//! as an operator, and the template's tail is then asked for at the same offset
//! under [`Goal::TemplateContinuation`], where a `}` continues a template and
//! `/y/` is text rather than division.
//!
//! # An arrow function against a parenthesised expression
//!
//! The second ambiguity queue item 70 named, and the one this file settles.
//! `(a, b)` is a parenthesised expression and `(a, b) => c` is a parameter
//! list, and the two are the same characters until the `)` has been passed. So
//! the cursor **marks its place, tries the parameter list, and puts everything
//! back** if what follows is not `=>`; the decision is made on the token
//! stream, exactly as the item says, rather than by a lexer heuristic.
//!
//! Trying costs a second read of what is inside the parentheses, and a
//! `(` inside a `(` would pay that cost again at every level — which is
//! quadratic on a page that nests them, and a page chooses how deeply it
//! nests. So a `(` that turned out **not** to open a parameter list is
//! remembered by its offset ([`Parser::not_a_parameter_list`]) and never tried
//! twice. The memory is bounded by the source length, which
//! [`bounds::LONGEST_SOURCE`] already bounds.
//!
//! # Automatic semicolon insertion
//!
//! The one rule in the language that can see a line ending, which is why
//! [`crate::token::Token::newline_before`] exists. [`Parser::semicolon`] is all
//! three of its cases — a line ending before the offending token, a `}`, and
//! the end of the source. The other half of the rule is the handful of places
//! where a line ending *ends* the statement whatever follows: `return`,
//! `throw`, `break`, `continue`, `yield`, a postfix `++` and the `=>` of an
//! arrow. Each of those asks [`Parser::newline_before`] where it stands, and
//! each says in its own file what it does about the answer, because what they
//! do differs — `return\n1` inserts a semicolon and `a\n=> b` is not a program
//! at all.
//!
//! # A depth this parser will not go past
//!
//! [`crate::bounds::LONGEST_SOURCE`] bounds the lexer, which does not recurse.
//! This does, and a script of twenty thousand open brackets is a stack overflow
//! rather than a refusal in any parser that does not count. [`Parser::deeper`]
//! counts, and [`bounds::DEEPEST_NESTING`] is the ceiling with its reason
//! beside it.

pub mod binding;
pub mod class;
pub mod expression;
pub mod function;
pub mod module;
pub mod property;
pub mod statement;

use std::collections::HashSet;

use crate::ast::{Expression, ExpressionKind, Program, Source, Statement, StatementKind};
use crate::error::{Reason, SyntaxError};
use crate::lexer::{Goal, Lexer};
use crate::punctuator::Punctuator;
use crate::token::{Kind, Token};
use crate::word::{Keyword, Status};
use crate::{bounds, read, word};

/// Where an operand may begin, so `/` opens a regular expression.
pub const OPERAND: Goal = Goal::RegularExpression;

/// Where an operator or a closing bracket must come next, so `/` divides.
pub const OPERATOR: Goal = Goal::Division;

/// The one token of lookahead, and the goal it was read under.
#[derive(Debug, Clone)]
struct Peeked<'a> {
    goal: Goal,
    token: Token,
    /// The lexer as it stands *after* the token — kept so that consuming it is
    /// free and re-reading it under another goal is possible.
    after: Lexer<'a>,
}

/// What kind of body the parser is inside.
///
/// One value rather than three flags, because the three are not independent:
/// `yield` is reserved in a generator whether or not it is `async`, `await` is
/// reserved in an `async` function and at a module's top level, and `return`
/// means something in all of them but one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Inside {
    /// A script's top level, where `await` and `yield` are ordinary names.
    AScript,
    /// A module's top level, which may `await` without being a function.
    AModule,
    /// `function a() {}`
    APlainFunction,
    /// `async function a() {}`
    AnAsyncFunction,
    /// `function* a() {}`
    AGenerator,
    /// `async function* a() {}`
    AnAsyncGenerator,
}

impl Inside {
    /// The body an `async` and a `*` together describe.
    fn a_function(is_async: bool, is_generator: bool) -> Self {
        match (is_async, is_generator) {
            (false, false) => Self::APlainFunction,
            (true, false) => Self::AnAsyncFunction,
            (false, true) => Self::AGenerator,
            (true, true) => Self::AnAsyncGenerator,
        }
    }

    /// Whether there is something here to `return` from.
    fn is_a_function(self) -> bool {
        !matches!(self, Self::AScript | Self::AModule)
    }

    /// Whether `await` is an operator here rather than a name.
    fn awaits(self) -> bool {
        matches!(
            self,
            Self::AModule | Self::AnAsyncFunction | Self::AnAsyncGenerator
        )
    }

    /// Whether `yield` is an operator here rather than a name.
    fn yields(self) -> bool {
        matches!(self, Self::AGenerator | Self::AnAsyncGenerator)
    }
}

/// What `super` has to look in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Home {
    /// Nothing: `super` is not a thing here.
    Nowhere,
    /// A method, so `super.a` reads from what it was written in.
    AMethod,
    /// The constructor of a class that extends something, which is the only
    /// place `super()` means anything.
    ADerivedConstructor,
}

/// What `break` and `continue` have to leave.
///
/// Two facts rather than one, and that is the whole reason they are written
/// out: a `switch` inside a loop is something `break` may leave and something
/// `continue` may not, and a single "innermost thing" would lose the loop the
/// moment the `switch` was entered.
#[derive(Debug, Clone, Copy, Default)]
struct Leaving {
    /// A loop, which `break` and `continue` may both leave.
    a_loop: bool,
    /// A `switch`, which only `break` may leave.
    a_switch: bool,
}

/// What is true of the code being parsed at this point.
///
/// Copied and put back around every body, because these are properties of
/// where you are and not of the parse: `await` is a name in a plain function
/// nested inside an `async` one, and a `return` is only refused where there is
/// nothing to return from.
#[derive(Debug, Clone, Copy)]
struct Context {
    /// Strict code: a module, a class body, or a body with a `"use strict"`.
    strict: bool,
    /// Inside a class body, so `#a` means something.
    in_class: bool,
    /// Which kind of body.
    inside: Inside,
    /// What `super` looks in.
    home: Home,
    /// What `break` and `continue` leave.
    leaving: Leaving,
}

impl Context {
    /// The context at the top of a script, before anything has been entered.
    fn top(source: Source) -> Self {
        Self {
            strict: source == Source::Module,
            in_class: false,
            inside: match source {
                Source::Script => Inside::AScript,
                Source::Module => Inside::AModule,
            },
            home: Home::Nowhere,
            leaving: Leaving::default(),
        }
    }

    /// Whether `break` has something to leave.
    fn can_break(self) -> bool {
        self.leaving.a_loop || self.leaving.a_switch
    }
}

/// Where the cursor is, so that a speculative parse can be undone.
#[derive(Debug, Clone)]
struct Mark<'a> {
    lexer: Lexer<'a>,
    peeked: Option<Peeked<'a>>,
    last_end: usize,
    depth: usize,
    context: Context,
    kept_refusal: Option<SyntaxError>,
}

/// A parser over one piece of source text.
#[derive(Debug)]
pub struct Parser<'a> {
    source: &'a str,
    which: Source,
    lexer: Lexer<'a>,
    peeked: Option<Peeked<'a>>,
    /// The end of the last token consumed, which is where a node ends.
    last_end: usize,
    depth: usize,
    context: Context,
    /// The offsets of every `(` that was tried as a parameter list and was not
    /// one — see this file's header.
    not_a_parameter_list: HashSet<usize>,
    /// The first thing read that is only legal if what is being read turns out
    /// to be a pattern.
    ///
    /// `{ a = 1 }` is a destructuring pattern with a default and is not an
    /// object literal, and which of the two it is, is decided by an `=` that
    /// comes after it: `[{ a = 1 }] = b` is ordinary and `f({ a = 1 })` is not
    /// a program. So it is read, the refusal is **kept rather than raised**,
    /// and it is dropped the moment the thing holding it is turned into a
    /// pattern. [`Parser::value_expression`] is where an expression that can no
    /// longer become one is finished, and where a kept refusal is finally
    /// raised.
    kept_refusal: Option<SyntaxError>,
}

impl<'a> Parser<'a> {
    /// A parser over `source`, read as a script or as a module.
    ///
    /// # Errors
    ///
    /// [`Reason::SourceTooLong`], which is the lexer's one refusal at this
    /// point; nothing has been read yet.
    pub fn new(source: &'a str, which: Source) -> Result<Self, SyntaxError> {
        let lexer = Lexer::new(source)?;
        let last_end = lexer.offset();
        Ok(Self {
            source,
            which,
            lexer,
            peeked: None,
            last_end,
            depth: 0,
            context: Context::top(which),
            not_a_parameter_list: HashSet::new(),
            kept_refusal: None,
        })
    }

    /// Parse the whole of it, on a stack of our own.
    ///
    /// The thread is not an optimisation and not concurrency: it is what makes
    /// [`bounds::DEEPEST_NESTING`] a bound. A recursive descent parser's real
    /// ceiling is how much stack it was called on, which is the caller's
    /// property and differs between a test thread, a process's first thread and
    /// whatever a renderer was given — so the parse runs on
    /// [`bounds::STACK_FOR_A_PARSE`] instead, and the depth that is refused is
    /// the same everywhere. `alo-net` makes the same argument about every
    /// ceiling it has: *a limit somebody else chooses is not a limit*.
    ///
    /// It is a **scoped** thread, so the source text is still borrowed and
    /// nothing is copied to get it there, and a panic inside is raised again
    /// here rather than turned into a refusal — a panic is a bug in this crate,
    /// and a bug reported as a syntax error is a bug nobody finds.
    ///
    /// # Errors
    ///
    /// The first thing it cannot read, as a [`SyntaxError`] naming what was
    /// wrong and where. Nothing here panics on any source text, which is
    /// ADR 0013 § 4 and is asserted rather than asserted about.
    pub fn program(self) -> Result<Program, SyntaxError> {
        std::thread::scope(|scope| {
            let started = std::thread::Builder::new()
                .name("alo-js parse".to_owned())
                .stack_size(bounds::STACK_FOR_A_PARSE)
                .spawn_scoped(scope, || self.read_it_all());
            match started {
                Ok(parse) => match parse.join() {
                    Ok(program) => program,
                    Err(panic) => std::panic::resume_unwind(panic),
                },
                Err(_) => Err(SyntaxError::new(Reason::NoStackOfItsOwn, 0)),
            }
        })
    }

    /// The parse itself, once it is on the stack it asked for.
    fn read_it_all(mut self) -> Result<Program, SyntaxError> {
        let (body, strict) = self.directives_then_statements(None)?;
        let next = self.look(OPERAND)?;
        if next.kind != Kind::End {
            let at = next.start;
            return Err(SyntaxError::new(
                Reason::Expected {
                    wanted: "the end of the script",
                },
                at,
            ));
        }
        Ok(Program {
            source: self.which,
            strict,
            body,
        })
    }

    // --- The token stream ---------------------------------------------------

    /// The next token, read the way `goal` says, without consuming it.
    fn look(&mut self, goal: Goal) -> Result<&Token, SyntaxError> {
        if self.peeked.as_ref().is_none_or(|p| p.goal != goal) {
            let mut after = self.lexer.clone();
            let token = after.next(goal)?;
            self.peeked = Some(Peeked { goal, token, after });
        }
        // The branch above has just filled it, and a `Peeked` is never taken
        // without also moving the lexer past it.
        self.peeked
            .as_ref()
            .map(|p| &p.token)
            .ok_or_else(|| SyntaxError::new(Reason::Expected { wanted: "a token" }, self.last_end))
    }

    /// The next token, consumed.
    fn bump(&mut self, goal: Goal) -> Result<Token, SyntaxError> {
        self.look(goal)?;
        match self.peeked.take() {
            Some(peeked) => {
                self.lexer = peeked.after;
                self.last_end = peeked.token.end;
                Ok(peeked.token)
            }
            None => Err(SyntaxError::new(
                Reason::Expected { wanted: "a token" },
                self.last_end,
            )),
        }
    }

    /// Where the next token begins.
    fn start_of_next(&mut self, goal: Goal) -> Result<usize, SyntaxError> {
        Ok(self.look(goal)?.start)
    }

    /// Whether a line ended before the next token.
    fn newline_before(&mut self, goal: Goal) -> Result<bool, SyntaxError> {
        Ok(self.look(goal)?.newline_before)
    }

    /// Whether the source has ended.
    fn at_end(&mut self, goal: Goal) -> Result<bool, SyntaxError> {
        Ok(self.look(goal)?.kind == Kind::End)
    }

    /// Whether the next token is this punctuator.
    fn at(&mut self, goal: Goal, punctuator: Punctuator) -> Result<bool, SyntaxError> {
        Ok(self.look(goal)?.kind == Kind::Punctuator(punctuator))
    }

    /// Consume the next token if it is this punctuator.
    fn eat(&mut self, goal: Goal, punctuator: Punctuator) -> Result<bool, SyntaxError> {
        if self.at(goal, punctuator)? {
            self.bump(goal)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Consume this punctuator, or refuse.
    fn expect(&mut self, goal: Goal, punctuator: Punctuator) -> Result<Token, SyntaxError> {
        if self.at(goal, punctuator)? {
            return self.bump(goal);
        }
        let at = self.start_of_next(goal)?;
        Err(SyntaxError::new(
            Reason::Expected {
                wanted: punctuator.as_str(),
            },
            at,
        ))
    }

    /// Whether the next token is this keyword, written plainly.
    ///
    /// A word with an escape in it is never a keyword — see [`word::Word`] —
    /// so `if` answers `false` here and is refused as a name by
    /// [`Parser::name_of`] when it is used as one.
    fn at_keyword(&mut self, goal: Goal, keyword: Keyword) -> Result<bool, SyntaxError> {
        Ok(match &self.look(goal)?.kind {
            Kind::Name(word) => !word.escaped && word.name == keyword.as_str(),
            _ => false,
        })
    }

    /// Consume the next token if it is this keyword.
    fn eat_keyword(&mut self, goal: Goal, keyword: Keyword) -> Result<bool, SyntaxError> {
        if self.at_keyword(goal, keyword)? {
            self.bump(goal)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Consume this keyword, or refuse.
    ///
    /// A word that spells it but was written with an escape is refused **by
    /// that name**: `new.target` is not `new.target`, and being told it
    /// was spelled with an escape is the difference between a bug somebody can
    /// see and one they cannot.
    fn expect_keyword(&mut self, goal: Goal, keyword: Keyword) -> Result<Token, SyntaxError> {
        if self.at_keyword(goal, keyword)? {
            return self.bump(goal);
        }
        let token = self.look(goal)?;
        let at = token.start;
        if let Kind::Name(found) = &token.kind {
            if found.escaped && found.name == keyword.as_str() {
                let name = found.name.clone();
                return Err(SyntaxError::new(
                    Reason::KeywordWrittenWithAnEscape(name),
                    at,
                ));
            }
        }
        Err(SyntaxError::new(
            Reason::Expected {
                wanted: keyword.as_str(),
            },
            at,
        ))
    }

    /// Whether the next token is a name at all — a keyword is one.
    fn at_name(&mut self, goal: Goal) -> Result<bool, SyntaxError> {
        Ok(matches!(self.look(goal)?.kind, Kind::Name(_)))
    }

    // --- Names, and the words that may not be one ---------------------------

    /// The name a token spells, refusing the words that may not be one here.
    ///
    /// This is where [`Status`] earns its place: which words are reserved is a
    /// question about *where you are*, and the four answers are all decided
    /// here rather than in four lists that drift apart.
    fn name_of(&self, token: &Token) -> Result<String, SyntaxError> {
        let Kind::Name(found) = &token.kind else {
            return Err(SyntaxError::new(
                Reason::Expected { wanted: "a name" },
                token.start,
            ));
        };
        if found.escaped {
            // A reserved word written with an escape is not the keyword, and it
            // is not a name either — the specification makes it an early error
            // exactly so that nobody can smuggle a keyword past a check that
            // compared text.
            if word::keyword(&found.name).is_some() {
                return Err(SyntaxError::new(
                    Reason::KeywordWrittenWithAnEscape(found.name.clone()),
                    token.start,
                ));
            }
            return Ok(found.name.clone());
        }
        let refused = match word::keyword(&found.name).map(Keyword::status) {
            Some(Status::Reserved) => true,
            Some(Status::ReservedInStrictCode) => self.context.strict,
            Some(Status::ReservedWhereItMeansSomething) => {
                (found.name == "await" && self.context.inside.awaits())
                    || (found.name == "yield" && self.context.inside.yields())
                    || (found.name == "yield" && self.context.strict)
            }
            Some(Status::Contextual) | None => false,
        };
        if refused {
            return Err(SyntaxError::new(
                Reason::ReservedWordAsAName(found.name.clone()),
                token.start,
            ));
        }
        Ok(found.name.clone())
    }

    /// Read a name, refusing the words that may not be one.
    fn binding_name(&mut self, goal: Goal) -> Result<String, SyntaxError> {
        let token = self.bump(goal)?;
        self.name_of(&token)
    }

    /// Read a word that is only ever a name for something else — a property
    /// after a `.`, a key in an object literal, a name in an `import`.
    ///
    /// Every word is allowed here, `if` and `class` included, and so is an
    /// escape: `a.\u0069f` really is the property `if`. That is the difference
    /// between the specification's `IdentifierName` and its `Identifier`, and
    /// it is why the escape rule in [`Parser::name_of`] is not repeated here —
    /// the rule is about a keyword being *used as one*, and nothing here is.
    fn any_name(&mut self, goal: Goal) -> Result<String, SyntaxError> {
        let token = self.bump(goal)?;
        match token.kind {
            Kind::Name(found) => Ok(found.name),
            _ => Err(SyntaxError::new(
                Reason::Expected { wanted: "a name" },
                token.start,
            )),
        }
    }

    // --- The rules that need the line endings -------------------------------

    /// Consume a `;`, or insert one where the three rules allow.
    ///
    /// The offending token is asked for as an *operator*, which is right for
    /// the reason the famous case gives: in `a\n/b/g` the `/` continues the
    /// expression and no semicolon is inserted at all, so by the time this is
    /// reached the expression has genuinely ended.
    fn semicolon(&mut self) -> Result<(), SyntaxError> {
        if self.eat(OPERATOR, Punctuator::Semicolon)? {
            return Ok(());
        }
        let token = self.look(OPERATOR)?;
        let inserted = token.newline_before
            || token.kind == Kind::End
            || token.kind == Kind::Punctuator(Punctuator::RightBrace);
        if inserted {
            return Ok(());
        }
        let at = token.start;
        Err(SyntaxError::new(Reason::Expected { wanted: ";" }, at))
    }

    // --- Depth --------------------------------------------------------------

    /// One level further in, or a refusal.
    fn deeper(&mut self, at: usize) -> Result<(), SyntaxError> {
        self.depth = self.depth.saturating_add(1);
        if self.depth > bounds::DEEPEST_NESTING {
            return Err(SyntaxError::new(
                Reason::TooDeeplyNested {
                    most: bounds::DEEPEST_NESTING,
                },
                at,
            ));
        }
        Ok(())
    }

    /// One level back out.
    fn shallower(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    // --- Speculation --------------------------------------------------------

    /// Where the cursor is, so it can be put back.
    fn mark(&self) -> Mark<'a> {
        Mark {
            lexer: self.lexer.clone(),
            peeked: self.peeked.clone(),
            last_end: self.last_end,
            depth: self.depth,
            context: self.context,
            kept_refusal: self.kept_refusal.clone(),
        }
    }

    /// Put the cursor back where a [`Parser::mark`] was taken.
    ///
    /// By reference, because one mark is often put back from more than one
    /// place: `{ get: 1 }` and `{ get a() {} }` and `{ get() {} }` all give up
    /// at different points and all give up to the same place.
    fn back_to(&mut self, mark: &Mark<'a>) {
        self.lexer.clone_from(&mark.lexer);
        self.peeked.clone_from(&mark.peeked);
        self.last_end = mark.last_end;
        self.depth = mark.depth;
        self.context = mark.context;
        self.kept_refusal.clone_from(&mark.kept_refusal);
    }

    /// Whether this is being read as a module.
    fn is_module(&self) -> bool {
        self.which == Source::Module
    }

    /// A node that began at `start` and ends where the last token ended.
    fn expression_at(&self, kind: ExpressionKind, start: usize) -> Expression {
        Expression {
            kind,
            start,
            end: self.last_end,
        }
    }

    /// A statement that began at `start` and ends where the last token ended.
    fn statement_at(&self, kind: StatementKind, start: usize) -> Statement {
        Statement {
            kind,
            start,
            end: self.last_end,
        }
    }

    // --- Bodies -------------------------------------------------------------

    /// The statements of a program or a function body, with the strictness its
    /// directive prologue asks for.
    ///
    /// A directive is a string literal statement before anything else, judged
    /// by **what was written** rather than by what it means: `"use strict"`
    /// is a string whose value is `use strict` and is not the directive, which
    /// is why this compares the source text.
    fn directives_then_statements(
        &mut self,
        closer: Option<Punctuator>,
    ) -> Result<(Vec<Statement>, bool), SyntaxError> {
        let mut body = Vec::new();
        let mut prologue = true;
        loop {
            if let Some(closer) = closer {
                if self.at(OPERAND, closer)? {
                    break;
                }
            }
            if self.at_end(OPERAND)? {
                break;
            }
            let statement = self.statement()?;
            if prologue {
                match self.directive(&statement) {
                    Some(text) => {
                        if text == "\"use strict\"" || text == "'use strict'" {
                            self.context.strict = true;
                        }
                    }
                    None => prologue = false,
                }
            }
            body.push(statement);
        }
        Ok((body, self.context.strict))
    }

    /// The source text of a statement that is a lone string literal, which is
    /// what a directive is.
    fn directive(&self, statement: &Statement) -> Option<&'a str> {
        let StatementKind::Expression(expression) = &statement.kind else {
            return None;
        };
        if !matches!(expression.kind, ExpressionKind::String(_)) {
            return None;
        }
        Some(read::slice(self.source, expression.start, expression.end))
    }
}

/// Parse a script.
///
/// # Errors
///
/// The first thing it cannot read — see [`Reason`].
pub fn script(source: &str) -> Result<Program, SyntaxError> {
    Parser::new(source, Source::Script)?.program()
}

/// Parse a module.
///
/// # Errors
///
/// The first thing it cannot read — see [`Reason`].
pub fn module(source: &str) -> Result<Program, SyntaxError> {
    Parser::new(source, Source::Module)?.program()
}
