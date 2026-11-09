/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Chrome, started and spoken to over a pipe (ADR 0058 § 3, queue item 619).
//!
//! The browser process starts the Chrome already installed on the machine
//! ([`crate::beside::chrome_places`]) — or, inside the application, only the
//! Chromium the application carries ([`Chromium::Carried`], ADR 0077) — and
//! speaks its DevTools protocol over
//! the process's own descriptors, with `--remote-debugging-pipe`. **Never a
//! port**: a page's own script can reach `localhost`, and ADR 0055 § 1
//! refused a port for the agent's socket for the same reason. Messages are
//! framed by [`crate::chrome_wire`] and read by [`crate::chrome_protocol`],
//! on a thread of their own, and held until taken by [`crate::chrome_held`]
//! — at most [`HELD_BYTES`] of them, whatever Chrome is telling of.
//!
//! # How the pipe reaches Chrome's descriptors 3 and 4
//!
//! Chrome reads commands from descriptor 3 and writes to descriptor 4. The
//! standard library can only place a child's 0, 1 and 2, and placing any
//! other needs code run between `fork` and `exec`, which is `unsafe` (law 4).
//! So Chrome is started through `/bin/sh`, whose own redirections move the
//! two pipes this process holds on Chrome's standard input and output to 3
//! and 4, point 0 and 1 at `/dev/null`, and `exec` Chrome in its own place —
//! so the process this one holds *is* Chrome, and nothing of the shell is
//! left running. The shell is handed Chrome's path and flags as arguments,
//! never as text it parses.
//!
//! # A profile of its own, and gone with the browser
//!
//! Each Chrome runs on a new, private profile, never the person's — their
//! cookies and sessions are not this browser's to hand a renderer, and
//! Chrome refuses to be driven on its default profile anyway. It is empty
//! but for the one file [`crate::chrome_profile`] writes first, which keeps
//! Chrome's own encrypted DNS off (ADR 0008, item 635). Its own DNS client is
//! switched off too ([`FEATURES_OFF`], item 650), so every name it looks up
//! is asked of the machine's resolver.
//!
//! # Its own traffic, switched off
//!
//! Chrome started bare asks Google for a dozen things before any page is
//! open, none of them a tab's and so none paused, judged or recorded. It is
//! started with the switches that stop them ([`OWN_TRAFFIC_OFF`], ADR 0073,
//! item 649); two still leave — `accounts.google.com/ListAccounts` and an
//! on-demand install of the on-device models' manifest — and are named as
//! not decided, as ADR 0060 § 2 names a WebSocket.
//! When this handle is dropped Chrome is asked to close, then killed if it has
//! not, then waited for, and its profile is removed. When the browser process
//! ends without dropping it — killed, or crashed — the pipe closes with it,
//! and Chrome ends when its pipe closes: both are asserted by
//! `tests/chrome_over_a_pipe.rs`. The profile such a browser left is removed
//! by the next Chrome started, since nobody holds its lock any more
//! ([`crate::chrome_profile`], ADR 0069): what that start removed is
//! [`Chrome::swept`], asserted by `tests/a_killed_browsers_chrome_profile.rs`.
//!
//! # Headless, or shown in a window of its own
//!
//! [`Chrome::start`] runs it headless, which is what every test of a page's
//! tree and verbs is read through. [`Chrome::start_shown`] runs it with a
//! window of its own on the screen, in `--app` mode — no tab strip and no
//! address bar, only a title bar over the page — which is how a person
//! looks at a page on the product path (ADR 0061). Its window opens on
//! `data:,`, an empty page that asks nothing of any network, and the one
//! tab opened in it is that window's page ([`crate::chrome_tab`]), placed
//! on the screen by [`crate::chrome_window`].
//!
//! # What it is not yet
//!
//! It answers `Browser.getVersion`, holds tabs ([`crate::chrome_tab`], items
//! 620 and 633) and closes; the verbs are carried out through it
//! ([`crate::chrome_act`], item 621).

use crate::beside::{Chromium, NotCarried, NotInstalled, chrome_on_this_machine, is_program};
use crate::chrome_held::{Taker, Untaken, held};
use crate::chrome_hold::Unheld;
use crate::chrome_pin::Pinned;
use crate::chrome_profile::{Profile, Swept, new_profile};
use crate::chrome_protocol::{self, Command, Declined, Malformed, Message, Version};
use crate::chrome_record::Record;
use crate::chrome_told::{self, Told};
use crate::chrome_wire::{self, Frame, Frames, MOST_MESSAGE, Unframed};
use crate::scale::Scale;
use core::fmt;
use std::io::ErrorKind;
use std::io::{BufRead as _, Read as _};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStderr, ChildStdin, Command as Process, Stdio};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// How long Chrome is waited for, to answer and to close, unless a caller
/// asks for another bound. Chrome 155 answers its first command in under a
/// second on this machine; a cold start of a large program on a loaded one
/// is given thirty.
pub const LONGEST_SILENCE: Duration = Duration::from_secs(30);

/// The switch telling Chromium to draw at `scale` whatever display its
/// window is on: Chromium's own, which its CEF reads as Chrome does
/// (measured, ADR 0090) — `--force-device-scale-factor=2`, the factor
/// written as the shortest number that reads back as it.
pub fn scaled(scale: Scale) -> String {
    format!("--force-device-scale-factor={}", scale.factor())
}

/// The switch alo's CEF host is started with when its window holds a page
/// shown as a card (ADR 0076 B4 as amended, queue item 771): the host opens
/// the window unseen — fully transparent, letting the pointer through — and
/// makes it seen once it is put where a person is to see it, which is
/// *Expand* (`crates/alo-cef-host`'s `parked.rs`). The host's own; the
/// machine's Chrome is never given it, having no such thing to do.
pub const CARD: &str = "--alo-card";

/// Whether a card's Chromium is told its scale at start ([`scaled`], ADR
/// 0090) and keeps it, or draws at the scale of the display its window is
/// parked by, parked by one at the conversation's (`chrome_park.rs`, ADR
/// 0090 as amended, queue item 772): the machine's Chrome is told, since a
/// strip of its window beside the conversation would be seen; the Chromium
/// alo carries, whose parked window is unseen ([`CARD`]), is not.
pub fn told_its_scale(chromium: &Chromium) -> bool {
    !matches!(chromium, Chromium::Carried(_))
}

/// What a card's Chromium is started with beside every Chromium's
/// switches: for the machine's Chrome, the conversation window's `scale`
/// ([`scaled`], ADR 0090), which it then keeps; for the Chromium alo
/// carries, [`CARD`] and no scale, which a switch would fix for its life
/// (measured, queue item 772) — its window is parked by a display at the
/// scale instead ([`told_its_scale`]).
pub fn as_a_card(chromium: &Chromium, scale: Scale) -> Vec<String> {
    if told_its_scale(chromium) {
        vec![scaled(scale)]
    } else {
        vec![CARD.to_owned()]
    }
}

/// How many messages read from Chrome are held for an answer to be found
/// among. Past this the reader waits, and so does Chrome's writing: a far
/// side telling of events nobody asked for is held back, never stored.
const HELD_MESSAGES: usize = 256;

/// How many bytes the messages read from Chrome and not yet taken may hold
/// together, held to it as [`HELD_MESSAGES`] is ([`crate::chrome_held`],
/// item 653): two of the longest message the pipe reads, so one that long
/// never waits on another that is not.
pub const HELD_BYTES: usize = 2 * MOST_MESSAGE;

/// What stops the requests Chrome makes of its own accord (ADR 0073 § 2),
/// each because a measurement stopped something with it: background
/// networking the variations seed and extension updates, component updates
/// the scheduled ones, and the features AI-mode eligibility, the search
/// engine's preconnect, network time, on-device model fetches and Safe
/// Browsing's real-time keys. Two requests still leave, named by the ADR
/// and by `tests/what_chrome_asks_for_of_its_own.rs`. The features are in
/// [`FEATURES_OFF`].
const OWN_TRAFFIC_OFF: [&str; 2] = [
    "--disable-background-networking",
    "--disable-component-update",
];

/// Every feature of Chrome's switched off, as the one switch it must be:
/// Chrome 156 reads only the last `--disable-features` it is given, so a
/// second would turn the first's back on (measured, item 650).
///
/// All but the last are ADR 0073 § 2's, with [`OWN_TRAFFIC_OFF`]. The last,
/// `AsyncDns`, is Chrome's own DNS client: with it, Chrome asks the
/// machine's nameservers itself, beside whatever else the system's resolver
/// would have done — a VPN's scoped resolvers, the system's own encrypted
/// DNS; without it, Chrome asks every name of `getaddrinfo`, which is the
/// system's resolver (ADR 0008, ADR 0060's fourth amendment). Asserted by
/// `tests/chromes_resolver_is_the_machines.rs`.
const FEATURES_OFF: &str = "--disable-features=AimEnabled,PreconnectToSearch,\
     NetworkTimeServiceQuerying,OptimizationHints,OptGuideOnDeviceModel,\
     SafeBrowsingHashPrefixRealTimeLookups,AsyncDns";

/// The shell's whole script: move the pipes to 3 and 4 — and the error
/// output alo reads to 5, where the Chromium alo carries tells what the
/// pipe does not carry ([`crate::chrome_told`], queue items 799 and 798),
/// its own error output going nowhere as before — then become Chrome.
const STARTER: &str = r#"exec "$0" "$@" 3<&0 4>&1 5>&2 0</dev/null 1>/dev/null 2>/dev/null"#;

/// The most lines of what a Chromium told beside its pipe held unread: a
/// card's thread reads them every few milliseconds while it streams, and
/// one that is not streaming drops what it is not told of in time.
const HELD_TOLD: usize = 256;

/// Chrome, running, with a pipe to it.
pub struct Chrome {
    child: Child,
    to: Option<ChildStdin>,
    arrived: Taker<Result<Message, Failed>>,
    asked: u64,
    patience: Duration,
    /// Removed, and its lock let go, as this value's fields are dropped —
    /// after [`Drop::drop`] has waited for Chrome to end.
    profile: Profile,
    /// Whether it draws in a window of its own on the screen.
    shown: bool,
    /// What its tabs asked for, and what caused each (ADR 0060 § 2).
    record: Record,
    /// What it told beside its pipe, read on a thread of its own (queue
    /// items 799 and 798): only the Chromium alo carries tells anything.
    told: Receiver<Told>,
}

/// Why Chrome was not started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unstarted {
    /// It is not installed where it was looked for.
    NotInstalled(NotInstalled),
    /// The application's own Chromium is not where the application carries
    /// it (ADR 0077).
    NotCarried(NotCarried),
    /// A profile of its own could not be made.
    NoProfile(ErrorKind),
    /// The program could not be started.
    Unspawned(ErrorKind),
}

impl fmt::Display for Unstarted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled(why) => why.fmt(f),
            Self::NotCarried(why) => why.fmt(f),
            Self::NoProfile(kind) => write!(f, "Chrome's profile could not be made: {kind}"),
            Self::Unspawned(kind) => write!(f, "Chrome could not be started: {kind}"),
        }
    }
}

/// Why a command to Chrome was not answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failed {
    /// Chrome has gone: its pipe closed.
    Gone,
    /// Chrome said nothing for as long as it was waited for.
    Silent {
        /// How long that was.
        waited: Duration,
    },
    /// Chrome's pipe could not be read as messages; nothing after this is.
    Unframed(Unframed),
    /// A message from Chrome was not one the protocol describes.
    Malformed(Malformed),
    /// Chrome's answer was longer than [`MOST_MESSAGE`] bytes, and was
    /// skipped rather than read (`chrome_wire`); the pipe goes on.
    TooLong,
    /// Chrome declined the command, and said why.
    Declined(Declined),
    /// The command could not be written.
    Unwritten(ErrorKind),
    /// A target of a tab could not be held, and the tab is not let go on
    /// ([`crate::chrome_hold`]). Boxed, so that every other failure, which
    /// a tree's reading carries down each level it recurses, stays small.
    Unheld(Box<Unheld>),
    /// The tab's page was closed and Chrome detached its session, which
    /// answers nothing sent through it after (item 672).
    Detached,
}

impl fmt::Display for Failed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gone => f.write_str("Chrome has gone"),
            Self::Silent { waited } => {
                write!(f, "Chrome said nothing for {} ms", waited.as_millis())
            }
            Self::Unframed(why) => why.fmt(f),
            Self::Malformed(why) => why.fmt(f),
            Self::TooLong => write!(
                f,
                "Chrome's answer ran past {MOST_MESSAGE} bytes and was not read"
            ),
            Self::Declined(why) => why.fmt(f),
            Self::Unwritten(kind) => write!(f, "Chrome could not be written to: {kind}"),
            Self::Unheld(why) => why.fmt(f),
            Self::Detached => {
                f.write_str("the tab's page was closed, and Chrome answers nothing of it")
            }
        }
    }
}

/// What a Chromium tells on `beside`, its error output moved to 5, read
/// line by line on a thread of its own as [`crate::chrome_told`] reads
/// them: a line past the longest is passed over, one that is not as the
/// host writes it dropped, and at most [`HELD_TOLD`] held unread, newer
/// ones dropped meanwhile. The thread ends with the Chromium.
fn told_beside(beside: ChildStderr) -> Receiver<Told> {
    let (tell, told) = std::sync::mpsc::sync_channel(HELD_TOLD);
    std::thread::spawn(move || {
        let mut reading = std::io::BufReader::new(beside);
        let mut line = Vec::new();
        loop {
            line.clear();
            match (&mut reading)
                .take(u64::try_from(chrome_told::LONGEST_LINE + 2).unwrap_or(u64::MAX))
                .read_until(b'\n', &mut line)
            {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }
            if line.last() != Some(&b'\n') {
                // Past the longest: passed over, to its end.
                if skip_line(&mut reading).is_err() {
                    return;
                }
                continue;
            }
            line.pop();
            let read = std::str::from_utf8(&line).ok().and_then(chrome_told::read);
            if let Some(read) = read
                && let Err(std::sync::mpsc::TrySendError::Disconnected(_)) = tell.try_send(read)
            {
                return;
            }
        }
    });
    told
}

/// Read `reading` to the end of the line it is in.
fn skip_line(reading: &mut impl std::io::BufRead) -> std::io::Result<()> {
    let mut passed = Vec::new();
    loop {
        passed.clear();
        let read = reading.take(4096).read_until(b'\n', &mut passed)?;
        if read == 0 || passed.last() == Some(&b'\n') {
            return Ok(());
        }
    }
}

impl Chrome {
    /// Start the Chrome installed on this machine.
    ///
    /// # Errors
    ///
    /// [`Unstarted::NotInstalled`] when it is not at any place its installer
    /// puts it, or another [`Unstarted`].
    pub fn start() -> Result<Self, Unstarted> {
        Self::start_from(&Chromium::Machines, false)
    }

    /// Start the Chrome installed on this machine shown, with a window of
    /// its own on the screen (ADR 0061).
    ///
    /// # Errors
    ///
    /// As [`Chrome::start`].
    pub fn start_shown() -> Result<Self, Unstarted> {
        Self::start_from(&Chromium::Machines, true)
    }

    /// [`Chrome::start_from`], shown, for a page shown as a card
    /// ([`as_a_card`]): the machine's Chrome with its every window drawn at
    /// `scale` whatever display it is on (`--force-device-scale-factor`,
    /// ADR 0090), its window parked by whichever display the window server
    /// chooses (queue item 738); the Chromium alo carries unseen until the
    /// card is expanded ([`CARD`], queue item 771) and drawn at the scale of
    /// the display it is parked by, which is chosen for `scale` once it is
    /// shown (queue item 772). With `pinned`, it takes that name as the
    /// address it holds and never resolves it (queue item 881,
    /// [`crate::chrome_pin`]).
    ///
    /// # Errors
    ///
    /// As [`Chrome::start_from`].
    pub fn start_shown_at(
        chromium: &Chromium,
        scale: Scale,
        pinned: Option<&Pinned>,
    ) -> Result<Self, Unstarted> {
        let mut added = as_a_card(chromium, scale);
        added.extend(pinned.map(Pinned::switch));
        Self::launch(&Self::program(chromium)?, true, &added)
    }

    /// Start the Chromium `chromium` names, headless or `shown`: the
    /// machine's Chrome where its installer puts it, or the application's
    /// own at the one place it carries it — never anywhere else (ADR 0077).
    ///
    /// # Errors
    ///
    /// [`Unstarted::NotInstalled`] when the machine's Chrome is not at any
    /// place its installer puts it, [`Unstarted::NotCarried`] when the
    /// application's own is not a program at its place, or another
    /// [`Unstarted`].
    pub fn start_from(chromium: &Chromium, shown: bool) -> Result<Self, Unstarted> {
        Self::launch(&Self::program(chromium)?, shown, &[])
    }

    /// The program `chromium` names: the machine's Chrome where its
    /// installer puts it, or the application's own at its one place.
    fn program(chromium: &Chromium) -> Result<PathBuf, Unstarted> {
        match chromium {
            Chromium::Machines => chrome_on_this_machine().map_err(Unstarted::NotInstalled),
            Chromium::Carried(program) if is_program(program) => Ok(program.clone()),
            Chromium::Carried(program) => Err(Unstarted::NotCarried(NotCarried {
                looked: program.clone(),
            })),
        }
    }

    /// Start the Chrome at `program`.
    ///
    /// # Errors
    ///
    /// [`Unstarted::NotInstalled`] when there is no program there, so that a
    /// Chrome that is not there is never mistaken for one that would not
    /// answer; or another [`Unstarted`].
    pub fn start_at(program: &Path) -> Result<Self, Unstarted> {
        Self::launch(program, false, &[])
    }

    /// Start the Chrome at `program`, headless or `shown`, with the switches
    /// `added` after every Chromium's ([`as_a_card`]'s for a card).
    fn launch(program: &Path, shown: bool, added: &[String]) -> Result<Self, Unstarted> {
        if !is_program(program) {
            return Err(Unstarted::NotInstalled(NotInstalled {
                looked: vec![program.to_path_buf()],
            }));
        }
        let profile = new_profile().map_err(|why| Unstarted::NoProfile(why.kind()))?;
        let mut user_data = std::ffi::OsString::from("--user-data-dir=");
        user_data.push(profile.path());
        let spawned = Process::new("/bin/sh")
            .arg("-c")
            .arg(STARTER)
            .arg(program)
            .arg("--remote-debugging-pipe")
            .arg(user_data)
            .args(["--no-first-run", "--no-default-browser-check"])
            .args(OWN_TRAFFIC_OFF)
            .arg(FEATURES_OFF)
            // Measured against Chrome 155: `--app=about:blank` is passed
            // over, and a window with a tab strip opens on the new-tab page;
            // `data:,` is taken as the application, and is empty.
            .args(if shown {
                &["--app=data:,"][..]
            } else {
                &["--headless", "about:blank"][..]
            })
            .args(added)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(why) => {
                return Err(Unstarted::Unspawned(why.kind()));
            }
        };
        let (to, from, beside) = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        let (Some(to), Some(from), Some(beside)) = (to, from, beside) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Unstarted::Unspawned(ErrorKind::BrokenPipe));
        };
        let (tell, arrived) = held(HELD_MESSAGES, HELD_BYTES);
        // Its own thread, so that waiting for Chrome is a wait with a bound
        // rather than a read that may never return. It ends when the pipe
        // does, or at the first thing that cannot be read as messages.
        std::thread::spawn(move || {
            let mut frames = Frames::from(from);
            loop {
                let next = match frames.next_message() {
                    Ok(Some(Frame::Whole(message))) => {
                        chrome_protocol::read(&message).map_err(Failed::Malformed)
                    }
                    Ok(Some(Frame::Skipped { head })) => Ok(chrome_protocol::skipped(&head)),
                    Ok(None) => return,
                    Err(why) => Err(Failed::Unframed(why)),
                };
                let ended = matches!(next, Err(Failed::Unframed(_)));
                let bytes = weight(&next);
                if tell.tell(next, bytes).is_err() || ended {
                    return;
                }
            }
        });
        Ok(Self {
            child,
            to: Some(to),
            arrived,
            asked: 0,
            patience: LONGEST_SILENCE,
            profile,
            shown,
            record: Record::default(),
            told: told_beside(beside),
        })
    }

    /// What it told beside its pipe since this was last asked, oldest
    /// first (queue items 799 and 798).
    pub fn take_told(&self) -> Vec<Told> {
        self.told.try_iter().collect()
    }

    /// Wait at most `patience` for an answer, and for Chrome to close.
    #[must_use]
    pub fn waiting_at_most(mut self, patience: Duration) -> Self {
        // A day is longer than anybody waits, and no deadline past it can
        // overflow the clock.
        self.patience = patience.min(Duration::from_secs(24 * 60 * 60));
        self
    }

    /// Chrome's process id, for a test that wants to see it gone — and the
    /// owner the window server names a shown Chrome's window by.
    pub fn process(&self) -> u32 {
        self.child.id()
    }

    /// The profile Chrome was started on, for a test that wants to see it
    /// removed.
    pub fn profile(&self) -> &Path {
        self.profile.path()
    }

    /// What starting it removed of the profiles killed browsers left, and
    /// what it could not (ADR 0069).
    pub fn swept(&self) -> &Swept {
        self.profile.swept()
    }

    /// Whether it draws in a window of its own on the screen
    /// ([`Chrome::start_shown`]) rather than headless.
    pub fn is_shown(&self) -> bool {
        self.shown
    }

    /// What the messages read from Chrome and not yet taken hold, in
    /// bytes: never more than [`HELD_BYTES`].
    pub fn held_bytes(&self) -> usize {
        self.arrived.bytes()
    }

    /// How long it is waited for.
    pub fn patience(&self) -> Duration {
        self.patience
    }

    /// How many commands have been written to Chrome so far, for a test that
    /// wants to see that something was refused before Chrome heard of it.
    pub fn written(&self) -> u64 {
        self.asked
    }

    /// What its tabs from the network asked for, each request a line with
    /// the cause this process gave it ([`crate::chrome_record`]).
    pub fn record(&self) -> &Record {
        &self.record
    }

    /// The record, to write in.
    pub(crate) fn record_mut(&mut self) -> &mut Record {
        &mut self.record
    }

    /// Which Chrome this is, as it says itself.
    ///
    /// # Errors
    ///
    /// [`Failed`], when it does not answer, or answers with something that is
    /// not a version.
    pub fn version(&mut self) -> Result<Version, Failed> {
        let result = self.ask(None, &Command::GetVersion)?;
        chrome_protocol::version(&result).map_err(Failed::Malformed)
    }

    /// Send `command`, through `session` when it is for a tab, and wait for
    /// its result — passing over every event told of meanwhile, which is
    /// what a command to Chrome itself wants. A tab, which has events to
    /// answer, waits with [`Chrome::send`] and [`Chrome::receive`] instead.
    ///
    /// # Errors
    ///
    /// [`Failed`], when it is not written, not answered within the patience,
    /// or declined.
    pub fn ask(&mut self, session: Option<&str>, command: &Command) -> Result<Vec<u8>, Failed> {
        let id = self.send(session, command)?;
        let until = Instant::now() + self.patience;
        loop {
            match self.receive(until)? {
                Message::Reply {
                    id: answered,
                    result,
                    ..
                } if answered == id => return result.map_err(Failed::Declined),
                Message::Skipped {
                    id: Some(answered), ..
                } if answered == id => return Err(Failed::TooLong),
                // An event, or the answer to a command given up on before it
                // came.
                _ => {}
            }
        }
    }

    /// Write `command`, through `session` when it is for a tab, without
    /// waiting: the id its answer will carry.
    ///
    /// # Errors
    ///
    /// [`Failed::Gone`] when nobody reads the far end, or
    /// [`Failed::Unwritten`].
    pub fn send(&mut self, session: Option<&str>, command: &Command) -> Result<u64, Failed> {
        self.asked = self.asked.wrapping_add(1);
        let id = self.asked;
        let to = self.to.as_mut().ok_or(Failed::Gone)?;
        chrome_wire::write(to, &chrome_protocol::written(id, session, command)).map_err(|why| {
            match why.kind() {
                // Nobody reads the far end: Chrome has gone.
                ErrorKind::BrokenPipe => Failed::Gone,
                kind => Failed::Unwritten(kind),
            }
        })?;
        Ok(id)
    }

    /// The next message from Chrome, waiting no later than `until`.
    ///
    /// # Errors
    ///
    /// [`Failed::Silent`] when nothing came by then, [`Failed::Gone`] when the
    /// pipe has closed, or what made its messages unreadable.
    pub fn receive(&mut self, until: Instant) -> Result<Message, Failed> {
        match self.arrived.take(until) {
            Ok(arrived) => arrived,
            Err(Untaken::Waited) => Err(Failed::Silent {
                waited: self.patience,
            }),
            Err(Untaken::Ended) => Err(Failed::Gone),
        }
    }
}

/// What a message read from Chrome holds while it waits to be taken, in
/// bytes: its own size and every text and buffer it owns. Its `result` or
/// `params` is the JSON written again, which is what is held, rather than
/// what the pipe carried.
fn weight(next: &Result<Message, Failed>) -> usize {
    let owned = match next {
        Ok(Message::Reply {
            session, result, ..
        }) => {
            session.as_ref().map_or(0, String::len)
                + match result {
                    Ok(result) => result.len(),
                    Err(declined) => declined.message.len(),
                }
        }
        Ok(Message::Event {
            method,
            session,
            params,
        }) => method.len() + session.as_ref().map_or(0, String::len) + params.len(),
        Ok(Message::Skipped { method, .. }) => method.as_ref().map_or(0, String::len),
        Err(_) => 0,
    };
    owned.saturating_add(core::mem::size_of::<Result<Message, Failed>>())
}

impl Chrome {
    /// Close Chrome as dropping it does — `Browser.close`, then its pipe
    /// closed — and wait for it to end, as long as it is given to answer:
    /// how it ended, or [`None`] when it had not and was killed. For a test
    /// that wants to see it end in order rather than crash (queue item
    /// 734); its profile is removed as it is dropped.
    pub fn close(mut self) -> Option<std::process::ExitStatus> {
        self.ended()
    }

    /// Ask Chrome to close, close its pipe and wait for it as long as it is
    /// given to answer: how it ended by itself, if it did.
    fn ended(&mut self) -> Option<std::process::ExitStatus> {
        if let Some(mut to) = self.to.take() {
            let _ =
                chrome_wire::write(&mut to, &chrome_protocol::written(0, None, &Command::Close));
            // Dropped here, closing the pipe: a Chrome that did not read the
            // command ends at its pipe closing.
        }
        let until = Instant::now() + self.patience;
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) if Instant::now() < until => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                _ => return None,
            }
        }
    }
}

impl Drop for Chrome {
    fn drop(&mut self) {
        if self.ended().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        // The profile is removed as the fields are dropped, after this.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scale_is_told_as_chromium_reads_it() {
        assert_eq!(scaled(Scale::ONE), "--force-device-scale-factor=1");
        assert_eq!(scaled(Scale::TWO), "--force-device-scale-factor=2");
        assert_eq!(
            scaled(Scale::ONE_AND_A_HALF),
            "--force-device-scale-factor=1.5"
        );
        assert_eq!(
            scaled(Scale::of_screen(3.0)),
            "--force-device-scale-factor=3"
        );
    }

    #[test]
    fn only_the_machines_chrome_is_told_its_scale_and_only_alos_own_that_it_is_a_card() {
        let carried = Chromium::Carried(PathBuf::from(
            "/A/alo Chromium.app/Contents/MacOS/alo Chromium",
        ));
        assert!(told_its_scale(&Chromium::Machines));
        assert!(!told_its_scale(&carried));
        assert_eq!(
            as_a_card(&Chromium::Machines, Scale::TWO),
            ["--force-device-scale-factor=2"]
        );
        assert_eq!(
            as_a_card(&Chromium::Machines, Scale::ONE),
            ["--force-device-scale-factor=1"]
        );
        // Told, it would keep the scale for its life (queue item 772).
        assert_eq!(as_a_card(&carried, Scale::ONE), ["--alo-card"]);
        assert_eq!(as_a_card(&carried, Scale::TWO), ["--alo-card"]);
    }
}
