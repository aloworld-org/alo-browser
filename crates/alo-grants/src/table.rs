/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The one table the browser process holds, and the question it answers: *may
//! this key use this capability now*.
//!
//! ADR 0026 is the decision. [`crate::ask`] is the facts of an ask,
//! [`crate::grant`] the answers and when they end, [`crate::history`] the
//! record and [`crate::disk`] the file. This is the order things happen in.
//!
//! # An ask, in order
//!
//! 1. An opaque origin has no key, and is refused before anything is recorded:
//!    there is nowhere to file it.
//! 2. The ask is recorded.
//! 3. An insecure context, a frame its embedder did not allow, and a
//!    capability the person refused everywhere are refused.
//! 4. A remembered row that has ended is ended now, and recorded.
//! 5. A grant that holds answers *granted*, and a remembered *don't allow*
//!    answers *refused*. Neither prompts: the person has answered already.
//! 6. A document refused before, and a document nobody is using, are refused.
//! 7. What is left is a prompt.
//!
//! # Only a person answers, and only a person takes away
//!
//! Every act that makes, answers or removes a grant takes the [`Cause`] it
//! came from and refuses anything but [`Cause::Person`]. ADR 0026 § 4: *"No
//! agent verb makes, changes or revokes a grant."* The surfaces that will call
//! these (items 127, 128 and 308) read a person's input before any renderer
//! does, and the cause is the browser process's to assign (ADR 0012), so a
//! renderer has nothing to forge one with.
//!
//! # Checked at every use, and revoked at once
//!
//! § 7: the table is asked **when a frame is sent**, not only when it was
//! asked for, through [`Table::allows`]. A use is begun and ended here, so
//! that revoking a row, refusing a capability everywhere, clearing a site, a
//! page closing and a grant ending each return the documents whose use stopped
//! in the same act. The caller stops those devices before it returns to
//! anybody; a revocation that waited for the page would not be one.
//!
//! # What is written, and when
//!
//! Every change to a remembered row, a refusal everywhere or a history writes
//! the whole table, beside and then over the old one. *Allow while this page is
//! open* and a document's own refusals are memory only, because they end with
//! the document and a restart ends every document. A table for a private
//! session ([`Table::for_the_session`]) is never given a directory, so it has
//! nowhere to put a file.
//!
//! A write the filesystem refuses does not undo the change in memory: a
//! revocation holds whether or not the disk agreed. [`Table::unwritten`] says
//! why, for the person to be told.

use crate::ask::{Ask, Decision, Prompted, Refusal};
use crate::capability::Capability;
use crate::disk::{Disk, Opened};
use crate::file::Saved;
use crate::grant::{Answer, Ending, Kept, Remembered};
use crate::history::{Entry, Happened, History};
use alo_net::Cause;
use alo_net::cause::DocumentId;
use alo_net::deed::Link;
use alo_storage::StorageKey;
use core::fmt;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::SystemTime;

/// Something other than a person tried to answer, grant or take away.
#[derive(Debug, PartialEq, Eq)]
pub struct NotAPerson {
    /// The prompt it tried to answer, still waiting for a person, when it was
    /// an answer.
    pub still_waiting: Option<Box<Prompted>>,
}

impl fmt::Display for NotAPerson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("only a person answers, grants or revokes a permission (ADR 0026 § 4)")
    }
}

impl std::error::Error for NotAPerson {}

/// A use refused because nothing grants it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotGranted;

impl fmt::Display for NotGranted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("nothing a person said allows this use (ADR 0026 § 7)")
    }
}

impl std::error::Error for NotGranted {}

/// An *allow while this page is open*, held beside its document.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WhileOpen {
    key: StorageKey,
}

/// A use in progress.
#[derive(Debug, Clone, PartialEq, Eq)]
struct InUse {
    key: StorageKey,
    by: Link,
}

/// The browser process's permission table.
#[derive(Debug)]
pub struct Table {
    disk: Option<Disk>,
    remembered: BTreeMap<(StorageKey, Capability), Remembered>,
    everywhere: BTreeSet<Capability>,
    histories: BTreeMap<StorageKey, History>,
    while_open: BTreeMap<(DocumentId, Capability), WhileOpen>,
    /// Documents refused, or whose prompt was dismissed, for a capability.
    refused: BTreeMap<(DocumentId, Capability), StorageKey>,
    in_use: BTreeMap<(DocumentId, Capability), InUse>,
    set_aside: Option<String>,
    unwritten: Option<String>,
}

impl Table {
    /// The table kept in this directory, read back from what is there.
    ///
    /// A table that fails its check is set aside and this one starts empty:
    /// [`Table::set_aside`] says so, for the person to be told.
    ///
    /// # Errors
    ///
    /// A sentence, when the directory cannot be made or made private.
    pub fn at(directory: impl AsRef<Path>) -> Result<Self, String> {
        let disk = Disk::at(directory)?;
        let mut table = Self::empty(Some(disk));
        match table.disk.as_ref().map(Disk::open) {
            Some(Opened::Table(saved)) => table.load(saved),
            Some(Opened::SetAside(why)) => table.set_aside = Some(why),
            Some(Opened::Nothing) | None => {}
        }
        Ok(table)
    }

    /// A private session's table: it starts with no grants, and nothing it
    /// grants or refuses is written anywhere (ADR 0026 § 5).
    pub fn for_the_session() -> Self {
        Self::empty(None)
    }

    fn empty(disk: Option<Disk>) -> Self {
        Self {
            disk,
            remembered: BTreeMap::new(),
            everywhere: BTreeSet::new(),
            histories: BTreeMap::new(),
            while_open: BTreeMap::new(),
            refused: BTreeMap::new(),
            in_use: BTreeMap::new(),
            set_aside: None,
            unwritten: None,
        }
    }

    fn load(&mut self, saved: Saved) {
        for row in saved.remembered {
            self.remembered
                .insert((row.key.clone(), row.capability), row);
        }
        self.everywhere = saved.everywhere;
        self.histories = saved.histories;
    }

    /// Where the table is kept, or [`None`] for a private session's.
    pub fn directory(&self) -> Option<&Path> {
        self.disk.as_ref().map(Disk::directory)
    }

    /// Why the table found at start was set aside, when it was.
    pub fn set_aside(&self) -> Option<&str> {
        self.set_aside.as_deref()
    }

    /// Why the last write did not reach the disk, until one does.
    pub fn unwritten(&self) -> Option<&str> {
        self.unwritten.as_deref()
    }

    /// Every remembered answer: the rows of the list a person sees, beside the
    /// grants that last while a page is open.
    pub fn rows(&self) -> impl Iterator<Item = &Remembered> {
        self.remembered.values()
    }

    /// The capabilities a person refused for every site.
    pub fn refused_everywhere(&self) -> &BTreeSet<Capability> {
        &self.everywhere
    }

    /// One key's history, when it has one.
    pub fn history(&self, key: &StorageKey) -> Option<&History> {
        self.histories.get(key)
    }

    /// The documents using this capability now, which is what the indicator
    /// shows for the three that hold a device (ADR 0026 § 6).
    pub fn in_use(&self, capability: Capability) -> Vec<DocumentId> {
        self.in_use
            .keys()
            .filter(|(_, using)| *using == capability)
            .map(|(document, _)| *document)
            .collect()
    }

    // --- Asking --------------------------------------------------------------

    /// What comes of a page asking. See this module's documentation for the
    /// order.
    pub fn ask(&mut self, ask: Ask, now: SystemTime) -> Decision {
        let Some(key) = ask.key else {
            return Decision::Refused(Refusal::OpaqueOrigin);
        };
        let capability = ask.capability;
        let by = Link::of(&ask.cause);
        self.record(&key, capability, now, Happened::Asked { by: by.clone() });
        self.end_remembered_if_over(&key, capability, now);

        let refusal = if !ask.secure {
            Some(Refusal::InsecureContext)
        } else if !ask.delegated {
            Some(Refusal::NotDelegated)
        } else if self.everywhere.contains(&capability) {
            Some(Refusal::RefusedEverywhere)
        } else if self.allows(&key, capability, ask.document, now) {
            self.save();
            return Decision::Granted;
        } else if self
            .remembered
            .get(&(key.clone(), capability))
            .is_some_and(|row| row.kept == Kept::Refused)
        {
            Some(Refusal::RememberedRefusal)
        } else if self.refused.contains_key(&(ask.document, capability)) {
            Some(Refusal::RefusedThisDocument)
        } else if !ask.activated {
            Some(Refusal::NoGesture)
        } else {
            None
        };
        if let Some(refusal) = refusal {
            self.record(
                &key,
                capability,
                now,
                Happened::RefusedWithoutAPrompt { refusal, by },
            );
            self.save();
            return Decision::Refused(refusal);
        }
        self.save();
        Decision::Prompt(Prompted {
            key,
            capability,
            document: ask.document,
            cause: ask.cause,
        })
    }

    /// A person's answer to a prompt.
    ///
    /// # Errors
    ///
    /// [`NotAPerson`] for any other cause, with the prompt handed back still
    /// waiting. Nothing is recorded as an answer and nothing is granted.
    pub fn answer(
        &mut self,
        prompted: Prompted,
        answer: Answer,
        by: &Cause,
        now: SystemTime,
    ) -> Result<(), NotAPerson> {
        if !matches!(by, Cause::Person { .. }) {
            return Err(NotAPerson {
                still_waiting: Some(Box::new(prompted)),
            });
        }
        let Prompted {
            key,
            capability,
            document,
            ..
        } = prompted;
        self.record(
            &key,
            capability,
            now,
            Happened::Answered {
                answer,
                by: Link::of(by),
            },
        );
        match answer {
            Answer::WhileThisPageIsOpen => {
                self.while_open
                    .insert((document, capability), WhileOpen { key });
            }
            Answer::OnThisSite | Answer::DoNotAllow => {
                let kept = if answer == Answer::OnThisSite {
                    Kept::Allowed
                } else {
                    self.refused.insert((document, capability), key.clone());
                    Kept::Refused
                };
                self.remembered.insert(
                    (key.clone(), capability),
                    Remembered {
                        key,
                        capability,
                        kept,
                        given: now,
                        visited: now,
                        last_used: None,
                    },
                );
            }
        }
        self.save();
        Ok(())
    }

    /// A person dismissed a prompt: that document is refused for that
    /// capability until it is gone, and nothing is remembered.
    ///
    /// # Errors
    ///
    /// [`NotAPerson`] for any other cause, with the prompt handed back.
    pub fn dismiss(
        &mut self,
        prompted: Prompted,
        by: &Cause,
        now: SystemTime,
    ) -> Result<(), NotAPerson> {
        if !matches!(by, Cause::Person { .. }) {
            return Err(NotAPerson {
                still_waiting: Some(Box::new(prompted)),
            });
        }
        self.record(
            &prompted.key,
            prompted.capability,
            now,
            Happened::Dismissed { by: Link::of(by) },
        );
        self.refused
            .insert((prompted.document, prompted.capability), prompted.key);
        self.save();
        Ok(())
    }

    // --- Using ---------------------------------------------------------------

    /// Whether this key may use this capability in this document now.
    ///
    /// Asked at every use (ADR 0026 § 7), and it changes nothing: a row that
    /// has ended answers *no* here whether or not [`Table::expire`] has
    /// removed it yet.
    pub fn allows(
        &self,
        key: &StorageKey,
        capability: Capability,
        document: DocumentId,
        now: SystemTime,
    ) -> bool {
        if self.everywhere.contains(&capability) {
            return false;
        }
        if self
            .while_open
            .get(&(document, capability))
            .is_some_and(|grant| grant.key == *key)
        {
            return true;
        }
        self.remembered
            .get(&(key.clone(), capability))
            .is_some_and(|row| row.kept == Kept::Allowed && row.over(now).is_none())
    }

    /// A use begins: the first frame, the first position, the notification
    /// shown.
    ///
    /// It never extends a grant. Only the person opening the site does that
    /// ([`Table::person_opened`]).
    ///
    /// # Errors
    ///
    /// [`NotGranted`], when [`Table::allows`] says no. Nothing is recorded as
    /// a use.
    pub fn begin_using(
        &mut self,
        key: &StorageKey,
        capability: Capability,
        document: DocumentId,
        cause: &Cause,
        now: SystemTime,
    ) -> Result<(), NotGranted> {
        if !self.allows(key, capability, document, now) {
            return Err(NotGranted);
        }
        let by = Link::of(cause);
        self.in_use.insert(
            (document, capability),
            InUse {
                key: key.clone(),
                by: by.clone(),
            },
        );
        if let Some(row) = self.remembered.get_mut(&(key.clone(), capability)) {
            row.last_used = Some(now);
        }
        self.record(key, capability, now, Happened::UseBegan { by });
        self.save();
        Ok(())
    }

    /// A use ends because the page stopped it.
    pub fn stop_using(&mut self, document: DocumentId, capability: Capability, now: SystemTime) {
        if self.end_use(document, capability, now) {
            self.save();
        }
    }

    // --- Ending --------------------------------------------------------------

    /// The person opened a page themselves: a top-level load whose cause is
    /// [`Cause::Person`] (ADR 0012).
    ///
    /// Every remembered row for that page's origin, under any top-level site,
    /// counts its thirty days from now. A load a page or an agent caused
    /// extends nothing, and neither does a key that is not a top-level page's.
    /// A clock earlier than the row's own visit moves nothing, so that
    /// [`Remembered::over`] can end the row for it.
    pub fn person_opened(&mut self, top_level: &StorageKey, cause: &Cause, now: SystemTime) {
        if !matches!(cause, Cause::Person { .. }) || top_level.site() != top_level.partition() {
            return;
        }
        let mut moved = false;
        for row in self.remembered.values_mut() {
            if row.key.origin() == top_level.origin() && row.visited <= now {
                row.visited = now;
                moved = true;
            }
        }
        if moved {
            self.save();
        }
    }

    /// The document is gone: navigated away, closed, or its renderer died.
    ///
    /// Its *allow while open* grants end, its refusals are forgotten, and its
    /// uses end. Returns the capabilities whose use stopped.
    pub fn document_gone(&mut self, document: DocumentId, now: SystemTime) -> Vec<Capability> {
        let mut stopped = Vec::new();
        let using: Vec<Capability> = self
            .in_use
            .keys()
            .filter(|(of, _)| *of == document)
            .map(|(_, capability)| *capability)
            .collect();
        for capability in using {
            if self.end_use(document, capability, now) {
                stopped.push(capability);
            }
        }
        let open: Vec<(DocumentId, Capability)> = self
            .while_open
            .keys()
            .filter(|(of, _)| *of == document)
            .copied()
            .collect();
        for at in open {
            if let Some(grant) = self.while_open.remove(&at) {
                self.record(
                    &grant.key,
                    at.1,
                    now,
                    Happened::Ended {
                        ending: Ending::PageClosed,
                    },
                );
            }
        }
        self.refused.retain(|(of, _), _| *of != document);
        self.save();
        stopped
    }

    /// End every remembered row that is over by `now`, recording why, and
    /// every use nothing allows any more. Returns the uses that stopped.
    pub fn expire(&mut self, now: SystemTime) -> Vec<(DocumentId, Capability)> {
        let over: Vec<(StorageKey, Capability)> = self
            .remembered
            .iter()
            .filter(|(_, row)| row.over(now).is_some())
            .map(|(at, _)| at.clone())
            .collect();
        let changed = !over.is_empty();
        for (key, capability) in over {
            self.end_remembered_if_over(&key, capability, now);
        }
        let stopped = self.stop_what_is_not_allowed(now);
        if changed || !stopped.is_empty() {
            self.save();
        }
        stopped
    }

    /// A person takes away a remembered answer, or an *allow while open*, for
    /// one key and one capability. Every use under it stops before this
    /// returns, and the documents whose use stopped are returned for the
    /// caller to stop their devices.
    ///
    /// # Errors
    ///
    /// [`NotAPerson`] for any other cause. Nothing changes.
    pub fn revoke(
        &mut self,
        key: &StorageKey,
        capability: Capability,
        by: &Cause,
        now: SystemTime,
    ) -> Result<Vec<DocumentId>, NotAPerson> {
        Self::only_a_person(by)?;
        self.remembered.remove(&(key.clone(), capability));
        self.while_open
            .retain(|(_, of), grant| !(*of == capability && grant.key == *key));
        self.record(key, capability, now, Happened::Revoked { by: Link::of(by) });
        let stopped = self
            .stop_what_is_not_allowed(now)
            .into_iter()
            .map(|(document, _)| document)
            .collect();
        self.save();
        Ok(stopped)
    }

    /// A person refuses a capability for every site: *never ask me about
    /// notifications*. Every grant of it is revoked, recorded under its key,
    /// and every use of it stops.
    ///
    /// # Errors
    ///
    /// [`NotAPerson`] for any other cause. Nothing changes.
    pub fn refuse_everywhere(
        &mut self,
        capability: Capability,
        by: &Cause,
        now: SystemTime,
    ) -> Result<Vec<DocumentId>, NotAPerson> {
        Self::only_a_person(by)?;
        self.everywhere.insert(capability);
        let mut granted: BTreeSet<StorageKey> = self
            .remembered
            .values()
            .filter(|row| row.capability == capability && row.kept == Kept::Allowed)
            .map(|row| row.key.clone())
            .collect();
        granted.extend(
            self.while_open
                .iter()
                .filter(|((_, of), _)| *of == capability)
                .map(|(_, grant)| grant.key.clone()),
        );
        for key in &granted {
            self.remembered.remove(&(key.clone(), capability));
            self.record(key, capability, now, Happened::Revoked { by: Link::of(by) });
        }
        self.while_open.retain(|(_, of), _| *of != capability);
        let stopped = self
            .stop_what_is_not_allowed(now)
            .into_iter()
            .map(|(document, _)| document)
            .collect();
        self.save();
        Ok(stopped)
    }

    /// A person takes back a refusal for every site. Sites ask again, with a
    /// gesture.
    ///
    /// # Errors
    ///
    /// [`NotAPerson`] for any other cause. Nothing changes.
    pub fn ask_again_everywhere(
        &mut self,
        capability: Capability,
        by: &Cause,
    ) -> Result<(), NotAPerson> {
        Self::only_a_person(by)?;
        if self.everywhere.remove(&capability) {
            self.save();
        }
        Ok(())
    }

    /// Clear a site: every grant, refusal and history of every key whose
    /// origin is of that site, under every top-level site, in one act (ADR
    /// 0026 § 7, beside ADR 0025 § 8). Every use under them stops; the
    /// documents are returned.
    ///
    /// Nothing is recorded, because the record is part of what is cleared.
    pub fn clear_site(&mut self, site: &str) -> Vec<DocumentId> {
        self.remembered.retain(|(key, _), _| key.site() != site);
        self.histories.retain(|key, _| key.site() != site);
        self.while_open.retain(|_, grant| grant.key.site() != site);
        self.refused.retain(|_, key| key.site() != site);
        let stopped: Vec<(DocumentId, Capability)> = self
            .in_use
            .iter()
            .filter(|(_, using)| using.key.site() == site)
            .map(|(at, _)| *at)
            .collect();
        for at in &stopped {
            self.in_use.remove(at);
        }
        self.save();
        stopped.into_iter().map(|(document, _)| document).collect()
    }

    // --- Within ----------------------------------------------------------------

    fn only_a_person(by: &Cause) -> Result<(), NotAPerson> {
        if matches!(by, Cause::Person { .. }) {
            Ok(())
        } else {
            Err(NotAPerson {
                still_waiting: None,
            })
        }
    }

    /// Remove a remembered row that is over, recording why.
    fn end_remembered_if_over(
        &mut self,
        key: &StorageKey,
        capability: Capability,
        now: SystemTime,
    ) {
        let at = (key.clone(), capability);
        let Some(ending) = self.remembered.get(&at).and_then(|row| row.over(now)) else {
            return;
        };
        self.remembered.remove(&at);
        self.record(key, capability, now, Happened::Ended { ending });
    }

    /// End every use the table no longer allows, recording each.
    fn stop_what_is_not_allowed(&mut self, now: SystemTime) -> Vec<(DocumentId, Capability)> {
        let stopping: Vec<(DocumentId, Capability)> = self
            .in_use
            .iter()
            .filter(|((document, capability), using)| {
                !self.allows(&using.key, *capability, *document, now)
            })
            .map(|(at, _)| *at)
            .collect();
        for (document, capability) in &stopping {
            self.end_use(*document, *capability, now);
        }
        stopping
    }

    /// End one use, recording it. Whether there was one.
    fn end_use(&mut self, document: DocumentId, capability: Capability, now: SystemTime) -> bool {
        let Some(using) = self.in_use.remove(&(document, capability)) else {
            return false;
        };
        self.record(
            &using.key,
            capability,
            now,
            Happened::UseEnded { by: using.by },
        );
        true
    }

    fn record(
        &mut self,
        key: &StorageKey,
        capability: Capability,
        at: SystemTime,
        happened: Happened,
    ) {
        self.histories.entry(key.clone()).or_default().add(Entry {
            at,
            capability,
            happened,
        });
    }

    /// Write the table, when it has somewhere to go.
    fn save(&mut self) {
        let Some(disk) = &self.disk else {
            return;
        };
        let saved = Saved {
            remembered: self.remembered.values().cloned().collect(),
            everywhere: self.everywhere.clone(),
            histories: self.histories.clone(),
        };
        self.unwritten = disk.write(&saved).err();
    }
}
