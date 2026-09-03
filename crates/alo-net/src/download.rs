//! Fetching the whole of something, and asking for the rest when it stops.
//!
//! A page that arrives half way is an error ([`crate::body`]). A *file* that
//! arrives half way is a hundred megabytes somebody already waited for, and
//! throwing them away to start again is the difference between a browser
//! somebody downloads with and one they do not.
//!
//! # The deciding is a pure function, deliberately
//!
//! Everything in this file works on a [`Response`] that is already in memory.
//! There is no socket here and no pool: [`Download::asking`] says what to ask
//! for next and [`Download::take`] says what an answer means, and both can be
//! driven from a table. That is the same shape [`crate::redirect`] has, for the
//! same reason — every rule below is a rule about *placing bytes at an offset*,
//! and a rule like that is asserted honestly only when nothing else is moving.
//!
//! [`crate::Pool::download`] is the loop that does the I/O, and it is short
//! because none of the deciding is in it.
//!
//! # The four rules, and what each one is protecting
//!
//! - **A resume needs a validator.** `If-Range` is what lets a server say "the
//!   thing you have half of is not the thing I have any more" — and without an
//!   `ETag` or a `Last-Modified` there is nothing to say it with. So a download
//!   of something that carries neither **starts again** rather than resuming.
//!   Splicing the middle of one file onto the start of another produces a file
//!   of exactly the right length that is not the thing, and nothing downstream
//!   could ever notice.
//! - **A `200` answering a range request is never appended.** It is byte zero
//!   onwards, whatever we asked for. A server that ignores `Range` is common
//!   and is not misbehaving; a client that appended its answer would corrupt
//!   every download from it. So the bytes so far are dropped and it starts
//!   again — and [`Download::restarts`] counts it, because a server that does
//!   this every time must run out of attempts rather than loop.
//! - **A `206` must start exactly where we stopped.** `Content-Range` is
//!   checked against the length we hold, not trusted to be the answer to what
//!   we asked. An off-by-one there is a byte inserted into the middle of a
//!   file.
//! - **Nothing coded is spliced.** A download asks for `identity` from its
//!   first request, because a byte range of a compressed stream is a range
//!   nobody can decompress. A `206` carrying a `Content-Encoding` is refused
//!   outright: its offsets are offsets into the *coded* representation, and the
//!   bytes we already hold are not.

use crate::headers::Headers;
use crate::range::{self, Sent};
use crate::request::Request;
use crate::response::{Response, Status};
use alo_url::Url;
use core::fmt;

/// How many answers one download will fold in before giving up.
///
/// A bound rather than a limit somebody hit. Every kind of answer that does not
/// finish a download — a body that stops, a server that ignores `Range`, a
/// server that sends nothing at all — costs one of these, so a server that
/// never makes progress runs out rather than being asked for ever.
const MOST_ANSWERS: usize = 8;

/// The most bytes one download will hold.
///
/// The same bound a single response has, for the same reason and because a
/// download *is* one response as far as everything above it is concerned. It is
/// counted across the whole download rather than per answer, which is the point:
/// eight answers of the per-answer bound would be eight times the bound.
pub const LARGEST: u64 = crate::body::LARGEST_BODY;

/// What to do after an answer has been folded in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The whole thing has arrived.
    Done,
    /// It stopped short of the whole thing. Ask again.
    More,
}

/// Why an answer could not be made part of a download.
///
/// Named rather than collapsed into a string, for [`crate::redirect::Refusal`]'s
/// reason: "the file changed while you were downloading it" and "this server
/// will not resume" are different things to tell somebody, and only one of them
/// is worth offering to start again after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unusable {
    /// A range that begins somewhere other than where this download stopped.
    AtTheWrongOffset {
        /// Where the next byte goes.
        wanted: u64,
        /// Where the server says its bytes go.
        sent: u64,
    },
    /// A `206` with nothing saying which bytes it is carrying.
    NoContentRange,
    /// A `Content-Range` this engine will not place bytes by.
    ARangeWeCannotPlace {
        /// Which reading it was refused for, from [`crate::range`].
        why: String,
    },
    /// More bytes than the range they claim to be.
    MoreThanItSaid {
        /// How long the range said it was.
        said: u64,
        /// How many bytes came.
        sent: u64,
    },
    /// The whole thing is a different length than it was, so it is a different
    /// thing and what we hold is half of something that no longer exists.
    ItChanged {
        /// What the length was.
        was: u64,
        /// What it is now.
        now: u64,
    },
    /// A coding was applied to a piece, so the pieces cannot be joined.
    Coded {
        /// Which one.
        coding: String,
    },
    /// The body stopped short and the server has said it will not take a range.
    WillNotResume,
    /// More than this engine will hold.
    TooLong {
        /// How many bytes it would have been.
        bytes: u64,
    },
    /// The answers ran out before the download did.
    NoProgress {
        /// How many were taken.
        answers: usize,
    },
    /// A status a download can do nothing with.
    NotAnAnswer {
        /// What came back.
        status: Status,
    },
}

impl fmt::Display for Unusable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unusable::AtTheWrongOffset { wanted, sent } => write!(
                f,
                "the server sent the bytes from {sent} when the download stopped at {wanted}"
            ),
            Unusable::NoContentRange => {
                f.write_str("the server sent a range without saying which bytes it is")
            }
            Unusable::ARangeWeCannotPlace { why } => {
                write!(
                    f,
                    "the server sent a range this browser cannot place: {why}"
                )
            }
            Unusable::MoreThanItSaid { said, sent } => {
                write!(f, "the server sent {sent} bytes as a range of {said} bytes")
            }
            Unusable::ItChanged { was, now } => write!(
                f,
                "what is being downloaded changed while it was downloading: it was \
                 {was} bytes and is now {now}"
            ),
            Unusable::Coded { coding } => write!(
                f,
                "the server sent part of this {coding}-encoded, and parts of an \
                 encoded thing cannot be joined"
            ),
            Unusable::WillNotResume => {
                f.write_str("the download stopped early and this server will not resume one")
            }
            Unusable::TooLong { bytes } => write!(
                f,
                "a download of {bytes} bytes, which is more than this browser holds"
            ),
            Unusable::NoProgress { answers } => write!(
                f,
                "the download did not finish in {answers} attempts and was not getting closer"
            ),
            Unusable::NotAnAnswer { status } => {
                write!(f, "the server answered {status}, which is not a download")
            }
        }
    }
}

impl std::error::Error for Unusable {}

/// A download in progress.
///
/// Holds the bytes so far and everything needed to ask for the rest of them.
/// Nothing in here reads a clock or a socket.
#[derive(Debug, Clone, Default)]
pub struct Download {
    /// What has arrived, in order.
    got: Vec<u8>,
    /// The headers of the answer these bytes started with — always a `200`,
    /// because a `206` is only ever taken as a continuation of one.
    ///
    /// Kept so that the finished download has the head of the *representation*
    /// rather than of its last piece, whose `Content-Length` describes the
    /// piece and whose `Content-Range` would be a lie about the whole.
    head: Option<Headers>,
    /// How long the whole thing is, once a server has said. `None` while
    /// nobody has, which is not the same as zero.
    whole: Option<u64>,
    /// What to send as `If-Range`, from the first answer's `ETag` or
    /// `Last-Modified`. `None` means this download cannot resume and must
    /// start again instead — see this file's own note.
    validator: Option<String>,
    /// Whether the server has said outright that it will not take a range.
    resumable: bool,
    /// How many answers have been folded in.
    answers: usize,
    /// How many times a server answered a range request with the whole thing.
    restarts: usize,
}

impl Download {
    /// One that has nothing yet.
    pub fn new() -> Self {
        Self {
            resumable: true,
            ..Self::default()
        }
    }

    /// The bytes so far.
    pub fn bytes(&self) -> &[u8] {
        &self.got
    }

    /// How long the whole thing is, when a server has said.
    pub fn whole(&self) -> Option<u64> {
        self.whole
    }

    /// How many times this download was started again because a server
    /// answered a range request with the whole thing.
    ///
    /// Public because that is what "noticed rather than believed" means: it is
    /// not enough to refuse to append the bytes, somebody has to be able to see
    /// that it happened.
    pub fn restarts(&self) -> usize {
        self.restarts
    }

    /// How many answers have been folded in.
    pub fn answers(&self) -> usize {
        self.answers
    }

    /// The request to send next.
    ///
    /// `Accept-Encoding: identity` from the very first ask, because a byte
    /// range of a compressed stream is a range nobody can decompress and a
    /// first piece that is gzip beside a second that is not is worse still.
    /// [`crate::http::write_request`] leaves a caller's `Accept-Encoding` alone
    /// for exactly this.
    ///
    /// `Range` and `If-Range` only once there is something to continue *and*
    /// something to continue it against.
    pub fn asking(&self, request: &Request) -> Request {
        let mut asking = request.clone();
        asking.headers.replace("Accept-Encoding", "identity");
        if let Some(validator) = self.resuming() {
            asking
                .headers
                .replace("Range", &range::from_here_on(self.so_far()));
            asking.headers.replace("If-Range", validator);
        }
        asking
    }

    /// The validator to resume against, when there is anything to resume.
    ///
    /// Both halves in one place because [`Download::asking`] and
    /// [`Download::take`] have to agree about whether a range was asked for:
    /// two spellings of this condition is how a `206` gets read as an answer to
    /// a question nobody put.
    fn resuming(&self) -> Option<&str> {
        if self.got.is_empty() {
            return None;
        }
        self.validator.as_deref()
    }

    /// How many bytes are held.
    fn so_far(&self) -> u64 {
        u64::try_from(self.got.len()).unwrap_or(u64::MAX)
    }

    /// Fold an answer into the download.
    ///
    /// `ended_early` is [`crate::connection::Exchanged::short`] — whether the
    /// body stopped before its framing said it would.
    ///
    /// # Errors
    ///
    /// [`Unusable`] for every answer this engine will not place bytes by, each
    /// of them one of the four rules in this file's own note.
    pub fn take(&mut self, response: &Response, ended_early: bool) -> Result<Step, Unusable> {
        self.answers += 1;
        if self.answers > MOST_ANSWERS {
            // The backstop, for a caller that kept going after being told to
            // stop. The bound that actually ends a download is at the bottom.
            return Err(Unusable::NoProgress {
                answers: MOST_ANSWERS,
            });
        }
        // A piece whose codings were not undone is raw coded bytes, and nothing
        // knows where in the coded stream it ends. `crate::connection` undoes
        // them only for a body that arrived whole, which is why this turns on
        // `ended_early` rather than on the header alone.
        if ended_early {
            if let Some(coding) = coding_applied(&response.headers) {
                return Err(Unusable::Coded { coding });
            }
        }

        match response.status.0 {
            200 => self.restart(response)?,
            206 => {
                if self.resuming().is_none() {
                    // Nothing asked for a range, so a range is not an answer.
                    return Err(Unusable::NotAnAnswer {
                        status: response.status,
                    });
                }
                // The offsets in a `Content-Range` are offsets into the
                // representation the server is sending. Ours are offsets into
                // an identity one, and a coded piece's are not.
                if let Some(coding) = content_coding(&response.headers) {
                    return Err(Unusable::Coded { coding });
                }
                self.place(response)?;
            }
            _ => {
                return Err(Unusable::NotAnAnswer {
                    status: response.status,
                });
            }
        }

        if !ended_early {
            // A body that arrived as its framing said may still be short of the
            // whole thing: `Framing::UntilClose` cannot tell a finished body
            // from a truncated one, and it is what a response with no length
            // and no chunking gets. A length somebody stated is the better
            // authority, so it is asked first.
            if self.whole.is_none_or(|whole| self.so_far() >= whole) {
                return Ok(Step::Done);
            }
        }
        if !self.resumable {
            return Err(Unusable::WillNotResume);
        }
        if self.answers >= MOST_ANSWERS {
            // Here rather than at the top, so the bound is on answers *taken*
            // rather than on requests sent: giving up after asking one more
            // time than it will ever read spends a round trip to learn nothing.
            return Err(Unusable::NoProgress {
                answers: MOST_ANSWERS,
            });
        }
        Ok(Step::More)
    }

    /// Take these bytes as the whole thing, from byte zero.
    ///
    /// Whatever was held is dropped rather than appended: a `200` is the start
    /// of the thing however it was asked for, and a client that appended one to
    /// a half-finished download would produce a file with its own beginning in
    /// the middle of it.
    fn restart(&mut self, response: &Response) -> Result<(), Unusable> {
        if self.resuming().is_some() {
            self.restarts += 1;
        }
        self.got.clear();
        hold(&mut self.got, &response.body)?;
        // A `Content-Length` counts the bytes on the wire. When a coding was
        // applied those are the coded ones and the body has since been undone,
        // so the number describes something other than what is held — and a
        // download that believed it would ask for a range past the end.
        self.whole = if coding_applied(&response.headers).is_some() {
            None
        } else {
            stated_length(&response.headers)
        };
        self.validator = validator_in(&response.headers);
        self.resumable = range::will_take_a_range(&response.headers);
        self.head = Some(response.headers.clone());
        Ok(())
    }

    /// Put a range's bytes where its `Content-Range` says they go — after
    /// checking that where it says is where the next byte belongs.
    fn place(&mut self, response: &Response) -> Result<(), Unusable> {
        let sent: Sent = range::what_was_sent(&response.headers)
            .map_err(|refused| Unusable::ARangeWeCannotPlace { why: refused.why })?
            .ok_or(Unusable::NoContentRange)?;

        let wanted = self.so_far();
        if sent.first != wanted {
            return Err(Unusable::AtTheWrongOffset {
                wanted,
                sent: sent.first,
            });
        }
        if let (Some(was), Some(now)) = (self.whole, sent.complete)
            && was != now
        {
            return Err(Unusable::ItChanged { was, now });
        }
        // Fewer bytes than the range said is a body that stopped, which is the
        // ordinary case here. *More* is a server contradicting itself, and the
        // extra bytes would land where the next piece goes.
        let arrived = u64::try_from(response.body.len()).unwrap_or(u64::MAX);
        if arrived > sent.count() {
            return Err(Unusable::MoreThanItSaid {
                said: sent.count(),
                sent: arrived,
            });
        }

        hold(&mut self.got, &response.body)?;
        if self.whole.is_none() {
            self.whole = sent.complete;
        }
        Ok(())
    }

    /// The download as one response, at a URL.
    ///
    /// The head is the one the bytes started with, with `Content-Length`
    /// corrected to the truth about the body — because that head's own length
    /// described one answer and this body is however many it took.
    pub fn into_response(self, url: Url) -> Response {
        let mut headers = self.head.unwrap_or_default();
        headers.replace("Content-Length", &self.got.len().to_string());
        Response {
            url,
            status: Status::OK,
            headers,
            body: self.got,
        }
    }
}

/// Append, within the bound on the whole download.
fn hold(got: &mut Vec<u8>, more: &[u8]) -> Result<(), Unusable> {
    let after = u64::try_from(got.len().saturating_add(more.len())).unwrap_or(u64::MAX);
    if after > LARGEST {
        return Err(Unusable::TooLong { bytes: after });
    }
    got.extend_from_slice(more);
    Ok(())
}

/// The length a head states, when it states one this engine can read.
fn stated_length(headers: &Headers) -> Option<u64> {
    headers.get("Content-Length")?.trim().parse().ok()
}

/// What to put in `If-Range`, or nothing.
///
/// A **weak** `ETag` is deliberately not taken: it says two representations are
/// good enough to swap for one another, which is a different claim from "these
/// are the same bytes" and is exactly the wrong claim to splice on. The
/// standard says a client must not send one here, and the reason is this.
fn validator_in(headers: &Headers) -> Option<String> {
    if let Some(tag) = headers.get("ETag") {
        let tag = tag.trim();
        if tag.starts_with('"') && tag.ends_with('"') && tag.len() >= 2 {
            return Some(tag.to_owned());
        }
    }
    // A date is a coarser validator — two versions a second apart share one —
    // but a server comparing it will still refuse a thing that changed a day
    // ago, and the `Content-Range` length check catches what it misses.
    headers
        .get("Last-Modified")
        .map(str::trim)
        .filter(|held| !held.is_empty())
        .map(str::to_owned)
}

/// The first `Content-Encoding` that is not `identity`.
fn content_coding(headers: &Headers) -> Option<String> {
    named_coding(headers, "Content-Encoding")
}

/// The first coding of either kind that is not `identity` or `chunked`.
///
/// `chunked` is framing rather than a coding: it is taken off by
/// [`crate::body`] on the way in and there is nothing left of it in the bytes.
fn coding_applied(headers: &Headers) -> Option<String> {
    content_coding(headers).or_else(|| named_coding(headers, "Transfer-Encoding"))
}

fn named_coding(headers: &Headers, name: &str) -> Option<String> {
    headers
        .all(name)
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .find(|coding| {
            !coding.is_empty()
                && !coding.eq_ignore_ascii_case("identity")
                && !coding.eq_ignore_ascii_case("chunked")
        })
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::{Download, Step, Unusable};
    use crate::request::Request;
    use crate::response::{Response, Status};

    fn url() -> alo_url::Url {
        alo_url::parse("https://example.com/big.bin").expect("a URL")
    }

    /// An answer, built the way a server would.
    fn answer(status: u16, body: &[u8], headers: &[(&str, &str)]) -> Response {
        let mut response = Response {
            url: url(),
            status: Status(status),
            headers: crate::headers::Headers::new(),
            body: body.to_vec(),
        };
        for (name, value) in headers {
            response.headers.add(*name, *value);
        }
        response
    }

    /// The whole thing, with an `ETag` so a resume has something to ask with.
    fn whole(body: &[u8]) -> Response {
        answer(
            200,
            body,
            &[
                ("Content-Length", &body.len().to_string()),
                ("ETag", "\"v1\""),
                ("Accept-Ranges", "bytes"),
            ],
        )
    }

    #[test]
    fn a_download_that_arrives_whole_is_one_answer_and_no_range() {
        let mut download = Download::new();
        let request = Request::get(url());
        let asking = download.asking(&request);
        assert_eq!(asking.headers.get("Range"), None, "nothing to continue yet");
        assert_eq!(
            asking.headers.get("Accept-Encoding"),
            Some("identity"),
            "from the first ask, not only from the resumed one"
        );

        assert_eq!(download.take(&whole(b"0123456789"), false), Ok(Step::Done));
        assert_eq!(download.bytes(), b"0123456789");
        assert_eq!(download.restarts(), 0);
    }

    #[test]
    fn a_body_that_stops_half_way_is_asked_for_from_where_it_stopped() {
        // The item's own closing condition, without a socket in it.
        let mut download = Download::new();
        let request = Request::get(url());

        let mut cut = whole(b"0123456789");
        cut.body = b"01234".to_vec();
        assert_eq!(download.take(&cut, true), Ok(Step::More));
        assert_eq!(download.bytes(), b"01234");

        let asking = download.asking(&request);
        assert_eq!(asking.headers.get("Range"), Some("bytes=5-"));
        assert_eq!(
            asking.headers.get("If-Range"),
            Some("\"v1\""),
            "so the server can say the thing changed"
        );

        let rest = answer(
            206,
            b"56789",
            &[("Content-Range", "bytes 5-9/10"), ("ETag", "\"v1\"")],
        );
        assert_eq!(download.take(&rest, false), Ok(Step::Done));
        assert_eq!(
            download.bytes(),
            b"0123456789",
            "the same bytes as an uninterrupted one"
        );
    }

    #[test]
    fn a_server_answering_a_range_with_the_whole_thing_is_noticed_rather_than_believed() {
        // The other half of the closing condition. Appending would give
        // `01234` + `0123456789`: eleven correct-looking bytes too many.
        let mut download = Download::new();
        let mut cut = whole(b"0123456789");
        cut.body = b"01234".to_vec();
        assert_eq!(download.take(&cut, true), Ok(Step::More));

        assert_eq!(download.take(&whole(b"0123456789"), false), Ok(Step::Done));
        assert_eq!(download.bytes(), b"0123456789");
        assert_eq!(download.restarts(), 1, "and it is visible that it happened");
    }

    #[test]
    fn a_server_that_will_never_resume_runs_out_of_attempts_rather_than_looping() {
        let mut download = Download::new();
        let mut cut = whole(b"0123456789");
        cut.body = b"01234".to_vec();
        let mut last = Ok(Step::More);
        for _ in 0..20 {
            last = download.take(&cut, true);
            if last.is_err() {
                break;
            }
        }
        assert_eq!(last, Err(Unusable::NoProgress { answers: 8 }));
        assert!(download.restarts() > 0, "each one was a fresh start");
    }

    #[test]
    fn a_range_that_starts_anywhere_but_where_we_stopped_is_refused() {
        let mut download = Download::new();
        let mut cut = whole(b"0123456789");
        cut.body = b"01234".to_vec();
        assert_eq!(download.take(&cut, true), Ok(Step::More));

        let wrong = answer(
            206,
            b"6789",
            &[("Content-Range", "bytes 6-9/10"), ("ETag", "\"v1\"")],
        );
        assert_eq!(
            download.take(&wrong, false),
            Err(Unusable::AtTheWrongOffset { wanted: 5, sent: 6 }),
            "one byte off is one byte missing from the middle of a file"
        );
    }

    #[test]
    fn a_thing_that_changed_length_underneath_the_download_is_refused() {
        let mut download = Download::new();
        let mut cut = whole(b"0123456789");
        cut.body = b"01234".to_vec();
        assert_eq!(download.take(&cut, true), Ok(Step::More));

        let changed = answer(
            206,
            b"56789abc",
            &[("Content-Range", "bytes 5-12/13"), ("ETag", "\"v2\"")],
        );
        assert_eq!(
            download.take(&changed, false),
            Err(Unusable::ItChanged { was: 10, now: 13 })
        );
    }

    #[test]
    fn a_range_carrying_more_bytes_than_it_claims_is_refused() {
        let mut download = Download::new();
        let mut cut = whole(b"0123456789");
        cut.body = b"01234".to_vec();
        assert_eq!(download.take(&cut, true), Ok(Step::More));

        let too_much = answer(
            206,
            b"56789xxxx",
            &[("Content-Range", "bytes 5-9/10"), ("ETag", "\"v1\"")],
        );
        assert_eq!(
            download.take(&too_much, false),
            Err(Unusable::MoreThanItSaid { said: 5, sent: 9 })
        );
    }

    #[test]
    fn a_download_with_no_validator_starts_again_rather_than_splicing() {
        // Nothing to put in `If-Range`, so nothing could tell us the file
        // changed between the two asks. Starting again is slower and is the
        // only answer that cannot be silently wrong.
        let mut download = Download::new();
        let cut = answer(200, b"01234", &[("Content-Length", "10")]);
        assert_eq!(download.take(&cut, true), Ok(Step::More));

        let asking = download.asking(&Request::get(url()));
        assert_eq!(asking.headers.get("Range"), None, "no range without one");
        assert_eq!(asking.headers.get("If-Range"), None);
    }

    #[test]
    fn a_server_that_says_it_will_not_take_a_range_is_a_failure_rather_than_a_retry() {
        let mut download = Download::new();
        let cut = answer(
            200,
            b"01234",
            &[
                ("Content-Length", "10"),
                ("ETag", "\"v1\""),
                ("Accept-Ranges", "none"),
            ],
        );
        assert_eq!(download.take(&cut, true), Err(Unusable::WillNotResume));
    }

    #[test]
    fn a_range_nobody_asked_for_is_not_an_answer() {
        let mut download = Download::new();
        let unasked = answer(206, b"56789", &[("Content-Range", "bytes 5-9/10")]);
        assert_eq!(
            download.take(&unasked, false),
            Err(Unusable::NotAnAnswer {
                status: Status(206)
            })
        );
    }

    #[test]
    fn a_range_with_nothing_saying_which_bytes_it_is_is_refused() {
        let mut download = Download::new();
        let mut cut = whole(b"0123456789");
        cut.body = b"01234".to_vec();
        assert_eq!(download.take(&cut, true), Ok(Step::More));

        let mute = answer(206, b"56789", &[("ETag", "\"v1\"")]);
        assert_eq!(download.take(&mute, false), Err(Unusable::NoContentRange));
    }

    #[test]
    fn a_content_range_this_engine_will_not_read_refuses_by_its_own_words() {
        let mut download = Download::new();
        let mut cut = whole(b"0123456789");
        cut.body = b"01234".to_vec();
        assert_eq!(download.take(&cut, true), Ok(Step::More));

        let nonsense = answer(206, b"56789", &[("Content-Range", "bytes */10")]);
        let why = match download.take(&nonsense, false) {
            Err(Unusable::ARangeWeCannotPlace { why }) => why,
            other => format!("{other:?}"),
        };
        assert!(why.contains("were not sent"), "{why:?}");
    }

    #[test]
    fn no_piece_of_a_download_is_ever_spliced_encoded() {
        // A short body has not had its codings undone, so what is held is raw
        // gzip and nothing knows where in the gzip it stops.
        let mut download = Download::new();
        let cut = answer(
            200,
            b"\x1f\x8b\x08",
            &[("Content-Encoding", "gzip"), ("ETag", "\"v1\"")],
        );
        assert_eq!(
            download.take(&cut, true),
            Err(Unusable::Coded {
                coding: "gzip".to_owned()
            })
        );

        // And a range of a coded representation, whose offsets are offsets
        // into bytes we do not have.
        let mut download = Download::new();
        let mut short = whole(b"0123456789");
        short.body = b"01234".to_vec();
        assert_eq!(download.take(&short, true), Ok(Step::More));
        let coded = answer(
            206,
            b"56789",
            &[
                ("Content-Range", "bytes 5-9/10"),
                ("Content-Encoding", "br"),
                ("ETag", "\"v1\""),
            ],
        );
        assert_eq!(
            download.take(&coded, false),
            Err(Unusable::Coded {
                coding: "br".to_owned()
            })
        );
    }

    #[test]
    fn a_whole_body_that_was_compressed_is_finished_rather_than_resumed() {
        // `crate::connection` has already undone the coding, so the body is
        // longer than the `Content-Length` that counted its compressed bytes —
        // and a download that believed that length would ask for a range past
        // the end of something it already has all of.
        let mut download = Download::new();
        let decoded = answer(
            200,
            b"0123456789",
            &[("Content-Length", "4"), ("Content-Encoding", "gzip")],
        );
        assert_eq!(download.take(&decoded, false), Ok(Step::Done));
        assert_eq!(download.whole(), None, "that length described other bytes");
        assert_eq!(download.bytes(), b"0123456789");
    }

    #[test]
    fn a_body_that_ends_when_the_connection_does_is_still_measured_against_a_length() {
        // `Framing::UntilClose` cannot tell a finished body from a truncated
        // one, so it never reports one short. A stated length can.
        let mut download = Download::new();
        let cut = answer(
            200,
            b"01234",
            &[
                ("Content-Length", "10"),
                ("ETag", "\"v1\""),
                ("Accept-Ranges", "bytes"),
            ],
        );
        assert_eq!(
            download.take(&cut, false),
            Ok(Step::More),
            "the framing was happy and the length was not"
        );
    }

    #[test]
    fn a_weak_etag_is_never_what_a_resume_is_asked_against() {
        let mut download = Download::new();
        let cut = answer(
            200,
            b"01234",
            &[("Content-Length", "10"), ("ETag", "W/\"v1\"")],
        );
        assert_eq!(download.take(&cut, true), Ok(Step::More));
        let asking = download.asking(&Request::get(url()));
        assert_eq!(
            asking.headers.get("If-Range"),
            None,
            "a weak tag says two things are good enough to swap, not that they \
             are the same bytes"
        );
    }

    #[test]
    fn a_last_modified_is_taken_when_there_is_no_strong_tag() {
        let mut download = Download::new();
        let cut = answer(
            200,
            b"01234",
            &[
                ("Content-Length", "10"),
                ("Last-Modified", "Wed, 21 Oct 2015 07:28:00 GMT"),
            ],
        );
        assert_eq!(download.take(&cut, true), Ok(Step::More));
        let asking = download.asking(&Request::get(url()));
        assert_eq!(
            asking.headers.get("If-Range"),
            Some("Wed, 21 Oct 2015 07:28:00 GMT")
        );
    }

    #[test]
    fn the_finished_download_says_how_long_it_actually_is() {
        let mut download = Download::new();
        let mut cut = whole(b"0123456789");
        cut.body = b"01234".to_vec();
        assert_eq!(download.take(&cut, true), Ok(Step::More));
        let rest = answer(
            206,
            b"56789",
            &[
                ("Content-Range", "bytes 5-9/10"),
                ("Content-Length", "5"),
                ("ETag", "\"v1\""),
            ],
        );
        assert_eq!(download.take(&rest, false), Ok(Step::Done));

        let response = download.into_response(url());
        assert_eq!(response.status, Status::OK, "not the 206 of its last piece");
        assert_eq!(response.body, b"0123456789");
        assert_eq!(
            response.headers.get("Content-Length"),
            Some("10"),
            "the truth about the body rather than about one answer"
        );
        assert_eq!(
            response.headers.get("Content-Range"),
            None,
            "the head is the representation's, and a 200 carries no range"
        );
        assert_eq!(response.headers.get("ETag"), Some("\"v1\""));
    }

    #[test]
    fn a_status_a_download_can_do_nothing_with_says_so() {
        for status in [204u16, 301, 404, 416, 500] {
            let mut download = Download::new();
            let answered = answer(status, b"", &[]);
            assert_eq!(
                download.take(&answered, false),
                Err(Unusable::NotAnAnswer {
                    status: Status(status)
                }),
                "{status}"
            );
        }
    }

    #[test]
    fn nothing_a_server_can_send_makes_this_panic() {
        // Not an assertion about what each means — an assertion that folding in
        // bytes a stranger chose is a result rather than a crash.
        let ranges = [
            "bytes 0-0/0",
            "bytes 18446744073709551615-18446744073709551615/18446744073709551615",
            "bytes 5-4/10",
            "",
            "bytes",
            "\u{1f600}",
        ];
        for range in ranges {
            for lengths in ["0", "18446744073709551615", "-1", "nine"] {
                let mut download = Download::new();
                let first = answer(
                    200,
                    b"01",
                    &[("Content-Length", lengths), ("ETag", "\"v1\"")],
                );
                let _ = download.take(&first, true);
                let next = answer(206, b"23", &[("Content-Range", range)]);
                let _ = download.take(&next, false);
                let _ = download.asking(&Request::get(url()));
            }
        }
    }
}
