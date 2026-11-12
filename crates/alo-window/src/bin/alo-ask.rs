/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! `alo-ask`: ask a running browser what an agent would, a line at a time
//! (ADR 0055).
//!
//! `alo-ask [SOCKET]` sends each line it reads to the browser's agent socket
//! — the one under the person's runtime directory if none is named — and
//! prints each answer on a line of its own. It reads nothing of what it sends
//! or receives: the protocol is `docs/contracts/agent-socket.md`, and this is
//! the smallest caller of it, for a person at a terminal and for the test
//! that drives the browser from a process of its own.
//!
//! `{"v":1,"ask":"renderer","tab":ADDRESS}` is the line a person sends to
//! ask the running browser which renderer draws a tab (queue item 622).
//!
//! `alo-ask --plain [SOCKET]` prints each answer for a person instead
//! (queue item 915, [`alo_window::socket_plain`]): a tree as one node a
//! line, indented, each its number, its role, its name and its states, and
//! no position; a verb's outcome and a refusal as the browser's sentence;
//! a page's text (`{"v":1,"ask":"page_text","tab":ADDRESS}`, queue item
//! 964) as its words, a line of the page to a line. What it sends is the
//! same.
//!
//! A blank line is sent and not waited on, as the browser answers none. It
//! stops when its input ends or the browser closes the connection.

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;
    use std::process::ExitCode;

    let mut named = std::env::args_os().skip(1).peekable();
    let plain = named.next_if(|first| first == "--plain").is_some();
    let socket = if let Some(named) = named.next() {
        PathBuf::from(named)
    } else {
        let Some(runtime) = alo_window::socket_place::runtime_directory() else {
            eprintln!("alo-ask: name the socket: neither XDG_RUNTIME_DIR nor TMPDIR is set");
            return ExitCode::from(2);
        };
        alo_window::socket_place::socket_under(&runtime)
    };
    let stream = match UnixStream::connect(&socket) {
        Ok(stream) => stream,
        Err(why) => {
            eprintln!("alo-ask: no browser answers at {}: {why}", socket.display());
            return ExitCode::from(2);
        }
    };
    let Ok(mut sending) = stream.try_clone() else {
        eprintln!("alo-ask: the connection could not be used both ways");
        return ExitCode::FAILURE;
    };
    let mut answers = BufReader::new(stream);
    let mut printed = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else {
            return ExitCode::FAILURE;
        };
        if sending.write_all(format!("{line}\n").as_bytes()).is_err() {
            // **The answer may already be here.** A refusal that ends the
            // conversation is answered and then the connection is closed —
            // `LineTooLong` is one: the browser takes what it will of the
            // line, says so, and goes, while this process is still writing
            // the rest of it and is given a broken pipe for the remainder.
            // Leaving then threw away a refusal that had already arrived,
            // which is how `every_refusal_crosses_as_the_same_kind_through_
            // chrome` lost its last refusal and stopped the build loop on
            // 2026-10-25. A failed write says nothing about what was read.
            let mut answer = String::new();
            if answers.read_line(&mut answer).is_ok_and(|read| read > 0) {
                let _ = printed.write_all(shown(&answer, plain).as_bytes());
                let _ = printed.flush();
            }
            return ExitCode::SUCCESS;
        }
        if line.trim().is_empty() {
            continue;
        }
        let mut answer = String::new();
        match answers.read_line(&mut answer) {
            Ok(0) | Err(_) => return ExitCode::SUCCESS,
            Ok(_) => {}
        }
        if printed.write_all(shown(&answer, plain).as_bytes()).is_err() || printed.flush().is_err()
        {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

/// `answer` as it is printed: as it came, or written for a person.
#[cfg(unix)]
fn shown(answer: &str, plain: bool) -> std::borrow::Cow<'_, str> {
    if plain {
        std::borrow::Cow::Owned(alo_window::socket_plain::written(answer))
    } else {
        std::borrow::Cow::Borrowed(answer)
    }
}

#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    eprintln!("alo-ask: on Windows how an agent reaches this browser is undecided (ADR 0055 § 1)");
    std::process::ExitCode::from(2)
}
