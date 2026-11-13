/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 987, ADR 0115 phase (a): the conversation's window on macOS's
//! accessibility API, read and pressed by another program as a screen
//! reader would — `osascript` asking System Events — from the tree alo
//! already has.
//!
//! `alo --conversation --frozen-fonts --data DIR` is the built program, run
//! as a person would run it, with its window shown; with a data directory
//! of its own it touches none of the person's keychain, chats or sign-ins,
//! and no card is opened, so no Chromium is started. Then, through System
//! Events alone:
//!
//! - the panel's *New chat* is found as a link by its name, and the first
//!   screen's box as a text field by its name, each at its place and size
//!   in the window in numbers — the layout `tests/the_chats_panel.rs` and
//!   the first screen draw, in points, from the window's own corner;
//! - every node of alo's lists no action but `AXPress`: nothing that takes
//!   a point (ADR 0002);
//! - `AXPress` on *New chat* reaches alo's press — a second chat is in the
//!   panel — and is written in the record as assistive technology's;
//! - the box's value set reaches alo's box, and reads back.
//!
//! **Skipped, saying why, never quietly passed**, when the screen is locked
//! (a locked session's windows are not read), or when the program running
//! the test may not read other programs' accessibility objects (the Mac's
//! *Accessibility* permission, which the person grants). Run it with
//! `--no-capture` to see the line.

#![cfg(target_os = "macos")]

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// How long alo's window is waited for to be on the bus with its tree.
const AT_MOST: Duration = Duration::from_secs(60);

/// How long one `osascript` is given before it is killed.
const ONE_ASK: Duration = Duration::from_secs(30);

/// How long between two readings while waiting.
const BETWEEN: Duration = Duration::from_millis(250);

/// The design's window, in points: what the conversation's window opens at.
const WINDOW: (i64, i64) = (1280, 820);

/// Every node under `start`, one a line: its role, its title, its value,
/// its place and size, and its actions, each between bars — read through
/// each element's own children, never `entire contents`, which names one
/// row's twin twice.
const WALK: &str = r#"
on walk(e, out)
	tell application "System Events"
	try
		set r to role of e
	on error
		set r to "?"
	end try
	try
		set t to title of e
		if t is missing value then set t to ""
	on error
		set t to ""
	end try
	try
		set v to value of e
		if v is missing value then set v to ""
	on error
		set v to ""
	end try
	try
		set pos to position of e
		set sz to size of e
		set g to ((item 1 of pos) as text) & "|" & ((item 2 of pos) as text) & "|" & ((item 1 of sz) as text) & "|" & ((item 2 of sz) as text)
	on error
		set g to "|||"
	end try
	try
		set acts to name of every action of e
		set AppleScript's text item delimiters to ","
		set a to acts as text
		set AppleScript's text item delimiters to ""
	on error
		set a to ""
	end try
	set out to out & r & "|" & t & "|" & (v as text) & "|" & g & "|" & a & linefeed
	try
		repeat with c in (UI elements of e)
			set out to my walk(contents of c, out)
		end repeat
	end try
	end tell
	return out
end walk

on run argv
	set pid to (item 1 of argv) as integer
	tell application "System Events"
		set p to first process whose unix id is pid
		if (count of windows of p) is 0 then return ""
		set ours to first UI element of window 1 of p
	end tell
	return my walk(ours, "")
end run
"#;

/// Press, or set the value of, the first node of alo's whose role and
/// title are the second and third arguments: `press`, or `set` to the
/// fifth. The first is found among `entire contents`, which names it
/// once, however it names a twin after it.
const ACT: &str = r#"
on run argv
	set pid to (item 1 of argv) as integer
	tell application "System Events"
		set p to first process whose unix id is pid
		set everything to entire contents of (first UI element of window 1 of p)
		repeat with e in everything
			set matched to false
			try
				set matched to (role of e is (item 2 of argv) and title of e is (item 3 of argv))
			end try
			if matched then
				if (item 4 of argv) is "press" then
					perform action "AXPress" of e
				else
					set value of e to (item 5 of argv)
				end if
				return "done"
			end if
		end repeat
	end tell
	return "not found"
end run
"#;

/// One node as System Events read it.
#[derive(Debug, Clone, PartialEq)]
struct Seen {
    role: String,
    title: String,
    value: String,
    /// Its place and size, in points from the screen's corner.
    frame: Option<(i64, i64, i64, i64)>,
    actions: Vec<String>,
}

impl Seen {
    fn of(line: &str) -> Option<Self> {
        let fields: Vec<&str> = line.split('|').collect();
        let [role, title, value, x, y, width, height, actions] = fields.as_slice() else {
            return None;
        };
        let number = |field: &str| field.trim().parse::<i64>().ok();
        Some(Self {
            role: (*role).to_owned(),
            title: (*title).to_owned(),
            value: (*value).to_owned(),
            frame: number(x)
                .zip(number(y))
                .zip(number(width).zip(number(height)))
                .map(|((x, y), (width, height))| (x, y, width, height)),
            actions: actions
                .split(',')
                .filter(|action| !action.is_empty())
                .map(str::to_owned)
                .collect(),
        })
    }
}

/// A scratch directory of the test's own, private as alo asks a data
/// directory to be, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Self, String> {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("alo-a11y-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("data")).map_err(|why| why.to_string())?;
        std::fs::set_permissions(path.join("data"), std::fs::Permissions::from_mode(0o700))
            .map_err(|why| why.to_string())?;
        for (name, script) in [("walk.applescript", WALK), ("act.applescript", ACT)] {
            std::fs::write(path.join(name), script).map_err(|why| why.to_string())?;
        }
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// alo, started; killed and waited for when dropped.
struct Alo {
    child: Child,
    said: Receiver<String>,
}

impl Drop for Alo {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Alo {
    fn start(data: &Path) -> Result<Self, String> {
        let mut child = Command::new(env!("CARGO_BIN_EXE_alo"))
            .args(["--conversation", "--frozen-fonts", "--data"])
            .arg(data)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|why| format!("alo did not start: {why}"))?;
        let stderr = child.stderr.take().ok_or("alo has no error output")?;
        let (tell, said) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else {
                    return;
                };
                if tell.send(line).is_err() {
                    return;
                }
            }
        });
        Ok(Self { child, said })
    }

    /// Wait for alo to say `line`, at most [`AT_MOST`]: what it said.
    fn heard(&self, line: &str) -> Result<(), String> {
        let by = Instant::now() + AT_MOST;
        let mut heard = Vec::new();
        while let Some(left) = by.checked_duration_since(Instant::now()) {
            let Ok(said) = self.said.recv_timeout(left) else {
                break;
            };
            if said == line {
                return Ok(());
            }
            heard.push(said);
        }
        Err(format!("alo never said {line:?}; it said {heard:?}"))
    }
}

/// What `osascript` printed for `script` with `arguments`, given at most
/// [`ONE_ASK`] — or what it said went wrong.
fn osascript(script: &Path, arguments: &[&str]) -> Result<String, String> {
    let mut child = Command::new("/usr/bin/osascript")
        .arg(script)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|why| format!("osascript did not start: {why}"))?;
    let mut stdout = child.stdout.take().ok_or("osascript has no output")?;
    let mut stderr = child.stderr.take().ok_or("osascript has no error output")?;
    let (out, read_out) = channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        let _ = out.send(text);
    });
    let (err, read_err) = channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        let _ = err.send(text);
    });
    let by = Instant::now() + ONE_ASK;
    let status = loop {
        if let Ok(Some(status)) = child.try_wait() {
            break status;
        }
        if Instant::now() > by {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("osascript said nothing for {ONE_ASK:?}"));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let printed = read_out.recv_timeout(ONE_ASK).unwrap_or_default();
    let complained = read_err.recv_timeout(ONE_ASK).unwrap_or_default();
    if status.success() {
        Ok(printed.trim_end().to_owned())
    } else {
        Err(complained.trim().to_owned())
    }
}

/// Why this Mac cannot run the test now, if it cannot: its screen is
/// locked, or this program may not read other programs' accessibility
/// objects.
fn cannot_run() -> Option<String> {
    let locked = Command::new("/usr/sbin/ioreg")
        .args(["-n", "Root", "-d1", "-a"])
        .output()
        .ok()
        .map(|read| String::from_utf8_lossy(&read.stdout).into_owned())
        .and_then(|plist| {
            let after = plist.split("<key>IOConsoleLocked</key>").nth(1)?;
            Some(after.trim_start().starts_with("<true/>"))
        });
    if locked == Some(true) {
        return Some(
            "the screen is locked, and a locked session's windows are not read".to_owned(),
        );
    }
    let trusted = Command::new("/usr/bin/osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get UI elements enabled",
        ])
        .output()
        .ok()
        .map(|read| String::from_utf8_lossy(&read.stdout).trim().to_owned());
    if trusted.as_deref() != Some("true") {
        return Some(format!(
            "osascript may not read other programs' accessibility objects (System Settings › \
             Privacy & Security › Accessibility, for the program running this test): {trusted:?}"
        ));
    }
    None
}

/// Whether `complaint`, from `osascript`, says it was refused the
/// accessibility objects it asked for.
fn refused_access(complaint: &str) -> bool {
    complaint.contains("-25211") || complaint.contains("-1743") || complaint.contains("assistive")
}

/// alo's nodes as System Events reads them, once `ready` says they are
/// all there, waited for at most [`AT_MOST`].
fn read_until(
    scripts: &Path,
    pid: &str,
    ready: impl Fn(&[Seen]) -> bool,
) -> Result<Vec<Seen>, String> {
    let by = Instant::now() + AT_MOST;
    let mut last = String::new();
    while Instant::now() < by {
        match osascript(&scripts.join("walk.applescript"), &[pid]) {
            Ok(printed) => {
                let read: Vec<Seen> = printed.lines().filter_map(Seen::of).collect();
                if ready(&read) {
                    return Ok(read);
                }
                last = printed;
            }
            Err(why) if refused_access(&why) => return Err(format!("refused: {why}")),
            // A script that does not compile never will.
            Err(why) if why.contains("script error") => return Err(why),
            // The window not there yet, or System Events still finding it.
            Err(why) => last = why,
        }
        std::thread::sleep(BETWEEN);
    }
    Err(format!(
        "alo's tree was not read as wanted within {AT_MOST:?}; last: {last}"
    ))
}

fn found<'a>(read: &'a [Seen], role: &str, title: &str) -> Option<&'a Seen> {
    read.iter()
        .find(|node| node.role == role && node.title == title)
}

fn chats(read: &[Seen]) -> usize {
    // The chats are the panel's links between *New chat* and *Settings*.
    read.iter()
        .filter(|node| node.role == "AXLink" && node.title == "New chat")
        .count()
        .saturating_sub(1)
}

/// The box's name, as the first screen names it.
const THE_BOX: &str = "Ask anything, search, or type a website";

#[test]
fn the_conversation_window_is_read_and_pressed_through_macos_accessibility() -> Result<(), String> {
    if let Some(why) = cannot_run() {
        eprintln!("skipped: {why} (queue item 987)");
        return Ok(());
    }
    let scratch = Scratch::new()?;
    let alo = Alo::start(&scratch.0.join("data"))?;
    let pid = alo.child.id().to_string();
    let read = match read_until(&scratch.0, &pid, |read| {
        found(read, "AXLink", "New chat").is_some() && found(read, "AXTextField", THE_BOX).is_some()
    }) {
        Ok(read) => read,
        Err(why) if why.starts_with("refused: ") => {
            eprintln!("skipped: System Events refused alo's tree: {why} (queue item 987)");
            return Ok(());
        }
        Err(why) => return Err(why),
    };
    // The window's own node, the first of alo's: the inside of the
    // window, the design's size, from which every node is placed.
    let window = read.first().ok_or("nothing of alo's was read")?;
    let (left, top, width, height) = window.frame.ok_or("the window has no frame")?;
    assert_eq!((width, height), WINDOW, "{window:?}");
    // *New chat* is a link, 10 in and 56 down in the panel, 220 by 40.
    let new_chat = found(&read, "AXLink", "New chat").ok_or("no New chat")?;
    assert_eq!(new_chat.frame, Some((left + 10, top + 56, 220, 40)));
    assert_eq!(new_chat.actions, ["AXPress"]);
    // The box is a text field on the first screen, 240 in: at (183, 266)
    // in it, 632 by 40.
    let the_box = found(&read, "AXTextField", THE_BOX).ok_or("no box")?;
    assert_eq!(the_box.frame, Some((left + 240 + 183, top + 266, 632, 40)));
    assert_eq!(the_box.value, "");
    // *Settings* is a link too, and *Go* a button.
    assert!(found(&read, "AXLink", "Settings").is_some(), "{read:#?}");
    assert!(found(&read, "AXButton", "Go").is_some(), "{read:#?}");
    // Nothing of alo's offers anything but a press: no action takes a point.
    let offered: Vec<&Seen> = read
        .iter()
        .filter(|node| node.actions.iter().any(|action| action != "AXPress"))
        .collect();
    assert!(offered.is_empty(), "{offered:#?}");
    assert_eq!(chats(&read), 1, "one chat as alo opens: {read:#?}");

    // A press by AXPress reaches alo's press: a second chat.
    let pressed = osascript(
        &scratch.0.join("act.applescript"),
        &[&pid, "AXLink", "New chat", "press"],
    )?;
    assert_eq!(pressed, "done");
    alo.heard("alo: assistive technology: a press on the chats panel came to NewChat")?;
    let read = read_until(&scratch.0, &pid, |read| chats(read) == 2)?;
    assert_eq!(chats(&read), 2);

    // The box's value set reaches the box, and reads back.
    let set = osascript(
        &scratch.0.join("act.applescript"),
        &[&pid, "AXTextField", THE_BOX, "set", "porto hotels"],
    )?;
    assert_eq!(set, "done");
    alo.heard(
        "alo: assistive technology: text put in, 12 characters on the first screen came to TheBox",
    )?;
    let read = read_until(&scratch.0, &pid, |read| {
        found(read, "AXTextField", THE_BOX).is_some_and(|the_box| the_box.value == "porto hotels")
    })?;
    assert!(found(&read, "AXTextField", THE_BOX).is_some());
    Ok(())
}
