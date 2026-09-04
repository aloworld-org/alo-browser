/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Names, and the words the language keeps for itself.
//!
//! # Every word is a name here, and the parser decides which are keywords
//!
//! It is usual for a lexer to answer `if` with a keyword token and `foo` with
//! an identifier, and it is wrong for this language: **which words are reserved
//! depends on context.** `yield` is an identifier outside a generator, `await`
//! is one outside a module and an async function, and `let`, `static` and
//! `private` are identifiers everywhere except strict mode. A lexer that
//! decided would be deciding a thing it cannot see.
//!
//! So [`scan`] answers a [`Word`], every time, and [`keyword`] is the table the
//! parser asks — with [`Status`] saying *how* reserved a word is, so that item
//! 204 reads a rule rather than remembering four lists.
//!
//! # The one thing only the lexer knows: whether a word was spelled with escapes
//!
//! `if` is a name whose characters are `i` and `f`, and it is **not**
//! the keyword `if` — the specification says a reserved word written with any
//! escape in it is an early error, exactly so that nobody can smuggle a keyword
//! past a check that compared text. Once a word is a `String` that is
//! unknowable, so [`Word::escaped`] records it at the only moment it is visible.

use crate::error::{Reason, SyntaxError};
use crate::{escape, read, unicode};

/// A name, as it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    /// The characters it stands for, with any escapes resolved.
    pub name: String,
    /// Whether any of it was written as a `\u` escape.
    ///
    /// A word with this set is never a keyword, however it reads.
    pub escaped: bool,
}

/// Read the name beginning at `at`.
///
/// `at` must be a character that starts a name or a `\`; the caller has already
/// looked. Answers the word and the offset just past it.
///
/// # Errors
///
/// A `\` that is not a `\u`, a malformed `\u`, or an escape standing for a
/// character that may not be in a name.
pub fn scan(source: &str, at: usize) -> Result<(Word, usize), SyntaxError> {
    let mut name = String::new();
    let mut escaped = false;
    let mut end = at;
    let mut first = true;
    loop {
        if read::char_at(source, end) == Some('\\') {
            let after_backslash = end.saturating_add(1);
            if read::char_at(source, after_backslash) != Some('u') {
                return Err(SyntaxError::new(Reason::BadUnicodeEscape, end));
            }
            let (value, after) = escape::code_point(source, after_backslash.saturating_add(1))?;
            let c = char::from_u32(value)
                .filter(|c| {
                    if first {
                        unicode::starts_a_name(*c)
                    } else {
                        unicode::continues_a_name(*c)
                    }
                })
                .ok_or_else(|| SyntaxError::new(Reason::EscapeIsNotANameCharacter, end))?;
            name.push(c);
            escaped = true;
            end = after;
        } else {
            let Some((c, after)) = read::next_char(source, end) else {
                break;
            };
            let allowed = if first {
                unicode::starts_a_name(c)
            } else {
                unicode::continues_a_name(c)
            };
            if !allowed {
                break;
            }
            name.push(c);
            end = after;
        }
        first = false;
    }
    if name.is_empty() {
        let c = read::char_at(source, at).unwrap_or('\u{0}');
        return Err(SyntaxError::new(Reason::UnexpectedCharacter(c), at));
    }
    Ok((Word { name, escaped }, end))
}

/// How reserved a word is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Never a name, anywhere: `if`, `class`, `function`.
    Reserved,
    /// A name outside one construction and reserved inside it — `await` outside
    /// a module and an async function, `yield` outside a generator.
    ReservedWhereItMeansSomething,
    /// A name in sloppy code and reserved in strict: `let`, `static`,
    /// `private`. Which of the two a piece of code is, is the parser's to know.
    ReservedInStrictCode,
    /// Never reserved, and meaningful in one place: `of` in a `for`, `from` in
    /// an import, `get` before a method name.
    Contextual,
}

/// A word the language has a use for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Keyword {
    /// `as`
    As,
    /// `async`
    Async,
    /// `await`
    Await,
    /// `break`
    Break,
    /// `case`
    Case,
    /// `catch`
    Catch,
    /// `class`
    Class,
    /// `const`
    Const,
    /// `continue`
    Continue,
    /// `debugger`
    Debugger,
    /// `default`
    Default,
    /// `delete`
    Delete,
    /// `do`
    Do,
    /// `else`
    Else,
    /// `enum`
    Enum,
    /// `export`
    Export,
    /// `extends`
    Extends,
    /// `false`
    False,
    /// `finally`
    Finally,
    /// `for`
    For,
    /// `from`
    From,
    /// `function`
    Function,
    /// `get`
    Get,
    /// `if`
    If,
    /// `implements`
    Implements,
    /// `import`
    Import,
    /// `in`
    In,
    /// `instanceof`
    InstanceOf,
    /// `interface`
    Interface,
    /// `let`
    Let,
    /// `meta`
    Meta,
    /// `new`
    New,
    /// `null`
    Null,
    /// `of`
    Of,
    /// `package`
    Package,
    /// `private`
    Private,
    /// `protected`
    Protected,
    /// `public`
    Public,
    /// `return`
    Return,
    /// `set`
    Set,
    /// `static`
    Static,
    /// `super`
    Super,
    /// `switch`
    Switch,
    /// `target`
    Target,
    /// `this`
    This,
    /// `throw`
    Throw,
    /// `true`
    True,
    /// `try`
    Try,
    /// `typeof`
    TypeOf,
    /// `var`
    Var,
    /// `void`
    Void,
    /// `while`
    While,
    /// `with`
    With,
    /// `yield`
    Yield,
}

impl Keyword {
    /// How it is spelled.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::As => "as",
            Self::Async => "async",
            Self::Await => "await",
            Self::Break => "break",
            Self::Case => "case",
            Self::Catch => "catch",
            Self::Class => "class",
            Self::Const => "const",
            Self::Continue => "continue",
            Self::Debugger => "debugger",
            Self::Default => "default",
            Self::Delete => "delete",
            Self::Do => "do",
            Self::Else => "else",
            Self::Enum => "enum",
            Self::Export => "export",
            Self::Extends => "extends",
            Self::False => "false",
            Self::Finally => "finally",
            Self::For => "for",
            Self::From => "from",
            Self::Function => "function",
            Self::Get => "get",
            Self::If => "if",
            Self::Implements => "implements",
            Self::Import => "import",
            Self::In => "in",
            Self::InstanceOf => "instanceof",
            Self::Interface => "interface",
            Self::Let => "let",
            Self::Meta => "meta",
            Self::New => "new",
            Self::Null => "null",
            Self::Of => "of",
            Self::Package => "package",
            Self::Private => "private",
            Self::Protected => "protected",
            Self::Public => "public",
            Self::Return => "return",
            Self::Set => "set",
            Self::Static => "static",
            Self::Super => "super",
            Self::Switch => "switch",
            Self::Target => "target",
            Self::This => "this",
            Self::Throw => "throw",
            Self::True => "true",
            Self::Try => "try",
            Self::TypeOf => "typeof",
            Self::Var => "var",
            Self::Void => "void",
            Self::While => "while",
            Self::With => "with",
            Self::Yield => "yield",
        }
    }

    /// How reserved it is.
    ///
    /// `with` is [`Status::Reserved`] here and is refused by the parser rather
    /// than by the lexer: ADR 0013 § 3 sends `with` to the legacy tail, and a
    /// program that uses it should be told that it is refused rather than told
    /// that `with` is an undeclared name.
    pub fn status(self) -> Status {
        match self {
            Self::Await | Self::Yield => Status::ReservedWhereItMeansSomething,
            Self::Implements
            | Self::Interface
            | Self::Let
            | Self::Package
            | Self::Private
            | Self::Protected
            | Self::Public
            | Self::Static => Status::ReservedInStrictCode,
            Self::As
            | Self::Async
            | Self::From
            | Self::Get
            | Self::Meta
            | Self::Of
            | Self::Set
            | Self::Target => Status::Contextual,
            _ => Status::Reserved,
        }
    }
}

/// Every word in the table, in the order they are spelled.
///
/// A slice rather than a map: fifty-three entries compared by length and then
/// by bytes is faster than hashing, and it is one place a new word is added.
const ALL: &[Keyword] = &[
    Keyword::As,
    Keyword::Async,
    Keyword::Await,
    Keyword::Break,
    Keyword::Case,
    Keyword::Catch,
    Keyword::Class,
    Keyword::Const,
    Keyword::Continue,
    Keyword::Debugger,
    Keyword::Default,
    Keyword::Delete,
    Keyword::Do,
    Keyword::Else,
    Keyword::Enum,
    Keyword::Export,
    Keyword::Extends,
    Keyword::False,
    Keyword::Finally,
    Keyword::For,
    Keyword::From,
    Keyword::Function,
    Keyword::Get,
    Keyword::If,
    Keyword::Implements,
    Keyword::Import,
    Keyword::In,
    Keyword::InstanceOf,
    Keyword::Interface,
    Keyword::Let,
    Keyword::Meta,
    Keyword::New,
    Keyword::Null,
    Keyword::Of,
    Keyword::Package,
    Keyword::Private,
    Keyword::Protected,
    Keyword::Public,
    Keyword::Return,
    Keyword::Set,
    Keyword::Static,
    Keyword::Super,
    Keyword::Switch,
    Keyword::Target,
    Keyword::This,
    Keyword::Throw,
    Keyword::True,
    Keyword::Try,
    Keyword::TypeOf,
    Keyword::Var,
    Keyword::Void,
    Keyword::While,
    Keyword::With,
    Keyword::Yield,
];

/// The keyword a name spells, if it spells one.
///
/// Takes the text rather than a [`Word`] on purpose: a word written with an
/// escape is never a keyword, and making the caller pass
/// [`Word::name`] alongside its own check of [`Word::escaped`] would let that
/// rule be forgotten. [`crate::token::Kind::Name`] carries both, and the parser
/// asks this only when the word was written plainly.
pub fn keyword(name: &str) -> Option<Keyword> {
    ALL.iter().copied().find(|k| k.as_str() == name)
}
