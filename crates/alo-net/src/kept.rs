/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What an agent did, kept until the person deletes it.
//!
//! ADR 0012 § 6's second half. [`crate::activity`] is everything, for the
//! session, in memory; this is the small durable half beside it: *"only requests
//! whose cause chain reaches an agent action, plus the action and its outcome. A
//! person's own browsing is not in it."*
//!
//! [`crate::deed`] is the bytes of one action's file; this is the directory of
//! them, the policy, the bound, and the one way a line gets in. The same
//! division as [`crate::record`] and [`crate::disk`], for the same reason.
//!
//! # Why it exists at all, and why it is this small
//!
//! *"A record that vanishes when the browser closes cannot answer **what did it
//! do while I was not watching**, and that question is the entire reason
//! `alo-os` ADR 0001 records anything at all."* So this is the one place in the
//! browser that deliberately keeps a durable browsing record — and what makes it
//! affordable is that it is small on purpose. Everything a person did
//! themselves is in [`crate::activity`] and dies with the process.
//!
//! # Never opened, rather than opened and emptied
//!
//! ADR 0011 § 2's rule, taken unchanged for this file: a session-scoped profile
//! does not get one. Not a record that is deleted when the window closes — a
//! record that was never created, because *"a file that was deleted is a file
//! that was on the disk: recoverable, and present for the whole window between
//! the two operations, which is precisely the window a crash or a power cut
//! lands in."*
//!
//! That is kept by there being **no default**: [`crate::Pool`] holds
//! [`Option<Kept>`] and starts with [`None`], exactly as
//! [`crate::cache::Cache::new`] starts with no disk. Private browsing is not a
//! flag anything here reads; it is a [`Kept`] nobody made.
//!
//! # It is not kept where the system keeps caches
//!
//! ADR 0012 § 6 says *"under ADR 0011 section 3's rules unchanged"*, and section
//! 3 says *"in the place the operating system keeps caches"*. The rules are
//! taken unchanged; the **place** is not, and the difference is the point: an
//! operating system deletes caches when a disk fills up, and it is right to,
//! because everything in a cache can be fetched again. Nothing here can. A
//! record of what an agent did while nobody was watching, quietly removed by the
//! system on a Tuesday, is the failure this whole decision exists to prevent —
//! so it lives where the system keeps things it is not entitled to delete. See
//! [`where_the_system_keeps_records`].
//!
//! # Bounded in actions, and the reason it is not bytes
//!
//! ADR 0012 § 6: *"the bound is counted in actions, not bytes. When it is
//! reached the oldest actions go whole. An agent's most recent work is the thing
//! somebody is most likely to be asking about, and a bound in bytes would let one
//! action with three hundred requests in it evict a week of ordinary ones."*
//!
//! So [`MOST_ACTIONS`] is the bound, and the oldest action's file goes whole.
//! One action is *also* bounded in itself ([`MOST_PER_ACTION`],
//! [`LARGEST_DEED`]) — not as a second policy, but because a bound counted in
//! actions is not a bound on a disk unless one action has a size. Within an
//! action the **earliest** requests are kept and the rest are counted
//! ([`crate::deed::Deed::forgotten`]): the first requests are the ones that say
//! what the action did, and the three hundredth picture is not.
//!
//! # An action from an earlier session is never added to
//!
//! ADR 0003's ids are minted once per browser process, so `action#0` exists in
//! every session that had one. A record that filed this morning's `action#0`
//! into last week's file would join two unrelated pieces of somebody's history
//! into one story — the exact failure ADR 0003 exists to prevent. So an action
//! is matched to a file **only within the session that minted it**, and every
//! session's actions get files of their own. What names an action across
//! sessions is [`crate::deed::Deed::number`], which counts up on the disk.
//!
//! # Who may read it
//!
//! ADR 0012 § 7, unchanged and kept the same way [`crate::activity`] keeps it:
//! the browser process only. A renderer holds no [`crate::Pool`], nothing
//! crossing the process boundary carries any of this, and `alo-agent` does not
//! depend on this crate — *"the record is about the agent and kept for the
//! person"*.

use crate::activity::Activity;
use crate::chain::Documents;
use crate::deed::{self, Deed, Did};
use crate::private::{make_the_directory_private, read_at_most, write_privately};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// How many agent actions are kept at once.
///
/// **A choice rather than a measurement**, in the shape every bound in
/// `alo-net` is made: a limit somebody else chooses is not a limit. Set where
/// weeks of an agent's ordinary work fit inside it, since *what did it do while
/// I was not watching* is a question asked about days rather than about minutes.
pub const MOST_ACTIONS: usize = 500;

/// How many requests are kept for one action.
///
/// The earliest ones, and the rest are counted rather than kept — see this
/// module's own documentation for why that is the useful half.
pub const MOST_PER_ACTION: usize = 64;

/// The most bytes one action's file may be.
///
/// It is also the bound on what is **read back**: a file in the directory larger
/// than this is not opened at all, whoever put it there. With
/// [`MOST_PER_ACTION`] and [`crate::deed::LONGEST_URL`] it is what makes a bound
/// counted in actions a bound on a disk as well.
pub const LARGEST_DEED: u64 = 256 * 1024;

/// What a file holding one action is called.
const DEED: &str = "deed";

/// What a file being written is called until the rename makes it a record.
const WRITING: &str = "writing";

/// The durable record of what an agent did.
///
/// Holding one of these is what makes an agent's work survive a restart. **Not
/// holding one is what makes a session-scoped profile leave nothing behind**,
/// and that is the whole of how private browsing is implemented here.
#[derive(Debug)]
pub struct Kept {
    directory: PathBuf,
    /// The file each action's record is in, by its durable number, so the
    /// oldest can go first. The clock is not involved: numbers count up.
    held: BTreeMap<u64, String>,
    /// The number the next action written gets.
    next: u64,
    /// Which file each of **this session's** actions is in. Empty at start-up,
    /// deliberately — see this module's own documentation.
    mine: HashMap<u64, u64>,
    /// The bound, in actions. A value rather than the constant because a bound
    /// *is* a value, and because a test wants one it can reach.
    most: usize,
    /// How far through [`Activity`] this has taken lines.
    taken: u64,
    /// Lines this could not account for: made before it was opened, or dropped
    /// under the session record's own bound before it swept.
    ///
    /// Counted rather than left implicit, for [`Activity::forgotten`]'s reason:
    /// a record that quietly had holes in it would read as a session in which
    /// less happened. It is about **this session's** sweeping rather than about
    /// the disk, so it is not written to one.
    missed: u64,
    /// Files in the directory that are ours and did not read.
    ///
    /// A gap rather than an error: ADR 0011 § 4's *"an unreadable entry is a
    /// miss"*, with the consequence changed to suit what this holds. A cache
    /// miss is fetched again; a record nobody can read is a piece of somebody's
    /// history that is not there, and the honest thing is to say how many.
    unreadable: usize,
}

impl Kept {
    /// Open the record at this path, creating it private to its owner.
    ///
    /// Whatever is already there is read: that is what *survives a restart*
    /// means.
    ///
    /// # Errors
    ///
    /// A sentence, when the directory cannot be made, cannot be made private, or
    /// cannot be listed. A caller that gets one should say so rather than run
    /// without a record: a browser that silently stopped keeping what an agent
    /// did would be answering *what did it do* with a shrug.
    pub fn at(directory: impl AsRef<Path>) -> Result<Self, String> {
        let directory = directory.as_ref().to_path_buf();
        make_the_directory_private(&directory)?;
        let mut kept = Self {
            directory,
            held: BTreeMap::new(),
            next: 0,
            mine: HashMap::new(),
            most: MOST_ACTIONS,
            taken: 0,
            missed: 0,
            unreadable: 0,
        };
        kept.read_what_is_there()?;
        Ok(kept)
    }

    /// The same record, holding at most this many actions.
    ///
    /// A bound below what is already held is applied at once rather than argued
    /// with, the same as [`crate::disk::Disk::bounded_to`].
    #[must_use]
    pub fn bounded_to(mut self, actions: usize) -> Self {
        self.most = actions;
        self.stay_within_bounds();
        self
    }

    /// Take from the session's record everything that followed from an agent's
    /// action, and write it down.
    ///
    /// **The only way a line gets in**, and it takes the session's record and the
    /// browser process's own documents rather than a line anybody composed — so
    /// a durable line is exactly as unforgeable as a session line, which is
    /// ADR 0012 § 4. Nothing here trusts a caller to say what followed from
    /// what: the walk is made here, against [`Documents`].
    ///
    /// # Why this is taken rather than written as it happens
    ///
    /// The two halves of the answer live in two objects. The requests are in
    /// [`Activity`], which [`crate::Pool`] holds because a pool is what a session
    /// holds; what caused each document's load is in [`Documents`], which the
    /// browser process holds because ADR 0012 § 4 says a renderer may never state
    /// one. **Deciding whether a request followed from an action needs both at
    /// once**, and putting a copy of either beside the other is precisely the
    /// side table ADR 0012 § 3 refuses.
    ///
    /// So the browser process brings them together, which is the one thing it is
    /// for. It is idempotent — lines are taken by [`crate::activity::Entry::sequence`]
    /// and never twice — so it may be called after every load, and
    /// [`Kept::missed`] says how many lines went by uncounted if it was not.
    pub fn take_from(&mut self, record: &Activity, documents: &Documents) {
        if let Some(first) = record.entries().next() {
            // Lines that were made and are no longer in the session's record:
            // dropped under its bound, or made before this record was opened.
            // From here those are one thing — requests it cannot account for.
            self.missed = self.missed.saturating_add(
                first
                    .sequence()
                    .saturating_sub(self.taken.saturating_add(1)),
            );
        }
        for entry in record.entries() {
            if entry.sequence() <= self.taken {
                continue;
            }
            self.taken = entry.sequence();
            let chain = entry.chain(documents);
            // The whole of what makes this record small, decided **here and
            // nowhere else**: a person's own browsing reaches no action, so it
            // is never written down. [`Kept::keep`] takes the action as an
            // argument rather than asking a second time, because a rule this
            // important asked in two places is a rule that can come to be
            // answered differently in one of them.
            let Some(action) = chain.action() else {
                continue;
            };
            self.keep(action.as_u64(), &Did::of(entry, &chain));
        }
    }

    /// Everything held, oldest action first.
    ///
    /// Read from the disk each time rather than from anything held in memory,
    /// because the disk is where it is: a copy kept beside it is a second answer
    /// that can come to disagree.
    ///
    /// A file that does not read is skipped and counted ([`Kept::unreadable`]),
    /// never an error: a record with a gap in it is worth having and a record
    /// that refuses to open is not.
    pub fn all(&mut self) -> Vec<Deed> {
        let mut found = Vec::new();
        for number in self.held.keys().copied().collect::<Vec<u64>>() {
            match self.read(number) {
                Some(deed) => found.push(deed),
                None => self.unreadable = self.unreadable.saturating_add(1),
            }
        }
        found
    }

    /// One action's record, by its durable number, if it reads.
    pub fn read(&self, number: u64) -> Option<Deed> {
        let file = self.held.get(&number)?;
        let bytes = read_at_most(&self.directory.join(file), LARGEST_DEED)?;
        match deed::decode(&bytes) {
            Ok(found) if found.number == number => Some(found),
            Ok(_) | Err(_) => None,
        }
    }

    /// How many actions are held.
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether none are.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Where this is, for a person who wants to look at it or delete it
    /// themselves.
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// How many of this session's requests this record cannot account for.
    pub fn missed(&self) -> u64 {
        self.missed
    }

    /// How many files of ours were found and could not be read.
    pub fn unreadable(&self) -> usize {
        self.unreadable
    }

    /// Delete all of it.
    ///
    /// ADR 0012 § 7: *"it is theirs, it is deletable, and deleting it is real —
    /// the file, not a flag on it."* What this session had taken is forgotten
    /// with it, so that a request already written down is not written again the
    /// next time this sweeps.
    pub fn empty(&mut self) {
        for file in std::mem::take(&mut self.held).values() {
            let _ = fs::remove_file(self.directory.join(file));
        }
        self.mine.clear();
        self.missed = 0;
        self.unreadable = 0;
    }

    /// Write one request down, in its action's file.
    ///
    /// The action is the caller's answer rather than this function's, for the
    /// reason [`Kept::take_from`] gives where it works it out.
    fn keep(&mut self, action: u64, did: &Did) {
        let number = if let Some(number) = self.mine.get(&action) {
            *number
        } else {
            let number = self.next;
            self.next = self.next.saturating_add(1);
            self.mine.insert(action, number);
            number
        };
        let mut deed = self.read(number).unwrap_or(Deed {
            number,
            was: action,
            forgotten: 0,
            requests: Vec::new(),
        });
        if deed.requests.len() >= MOST_PER_ACTION {
            deed.forgotten = deed.forgotten.saturating_add(1);
        } else {
            deed.requests.push(did.clone());
        }
        let mut bytes = deed::encode(&deed);
        if bytes.len() as u64 > LARGEST_DEED {
            // The request that would have taken the file past its size is
            // counted instead of kept, which is the same answer the count bound
            // gives and is reached by a different road: a few enormous
            // addresses rather than many ordinary ones.
            deed.requests.pop();
            deed.forgotten = deed.forgotten.saturating_add(1);
            bytes = deed::encode(&deed);
        }
        self.write(number, &bytes);
    }

    /// Put these bytes in this action's file, replacing what was there.
    fn write(&mut self, number: u64, bytes: &[u8]) {
        let file = file_for(number);
        let being_written = self.directory.join(format!("{file}.{WRITING}"));
        // Written beside it and renamed over it, so that a power cut leaves
        // either the old record or the new one and never half of either.
        if write_privately(&being_written, bytes).is_err() {
            let _ = fs::remove_file(&being_written);
            return;
        }
        if fs::rename(&being_written, self.directory.join(&file)).is_err() {
            let _ = fs::remove_file(&being_written);
            return;
        }
        self.held.insert(number, file);
        self.stay_within_bounds();
    }

    /// Oldest actions go whole, until the bound is met.
    fn stay_within_bounds(&mut self) {
        while self.held.len() > self.most {
            let Some(oldest) = self.held.keys().next().copied() else {
                break;
            };
            let Some(file) = self.held.remove(&oldest) else {
                break;
            };
            let _ = fs::remove_file(self.directory.join(file));
            // An action whose file has gone must not be appended to again: the
            // next request that followed from it starts a file of its own,
            // rather than resurrecting one the bound decided to drop.
            self.mine.retain(|_, number| *number != oldest);
        }
    }

    /// What is already in the directory, put back in the order it was written.
    fn read_what_is_there(&mut self) -> Result<(), String> {
        let listing = fs::read_dir(&self.directory)
            .map_err(|why| format!("the record directory cannot be listed: {why}"))?;
        let mut found: Vec<(u64, String)> = Vec::new();
        for entry in listing {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            // A file left behind by a crash between writing and renaming. It was
            // never a record, and nothing will ever finish it.
            if path.extension().is_some_and(|kind| kind == WRITING) {
                let _ = fs::remove_file(&path);
                continue;
            }
            // Anything else in the directory belongs to somebody else and is
            // left exactly as it is.
            if path.extension().is_none_or(|kind| kind != DEED) {
                continue;
            }
            let Ok(about) = entry.metadata() else {
                continue;
            };
            if !about.is_file() || about.len() > LARGEST_DEED {
                self.unreadable = self.unreadable.saturating_add(1);
                continue;
            }
            match prefix_of(&path) {
                Some(number) => found.push((number, name.to_owned())),
                // One of ours that is not readable as one of ours: a version we
                // do not know, a half-finished write, or a name collision with
                // something else. **Kept where it is** — unlike a cache entry,
                // which is deleted because it can never be served. This is
                // somebody's record, we are the ones who cannot read it, and a
                // later version of this engine may be able to.
                None => self.unreadable = self.unreadable.saturating_add(1),
            }
        }
        for (number, file) in found {
            if let Some(displaced) = self.held.insert(number, file) {
                // Two files claiming one place in the order can only be a file
                // somebody else wrote. Neither is deleted, for the reason above.
                let _ = displaced;
                self.unreadable = self.unreadable.saturating_add(1);
            }
            self.next = self.next.max(number.saturating_add(1));
        }
        self.stay_within_bounds();
        Ok(())
    }
}

/// Where this operating system keeps things it does not delete on a person's
/// behalf, and our directory inside it.
///
/// **Not the cache directory**, and that is the one place ADR 0012 § 6's *"ADR
/// 0011 section 3's rules unchanged"* does not mean the same path: a system is
/// entitled to empty a cache when a disk fills, because everything in one can be
/// fetched again. Nothing here can. See this module's own documentation.
///
/// [`None`] when there is nowhere to put it — no home directory, or a profile
/// name that is not a single plain path component.
pub fn where_the_system_keeps_records(profile: &str) -> Option<PathBuf> {
    let home = crate::private::home()?;
    let place = if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .filter(|set| !set.is_empty())
            .map_or_else(|| home.join(".local").join("share"), PathBuf::from)
    };
    crate::private::for_profile(&place, profile, "agent")
}

/// What the file holding this action is called.
///
/// The durable number, in hexadecimal, so the directory reads in the order it
/// was written. Unlike a cache file's name this is not a hash of anything: there
/// is nothing here a URL could name, and a number that counts up is the whole of
/// what a name has to say.
fn file_for(number: u64) -> String {
    format!("{number:016x}.{DEED}")
}

/// The durable number in a file's prefix, without reading the rest of it.
fn prefix_of(path: &Path) -> Option<u64> {
    let mut file = fs::File::open(path).ok()?;
    let mut prefix = [0u8; deed::PREFIX];
    file.read_exact(&mut prefix).ok()?;
    deed::number_of(&prefix).ok()
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use crate::activity::Happened;
    use crate::cause::{ActionId, Cause, DocumentId, Identities};
    use crate::deed::LONGEST_URL;
    use crate::request::{Purpose, Request};
    use crate::response::Status;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    /// A directory of this test's own, in the place the machine keeps temporary
    /// things. Named after the caller so two tests never share one.
    fn somewhere(called: &str) -> PathBuf {
        let place = std::env::temp_dir().join(format!(
            "alo-kept-{}-{called}",
            std::process::id().wrapping_mul(2_654_435_761)
        ));
        let _ = fs::remove_dir_all(&place);
        place
    }

    fn moment(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn url(text: &str) -> alo_url::Url {
        match alo_url::parse(text) {
            Ok(url) => url,
            Err(why) => panic!("{text} is not a URL: {why}"),
        }
    }

    fn answered() -> Happened {
        Happened::Answered {
            status: Status::OK,
            whole: true,
        }
    }

    /// A browser process in the middle of an agent's work: a page the person
    /// opened, an action taken in it, and a page that action opened.
    struct Browsing {
        minting: Identities,
        documents: Documents,
        record: Activity,
        theirs: DocumentId,
        action: ActionId,
        its: DocumentId,
        at: u64,
    }

    impl Browsing {
        fn new() -> Self {
            let mut minting = Identities::default();
            let mut documents = Documents::default();
            let tab = minting.a_tab();
            let theirs = documents.opened(&mut minting, Cause::Person { tab });
            let action = minting.an_action();
            let its = documents.opened(
                &mut minting,
                Cause::Agent {
                    action,
                    document: theirs,
                },
            );
            Self {
                minting,
                documents,
                record: Activity::new(),
                theirs,
                action,
                its,
                at: 0,
            }
        }

        /// One request, by whatever caused it, written into the session record.
        fn fetched(&mut self, address: &str, cause: Cause) {
            self.at = self.at.saturating_add(1);
            self.record.happened(
                &Request::get(url(address), cause).for_purpose(Purpose::Script),
                moment(self.at),
                answered(),
            );
        }

        /// A second action, in the page the first one opened.
        fn acts_again(&mut self) -> ActionId {
            self.minting.an_action()
        }
    }

    // --- Only what reaches an agent action (ADR 0012 § 6) ---------------------

    /// The claim in one test: what the agent set off is written down, and the
    /// person's own browsing is not. A record that kept both would be the
    /// surveillance file ADR 0012 refuses by name.
    #[test]
    fn an_agents_work_is_kept_and_a_persons_own_browsing_is_not() {
        let place = somewhere("only-the-agents");
        let mut browsing = Browsing::new();
        let mut kept = Kept::at(&place).expect("a record");

        browsing.fetched(
            "https://example.com/theirs.css",
            Cause::Document {
                document: browsing.theirs,
            },
        );
        browsing.fetched(
            "https://example.com/its.js",
            Cause::Document {
                document: browsing.its,
            },
        );
        kept.take_from(&browsing.record, &browsing.documents);

        let held = kept.all();
        assert_eq!(held.len(), 1, "one action, one file: {held:?}");
        let Some(deed) = held.first() else {
            panic!("nothing was kept");
        };
        assert_eq!(deed.requests.len(), 1);
        assert_eq!(deed.was, browsing.action.as_u64());
        let said = deed.to_string();
        assert!(said.contains("action#0"), "{said}");
        let whole = format!("{deed:?}");
        assert!(
            !whole.contains("theirs.css"),
            "a person's own browsing was written to a disk: {whole}"
        );
        assert!(whole.contains("its.js"), "{whole}");
        let _ = fs::remove_dir_all(&place);
    }

    /// The chain is frozen, which is the thing this record cannot inherit from
    /// the session's. It says what caused what without any documents to walk.
    #[test]
    fn what_is_kept_says_the_whole_chain_without_anything_to_walk() {
        let place = somewhere("frozen");
        let mut browsing = Browsing::new();
        let mut kept = Kept::at(&place).expect("a record");
        browsing.fetched(
            "https://example.com/its.js",
            Cause::Document {
                document: browsing.its,
            },
        );
        kept.take_from(&browsing.record, &browsing.documents);

        // Everything a walk would have needed, gone.
        drop(browsing.documents);

        let held = kept.all();
        let Some(did) = held.first().and_then(|deed| deed.requests.first()) else {
            panic!("nothing was kept");
        };
        assert_eq!(
            did.to_string(),
            "GET https://example.com/its.js (script), caused by document#1, \
             caused by action#0, in document#0, caused by the person, in tab#0 — answered 200",
        );
        assert_eq!(did.action(), Some(0));
        assert!(did.ended.is_whole(), "the far end is the person who asked");
        let _ = fs::remove_dir_all(&place);
    }

    /// Two actions are two files, so the bound can drop the older one whole.
    #[test]
    fn each_action_is_a_file_of_its_own() {
        let place = somewhere("apart");
        let mut browsing = Browsing::new();
        let mut kept = Kept::at(&place).expect("a record");
        let second = browsing.acts_again();

        browsing.fetched(
            "https://example.com/first.js",
            Cause::Agent {
                action: browsing.action,
                document: browsing.theirs,
            },
        );
        browsing.fetched(
            "https://example.com/second.js",
            Cause::Agent {
                action: second,
                document: browsing.its,
            },
        );
        kept.take_from(&browsing.record, &browsing.documents);

        assert_eq!(kept.len(), 2);
        let held = kept.all();
        assert_eq!(
            held.iter().map(|deed| deed.was).collect::<Vec<u64>>(),
            vec![browsing.action.as_u64(), second.as_u64()],
            "oldest first",
        );
        let _ = fs::remove_dir_all(&place);
    }

    // --- It survives a restart -----------------------------------------------

    #[test]
    fn what_was_written_is_there_when_the_directory_is_opened_again() {
        let place = somewhere("restart");
        let mut browsing = Browsing::new();
        {
            let mut kept = Kept::at(&place).expect("a record");
            browsing.fetched(
                "https://example.com/its.js",
                Cause::Document {
                    document: browsing.its,
                },
            );
            kept.take_from(&browsing.record, &browsing.documents);
            assert_eq!(kept.len(), 1);
        }

        let mut reopened = Kept::at(&place).expect("the same directory");
        let held = reopened.all();
        assert_eq!(held.len(), 1, "an agent's work did not survive a restart");
        assert!(
            held.first()
                .and_then(|deed| deed.requests.first())
                .is_some_and(|did| did.url.said().ends_with("its.js")),
            "{held:?}",
        );
        assert_eq!(reopened.unreadable(), 0);
        let _ = fs::remove_dir_all(&place);
    }

    /// This morning's `action#0` is not last week's. A record that appended to
    /// the old file would join two unrelated pieces of somebody's history into
    /// one story, which is ADR 0003's whole reason for existing.
    #[test]
    fn an_action_from_an_earlier_session_is_never_added_to() {
        let place = somewhere("sessions");
        {
            let mut first = Browsing::new();
            let mut kept = Kept::at(&place).expect("a record");
            first.fetched(
                "https://example.com/monday.js",
                Cause::Document {
                    document: first.its,
                },
            );
            kept.take_from(&first.record, &first.documents);
        }

        let mut second = Browsing::new();
        let mut kept = Kept::at(&place).expect("the same directory");
        second.fetched(
            "https://example.com/tuesday.js",
            Cause::Document {
                document: second.its,
            },
        );
        kept.take_from(&second.record, &second.documents);

        let held = kept.all();
        assert_eq!(held.len(), 2, "one session's action joined another's");
        assert!(held.iter().all(|deed| deed.requests.len() == 1), "{held:?}");
        assert_eq!(
            held.iter().map(|deed| deed.was).collect::<Vec<u64>>(),
            vec![0, 0],
            "both sessions called it action#0, and they are two records",
        );
        let _ = fs::remove_dir_all(&place);
    }

    // --- Bounded in actions (ADR 0012 § 6) -----------------------------------

    /// The bound the ADR asks for by name: the oldest **actions** go whole, and
    /// one busy action cannot evict a week of ordinary ones.
    #[test]
    fn the_oldest_actions_go_whole_and_a_busy_one_evicts_nothing() {
        let place = somewhere("bounded");
        let mut browsing = Browsing::new();
        let mut kept = Kept::at(&place).expect("a record").bounded_to(3);

        // One action with a great many requests in it, and then four more
        // ordinary ones.
        for asked in 0..50 {
            browsing.fetched(
                &format!("https://example.com/busy/{asked}"),
                Cause::Agent {
                    action: browsing.action,
                    document: browsing.theirs,
                },
            );
        }
        let mut ordinary = Vec::new();
        for _ in 0..4 {
            let action = browsing.acts_again();
            ordinary.push(action);
            browsing.fetched(
                "https://example.com/ordinary.js",
                Cause::Agent {
                    action,
                    document: browsing.its,
                },
            );
        }
        kept.take_from(&browsing.record, &browsing.documents);

        assert_eq!(kept.len(), 3, "the bound is not a bound");
        let held = kept.all();
        assert_eq!(
            held.iter().map(|deed| deed.was).collect::<Vec<u64>>(),
            ordinary
                .get(1..)
                .unwrap_or_default()
                .iter()
                .map(|action| action.as_u64())
                .collect::<Vec<u64>>(),
            "the busy action and the oldest ordinary one went, and the rest stayed",
        );
        let _ = fs::remove_dir_all(&place);
    }

    /// One action still has a size, or a bound counted in actions is not a
    /// bound on a disk. What is kept is the **earliest** requests, and the rest
    /// are counted rather than silently missing.
    #[test]
    fn one_action_keeps_its_first_requests_and_counts_the_rest() {
        let place = somewhere("busy");
        let mut browsing = Browsing::new();
        let mut kept = Kept::at(&place).expect("a record");
        for asked in 0..(MOST_PER_ACTION + 20) {
            browsing.fetched(
                &format!("https://example.com/{asked}"),
                Cause::Agent {
                    action: browsing.action,
                    document: browsing.theirs,
                },
            );
        }
        kept.take_from(&browsing.record, &browsing.documents);

        let held = kept.all();
        let Some(deed) = held.first() else {
            panic!("nothing was kept");
        };
        assert_eq!(deed.requests.len(), MOST_PER_ACTION);
        assert_eq!(deed.forgotten, 20, "it shortened itself quietly");
        assert!(
            deed.requests
                .first()
                .is_some_and(|did| did.url.said().ends_with("/0")),
            "the earliest request went instead of the latest: {deed:?}",
        );
        assert!(deed.to_string().contains("20 no longer kept"), "{deed}");
        let _ = fs::remove_dir_all(&place);
    }

    /// A page chooses how long its addresses are, so the file bound is reached
    /// by a road the count bound does not cover.
    #[test]
    fn enormous_addresses_cannot_grow_one_action_past_its_size() {
        let place = somewhere("enormous");
        let mut browsing = Browsing::new();
        let mut kept = Kept::at(&place).expect("a record");
        let enormous = format!("https://example.com/{}", "a".repeat(LONGEST_URL * 4));
        for _ in 0..MOST_PER_ACTION {
            browsing.fetched(
                &enormous,
                Cause::Agent {
                    action: browsing.action,
                    document: browsing.theirs,
                },
            );
        }
        kept.take_from(&browsing.record, &browsing.documents);

        let file = place.join(file_for(0));
        let about = fs::metadata(&file).expect("the file");
        assert!(about.len() <= LARGEST_DEED, "{} bytes", about.len());
        let held = kept.all();
        let Some(deed) = held.first() else {
            panic!("nothing was kept");
        };
        assert!(
            deed.requests.first().is_some_and(|did| !did.url.is_whole()),
            "an address a page chose the length of was kept whole",
        );
        let _ = fs::remove_dir_all(&place);
    }

    // --- Read back from a stranger (ADR 0011 § 4) ----------------------------

    /// The item's own closing clause: a file that does not read is a record
    /// with a gap rather than an error. The rest still reads, and the record
    /// says how many it could not.
    #[test]
    fn a_file_that_does_not_read_is_a_gap_and_the_rest_still_reads() {
        let place = somewhere("gap");
        let mut browsing = Browsing::new();
        let mut kept = Kept::at(&place).expect("a record");
        let second = browsing.acts_again();
        browsing.fetched(
            "https://example.com/first.js",
            Cause::Agent {
                action: browsing.action,
                document: browsing.theirs,
            },
        );
        browsing.fetched(
            "https://example.com/second.js",
            Cause::Agent {
                action: second,
                document: browsing.its,
            },
        );
        kept.take_from(&browsing.record, &browsing.documents);
        drop(kept);

        // Somebody else's bytes, over the first action's file.
        fs::write(place.join(file_for(0)), b"not a record, and never was").expect("a write");

        let mut reopened = Kept::at(&place).expect("the same directory");
        let held = reopened.all();
        assert_eq!(held.len(), 1, "the readable one did not read: {held:?}");
        assert!(
            held.first()
                .and_then(|deed| deed.requests.first())
                .is_some_and(|did| did.url.said().ends_with("second.js")),
            "{held:?}",
        );
        assert_eq!(reopened.unreadable(), 1, "the gap was not counted");
        assert!(
            place.join(file_for(0)).exists(),
            "somebody's record was deleted because this engine could not read it",
        );
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn a_file_somebody_else_put_there_is_left_alone() {
        let place = somewhere("theirs");
        Kept::at(&place).expect("a record");
        let theirs = place.join("notes.txt");
        fs::write(&theirs, b"somebody else's file").expect("a write");

        let reopened = Kept::at(&place).expect("the same directory");
        assert_eq!(reopened.len(), 0);
        assert_eq!(
            reopened.unreadable(),
            0,
            "a file that is not ours is not a gap"
        );
        assert!(theirs.exists(), "a file that is not ours was deleted");
        let _ = fs::remove_dir_all(&place);
    }

    #[test]
    fn a_half_written_file_is_never_a_record() {
        let place = somewhere("half");
        let mut browsing = Browsing::new();
        {
            let mut kept = Kept::at(&place).expect("a record");
            browsing.fetched(
                "https://example.com/its.js",
                Cause::Document {
                    document: browsing.its,
                },
            );
            kept.take_from(&browsing.record, &browsing.documents);
        }
        // What a crash between the write and the rename leaves behind.
        let leftover = place.join(format!("{}.{WRITING}", file_for(0)));
        fs::write(&leftover, b"half").expect("a leftover");

        let reopened = Kept::at(&place).expect("the same directory");
        assert_eq!(reopened.len(), 1, "a leftover was counted as a record");
        assert!(
            !leftover.exists(),
            "a leftover nothing will finish was kept"
        );
        let _ = fs::remove_dir_all(&place);
    }

    // --- What a person can do with it (ADR 0012 § 7) -------------------------

    /// Deleting is real: the files go, rather than a flag being set beside them.
    #[test]
    fn a_person_can_delete_it_and_the_files_are_gone() {
        let place = somewhere("deleted");
        let mut browsing = Browsing::new();
        let mut kept = Kept::at(&place).expect("a record");
        browsing.fetched(
            "https://example.com/its.js",
            Cause::Document {
                document: browsing.its,
            },
        );
        kept.take_from(&browsing.record, &browsing.documents);
        assert_eq!(kept.len(), 1);

        kept.empty();

        assert!(kept.is_empty());
        assert!(kept.all().is_empty());
        let left: Vec<PathBuf> = fs::read_dir(&place)
            .expect("a listing")
            .flatten()
            .map(|entry| entry.path())
            .collect();
        assert!(left.is_empty(), "deleting left files behind: {left:?}");
        assert!(
            Kept::at(&place).expect("the same directory").is_empty(),
            "what was deleted came back after a restart",
        );
        let _ = fs::remove_dir_all(&place);
    }

    /// Taking twice keeps one copy: a record that grew every time somebody
    /// looked at it would be counting one request as several.
    #[test]
    fn taking_the_same_lines_twice_writes_them_once() {
        let place = somewhere("twice");
        let mut browsing = Browsing::new();
        let mut kept = Kept::at(&place).expect("a record");
        browsing.fetched(
            "https://example.com/its.js",
            Cause::Document {
                document: browsing.its,
            },
        );

        kept.take_from(&browsing.record, &browsing.documents);
        kept.take_from(&browsing.record, &browsing.documents);
        kept.take_from(&browsing.record, &browsing.documents);

        let held = kept.all();
        assert_eq!(held.len(), 1);
        assert_eq!(
            held.first().map(|deed| deed.requests.len()),
            Some(1),
            "one request was written down three times",
        );
        assert_eq!(kept.missed(), 0);
        let _ = fs::remove_dir_all(&place);
    }

    /// A line dropped from the session's record before it was taken is a
    /// request this record cannot account for, and it says how many rather than
    /// reading as a session in which less happened.
    #[test]
    fn lines_that_went_by_before_they_were_taken_are_counted() {
        let place = somewhere("missed");
        let mut browsing = Browsing::new();
        browsing.record = Activity::new().bounded_to(2, crate::activity::LARGEST_RECORD);
        let mut kept = Kept::at(&place).expect("a record");
        for asked in 0..6 {
            browsing.fetched(
                &format!("https://example.com/{asked}"),
                Cause::Document {
                    document: browsing.its,
                },
            );
        }

        kept.take_from(&browsing.record, &browsing.documents);

        assert_eq!(
            browsing.record.forgotten(),
            4,
            "the session record kept all six"
        );
        assert_eq!(kept.missed(), 4, "the gap was not counted");
        assert_eq!(
            kept.all().first().map(|deed| deed.requests.len()),
            Some(2),
            "what was still there was kept",
        );
        let _ = fs::remove_dir_all(&place);
    }

    // --- Where it lives -------------------------------------------------------

    /// It is not in the cache directory, because a system empties one of those.
    #[test]
    fn it_is_not_kept_where_the_system_keeps_caches() {
        let Some(record) = where_the_system_keeps_records("default") else {
            // No home directory on this machine, which is a real answer.
            return;
        };
        let Some(cache) = crate::disk::where_the_system_keeps_caches("default") else {
            return;
        };
        assert_ne!(record, cache);
        assert!(!record.starts_with(cache), "{record:?}");
        assert!(record.ends_with("agent"), "{record:?}");
        assert_eq!(where_the_system_keeps_records("../elsewhere"), None);
    }
}
