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
//! # The frame that follows Chrome's window
//!
//! A shown Chrome's window has a frame of ours around it (ADR 0062, item
//! 642), and this thread is the one that can ask Chrome where its window
//! is. So between orders — every [`FOLLOW_EVERY`] when nothing arrives —
//! it reads where the frame stands ([`chrome_frame::read`]) and tells the
//! frame's window what changed ([`Following`]). A reading that finds
//! Chrome's window gone tells the frame [`Follow::Closed`] and ends the
//! thread: a person closing Chrome's window closes ours.
//!
//! The other way, ours closed — [`Order::CloseEverything`], or the one
//! browser key that closes a tab, ⌘W, since closing the last tab closes
//! the window (ADR 0024 § 6) — closes Chrome's page, which is its window,
//! tells the frame [`Follow::Closed`] and ends the thread, and Chrome with
//! it. Every sender of orders gone ends it too.
//!
//! # What the frame shows of the page
//!
//! In the same cycle, after where the frame stands, this thread reads what
//! Chrome calls the page and where it is ([`chrome_named::read`]) and tells
//! the frame when that changes ([`ToFrame::Named`], queue item 681), so the
//! frame's tab follows a page that retitles itself or goes somewhere. A
//! reading that fails tells nothing, and the frame keeps the name it has.
//!
//! # What the frame says of agents
//!
//! What an agent's verb was answered with is told to the frame too
//! ([`ToFrame::Agent`], queue item 679) — the line [`AgentLine::of`] makes
//! of the answer this thread just sent, by the rule this engine's conductor
//! answers by, so a person watching is told the same of the same order
//! whichever renderer drew the page. Only a shown Chrome's frame is told
//! where to stand; [`ChromeServing::start_telling`] tells a headless
//! Chrome's agents' lines to whoever asked for them, and stands nothing
//! anywhere.
//!
//! # The box a verb acted on
//!
//! A shown Chrome is asked, after the agent has been answered and the frame
//! told, to mark the box the verb acted on over its page — Chrome's own
//! highlight of the node, never a window of ours ([`Marker`], ADR 0066,
//! queue item 680). A headless one is not: nobody would see it.
//!
//! # What it is not
//!
//! Chrome's window. A person's pointer and keys reach the page there
//! without passing this process, so the window's other orders — a resize,
//! a pointer, the other tab keys — are not carried out here: Chrome lays
//! its page out again at the size a person gives its window itself.

use crate::agent_lines::AgentLine;
use crate::agent_record::{AgentRecord, Caller};
use crate::agent_sites::AgentSites;
use crate::asking::{Answer, Ask, Refused};
use crate::chrome::{Chrome, Failed, Unstarted};
use crate::chrome_follow::{FOLLOW_EVERY, Follow, Following};
use crate::chrome_frame;
use crate::chrome_mark::{Marker, Marking};
use crate::chrome_named::{self, Naming};
use crate::chrome_pages::ChromePages;
use crate::chrome_profile::Swept;
use crate::chrome_protocol::Named;
use crate::chrome_tab::{Tab, Unopened};
use crate::chrome_window::{self, Placed, Unplaced};
use crate::frame_told::ToFrame;
use crate::inbox::{self, Arrival, Orders, arrivals};
use crate::message::{Order, TabOrder};
use crate::pages::RenderedBy;
use crate::serving::serve;
use alo_url::Url;
use core::fmt;
use std::io::ErrorKind;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::Instant;

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

/// Where the frame of ours around a shown Chrome's window is told what it
/// is to do and what agents did: a function that answers whether the frame
/// was still there to hear it.
pub type TellFrame = Box<dyn Fn(ToFrame) -> bool + Send>;

/// A tab in Chrome, answering agents.
#[derive(Debug)]
pub struct ChromeServing {
    orders: Orders,
    thread: JoinHandle<AgentRecord>,
    rendered_by: RenderedBy,
    placed: Option<Placed>,
    process: u32,
    swept: Swept,
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
        Self::begin(opening, size, Drawn::Headless, sites, None)
    }

    /// [`ChromeServing::start`], headless, with what each agent's verb was
    /// answered with told on `frame` as a shown Chrome's frame is told it —
    /// for a reader of the agent's lines that stands beside no window, such
    /// as a test of what the frame is told, which then brings no window in
    /// front of anybody else's.
    ///
    /// # Errors
    ///
    /// As [`ChromeServing::start`].
    pub fn start_telling(
        opening: Opening,
        size: (u32, u32),
        sites: AgentSites,
        frame: TellFrame,
    ) -> Result<Self, Unserved> {
        Self::begin(opening, size, Drawn::Headless, sites, Some(frame))
    }

    /// [`ChromeServing::start`], with the machine's Chrome shown: the page
    /// laid out at `size` CSS pixels in a window of Chrome's own, whose top
    /// left corner is put at `left`, `top` on the screen — and the frame of
    /// ours around it told on `frame` where to stand, as Chrome's window
    /// moves, hides and closes, and what each agent's verb was answered
    /// with.
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
        frame: TellFrame,
    ) -> Result<Self, Unserved> {
        Self::begin(
            opening,
            size,
            Drawn::Shown { left, top },
            sites,
            Some(frame),
        )
    }

    /// Start a Chrome drawing as `drawn` says, on a thread of its own.
    fn begin(
        opening: Opening,
        size: (u32, u32),
        drawn: Drawn,
        sites: AgentSites,
        frame: Option<TellFrame>,
    ) -> Result<Self, Unserved> {
        let (orders, results, inbox) = inbox::inbox();
        // Nothing here makes a request of this engine's network: no results
        // are ever sent.
        drop(results);
        let (tell, started) = channel();
        let thread = std::thread::Builder::new()
            .name("alo-chrome".to_owned())
            .spawn(move || {
                serve_in_chrome(
                    opening,
                    size,
                    drawn,
                    &sites,
                    frame.as_deref(),
                    &inbox,
                    &tell,
                )
            })
            .map_err(|why| Unserved::NoThread(why.kind()))?;
        match started.recv() {
            Ok(Ok(Started {
                product,
                placed,
                process,
                swept,
            })) => Ok(Self {
                orders,
                thread,
                rendered_by: RenderedBy::Chrome { product },
                placed,
                process,
                swept,
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

    /// Chrome's own process: the owner the window server names a shown
    /// Chrome's window by ([`crate::window_server::on_the_screen`]).
    pub fn process(&self) -> u32 {
        self.process
    }

    /// What starting its Chrome removed of the profiles killed browsers
    /// left, and what it could not — which `alo` says on the terminal
    /// (ADR 0069 § 3).
    pub fn swept(&self) -> &Swept {
        &self.swept
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

/// What the thread holding Chrome says once the page is open.
struct Started {
    /// Which Chrome, as it says itself.
    product: String,
    /// Where its window went, when it is shown.
    placed: Option<Placed>,
    /// Its process.
    process: u32,
    /// What its start removed of killed browsers' profiles.
    swept: Swept,
}

/// Start a Chrome drawing as `drawn` says, and ask it which it is: its
/// product, as it says itself.
fn named_chrome(drawn: Drawn) -> Result<(Chrome, String), Unserved> {
    let mut chrome = match drawn {
        Drawn::Headless => Chrome::start(),
        Drawn::Shown { .. } => Chrome::start_shown(),
    }
    .map_err(Unserved::Unstarted)?;
    let product = chrome.version().map_err(Unserved::Unnamed)?.product;
    Ok((chrome, product))
}

/// Start Chrome, open the page, say whether that worked — and which Chrome
/// it is — on `tell`, and answer agents until told to close, telling
/// `frame`, if there is one, where the frame around Chrome's window stands.
fn serve_in_chrome(
    opening: Opening,
    size: (u32, u32),
    drawn: Drawn,
    sites: &AgentSites,
    frame: Option<&(dyn Fn(ToFrame) -> bool + Send)>,
    inbox: &Receiver<Arrival>,
    tell: &Sender<Result<Started, Unserved>>,
) -> AgentRecord {
    let mut record = AgentRecord::new();
    let (mut chrome, product) = match named_chrome(drawn) {
        Ok(named) => named,
        Err(why) => {
            let _ = tell.send(Err(why));
            return record;
        }
    };
    let swept = chrome.swept().clone();
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
    let mut marker = placed.is_some().then(Marker::default);
    let mut pages = ChromePages::of(tab, product.clone());
    if tell
        .send(Ok(Started {
            product,
            placed,
            process: pages.tab().chrome_process(),
            swept,
        }))
        .is_err()
    {
        return record;
    }
    // Only a shown Chrome has a window to follow; a headless one waits on
    // its orders alone.
    let mut following = frame.filter(|_| placed.is_some()).map(|frame| Follower {
        frame,
        told: Following::default(),
        named: Naming::default(),
        next: Instant::now(),
    });
    loop {
        let until = following.as_ref().map(|follower| follower.next);
        let Some(arrived) = arrivals(inbox, until) else {
            return record;
        };
        for arrival in arrived {
            match arrival {
                Arrival::Order(Order::Agent { caller, ask, reply }) => {
                    Serving {
                        pages: &mut pages,
                        record: &mut record,
                        sites,
                        frame,
                        marker: marker.as_mut(),
                    }
                    .answer(caller, ask, &reply);
                }
                Arrival::Order(Order::CloseEverything | Order::Tabs(TabOrder::CloseSelected)) => {
                    if let Some(follower) = following.as_mut() {
                        // A Chrome that does not close its page is closed
                        // with this thread, its window with it.
                        let _ = chrome_window::close(pages.tab_mut());
                        follower.tell(Follow::Closed);
                    }
                    return record;
                }
                Arrival::Abandoned => return record,
                // The window's other orders, which Chrome's own window
                // carries out itself; and the network thread's, of which
                // there is none.
                Arrival::Order(_) | Arrival::Exchanged(_) | Arrival::NetworkGone => {}
            }
        }
        if let Some(follower) = following.as_mut()
            && Instant::now() >= follower.next
        {
            let now = Follow::of(&chrome_frame::read(pages.tab_mut()));
            follower.tell(now);
            if now == Follow::Closed {
                return record;
            }
            follower.name(chrome_named::read(pages.tab_mut()).ok().flatten());
            follower.next = Instant::now()
                .checked_add(FOLLOW_EVERY)
                .unwrap_or_else(Instant::now);
        }
    }
}

/// What an agent's ask is answered with, and by whom it is heard.
struct Serving<'s, 'c> {
    pages: &'s mut ChromePages<'c>,
    record: &'s mut AgentRecord,
    sites: &'s AgentSites,
    /// The frame told what agents did, if there is one.
    frame: Option<&'s (dyn Fn(ToFrame) -> bool + Send)>,
    /// The mark over a shown Chrome's page, which a headless one has none of.
    marker: Option<&'s mut Marker>,
}

impl Serving<'_, '_> {
    /// Serve `ask` from `caller`, answer it on `reply`, tell the frame the
    /// line it makes, and mark the box a verb acted on.
    fn answer(&mut self, caller: Caller, ask: Result<Ask, Refused>, reply: &Sender<Answer>) {
        let a_verb = matches!(ask, Ok(Ask::Act { .. }));
        // The document the verb is asked of, as Chrome last said, for a
        // mark only on the page it was done to.
        let before = self.marker.as_ref().and_then(|_| {
            // Unreadable, the next ask of the tab says so.
            let _ = self.pages.tab_mut().catch_up();
            self.pages.tab().document()
        });
        let served = serve(self.pages, self.record, self.sites, caller, ask);
        let line = AgentLine::of(a_verb, &served.answer);
        let marking = Marking::of(a_verb, &served.answer);
        // Nobody to answer is a caller that has gone, which changes nothing
        // about what was done or recorded.
        let _ = reply.send(served.answer);
        if let (Some(frame), Some(line)) = (self.frame, line) {
            // A frame that has gone is a window closing, whose orders say
            // so; nothing here waits for it.
            let _ = frame(ToFrame::Agent(line));
        }
        if let Some(marker) = self.marker.as_mut() {
            // A mark is downstream of the verb, which has been answered: one
            // Chrome would not draw is a mark a person does not see, and
            // Chrome gone is said by the next reading of where its window
            // is.
            let _ = marker.mark(self.pages.tab_mut(), &marking, before);
        }
    }
}

/// The frame around a shown Chrome's window, and what it was last told.
struct Follower<'f> {
    frame: &'f (dyn Fn(ToFrame) -> bool + Send),
    told: Following,
    /// What it was last told the page is called.
    named: Naming,
    /// When where it stands is next read.
    next: Instant,
}

impl Follower<'_> {
    /// Tell the frame `now`, if it is not what it was told last.
    fn tell(&mut self, now: Follow) {
        if let Some(changed) = self.told.heard(now) {
            // A frame that has gone is a window closing, whose orders say
            // so; nothing here waits for it.
            let _ = (self.frame)(ToFrame::Stand(changed));
        }
    }

    /// Tell the frame what the page is called, if a reading said and it
    /// is not what it was told last.
    fn name(&mut self, now: Option<Named>) {
        if let Some(changed) = self.named.heard(now) {
            // As above: nothing here waits for a frame that has gone.
            let _ = (self.frame)(ToFrame::Named(changed));
        }
    }
}
