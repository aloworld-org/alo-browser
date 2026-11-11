/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The agent socket reaching a tab in Chrome (ADR 0058 § 4, ADR 0055, queue
//! item 630).
//!
//! A thread of its own holds the machine's Chrome — or the Chromium the
//! application carries, never the machine's, inside the application
//! ([`Chromium`], ADR 0077) — and the one tab open in it, and is sent [`Order::Agent`] as the conductor is — by the same
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
//! # Shown with no frame of ours
//!
//! [`ChromeServing::start_shown_alone`] holds a shown Chrome whose window
//! has no frame of ours around it, because a window of ours is already the
//! interface — the conversation's (ADR 0075, queue item 713). Nothing then
//! stands beside Chrome's window, so the window server is never asked where
//! it is; only Chrome is, every [`WATCH_EVERY`], and only whether its
//! window is gone ([`chrome_frame::gone`]), which is told as
//! [`Follow::Closed`] and ends the thread as above. Its page is not named
//! to anybody: the conversation draws no stranger's title in this process.
//!
//! # Shown as a card in the conversation
//!
//! [`ChromeServing::start_as_a_card`] holds a shown Chrome whose window is
//! parked off every display, its page laid out at the card's size, and
//! streams the page's frames when the conversation asks
//! ([`crate::message::Order::Card`], [`crate::chrome_card`], ADR 0076 B,
//! queue item 717). While it streams, this thread reads Chrome every
//! [`chrome_card::CAST_EVERY`] between orders, answering every frame and telling the
//! newest; the stream is stopped, and Chrome's answer heard, before the
//! page is closed.
//!
//! The other way, ours closed — [`Order::CloseEverything`], or the one
//! browser key that closes a tab, ⌘W, since closing the last tab closes
//! the window (ADR 0024 § 6) — closes Chrome's page, which is its window,
//! tells the frame [`Follow::Closed`] and ends the thread, and Chrome with
//! it. Every sender of orders gone ends it too.
//!
//! # The sign-ins kept
//!
//! A card started with a jar ([`ChromeServing::start_as_a_kept_card`],
//! ADR 0111) has the jar's cookies put into its Chromium before its page
//! opens, and this thread looks at its Chromium's cookies every
//! [`chrome_sign_ins::EVERY`] between orders, and once more as it ends,
//! making what changed to the jar ([`crate::chrome_sign_ins`]). What could
//! not be is said on the terminal, and the card goes on.
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
//! queue item 680). A headless one is not: nobody would see it; nor is a
//! card's page, whose frames would carry Chrome's inspector tooltip into
//! the conversation (queue item 822) — until it is expanded into Chrome's
//! own window. `CardOrder::Unmark` takes any mark away.
//!
//! # What it is not
//!
//! Chrome's window. A person's pointer and keys reach the page there
//! without passing this process, so the window's other orders — a resize,
//! a pointer, the other tab keys — are not carried out here: Chrome lays
//! its page out again at the size a person gives its window itself.

use crate::agent_lines::AgentLine;
use crate::agent_record::{AgentRecord, Caller};
use crate::agent_scope::AgentScope;
use crate::asking::{Answer, Ask, Refused};
use crate::beside::Chromium;
use crate::chrome::{self, Chrome, Failed, Unstarted};
use crate::chrome_card;
use crate::chrome_follow::{FOLLOW_EVERY, Follow, Following};
use crate::chrome_frame;
use crate::chrome_mark::{Marker, Marking};
use crate::chrome_named::{self, Naming};
use crate::chrome_pages::ChromePages;
use crate::chrome_park;
use crate::chrome_pin::Pinned;
use crate::chrome_profile::Swept;
use crate::chrome_protocol::Named;
use crate::chrome_sign_ins;
use crate::chrome_tab::{Tab, Unopened};
use crate::chrome_window::{self, Placed, Unplaced};
use crate::frame_told::ToFrame;
use crate::inbox::{self, Arrival, Orders, arrivals};
use crate::message::{Order, TabOrder};
use crate::pages::RenderedBy;
use crate::scale::Scale;
use crate::serving::serve;
use crate::sign_ins::SignIns;
use alo_url::Url;
use core::fmt;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often a shown Chrome with no frame of ours is asked whether its
/// window is gone: one reading over the pipe and no window server's, so a
/// person who closed it is heard within about a second.
pub const WATCH_EVERY: Duration = Duration::from_secs(1);

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
    profile: PathBuf,
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
        /// Whether a frame of ours stands beside its window and follows it
        /// (ADR 0062), or none does and only its closing is watched for
        /// (ADR 0075).
        framed: bool,
        /// Whether its window is brought in front of every other, or left
        /// where it is put — parked off every display while its page is a
        /// card in the conversation (ADR 0076 B4, queue item 717).
        front: bool,
        /// The scale its window is drawn at wherever it is — a card's, the
        /// conversation's (ADR 0090, queue item 738) — or, with none, its
        /// display's.
        scale: Option<Scale>,
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
    pub fn start(
        opening: Opening,
        size: (u32, u32),
        sites: impl Into<AgentScope>,
    ) -> Result<Self, Unserved> {
        Self::begin(
            opening,
            size,
            (Chromium::Machines, Drawn::Headless),
            sites,
            None,
        )
    }

    /// [`ChromeServing::start`], headless, in the Chromium `chromium` names
    /// — the machine's Chrome, or the one the application carries — so that
    /// a test reads the socket through either (queue item 956).
    ///
    /// # Errors
    ///
    /// As [`ChromeServing::start`].
    pub fn start_from(
        chromium: Chromium,
        opening: Opening,
        size: (u32, u32),
        sites: impl Into<AgentScope>,
    ) -> Result<Self, Unserved> {
        Self::begin(opening, size, (chromium, Drawn::Headless), sites, None)
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
        sites: impl Into<AgentScope>,
        frame: TellFrame,
    ) -> Result<Self, Unserved> {
        Self::begin(
            opening,
            size,
            (Chromium::Machines, Drawn::Headless),
            sites,
            Some(frame),
        )
    }

    /// [`ChromeServing::start`], with the Chromium `chromium` names shown —
    /// the machine's Chrome, or inside the application only its own (ADR
    /// 0077): the page laid out at `size` CSS pixels in a window of
    /// Chrome's own, whose top left corner is put at `left`, `top` on the
    /// screen — and the frame of ours around it told on `frame` where to
    /// stand, as Chrome's window moves, hides and closes, and what each
    /// agent's verb was answered with.
    ///
    /// # Errors
    ///
    /// As [`ChromeServing::start`], and [`Unserved::Unplaced`] when the
    /// window was not put there.
    pub fn start_shown(
        chromium: Chromium,
        opening: Opening,
        size: (u32, u32),
        (left, top): (i32, i32),
        sites: impl Into<AgentScope>,
        frame: TellFrame,
    ) -> Result<Self, Unserved> {
        Self::begin(
            opening,
            size,
            (
                chromium,
                Drawn::Shown {
                    left,
                    top,
                    framed: true,
                    front: true,
                    scale: None,
                },
            ),
            sites,
            Some(frame),
        )
    }

    /// [`ChromeServing::start_shown`], with no frame of ours around
    /// Chrome's window (ADR 0075): `told` hears what each agent's verb was
    /// answered with and, once Chrome's window is gone — closed by a person,
    /// by this browser, or Chrome itself gone — [`Follow::Closed`], and
    /// never where to stand or what the page is called.
    ///
    /// # Errors
    ///
    /// As [`ChromeServing::start_shown`].
    pub fn start_shown_alone(
        chromium: Chromium,
        opening: Opening,
        size: (u32, u32),
        (left, top): (i32, i32),
        sites: impl Into<AgentScope>,
        told: TellFrame,
    ) -> Result<Self, Unserved> {
        Self::begin(
            opening,
            size,
            (
                chromium,
                Drawn::Shown {
                    left,
                    top,
                    framed: false,
                    front: true,
                    scale: None,
                },
            ),
            sites,
            Some(told),
        )
    }

    /// [`ChromeServing::start_shown_alone`], for a page the conversation
    /// shows as a card (ADR 0076 B, queue item 717): laid out at `size` CSS
    /// pixels — the card's page box — in Chrome's own window, which is
    /// parked ([`chrome_window::PARKED`]) — drawn transparent there by the
    /// Chromium alo carries, which is told it holds a card
    /// ([`crate::chrome::CARD`], queue item 771) — and never brought in
    /// front until the card is expanded
    /// ([`crate::message::CardOrder::Expand`]). Nothing is streamed until
    /// the conversation asks ([`crate::message::CardOrder::Stream`]); what
    /// is streamed, and that it stopped, is told on `told` beside what
    /// [`ChromeServing::start_shown_alone`] tells — and, while it streams,
    /// where its page is and whether it is still loading.
    ///
    /// A page from the network returns once its navigation has committed,
    /// not once it has loaded ([`Tab::open_as_a_card`], queue item 751), so
    /// a slow page is streamed from its first paint.
    ///
    /// The page is drawn at `scale`, the conversation's window's: the
    /// machine's Chrome is told it, and draws at it wherever its parked
    /// window is, for its life (ADR 0090, queue item 738) — the window
    /// server parks a window by a display of its own choosing, which is the
    /// conversation's only by chance; the Chromium alo carries is told no
    /// scale and its window is parked by a display at `scale`
    /// ([`chrome_park`], ADR 0090 as amended, queue item 772), and parked
    /// again when a stream is asked at another.
    ///
    /// # Errors
    ///
    /// As [`ChromeServing::start_shown`].
    pub fn start_as_a_card(
        chromium: Chromium,
        opening: Opening,
        size: (u32, u32),
        scale: Scale,
        sites: impl Into<AgentScope>,
        told: TellFrame,
    ) -> Result<Self, Unserved> {
        Self::start_as_a_pinned_card(chromium, opening, (size, scale), sites, None, told)
    }

    /// [`ChromeServing::start_as_a_pinned_card`], its Chromium given the
    /// sign-ins `kept` holds before its page opens and keeping its own in
    /// it as it runs (ADR 0111, [`crate::chrome_sign_ins`]); with none, as
    /// that.
    ///
    /// # Errors
    ///
    /// As [`ChromeServing::start_shown`].
    pub fn start_as_a_kept_card(
        chromium: Chromium,
        opening: Opening,
        (size, scale): ((u32, u32), Scale),
        (sites, pinned, kept): (impl Into<AgentScope>, Option<Pinned>, Option<Arc<SignIns>>),
        told: TellFrame,
    ) -> Result<Self, Unserved> {
        let (left, top) = chrome_window::PARKED;
        Self::begin_pinned(
            opening,
            size,
            (
                chromium,
                Drawn::Shown {
                    left,
                    top,
                    framed: false,
                    front: false,
                    scale: Some(scale),
                },
            ),
            (sites.into(), pinned, kept),
            Some(told),
        )
    }

    /// [`ChromeServing::start_as_a_card`], its Chromium taking the name
    /// `pinned` holds as the address it holds, never asking a resolver for
    /// it (queue item 881, [`crate::chrome_pin`]): a page alo's agent opens.
    ///
    /// # Errors
    ///
    /// As [`ChromeServing::start_shown`].
    pub fn start_as_a_pinned_card(
        chromium: Chromium,
        opening: Opening,
        (size, scale): ((u32, u32), Scale),
        sites: impl Into<AgentScope>,
        pinned: Option<Pinned>,
        told: TellFrame,
    ) -> Result<Self, Unserved> {
        let (left, top) = chrome_window::PARKED;
        Self::begin_pinned(
            opening,
            size,
            (
                chromium,
                Drawn::Shown {
                    left,
                    top,
                    framed: false,
                    front: false,
                    scale: Some(scale),
                },
            ),
            (sites.into(), pinned, None),
            Some(told),
        )
    }

    /// Start the Chromium named, drawing as `drawn` says, on a thread of
    /// its own.
    fn begin(
        opening: Opening,
        size: (u32, u32),
        named: (Chromium, Drawn),
        sites: impl Into<AgentScope>,
        frame: Option<TellFrame>,
    ) -> Result<Self, Unserved> {
        Self::begin_pinned(opening, size, named, (sites.into(), None, None), frame)
    }

    /// [`ChromeServing::begin`], the Chromium taking the name `pinned`
    /// holds as its address (queue item 881).
    fn begin_pinned(
        opening: Opening,
        size: (u32, u32),
        (chromium, drawn): (Chromium, Drawn),
        (sites, pinned, kept): (AgentScope, Option<Pinned>, Option<Arc<SignIns>>),
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
                    (opening, kept),
                    size,
                    (&chromium, drawn, pinned.as_ref()),
                    &sites,
                    frame.as_deref(),
                    &inbox,
                    &tell,
                )
            })
            .map_err(|why| Unserved::NoThread(why.kind()))?;
        match started.recv() {
            Ok(Ok(Started {
                rendered_by,
                placed,
                process,
                profile,
                swept,
            })) => Ok(Self {
                orders,
                thread,
                rendered_by,
                placed,
                process,
                profile,
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

    /// Where its Chrome's profile is: removed once Chrome has ended, as
    /// [`ChromeServing::records`] returns (ADR 0069).
    pub fn profile(&self) -> &std::path::Path {
        &self.profile
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
    /// Which Chrome, as it says itself, and whose it is.
    rendered_by: RenderedBy,
    /// Where its window went, when it is shown.
    placed: Option<Placed>,
    /// Its process.
    process: u32,
    /// Its profile's directory.
    profile: PathBuf,
    /// What its start removed of killed browsers' profiles.
    swept: Swept,
}

/// Start the Chromium `chromium` names drawing as `drawn` says — a card's
/// taking the name `pinned` holds as its address — and ask it which it is:
/// its product, as it says itself, and whose it is — the machine's Chrome,
/// or the application's own (ADR 0077) — given the sign-ins `kept` holds,
/// if there is a jar (ADR 0111).
fn named_chrome(
    chromium: &Chromium,
    drawn: Drawn,
    pinned: Option<&Pinned>,
    kept: Option<Arc<SignIns>>,
) -> Result<(Chrome, RenderedBy, Option<chrome_sign_ins::Keeping>), Unserved> {
    let chrome = match drawn {
        Drawn::Shown {
            scale: Some(scale), ..
        } => Chrome::start_shown_at(chromium, scale, pinned),
        Drawn::Shown { .. } => Chrome::start_from(chromium, true),
        Drawn::Headless => Chrome::start_from(chromium, false),
    };
    let mut chrome = chrome.map_err(Unserved::Unstarted)?;
    let product = chrome.version().map_err(Unserved::Unnamed)?.product;
    let rendered_by = match chromium {
        Chromium::Machines => RenderedBy::Chrome { product },
        Chromium::Carried(_) => RenderedBy::Carried { product },
    };
    // The sign-ins kept, given before any page opens (ADR 0111).
    let keeping = given_to(&mut chrome, kept);
    Ok((chrome, rendered_by, keeping))
}

/// Start Chrome, open the page, say whether that worked — and which Chrome
/// it is — on `tell`, and answer agents until told to close, telling
/// `frame`, if there is one, where the frame around Chrome's window stands.
fn serve_in_chrome(
    (opening, kept): (Opening, Option<Arc<SignIns>>),
    size: (u32, u32),
    (chromium, drawn, pinned): (&Chromium, Drawn, Option<&Pinned>),
    sites: &AgentScope,
    frame: Option<&(dyn Fn(ToFrame) -> bool + Send)>,
    inbox: &Receiver<Arrival>,
    tell: &Sender<Result<Started, Unserved>>,
) -> AgentRecord {
    let mut record = AgentRecord::new();
    let (mut chrome, rendered_by, mut keeping) = match named_chrome(chromium, drawn, pinned, kept) {
        Ok(named) => named,
        Err(why) => {
            let _ = tell.send(Err(why));
            return record;
        }
    };
    let swept = chrome.swept().clone();
    let profile = chrome.profile().to_path_buf();
    let mut tab = match open(&mut chrome, opening, size, drawn) {
        Ok(tab) => tab,
        Err(why) => {
            let _ = tell.send(Err(Unserved::Unopened(why)));
            return record;
        }
    };
    let (placed, scaled) = match placed_and_scaled(&mut tab, chromium, drawn) {
        Ok(both) => both,
        Err(why) => {
            let _ = tell.send(Err(Unserved::Unplaced(why)));
            return record;
        }
    };
    // Over a card's page Chrome draws nothing (queue item 822).
    let mut marker = placed
        .is_some()
        .then(|| Marker::over(matches!(drawn, Drawn::Shown { front: false, .. })));
    let mut pages = ChromePages::of(tab, rendered_by.clone());
    if tell
        .send(Ok(Started {
            rendered_by,
            placed,
            process: pages.tab().chrome_process(),
            profile,
            swept,
        }))
        .is_err()
    {
        return record;
    }
    // Only a shown Chrome has a window to follow; a headless one waits on
    // its orders alone.
    let framed = matches!(drawn, Drawn::Shown { framed: true, .. });
    let mut following = frame.filter(|_| placed.is_some()).map(|frame| Follower {
        frame,
        framed,
        told: Following::default(),
        named: Naming::default(),
        next: Instant::now(),
    });
    // What the card was last told of its page (queue items 742, 751, 737,
    // 736).
    let mut told_card = chrome_card::Told::default();
    // And the size its page was last laid out at (queue item 774), and
    // whether its window is still parked (queue item 771).
    let mut fitted = chrome_card::Fitted::parked(size, scaled);
    loop {
        let next = following.as_ref().map(|follower| follower.next);
        let until = looked_by(keeping.as_mut(), pages.tab_mut(), next);
        let Some(arrived) = arrivals(inbox, until) else {
            ending(pages.tab_mut(), frame, keeping.as_mut());
            return record;
        };
        for arrival in chrome_card::to_carry_out(arrived) {
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
                Arrival::Order(Order::Card(order)) => {
                    if let Some(marker) = marker.as_mut() {
                        marker.heed(pages.tab_mut(), &order);
                    }
                    chrome_card::carry_out(pages.tab_mut(), order, frame, &mut fitted);
                }
                Arrival::Order(Order::CloseEverything | Order::Tabs(TabOrder::CloseSelected)) => {
                    // A stream is stopped, and Chrome's word on it heard,
                    // before its page goes — and its sign-ins kept.
                    ending(pages.tab_mut(), frame, keeping.as_mut());
                    if let Some(follower) = following.as_mut() {
                        // A Chrome that does not close its page is closed
                        // with this thread, its window with it.
                        let _ = chrome_window::close(pages.tab_mut());
                        follower.tell(Follow::Closed);
                    }
                    return record;
                }
                Arrival::Abandoned => {
                    ending(pages.tab_mut(), frame, keeping.as_mut());
                    return record;
                }
                // The window's other orders, which Chrome's own window
                // carries out itself; and the network thread's, of which
                // there is none.
                Arrival::Order(_) | Arrival::Exchanged(_) | Arrival::NetworkGone => {}
            }
        }
        told_card.tell(pages.tab_mut(), frame);
        if let Some(follower) = following.as_mut()
            && Instant::now() >= follower.next
            && follower.follow(pages.tab_mut()) == Follow::Closed
        {
            return looked_last(record, keeping.as_mut(), pages.tab_mut());
        }
    }
}

/// `record`, once the card's Chromium's cookies were looked at once more
/// as its window went, if it keeps its sign-ins.
fn looked_last(
    record: AgentRecord,
    keeping: Option<&mut chrome_sign_ins::Keeping>,
    tab: &mut Tab<'_>,
) -> AgentRecord {
    looked(keeping, tab, true);
    record
}

/// The jar `kept`, if there is one, given to `chrome` before any page
/// opens (ADR 0111); one that could not be is said, and the card opens
/// signed in to nothing.
fn given_to(chrome: &mut Chrome, kept: Option<Arc<SignIns>>) -> Option<chrome_sign_ins::Keeping> {
    let jar = kept?;
    chrome_sign_ins::given(chrome, jar)
        .inspect_err(|why| {
            eprintln!(
                "alo: the card opens signed in to nothing: the sign-ins kept were not given to \
                 its Chromium ({why})"
            );
        })
        .ok()
}

/// When the thread holding `tab` next looks at Chrome, waiting `until`
/// ([`chrome_card::until`]): no later than its Chromium's cookies are next
/// to be looked at, when it keeps them — and they are looked at first, if
/// it is time.
fn looked_by(
    keeping: Option<&mut chrome_sign_ins::Keeping>,
    tab: &mut Tab<'_>,
    until: Option<Instant>,
) -> Option<Instant> {
    let until = chrome_card::until(tab, until);
    let Some(keeping) = keeping else {
        return until;
    };
    looked(Some(&mut *keeping), tab, false);
    let next = keeping.next();
    Some(until.map_or(next, |until| until.min(next)))
}

/// The card's Chromium's cookies looked at — `now`, or if it is time — if
/// it keeps its sign-ins, and what could not be said.
fn looked(keeping: Option<&mut chrome_sign_ins::Keeping>, tab: &mut Tab<'_>, now: bool) {
    let Some(keeping) = keeping else {
        return;
    };
    let looked = if now {
        keeping.look(tab)
    } else {
        keeping.look_if_due(tab)
    };
    if let Err(why) = looked {
        eprintln!("alo: {why}");
    }
}

/// A card's thread ending: its stream stopped, and Chrome's word on it
/// heard, before its page goes — and its sign-ins looked at once more.
fn ending(
    tab: &mut Tab<'_>,
    frame: Option<&(dyn Fn(ToFrame) -> bool + Send)>,
    keeping: Option<&mut chrome_sign_ins::Keeping>,
) {
    chrome_card::stop(tab, frame);
    looked(keeping, tab, true);
}

/// Open `opening` in `chrome` at `size`, for a Chrome drawing as `drawn`
/// says. Only a card's window is parked rather than brought in front, and a
/// card's page from the network is streamed from its first paint, not its
/// load (queue item 751): it is waited for only until it commits. A card's
/// tab refuses text put into a password field, for every agent.
fn open(
    chrome: &mut Chrome,
    opening: Opening,
    size: (u32, u32),
    drawn: Drawn,
) -> Result<Tab<'_>, Unopened> {
    let a_card = matches!(drawn, Drawn::Shown { front: false, .. });
    let mut tab = match opening {
        Opening::Frozen { url, page } => Tab::open_frozen(chrome, &url, page, size),
        Opening::Network(address) if a_card => Tab::open_as_a_card(chrome, &address, size),
        Opening::Network(address) => Tab::open(chrome, &address, size),
    }?;
    // A card's page: no agent types a password on it (queue item 704, ADR
    // 0096).
    if a_card {
        tab.keep_passwords();
        tab.show_as_a_card();
    }
    Ok(tab)
}

/// [`placing`], and how the scale its page is drawn at is had: a card of
/// the Chromium alo carries is parked by a display at its scale, which it
/// then draws at (queue item 772); every other window is put where it was
/// asked to be, and a card's Chromium was told its scale.
fn placed_and_scaled(
    tab: &mut Tab<'_>,
    chromium: &Chromium,
    drawn: Drawn,
) -> Result<(Option<Placed>, chrome_card::Scaled), Unplaced> {
    match drawn {
        Drawn::Shown {
            scale: Some(scale), ..
        } if !chrome::told_its_scale(chromium) => {
            let parked = chrome_park::park(tab, scale);
            Ok((
                Some(chrome_window::placed(tab)?),
                chrome_card::Scaled::ByItsDisplay(parked),
            ))
        }
        _ => Ok((placing(tab, drawn)?, chrome_card::Scaled::Told)),
    }
}

/// Put a shown Chrome's window where `drawn` says — in front of every other
/// window, or parked where nobody sees it while its page is a card — and
/// where it went; nothing for a headless one.
fn placing(tab: &mut Tab<'_>, drawn: Drawn) -> Result<Option<Placed>, Unplaced> {
    match drawn {
        Drawn::Headless => Ok(None),
        Drawn::Shown {
            left,
            top,
            front: true,
            ..
        } => chrome_window::place(tab, left, top).map(Some),
        Drawn::Shown { left, top, .. } => chrome_window::put_at(tab, left, top).map(Some),
    }
}

/// What an agent's ask is answered with, and by whom it is heard.
struct Serving<'s, 'c> {
    pages: &'s mut ChromePages<'c>,
    record: &'s mut AgentRecord,
    sites: &'s AgentScope,
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
    /// Whether it is a frame of ours that stands beside Chrome's window, or
    /// a listener told only that the window is gone (ADR 0075).
    framed: bool,
    told: Following,
    /// What it was last told the page is called.
    named: Naming,
    /// When where it stands is next read.
    next: Instant,
}

impl Follower<'_> {
    /// Read where Chrome's window of `tab` is, tell the frame what changed
    /// and when to read again: [`Follow::Closed`] once the window is gone.
    ///
    /// A frame of ours is told where to stand and what the page is called,
    /// every [`FOLLOW_EVERY`]; a listener with no frame (ADR 0075) is told
    /// only that the window is gone, read by Chrome's word alone every
    /// [`WATCH_EVERY`].
    fn follow(&mut self, tab: &mut Tab<'_>) -> Follow {
        let (now, every) = if self.framed {
            let now = Follow::of(&chrome_frame::read(tab));
            self.tell(now);
            if now != Follow::Closed {
                self.name(chrome_named::read(tab).ok().flatten());
            }
            (now, FOLLOW_EVERY)
        } else {
            // Unread is a Chrome that still holds the page: the next
            // reading asks again.
            let now = match chrome_frame::gone(tab) {
                Ok(Some(_)) => Follow::Closed,
                Ok(None) | Err(_) => Follow::Aside,
            };
            if now == Follow::Closed {
                self.tell(now);
            }
            (now, WATCH_EVERY)
        };
        self.next = Instant::now()
            .checked_add(every)
            .unwrap_or_else(Instant::now);
        now
    }

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
