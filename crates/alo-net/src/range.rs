//! Byte ranges: asking for part of something, and reading what came back.
//!
//! # Why the grammar is its own file
//!
//! `Content-Range` is three numbers in a string, and every one of them is a
//! number a stranger chose. What is done with them is worse than usual: they
//! decide **where in a file the bytes that follow are written**. A parser that
//! is generous here does not render something wrong — it splices the middle of
//! a download into the wrong offset and hands up a file that is the right
//! length and is not the thing.
//!
//! So the reading of the two headers is here, with nothing else in it, and the
//! conversation that uses them is [`crate::download`].
//!
//! # What is refused, and why each one
//!
//! - **A unit that is not `bytes`.** The standard allows other units and
//!   nobody has ever defined one. A range in a unit we do not know is a range
//!   we cannot place.
//! - **The unsatisfied form**, `bytes */1234`. It is what a `416` says, and it
//!   describes what was *not* sent. Read as a description of bytes that were,
//!   it is a range starting nowhere.
//! - **A first byte after the last one.** `bytes 9-4/10` is not a range, and
//!   the subtraction that would give its length underflows.
//! - **A last byte at or past the end.** Positions count from zero, so
//!   `bytes 0-10/10` claims eleven bytes of a ten-byte thing.
//! - **Anything that is not digits**, including a sign. `bytes -1-5/10` reads
//!   as a negative start to a person and as nonsense to a parser, and the two
//!   readings must not both be available.

use crate::headers::Headers;
use crate::http::{Malformed, shorten};

/// The `Range` header for everything from here to the end.
///
/// Only the open-ended form, because that is the only one a resuming download
/// asks: it wants the rest, and it does not know how much rest there is.
pub fn from_here_on(first: u64) -> String {
    format!("bytes={first}-")
}

/// Which bytes a `206` says it is carrying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sent {
    /// The first byte's position, counting from zero.
    pub first: u64,
    /// The last byte's position, which is *included*.
    pub last: u64,
    /// How long the whole thing is, when the server said. `None` for the `*`
    /// form, which is a server that is sending a range of something whose full
    /// length it does not know.
    pub complete: Option<u64>,
}

impl Sent {
    /// How many bytes this range is.
    ///
    /// Never underflows: [`what_was_sent`] refuses a first byte after the last
    /// one, so the subtraction here is on numbers that have already been
    /// checked against each other.
    pub fn count(self) -> u64 {
        self.last.saturating_sub(self.first).saturating_add(1)
    }

    /// Whether this range reaches the end of the whole thing.
    ///
    /// `None` when nobody said how long the whole thing is, which is not the
    /// same as `false` — it is "there is no way to tell from this header".
    pub fn reaches_the_end(self) -> Option<bool> {
        self.complete
            .map(|complete| self.last.saturating_add(1) >= complete)
    }
}

/// What a response's `Content-Range` says, if it says anything.
///
/// # Errors
///
/// [`Malformed`] for every reading in this file's own note, and for a response
/// carrying two `Content-Range` headers — two answers to "where do these bytes
/// go" is not a question this engine picks between.
pub fn what_was_sent(headers: &Headers) -> Result<Option<Sent>, Malformed> {
    let mut lines = headers.all("Content-Range");
    let Some(line) = lines.next() else {
        return Ok(None);
    };
    if lines.next().is_some() {
        return Err(Malformed {
            why: "more than one Content-Range, which is two answers to where \
                  these bytes go"
                .to_owned(),
        });
    }
    parse(line).map(Some)
}

/// One `Content-Range` value: `bytes 0-499/1234`.
///
/// # Errors
///
/// [`Malformed`], in words, for anything this engine will not place bytes by.
pub fn parse(value: &str) -> Result<Sent, Malformed> {
    let refuse = |why: String| Malformed { why };
    let value = value.trim();
    let Some((unit, span)) = value.split_once(' ') else {
        return Err(refuse(format!(
            "a Content-Range with no unit in it: {:?}",
            shorten(value)
        )));
    };
    if !unit.eq_ignore_ascii_case("bytes") {
        return Err(refuse(format!(
            "a Content-Range in {:?}, which is not bytes",
            shorten(unit)
        )));
    }
    let span = span.trim();
    let Some((positions, complete)) = span.split_once('/') else {
        return Err(refuse(format!(
            "a Content-Range that does not say what it is part of: {:?}",
            shorten(span)
        )));
    };
    if positions.trim() == "*" {
        // The form a `416` carries. It says which bytes were *not* sent, and
        // reading it as a range of bytes that were is how a download writes a
        // refusal into the middle of a file.
        return Err(refuse(
            "a Content-Range that says which bytes were not sent".to_owned(),
        ));
    }
    let Some((first, last)) = positions.split_once('-') else {
        return Err(refuse(format!(
            "a Content-Range with no range in it: {:?}",
            shorten(positions)
        )));
    };
    let first = number(first, "the first byte")?;
    let last = number(last, "the last byte")?;
    if first > last {
        return Err(refuse(format!(
            "a Content-Range whose first byte {first} is after its last byte {last}"
        )));
    }
    let complete = complete.trim();
    let complete = if complete == "*" {
        None
    } else {
        let complete = number(complete, "the length of the whole thing")?;
        if last >= complete {
            // Positions count from zero, so the last byte of a ten-byte thing
            // is nine. A server saying otherwise is describing bytes that
            // cannot exist, and appending them would make the download longer
            // than the thing it is of.
            return Err(refuse(format!(
                "a Content-Range whose last byte {last} is past the end of the \
                 {complete} bytes it says the whole thing is"
            )));
        }
        Some(complete)
    };
    Ok(Sent {
        first,
        last,
        complete,
    })
}

/// One number, digits only.
///
/// `u64::from_str_radix` would take a leading `+`, and `parse` takes a leading
/// `-` into a failure rather than into a refusal somebody can read. Both are
/// checked here so the error says which number was wrong.
fn number(text: &str, which: &str) -> Result<u64, Malformed> {
    let text = text.trim();
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Malformed {
            why: format!(
                "{which} of a Content-Range is not a number: {:?}",
                shorten(text)
            ),
        });
    }
    text.parse().map_err(|_| Malformed {
        why: format!("{which} of a Content-Range does not fit in a number"),
    })
}

/// Whether a server has said it will not answer a range request.
///
/// The absent header is **not** a refusal: a great many servers answer ranges
/// without ever advertising it, and treating silence as no would mean never
/// resuming from them. `Accept-Ranges: none` is a server saying so outright,
/// and that is honoured — asking anyway would spend a round trip to be told.
pub fn will_take_a_range(headers: &Headers) -> bool {
    !headers
        .all("Accept-Ranges")
        .any(|held| held.trim().eq_ignore_ascii_case("none"))
}

#[cfg(test)]
mod tests {
    use super::{Sent, from_here_on, parse, what_was_sent, will_take_a_range};
    use crate::headers::Headers;

    #[test]
    fn a_resumed_request_asks_for_the_rest_rather_than_for_a_size() {
        // A download knows where it stopped and not how much is left.
        assert_eq!(from_here_on(0), "bytes=0-");
        assert_eq!(from_here_on(4096), "bytes=4096-");
    }

    #[test]
    fn the_ordinary_answer_reads_as_three_numbers() {
        let sent = parse("bytes 0-499/1234").expect("the ordinary form");
        assert_eq!(
            sent,
            Sent {
                first: 0,
                last: 499,
                complete: Some(1234),
            }
        );
        assert_eq!(sent.count(), 500, "the last byte is included");
        assert_eq!(sent.reaches_the_end(), Some(false));
    }

    #[test]
    fn the_last_range_of_a_thing_is_the_one_that_reaches_its_end() {
        let sent = parse("bytes 1000-1233/1234").expect("the last range");
        assert_eq!(sent.reaches_the_end(), Some(true));
        assert_eq!(sent.count(), 234);
    }

    #[test]
    fn a_server_that_does_not_know_the_whole_length_still_places_its_bytes() {
        let sent = parse("bytes 100-199/*").expect("a length nobody knows");
        assert_eq!(sent.complete, None);
        assert_eq!(
            sent.reaches_the_end(),
            None,
            "not false — there is no way to tell from this header"
        );
    }

    #[test]
    fn the_unit_is_read_however_it_was_capitalised() {
        assert_eq!(parse("BYTES 0-1/2"), parse("bytes 0-1/2"));
    }

    /// The refusal, or the reason a test wanted one and did not get it.
    fn refusal(value: &str) -> String {
        match parse(value) {
            Err(refused) => refused.why,
            Ok(sent) => format!("accepted, as {sent:?}"),
        }
    }

    #[test]
    fn every_range_that_could_be_placed_wrongly_is_refused() {
        for (value, expected) in [
            ("items 0-9/10", "not bytes"),
            ("bytes */1234", "were not sent"),
            ("bytes 9-4/10", "is after its last byte"),
            ("bytes 0-10/10", "past the end"),
            ("bytes 10-10/10", "past the end"),
            ("bytes -1-5/10", "not a number"),
            ("bytes +1-5/10", "not a number"),
            ("bytes 0-/10", "not a number"),
            ("bytes -/10", "not a number"),
            ("bytes 0-9", "does not say what it is part of"),
            ("bytes 0/10", "no range in it"),
            ("0-9/10", "no unit in it"),
            ("", "no unit in it"),
            ("bytes 0-9/1e3", "not a number"),
            ("bytes 0-9/-1", "not a number"),
            (
                "bytes 0-99999999999999999999999999/*",
                "does not fit in a number",
            ),
        ] {
            let why = refusal(value);
            assert!(why.contains(expected), "{value:?} gave {why:?}");
        }
    }

    #[test]
    fn the_largest_numbers_there_are_do_not_overflow_anything() {
        let sent = parse("bytes 0-18446744073709551614/18446744073709551615")
            .expect("the biggest range a u64 can describe");
        assert_eq!(sent.count(), u64::MAX);
        assert_eq!(sent.reaches_the_end(), Some(true));
    }

    #[test]
    fn nothing_a_server_can_write_makes_this_panic() {
        // Not an assertion about what each means — an assertion that reading
        // bytes a stranger chose is a result rather than a crash.
        for value in [
            "bytes",
            "bytes ",
            "bytes /",
            "bytes -",
            "bytes 0-1/",
            "bytes 0--1/2",
            "bytes 0-1/2/3",
            "bytes\t0-1/2",
            "bytes 0-1/2 extra",
            "\u{1f600} 0-1/2",
            "bytes 0-1/\u{1f600}",
            &"9".repeat(4096),
            &format!("bytes 0-1/{}", "9".repeat(4096)),
        ] {
            let _ = parse(value);
        }
    }

    #[test]
    fn an_error_does_not_carry_a_whole_header_into_a_message() {
        let why = refusal(&format!("{} 0-1/2", "x".repeat(4096)));
        assert!(why.len() < 200, "an error {} bytes long", why.len());
    }

    #[test]
    fn two_content_ranges_are_two_answers_and_neither_is_taken() {
        let mut headers = Headers::new();
        headers.add("Content-Range", "bytes 0-9/20");
        headers.add("Content-Range", "bytes 10-19/20");
        let why = match what_was_sent(&headers) {
            Err(refused) => refused.why,
            Ok(sent) => format!("accepted, as {sent:?}"),
        };
        assert!(why.contains("more than one"), "{why:?}");
    }

    #[test]
    fn a_response_that_says_nothing_about_a_range_is_not_an_error() {
        assert_eq!(what_was_sent(&Headers::new()), Ok(None));
    }

    #[test]
    fn only_a_server_that_says_none_is_taken_to_mean_none() {
        let saying = |value: &str| {
            let mut headers = Headers::new();
            headers.add("Accept-Ranges", value);
            headers
        };
        assert!(
            will_take_a_range(&Headers::new()),
            "silence is not a refusal: most servers never advertise it"
        );
        assert!(will_take_a_range(&saying("bytes")));
        assert!(!will_take_a_range(&saying("none")));
        assert!(!will_take_a_range(&saying("None")));
    }
}
