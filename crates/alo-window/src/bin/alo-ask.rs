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
//! A blank line is sent and not waited on, as the browser answers none. It
//! stops when its input ends or the browser closes the connection.

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;
    use std::process::ExitCode;

    let socket = if let Some(named) = std::env::args_os().nth(1) {
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
        if printed.write_all(answer.as_bytes()).is_err() || printed.flush().is_err() {
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    eprintln!("alo-ask: on Windows how an agent reaches this browser is undecided (ADR 0055 § 1)");
    std::process::ExitCode::from(2)
}
