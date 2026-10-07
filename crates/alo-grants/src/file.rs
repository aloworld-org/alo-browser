/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The grant table as bytes on a disk, and reading those bytes back from a
//! stranger.
//!
//! ADR 0026 § 8 puts the table under ADR 0011 § 4's rules: *"a version, a
//! checksum per record, every length checked before anything is reserved, no
//! arithmetic a hostile number can overflow, and an error returned rather than
//! a panic."* The reading is `alo_net::bytes`, the one hostile-input reader
//! this engine has.
//!
//! ```text
//! "alogrant"  version  count
//! record × count:   length  body  checksum(body)
//!   body = kind, then one of:
//!     1 remembered   key  capability  kept  given  visited  last-used?
//!     2 everywhere   capability
//!     3 history      key  dropped  count  entry × count
//! ```
//!
//! # A table that fails its check grants nothing
//!
//! Every way of failing is the same answer, [`Unreadable`], and the caller
//! sets the whole file aside. There is no partial read: a table believed in
//! part is a table where one flipped byte could turn *don't allow* into
//! *allow*, and the ADR names that the breach rather than the nuisance.
//!
//! # What the checksum is for
//!
//! What every other record's says: a file half written, a flipped byte, a file
//! another program left with our name. It is not a defence against a program
//! running as the person, which can compute it as easily as we can (ADR 0011
//! § 3).

use crate::ask::Refusal;
use crate::capability::Capability;
use crate::grant::{Answer, Ending, Kept, Remembered};
use crate::history::{Entry, Happened, History};
use alo_net::bytes::{Reader, Unreadable, Writer, fingerprint, unreadable};
use alo_net::deed::Link;
use alo_storage::StorageKey;
use std::collections::{BTreeMap, BTreeSet};

/// What the file begins with.
pub const MAGIC: [u8; 8] = *b"alogrant";

/// The format this engine writes. Any other is unreadable: not upgraded and
/// not guessed at, and so set aside for an engine that can read it.
pub const VERSION: u16 = 1;

/// Everything the table keeps on a disk.
///
/// *Allow while this page is open* is not here. It ends with its document, and
/// a restart ends every document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Saved {
    /// Every remembered answer, one per key and capability.
    pub remembered: Vec<Remembered>,
    /// Every capability refused for every site.
    pub everywhere: BTreeSet<Capability>,
    /// Every key's history.
    pub histories: BTreeMap<StorageKey, History>,
}

const REMEMBERED: u8 = 1;
const EVERYWHERE: u8 = 2;
const HISTORY: u8 = 3;

/// The table's file.
pub fn encode(saved: &Saved) -> Vec<u8> {
    let mut records: Vec<Vec<u8>> = Vec::new();
    for row in &saved.remembered {
        let mut body = Writer::default();
        body.tag(REMEMBERED);
        write_key(&mut body, &row.key);
        body.tag(row.capability.tag());
        body.tag(match row.kept {
            Kept::Allowed => 1,
            Kept::Refused => 2,
        });
        body.time(row.given);
        body.time(row.visited);
        body.flag(row.last_used.is_some());
        if let Some(used) = row.last_used {
            body.time(used);
        }
        records.push(body.out);
    }
    for capability in &saved.everywhere {
        let mut body = Writer::default();
        body.tag(EVERYWHERE);
        body.tag(capability.tag());
        records.push(body.out);
    }
    for (key, history) in &saved.histories {
        let mut body = Writer::default();
        body.tag(HISTORY);
        write_key(&mut body, key);
        body.number(history.dropped());
        body.number(history.entries().len() as u64);
        for entry in history.entries() {
            write_entry(&mut body, entry);
        }
        records.push(body.out);
    }

    let mut out = Writer::default();
    for byte in MAGIC {
        out.tag(byte);
    }
    out.small(VERSION);
    out.number(records.len() as u64);
    for body in &records {
        out.bytes(body);
        out.number(fingerprint(body));
    }
    out.out
}

/// The table from its file, or why it is unreadable.
///
/// # Errors
///
/// [`Unreadable`], for anything at all: the wrong magic, an unknown version, a
/// record whose checksum does not match, a length or a count longer than what
/// is there, a tag no table this engine wrote holds, a key no document could
/// have, the same key and capability twice, a history longer than sixty-four,
/// or bytes after the end.
pub fn decode(bytes: &[u8]) -> Result<Saved, Unreadable> {
    let mut reader = Reader::new(bytes);
    let mut found = [0u8; 8];
    for slot in &mut found {
        *slot = reader.tag()?;
    }
    if found != MAGIC {
        return Err(unreadable("a grant table that is not one"));
    }
    let version = reader.small()?;
    if version != VERSION {
        return Err(unreadable(format!(
            "a grant table written in format {version}, and this engine reads {VERSION}"
        )));
    }
    let mut saved = Saved::default();
    let mut rows = BTreeSet::new();
    let how_many = reader.how_many()?;
    for _ in 0..how_many {
        let body = reader.bytes()?;
        if fingerprint(&body) != reader.number()? {
            return Err(unreadable(
                "a grant table record whose checksum does not match what is in it",
            ));
        }
        let mut record = Reader::new(&body);
        match record.tag()? {
            REMEMBERED => {
                let row = read_remembered(&mut record)?;
                if !rows.insert((row.key.clone(), row.capability)) {
                    return Err(unreadable("a grant table that answers one ask twice"));
                }
                saved.remembered.push(row);
            }
            EVERYWHERE => {
                if !saved.everywhere.insert(read_capability(&mut record)?) {
                    return Err(unreadable("a grant table that refuses one thing twice"));
                }
            }
            HISTORY => {
                let (key, history) = read_history(&mut record)?;
                if saved.histories.insert(key, history).is_some() {
                    return Err(unreadable("a grant table with one key's history twice"));
                }
            }
            _ => {
                return Err(unreadable(
                    "a grant table record of no kind this engine writes",
                ));
            }
        }
        if !record.is_done() {
            return Err(unreadable("a grant table record with bytes after its end"));
        }
    }
    if !reader.is_done() {
        return Err(unreadable("a grant table with bytes after its end"));
    }
    Ok(saved)
}

fn write_key(body: &mut Writer, key: &StorageKey) {
    body.text(key.origin());
    body.text(key.site());
    body.text(key.partition());
}

fn read_key(reader: &mut Reader<'_>) -> Result<StorageKey, Unreadable> {
    let origin = reader.text()?;
    let site = reader.text()?;
    let partition = reader.text()?;
    StorageKey::from_parts(origin, site, partition)
        .ok_or_else(|| unreadable("a grant to a key no document could have"))
}

fn read_capability(reader: &mut Reader<'_>) -> Result<Capability, Unreadable> {
    Capability::from_tag(reader.tag()?)
        .ok_or_else(|| unreadable("a capability that is not on the list"))
}

fn read_remembered(reader: &mut Reader<'_>) -> Result<Remembered, Unreadable> {
    let key = read_key(reader)?;
    let capability = read_capability(reader)?;
    let kept = match reader.tag()? {
        1 => Kept::Allowed,
        2 => Kept::Refused,
        _ => return Err(unreadable("a remembered answer that is neither")),
    };
    let given = reader.time()?;
    let visited = reader.time()?;
    let last_used = if reader.flag()? {
        Some(reader.time()?)
    } else {
        None
    };
    Ok(Remembered {
        key,
        capability,
        kept,
        given,
        visited,
        last_used,
    })
}

fn read_history(reader: &mut Reader<'_>) -> Result<(StorageKey, History), Unreadable> {
    let key = read_key(reader)?;
    let dropped = reader.number()?;
    let how_many = reader.how_many()?;
    let mut entries = Vec::new();
    for _ in 0..how_many {
        entries.push(read_entry(reader)?);
    }
    let history = History::from_parts(entries, dropped)
        .ok_or_else(|| unreadable("a history longer than this engine keeps"))?;
    Ok((key, history))
}

fn write_link(body: &mut Writer, link: &Link) {
    match link {
        Link::Person { tab } => {
            body.tag(1);
            body.number(*tab);
        }
        Link::Document { document } => {
            body.tag(2);
            body.number(*document);
        }
        Link::Agent { action, document } => {
            body.tag(3);
            body.number(*action);
            body.number(*document);
        }
    }
}

fn read_link(reader: &mut Reader<'_>) -> Result<Link, Unreadable> {
    Ok(match reader.tag()? {
        1 => Link::Person {
            tab: reader.number()?,
        },
        2 => Link::Document {
            document: reader.number()?,
        },
        3 => Link::Agent {
            action: reader.number()?,
            document: reader.number()?,
        },
        _ => return Err(unreadable("a cause that is none of the three")),
    })
}

fn write_entry(body: &mut Writer, entry: &Entry) {
    body.time(entry.at);
    body.tag(entry.capability.tag());
    match &entry.happened {
        Happened::Asked { by } => {
            body.tag(1);
            write_link(body, by);
        }
        Happened::RefusedWithoutAPrompt { refusal, by } => {
            body.tag(2);
            body.tag(refusal.tag());
            write_link(body, by);
        }
        Happened::Answered { answer, by } => {
            body.tag(3);
            body.tag(answer.tag());
            write_link(body, by);
        }
        Happened::Dismissed { by } => {
            body.tag(4);
            write_link(body, by);
        }
        Happened::UseBegan { by } => {
            body.tag(5);
            write_link(body, by);
        }
        Happened::UseEnded { by } => {
            body.tag(6);
            write_link(body, by);
        }
        Happened::Revoked { by } => {
            body.tag(7);
            write_link(body, by);
        }
        Happened::Ended { ending } => {
            body.tag(8);
            body.tag(ending.tag());
        }
    }
}

fn read_entry(reader: &mut Reader<'_>) -> Result<Entry, Unreadable> {
    let at = reader.time()?;
    let capability = read_capability(reader)?;
    let happened = match reader.tag()? {
        1 => Happened::Asked {
            by: read_link(reader)?,
        },
        2 => Happened::RefusedWithoutAPrompt {
            refusal: Refusal::from_tag(reader.tag()?)
                .ok_or_else(|| unreadable("a refusal by no rule"))?,
            by: read_link(reader)?,
        },
        3 => Happened::Answered {
            answer: Answer::from_tag(reader.tag()?)
                .ok_or_else(|| unreadable("an answer no prompt offers"))?,
            by: read_link(reader)?,
        },
        4 => Happened::Dismissed {
            by: read_link(reader)?,
        },
        5 => Happened::UseBegan {
            by: read_link(reader)?,
        },
        6 => Happened::UseEnded {
            by: read_link(reader)?,
        },
        7 => Happened::Revoked {
            by: read_link(reader)?,
        },
        8 => Happened::Ended {
            ending: Ending::from_tag(reader.tag()?)
                .ok_or_else(|| unreadable("an ending of no kind"))?,
        },
        _ => return Err(unreadable("an entry of no kind this engine records")),
    };
    Ok(Entry {
        at,
        capability,
        happened,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_net::Partition;
    use alo_url::Origin;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn key(document: &str, top_level: &str) -> StorageKey {
        let url = |text: &str| alo_url::parse(text).expect("a URL");
        StorageKey::of(&Origin::of(&url(document)), &Partition::of(&url(top_level))).expect("a key")
    }

    fn at(second: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_700_000_000 + second)
    }

    /// A table with one of everything the format can hold.
    fn a_table() -> Saved {
        let meet = key("https://meet.example/", "https://meet.example/");
        let widget = key("https://widget.example/", "https://news.example/");
        let mut history = History::default();
        let person = Link::Person { tab: 2 };
        let page = Link::Document { document: 9 };
        let agent = Link::Agent {
            action: 4,
            document: 9,
        };
        for (second, happened) in [
            Happened::Asked { by: agent.clone() },
            Happened::RefusedWithoutAPrompt {
                refusal: Refusal::NoGesture,
                by: page.clone(),
            },
            Happened::Answered {
                answer: Answer::OnThisSite,
                by: person.clone(),
            },
            Happened::Dismissed { by: person.clone() },
            Happened::UseBegan { by: page.clone() },
            Happened::UseEnded { by: page },
            Happened::Revoked { by: person },
            Happened::Ended {
                ending: Ending::ClockWentBack,
            },
        ]
        .into_iter()
        .enumerate()
        {
            history.add(Entry {
                at: at(second as u64),
                capability: Capability::Camera,
                happened,
            });
        }
        Saved {
            remembered: vec![
                Remembered {
                    key: meet.clone(),
                    capability: Capability::Camera,
                    kept: Kept::Allowed,
                    given: at(1),
                    visited: at(2),
                    last_used: Some(at(3)),
                },
                Remembered {
                    key: widget,
                    capability: Capability::Notifications,
                    kept: Kept::Refused,
                    given: at(4),
                    visited: at(4),
                    last_used: None,
                },
            ],
            everywhere: [Capability::Location].into_iter().collect(),
            histories: [(meet, history)].into_iter().collect(),
        }
    }

    #[test]
    fn what_went_in_is_what_comes_out() {
        assert_eq!(decode(&encode(&a_table())), Ok(a_table()));
        assert_eq!(decode(&encode(&Saved::default())), Ok(Saved::default()));
    }

    #[test]
    fn every_truncation_is_refused() {
        let whole = encode(&a_table());
        for cut in 0..whole.len() {
            assert!(
                decode(&whole[..cut]).is_err(),
                "a table cut to {cut} of {} bytes was read",
                whole.len()
            );
        }
    }

    #[test]
    fn a_single_flipped_byte_anywhere_is_refused() {
        let whole = encode(&a_table());
        for at in 0..whole.len() {
            let mut damaged = whole.clone();
            damaged[at] ^= 0xff;
            assert!(
                decode(&damaged).is_err(),
                "byte {at} flipped and the table was still read"
            );
        }
    }

    #[test]
    fn a_version_this_engine_does_not_know_is_refused() {
        let mut written = encode(&a_table());
        written[8..10].copy_from_slice(&2u16.to_be_bytes());
        let refused = decode(&written).expect_err("a future format");
        assert!(refused.why.contains("format 2"), "{refused}");
    }

    /// A table whose records are these bodies, each sealed honestly, so that
    /// the check a test aims at is the one that refuses.
    fn sealed(bodies: &[Vec<u8>]) -> Vec<u8> {
        let mut out = Writer::default();
        for byte in MAGIC {
            out.tag(byte);
        }
        out.small(VERSION);
        out.number(bodies.len() as u64);
        for body in bodies {
            out.bytes(body);
            out.number(fingerprint(body));
        }
        out.out
    }

    fn body(build: impl FnOnce(&mut Writer)) -> Vec<u8> {
        let mut writer = Writer::default();
        build(&mut writer);
        writer.out
    }

    #[test]
    fn honestly_sealed_records_no_engine_wrote_are_still_refused() {
        let meet = key("https://meet.example/", "https://meet.example/");
        let a_row = |capability: u8, kept: u8| {
            body(|w| {
                w.tag(REMEMBERED);
                write_key(w, &meet);
                w.tag(capability);
                w.tag(kept);
                w.time(at(0));
                w.time(at(0));
                w.flag(false);
            })
        };
        assert!(decode(&sealed(&[a_row(1, 1)])).is_ok());
        for (why, bodies) in [
            ("a capability off the list", vec![a_row(9, 1)]),
            ("a third kind of remembered answer", vec![a_row(1, 3)]),
            ("one ask answered twice", vec![a_row(1, 1), a_row(1, 2)]),
            (
                "a record of no kind",
                vec![body(|w| {
                    w.tag(4);
                })],
            ),
            (
                "an opaque origin's key",
                vec![body(|w| {
                    w.tag(REMEMBERED);
                    w.text("null");
                    w.text("example.com");
                    w.text("example.com");
                    w.tag(1);
                    w.tag(1);
                    w.time(at(0));
                    w.time(at(0));
                    w.flag(false);
                })],
            ),
            (
                "one refusal everywhere twice",
                vec![
                    body(|w| {
                        w.tag(EVERYWHERE);
                        w.tag(1);
                    }),
                    body(|w| {
                        w.tag(EVERYWHERE);
                        w.tag(1);
                    }),
                ],
            ),
            (
                "bytes after a record's end",
                vec![body(|w| {
                    w.tag(EVERYWHERE);
                    w.tag(1);
                    w.tag(0);
                })],
            ),
            (
                "a history of sixty-five",
                vec![body(|w| {
                    w.tag(HISTORY);
                    write_key(w, &meet);
                    w.number(0);
                    w.number(65);
                    for _ in 0..65 {
                        w.time(at(0));
                        w.tag(1);
                        w.tag(8);
                        w.tag(1);
                    }
                })],
            ),
            (
                "a cause that is none of the three",
                vec![body(|w| {
                    w.tag(HISTORY);
                    write_key(w, &meet);
                    w.number(0);
                    w.number(1);
                    w.time(at(0));
                    w.tag(1);
                    w.tag(1);
                    w.tag(4);
                    w.number(0);
                })],
            ),
        ] {
            assert!(decode(&sealed(&bodies)).is_err(), "{why} was read");
        }
    }

    #[test]
    fn a_length_or_a_count_a_stranger_chose_is_never_believed() {
        for claimed in [u64::MAX, 1 << 40, u64::from(u32::MAX)] {
            // The record count.
            let mut written = encode(&a_table());
            written[10..18].copy_from_slice(&claimed.to_be_bytes());
            assert!(
                decode(&written).is_err(),
                "a count of {claimed} was believed"
            );
            // The first record's length.
            let mut written = encode(&a_table());
            written[18..26].copy_from_slice(&claimed.to_be_bytes());
            assert!(
                decode(&written).is_err(),
                "a length of {claimed} was believed"
            );
            // A history's entry count, inside an honestly sealed record.
            let lying = body(|w| {
                w.tag(HISTORY);
                write_key(w, &key("https://a.example/", "https://a.example/"));
                w.number(0);
                w.number(claimed);
            });
            assert!(decode(&sealed(&[lying])).is_err());
        }
    }

    #[test]
    fn a_file_that_is_not_a_table_at_all_is_refused() {
        assert!(decode(b"").is_err());
        assert!(decode(b"\x89PNG\r\n\x1a\n and then a picture").is_err());
        assert!(decode(&[0xff; 64]).is_err());
        assert!(decode(b"alostore and the rest of a bucket").is_err());
    }
}
