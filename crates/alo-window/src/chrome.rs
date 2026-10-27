/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Chrome, started and spoken to over a pipe (ADR 0058 § 3, queue item 619).
//!
//! The browser process starts the Chrome already installed on the machine
//! ([`crate::beside::chrome_places`]) and speaks its DevTools protocol over
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
//! Chrome on the machine's resolver (ADR 0008, item 635).
//! When this handle is dropped Chrome is asked to close, then killed if it has
//! not, then waited for, and its profile is removed. When the browser process
//! ends without dropping it — killed, or crashed — the pipe closes with it,
//! and Chrome ends when its pipe closes: both are asserted by
//! `tests/chrome_over_a_pipe.rs`.
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

use crate::beside::{NotInstalled, chrome_on_this_machine, is_program};
use crate::chrome_held::{Taker, Untaken, held};
use crate::chrome_hold::Unheld;
use crate::chrome_profile::new_profile;
use crate::chrome_protocol::{self, Command, Declined, Malformed, Message, Version};
use crate::chrome_record::Record;
use crate::chrome_wire::{self, Frame, Frames, MOST_MESSAGE, Unframed};
use core::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command as Process, Stdio};
use std::time::{Duration, Instant};

/// How long Chrome is waited for, to answer and to close, unless a caller
/// asks for another bound. Chrome 155 answers its first command in under a
/// second on this machine; a cold start of a large program on a loaded one
/// is given thirty.
pub const LONGEST_SILENCE: Duration = Duration::from_secs(30);

/// How many messages read from Chrome are held for an answer to be found
/// among. Past this the reader waits, and so does Chrome's writing: a far
/// side telling of events nobody asked for is held back, never stored.
const HELD_MESSAGES: usize = 256;

/// How many bytes the messages read from Chrome and not yet taken may hold
/// together, held to it as [`HELD_MESSAGES`] is ([`crate::chrome_held`],
/// item 653): two of the longest message the pipe reads, so one that long
/// never waits on another that is not.
pub const HELD_BYTES: usize = 2 * MOST_MESSAGE;

/// The shell's whole script: move the pipes to 3 and 4, then become Chrome.
const STARTER: &str = r#"exec "$0" "$@" 3<&0 4>&1 0</dev/null 1>/dev/null"#;

/// Chrome, running, with a pipe to it.
pub struct Chrome {
    child: Child,
    to: Option<ChildStdin>,
    arrived: Taker<Result<Message, Failed>>,
    asked: u64,
    patience: Duration,
    profile: PathBuf,
    /// Whether it draws in a window of its own on the screen.
    shown: bool,
    /// What its tabs asked for, and what caused each (ADR 0060 § 2).
    record: Record,
}

/// Why Chrome was not started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unstarted {
    /// It is not installed where it was looked for.
    NotInstalled(NotInstalled),
    /// A profile of its own could not be made.
    NoProfile(ErrorKind),
    /// The program could not be started.
    Unspawned(ErrorKind),
}

impl fmt::Display for Unstarted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInstalled(why) => why.fmt(f),
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

impl Chrome {
    /// Start the Chrome installed on this machine.
    ///
    /// # Errors
    ///
    /// [`Unstarted::NotInstalled`] when it is not at any place its installer
    /// puts it, or another [`Unstarted`].
    pub fn start() -> Result<Self, Unstarted> {
        let program = chrome_on_this_machine().map_err(Unstarted::NotInstalled)?;
        Self::start_at(&program)
    }

    /// Start the Chrome installed on this machine shown, with a window of
    /// its own on the screen (ADR 0061).
    ///
    /// # Errors
    ///
    /// As [`Chrome::start`].
    pub fn start_shown() -> Result<Self, Unstarted> {
        let program = chrome_on_this_machine().map_err(Unstarted::NotInstalled)?;
        Self::launch(&program, true)
    }

    /// Start the Chrome at `program`.
    ///
    /// # Errors
    ///
    /// [`Unstarted::NotInstalled`] when there is no program there, so that a
    /// Chrome that is not there is never mistaken for one that would not
    /// answer; or another [`Unstarted`].
    pub fn start_at(program: &Path) -> Result<Self, Unstarted> {
        Self::launch(program, false)
    }

    /// Start the Chrome at `program`, headless or `shown`.
    fn launch(program: &Path, shown: bool) -> Result<Self, Unstarted> {
        if !is_program(program) {
            return Err(Unstarted::NotInstalled(NotInstalled {
                looked: vec![program.to_path_buf()],
            }));
        }
        let profile = new_profile().map_err(|why| Unstarted::NoProfile(why.kind()))?;
        let mut user_data = std::ffi::OsString::from("--user-data-dir=");
        user_data.push(&profile);
        let spawned = Process::new("/bin/sh")
            .arg("-c")
            .arg(STARTER)
            .arg(program)
            .arg("--remote-debugging-pipe")
            .arg(user_data)
            .args(["--no-first-run", "--no-default-browser-check"])
            // Measured against Chrome 155: `--app=about:blank` is passed
            // over, and a window with a tab strip opens on the new-tab page;
            // `data:,` is taken as the application, and is empty.
            .args(if shown {
                &["--app=data:,"][..]
            } else {
                &["--headless", "about:blank"][..]
            })
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(why) => {
                let _ = std::fs::remove_dir_all(&profile);
                return Err(Unstarted::Unspawned(why.kind()));
            }
        };
        let (to, from) = (child.stdin.take(), child.stdout.take());
        let (Some(to), Some(from)) = (to, from) else {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_dir_all(&profile);
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
        })
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
        &self.profile
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

impl Drop for Chrome {
    fn drop(&mut self) {
        if let Some(mut to) = self.to.take() {
            let _ =
                chrome_wire::write(&mut to, &chrome_protocol::written(0, None, &Command::Close));
            // Dropped here, closing the pipe: a Chrome that did not read the
            // command ends at its pipe closing.
        }
        let until = Instant::now() + self.patience;
        while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}
