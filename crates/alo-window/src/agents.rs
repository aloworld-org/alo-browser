/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The socket an agent outside this browser speaks to (ADR 0055).
//!
//! A Unix domain socket the browser process holds, and **never a port**: a
//! page's own script can reach `localhost`, and a port would let any page
//! drive the browser showing it with no grant consulted. Where it is, and why
//! only its person can open it, is [`crate::socket_place`]'s.
//!
//! # Who does what
//!
//! A thread accepts connections, at most [`MOST_CALLERS`] at once, and each
//! connection has a thread that reads its lines — bounded at [`MOST_LINE`]
//! before anything parses them — and writes its answers. Neither touches a
//! tab: a request goes to the conductor as an [`Order::Agent`], and the
//! conductor, which holds the tabs, answers it and records it
//! ([`crate::serving`]). So a verb reaches a renderer over the boundary that
//! already carries one, and the thing reachable from outside holds no page.
//!
//! # One at a time
//!
//! A connection's requests are answered in the order sent, one before the
//! next is read: a caller has at most one request in flight, so how many are
//! in flight is bounded by how many callers there are. A blank line is no
//! request and is not answered.

use crate::agent_record::Caller;
use crate::asking::{Answer, MOST_CALLERS, MOST_LINE, Refused};
use crate::inbox::Orders;
use crate::message::Order;
use crate::protocol;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

/// The connections open, so that stopping can close them.
type Open = Arc<Mutex<Vec<(Caller, UnixStream)>>>;

/// A socket being listened on.
#[derive(Debug)]
pub struct Listening {
    path: PathBuf,
    stopping: Arc<AtomicBool>,
    accepting: JoinHandle<()>,
}

impl Listening {
    /// Listen at `path`, sending what agents ask to the conductor at
    /// `orders`. The socket is made so that only its owner can open it.
    ///
    /// # Errors
    ///
    /// What the operating system said when it would not make the socket or
    /// start its thread. Nothing is left at `path` when it would not.
    pub fn at(path: &Path, orders: Orders) -> std::io::Result<Self> {
        let listener = UnixListener::bind(path)?;
        let started = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .and_then(|()| {
                let stopping = Arc::new(AtomicBool::new(false));
                let flag = Arc::clone(&stopping);
                std::thread::Builder::new()
                    .name("alo-agents".to_owned())
                    .spawn(move || accept(&listener, &orders, &flag))
                    .map(|accepting| (stopping, accepting))
            });
        match started {
            Ok((stopping, accepting)) => Ok(Self {
                path: path.to_path_buf(),
                stopping,
                accepting,
            }),
            Err(why) => {
                let _ = std::fs::remove_file(path);
                Err(why)
            }
        }
    }

    /// Where it is.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Stop listening, close every connection, and remove the socket.
    ///
    /// A connection waiting on the conductor for an answer is waited for:
    /// the conductor answers every request it is sent, or goes.
    pub fn stop(self) {
        self.stopping.store(true, Ordering::SeqCst);
        // Wake the accepting thread, which waits on nothing else.
        let _ = UnixStream::connect(&self.path);
        let _ = self.accepting.join();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Accept connections until told to stop, then close every one still open
/// and wait for its thread.
fn accept(listener: &UnixListener, orders: &Orders, stopping: &AtomicBool) {
    let open: Open = Arc::new(Mutex::new(Vec::new()));
    let mut serving: Vec<JoinHandle<()>> = Vec::new();
    let mut next = 0u64;
    for stream in listener.incoming() {
        if stopping.load(Ordering::SeqCst) {
            break;
        }
        let Ok(mut stream) = stream else {
            continue;
        };
        let caller = Caller(next);
        next = next.saturating_add(1);
        serving.retain(|thread| !thread.is_finished());
        if serving.len() >= MOST_CALLERS {
            let refused = Answer::Refused(Refused::TooManyCallers { most: MOST_CALLERS });
            let _ = stream.write_all(protocol::write(None, &refused).as_bytes());
            continue;
        }
        let Ok(kept) = stream.try_clone() else {
            continue;
        };
        lock(&open).push((caller, kept));
        let (orders, still_open) = (orders.clone(), Arc::clone(&open));
        let started = std::thread::Builder::new()
            .name(format!("alo-{caller}"))
            .spawn(move || {
                converse(stream, caller, &orders);
                lock(&still_open).retain(|(open, _)| *open != caller);
            });
        match started {
            Ok(thread) => serving.push(thread),
            Err(_) => lock(&open).retain(|(open, _)| *open != caller),
        }
    }
    for (_, stream) in lock(&open).iter() {
        let _ = stream.shutdown(std::net::Shutdown::Both);
    }
    for thread in serving {
        let _ = thread.join();
    }
}

/// The list of open connections, whether or not a thread panicked holding
/// it — which the lints make unreachable, and which would leave the list as
/// it was.
fn lock(open: &Open) -> std::sync::MutexGuard<'_, Vec<(Caller, UnixStream)>> {
    open.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Read one connection's requests and write its answers, until it closes,
/// the conductor goes, or a line is too long to find the next.
fn converse(stream: UnixStream, caller: Caller, orders: &Orders) {
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    let most = u64::try_from(MOST_LINE).unwrap_or(u64::MAX);
    loop {
        let mut line = Vec::new();
        match reader.by_ref().take(most).read_until(b'\n', &mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let ended = line.last() == Some(&b'\n');
        let (echo, ask, closing) = if !ended && line.len() >= MOST_LINE {
            let refused = Refused::LineTooLong { most: MOST_LINE };
            (None, Err(refused), true)
        } else {
            if ended {
                line.pop();
            }
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let request = protocol::read(&line);
            (request.echo, request.ask, false)
        };
        let (reply, answered) = channel();
        if orders.send(Order::Agent { caller, ask, reply }).is_err() {
            return;
        }
        let Ok(answer) = answered.recv() else {
            return;
        };
        let written = protocol::write(echo.as_ref(), &answer);
        if writer.write_all(written.as_bytes()).is_err() || closing {
            return;
        }
    }
}
