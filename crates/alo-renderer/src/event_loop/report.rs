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
//! string a page made cut short. **Where** it was thrown follows it (queue
//! item 241): the script, line and column of the throw, then of each call it
//! left on its way out, innermost first — see [`source`](super::source) for how an offset
//! becomes a line and a column, and why a column.

use core::fmt;

use alo_js::abrupt::Thrown;
use alo_js::object::Objects;

use super::described;
use super::source::{Place, Trace};

/// Script that did not finish, and why, in words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// It threw, and nothing caught it.
    Threw {
        /// What was thrown, in words.
        what: String,
        /// Where: the throw, then each call it left.
        trace: Trace,
    },
    /// A script's text is not a script.
    NotParsed(String),
    /// A script uses something this engine has not built, so none of it ran.
    NotCompiled(String),
}

impl Report {
    /// A throw nothing caught, described while what was thrown is still alive,
    /// and where it was.
    pub fn thrown(objects: &Objects, thrown: &Thrown, trace: Trace) -> Self {
        Report::Threw {
            what: match thrown {
                Thrown::Error { kind, message, .. } => format!("{}: {message}", kind.name()),
                Thrown::Value { value, .. } => described::thrown(objects, *value),
            },
            trace,
        }
    }
}

impl fmt::Display for Report {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Report::Threw { what, trace } => {
                write!(out, "uncaught: {what}")?;
                said_where(out, trace)
            }
            Report::NotParsed(why) => write!(out, "not a script: {why}"),
            Report::NotCompiled(why) => write!(out, "not run: {why}"),
        }
    }
}

/// ` (at …; called from …)`, or nothing for a throw no call was placed for.
fn said_where(out: &mut fmt::Formatter<'_>, trace: &Trace) -> fmt::Result {
    for (which, place) in trace.places.iter().enumerate() {
        out.write_str(if which == 0 {
            " (at "
        } else {
            "; called from "
        })?;
        match place {
            Place::Script { name, line, column } => {
                write!(out, "{name}, line {line}, column {column}")?;
            }
            Place::Elsewhere { at } => {
                write!(out, "a program this page did not run, byte {at}")?;
            }
        }
    }
    if trace.left_out > 0 {
        write!(out, "; and {} calls further out", trace.left_out)?;
    }
    if trace.places.is_empty() {
        Ok(())
    } else {
        out.write_str(")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_js::object::Value;

    #[test]
    fn a_report_says_which_of_the_three_it_is() {
        assert_eq!(
            Report::Threw {
                what: "TypeError: x".to_owned(),
                trace: Trace::default(),
            }
            .to_string(),
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
        let thrown = |value| {
            Report::thrown(&objects, &Thrown::Value { value, at: 0 }, Trace::default()).to_string()
        };
        assert_eq!(thrown(Value::Number(4.0)), "uncaught: 4");
        assert_eq!(thrown(Value::Null), "uncaught: null");
        assert_eq!(thrown(Value::Bool(false)), "uncaught: false");
    }

    #[test]
    fn a_trace_is_said_innermost_first_with_what_was_left_out() {
        let place = |name: &str, line, column| Place::Script {
            name: name.to_owned(),
            line,
            column,
        };
        let report = Report::Threw {
            what: "Error: e".to_owned(),
            trace: Trace {
                places: vec![
                    place("script 2", 1, 15),
                    place("script 1", 3, 1),
                    Place::Elsewhere { at: 7 },
                ],
                left_out: 4,
            },
        };
        assert_eq!(
            report.to_string(),
            "uncaught: Error: e (at script 2, line 1, column 15; called from script 1, \
             line 3, column 1; called from a program this page did not run, byte 7; and 4 \
             calls further out)"
        );
    }
}
