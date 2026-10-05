/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the loop says about script that did not finish: HTML's *report the
//! exception*, in words.
//!
//! A throw a page did not catch is reported and the loop runs on — the next
//! callback, the next job, the next task — as every browser does. The words
//! are made **as it happens**, while what was thrown is still somewhere the
//! collector can see: a thrown value is a reference nothing roots once its
//! script has gone.
//!
//! Two more things are reported rather than ending the page, because each
//! happens before anything runs and so leaves nothing half done: a script
//! that does not parse — the `SyntaxError` a page would see — and one this
//! engine will not compile because it uses something not built yet (ADR 0013
//! § 3, *absent beats approximate*).
//!
//! What was thrown is put into words by [`described`], which reads the heap
//! and runs nothing: an error object by its `name` and `message`, and any
//! string a page made cut short.

use core::fmt;

use alo_js::abrupt::Thrown;
use alo_js::object::Objects;

use super::described;

/// Script that did not finish, and why, in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// It threw, and nothing caught it.
    Threw(String),
    /// A script's text is not a script.
    NotParsed(String),
    /// A script uses something this engine has not built, so none of it ran.
    NotCompiled(String),
}

impl Report {
    /// A throw nothing caught, described while what was thrown is still alive.
    pub fn thrown(objects: &Objects, thrown: &Thrown) -> Self {
        Report::Threw(match thrown {
            Thrown::Error { kind, message, .. } => format!("{}: {message}", kind.name()),
            Thrown::Value { value, .. } => described::thrown(objects, *value),
        })
    }
}

impl fmt::Display for Report {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Report::Threw(what) => write!(out, "uncaught: {what}"),
            Report::NotParsed(why) => write!(out, "not a script: {why}"),
            Report::NotCompiled(why) => write!(out, "not run: {why}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_js::object::Value;

    #[test]
    fn a_report_says_which_of_the_three_it_is() {
        assert_eq!(
            Report::Threw("TypeError: x".to_owned()).to_string(),
            "uncaught: TypeError: x"
        );
        assert_eq!(
            Report::NotParsed("y".to_owned()).to_string(),
            "not a script: y"
        );
        assert_eq!(
            Report::NotCompiled("z".to_owned()).to_string(),
            "not run: z"
        );
    }

    #[test]
    fn a_thrown_primitive_is_written_as_itself() {
        let objects = Objects::new();
        let thrown = |value| Report::thrown(&objects, &Thrown::Value { value, at: 0 });
        assert_eq!(thrown(Value::Number(4.0)), Report::Threw("4".to_owned()));
        assert_eq!(thrown(Value::Null), Report::Threw("null".to_owned()));
        assert_eq!(
            thrown(Value::Bool(false)),
            Report::Threw("false".to_owned())
        );
    }
}
