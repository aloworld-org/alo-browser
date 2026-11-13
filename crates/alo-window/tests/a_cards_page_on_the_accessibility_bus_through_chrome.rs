/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 988, ADR 0115 phase (b): a card's page on macOS's
//! accessibility API, grafted under the card in alo's own tree, read from
//! the tree the agent socket reads and pressed by node through the
//! socket's own `act` — read and pressed by another program as a screen
//! reader would, `osascript` asking System Events.
//!
//! `alo --conversation --frozen-fonts --data DIR ADDRESS` is the built
//! program, run as a person would run it, its window shown, the card's page
//! in the machine's Chrome. With a data directory of its own it touches
//! none of the person's keychain, chats or sign-ins; the page is the test's
//! own, served on `127.0.0.1`, and no card or payment is real. Then,
//! through System Events alone:
//!
//! - the page is a group named by its title, *Kampala Coffee*, inside the
//!   card, at the card's page box — the first screen 240 in, the page box
//!   81 in and 145 down in it, 878 by 520 points — and its *Say hello* a
//!   button in it, at its place on the page, in numbers. (AccessKit's
//!   macOS adapter shows a graft's subtree in the graft's place: the
//!   card's picture of the page, which the page is grafted at, is not an
//!   element of its own.)
//! - the page's password field is a secure text field (AccessKit's
//!   `PasswordInput`, known by the dots Chrome's tree holds in it) with no
//!   value, and its password is nowhere in alo's tree, nor its dots;
//! - nothing of the page lists an action but `AXPress`: none takes a point;
//! - `AXPress` on *Say hello* reaches the page — it writes that it was
//!   pressed, which is then read back through the bus — and is written as
//!   assistive technology's;
//! - a field's value set reaches the page's field, and reads back;
//! - `AXPress` on *Pay UGX 45,000* is held for the person's yes (ADR 0115,
//!   *Answered* 1): the page is not pressed, and the card asks with *Pay*;
//!   `AXPress` on alo's *Pay* — the bus's press on *Allow* is taken as any
//!   press is (*Answered* 4) — and only then does the page say it was paid.
//!
//! **Skipped, saying why, never quietly passed**, when the screen is locked
//! or the program running the test may not read other programs'
//! accessibility objects (`bus::cannot_run`). Run it with `--no-capture`
//! to see the line.

#![cfg(target_os = "macos")]

mod bus;
mod served;

use bus::{Alo, Scratch, Seen, WINDOW, cannot_run, found, osascript, read_until};

/// The checkout, laid out by hand so its boxes' places are known in CSS
/// pixels from the page's corner.
const PAGE: &str = r#"<!doctype html>
<html><head><title>Kampala Coffee</title>
<style>
  body { margin: 0; font-family: sans-serif; font-size: 16px }
  button, input { position: absolute; margin: 0; box-sizing: border-box; font-size: 16px }
  #hello { left: 40px; top: 60px; width: 200px; height: 40px }
  #name { left: 40px; top: 120px; width: 200px; height: 30px }
  #password { left: 40px; top: 170px; width: 200px; height: 30px }
  #pay { left: 40px; top: 220px; width: 200px; height: 40px }
  #said { position: absolute; left: 300px; top: 60px; margin: 0 }
</style></head>
<body>
<button id="hello" onclick="document.getElementById('said').textContent = 'Pressed by a click'">Say hello</button>
<input id="name" aria-label="Your name">
<input id="password" type="password" aria-label="Password" value="hunter2">
<button id="pay" onclick="document.getElementById('said').textContent = 'Paid UGX 45,000'">Pay UGX 45,000</button>
<p id="said">Not pressed yet</p>
</body></html>
"#;

/// The subrole of the first text field of alo's whose title is the
/// second argument.
const SUBROLE: &str = r#"
on run argv
	set pid to (item 1 of argv) as integer
	tell application "System Events"
		set p to first process whose unix id is pid
		set everything to entire contents of (first UI element of window 1 of p)
		repeat with e in everything
			set matched to false
			try
				set matched to (role of e is "AXTextField" and title of e is (item 2 of argv))
			end try
			if matched then
				try
					return (subrole of e) as text
				on error
					return "missing value"
				end try
			end if
		end repeat
	end tell
	return "not found"
end run
"#;

/// What the page is called: its title, which Chrome names its root by.
const TITLED: &str = "Kampala Coffee";

/// The node of the page's that holds `words`, as its value.
fn saying<'a>(read: &'a [Seen], words: &str) -> Option<&'a Seen> {
    read.iter().find(|node| node.value == words)
}

/// What alo said of the bus's action on the card's page, beginning so.
fn on_the_page(said: &str, begins: &str, ends: &str) -> bool {
    said.strip_prefix("alo: assistive technology: ")
        .is_some_and(|said| said.starts_with(begins) && said.contains(ends))
}

#[test]
fn a_cards_page_is_read_and_pressed_through_macos_accessibility() -> Result<(), String> {
    if let Some(why) = cannot_run() {
        eprintln!("skipped: {why} (queue item 988)");
        return Ok(());
    }
    let port = served::serve(PAGE)?;
    let address = format!("http://127.0.0.1:{port}/checkout.html");
    let scratch = Scratch::new()?;
    let alo = Alo::start(&scratch.0.join("data"), &[address.as_str()])?;
    let pid = alo.child.id().to_string();
    let read = match read_until(&scratch.0, &pid, |read| {
        found(read, "AXGroup", TITLED).is_some()
            && found(read, "AXButton", "Say hello").is_some()
            && found(read, "AXTextField", "Password").is_some()
    }) {
        Ok(read) => read,
        Err(why) if why.starts_with("refused: ") => {
            eprintln!("skipped: System Events refused alo's tree: {why} (queue item 988)");
            return Ok(());
        }
        Err(why) => return Err(why),
    };

    // The window's own node, the design's size, from which every node is
    // placed; the card's page where its page box is.
    let window = read.first().ok_or("nothing of alo's was read")?;
    let (left, top, width, height) = window.frame.ok_or("the window has no frame")?;
    assert_eq!((width, height), WINDOW, "{window:?}");
    let page = found(&read, "AXGroup", TITLED).ok_or("no page")?;
    let (x, y, page_width, page_height) = page.frame.ok_or("the page has no frame")?;
    assert_eq!(
        (x, y, page_width, page_height),
        (left + 240 + 81, top + 145, 878, 520),
        "{read:#?}"
    );
    // *Say hello* at (40, 60) on the page, 200 by 40; *Pay* at (40, 220).
    let hello = found(&read, "AXButton", "Say hello").ok_or("no Say hello")?;
    assert_eq!(hello.frame, Some((x + 40, y + 60, 200, 40)), "{hello:?}");
    assert_eq!(hello.actions, ["AXPress"]);
    let pay = found(&read, "AXButton", "Pay UGX 45,000").ok_or("no Pay")?;
    assert_eq!(pay.frame, Some((x + 40, y + 220, 200, 40)), "{pay:?}");
    // The password field: there, by its name, with no value; its password
    // and its dots nowhere.
    let password = found(&read, "AXTextField", "Password").ok_or("no password")?;
    assert_eq!(password.value, "", "{password:?}");
    assert_eq!(password.frame, Some((x + 40, y + 170, 200, 30)));
    // Published as a password field (AccessKit's `PasswordInput`), which
    // macOS calls a secure text field — known by the dots it holds.
    let subrole = scratch.0.join("subrole.applescript");
    std::fs::write(&subrole, SUBROLE).map_err(|why| why.to_string())?;
    assert_eq!(
        osascript(&subrole, &[&pid, "Password"])?,
        "AXSecureTextField"
    );
    assert_eq!(osascript(&subrole, &[&pid, "Your name"])?, "missing value");
    assert!(
        read.iter()
            .all(|node| !node.value.contains("hunter2") && !node.value.contains('\u{2022}')),
        "{read:#?}"
    );
    // Nothing offers anything but a press: no action takes a point.
    let offered: Vec<&Seen> = read
        .iter()
        .filter(|node| node.actions.iter().any(|action| action != "AXPress"))
        .collect();
    assert!(offered.is_empty(), "{offered:#?}");
    assert!(saying(&read, "Not pressed yet").is_some(), "{read:#?}");

    // A press by AXPress reaches the page by node, as the socket's
    // `activate`: the page says so, read back through the bus.
    let act = scratch.0.join("act.applescript");
    let pressed = osascript(&act, &[&pid, "AXButton", "Say hello", "press"])?;
    assert_eq!(pressed, "done");
    alo.heard_that("the press on Say hello came to the page", |said| {
        on_the_page(
            said,
            "a press on node ",
            "of the card's page came to activated",
        )
    })?;
    read_until(&scratch.0, &pid, |read| {
        saying(read, "Pressed by a click").is_some()
    })?;

    // A field's value set reaches the page's field, as `put_text`.
    let set = osascript(&act, &[&pid, "AXTextField", "Your name", "set", "Ada"])?;
    assert_eq!(set, "done");
    alo.heard_that("the text put in came to the page", |said| {
        on_the_page(said, "text put in, 3 characters on node ", "came to")
    })?;
    read_until(&scratch.0, &pid, |read| {
        found(read, "AXTextField", "Your name").is_some_and(|name| name.value == "Ada")
    })?;

    paid_only_once_allowed(&alo, &scratch, &pid)
}

/// `AXPress` on the page's *Pay UGX 45,000* held for the person's yes —
/// nothing paid, and the card asking with *Pay* — then `AXPress` on alo's
/// *Pay*, the bus's press on *Allow*, taken as any press is (ADR 0115,
/// *Answered* 4): the same node pressed, and the page saying it was paid.
fn paid_only_once_allowed(alo: &Alo, scratch: &Scratch, pid: &str) -> Result<(), String> {
    let act = scratch.0.join("act.applescript");
    let paying = osascript(&act, &[pid, "AXButton", "Pay UGX 45,000", "press"])?;
    if paying != "done" {
        return Err(format!("Pay UGX 45,000 was not pressed: {paying}"));
    }
    let held = alo.heard_that("the press on Pay held", |said| {
        on_the_page(
            said,
            "a press on node ",
            "pays, and waits on the person's answer",
        )
    })?;
    let node = held
        .split("a press on node ")
        .nth(1)
        .and_then(|after| after.split(' ').next())
        .ok_or_else(|| format!("no node in {held:?}"))?;
    let asked = read_until(&scratch.0, pid, |read| {
        found(read, "AXButton", "Pay").is_some()
    })?;
    if saying(&asked, "Paid UGX 45,000").is_some() {
        return Err(format!("paid before the person's yes: {asked:#?}"));
    }
    let allowed = osascript(&act, &[pid, "AXButton", "Pay", "press"])?;
    if allowed != "done" {
        return Err(format!("alo's Pay was not pressed: {allowed}"));
    }
    alo.heard(&format!(
        "alo: assistive technology: a press on node {node} of the card's page was allowed by \
         the person"
    ))?;
    read_until(&scratch.0, pid, |read| {
        saying(read, "Paid UGX 45,000").is_some()
    })
    .map(|_| ())
}
