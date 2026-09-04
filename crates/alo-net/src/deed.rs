/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What one agent action did, as bytes on a disk.
//!
//! ADR 0012 § 6's second half: *"What an agent did is kept until the person
//! deletes it. A record that vanishes when the browser closes cannot answer
//! **what did it do while I was not watching**, and that question is the entire
//! reason `alo-os` ADR 0001 records anything at all."*
//!
//! [`crate::activity`] is the session's record and dies with the process. This
//! is one **action**, with the requests that followed from it, written down so
//! that it does not. [`crate::kept`] is the directory of them and the policy;
//! this file is the bytes, which is the whole of the untrusted surface — the
//! same division as [`crate::record`] and [`crate::disk`], and it reads its
//! bytes with the same reader ([`crate::bytes`]).
//!
//! # The chain is frozen here, and only here
//!
//! ADR 0012 § 3 says a chain is *walked rather than assembled*, and
//! [`crate::activity::Entry`] keeps one link and walks the rest against
//! [`crate::chain::Documents`]. **A durable entry cannot do that**: the
//! documents are bounded and die with the process, so a file read back next week
//! would have nothing to walk against.
//!
//! So a [`Did`] freezes the chain at the moment it is written. That is *not* the
//! side table § 3 refuses, and the difference is precise: the thing § 3 refuses
//! is a second record kept **beside** a live one it could come to disagree with.
//! Here there is nothing left to disagree with — the documents are gone, the
//! process is gone, and a frozen chain is the only form in which the answer can
//! survive at all.
//!
//! # An identity in a file is not an identity
//!
//! ADR 0003's ids are allocated once and never reused **by one
//! [`Identities`](crate::cause::Identities)**, and there is one of those per
//! browser process. So `action#3` in a file written last week and `action#3`
//! minted this morning are two different actions with one name.
//!
//! That is why a frozen link is a [`Link`] holding plain numbers rather than a
//! [`Cause`](crate::cause::Cause) holding live ids: a decoded id must never
//! compare equal to one this process minted, and the surest way to promise that
//! is for there to be no way to make one. What names an action across sessions
//! is the number [`crate::kept`] files it under, which counts up on the disk.
//!
//! # What may go in, and what may not
//!
//! Everything ADR 0012 § 5 refuses is already refused by [`Did`] being built
//! from an [`Entry`](crate::activity::Entry), which holds no body and no
//! headers. Two things are refused **again here**, because a durable file is
//! worse than a session's memory in exactly two ways:
//!
//! - **A `data:` URL is content rather than an address.** Writing one whole into
//!   a file is writing the bytes a page chose, which is § 5's refusal wearing a
//!   URL's clothes — a picture, a script, or somebody's secret. Only its scheme
//!   and its media type are kept ([`Address::cut`]).
//! - **A URL longer than [`LONGEST_URL`] is cut**, and says so. What a line
//!   costs is mostly the length of its address, and a page chooses that.

use crate::activity::{Entry, Happened};
use crate::bytes::{Reader, Unreadable, Writer, unreadable};
use crate::cause::Cause;
use crate::chain::{Chain, End};
use crate::request::Purpose;
use crate::response::Status;
use core::fmt;
use std::time::SystemTime;

/// What every file begins with, so one that is not ours is refused before
/// anything else is read.
pub const MAGIC: [u8; 8] = *b"alodeeds";

/// The format this engine writes.
///
/// A version we do not recognise is a **gap** rather than an error: the same
/// rule ADR 0011 § 4 gives a cache entry, with a different consequence, because
/// a cache miss is fetched again and a record nobody can read is a piece of
/// somebody's history that is not there. [`crate::kept`] counts them and says
/// how many.
pub const VERSION: u16 = 1;

/// The bytes before the checksummed part: magic, version, the durable number,
/// checksum.
pub const PREFIX: usize = 8 + 2 + 8 + 8;

/// The most characters of a URL that are kept.
///
/// Generous enough that an ordinary address with a query on it is whole, and
/// short enough that a page cannot decide how large this file is.
pub const LONGEST_URL: usize = 2000;

/// One link of a chain that has been frozen.
///
/// The same three causes ADR 0012 § 2 allows, in the one form that can outlive
/// the process that minted them: numbers, which name nothing live. See this
/// module's own documentation for why that matters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// A navigation somebody made, in the tab they made it in.
    Person {
        /// What that session called the tab.
        tab: u64,
    },
    /// A page loading what it needs.
    Document {
        /// What that session called the document.
        document: u64,
    },
    /// A verb this engine performed on an agent's behalf.
    Agent {
        /// What that session called the action.
        action: u64,
        /// The document it acted in.
        document: u64,
    },
}

impl Link {
    /// The frozen form of a live cause.
    pub fn of(cause: &Cause) -> Self {
        match cause {
            Cause::Person { tab } => Link::Person { tab: tab.as_u64() },
            Cause::Document { document } => Link::Document {
                document: document.as_u64(),
            },
            Cause::Agent { action, document } => Link::Agent {
                action: action.as_u64(),
                document: document.as_u64(),
            },
        }
    }

    /// The action this link names, when it names one.
    pub fn action(&self) -> Option<u64> {
        match self {
            Link::Agent { action, .. } => Some(*action),
            Link::Person { .. } | Link::Document { .. } => None,
        }
    }
}

impl fmt::Display for Link {
    /// The words [`Cause`] uses, so that a line read out of a file and a line
    /// read out of this session say the same thing.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Link::Person { tab } => write!(f, "the person, in tab#{tab}"),
            Link::Document { document } => write!(f, "document#{document}"),
            Link::Agent { action, document } => {
                write!(f, "action#{action}, in document#{document}")
            }
        }
    }
}

/// Where a frozen walk stopped, and why.
///
/// [`End`] with the identities flattened, for [`Link`]'s reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// A person's own navigation, in this tab. Where a whole chain ends.
    Person {
        /// What that session called the tab.
        tab: u64,
    },
    /// The cause of this document's load had been dropped under the session's
    /// bound before the walk reached it.
    Forgotten {
        /// What that session called the document.
        document: u64,
    },
    /// Nothing ever recorded what caused this document's load.
    Unrecorded {
        /// What that session called the document.
        document: u64,
    },
    /// The walk arrived at a document it had already been through.
    CameRound {
        /// What that session called the document.
        document: u64,
    },
}

impl Ended {
    /// The frozen form of where a live walk stopped.
    pub fn of(end: End) -> Self {
        match end {
            End::Person(tab) => Ended::Person { tab: tab.as_u64() },
            End::Forgotten(document) => Ended::Forgotten {
                document: document.as_u64(),
            },
            End::Unrecorded(document) => Ended::Unrecorded {
                document: document.as_u64(),
            },
            End::CameRound(document) => Ended::CameRound {
                document: document.as_u64(),
            },
        }
    }

    /// Whether the walk reached a person rather than running out of record.
    pub fn is_whole(self) -> bool {
        matches!(self, Ended::Person { .. })
    }
}

impl fmt::Display for Ended {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ended::Person { tab } => write!(f, "the person, in tab#{tab}"),
            Ended::Forgotten { document } => {
                write!(f, "what caused document#{document} is no longer remembered")
            }
            Ended::Unrecorded { document } => {
                write!(f, "nothing recorded what caused document#{document}")
            }
            Ended::CameRound { document } => write!(f, "document#{document} came round again"),
        }
    }
}

/// A URL as a durable record may hold it.
///
/// Whole for the ordinary case, and cut for the two this module's documentation
/// names. Cut rather than dropped, because *what it read* is the question — an
/// entry saying only that something was fetched answers nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    /// The address, entire.
    Whole {
        /// What was asked for.
        url: String,
    },
    /// As much of it as may be kept, and it says so where it stops.
    Cut {
        /// The part that is kept.
        beginning: String,
        /// Why the rest is not here, in words.
        why: &'static str,
    },
}

impl Address {
    /// What may durably be written down about this URL.
    ///
    /// A `data:` URL keeps its scheme and its media type and loses the content,
    /// because the content is what a `data:` URL *is*. Anything else is kept
    /// whole up to [`LONGEST_URL`] characters.
    pub fn of(url: &str) -> Self {
        if let Some(rest) = url.strip_prefix("data:") {
            let kind = rest.split(',').next().unwrap_or_default();
            return Address::Cut {
                beginning: format!("data:{}", first_of(kind, LONGEST_URL)),
                why: "a data URL is content rather than an address",
            };
        }
        if url.char_indices().nth(LONGEST_URL).is_none() {
            return Address::Whole {
                url: url.to_owned(),
            };
        }
        Address::Cut {
            beginning: first_of(url, LONGEST_URL),
            why: "an address longer than this record keeps",
        }
    }

    /// What is held, whether or not it is whole.
    pub fn said(&self) -> &str {
        match self {
            Address::Whole { url } => url,
            Address::Cut { beginning, .. } => beginning,
        }
    }

    /// Whether the whole of it is here.
    pub fn is_whole(&self) -> bool {
        matches!(self, Address::Whole { .. })
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Address::Whole { url } => f.write_str(url),
            Address::Cut { beginning, why } => write!(f, "{beginning}… ({why})"),
        }
    }
}

/// One request that followed from an action, frozen.
///
/// The six things ADR 0012 § 5 lists, with the chain in place of the one link a
/// session's entry keeps. Built by [`Did::of`] from an
/// [`Entry`](crate::activity::Entry) and the walk that was made against the
/// browser process's own documents — never from anything a caller composed,
/// which is what keeps a durable line as unforgeable as a session one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Did {
    /// When it was asked for.
    pub at: SystemTime,
    /// What caused it, and what caused that, nearest first. Never empty.
    pub chain: Vec<Link>,
    /// Where the walk stopped, and why.
    pub ended: Ended,
    /// The method.
    pub method: String,
    /// What was asked for.
    pub url: Address,
    /// What kind of thing it was.
    pub purpose: Purpose,
    /// What became of it.
    pub happened: Happened,
}

impl Did {
    /// Freeze one line of the session's record, with the walk already made.
    pub fn of(entry: &Entry, chain: &Chain) -> Self {
        Self {
            at: entry.at(),
            chain: chain.links().iter().map(Link::of).collect(),
            ended: Ended::of(chain.end()),
            method: entry.method().to_owned(),
            url: Address::of(&entry.url().serialised),
            purpose: entry.purpose().clone(),
            happened: entry.happened().clone(),
        }
    }

    /// The nearest action in the frozen chain, if there is one.
    pub fn action(&self) -> Option<u64> {
        self.chain.iter().find_map(Link::action)
    }

    /// Roughly what this costs, for the bound on a file.
    pub fn weighs(&self) -> usize {
        let said = match &self.happened {
            Happened::Refused { rule } => rule.len(),
            Happened::Failed { why } => why.len(),
            Happened::Answered { .. } | Happened::Served { .. } => 0,
        };
        size_of::<Did>()
            .saturating_add(self.method.len())
            .saturating_add(self.url.said().len())
            .saturating_add(self.chain.len().saturating_mul(size_of::<Link>()))
            .saturating_add(said)
    }
}

impl fmt::Display for Did {
    /// The line a person reads. The moment is not in it: a time is formatted
    /// where it is shown, in whatever the person's own reckoning of one is.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} ({})", self.method, self.url, self.purpose)?;
        for link in &self.chain {
            write!(f, ", caused by {link}")?;
        }
        if !self.ended.is_whole() {
            write!(f, " — {}", self.ended)?;
        }
        write!(f, " — {}", self.happened)
    }
}

/// One agent action, and what followed from it.
///
/// What a file in [`crate::kept`]'s directory holds. Bounded in requests, and
/// honest about it: see [`Deed::forgotten`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deed {
    /// Which action, by the number a durable record files it under.
    ///
    /// Not the [`ActionId`](crate::cause::ActionId): that counts from zero in
    /// every process, so it names one action only within the session that
    /// minted it. This counts up on the disk and never comes round again.
    pub number: u64,
    /// What the session that made it called the action, for reading a durable
    /// line beside a live one.
    pub was: u64,
    /// How many requests were dropped under this file's own bound.
    ///
    /// A record that quietly shortened itself would read as an action that did
    /// less than it did — [`crate::activity::Activity::forgotten`]'s rule, one
    /// layer along.
    pub forgotten: u64,
    /// What followed from the action, oldest first.
    pub requests: Vec<Did>,
}

impl Deed {
    /// When the first of its requests was made, if it has any.
    pub fn began(&self) -> Option<SystemTime> {
        self.requests.first().map(|did| did.at)
    }
}

impl fmt::Display for Deed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "action#{}: {} requests", self.was, self.requests.len())?;
        if self.forgotten > 0 {
            write!(f, ", and {} no longer kept", self.forgotten)?;
        }
        Ok(())
    }
}

/// The checksum a file carries.
///
/// Over the durable number **and** the body, for [`crate::record::checksum`]'s
/// reason: the number decides which file is evicted first, so a file with a
/// flipped byte in it would otherwise be a valid record that lies about its
/// place in the order.
pub fn checksum(number: u64, body: &[u8]) -> u64 {
    crate::record::checksum(number, body)
}

/// A deed as bytes.
pub fn encode(deed: &Deed) -> Vec<u8> {
    let mut body = Writer::default();
    body.number(deed.was);
    body.number(deed.forgotten);
    body.number(deed.requests.len() as u64);
    for did in &deed.requests {
        body.time(did.at);
        body.number(did.chain.len() as u64);
        for link in &did.chain {
            match link {
                Link::Person { tab } => {
                    body.tag(0);
                    body.number(*tab);
                }
                Link::Document { document } => {
                    body.tag(1);
                    body.number(*document);
                }
                Link::Agent { action, document } => {
                    body.tag(2);
                    body.number(*action);
                    body.number(*document);
                }
            }
        }
        match did.ended {
            Ended::Person { tab } => {
                body.tag(0);
                body.number(tab);
            }
            Ended::Forgotten { document } => {
                body.tag(1);
                body.number(document);
            }
            Ended::Unrecorded { document } => {
                body.tag(2);
                body.number(document);
            }
            Ended::CameRound { document } => {
                body.tag(3);
                body.number(document);
            }
        }
        body.text(&did.method);
        match &did.url {
            Address::Whole { url } => {
                body.flag(true);
                body.text(url);
            }
            Address::Cut { beginning, why } => {
                body.flag(false);
                body.text(beginning);
                body.text(why);
            }
        }
        body.tag(purpose_tag(&did.purpose));
        match &did.happened {
            Happened::Answered { status, whole } => {
                body.tag(0);
                body.small(status.0);
                body.flag(*whole);
            }
            Happened::Served { status } => {
                body.tag(1);
                body.small(status.0);
            }
            Happened::Refused { rule } => {
                body.tag(2);
                body.text(rule);
            }
            Happened::Failed { why } => {
                body.tag(3);
                body.text(why);
            }
        }
    }

    let mut out = Vec::with_capacity(PREFIX.saturating_add(body.out.len()));
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_be_bytes());
    out.extend_from_slice(&deed.number.to_be_bytes());
    out.extend_from_slice(&checksum(deed.number, &body.out).to_be_bytes());
    out.extend_from_slice(&body.out);
    out
}

/// The durable number a file carries, without reading the rest of it.
///
/// # Errors
///
/// [`Unreadable`], for a file that is too short, is not ours, or is a version
/// this engine does not know.
pub fn number_of(bytes: &[u8]) -> Result<u64, Unreadable> {
    let magic = bytes
        .get(..8)
        .ok_or_else(|| unreadable("a record too short to be one"))?;
    if magic != MAGIC {
        return Err(unreadable("a file in the record that is not a record"));
    }
    let version = bytes
        .get(8..10)
        .ok_or_else(|| unreadable("a record that stops where its version should be"))?;
    let mut two = [0u8; 2];
    two.copy_from_slice(version);
    let version = u16::from_be_bytes(two);
    if version != VERSION {
        return Err(unreadable(format!(
            "a record written in format {version}, and this engine reads {VERSION}"
        )));
    }
    let number = bytes
        .get(10..18)
        .ok_or_else(|| unreadable("a record that stops where its number should be"))?;
    let mut eight = [0u8; 8];
    eight.copy_from_slice(number);
    Ok(u64::from_be_bytes(eight))
}

/// A deed from bytes, or the reason it cannot be read.
///
/// # Errors
///
/// [`Unreadable`], for anything at all: the wrong magic, an unknown version, a
/// checksum that does not match, a length longer than what is there, a tag this
/// engine has no meaning for, text that is not UTF-8, a moment that cannot
/// exist, a chain with no links in it, or bytes left over at the end.
pub fn decode(bytes: &[u8]) -> Result<Deed, Unreadable> {
    let number = number_of(bytes)?;

    let claimed = bytes
        .get(18..PREFIX)
        .ok_or_else(|| unreadable("a record that stops where its checksum should be"))?;
    let mut eight = [0u8; 8];
    eight.copy_from_slice(claimed);
    let claimed = u64::from_be_bytes(eight);

    let body = bytes
        .get(PREFIX..)
        .ok_or_else(|| unreadable("a record with a header and nothing else"))?;
    if checksum(number, body) != claimed {
        return Err(unreadable(
            "a record whose checksum does not match what is in it",
        ));
    }

    let mut reader = Reader::new(body);
    let was = reader.number()?;
    let forgotten = reader.number()?;
    let how_many = reader.how_many()?;
    let mut requests = Vec::new();
    for _ in 0..how_many {
        requests.push(one_request(&mut reader)?);
    }
    if !reader.is_done() {
        return Err(unreadable("a record with bytes after the end of it"));
    }

    Ok(Deed {
        number,
        was,
        forgotten,
        requests,
    })
}

/// One frozen request, from bytes a stranger may have written.
fn one_request(reader: &mut Reader<'_>) -> Result<Did, Unreadable> {
    let at = reader.time()?;
    let links = reader.how_many()?;
    let mut chain = Vec::new();
    for _ in 0..links {
        chain.push(match reader.tag()? {
            0 => Link::Person {
                tab: reader.number()?,
            },
            1 => Link::Document {
                document: reader.number()?,
            },
            2 => Link::Agent {
                action: reader.number()?,
                document: reader.number()?,
            },
            other => {
                return Err(unreadable(format!(
                    "a record naming a cause this engine does not have ({other})"
                )));
            }
        });
    }
    // [`Chain::links`] promises it is never empty, and a frozen one that was
    // would be a request nothing caused — which ADR 0012 § 2 says is not a
    // thing. Refusing it here is what keeps that promise true of what comes off
    // a disk as well as of what is walked.
    if chain.is_empty() {
        return Err(unreadable("a record with a request nothing caused"));
    }
    let ended = match reader.tag()? {
        0 => Ended::Person {
            tab: reader.number()?,
        },
        1 => Ended::Forgotten {
            document: reader.number()?,
        },
        2 => Ended::Unrecorded {
            document: reader.number()?,
        },
        3 => Ended::CameRound {
            document: reader.number()?,
        },
        other => {
            return Err(unreadable(format!(
                "a record naming an ending this engine does not have ({other})"
            )));
        }
    };
    let method = reader.text()?;
    let url = if reader.flag()? {
        Address::Whole {
            url: reader.text()?,
        }
    } else {
        let beginning = reader.text()?;
        // The reason is one of ours, so it is matched back to one of ours
        // rather than kept as whatever the file said: a sentence read off a
        // disk and shown to a person is a sentence somebody else could have
        // written.
        let said = reader.text()?;
        Address::Cut {
            beginning,
            why: why_it_was_cut(&said),
        }
    };
    let purpose = purpose_of(reader.tag()?)?;
    let happened = match reader.tag()? {
        0 => Happened::Answered {
            status: Status(reader.small()?),
            whole: reader.flag()?,
        },
        1 => Happened::Served {
            status: Status(reader.small()?),
        },
        2 => Happened::Refused {
            rule: reader.text()?,
        },
        3 => Happened::Failed {
            why: reader.text()?,
        },
        other => {
            return Err(unreadable(format!(
                "a record naming an outcome this engine does not have ({other})"
            )));
        }
    };
    Ok(Did {
        at,
        chain,
        ended,
        method,
        url,
        purpose,
        happened,
    })
}

/// Which of this engine's two reasons for cutting an address a file names.
///
/// Anything else is a file that was not written by this engine, and the honest
/// thing to show a person is that we do not know why it stops rather than a
/// sentence somebody else composed.
fn why_it_was_cut(said: &str) -> &'static str {
    const REASONS: [&str; 2] = [
        "a data URL is content rather than an address",
        "an address longer than this record keeps",
    ];
    REASONS
        .into_iter()
        .find(|reason| *reason == said)
        .unwrap_or("cut, for a reason this engine did not write")
}

/// The first `most` characters of a string.
///
/// By characters rather than by bytes, so an address in a script other than
/// this one is cut between letters instead of through one.
fn first_of(text: &str, most: usize) -> String {
    text.chars().take(most).collect()
}

fn purpose_tag(purpose: &Purpose) -> u8 {
    match purpose {
        Purpose::Document => 0,
        Purpose::Style => 1,
        Purpose::Image => 2,
        Purpose::Script => 3,
        Purpose::Fetch => 4,
        Purpose::Report => 5,
    }
}

fn purpose_of(tag: u8) -> Result<Purpose, Unreadable> {
    match tag {
        0 => Ok(Purpose::Document),
        1 => Ok(Purpose::Style),
        2 => Ok(Purpose::Image),
        3 => Ok(Purpose::Script),
        4 => Ok(Purpose::Fetch),
        5 => Ok(Purpose::Report),
        other => Err(unreadable(format!(
            "a record naming a purpose this engine does not have ({other})"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::Activity;
    use crate::cause::{Cause, Identities};
    use crate::chain::Documents;
    use crate::request::Request;
    use std::time::{Duration, UNIX_EPOCH};

    fn moment(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn a_deed() -> Deed {
        Deed {
            number: 7,
            was: 3,
            forgotten: 2,
            requests: vec![
                Did {
                    at: moment(1_700_000_000),
                    chain: vec![
                        Link::Document { document: 4 },
                        Link::Agent {
                            action: 3,
                            document: 1,
                        },
                        Link::Person { tab: 0 },
                    ],
                    ended: Ended::Person { tab: 0 },
                    method: "GET".to_owned(),
                    url: Address::Whole {
                        url: "https://example.com/app.js".to_owned(),
                    },
                    purpose: Purpose::Script,
                    happened: Happened::Answered {
                        status: Status(200),
                        whole: true,
                    },
                },
                Did {
                    at: moment(1_700_000_005),
                    chain: vec![Link::Agent {
                        action: 3,
                        document: 1,
                    }],
                    ended: Ended::Forgotten { document: 1 },
                    method: "POST".to_owned(),
                    url: Address::Cut {
                        beginning: "data:image/png".to_owned(),
                        why: "a data URL is content rather than an address",
                    },
                    purpose: Purpose::Fetch,
                    happened: Happened::Refused {
                        rule: "redirected in a circle".to_owned(),
                    },
                },
            ],
        }
    }

    // --- What went in comes out ----------------------------------------------

    #[test]
    fn what_went_in_is_what_comes_out() {
        let deed = a_deed();
        let read = decode(&encode(&deed)).expect("a record this engine just wrote");
        assert_eq!(read, deed, "a round trip changed something");
        assert_eq!(
            number_of(&encode(&deed)).expect("a prefix"),
            7,
            "the order is readable without decoding the rest",
        );
    }

    /// Every outcome and every purpose, because a tag that round-tripped for
    /// one variant and not another would be a record that quietly changed what
    /// happened.
    #[test]
    fn every_outcome_and_every_purpose_survives_the_disk() {
        for happened in [
            Happened::Answered {
                status: Status(206),
                whole: false,
            },
            Happened::Served {
                status: Status(304),
            },
            Happened::Refused {
                rule: "a rule with a name".to_owned(),
            },
            Happened::Failed {
                why: "there is no such host".to_owned(),
            },
        ] {
            for purpose in [
                Purpose::Document,
                Purpose::Style,
                Purpose::Image,
                Purpose::Script,
                Purpose::Fetch,
                Purpose::Report,
            ] {
                let mut deed = a_deed();
                if let Some(did) = deed.requests.first_mut() {
                    did.happened = happened.clone();
                    did.purpose = purpose.clone();
                }
                let read = decode(&encode(&deed)).expect("a record");
                assert_eq!(read, deed, "{happened} as {purpose} did not survive");
            }
        }
    }

    /// Every way a walk can stop, including the three that say a piece is
    /// missing. A format that flattened them would turn *we no longer know* into
    /// *it ended here*, which is the guess the whole record exists not to make.
    #[test]
    fn every_ending_survives_the_disk_as_the_one_it_was() {
        for ended in [
            Ended::Person { tab: 9 },
            Ended::Forgotten { document: 9 },
            Ended::Unrecorded { document: 9 },
            Ended::CameRound { document: 9 },
        ] {
            let mut deed = a_deed();
            if let Some(did) = deed.requests.first_mut() {
                did.ended = ended;
            }
            let read = decode(&encode(&deed)).expect("a record");
            assert_eq!(read, deed, "{ended} did not survive");
        }
    }

    // --- Frozen from the session's own record ---------------------------------

    /// A durable line is frozen from a session line and the walk that was made
    /// against the browser process's documents — never composed.
    #[test]
    fn a_frozen_line_says_what_the_session_line_and_the_walk_said() {
        let mut minting = Identities::default();
        let mut documents = Documents::default();
        let tab = minting.a_tab();
        let read = documents.opened(&mut minting, Cause::Person { tab });
        let action = minting.an_action();
        let opened = documents.opened(
            &mut minting,
            Cause::Agent {
                action,
                document: read,
            },
        );
        let mut record = Activity::new();
        let url = alo_url::parse("https://example.com/app.js").expect("a URL");
        record.happened(
            &Request::get(url, Cause::Document { document: opened }).for_purpose(Purpose::Script),
            moment(1),
            Happened::Answered {
                status: Status(200),
                whole: true,
            },
        );

        let Some(line) = record.latest() else {
            panic!("nothing was written down");
        };
        let did = Did::of(line, &line.chain(&documents));

        assert_eq!(did.action(), Some(action.as_u64()));
        assert_eq!(did.ended, Ended::Person { tab: tab.as_u64() });
        assert_eq!(did.chain.len(), 3, "{did}");
        assert_eq!(did.method, "GET");
        assert!(did.url.is_whole());
        assert_eq!(
            did.to_string(),
            "GET https://example.com/app.js (script), caused by document#1, \
             caused by action#0, in document#0, caused by the person, in tab#0 — answered 200",
        );
        assert_eq!(
            decode(&encode(&a_deed_of(did))).map(|read| read.requests.len()),
            Ok(1)
        );
    }

    fn a_deed_of(did: Did) -> Deed {
        Deed {
            number: 0,
            was: did.action().unwrap_or_default(),
            forgotten: 0,
            requests: vec![did],
        }
    }

    // --- What a durable file may not hold -------------------------------------

    /// The refusal this file adds to ADR 0012 § 5's list: a `data:` URL is the
    /// content, and writing one whole into a file on somebody's disk is writing
    /// the bytes a page chose.
    #[test]
    fn a_data_url_keeps_its_kind_and_loses_its_content() {
        let kept = Address::of("data:text/plain;base64,c2VjcmV0IHRoaW5n");
        assert_eq!(kept.said(), "data:text/plain;base64");
        assert!(!kept.is_whole());
        assert!(
            !kept.to_string().contains("c2VjcmV0"),
            "the content reached the record: {kept}"
        );
        assert!(
            kept.to_string().contains("content rather than an address"),
            "it was cut without saying why: {kept}",
        );

        // And one with nothing after the comma is still not an address.
        assert_eq!(Address::of("data:,").said(), "data:");
    }

    /// A page chooses how long its addresses are, so a durable line's size
    /// cannot be a page's decision.
    #[test]
    fn an_address_longer_than_the_bound_is_cut_and_says_so() {
        let enormous = format!("https://example.com/{}", "a".repeat(8192));
        let kept = Address::of(&enormous);
        assert_eq!(kept.said().chars().count(), LONGEST_URL);
        assert!(!kept.is_whole());
        assert!(kept.to_string().contains('…'), "{kept}");

        let ordinary = "https://example.com/a?b=c#d";
        assert_eq!(
            Address::of(ordinary),
            Address::Whole {
                url: ordinary.to_owned()
            },
            "an ordinary address was cut",
        );
    }

    /// Cut between letters rather than through one.
    #[test]
    fn an_address_in_another_script_is_cut_between_letters() {
        let long = format!("https://example.com/{}", "日".repeat(4000));
        let kept = Address::of(&long);
        assert!(kept.said().ends_with('日'), "{kept}");
        assert_eq!(kept.said().chars().count(), LONGEST_URL);
    }

    // --- Read back from a stranger (ADR 0011 § 4's rule, unchanged) ------------

    #[test]
    fn every_truncation_of_a_record_is_refused_rather_than_believed() {
        let whole = encode(&a_deed());
        for cut in 0..whole.len() {
            let short = whole.get(..cut).expect("a prefix of what we wrote");
            assert!(
                decode(short).is_err(),
                "a record cut to {cut} bytes was read as though it were whole"
            );
        }
        assert!(decode(&whole).is_ok(), "the whole of it still reads");
    }

    #[test]
    fn a_single_flipped_byte_anywhere_is_refused() {
        let whole = encode(&a_deed());
        for at in 0..whole.len() {
            let mut damaged = whole.clone();
            if let Some(byte) = damaged.get_mut(at) {
                *byte ^= 0xff;
            }
            assert!(
                decode(&damaged).is_err(),
                "a byte flipped at {at} was read as though nothing had happened"
            );
        }
    }

    #[test]
    fn a_version_this_engine_does_not_know_is_refused_rather_than_guessed_at() {
        let mut written = encode(&a_deed());
        if let Some(slot) = written.get_mut(8..10) {
            slot.copy_from_slice(&99u16.to_be_bytes());
        }
        let refused = decode(&written).expect_err("a format from the future");
        assert!(refused.why.contains("format 99"), "{refused}");
    }

    #[test]
    fn a_file_that_is_not_one_of_ours_is_refused_before_anything_is_read() {
        assert!(decode(b"").is_err());
        assert!(decode(b"alocache and then a cache entry").is_err());
        assert!(decode(&[0xff; 64]).is_err());
        assert!(decode(&MAGIC).is_err());
    }

    /// The same bytes with the checksum made to agree with them again, so that
    /// the check a test is aiming at is the one that refuses.
    fn resealed(mut bytes: Vec<u8>) -> Vec<u8> {
        let number = number_of(&bytes).unwrap_or_default();
        let body = bytes.get(PREFIX..).unwrap_or_default().to_vec();
        if let Some(slot) = bytes.get_mut(18..PREFIX) {
            slot.copy_from_slice(&checksum(number, &body).to_be_bytes());
        }
        bytes
    }

    #[test]
    fn a_count_a_stranger_chose_is_never_believed() {
        // The third field of the body is how many requests follow.
        for claimed in [u64::MAX, 1 << 40, 4096] {
            let mut written = encode(&a_deed());
            if let Some(slot) = written.get_mut(PREFIX + 16..PREFIX + 24) {
                slot.copy_from_slice(&claimed.to_be_bytes());
            }
            let refused = decode(&resealed(written)).expect_err("a count nothing backs");
            assert!(
                refused.why.contains("bytes left") || refused.why.contains("stop"),
                "a count of {claimed} was refused for the wrong reason: {refused}"
            );
        }
    }

    #[test]
    fn a_tag_this_engine_has_no_meaning_for_is_refused() {
        let deed = a_deed();
        let whole = encode(&deed);
        // The first request's first chain link's tag, which follows the
        // action, the forgotten count, the request count, the moment and the
        // link count.
        let at = PREFIX + 8 + 8 + 8 + 12 + 8;
        let mut written = whole.clone();
        if let Some(byte) = written.get_mut(at) {
            *byte = 9;
        }
        let refused = decode(&resealed(written)).expect_err("a cause that is not one");
        assert!(
            refused.why.contains("cause this engine does not have"),
            "{refused}"
        );
    }

    /// A request nothing caused is not a thing ADR 0012 § 2 allows, and
    /// [`Chain::links`] promises the same. A file claiming one is refused rather
    /// than read into something that breaks the promise.
    #[test]
    fn a_request_with_no_chain_at_all_is_refused() {
        let mut deed = a_deed();
        if let Some(did) = deed.requests.first_mut() {
            did.chain.clear();
        }
        let refused = decode(&encode(&deed)).expect_err("a request nothing caused");
        assert!(refused.why.contains("nothing caused"), "{refused}");
    }

    #[test]
    fn bytes_appended_after_a_record_make_it_unreadable() {
        let mut written = encode(&a_deed());
        written.extend_from_slice(b"and then something else");
        assert!(
            decode(&written).is_err(),
            "a file half overwritten by another program was read as a record"
        );
    }

    /// A reason for a cut address is one of ours or it is nobody's. A sentence
    /// read off a disk and shown to a person is a sentence somebody else could
    /// have written.
    #[test]
    fn a_reason_this_engine_never_wrote_is_not_shown_as_though_it_had() {
        assert_eq!(
            why_it_was_cut("a data URL is content rather than an address"),
            "a data URL is content rather than an address",
        );
        assert_eq!(
            why_it_was_cut("visit example.com to recover your account"),
            "cut, for a reason this engine did not write",
        );
    }
}
