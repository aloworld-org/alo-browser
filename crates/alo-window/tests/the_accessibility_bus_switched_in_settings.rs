/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 989, ADR 0115's guard, (a) and (d): *Settings › Accessibility
//! bus* through macOS's accessibility API, read and pressed by another
//! program as a screen reader would — `osascript` asking System Events.
//!
//! `alo --conversation --frozen-fonts --data DIR` is the built program, run
//! as a person would run it, its window shown, with a data directory of
//! its own whose settings file the test writes first — never the person's
//! settings, keychain, chats or sign-ins — and no card, so no Chromium.
//!
//! - **Off**: once the bus began listening — the line alo says and keeps
//!   in its record, naming no program — nothing of alo's interface is on
//!   the bus, read again and again for some seconds: no *New chat*, no box,
//!   no *Settings*. Then a person's keys alone, as a person who cannot see
//!   the screen has them — ⌘, to open *Settings*, ⇧Tab to its last stop,
//!   the group *Screen readers*, → round to *Read and act* — and the
//!   whole interface is on the bus again.
//! - **Read only**: the interface is on the bus as ever, and `AXPress` on
//!   *New chat* is refused and written — no second chat — as a value set
//!   on the box is: the box stays empty.
//!
//! **Skipped, saying why, never quietly passed**, when the screen is locked
//! or the program running the test may not read other programs'
//! accessibility objects (`bus::cannot_run`). Run it with `--no-capture`
//! to see the line.

#![cfg(target_os = "macos")]

mod bus;

use bus::{AT_MOST, Alo, BETWEEN, Scratch, Seen, WINDOW, cannot_run, found, osascript, read_until};
use std::path::Path;
use std::time::{Duration, Instant};

/// The box's name, as the first screen names it.
const THE_BOX: &str = "Ask anything, search, or type a website";

/// How long the bus is read, once it listens, to see nothing of alo's on it.
const WATCHED: Duration = Duration::from_secs(4);

/// `text` kept as the settings a person chose, in the data directory
/// `data` — its own chats' directory, never a person's — before alo starts
/// there: readable by this user alone, as alo keeps it.
fn kept_settings(data: &Path, text: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let chats = data.join("Chats");
    std::fs::create_dir_all(&chats).map_err(|why| why.to_string())?;
    std::fs::set_permissions(&chats, std::fs::Permissions::from_mode(0o700))
        .map_err(|why| why.to_string())?;
    let file = chats.join("settings.txt");
    std::fs::write(&file, text).map_err(|why| why.to_string())?;
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600))
        .map_err(|why| why.to_string())
}

/// The settings file of a person who chose `bus` and nothing else.
fn settings(bus: &str) -> String {
    format!(
        "# alo: what a person chose in Settings (docs/design/shell.md)\nfont Manrope\nbus {bus}\n"
    )
}

/// Bring alo's window in front and press ⌘, then ⇧Tab then →, as a person
/// at its keyboard would.
const KEYS: &str = r#"
on run argv
	set pid to (item 1 of argv) as integer
	tell application "System Events"
		set p to first process whose unix id is pid
		set frontmost of p to true
		delay 1
		keystroke "," using command down
		delay 1
		key code 48 using shift down
		delay 0.5
		key code 124
	end tell
	return "done"
end run
"#;

/// Whether `read` holds anything of alo's own interface.
fn interface(read: &[Seen]) -> Option<&Seen> {
    read.iter().find(|node| {
        node.title == "New chat"
            || node.title == "Settings"
            || node.title == THE_BOX
            || node.role == "AXTextField"
            || node.role == "AXLink"
    })
}

/// The lines of alo's tree as System Events reads them now: nothing when
/// there is no node under the window to walk from.
fn walked(scratch: &Scratch, pid: &str) -> Result<Vec<Seen>, String> {
    match osascript(&scratch.0.join("walk.applescript"), &[pid]) {
        Ok(printed) => Ok(printed.lines().filter_map(Seen::of).collect()),
        Err(why) if bus::refused_access(&why) || why.contains("script error") => Err(why),
        Err(_) => Ok(Vec::new()),
    }
}

#[test]
fn off_publishes_nothing_until_a_persons_keys_choose_read_and_act() -> Result<(), String> {
    if let Some(why) = cannot_run() {
        eprintln!("skipped: {why} (queue item 989)");
        return Ok(());
    }
    let scratch = Scratch::new()?;
    let data = scratch.0.join("data");
    kept_settings(&data, &settings("off"))?;
    let alo = Alo::start(&data, &[])?;
    let pid = alo.child.id().to_string();
    // Read until the bus has asked alo for its tree: the line in its
    // record, which names no program.
    let by = Instant::now() + AT_MOST;
    let began = loop {
        if Instant::now() > by {
            return Err(format!("the bus never began listening within {AT_MOST:?}"));
        }
        let read = walked(&scratch, &pid)?;
        if let Some(seen) = interface(&read) {
            return Err(format!("published while Off: {seen:?} in {read:#?}"));
        }
        if let Ok(line) = alo.heard_within(
            "the bus began listening",
            |said| {
                said.starts_with(
                    "alo: assistive technology: the accessibility bus began listening, ",
                )
            },
            BETWEEN,
        ) {
            break line;
        }
    };
    assert!(began.ends_with(" GMT"), "{began}");
    // Read again and again, for some seconds: still nothing of alo's.
    let watched = Instant::now() + WATCHED;
    let mut reads = 0;
    while Instant::now() < watched {
        let read = walked(&scratch, &pid)?;
        if let Some(seen) = interface(&read) {
            return Err(format!("published while Off: {seen:?} in {read:#?}"));
        }
        reads += 1;
        std::thread::sleep(BETWEEN);
    }
    assert!(reads > 1, "read {reads} times");

    // A person's keys alone turn it back on: ⌘, ⇧Tab →.
    let keys = scratch.0.join("keys.applescript");
    std::fs::write(&keys, KEYS).map_err(|why| why.to_string())?;
    assert_eq!(osascript(&keys, &[&pid])?, "done");
    alo.heard("alo: Screen readers: Read and act, as chosen in Settings")?;
    let read = read_until(&scratch.0, &pid, |read| {
        found(read, "AXRadioButton", "Read and act").is_some()
    })?;
    assert!(found(&read, "AXLink", "Settings").is_some(), "{read:#?}");
    Ok(())
}

#[test]
fn read_only_publishes_the_interface_and_refuses_every_action() -> Result<(), String> {
    if let Some(why) = cannot_run() {
        eprintln!("skipped: {why} (queue item 989)");
        return Ok(());
    }
    let scratch = Scratch::named("-read-only")?;
    let data = scratch.0.join("data");
    kept_settings(&data, &settings("read"))?;
    let alo = Alo::start(&data, &[])?;
    let pid = alo.child.id().to_string();
    let read = match read_until(&scratch.0, &pid, |read| {
        found(read, "AXLink", "New chat").is_some() && found(read, "AXTextField", THE_BOX).is_some()
    }) {
        Ok(read) => read,
        Err(why) if why.starts_with("refused: ") => {
            eprintln!("skipped: System Events refused alo's tree: {why} (queue item 989)");
            return Ok(());
        }
        Err(why) => return Err(why),
    };
    // Published as ever: the window, the design's size, holding the
    // interface.
    let window = read.first().ok_or("nothing of alo's was read")?;
    let (_, _, width, height) = window.frame.ok_or("the window has no frame")?;
    assert_eq!((width, height), WINDOW, "{window:?}");
    let chats = |read: &[Seen]| {
        read.iter()
            .filter(|node| node.role == "AXLink" && node.title == "New chat")
            .count()
    };
    assert_eq!(chats(&read), 2, "New chat and the one chat: {read:#?}");
    alo.heard_that("the bus began listening", |said| {
        said.starts_with("alo: assistive technology: the accessibility bus began listening, ")
    })?;

    // AXPress on *New chat*: refused, written, and no chat made.
    let act = scratch.0.join("act.applescript");
    assert_eq!(
        osascript(&act, &[&pid, "AXLink", "New chat", "press"])?,
        "done"
    );
    alo.heard_that("the press refused", |said| {
        said.starts_with("alo: assistive technology: a press on node ")
            && said.ends_with(
                "of the chats panel was refused: Settings › Screen readers is set to Read only",
            )
    })?;
    // The box's value set: refused, and the box as it was.
    let set = osascript(&act, &[&pid, "AXTextField", THE_BOX, "set", "porto"])?;
    assert_eq!(set, "done");
    alo.heard_that("the text refused", |said| {
        said.starts_with("alo: assistive technology: text put in, 5 characters on node ")
            && said.ends_with("is set to Read only")
    })?;
    let read = read_until(&scratch.0, &pid, |read| {
        found(read, "AXTextField", THE_BOX).is_some()
    })?;
    assert_eq!(chats(&read), 2, "no chat made: {read:#?}");
    assert_eq!(
        found(&read, "AXTextField", THE_BOX).map(|the_box| the_box.value.as_str()),
        Some("")
    );
    Ok(())
}
