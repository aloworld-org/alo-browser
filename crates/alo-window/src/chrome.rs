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
//! framed by [`crate::chrome_wire`] and read by [`crate::chrome_protocol`].
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
//! Each Chrome runs headless on a new, private, empty profile, never the
//! person's — their cookies and sessions are not this browser's to hand a
//! renderer, and Chrome refuses to be driven on its default profile anyway.
//! When this handle is dropped Chrome is asked to close, then killed if it has
//! not, then waited for, and its profile is removed. When the browser process
//! ends without dropping it — killed, or crashed — the pipe closes with it,
//! and Chrome ends when its pipe closes: both are asserted by
//! `tests/chrome_over_a_pipe.rs`.
//!
//! # What it is not yet
//!
//! It answers `Browser.getVersion`, holds tabs ([`crate::chrome_tab`], items
//! 620 and 633) and closes; the verbs are carried out through it
//! ([`crate::chrome_act`], item 621).

use crate::beside::{NotInstalled, chrome_in, chrome_places, is_program};
use crate::chrome_hold::Unheld;
use crate::chrome_protocol::{self, Command, Declined, Malformed, Message, Version};
use crate::chrome_wire::{self, Frames, Unframed};
use core::fmt;
use std::io::ErrorKind;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command as Process, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, sync_channel};
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

/// The shell's whole script: move the pipes to 3 and 4, then become Chrome.
const STARTER: &str = r#"exec "$0" "$@" 3<&0 4>&1 0</dev/null 1>/dev/null"#;

/// Chrome, running, with a pipe to it.
pub struct Chrome {
    child: Child,
    to: Option<ChildStdin>,
    arrived: Receiver<Result<Message, Failed>>,
    asked: u64,
    patience: Duration,
    profile: PathBuf,
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
    /// Chrome declined the command, and said why.
    Declined(Declined),
    /// The command could not be written.
    Unwritten(ErrorKind),
    /// A target of a tab could not be held, and the tab is not let go on
    /// ([`crate::chrome_hold`]). Boxed, so that every other failure, which
    /// a tree's reading carries down each level it recurses, stays small.
    Unheld(Box<Unheld>),
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
            Self::Declined(why) => why.fmt(f),
            Self::Unwritten(kind) => write!(f, "Chrome could not be written to: {kind}"),
            Self::Unheld(why) => why.fmt(f),
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
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let program =
            chrome_in(&chrome_places(home.as_deref())).map_err(Unstarted::NotInstalled)?;
        Self::start_at(&program)
    }

    /// Start the Chrome at `program`.
    ///
    /// # Errors
    ///
    /// [`Unstarted::NotInstalled`] when there is no program there, so that a
    /// Chrome that is not there is never mistaken for one that would not
    /// answer; or another [`Unstarted`].
    pub fn start_at(program: &Path) -> Result<Self, Unstarted> {
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
            .args(["--headless", "--remote-debugging-pipe"])
            .arg(user_data)
            .args([
                "--no-first-run",
                "--no-default-browser-check",
                "about:blank",
            ])
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
        let (tell, arrived) = sync_channel(HELD_MESSAGES);
        // Its own thread, so that waiting for Chrome is a wait with a bound
        // rather than a read that may never return. It ends when the pipe
        // does, or at the first thing that cannot be read as messages.
        std::thread::spawn(move || {
            let mut frames = Frames::from(from);
            loop {
                let next = match frames.next_message() {
                    Ok(Some(message)) => chrome_protocol::read(&message).map_err(Failed::Malformed),
                    Ok(None) => return,
                    Err(why) => Err(Failed::Unframed(why)),
                };
                let ended = matches!(next, Err(Failed::Unframed(_)));
                if tell.send(next).is_err() || ended {
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

    /// Chrome's process id, for a test that wants to see it gone.
    pub fn process(&self) -> u32 {
        self.child.id()
    }

    /// The profile Chrome was started on, for a test that wants to see it
    /// removed.
    pub fn profile(&self) -> &Path {
        &self.profile
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
        let left = until.saturating_duration_since(Instant::now());
        match self.arrived.recv_timeout(left) {
            Ok(arrived) => arrived,
            Err(RecvTimeoutError::Timeout) => Err(Failed::Silent {
                waited: self.patience,
            }),
            Err(RecvTimeoutError::Disconnected) => Err(Failed::Gone),
        }
    }
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

/// A new, empty directory only this user can read, for one Chrome's profile.
///
/// Made rather than reused: a directory that is already there may be one
/// somebody else put there for Chrome to read.
fn new_profile() -> std::io::Result<PathBuf> {
    static MADE: AtomicU64 = AtomicU64::new(0);
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let path = std::env::temp_dir().join(format!(
        "alo-chrome-{}-{}-{since}",
        std::process::id(),
        MADE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::DirBuilder::new().mode(0o700).create(&path)?;
    Ok(path)
}
