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

mod bus;

use bus::{Alo, Scratch, Seen, WINDOW, cannot_run, found, osascript, read_until};

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
    let alo = Alo::start(&scratch.0.join("data"), &[])?;
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
