/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The five ways running a program ends without a value, kept apart because
//! they are answered by different people.
//!
//! A specification calls all of these *abrupt completions* and an engine that
//! flattened them into one error type would have made a decision it cannot
//! take back: **who is told**. A `TypeError` is the page's, and its own `catch`
//! is how it survives us (ADR 0013 § 3). A full heap is the embedder's, and it
//! stops the tab (ADR 0014 § 9). A stale reference is *ours*, and it is a bug
//! in this engine rather than in anybody's page (ADR 0014 § 3). An interrupt is
//! the browser process deciding a tab has stopped answering (ADR 0013 § 4).
//! And something this engine has not built yet is none of those, which is the
//! variant most engines do not have and this one needs while it is being
//! written.
//!
//! # [`Missing`] is *absent beats approximate*, at run time
//!
//! ADR 0013 § 3 says a builtin we have not written is **not defined** rather
//! than a stub returning a plausible value. The same rule has a second half
//! that only shows up once something runs: where the engine reaches a place the
//! language specifies and this engine has not built, the honest answer is
//! neither a value nor a `TypeError` the language never mentions. It is *this
//! is not built yet, and here is the queue item that builds it* — a sentence a
//! person reads, never something a page can catch and act on, because a page
//! acting on it would be acting on a lie.

use std::fmt;

use crate::heap::Full;
use crate::object::{Fault, Named, Refused, Value};

/// Which of the language's errors this is.
///
/// The four this engine can produce before it has builtins. Each is a real
/// error the specification names, thrown where the specification says to throw
/// it — ADR 0013 § 3: *where the language itself specifies an error, we produce
/// that error, because a script's own `catch` is the page's way of surviving
/// us.*
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A value was not of a kind the operation could work on.
    TypeError,
    /// A number or a length was outside what may be made.
    RangeError,
    /// A name that resolves to nothing, or one read inside its dead zone.
    ReferenceError,
}

impl Kind {
    /// The name a page would see on the error object (queue item 73).
    pub const fn name(self) -> &'static str {
        match self {
            Kind::TypeError => "TypeError",
            Kind::RangeError => "RangeError",
            Kind::ReferenceError => "ReferenceError",
        }
    }
}

/// What a script threw.
///
/// Either one of the language's own errors or a value the script threw itself.
/// There is no `Error` **object** here, because a constructor is a builtin and
/// builtins are queue item 73 — so what is decided now is the part a page can
/// see either way: which error it is, what it says, and where it happened.
#[derive(Debug, Clone, PartialEq)]
pub enum Thrown {
    /// An error the language specifies.
    Error {
        /// Which one.
        kind: Kind,
        /// What went wrong, in words.
        message: String,
        /// The byte offset in the source it happened at.
        at: usize,
    },
    /// `throw a` — whatever the script chose.
    Value {
        /// What was thrown.
        value: Value,
        /// The byte offset in the source it was thrown at.
        at: usize,
    },
}

impl Thrown {
    /// A `TypeError` saying `message`.
    pub fn type_error(message: impl Into<String>, at: usize) -> Self {
        Self::Error {
            kind: Kind::TypeError,
            message: message.into(),
            at,
        }
    }

    /// A `RangeError` saying `message`.
    pub fn range_error(message: impl Into<String>, at: usize) -> Self {
        Self::Error {
            kind: Kind::RangeError,
            message: message.into(),
            at,
        }
    }

    /// A `ReferenceError` saying `message`.
    pub fn reference_error(message: impl Into<String>, at: usize) -> Self {
        Self::Error {
            kind: Kind::ReferenceError,
            message: message.into(),
            at,
        }
    }

    /// Where in the source it happened.
    pub const fn at(&self) -> usize {
        match self {
            Thrown::Error { at, .. } | Thrown::Value { at, .. } => *at,
        }
    }
}

impl fmt::Display for Thrown {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Thrown::Error { kind, message, at } => {
                write!(out, "{}: {message} (at byte {at})", kind.name())
            }
            Thrown::Value { at, .. } => write!(out, "the script threw a value (at byte {at})"),
        }
    }
}

/// Something the language has and this engine has not built yet.
///
/// Each names the queue item that builds it, because the useful half of this
/// answer is *what to do about it*. None of these is reachable by a page in a
/// browser this engine is not yet in; all of them are reachable by the tests
/// that are building it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    /// A property was read from a string, a number, a boolean or a symbol,
    /// which needs the wrapper objects the builtins bring (queue item 73).
    AWrapperObject,
    /// `a instanceof f` reached `Get(f, "prototype")`, and a function has no
    /// `prototype` property until it has a `[[Construct]]` (queue item 212).
    ///
    /// The two answers that come *before* it are given: a right-hand side that
    /// is not callable is the `TypeError` the language specifies, and a
    /// left-hand side that is not an object is `false`.
    APrototype,
    /// A builtin was handed an **object** where the specification turns one
    /// into a primitive first — `({}).hasOwnProperty({})` — and turning one
    /// into a primitive means calling the script's own `valueOf`, which a
    /// native cannot do until queue item 219.
    ///
    /// A primitive argument is converted here and now; it is only the object
    /// that has nowhere to go, which is why this is the narrow answer rather
    /// than a refusal of the whole method.
    AConversionInsideABuiltin,
    /// `Function.prototype.toString`, which answers the text a function was
    /// written as — text no [`Unit`](crate::unit::Unit) keeps (queue item 220).
    ///
    /// It is refused rather than left to `Object.prototype.toString`, which
    /// would answer `"[object Function]"`: a wrong answer that reads like a
    /// right one is the one thing *absent beats approximate* is against.
    AFunctionsSourceText,
}

impl fmt::Display for Missing {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Missing::AWrapperObject => write!(
                out,
                "a property of a primitive needs a wrapper object, which is queue item 73"
            ),
            Missing::APrototype => write!(
                out,
                "'instanceof' needs the `prototype` property a constructor has, which is queue item 212"
            ),
            Missing::AConversionInsideABuiltin => write!(
                out,
                "a builtin was given an object where a property key was wanted, and turning one into a primitive from inside a builtin is queue item 219"
            ),
            Missing::AFunctionsSourceText => write!(
                out,
                "a function's own source text, which Function.prototype.toString answers with, is queue item 220"
            ),
        }
    }
}

/// A mistake of the engine's own.
///
/// ADR 0014 § 3: a reference that names nothing means a root was missed, and it
/// *is never a page's doing*. So it ends the script with an internal error that
/// is reported — never the process, never a panic, and never a wrong answer
/// handed back as though it were right. Under test it fails the test, which is
/// what [`Heap::stress`](crate::heap::Heap::stress) is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Internal {
    /// A reference did not name what the engine believed it named.
    Lost(Fault),
    /// The interpreter's own stack did not hold what the compiler said it
    /// would.
    StackIsWrong,
    /// A jump named an instruction that is not in the chunk.
    JumpIsWrong,
}

impl fmt::Display for Internal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "this engine has a bug: ")?;
        match self {
            Internal::Lost(fault) => write!(out, "it lost a reference — {fault}"),
            Internal::StackIsWrong => {
                write!(
                    out,
                    "the interpreter's stack did not hold what was compiled"
                )
            }
            Internal::JumpIsWrong => write!(out, "an instruction jumped outside its own code"),
        }
    }
}

/// Every way running a program ends other than with a value.
#[derive(Debug, Clone, PartialEq)]
pub enum Escape {
    /// The script threw. Its own `catch` is what survives this (queue item 210).
    Thrown(Thrown),
    /// The engine reached something it has not built.
    NotBuiltYet(Missing),
    /// The heap is at its ceiling: the embedder's, and it stops the tab.
    Full(Full),
    /// The embedder asked for the script to stop.
    Interrupted,
    /// This engine has a bug.
    Broken(Internal),
}

impl Escape {
    /// A `TypeError`.
    pub fn type_error(message: impl Into<String>, at: usize) -> Self {
        Self::Thrown(Thrown::type_error(message, at))
    }

    /// A `RangeError`.
    pub fn range_error(message: impl Into<String>, at: usize) -> Self {
        Self::Thrown(Thrown::range_error(message, at))
    }

    /// A `ReferenceError`.
    pub fn reference_error(message: impl Into<String>, at: usize) -> Self {
        Self::Thrown(Thrown::reference_error(message, at))
    }

    /// What the object model refused, as the escape it is.
    ///
    /// The two halves go to different people and that is the whole reason
    /// [`Refused`] has two variants: a string longer than
    /// [`bounds::LONGEST_STRING`](crate::bounds) is the `RangeError` the
    /// language specifies for a string that cannot be made, and a heap at its
    /// ceiling is ADR 0014 § 9's, which no script may catch its way past.
    pub fn refused(refused: Refused, at: usize) -> Self {
        match refused {
            Refused::Full(full) => Self::Full(full),
            Refused::StringTooLong { units } => Self::range_error(
                format!("a string of {units} code units is longer than this engine will make"),
                at,
            ),
        }
    }

    /// A fault, which is always the engine's own mistake.
    pub const fn fault(fault: Fault) -> Self {
        Self::Broken(Internal::Lost(fault))
    }

    /// What a by-name operation refused, which is either of the two above.
    pub fn named(named: Named, at: usize) -> Self {
        match named {
            Named::Refused(refused) => Self::refused(refused, at),
            Named::Fault(fault) => Self::fault(fault),
        }
    }

    /// Whether a page could have seen this — which is exactly the set a `catch`
    /// will one day be able to reach (queue item 210).
    pub const fn is_the_pages(&self) -> bool {
        matches!(self, Escape::Thrown(_))
    }
}

impl fmt::Display for Escape {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Escape::Thrown(thrown) => thrown.fmt(out),
            Escape::NotBuiltYet(missing) => missing.fmt(out),
            Escape::Full(full) => full.fmt(out),
            Escape::Interrupted => write!(out, "the script was stopped"),
            Escape::Broken(internal) => internal.fmt(out),
        }
    }
}

impl From<Fault> for Escape {
    fn from(fault: Fault) -> Self {
        Self::fault(fault)
    }
}

impl From<Internal> for Escape {
    fn from(internal: Internal) -> Self {
        Self::Broken(internal)
    }
}

#[cfg(test)]
mod tests {
    use super::{Escape, Internal, Kind, Missing, Thrown};
    use crate::heap::Full;
    use crate::object::{Fault, Refused};

    #[test]
    fn a_string_too_long_is_the_pages_and_a_full_heap_is_not() {
        let too_long = Escape::refused(Refused::StringTooLong { units: 9 }, 3);
        assert!(too_long.is_the_pages(), "a RangeError is catchable");
        assert!(matches!(
            too_long,
            Escape::Thrown(Thrown::Error {
                kind: Kind::RangeError,
                ..
            })
        ));

        let full = Escape::refused(
            Refused::Full(Full {
                asked: 1,
                held: 2,
                ceiling: 3,
            }),
            3,
        );
        assert!(!full.is_the_pages(), "a full heap goes to the embedder");
    }

    #[test]
    fn a_lost_reference_is_ours_rather_than_the_pages() {
        let escape = Escape::fault(Fault::Gone);
        assert_eq!(escape, Escape::Broken(Internal::Lost(Fault::Gone)));
        assert!(!escape.is_the_pages());
        assert!(escape.to_string().contains("this engine has a bug"));
    }

    #[test]
    fn something_not_built_says_which_item_builds_it() {
        assert!(
            Escape::NotBuiltYet(Missing::APrototype)
                .to_string()
                .contains("212")
        );
        assert!(
            Escape::NotBuiltYet(Missing::AWrapperObject)
                .to_string()
                .contains("73")
        );
    }
}
