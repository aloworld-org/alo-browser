/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The agent socket reaching a tab in Chrome (ADR 0058 § 4, ADR 0055, queue
//! item 630).
//!
//! A thread of its own holds the machine's Chrome and the one tab open in
//! it, and is sent [`Order::Agent`] as the conductor is — by the same
//! [`crate::agents::Listening`], over the same [`Orders`] — so a socket
//! request reaches a page in Chrome through exactly what it reaches this
//! engine's through: [`crate::serving`], with its order of judging and the
//! grants of item 497 in front of every tab, and the record written on this
//! side of the pipe, naming Chrome as the renderer.
//!
//! # What it is not
//!
//! A window. Nothing a person sees is drawn from it yet: how a page in
//! Chrome is shown is ADR 0061's, and not built (item 627). So the window's
//! orders — a resize, a pointer, a tab key — are not carried out here, and
//! nothing on this path sends one. Closing is: [`Order::CloseEverything`],
//! or every sender of orders gone, closes the tab and Chrome with it.

use crate::agent_record::AgentRecord;
use crate::agent_sites::AgentSites;
use crate::chrome::{Chrome, Failed, Unstarted};
use crate::chrome_pages::ChromePages;
use crate::chrome_tab::{Tab, Unopened};
use crate::inbox::{self, Arrival, Orders, arrivals};
use crate::message::Order;
use crate::serving::serve;
use alo_url::Url;
use core::fmt;
use std::io::ErrorKind;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

/// The page a tab in Chrome is opened at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opening {
    /// A frozen page's bytes, served at the address they were taken from;
    /// nothing reaches a network ([`Tab::open_frozen`]).
    Frozen {
        /// Where it was taken from.
        url: String,
        /// Its bytes.
        page: Vec<u8>,
    },
    /// A page from the network, every request judged before it leaves
    /// ([`Tab::open`]).
    Network(Url),
}

/// Why no tab in Chrome is being served.
#[derive(Debug, Clone, PartialEq)]
pub enum Unserved {
    /// Chrome was not started.
    Unstarted(Unstarted),
    /// Chrome did not say which it is.
    Unnamed(Failed),
    /// The page was not opened.
    Unopened(Unopened),
    /// The thread could not be started.
    NoThread(ErrorKind),
    /// The thread went without saying whether it had started.
    Gone,
}

impl fmt::Display for Unserved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unstarted(why) => why.fmt(f),
            Self::Unnamed(why) => write!(f, "Chrome did not say which it is: {why}"),
            Self::Unopened(why) => why.fmt(f),
            Self::NoThread(kind) => write!(f, "no thread to hold Chrome: {kind}"),
            Self::Gone => f.write_str("the thread holding Chrome went before it started"),
        }
    }
}

impl std::error::Error for Unserved {}

/// A tab in Chrome, answering agents.
#[derive(Debug)]
pub struct ChromeServing {
    orders: Orders,
    thread: JoinHandle<AgentRecord>,
}

impl ChromeServing {
    /// Start the machine's Chrome on a thread of its own, open `opening` in
    /// it at `size` CSS pixels, and answer agents about it — kept to
    /// `sites`.
    ///
    /// Returns once the page has loaded, so a request sent after it is
    /// answered about the page.
    ///
    /// # Errors
    ///
    /// [`Unserved`], when Chrome does not start, say which it is, or open
    /// the page — Chrome is closed again — or no thread could be started.
    pub fn start(opening: Opening, size: (u32, u32), sites: AgentSites) -> Result<Self, Unserved> {
        let (orders, results, inbox) = inbox::inbox();
        // Nothing here makes a request of this engine's network: no results
        // are ever sent.
        drop(results);
        let (tell, started) = channel();
        let thread = std::thread::Builder::new()
            .name("alo-chrome".to_owned())
            .spawn(move || serve_in_chrome(opening, size, &sites, &inbox, &tell))
            .map_err(|why| Unserved::NoThread(why.kind()))?;
        match started.recv() {
            Ok(Ok(())) => Ok(Self { orders, thread }),
            Ok(Err(why)) => {
                let _ = thread.join();
                Err(why)
            }
            Err(_) => {
                let _ = thread.join();
                Err(Unserved::Gone)
            }
        }
    }

    /// Where to send it orders: what [`crate::agents::Listening::at`] sends
    /// agents' requests to.
    pub fn orders(&self) -> Orders {
        self.orders.clone()
    }

    /// Wait for it to finish — after [`Order::CloseEverything`], or once
    /// every sender of orders has gone — and take back what every agent
    /// read and did. [`None`] if its thread panicked, which the lints make
    /// unreachable, and which is said rather than assumed.
    pub fn records(self) -> Option<AgentRecord> {
        drop(self.orders);
        self.thread.join().ok()
    }
}

/// Start Chrome, open the page, say whether that worked on `tell`, and
/// answer agents until told to close.
fn serve_in_chrome(
    opening: Opening,
    size: (u32, u32),
    sites: &AgentSites,
    inbox: &Receiver<Arrival>,
    tell: &Sender<Result<(), Unserved>>,
) -> AgentRecord {
    let mut record = AgentRecord::new();
    let mut chrome = match Chrome::start() {
        Ok(chrome) => chrome,
        Err(why) => {
            let _ = tell.send(Err(Unserved::Unstarted(why)));
            return record;
        }
    };
    let product = match chrome.version() {
        Ok(version) => version.product,
        Err(why) => {
            let _ = tell.send(Err(Unserved::Unnamed(why)));
            return record;
        }
    };
    let opened = match opening {
        Opening::Frozen { url, page } => Tab::open_frozen(&mut chrome, &url, page, size),
        Opening::Network(address) => Tab::open(&mut chrome, &address, size),
    };
    let mut pages = match opened {
        Ok(tab) => ChromePages::of(tab, product),
        Err(why) => {
            let _ = tell.send(Err(Unserved::Unopened(why)));
            return record;
        }
    };
    if tell.send(Ok(())).is_err() {
        return record;
    }
    while let Some(arrived) = arrivals(inbox, None) {
        for arrival in arrived {
            match arrival {
                Arrival::Order(Order::Agent { caller, ask, reply }) => {
                    let served = serve(&mut pages, &mut record, sites, caller, ask);
                    // Nobody to answer is a caller that has gone, which
                    // changes nothing about what was done or recorded.
                    let _ = reply.send(served.answer);
                }
                Arrival::Order(Order::CloseEverything) | Arrival::Abandoned => return record,
                // The window's, which nothing on this path sends; and the
                // network thread's, of which there is none.
                Arrival::Order(_) | Arrival::Exchanged(_) | Arrival::NetworkGone => {}
            }
        }
    }
    record
}
