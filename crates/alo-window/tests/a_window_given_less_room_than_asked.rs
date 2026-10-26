/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A window given less room than it asked for is used, not refused (queue
//! item 671).
//!
//! On 2026-10-26, with the session locked, Chrome's window held the page at
//! 1200×571 where `alo --chrome` asked for 1200×800, and the program stopped
//! before it said where an agent reaches it. The same test passed minutes
//! later with the screen shown. A window manager may give a window less
//! than it asks for — a locked session, a laptop's screen, a dock — so
//! waiting for a screen to misbehave is no way to test it.
//!
//! Here the room is made too small by asking for more than any screen
//! has: a page 1,200 wide, as `alo --chrome` asks, and 6,000 tall — within
//! what is laid out at all, but five times the tallest screen this machine
//! has. Chrome is started shown, as `alo --chrome` starts it; the window is
//! given what the screen has, and that is what is answered, in numbers: the
//! page as wide as asked and shorter, said plainly, and the document's box
//! in the tree an agent reads — down the agent socket, through `alo-ask` —
//! that same size.
//!
//! Measured on macOS 26 with Chrome 155 while writing this: a window is cut
//! to the screen's height, under the menu bar, and not to its width — asked
//! for 8,000 by 6,000, Chrome's window held the page at 8,000 by 1,041 — so
//! it is the height that is asked for past the screen.

use alo_window::agent_sites::AgentSites;
use alo_window::agents::Listening;
use alo_window::chrome_serving::{ChromeServing, Opening};
use alo_window::opening::CHROME_AT;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What is asked: taller than any screen, and less than
/// [`alo_window::chrome_window::LARGEST`].
const ASKED: (u32, u32) = (1_200, 6_000);

/// Where the page says it was taken from. Frozen: nothing reaches a server.
const ADDRESS: &str = "https://example.com/room.html";

/// A heading and a link: something for an agent to read.
const PAGE: &str = "<!doctype html><title>Room</title><h1>Room</h1>\
                    <a href=\"#below\">Further down</a>\
                    <div style=\"height:9000px\"></div><h2 id=\"below\">Below</h2>";

/// A socket of this test's own, prepared as the browser prepares its own.
fn socket() -> Result<PathBuf, String> {
    let root = std::env::temp_dir().join(format!("alo-room-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).map_err(|why| why.to_string())?;
    alo_window::socket_place::prepare(&alo_window::socket_place::socket_under(&root))
        .map_err(|why| format!("no place for the socket: {why}"))
}

/// Send `lines` through `alo-ask`, a process of its own, and what it printed,
/// a line per answer.
fn ask(socket: &Path, lines: &[String]) -> Result<Vec<String>, String> {
    let mut asking = Command::new(env!("CARGO_BIN_EXE_alo-ask"))
        .arg(socket)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|why| format!("alo-ask did not start: {why}"))?;
    let mut stdin = asking.stdin.take().ok_or("alo-ask has no input")?;
    stdin
        .write_all(lines.join("\n").as_bytes())
        .map_err(|why| format!("the lines were not sent: {why}"))?;
    drop(stdin);
    let finished = asking
        .wait_with_output()
        .map_err(|why| format!("alo-ask did not finish: {why}"))?;
    if !finished.status.success() {
        return Err(format!(
            "alo-ask failed: {}",
            String::from_utf8_lossy(&finished.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&finished.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

#[test]
fn a_page_given_less_room_than_asked_is_laid_out_at_it_and_reached() -> Result<(), String> {
    let opening = Opening::Frozen {
        url: ADDRESS.to_owned(),
        page: PAGE.as_bytes().to_vec(),
    };
    let serving = ChromeServing::start_shown(opening, ASKED, CHROME_AT, AgentSites::Every)
        .map_err(|why| format!("refused where it should have run: {why}"))?;
    let placed = *serving.placed().ok_or("a shown Chrome has a window")?;

    // Less room than was asked, said as such, and a page still.
    let unlike = placed
        .unlike(ASKED)
        .ok_or("no screen holds 1200×6000, and the page was said to be laid out at it")?;
    assert!(unlike.is_less(), "{unlike}");
    let (wide, tall) = placed.page;
    assert!(
        (1.0..f64::from(ASKED.1)).contains(&tall),
        "shorter, and something: {wide}×{tall}"
    );
    // Whole pixels, read as Chrome's numbers print.
    let whole = |side: f64| format!("{side}").parse::<i64>();
    let (Ok(wide_whole), Ok(tall_whole)) = (whole(wide), whole(tall)) else {
        return Err(format!("not whole pixels: {wide}×{tall}"));
    };
    assert_eq!(wide_whole, 1200, "as wide as asked");
    assert_eq!(
        unlike.to_string(),
        format!(
            "Chrome's window was given less room than the 1200×6000 CSS pixels asked, and the \
             page is laid out at the {wide_whole}×{tall_whole} it holds"
        )
    );
    // The window holds the page and a title bar, and no more.
    let at = placed.window.bounds;
    assert_eq!(at.width, wide_whole, "an app window has no sides");
    assert!(
        (1..=64).contains(&(at.height - tall_whole)),
        "a title bar above the page: {}×{}",
        at.width,
        at.height
    );

    // The agent socket is opened and answered, and the tree it reads is the
    // page at the size the window was given.
    let socket = socket()?;
    let listening = Listening::at(&socket, serving.orders())
        .map_err(|why| format!("the socket was not opened: {why}"))?;
    let answers = ask(
        &socket,
        &[
            r#"{"v":1,"id":1,"ask":"tabs"}"#.to_owned(),
            format!(r#"{{"v":1,"id":2,"ask":"read","tab":"{ADDRESS}"}}"#),
        ],
    )?;
    listening.stop();
    let [tabs, tree] = answers.as_slice() else {
        return Err(format!("two answers, not {answers:?}"));
    };
    assert_eq!(
        tabs,
        &format!(r#"{{"answer":"tabs","id":1,"tabs":["{ADDRESS}"],"v":1}}"#)
    );
    assert!(tree.contains(r#""name":"Further down""#), "{tree}");
    assert!(
        tree.contains(&format!(
            r#""rect":{{"height":{tall:.1},"width":{wide:.1},"x":0.0,"y":0.0}},"role":"document""#
        )),
        "{tree}"
    );
    assert!(serving.records().is_some(), "Chrome's thread finished");
    Ok(())
}
