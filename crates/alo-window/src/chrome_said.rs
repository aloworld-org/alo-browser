/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page's script said while a verb ran through Chrome, in words
//! (queue item 639, `docs/contracts/agent-socket.md`).
//!
//! An answer's `issues` is what the page's script said while the verb ran.
//! Chrome tells of it once asked (`Runtime.enable`, read by
//! [`crate::chrome_protocol::console_called`] and
//! [`crate::chrome_protocol::thrown`]), and this puts each thing it tells
//! of into a line **in the engine's words where the engine has words for
//! it**, so that an agent reading `issues` reads one language whichever
//! renderer drew the page.
//!
//! # The same as the engine's
//!
//! - Each line begins with what it is about — `the click`, or `the text`
//!   for text put into a field — as the engine's does.
//! - A throw nothing caught is `uncaught:` and what was thrown, said by the
//!   engine's rules: an error by its name and message, a string quoted, a
//!   number or `true` as itself, any other object `an object`, a symbol
//!   `a symbol`. A promise rejected and not handled is said as a throw is,
//!   as the engine says it.
//! - Where it was follows — `(at …, line …, column …; called from …)`,
//!   innermost first, at most 32 places, and how many calls were further
//!   out.
//! - At most [`MOST_REPORTS`] lines a verb, each at most
//!   [`LONGEST_LINE`](alo_renderer::said::LONGEST_LINE) characters, and
//!   then how many more there were, in the engine's words.
//!
//! # Where it differs, and why
//!
//! - **A place is named by Chrome's address for the script**: the page's
//!   own address and the line in the page, for a script in its markup,
//!   where the engine names `script 1` and the line in the script. Chrome
//!   does not number a page's scripts, and an address is the truer name for
//!   one it fetched.
//! - **The console is Chrome's alone.** The engine has none (ADR 0013 § 3,
//!   *absent beats approximate*): a page calling `console.log` there throws
//!   `'console' is not defined`. Through Chrome the call is said as
//!   `console.log:` and what it was given — a string as it is, any other
//!   value by Chrome's own description of it — with no format directive
//!   (`%s`, `%c`) carried out.
//! - **An error is said by the first line of Chrome's description**, which
//!   is its name and message as they were when it was made; the engine
//!   reads them when it is thrown.
//! - **A frame from another site and a dedicated worker are the page's
//!   script too** (queue item 652): each is heard from a target of its
//!   own, its lines placed by its own script's address and said as the
//!   page's are. The engine runs no such frame or worker that a verb
//!   reaches, so there is nothing of the engine's to match them to yet.
//! - A line is about the verb, not the task: Chrome says nothing of which
//!   task said it, so a timer the page set earlier that fires while the
//!   verb runs is the verb's too. The engine's lines are what every task it
//!   ran to reach the verb's said, which is the same rule.

use crate::chrome_protocol::{Places, Said, Shown, Words};
use alo_renderer::event_loop::MOST_REPORTS;
use alo_renderer::said;
use core::fmt;

/// What a page's script said while one verb ran, as an answer's `issues`.
#[derive(Debug)]
pub struct Heard {
    /// What each line is about — `"the click"`, say.
    about: &'static str,
    lines: Vec<String>,
    left_out: usize,
}

impl Heard {
    /// Nothing heard yet, about `about`.
    pub const fn about(about: &'static str) -> Self {
        Self {
            about,
            lines: Vec::new(),
            left_out: 0,
        }
    }

    /// Hear `said`, if there is room, or count it.
    pub fn hear(&mut self, said: &Said) {
        self.say(&Line(said));
    }

    /// Hear that the page said something Chrome told of in a message too
    /// long to read, which was skipped.
    pub fn too_long(&mut self) {
        self.say(&format_args!(
            "the page's script said something longer than one message from Chrome may be ({} \
             bytes), and it was not read",
            crate::chrome_wire::MOST_MESSAGE
        ));
    }

    /// Hear that Chrome told of something the page's script said in words
    /// that are not the protocol's.
    pub fn unread(&mut self, why: &dyn fmt::Display) {
        self.say(&format_args!(
            "the page's script said something Chrome told of unreadably: {why}"
        ));
    }

    fn say(&mut self, what: &dyn fmt::Display) {
        if self.lines.len() < MOST_REPORTS {
            let about = self.about;
            self.lines
                .push(said::line(&format_args!("{about}: {what}")));
        } else {
            self.left_out = self.left_out.saturating_add(1);
        }
    }

    /// The lines, and one more saying how many were left out, if any were —
    /// in the engine's words for it.
    pub fn lines(mut self) -> Vec<String> {
        if self.left_out > 0 {
            let (left_out, about) = (self.left_out, self.about);
            self.lines.push(format!(
                "{left_out} more things {about}'s script said were not said: one action says \
                 at most {MOST_REPORTS}"
            ));
        }
        self.lines
    }
}

/// One thing said, as a line without what it is about.
struct Line<'s>(&'s Said);

impl fmt::Display for Line<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Said::Console {
                kind,
                args,
                more_args,
                at,
            } => {
                write!(f, "console.{}:", method_of(kind))?;
                for arg in args {
                    write!(f, " {}", Given(arg))?;
                }
                if *more_args > 0 {
                    write!(f, " … and {more_args} more values")?;
                }
                said_where(f, at)
            }
            Said::Thrown { what, text, at } => {
                match what {
                    Some(what) => write!(f, "uncaught: {}", Thrown(what))?,
                    None => write!(f, "uncaught: {text}")?,
                }
                said_where(f, at)
            }
        }
    }
}

/// The console's method that Chrome's `type` names: Chrome tells of
/// `console.warn` as `warning` and of the group calls by other words.
fn method_of(kind: &str) -> &str {
    match kind {
        "warning" => "warn",
        "startGroup" => "group",
        "startGroupCollapsed" => "groupCollapsed",
        "endGroup" => "groupEnd",
        other => other,
    }
}

/// A value given to the console: a string as it is, as a console shows
/// it, and anything else by Chrome's description.
struct Given<'s>(&'s Shown);

impl fmt::Display for Given<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Shown::Undefined => f.write_str("undefined"),
            Shown::Null => f.write_str("null"),
            Shown::Bool(is) => write!(f, "{is}"),
            Shown::Number(words)
            | Shown::BigInt(words)
            | Shown::Text(words)
            | Shown::Symbol(words)
            | Shown::Error(words)
            | Shown::Object(words) => write!(f, "{}", Cut(words)),
        }
    }
}

/// A value thrown, by the engine's rules (`alo-renderer`'s `described`).
struct Thrown<'s>(&'s Shown);

impl fmt::Display for Thrown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Shown::Undefined => f.write_str("undefined"),
            Shown::Null => f.write_str("null"),
            Shown::Bool(is) => write!(f, "{is}"),
            Shown::Number(words) | Shown::BigInt(words) | Shown::Error(words) => {
                write!(f, "{}", Cut(words))
            }
            Shown::Text(words) => {
                write!(f, "{:?}", words.kept)?;
                more(f, words.more)
            }
            Shown::Symbol(_) => f.write_str("a symbol"),
            Shown::Object(_) => f.write_str("an object"),
        }
    }
}

/// A page's text as it is, and how much more there was.
struct Cut<'s>(&'s Words);

impl fmt::Display for Cut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.kept)?;
        more(f, self.0.more)
    }
}

/// `… and N more characters`, for text cut short.
fn more(f: &mut fmt::Formatter<'_>, more: usize) -> fmt::Result {
    if more > 0 {
        write!(f, "… and {more} more characters")?;
    }
    Ok(())
}

/// ` (at …; called from …)`, or nothing for a place nobody gave — in the
/// engine's words for a trace.
fn said_where(f: &mut fmt::Formatter<'_>, at: &Places) -> fmt::Result {
    for (which, place) in at.places.iter().enumerate() {
        f.write_str(if which == 0 {
            " (at "
        } else {
            "; called from "
        })?;
        if place.url.kept.is_empty() {
            f.write_str("a script with no address")?;
        } else {
            write!(f, "{}", Cut(&place.url))?;
        }
        write!(f, ", line {}, column {}", place.line, place.column)?;
    }
    if at.left_out > 0 {
        write!(f, "; and {} calls further out", at.left_out)?;
    }
    if at.places.is_empty() {
        Ok(())
    } else {
        f.write_str(")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome_protocol::{self, Place};

    fn words(text: &str) -> Words {
        Words {
            kept: text.to_owned(),
            more: 0,
        }
    }

    fn at(line: u64, column: u64) -> Places {
        Places {
            places: vec![Place {
                url: words("https://example.test/page"),
                line,
                column,
            }],
            left_out: 0,
        }
    }

    fn heard(said: &[Said]) -> Vec<String> {
        let mut heard = Heard::about("the click");
        for said in said {
            heard.hear(said);
        }
        heard.lines()
    }

    #[test]
    fn a_throw_is_said_by_the_engines_rules_where_chrome_places_it() {
        let thrown = |what| Said::Thrown {
            text: "Uncaught".to_owned(),
            what: Some(what),
            at: at(5, 149),
        };
        assert_eq!(
            heard(&[
                thrown(Shown::Error(words("Error: the first listener"))),
                thrown(Shown::Number(words("4"))),
                thrown(Shown::Text(words("once"))),
                thrown(Shown::Object(words("Object"))),
                thrown(Shown::Symbol(words("Symbol(x)"))),
                thrown(Shown::Null),
                thrown(Shown::Bool(false)),
                thrown(Shown::Undefined),
            ]),
            [
                "the click: uncaught: Error: the first listener (at https://example.test/page, \
                 line 5, column 149)",
                "the click: uncaught: 4 (at https://example.test/page, line 5, column 149)",
                "the click: uncaught: \"once\" (at https://example.test/page, line 5, column 149)",
                "the click: uncaught: an object (at https://example.test/page, line 5, column 149)",
                "the click: uncaught: a symbol (at https://example.test/page, line 5, column 149)",
                "the click: uncaught: null (at https://example.test/page, line 5, column 149)",
                "the click: uncaught: false (at https://example.test/page, line 5, column 149)",
                "the click: uncaught: undefined (at https://example.test/page, line 5, column \
                 149)",
            ]
        );
    }

    #[test]
    fn a_console_call_is_said_by_its_method_and_what_it_was_given() {
        let console = |kind: &str, args| Said::Console {
            kind: kind.to_owned(),
            args,
            more_args: 0,
            at: at(3, 9),
        };
        assert_eq!(
            heard(&[
                console(
                    "log",
                    vec![
                        Shown::Text(words("clicked")),
                        Shown::Number(words("4")),
                        Shown::Object(words("Array(2)")),
                        Shown::Null,
                        Shown::Undefined,
                        Shown::Bool(true),
                    ]
                ),
                console("warning", vec![Shown::Text(words("w"))]),
                console("startGroup", Vec::new()),
            ]),
            [
                "the click: console.log: clicked 4 Array(2) null undefined true (at \
                 https://example.test/page, line 3, column 9)",
                "the click: console.warn: w (at https://example.test/page, line 3, column 9)",
                "the click: console.group: (at https://example.test/page, line 3, column 9)",
            ]
        );
    }

    #[test]
    fn what_was_cut_short_or_left_out_is_counted() {
        let long = Said::Console {
            kind: "log".to_owned(),
            args: vec![Shown::Text(Words {
                kept: "x".to_owned(),
                more: 7,
            })],
            more_args: 3,
            at: Places {
                places: vec![Place {
                    url: words(""),
                    line: 1,
                    column: 2,
                }],
                left_out: 31,
            },
        };
        assert_eq!(
            heard(&[long]),
            [
                "the click: console.log: x… and 7 more characters … and 3 more values (at a \
              script with no address, line 1, column 2; and 31 calls further out)"
            ]
        );
        let mut heard = Heard::about("the text");
        let said = Said::Thrown {
            text: "Uncaught".to_owned(),
            what: None,
            at: Places::default(),
        };
        for _ in 0..MOST_REPORTS + 2 {
            heard.hear(&said);
        }
        heard.too_long();
        let lines = heard.lines();
        assert_eq!(lines.len(), MOST_REPORTS + 1);
        assert_eq!(
            lines.first().map(String::as_str),
            Some("the text: uncaught: Uncaught")
        );
        assert_eq!(
            lines.last().map(String::as_str),
            Some(
                "3 more things the text's script said were not said: one action says at most \
                 256"
            )
        );
    }

    #[test]
    fn chromes_own_messages_read_as_the_lines_they_say() {
        // Chrome 155's own words, measured 2026-10-25, cut to what is read.
        let console = br#"{"type":"log","args":[{"type":"string","value":"clicked"},
            {"type":"number","value":4,"description":"4"},
            {"type":"object","className":"Object","description":"Object","objectId":"1.2.1",
             "preview":{"type":"object","description":"Object","overflow":false,
             "properties":[{"name":"a","type":"number","value":"1"}]}}],
            "executionContextId":2,"timestamp":1792917213324.373,
            "stackTrace":{"callFrames":[{"functionName":"","scriptId":"3",
            "url":"http://127.0.0.1:55902/page.html","lineNumber":4,"columnNumber":44}]}}"#;
        let thrown = br#"{"timestamp":1792917213325.844,"exceptionDetails":{"exceptionId":1,
            "text":"Uncaught","lineNumber":4,"columnNumber":148,"scriptId":"3",
            "url":"http://127.0.0.1:55902/page.html","stackTrace":{"callFrames":[
            {"functionName":"","scriptId":"3","url":"http://127.0.0.1:55902/page.html",
            "lineNumber":4,"columnNumber":148}]},"exception":{"type":"object",
            "subtype":"error","className":"Error","description":"Error: the first listener\n    at HTMLButtonElement.<anonymous> (http://127.0.0.1:55902/page.html:5:149)",
            "objectId":"1.2.3"},"executionContextId":2}}"#;
        let unparsed = br#"{"timestamp":1,"exceptionDetails":{"exceptionId":1,"text":"Uncaught",
            "lineNumber":5,"columnNumber":54,"scriptId":"3","url":"http://127.0.0.1:1/p",
            "exception":{"type":"object","subtype":"error","className":"SyntaxError",
            "description":"SyntaxError: missing ) after argument list"}}}"#;
        let said = [
            chrome_protocol::console_called(console).unwrap(),
            chrome_protocol::thrown(thrown).unwrap(),
            chrome_protocol::thrown(unparsed).unwrap(),
        ];
        assert_eq!(
            heard(&said),
            [
                "the click: console.log: clicked 4 Object (at http://127.0.0.1:55902/page.html, \
                 line 5, column 45)",
                "the click: uncaught: Error: the first listener (at \
                 http://127.0.0.1:55902/page.html, line 5, column 149)",
                "the click: uncaught: SyntaxError: missing ) after argument list (at \
                 http://127.0.0.1:1/p, line 6, column 55)",
            ]
        );
    }
}
