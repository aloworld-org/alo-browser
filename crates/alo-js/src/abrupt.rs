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
    /// The name a page would see on the error object (queue item 227).
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
/// An error this engine throws is not an `Error` **object** while it is
/// unwinding: it becomes an instance of its kind's constructor (queue item
/// 227) only when a `catch` lands it (item 210), because only then can a page
/// see it — and an uncaught one is reported to the embedder as the kind and
/// message it is.
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
    /// `a instanceof f` reached `OrdinaryHasInstance`'s `Get(f, "prototype")`
    /// and the walk up `a`'s chain (queue item 224).
    ///
    /// A constructor has its `prototype` now (queue item 212), and what is
    /// still owed is the rest of the operator: the `Symbol.hasInstance` method
    /// the specification consults *before* that, which is a well-known symbol
    /// and item 73's, and a `prototype` that is a getter, which is a call.
    ///
    /// The two answers that come *before* it are given: a right-hand side that
    /// is not callable is the `TypeError` the language specifies, and a
    /// left-hand side that is not an object is `false`.
    APrototype,
    /// `Function.prototype.toString`, which answers the text a function was
    /// written as — text no [`Unit`](crate::unit::Unit) keeps (queue item 220).
    ///
    /// It is refused rather than left to `Object.prototype.toString`, which
    /// would answer `"[object Function]"`: a wrong answer that reads like a
    /// right one is the one thing *absent beats approximate* is against.
    AFunctionsSourceText,
    /// An object assigned to an array's `length` (queue item 226).
    ///
    /// `ArraySetLength` converts the value with `ToUint32` **and** with
    /// `ToNumber`, so an object's `valueOf` is called twice and a page can
    /// count the calls; and the assignment still evaluates to the object
    /// rather than to the number it became. Converting it once in place, which
    /// is what every other operator here does, would get both wrong.
    AnObjectAsALength,
    /// `Error.prototype.toString` on an error whose `message` is a getter or
    /// an object whose own `toString` must run (queue item 228).
    ///
    /// By then `name` has been read and converted, and keeping it across a
    /// call is the traced scratch state of item 221; reading `name` again
    /// afterwards would be a second getter call a page can count.
    AMessageBehindACall,
    /// An array iterator's `next` reaching an element, or an array-like's
    /// `length`, that is behind a getter or is an object to convert (queue item
    /// 231).
    ///
    /// The specification writes an array iterator as a generator, and running
    /// a script from inside one makes two of a generator's states observable
    /// that this iterator does not keep: *executing*, which a getter calling
    /// `next` again sees as a `TypeError`, and *completed* after a throw, which
    /// every later `next` sees as `done`. An iterator that skipped either would
    /// answer a page that can tell.
    AnIteratedValueBehindACall,
    /// A builtin's second argument that must be turned into a primitive by
    /// running the script, when its first already was (queue item 221).
    ///
    /// The first argument's converted value is in the one slot the second
    /// conversion's answer is written to. Converting the first again
    /// afterwards would be a second `toString` a page can count, so
    /// `el.setAttribute(a, b)` with an object for both is refused by name
    /// until item 221 keeps the first in one of the slots ADR 0031 gives a
    /// native (built in item 332).
    ASecondArgumentBehindACall,
    /// A regular expression method whose string argument was turned into a
    /// primitive by running script, and which then needs a second call: an
    /// `exec` behind a getter, an `exec` the page replaced, or a `lastIndex`
    /// that is an object (queue item 221).
    ///
    /// The string is in the one slot the second call's answer is written to,
    /// and converting the argument again afterwards would be a second
    /// `toString` a page can count — `ASecondArgumentBehindACall`'s reason,
    /// met by `RegExp.prototype.exec` and `test` (queue item 74).
    ATwoCallRegExpMethod,
    /// A promise made by a constructor other than `Promise` itself — a
    /// subclass, or a `Symbol.species` or `this` naming another constructor
    /// (queue item 337).
    ///
    /// `NewPromiseCapability(C)` for any `C` calls it with an executor of its
    /// own and reads the two functions back, which is a capability this
    /// engine does not keep. `then`, `finally`, `Promise.resolve` and
    /// `Promise.reject` read the constructor the specification says they
    /// read, and refuse here when it is not `Promise`; a value that is not a
    /// constructor at all is still the `TypeError` the specification gives.
    APromiseOfAnotherConstructor,
    /// A date as text, either way (queue item 357): `Date.parse`, `new
    /// Date(string)`, `Date()` called as a function, and `toString`,
    /// `toDateString`, `toTimeString` and `toUTCString`.
    ///
    /// ADR 0036 § 4 decides what they read and write — the language's own
    /// forms, the page's zone being UTC, and nothing older — and item 357
    /// builds them when a frozen page writes or reads a date as text. Until
    /// then each is refused here rather than left to
    /// `Object.prototype.toString`, whose `"[object Date]"` would be a wrong
    /// answer that reads like a right one.
    ADateAsText,
    /// Something an **embedder's** native reached and its embedder has not
    /// built, in the embedder's own words — which name the embedder's queue
    /// item, as every other variant names this engine's.
    ///
    /// The engine never makes one and knows nothing of what it says (ADR
    /// 0013 § 6): a browser's `a.click()` on a link that would navigate is
    /// one (queue item 263), and the words are the browser's.
    InTheEmbedder(&'static str),
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
                "'instanceof' needs `Symbol.hasInstance` and `OrdinaryHasInstance`, which is queue item 224"
            ),
            Missing::AFunctionsSourceText => write!(
                out,
                "a function's own source text, which Function.prototype.toString answers with, is queue item 220"
            ),
            Missing::AnObjectAsALength => write!(
                out,
                "an object assigned to an array's length, which converts it twice, is queue item 226"
            ),
            Missing::AMessageBehindACall => write!(
                out,
                "Error.prototype.toString of a message that is a getter or an object is queue item 228"
            ),
            Missing::AnIteratedValueBehindACall => write!(
                out,
                "an array iterator reading an element or a length through a getter or a conversion is queue item 231"
            ),
            Missing::ASecondArgumentBehindACall => write!(
                out,
                "a second argument converted by running script after the first was is queue item 221"
            ),
            Missing::ATwoCallRegExpMethod => write!(
                out,
                "a regular expression method whose string argument ran script to convert and which then calls script again is queue item 221"
            ),
            Missing::APromiseOfAnotherConstructor => write!(
                out,
                "a promise made by a constructor other than Promise is queue item 337"
            ),
            Missing::ADateAsText => write!(
                out,
                "a date as text — Date.parse, new Date(string), Date() and the toString family — is queue item 357"
            ),
            Missing::InTheEmbedder(what) => out.write_str(what),
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
    /// A builtin was resumed somewhere it never suspended: it read the answer
    /// to a call it had not asked for, or was handed one it had no step for.
    /// Or it reached a slot it never declared, read a slot it wrote as one
    /// kind of value as another, or declared more slots than any builtin may
    /// keep (ADR 0031 §§ 2–4).
    BuiltinIsWrong,
    /// A constructor's `prototype` was not a data property of its own, which
    /// the attributes it was made with forbid: it is not configurable, so
    /// nothing can delete it or turn it into an accessor.
    ConstructorIsWrong,
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
            Internal::BuiltinIsWrong => {
                write!(
                    out,
                    "a builtin was resumed at a step it never asked for, or reached a slot it never kept"
                )
            }
            Internal::ConstructorIsWrong => write!(
                out,
                "a constructor's `prototype` was not the data property it was made as"
            ),
        }
    }
}

/// Every way running a program ends other than with a value.
#[derive(Debug, Clone, PartialEq)]
pub enum Escape {
    /// The script threw. Its own `catch` is what survives this (queue item
    /// 210), and nothing else here is caught.
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
            // No page made this builtin, so no page may catch it (ADR 0031 § 2).
            Refused::KeepsTooMuch { .. } => Self::Broken(Internal::BuiltinIsWrong),
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
    /// reaches (queue item 210).
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
    fn a_builtin_that_keeps_too_much_is_this_engines_mistake() {
        let refused = Escape::refused(Refused::KeepsTooMuch { kept: 9 }, 3);
        assert_eq!(refused, Escape::Broken(Internal::BuiltinIsWrong));
        assert!(!refused.is_the_pages(), "no page made the builtin");
    }

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
                .contains("224")
        );
        assert!(
            Escape::NotBuiltYet(Missing::AWrapperObject)
                .to_string()
                .contains("73")
        );
        assert!(
            Escape::NotBuiltYet(Missing::ASecondArgumentBehindACall)
                .to_string()
                .contains("221")
        );
    }
}
