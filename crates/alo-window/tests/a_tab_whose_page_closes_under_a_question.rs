/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A tab whose page closes while it is asking something of it says so at
//! once, rather than waiting out Chrome's patience (queue item 672).
//!
//! When a page closes — a person closing Chrome's window, or this process —
//! Chrome detaches the tab's session and answers nothing sent through it
//! after. Measured against Chrome 155 under 96 busy processes, a reading of
//! the window sent as its page closed was never answered: one run in four
//! of `a_frame_beside_chromes_window.rs` waited Chrome's thirty seconds and
//! failed the gate with *"Chrome said nothing for 30000 ms"*, where without
//! load the same reading was declined.
//!
//! So the order is made, not waited for (as in
//! `a_page_chrome_tells_of_after_what_it_asks_for.rs`). The Chrome this test
//! starts is the machine's own behind a relay — this test's own program,
//! run again as the far side of the pipe — which hands every message on as
//! it was sent until it is armed, and then takes the next command sent
//! through a session and sends Chrome `Page.close` through that session in
//! its place. Chrome closes the page and detaches the session, and the
//! command the tab sent is never answered, which is what Chrome was measured
//! to do. The tab must hear the session go and say the page was closed,
//! well inside its patience.

#![cfg(unix)]

use alo_window::beside::{chrome_in, chrome_places};
use alo_window::chrome::Chrome;
use alo_window::chrome_tab::Tab;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Set, in the program run again as the far side: the machine's Chrome.
const FAR_SIDE_OF: &str = "ALO_FAR_SIDE_OF";

/// Set beside it: the directory the far side was made in, which holds its
/// arguments, the mark that arms it, and the mark it leaves once it has
/// closed the page.
const FAR_SIDE_IN: &str = "ALO_FAR_SIDE_IN";

/// The id the far side sends `Page.close` with: one the browser never uses
/// so soon and Chrome reads as an id — a 32-bit integer — so its answer
/// is nobody's.
const CLOSE_ID: u64 = 2_000_000_000;

/// How long Chrome is waited for in this test: long enough that a tab
/// waiting it out is told apart from one that says at once.
const PATIENCE: Duration = Duration::from_secs(20);

/// A far side made for the test `test`, in a directory of its own: the
/// program to start as Chrome.
fn far_side(test: &str) -> Result<FarSide, String> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let chrome = chrome_in(&chrome_places(home.as_deref())).map_err(|why| why.to_string())?;
    let me = std::env::current_exe().map_err(|why| why.to_string())?;
    let directory =
        std::env::temp_dir().join(format!("alo-far-close-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir(&directory).map_err(|why| why.to_string())?;
    let quoted = |path: &Path| format!("'{}'", path.display().to_string().replace('\'', r"'\''"));
    let program = directory.join("chrome");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > {args}\n\
             {FAR_SIDE_OF}={chrome} {FAR_SIDE_IN}={directory} \
             exec {me} {test} --exact --test-threads=1\n",
            args = quoted(&directory.join("args")),
            chrome = quoted(&chrome),
            directory = quoted(&directory),
            me = quoted(&me),
        ),
    )
    .map_err(|why| why.to_string())?;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
        .map_err(|why| why.to_string())?;
    Ok(FarSide { directory })
}

/// A far side's directory, removed with it.
struct FarSide {
    directory: PathBuf,
}

impl FarSide {
    /// The program to start as Chrome.
    fn program(&self) -> PathBuf {
        self.directory.join("chrome")
    }

    /// Arm it: the next command through a session closes the page instead.
    fn arm(&self) -> Result<(), String> {
        File::create(self.directory.join("armed"))
            .map(drop)
            .map_err(|why| why.to_string())
    }

    /// Whether it closed the page in place of a command.
    fn closed(&self) -> bool {
        self.directory.join("closed").exists()
    }
}

impl Drop for FarSide {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// When this program was run again as a far side, be it and end;
/// otherwise do nothing.
fn be_the_far_side_if_asked() {
    let (Some(chrome), Some(directory)) =
        (std::env::var_os(FAR_SIDE_OF), std::env::var_os(FAR_SIDE_IN))
    else {
        return;
    };
    let ended = relay(Path::new(&chrome), Path::new(&directory));
    std::process::exit(i32::from(ended.is_err()));
}

/// The session a command the browser wrote is sent through, if any: this
/// browser's own bytes, as `chrome_protocol::written` writes them.
fn session_of(command: &[u8]) -> Option<String> {
    let text = core::str::from_utf8(command).ok()?;
    let (_, after) = text.split_once(r#""sessionId":""#)?;
    let (session, _) = after.split_once('"')?;
    Some(session.to_owned())
}

/// Start the machine's Chrome with the arguments this far side was given
/// and hand each message on between it and the browser on this side's
/// descriptors 3 and 4 — once armed, sending `Page.close` in place of the
/// next command through a session.
fn relay(chrome: &Path, directory: &Path) -> Result<(), String> {
    let args = std::fs::read_to_string(directory.join("args")).map_err(|why| why.to_string())?;
    let mut started = Command::new("/bin/sh")
        .arg("-c")
        .arg(r#"exec "$0" "$@" 3<&0 4>&1 0</dev/null 1>/dev/null"#)
        .arg(chrome)
        .args(args.lines())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|why| why.to_string())?;
    let mut to_chrome = started.stdin.take().ok_or("no stdin")?;
    let mut from_chrome = BufReader::new(started.stdout.take().ok_or("no stdout")?);
    let from_browser = File::open("/dev/fd/3").map_err(|why| why.to_string())?;
    let armed = directory.join("armed");
    let closed = directory.join("closed");
    std::thread::spawn(move || {
        let mut from_browser = BufReader::new(from_browser);
        let mut message = Vec::new();
        let mut done = false;
        while from_browser
            .read_until(0, &mut message)
            .is_ok_and(|read| read > 0)
        {
            let session = session_of(&message).filter(|_| !done && armed.exists());
            let sent = match session {
                Some(session) => {
                    done = true;
                    let close = format!(
                        r#"{{"id":{CLOSE_ID},"method":"Page.close","sessionId":"{session}"}}"#
                    );
                    let mut close = close.into_bytes();
                    close.push(0);
                    let written = to_chrome.write_all(&close);
                    if written.is_ok() && File::create(&closed).is_err() {
                        return;
                    }
                    written
                }
                None => to_chrome.write_all(&message),
            };
            if sent.is_err() {
                return;
            }
            message.clear();
        }
    });
    let mut to_browser = OpenOptions::new()
        .write(true)
        .open("/dev/fd/4")
        .map_err(|why| why.to_string())?;
    let mut message = Vec::new();
    while from_chrome
        .read_until(0, &mut message)
        .is_ok_and(|read| read > 0)
    {
        to_browser
            .write_all(&message)
            .map_err(|why| why.to_string())?;
        message.clear();
    }
    let _ = started.wait();
    Ok(())
}

#[test]
fn a_question_unanswered_as_the_page_closes_is_answered_that_it_closed() -> Result<(), String> {
    be_the_far_side_if_asked();
    let far = far_side("a_question_unanswered_as_the_page_closes_is_answered_that_it_closed")?;
    let mut chrome = Chrome::start_at(&far.program())
        .map_err(|why| why.to_string())?
        .waiting_at_most(PATIENCE);
    let mut tab = Tab::open_frozen(
        &mut chrome,
        "https://example.com/closing.html",
        b"<!doctype html><title>Closing</title><h1>Closing</h1>".to_vec(),
        (600, 400),
    )
    .map_err(|why| why.to_string())?;
    tab.read().map_err(|why| why.to_string())?;

    far.arm()?;
    let asked = Instant::now();
    let read = tab.read().map(drop).map_err(|why| why.to_string());
    let took = asked.elapsed();
    assert!(
        far.closed(),
        "the far side closed the page in place of a question"
    );
    let said = read.err().ok_or("a closed page's tree was read")?;
    assert!(
        said.contains("the tab's page was closed, and Chrome answers nothing of it"),
        "{said}"
    );
    assert!(
        took < PATIENCE / 2,
        "said in {} ms, where Chrome is waited for {} ms",
        took.as_millis(),
        PATIENCE.as_millis()
    );

    // And asked again, it is said before anything is sent.
    let again = Instant::now();
    let read = tab.read().map(drop).map_err(|why| why.to_string());
    assert!(read.is_err_and(|said| said.contains("the tab's page was closed")));
    assert!(again.elapsed() < Duration::from_secs(1));
    Ok(())
}
