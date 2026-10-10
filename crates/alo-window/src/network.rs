/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The network thread: the only holder of the session's network, making the
//! exchanges the conductor hands it (ADR 0041 § 1, queue item 351).
//!
//! It is given the session's [`Network`] — the pool with its cache and
//! record, the jar, the preflights — and nothing else touches it from then
//! on, so nothing is locked and the record has one writer. It is sent
//! [`Job`]s over one channel and does each in turn:
//!
//! - **make** an exchange, and send what it came to back to the conductor's
//!   inbox;
//! - **record** a refusal's line in the session's record;
//! - **close**: make nothing more, and hand the network back.
//!
//! It makes one exchange at a time. The conductor keeps the queue and its
//! order ([`alo_renderer::fetch_answering`]) and hands it the next exchange
//! only when the last has come back, so which thread waits on a server is
//! the only thing that changed. Several exchanges at once would be a claim
//! about speed, and none is made.
//!
//! # A thread that goes
//!
//! If it ends any way but by being asked — which the lints make unreachable
//! except by a panic — it says [`Arrival::NetworkGone`] as it goes, so the
//! conductor tells the person and answers what is owed as failed rather
//! than waiting for an answer that will never come.

use crate::inbox::Arrival;
use alo_renderer::fetch_exchange::{Exchange, Record};
use alo_renderer::fetch_make::Network;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

/// What the network thread is asked to do.
#[derive(Debug)]
pub enum Job {
    /// Make this exchange, and send back what it came to.
    Make(Box<Exchange>),
    /// Write this refusal into the session's record.
    Record(Record),
    /// Make nothing more.
    Close,
}

/// The network thread, running.
#[derive(Debug)]
pub struct Networking {
    jobs: Sender<Job>,
    thread: JoinHandle<Network>,
}

impl Networking {
    /// Start it over `network`, sending what each exchange came to into
    /// `inbox`.
    ///
    /// # Errors
    ///
    /// What the operating system said when it would not start a thread.
    pub fn start(network: Network, inbox: Sender<Arrival>) -> std::io::Result<Self> {
        let (jobs, work) = channel();
        let thread = std::thread::Builder::new()
            .name("alo-network".to_owned())
            .spawn(move || work_through(network, &work, Going(Some(inbox))))?;
        Ok(Self { jobs, thread })
    }

    /// Hand it `job`. Whether it was there to take it.
    pub fn send(&self, job: Job) -> bool {
        self.jobs.send(job).is_ok()
    }

    /// Ask it to close, wait for the exchange in flight if there is one,
    /// and take the network back — with its record of every request made
    /// and refused. [`None`] if it panicked.
    pub fn finish(self) -> Option<Network> {
        let _ = self.jobs.send(Job::Close);
        drop(self.jobs);
        self.thread.join().ok()
    }
}

/// Says [`Arrival::NetworkGone`] if dropped while still holding the inbox:
/// the thread ending without being asked to.
struct Going(Option<Sender<Arrival>>);

impl Going {
    /// Send what an exchange came to. Nobody listening is a conductor that
    /// has finished, and the next job is still done.
    fn send(&self, arrival: Arrival) {
        if let Some(inbox) = &self.0 {
            let _ = inbox.send(arrival);
        }
    }
}

impl Drop for Going {
    fn drop(&mut self) {
        if let Some(inbox) = self.0.take() {
            let _ = inbox.send(Arrival::NetworkGone);
        }
    }
}

fn work_through(mut network: Network, work: &Receiver<Job>, mut going: Going) -> Network {
    // Every sender gone is a conductor that went without closing: nothing
    // more will be asked.
    while let Ok(job) = work.recv() {
        match job {
            Job::Make(exchange) => {
                let exchanged = exchange.make(&mut network);
                going.send(Arrival::Exchanged(Box::new(exchanged)));
            }
            Job::Record(record) => {
                record.record(&mut network.pool);
            }
            Job::Close => break,
        }
    }
    // Asked, so nothing to say.
    going.0 = None;
    network
}
