/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a parsed program is.
//!
//! # Every node says where it was
//!
//! [`Statement::start`] and [`Expression::start`], with their ends, are byte
//! offsets into the source the parser was given — the same offsets
//! [`crate::Token`] carries and for the same three reasons: an error somebody is
//! shown points at one, a stack trace (queue item 78) is built from them, and
//! the source text of a function is what `Function.prototype.toString` has to
//! answer with. Adding them afterwards means touching every construction site
//! in the parser, which is the argument for having them from the first commit —
//! the same argument [`crate::token`] already makes and the same one ADR 0003
//! makes about node identity.
//!
//! # A parenthesis is not a node
//!
//! `(a)` and `a` are the same tree here. The parentheses that matter are the
//! ones the *grammar* reads — an arrow function's parameter list, a `for`
//! header — and those are nodes of their own. Keeping a node for the rest would
//! be keeping punctuation, and every reader of the tree would then have to know
//! to look past it. Where a page can tell the difference is
//! `Function.prototype.toString`, which answers with source text rather than
//! with a printed tree, and the offsets above are how it will.
//!
//! # What is deliberately not decided here
//!
//! A [`Pattern`] can hold a [`Pattern::Member`], which no *declaration* may
//! have — `let a.b = 1` is not a program. The parser refuses it where it parses
//! a declaration rather than the type refusing it everywhere, because the same
//! shape is exactly what an assignment needs: `[a.b] = c` is ordinary. One type
//! for both, with the rule written where the rule is, beats two types that
//! differ in one variant and drift.

use crate::number::Radix;
use crate::regexp;
use crate::template;

/// Which of the two things a piece of source text is read as.
///
/// It is the caller's to decide and never ours to guess: the same bytes are a
/// legal script and a legal module with different meanings — `await` is a name
/// in one and reserved in the other — and what decides is how the page asked
/// for the file, which is a fact about the HTML rather than about the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A classic script: `<script>` without `type="module"`.
    Script,
    /// A module: `<script type="module">`, or anything `import`ed.
    Module,
}

/// A parsed program.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    /// Which goal it was parsed for.
    pub source: Source,
    /// Whether the whole of it is strict code — a module, or a script whose
    /// first statements include a `"use strict"` directive.
    pub strict: bool,
    /// Its statements, in order.
    pub body: Vec<Statement>,
}

/// One statement, and where it was.
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    /// What it is.
    pub kind: StatementKind,
    /// The byte offset it starts at.
    pub start: usize,
    /// The byte offset just past it.
    pub end: usize,
}

/// What a statement is.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum StatementKind {
    /// `a + b;`
    Expression(Expression),
    /// `{ … }`
    Block(Vec<Statement>),
    /// `;`
    Empty,
    /// `var a = 1`, `let a`, `const a = 1`.
    Declaration(Declaration),
    /// `function a() {}`
    Function(Box<Function>),
    /// `class A {}`
    Class(Box<Class>),
    /// `if (a) b; else c;`
    If {
        /// The condition.
        test: Expression,
        /// What runs when it holds.
        consequent: Box<Statement>,
        /// What runs when it does not.
        alternate: Option<Box<Statement>>,
    },
    /// `for (a; b; c) d;`
    For {
        /// The first clause, which is a declaration, an expression or nothing.
        init: Option<ForInit>,
        /// The condition, tested before every pass.
        test: Option<Expression>,
        /// What runs after every pass.
        update: Option<Expression>,
        /// The body.
        body: Box<Statement>,
    },
    /// `for (a in b) c;`
    ForIn {
        /// What is assigned each key.
        left: ForTarget,
        /// The object whose keys are walked.
        right: Expression,
        /// The body.
        body: Box<Statement>,
    },
    /// `for (a of b) c;`, and `for await (a of b) c;`
    ForOf {
        /// What is assigned each value.
        left: ForTarget,
        /// The iterable.
        right: Expression,
        /// Whether each value is awaited.
        is_await: bool,
        /// The body.
        body: Box<Statement>,
    },
    /// `while (a) b;`
    While {
        /// The condition.
        test: Expression,
        /// The body.
        body: Box<Statement>,
    },
    /// `do a; while (b)`
    DoWhile {
        /// The body, which runs before the condition is first tested.
        body: Box<Statement>,
        /// The condition.
        test: Expression,
    },
    /// `switch (a) { case b: … }`
    Switch {
        /// What each case is compared with.
        discriminant: Expression,
        /// The cases, in the order they were written.
        cases: Vec<SwitchCase>,
    },
    /// `continue`, or `continue outer`.
    Continue(Option<String>),
    /// `break`, or `break outer`.
    Break(Option<String>),
    /// `return`, or `return a`.
    Return(Option<Expression>),
    /// `throw a`
    Throw(Expression),
    /// `try { … } catch { … } finally { … }`
    Try {
        /// The guarded block.
        block: Vec<Statement>,
        /// The handler, if there is one.
        handler: Option<Catch>,
        /// The block that runs either way, if there is one.
        finalizer: Option<Vec<Statement>>,
    },
    /// `outer: for (…) …`
    Labelled {
        /// The label.
        label: String,
        /// What it labels.
        body: Box<Statement>,
    },
    /// `debugger`
    Debugger,
    /// `import a from "b"` — a module declaration, never in a script.
    Import(Import),
    /// `export …` — a module declaration, never in a script.
    Export(Export),
}

/// A `var`, `let` or `const`, and what it declares.
#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    /// Which of the three it is.
    pub kind: DeclarationKind,
    /// What it declares, in order.
    pub declarators: Vec<Declarator>,
}

/// Which of the three declaration keywords was written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationKind {
    /// `var`
    Var,
    /// `let`
    Let,
    /// `const`
    Const,
}

/// One name — or one pattern — a declaration declares.
#[derive(Debug, Clone, PartialEq)]
pub struct Declarator {
    /// What is bound.
    pub pattern: Pattern,
    /// What it starts as, if anything.
    pub init: Option<Expression>,
}

/// The first clause of a `for (…;…;…)`.
#[derive(Debug, Clone, PartialEq)]
pub enum ForInit {
    /// `for (let a = 0; …)`
    Declaration(Declaration),
    /// `for (a = 0; …)`
    Expression(Expression),
}

/// What a `for…in` or `for…of` assigns to.
#[derive(Debug, Clone, PartialEq)]
pub enum ForTarget {
    /// `for (const a of b)`
    Declaration(Declaration),
    /// `for (a of b)`, `for ([a, b] of c)`
    Target(Pattern),
}

/// One `case`, or the `default`.
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    /// What it is compared with, or [`None`] for the `default`.
    pub test: Option<Expression>,
    /// What it runs.
    pub body: Vec<Statement>,
}

/// A `catch`, with or without the binding it may leave out.
#[derive(Debug, Clone, PartialEq)]
pub struct Catch {
    /// What the thrown value is bound to, or [`None`] for `catch { … }`.
    pub parameter: Option<Pattern>,
    /// The block.
    pub body: Vec<Statement>,
}

/// One expression, and where it was.
#[derive(Debug, Clone, PartialEq)]
pub struct Expression {
    /// What it is.
    pub kind: ExpressionKind,
    /// The byte offset it starts at.
    pub start: usize,
    /// The byte offset just past it.
    pub end: usize,
}

/// What an expression is.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ExpressionKind {
    /// A name: `a`.
    Name(String),
    /// `this`
    This,
    /// `super`, which is only ever the object of a member or the callee of a
    /// call — the parser refuses it anywhere else.
    Super,
    /// `#a`, which is an expression only in `#a in b`.
    PrivateName(String),
    /// `null`
    Null,
    /// `true` and `false`.
    Boolean(bool),
    /// A number, already rounded by the lexer.
    Number(f64),
    /// A `BigInt`, kept as digits until item 71 has something to hold one.
    BigInt {
        /// The digits, with any `_` removed.
        digits: String,
        /// What base they are in.
        radix: Radix,
    },
    /// A string, as UTF-16 code units.
    String(Vec<u16>),
    /// A regular expression literal, unread — the pattern is item 74's.
    RegularExpression(regexp::Literal),
    /// `` `a${b}c` ``
    Template(Template),
    /// `` tag`a${b}c` ``
    TaggedTemplate {
        /// What is called with the pieces.
        tag: Box<Expression>,
        /// The template it is called with.
        template: Template,
    },
    /// `[a, , b, ...c]`
    Array(Vec<ArrayElement>),
    /// `{ a, b: c, ...d }`
    Object(Vec<Property>),
    /// `function a() {}` as an expression, and `() => a`.
    Function(Box<Function>),
    /// `class A {}` as an expression.
    Class(Box<Class>),
    /// `-a`, `typeof a`, `delete a.b`.
    Unary {
        /// Which operator.
        operator: Unary,
        /// What it applies to.
        argument: Box<Expression>,
    },
    /// `a++`, `--a`.
    Update {
        /// Whether it adds or subtracts.
        increment: bool,
        /// Whether it was written before its argument.
        prefix: bool,
        /// What it applies to.
        argument: Box<Expression>,
    },
    /// `a + b`, `a instanceof b`, `#a in b`.
    Binary {
        /// Which operator.
        operator: Binary,
        /// The left side.
        left: Box<Expression>,
        /// The right side.
        right: Box<Expression>,
    },
    /// `a && b`, `a || b`, `a ?? b` — apart from [`ExpressionKind::Binary`]
    /// because they do not evaluate their right side.
    Logical {
        /// Which operator.
        operator: Logical,
        /// The left side.
        left: Box<Expression>,
        /// The right side, which may not be evaluated.
        right: Box<Expression>,
    },
    /// `a = b`, `a += b`, `[a, b] = c`.
    Assignment {
        /// Which operator.
        operator: Assign,
        /// What is assigned to.
        target: Pattern,
        /// What is assigned.
        value: Box<Expression>,
    },
    /// `a ? b : c`
    Conditional {
        /// The condition.
        test: Box<Expression>,
        /// What it is when the condition holds.
        consequent: Box<Expression>,
        /// What it is when it does not.
        alternate: Box<Expression>,
    },
    /// `a.b`, `a[b]`, `a?.b`, `a.#b`.
    Member {
        /// What is being read.
        object: Box<Expression>,
        /// Which member.
        member: Member,
        /// Whether it was written `?.`.
        optional: bool,
    },
    /// `a(b)`, `a?.(b)`.
    Call {
        /// What is called.
        callee: Box<Expression>,
        /// The arguments.
        arguments: Vec<Argument>,
        /// Whether it was written `?.(`.
        optional: bool,
    },
    /// `new a(b)`
    New {
        /// What is constructed.
        callee: Box<Expression>,
        /// The arguments.
        arguments: Vec<Argument>,
    },
    /// The whole of a chain that contains an optional link.
    ///
    /// A node of its own because short-circuiting is a property of the chain
    /// rather than of the link: `a?.b.c` skips `.c` as well when `a` is
    /// nullish, and an interpreter that read only the links would have to walk
    /// back up to know where to stop.
    Chain(Box<Expression>),
    /// `a, b` — the comma operator.
    Sequence(Vec<Expression>),
    /// `yield`, `yield a`, `yield* a`.
    Yield {
        /// What is yielded, if anything.
        argument: Option<Box<Expression>>,
        /// Whether it was written `yield*`.
        delegate: bool,
    },
    /// `await a`
    Await(Box<Expression>),
    /// `new.target`
    NewTarget,
    /// `import.meta`
    ImportMeta,
    /// `import(a)`, and `import(a, b)` with its options.
    ImportCall {
        /// What is imported.
        specifier: Box<Expression>,
        /// The options, if any were given.
        options: Option<Box<Expression>>,
    },
}

/// A template literal, in the pieces the lexer read it as.
///
/// There is always one more piece than there are expressions:
/// `` `a${b}c${d}e` `` is three pieces and two expressions, and
/// `` `a` `` is one piece and none.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    /// The text between the substitutions, in order.
    pub pieces: Vec<template::Piece>,
    /// The substitutions, in order.
    pub expressions: Vec<Expression>,
}

/// One element of an array literal.
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayElement {
    /// A hole: the gap in `[a, , b]`, which is not `undefined` and is why an
    /// element is not simply an expression.
    Hole,
    /// An ordinary element.
    Item(Expression),
    /// `...a`
    Spread(Expression),
}

/// One argument of a call.
#[derive(Debug, Clone, PartialEq)]
pub enum Argument {
    /// An ordinary argument.
    Item(Expression),
    /// `...a`
    Spread(Expression),
}

/// One property of an object literal.
#[derive(Debug, Clone, PartialEq)]
pub enum Property {
    /// `a: b`, and the `a` of `{ a }`.
    Named {
        /// The key.
        key: Key,
        /// The value.
        value: Expression,
        /// Whether it was written as `{ a }` rather than `{ a: a }`.
        shorthand: bool,
    },
    /// `a() {}`, `get a() {}`, `set a(b) {}`, `*a() {}`, `async a() {}`.
    Method(Method),
    /// `...a`
    Spread(Expression),
}

/// A property's name, as it was written.
#[derive(Debug, Clone, PartialEq)]
pub enum Key {
    /// `a: 1`, and every keyword — `{ if: 1 }` is a property called `if`.
    Name(String),
    /// `"a": 1`
    String(Vec<u16>),
    /// `1: a`
    Number(f64),
    /// `[a]: b`
    Computed(Expression),
    /// `#a` — only ever a class member.
    Private(String),
}

/// Which member of an object is read.
#[derive(Debug, Clone, PartialEq)]
pub enum Member {
    /// `a.b`
    Name(String),
    /// `a[b]`
    Computed(Box<Expression>),
    /// `a.#b`
    Private(String),
}

/// A unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unary {
    /// `delete a.b`
    Delete,
    /// `void a`
    Void,
    /// `typeof a`
    TypeOf,
    /// `+a`
    Plus,
    /// `-a`
    Minus,
    /// `~a`
    BitNot,
    /// `!a`
    Not,
}

/// A binary operator that evaluates both of its sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Binary {
    /// `a + b`
    Add,
    /// `a - b`
    Subtract,
    /// `a * b`
    Multiply,
    /// `a / b`
    Divide,
    /// `a % b`
    Remainder,
    /// `a ** b`
    Power,
    /// `a << b`
    ShiftLeft,
    /// `a >> b`
    ShiftRight,
    /// `a >>> b`
    ShiftRightUnsigned,
    /// `a < b`
    Less,
    /// `a > b`
    Greater,
    /// `a <= b`
    LessOrEqual,
    /// `a >= b`
    GreaterOrEqual,
    /// `a instanceof b`
    InstanceOf,
    /// `a in b`
    In,
    /// `a == b`
    Equal,
    /// `a != b`
    NotEqual,
    /// `a === b`
    StrictlyEqual,
    /// `a !== b`
    StrictlyNotEqual,
    /// `a & b`
    BitAnd,
    /// `a | b`
    BitOr,
    /// `a ^ b`
    BitXor,
}

/// An operator that may not evaluate its right side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Logical {
    /// `a && b`
    And,
    /// `a || b`
    Or,
    /// `a ?? b`
    Coalesce,
}

/// An assignment operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Assign {
    /// `a = b`
    Assign,
    /// `a += b`
    Add,
    /// `a -= b`
    Subtract,
    /// `a *= b`
    Multiply,
    /// `a /= b`
    Divide,
    /// `a %= b`
    Remainder,
    /// `a **= b`
    Power,
    /// `a <<= b`
    ShiftLeft,
    /// `a >>= b`
    ShiftRight,
    /// `a >>>= b`
    ShiftRightUnsigned,
    /// `a &= b`
    BitAnd,
    /// `a |= b`
    BitOr,
    /// `a ^= b`
    BitXor,
    /// `a &&= b`
    And,
    /// `a ||= b`
    Or,
    /// `a ??= b`
    Coalesce,
}

/// What a binding or an assignment target names.
///
/// One type for both, because `[a, b] = c` and `let [a, b] = c` take the same
/// shape apart from one variant — see this file's header for why the difference
/// is a rule in the parser rather than a second type.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `a`
    Name(String),
    /// `a.b`, `a[b]` — an assignment target and never a declaration.
    Member(Box<Expression>),
    /// `[a, , b, ...c]`
    Array {
        /// The elements, with [`None`] for a hole.
        elements: Vec<Option<Element>>,
        /// `...c`, if there is one.
        rest: Option<Box<Pattern>>,
    },
    /// `{ a, b: c, ...d }`
    Object {
        /// The properties.
        properties: Vec<PatternProperty>,
        /// `...d`, if there is one.
        rest: Option<Box<Pattern>>,
    },
}

/// One element of a pattern, with the value it takes when there is none.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// What is bound.
    pub pattern: Pattern,
    /// What it is when the value is `undefined`.
    pub default: Option<Expression>,
}

/// One property of an object pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct PatternProperty {
    /// Which property is read.
    pub key: Key,
    /// What it is bound to, and its default.
    pub value: Element,
    /// Whether it was written as `{ a }` rather than `{ a: a }`.
    pub shorthand: bool,
}

/// Which of the four kinds of function something is.
///
/// One value rather than two flags, because the two are read together
/// everywhere: what `await` and `yield` mean inside a body is decided by the
/// pair and never by one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionKind {
    /// `function a() {}`
    Plain,
    /// `async function a() {}`
    Async,
    /// `function* a() {}`
    Generator,
    /// `async function* a() {}`
    AsyncGenerator,
}

impl FunctionKind {
    /// The kind an `async` and a `*` together describe.
    pub fn of(is_async: bool, is_generator: bool) -> Self {
        match (is_async, is_generator) {
            (false, false) => Self::Plain,
            (true, false) => Self::Async,
            (false, true) => Self::Generator,
            (true, true) => Self::AsyncGenerator,
        }
    }

    /// Whether it was written `async`.
    pub fn is_async(self) -> bool {
        matches!(self, Self::Async | Self::AsyncGenerator)
    }

    /// Whether it was written with a `*`.
    pub fn is_generator(self) -> bool {
        matches!(self, Self::Generator | Self::AsyncGenerator)
    }
}

/// A function, an arrow function, or a method's body and parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    /// Its name, where it has one.
    pub name: Option<String>,
    /// Its parameters, in order.
    pub parameters: Vec<Element>,
    /// `...rest`, if there is one.
    pub rest: Option<Pattern>,
    /// Its body.
    pub body: Body,
    /// Which of the four kinds it is.
    pub kind: FunctionKind,
    /// Whether it was written as an arrow, which changes what `this` is.
    pub is_arrow: bool,
    /// Whether its body is strict code.
    pub strict: bool,
    /// The byte offset it starts at.
    pub start: usize,
    /// The byte offset just past it.
    pub end: usize,
}

/// What a function does.
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    /// `{ … }`
    Block(Vec<Statement>),
    /// The `a` of `() => a`, which returns without saying so.
    Expression(Box<Expression>),
}

/// A class.
#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    /// Its name, where it has one.
    pub name: Option<String>,
    /// What it extends, if anything.
    pub heritage: Option<Box<Expression>>,
    /// Its members, in the order they were written.
    pub members: Vec<ClassMember>,
    /// The byte offset it starts at.
    pub start: usize,
    /// The byte offset just past it.
    pub end: usize,
}

/// One member of a class.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassMember {
    /// `a() {}`, `get a() {}`, `constructor() {}`.
    Method(Method),
    /// `a = 1`, `#a`.
    Field {
        /// Its name.
        key: Key,
        /// What it starts as, if anything.
        value: Option<Expression>,
        /// Whether it was written `static`.
        is_static: bool,
    },
    /// `static { … }`
    StaticBlock(Vec<Statement>),
}

/// A method, of a class or of an object literal.
#[derive(Debug, Clone, PartialEq)]
pub struct Method {
    /// Its name.
    pub key: Key,
    /// Which kind of method it is.
    pub kind: MethodKind,
    /// Its parameters and body.
    pub function: Function,
    /// Whether it was written `static`, which is always false in an object.
    pub is_static: bool,
}

/// Which kind of method something is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    /// An ordinary one.
    Method,
    /// `constructor() {}`
    Constructor,
    /// `get a() {}`
    Get,
    /// `set a(b) {}`
    Set,
}

/// An `import` declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    /// What is bound, which is empty for `import "a"`.
    pub specifiers: Vec<ImportSpecifier>,
    /// Where it comes from, as the code units the author wrote — resolving it
    /// against a base is [`crate`]'s embedder's, not ours (ADR 0013 § 5).
    pub source: Vec<u16>,
}

/// One thing an `import` binds.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportSpecifier {
    /// `import a from "b"`
    Default(String),
    /// `import * as a from "b"`
    Namespace(String),
    /// `import { a as b } from "c"`
    Named {
        /// The name the module exports, which a string may spell.
        exported: ModuleName,
        /// The name it is bound to here.
        local: String,
    },
}

/// An `export` declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum Export {
    /// `export const a = 1`, `export function a() {}`.
    Declaration(Box<Statement>),
    /// `export { a as b }`, and `export { a } from "c"`.
    Named {
        /// What is exported.
        specifiers: Vec<ExportSpecifier>,
        /// Where they come from, for a re-export.
        source: Option<Vec<u16>>,
    },
    /// `export default a`, `export default function () {}`.
    Default(Box<Statement>),
    /// `export * from "a"`, and `export * as b from "a"`.
    All {
        /// The name the whole module is exported under, if it was given one.
        alias: Option<ModuleName>,
        /// Where it comes from.
        source: Vec<u16>,
    },
}

/// One thing an `export` names.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportSpecifier {
    /// The name here, which is a string only in a re-export.
    pub local: ModuleName,
    /// The name it is exported under.
    pub exported: ModuleName,
}

/// A name in an `import` or `export`, which may be written as a string.
///
/// `export { a as "b c" }` is legal and is how a module written in another
/// language exports a name JavaScript cannot spell. A string is kept as code
/// units for the reason every string here is: it may hold half a surrogate
/// pair, and a [`String`] cannot.
#[derive(Debug, Clone, PartialEq)]
pub enum ModuleName {
    /// `a`
    Name(String),
    /// `"a b"`
    String(Vec<u16>),
}
