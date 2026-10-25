/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An answer written just before the far side closed is not thrown away by
//! `alo-ask` (ADR 0055; found by queue item 648).
//!
//! The browser answers some refusals and then goes: `LineTooLong` takes what
//! it will of an over-long line, says so, and closes, because it cannot find
//! where the next line starts. The caller is still writing the rest of that
//! line and is given a broken pipe for it — **while the refusal is already in
//! its own buffer.** Leaving on the failed write loses it, which is what
//! happened on 2026-10-25: the gate's
//! `every_refusal_crosses_as_the_same_kind_through_chrome` saw seven refusals
//! where eight were sent, and the build loop stopped.
//!
//! A far side of this test's own plays that exactly, with no browser and no
//! Chrome: it answers, it closes, and the line sent to it is far larger than
//! any socket will take at once, so the broken pipe is certain rather than
//! lucky.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process::{Command, Stdio};

/// Bigger than any socket buffer, so the write cannot finish before the far
/// side has gone.
const FAR_TOO_LONG: usize = 4 * 1024 * 1024;

#[test]
fn an_answer_written_as_the_far_side_closed_is_still_printed() {
    let place = std::env::temp_dir().join(format!("alo-ask-going-{}", std::process::id()));
    let _ = std::fs::remove_file(&place);
    let listening = UnixListener::bind(&place).expect("the far side could not listen");

    let answer = r#"{"answer":"refused","refusal":{"kind":"LineTooLong","most":65536},"v":1}"#;
    let far_side = std::thread::spawn(move || {
        let (mut stream, _) = listening.accept().expect("nobody connected");
        // Read a little — as the browser reads what it will take — then
        // answer and go, with the caller still writing.
        let mut taken = [0u8; 4096];
        let _ = stream.read(&mut taken);
        let _ = stream.write_all(format!("{answer}\n").as_bytes());
        let _ = stream.flush();
        drop(stream);
    });

    let mut asking = Command::new(env!("CARGO_BIN_EXE_alo-ask"))
        .arg(&place)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("alo-ask did not start");
    let mut sending = asking.stdin.take().expect("alo-ask has no input");
    // The write itself is expected to fail partway, which is the case under
    // test; what matters is what alo-ask does next.
    let _ = sending
        .write_all(format!("{{\"v\":1,\"a\":\"{}\"}}\n", "a".repeat(FAR_TOO_LONG)).as_bytes());
    drop(sending);

    let finished = asking.wait_with_output().expect("alo-ask did not finish");
    far_side.join().expect("the far side panicked");
    let _ = std::fs::remove_file(&place);

    let said: Vec<String> = BufReader::new(finished.stdout.as_slice())
        .lines()
        .map_while(Result::ok)
        .collect();
    assert_eq!(
        said,
        vec![answer.to_owned()],
        "the answer was written before the far side closed, so it is the caller's to print; \
         stderr: {}",
        String::from_utf8_lossy(&finished.stderr)
    );
}

/// And the ordinary case is unchanged: a far side that goes without saying
/// anything leaves nothing to print.
#[test]
fn a_far_side_that_goes_without_answering_leaves_nothing() {
    let place = std::env::temp_dir().join(format!("alo-ask-silent-{}", std::process::id()));
    let _ = std::fs::remove_file(&place);
    let listening = UnixListener::bind(&place).expect("the far side could not listen");
    let far_side = std::thread::spawn(move || {
        let (stream, _) = listening.accept().expect("nobody connected");
        drop(stream);
    });

    let mut asking = Command::new(env!("CARGO_BIN_EXE_alo-ask"))
        .arg(&place)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("alo-ask did not start");
    let mut sending = asking.stdin.take().expect("alo-ask has no input");
    let _ = sending
        .write_all(format!("{{\"v\":1,\"a\":\"{}\"}}\n", "a".repeat(FAR_TOO_LONG)).as_bytes());
    drop(sending);
    let finished = asking.wait_with_output().expect("alo-ask did not finish");
    far_side.join().expect("the far side panicked");
    let _ = std::fs::remove_file(&place);
    assert!(
        finished.stdout.is_empty(),
        "nothing was said, so nothing is printed: {:?}",
        String::from_utf8_lossy(&finished.stdout)
    );
    let _ = UnixStream::connect(&place);
}
