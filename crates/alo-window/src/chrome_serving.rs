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
//! # Headless, or in Chrome's own window
//!
//! [`ChromeServing::start`] holds a headless Chrome, which is what the
//! socket's tests read through. [`ChromeServing::start_shown`] holds one
//! shown, its page in a window of Chrome's own placed on the screen by this
//! process (ADR 0061, item 627, [`crate::chrome_window`]) — which is what a
//! person running `alo --chrome` looks at.
//!
//! # What it is not
//!
//! A window of ours. Chrome's window is Chrome's: a person's pointer and
//! keys reach the page there without passing this process, and no window
//! of ours follows it yet (item 642). So the window's orders — a resize, a
//! pointer, a tab key — are not carried out here, and nothing on this path
//! sends one. Closing is: [`Order::CloseEverything`], or every sender of
//! orders gone, closes the tab and Chrome with it.

use crate::agent_record::AgentRecord;
use crate::agent_sites::AgentSites;
use crate::chrome::{Chrome, Failed, Unstarted};
use crate::chrome_pages::ChromePages;
use crate::chrome_tab::{Tab, Unopened};
use crate::chrome_window::{self, Placed, Unplaced};
use crate::inbox::{self, Arrival, Orders, arrivals};
use crate::message::Order;
use crate::pages::RenderedBy;
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
    /// Chrome's window was not put where it was asked.
    Unplaced(Unplaced),
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
            Self::Unplaced(why) => why.fmt(f),
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
    rendered_by: RenderedBy,
    placed: Option<Placed>,
}

/// Where Chrome draws the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drawn {
    /// Headless: nowhere a person sees.
    Headless,
    /// In a window of Chrome's own, its top left corner put at this point
    /// on the screen.
    Shown {
        /// In the screen's points.
        left: i32,
        /// In the screen's points.
        top: i32,
    },
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
        Self::begin(opening, size, Drawn::Headless, sites)
    }

    /// [`ChromeServing::start`], with the machine's Chrome shown: the page
    /// laid out at `size` CSS pixels in a window of Chrome's own, whose top
    /// left corner is put at `left`, `top` on the screen.
    ///
    /// # Errors
    ///
    /// As [`ChromeServing::start`], and [`Unserved::Unplaced`] when the
    /// window was not put there.
    pub fn start_shown(
        opening: Opening,
        size: (u32, u32),
        (left, top): (i32, i32),
        sites: AgentSites,
    ) -> Result<Self, Unserved> {
        Self::begin(opening, size, Drawn::Shown { left, top }, sites)
    }

    /// Start a Chrome drawing as `drawn` says, on a thread of its own.
    fn begin(
        opening: Opening,
        size: (u32, u32),
        drawn: Drawn,
        sites: AgentSites,
    ) -> Result<Self, Unserved> {
        let (orders, results, inbox) = inbox::inbox();
        // Nothing here makes a request of this engine's network: no results
        // are ever sent.
        drop(results);
        let (tell, started) = channel();
        let thread = std::thread::Builder::new()
            .name("alo-chrome".to_owned())
            .spawn(move || serve_in_chrome(opening, size, drawn, &sites, &inbox, &tell))
            .map_err(|why| Unserved::NoThread(why.kind()))?;
        match started.recv() {
            Ok(Ok((product, placed))) => Ok(Self {
                orders,
                thread,
                rendered_by: RenderedBy::Chrome { product },
                placed,
            }),
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

    /// Which Chrome draws its page, as that Chrome says itself — what the
    /// record names, and what `alo` says on the terminal (item 641).
    pub fn rendered_by(&self) -> &RenderedBy {
        &self.rendered_by
    }

    /// Where Chrome's window went and the page's size inside it, as read
    /// once it had stopped moving — or [`None`] for a headless Chrome.
    pub fn placed(&self) -> Option<&Placed> {
        self.placed.as_ref()
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

/// Start Chrome, open the page, say whether that worked — and which Chrome
/// it is — on `tell`, and answer agents until told to close.
fn serve_in_chrome(
    opening: Opening,
    size: (u32, u32),
    drawn: Drawn,
    sites: &AgentSites,
    inbox: &Receiver<Arrival>,
    tell: &Sender<Result<(String, Option<Placed>), Unserved>>,
) -> AgentRecord {
    let mut record = AgentRecord::new();
    let started = match drawn {
        Drawn::Headless => Chrome::start(),
        Drawn::Shown { .. } => Chrome::start_shown(),
    };
    let mut chrome = match started {
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
    let mut tab = match opened {
        Ok(tab) => tab,
        Err(why) => {
            let _ = tell.send(Err(Unserved::Unopened(why)));
            return record;
        }
    };
    let placed = match drawn {
        Drawn::Headless => None,
        Drawn::Shown { left, top } => match chrome_window::place(&mut tab, left, top) {
            Ok(placed) => Some(placed),
            Err(why) => {
                let _ = tell.send(Err(Unserved::Unplaced(why)));
                return record;
            }
        },
    };
    let mut pages = ChromePages::of(tab, product.clone());
    if tell.send(Ok((product, placed))).is_err() {
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
