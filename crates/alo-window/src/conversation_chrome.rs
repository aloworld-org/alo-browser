/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The address chosen from the conversation's one box, opened in the
//! machine's Chrome (queue item 713, ADR 0075).
//!
//! Return or *Go* on an address decides where to go
//! ([`crate::conversation::Conversation::going`], item 701). This opens it
//! as `alo --chrome ADDRESS` opens a page — through
//! [`ChromeServing`], so Chrome is started with our switches, every request
//! the page makes is held to our rules before it leaves and recorded (ADRs
//! 0058, 0060, 0073), and the page is drawn in Chrome's own `--app` window,
//! put on the screen by this process (ADR 0061) — from the conversation's
//! own process, with no second event loop.
//!
//! # Never waited on
//!
//! Starting Chrome and loading a page takes seconds, and the window's event
//! loop calls nothing that waits (ADR 0024 § 2). So the window hands the
//! address to a thread here ([`ChromeOpener::open`]), which returns at
//! once, and is told what came of it as an event like any other
//! ([`Went`]): opening, opened, why not, and closed. What it is told is
//! what the first screen's line says ([`Went::said`]).
//!
//! # No frame of ours
//!
//! `alo --chrome` floats a frame of ours beside Chrome's window, because
//! there it is the only window of ours (ADR 0062). Here the conversation's
//! window is ours and already says what happened, so Chrome's window is
//! shown alone ([`ChromeServing::start_shown_alone`]): the window server is
//! never asked where it is, and Chrome is asked only whether its window is
//! gone — which is said here as [`Went::Closed`].
//!
//! # One page at a time
//!
//! A second address chosen **replaces** the first: the Chrome showing it is
//! closed, waited for and its profile removed, and a new one opens the new
//! address. A conversation shows one page until item 702 decides where a
//! page goes inside it, and one [`ChromeServing`] holds one tab, with the
//! agent's record and grants for that tab; two would be two windows a
//! person did not ask for, with nothing of ours saying which is which.
//! Addresses chosen while a Chrome is still starting are not queued: the
//! newest is opened once it has.
//!
//! # What leaves this process
//!
//! The address, and nothing else a person typed: Chrome is told the
//! [`Url`] the box decided, as `alo --chrome` is told the one it was named.
//!
//! # Closed with the window
//!
//! [`ChromeOpener::finish`] is called as the conversation's window closes:
//! the Chrome showing a page is closed, waited for, and its profile removed
//! (ADR 0069) before it returns.

use crate::agent_sites::AgentSites;
use crate::chrome_follow::Follow;
use crate::chrome_protocol::Bounds;
use crate::chrome_serving::{ChromeServing, Opening, TellFrame};
use crate::conversation_place::PANEL_WIDTH;
use crate::frame_told::ToFrame;
use crate::opening::{CHROME_AT, CHROME_SIZE};
use crate::scale::Scale;
use alo_url::Url;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

/// Where Chrome's window is put, and the size its page is laid out at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placing {
    /// The window's top left corner, in the screen's points.
    pub at: (i32, i32),
    /// The page's size, in CSS pixels.
    pub size: (u32, u32),
}

impl Placing {
    /// Where `alo --chrome` puts it: when where the conversation's window
    /// is cannot be read.
    pub const APART: Self = Self {
        at: CHROME_AT,
        size: CHROME_SIZE,
    };

    /// Over the first screen of the conversation's window, leaving the
    /// chats panel seen: the window's top left corner where the window
    /// whose outer top left corner is at `outer` has its first screen
    /// begin, and the page as large as the first screen — `inner` less the
    /// panel. `outer` and `inner` are in device pixels, as the window
    /// server gives them to the window, at a screen's `scale_factor`.
    ///
    /// Chrome's title bar then lies over the conversation's, and its page
    /// over the first screen. Where a page goes *inside* the conversation
    /// is item 702's to measure and decide.
    pub fn over_the_first_screen(outer: (i32, i32), inner: (u32, u32), scale_factor: f64) -> Self {
        let scale = Scale::of_screen(scale_factor);
        let points = |device: i32| {
            let whole = i32::try_from(scale.fitting(device.unsigned_abs())).unwrap_or(i32::MAX);
            if device < 0 { -whole } else { whole }
        };
        let width = scale.fitting(inner.0).saturating_sub(PANEL_WIDTH).max(1);
        let height = scale.fitting(inner.1).max(1);
        let panel = i32::try_from(PANEL_WIDTH).unwrap_or(0);
        Self {
            at: (points(outer.0).saturating_add(panel), points(outer.1)),
            size: (width, height),
        }
    }
}

/// What became of an address chosen, as the window is told it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Went {
    /// Chrome is being started at it.
    Opening(Url),
    /// It is open in Chrome's own window.
    Opened {
        /// The address.
        url: Url,
        /// The Chrome showing it.
        shown: Shown,
    },
    /// It could not be opened, and why, in this browser's words.
    Unopened {
        /// The address.
        url: Url,
        /// Why.
        why: String,
    },
    /// Chrome's window showing it was closed — by a person, or Chrome
    /// itself gone — and the Chrome showing it with it.
    Closed(Url),
}

impl Went {
    /// What the first screen's line says of it, in plain words.
    pub fn said(&self) -> String {
        match self {
            Self::Opening(url) => format!("opening {url} in Chrome, in a window of its own"),
            Self::Opened { url, shown } => {
                format!("{url} is open in Chrome's own window — {}", shown.by)
            }
            Self::Unopened { url, why } => format!("{url} could not be opened: {why}"),
            Self::Closed(url) => format!("Chrome's window showing {url} was closed"),
        }
    }
}

/// The Chrome an address is open in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// Which Chrome, as it says itself, and that this browser drives it.
    pub by: String,
    /// Its process.
    pub process: u32,
    /// Its profile, removed once it has ended.
    pub profile: PathBuf,
    /// Where its window went, by Chrome's word, once it stopped moving.
    pub window: Option<Bounds>,
}

/// How the thread tells the window what became of an address: whether
/// anybody heard.
pub type TellWent = Box<dyn Fn(Went) -> bool + Send>;

/// A Chrome started at an address, and how to close it: a function that
/// closes it, waits for it and answers whether its thread finished.
struct Held {
    shown: Shown,
    close: Box<dyn FnOnce() -> bool + Send>,
}

/// How a Chrome is started at an address: the machine's, or a test's.
type Starter = Box<dyn FnMut(&Url, Placing, TellFrame) -> Result<Held, String> + Send>;

/// What the window hands the thread.
enum Work {
    /// Open this address, put as this says.
    Open(Url, Placing),
    /// The Chrome started at the open this numbers said its window is gone.
    Gone(u64),
    /// The window is closing: close Chrome and end.
    Finish,
}

/// A thread opening the addresses chosen in the conversation in the
/// machine's Chrome, one at a time.
#[derive(Debug)]
pub struct ChromeOpener {
    /// Where work is sent; [`None`] once finished.
    asked: Option<Sender<Work>>,
    thread: Option<JoinHandle<bool>>,
}

impl ChromeOpener {
    /// Start a thread that opens each address it is handed in the
    /// machine's Chrome, shown and with no frame of ours, and tells the
    /// window what became of it with `tell`. Chrome is not started until an
    /// address is.
    ///
    /// # Errors
    ///
    /// What the system said when no thread could be started.
    pub fn start(tell: TellWent) -> std::io::Result<Self> {
        Self::starting_with(Box::new(in_chrome), tell)
    }

    /// [`ChromeOpener::start`], each Chrome started by `starter`.
    fn starting_with(starter: Starter, tell: TellWent) -> std::io::Result<Self> {
        let (asked, asking) = channel();
        let again = asked.clone();
        let thread = std::thread::Builder::new()
            .name("alo-conversation-chrome".to_owned())
            .spawn(move || hold(starter, &asking, &again, &tell))?;
        Ok(Self {
            asked: Some(asked),
            thread: Some(thread),
        })
    }

    /// Open `url`, its window put as `placing` says, replacing whatever was
    /// open. Returns at once: what became of it is told to the window.
    pub fn open(&self, url: Url, placing: Placing) {
        if let Some(sender) = &self.asked {
            // A thread that has gone is a window that is closing, which
            // nothing here waits for.
            let _ = sender.send(Work::Open(url, placing));
        }
    }

    /// Close the Chrome showing a page, if one is, wait for it — its profile
    /// removed — and stop the thread: whether it ended rather than
    /// panicked, which the lints make unreachable and is said rather than
    /// assumed.
    ///
    /// Called as the window's event loop ends. A Chrome still starting is
    /// waited for first, for as long as Chrome is given to answer.
    pub fn finish(mut self) -> bool {
        if let Some(sender) = self.asked.take() {
            let _ = sender.send(Work::Finish);
        }
        self.thread
            .take()
            .is_none_or(|thread| thread.join().is_ok_and(|closed| closed))
    }
}

impl Drop for ChromeOpener {
    /// Never finished: the thread is still told to close Chrome, and is not
    /// waited for.
    fn drop(&mut self) {
        if let Some(sender) = self.asked.take() {
            let _ = sender.send(Work::Finish);
        }
    }
}

/// The open Chrome, as the thread keeps it.
struct Open {
    /// Which open it was, counted from one.
    number: u64,
    url: Url,
    held: Held,
}

/// Do the work `asking` sends, in order — of the addresses waiting, only
/// the newest — telling the window what became of each, until it is told
/// to finish: whether every Chrome it closed had a thread that finished.
fn hold(
    mut starter: Starter,
    asking: &Receiver<Work>,
    again: &Sender<Work>,
    tell: &TellWent,
) -> bool {
    let mut open: Option<Open> = None;
    let mut opened = 0u64;
    let mut finished = true;
    while let Ok(first) = asking.recv() {
        let waiting: Vec<Work> = core::iter::once(first).chain(asking.try_iter()).collect();
        let newest = waiting
            .iter()
            .rposition(|work| matches!(work, Work::Open(..)));
        for (place, work) in waiting.into_iter().enumerate() {
            match work {
                Work::Finish => {
                    if let Some(open) = open.take() {
                        finished &= (open.held.close)();
                    }
                    return finished;
                }
                Work::Gone(number) => {
                    if open.as_ref().is_some_and(|open| open.number == number)
                        && let Some(gone) = open.take()
                    {
                        finished &= (gone.held.close)();
                        // A window that has gone is told nothing more.
                        let _ = tell(Went::Closed(gone.url));
                    }
                }
                Work::Open(url, placing) if Some(place) == newest => {
                    if let Some(replaced) = open.take() {
                        finished &= (replaced.held.close)();
                    }
                    opened += 1;
                    let _ = tell(Went::Opening(url.clone()));
                    let number = opened;
                    let gone = again.clone();
                    let told: TellFrame = Box::new(move |told| {
                        if told == ToFrame::Stand(Follow::Closed) {
                            // Nobody to hear it is a thread that has ended.
                            let _ = gone.send(Work::Gone(number));
                        }
                        true
                    });
                    match starter(&url, placing, told) {
                        Ok(held) => {
                            let _ = tell(Went::Opened {
                                url: url.clone(),
                                shown: held.shown.clone(),
                            });
                            open = Some(Open { number, url, held });
                        }
                        Err(why) => {
                            let _ = tell(Went::Unopened { url, why });
                        }
                    }
                }
                // An address a newer one replaced before it was opened.
                Work::Open(..) => {}
            }
        }
    }
    // Every sender gone, which the thread's own keeps from happening.
    if let Some(open) = open.take() {
        finished &= (open.held.close)();
    }
    finished
}

/// Start the machine's Chrome at `url`, shown with no frame of ours and
/// put as `placing` says, held as `alo --chrome` holds its page.
fn in_chrome(url: &Url, placing: Placing, told: TellFrame) -> Result<Held, String> {
    let serving = ChromeServing::start_shown_alone(
        Opening::Network(url.clone()),
        placing.size,
        placing.at,
        // No socket is opened for the conversation's pages (queue item
        // 716): no agent reaches this tab, so it is kept to no site of its
        // own.
        AgentSites::Every,
        told,
    )
    .map_err(|why| why.to_string())?;
    let shown = Shown {
        by: serving.rendered_by().to_string(),
        process: serving.process(),
        profile: serving.profile().to_path_buf(),
        window: serving.placed().map(|placed| placed.window.bounds),
    };
    let orders = serving.orders();
    Ok(Held {
        shown,
        close: Box::new(move || {
            // Chrome's thread gone is Chrome already closing.
            let _ = orders.send(crate::message::Order::CloseEverything);
            drop(orders);
            serving.records().is_some()
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn url(text: &str) -> Url {
        alo_url::parse(text).unwrap()
    }

    /// What a test's starter did: each address started, each closed, and
    /// the way each Chrome tells that its window is gone.
    #[derive(Default)]
    struct Did {
        started: Vec<String>,
        closed: Vec<String>,
        told: Vec<TellFrame>,
    }

    /// A starter that refuses `refused` and starts anything else, writing
    /// down what it did in `did`.
    fn starter(did: &Arc<Mutex<Did>>, refused: &'static str) -> Starter {
        let did = Arc::clone(did);
        Box::new(move |url, _, told| {
            let address = url.to_string();
            if address == refused {
                return Err("Chrome is not installed on this machine".to_owned());
            }
            let mut doing = did.lock().unwrap();
            doing.started.push(address.clone());
            doing.told.push(told);
            let closing = Arc::clone(&did);
            Ok(Held {
                shown: Shown {
                    by: "Chrome/156.0.0.0, driven by this browser".to_owned(),
                    process: 7,
                    profile: PathBuf::from("/nowhere"),
                    window: None,
                },
                close: Box::new(move || {
                    closing.lock().unwrap().closed.push(address);
                    true
                }),
            })
        })
    }

    /// An opener with a test's starter, and what it tells the window.
    fn opener(did: &Arc<Mutex<Did>>, refused: &'static str) -> (ChromeOpener, Receiver<Went>) {
        let (tell, told) = channel();
        let opener = ChromeOpener::starting_with(
            starter(did, refused),
            Box::new(move |went| tell.send(went).is_ok()),
        )
        .unwrap();
        (opener, told)
    }

    fn next(told: &Receiver<Went>) -> Went {
        told.recv_timeout(Duration::from_secs(10)).unwrap()
    }

    #[test]
    fn an_address_chosen_is_said_opening_then_opened() {
        let did = Arc::new(Mutex::new(Did::default()));
        let (opener, told) = opener(&did, "");
        opener.open(url("https://example.com/"), Placing::APART);
        let opening = next(&told);
        assert_eq!(opening, Went::Opening(url("https://example.com/")));
        assert_eq!(
            opening.said(),
            "opening https://example.com/ in Chrome, in a window of its own"
        );
        let open = next(&told);
        assert!(
            matches!(&open, Went::Opened { url: at, .. } if at.to_string() == "https://example.com/")
        );
        assert_eq!(
            open.said(),
            "https://example.com/ is open in Chrome's own window — \
             Chrome/156.0.0.0, driven by this browser"
        );
        assert!(opener.finish());
        let did = did.lock().unwrap();
        assert_eq!(did.started, vec!["https://example.com/"]);
        assert_eq!(
            did.closed,
            vec!["https://example.com/"],
            "closed with the window"
        );
    }

    #[test]
    fn an_address_that_cannot_be_opened_is_said_why() {
        let did = Arc::new(Mutex::new(Did::default()));
        let (opener, told) = opener(&did, "https://nochrome.example/");
        opener.open(url("https://nochrome.example/"), Placing::APART);
        let _ = next(&told);
        let unopened = next(&told);
        assert_eq!(
            unopened.said(),
            "https://nochrome.example/ could not be opened: \
             Chrome is not installed on this machine"
        );
        assert!(opener.finish());
        assert!(did.lock().unwrap().closed.is_empty(), "nothing to close");
    }

    #[test]
    fn a_second_address_replaces_the_first() {
        let did = Arc::new(Mutex::new(Did::default()));
        let (opener, told) = opener(&did, "");
        opener.open(url("https://one.example/"), Placing::APART);
        let _ = (next(&told), next(&told));
        opener.open(url("https://two.example/"), Placing::APART);
        assert_eq!(next(&told), Went::Opening(url("https://two.example/")));
        assert_eq!(
            did.lock().unwrap().closed,
            vec!["https://one.example/"],
            "the first was closed before the second was started"
        );
        assert!(matches!(next(&told), Went::Opened { .. }));
        // The first Chrome saying its window went, as it closed, is not the
        // second's window closing.
        let first = did.lock().unwrap().told.remove(0);
        assert!(first(ToFrame::Stand(Follow::Closed)));
        assert!(opener.finish());
        assert_eq!(
            did.lock().unwrap().closed,
            vec!["https://one.example/", "https://two.example/"]
        );
        assert!(told.try_recv().is_err(), "nothing said of the first going");
    }

    #[test]
    fn chromes_window_closed_by_a_person_is_said_and_chrome_closed() {
        let did = Arc::new(Mutex::new(Did::default()));
        let (opener, told) = opener(&did, "");
        opener.open(url("https://example.com/"), Placing::APART);
        let _ = (next(&told), next(&told));
        // Only a window gone is heard; where to stand is never told here.
        let window = did.lock().unwrap().told.remove(0);
        assert!(window(ToFrame::Stand(Follow::Aside)));
        assert!(window(ToFrame::Stand(Follow::Closed)));
        let closed = next(&told);
        assert_eq!(closed, Went::Closed(url("https://example.com/")));
        assert_eq!(
            closed.said(),
            "Chrome's window showing https://example.com/ was closed"
        );
        assert_eq!(did.lock().unwrap().closed, vec!["https://example.com/"]);
        assert!(opener.finish());
        assert_eq!(
            did.lock().unwrap().closed.len(),
            1,
            "not closed a second time"
        );
    }

    #[test]
    fn of_addresses_chosen_while_chrome_starts_only_the_newest_is_opened() {
        let did = Arc::new(Mutex::new(Did::default()));
        let (tell, told) = channel();
        let (asked, asking) = channel();
        // Three sent before the thread reads any.
        for each in [
            "https://a.example/",
            "https://b.example/",
            "https://c.example/",
        ] {
            asked.send(Work::Open(url(each), Placing::APART)).unwrap();
        }
        asked.send(Work::Finish).unwrap();
        let tell: TellWent = Box::new(move |went| tell.send(went).is_ok());
        assert!(hold(starter(&did, ""), &asking, &asked, &tell));
        assert_eq!(did.lock().unwrap().started, vec!["https://c.example/"]);
        assert_eq!(next(&told), Went::Opening(url("https://c.example/")));
    }

    #[test]
    fn over_the_first_screen_leaves_the_panel_seen() {
        // The design's window at one, its outer corner at 100,50.
        assert_eq!(
            Placing::over_the_first_screen((100, 50), (1280, 820), 1.0),
            Placing {
                at: (340, 50),
                size: (1040, 820)
            }
        );
        // The same window on a Retina screen, in device pixels.
        assert_eq!(
            Placing::over_the_first_screen((200, 100), (2560, 1640), 2.0),
            Placing {
                at: (340, 50),
                size: (1040, 820)
            }
        );
        // On a screen left of the main one, its points are negative.
        assert_eq!(
            Placing::over_the_first_screen((-3840, 0), (2560, 1640), 2.0).at,
            (-1680, 0)
        );
        // A window narrower than the panel still asks for a page.
        assert_eq!(
            Placing::over_the_first_screen((0, 0), (100, 100), 1.0).size,
            (1, 100)
        );
    }
}
