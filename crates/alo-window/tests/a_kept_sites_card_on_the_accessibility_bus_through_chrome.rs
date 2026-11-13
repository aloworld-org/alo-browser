/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 989, ADR 0115's guard, (b): a card on a site kept from agents
//! read through macOS's accessibility API as another program would —
//! `osascript` asking System Events.
//!
//! `alo --conversation --frozen-fonts --data DIR --agent-site
//! https://example.com ADDRESS` is the built program, run as a person would
//! run it, its window shown, agents kept to `example.com`, and a card on a
//! page of the test's own served on `127.0.0.1` — another site — in the
//! machine's Chrome. With a data directory of its own it touches none of
//! the person's keychain, chats, settings or sign-ins. Then, through System
//! Events alone:
//!
//! - the card's page is one group, *this page is kept from agents*, where
//!   its page box is, holding nothing: none of the page's own words — its
//!   title, its button — is on the bus;
//! - the page's address is nowhere on the bus: no node's title or value
//!   holds its host, its port or its path — not the card's address bar, not
//!   what the person asked, not the chat's name in the panel, which is said
//!   as the same words;
//! - the rest of alo's interface is there: *New chat*, the card's *Close*.
//!
//! **Skipped, saying why, never quietly passed**, when the screen is locked
//! or the program running the test may not read other programs'
//! accessibility objects (`bus::cannot_run`). Run it with `--no-capture`
//! to see the line.

#![cfg(target_os = "macos")]

mod bus;
mod served;

use bus::{Alo, Scratch, Seen, WINDOW, cannot_run, found, read_until};

/// A page whose own words would say it was read.
const PAGE: &str = r"<!doctype html>
<html><head><title>Kampala Coffee</title>
<style>
  body { margin: 0; font-family: sans-serif; font-size: 16px }
  button { position: absolute; left: 40px; top: 60px; width: 200px; height: 40px; margin: 0 }
</style></head>
<body><button>Say hello</button></body></html>
";

/// What the bus is told of a kept page, and of its address.
const KEPT: &str = "this page is kept from agents";

/// The kept page: a group of [`KEPT`]'s words, the page box's size — the
/// panel's chat named by its address is a group of the same words too.
fn kept_page(read: &[Seen]) -> Option<&Seen> {
    read.iter().find(|node| {
        node.role == "AXGroup"
            && node.title == KEPT
            && node
                .frame
                .is_some_and(|(_, _, width, height)| (width, height) == (878, 520))
    })
}

#[test]
fn a_kept_sites_page_and_address_are_absent_from_the_bus() -> Result<(), String> {
    if let Some(why) = cannot_run() {
        eprintln!("skipped: {why} (queue item 989)");
        return Ok(());
    }
    let port = served::serve(PAGE)?;
    let address = format!("http://127.0.0.1:{port}/checkout.html");
    let scratch = Scratch::new()?;
    let alo = Alo::start(
        &scratch.0.join("data"),
        &["--agent-site", "https://example.com", address.as_str()],
    )?;
    let pid = alo.child.id().to_string();
    // Agents are kept to the one site named.
    alo.heard(
        "alo: this browser lets agents reach only https://example.com: a refusal of its own, \
         which narrows what reaches it and grants nothing",
    )?;
    let read = match read_until(&scratch.0, &pid, |read| {
        kept_page(read).is_some() && found(read, "AXButton", "Close").is_some()
    }) {
        Ok(read) => read,
        Err(why) if why.starts_with("refused: ") => {
            eprintln!("skipped: System Events refused alo's tree: {why} (queue item 989)");
            return Ok(());
        }
        Err(why) => return Err(why),
    };
    // The window, the design's size; the kept page where its page box is,
    // as a card's page is placed (item 988): 240 + 81 in, 145 down, 878 by
    // 520 — and nothing under it.
    let window = read.first().ok_or("nothing of alo's was read")?;
    let (left, top, width, height) = window.frame.ok_or("the window has no frame")?;
    assert_eq!((width, height), WINDOW, "{window:?}");
    let page = kept_page(&read).ok_or("no kept page")?;
    let page_box = (left + 240 + 81, top + 145, 878, 520);
    assert_eq!(page.frame, Some(page_box), "{read:#?}");
    // Nothing under it: no other node stands inside the page box.
    let (x, y, w, h) = page_box;
    let inside: Vec<&Seen> = read
        .iter()
        .filter(|node| !std::ptr::eq(*node, page))
        .filter(|node| {
            node.frame.is_some_and(|(nx, ny, nw, nh)| {
                nx >= x && ny >= y && nx + nw <= x + w && ny + nh <= y + h
            })
        })
        .collect();
    assert!(inside.is_empty(), "{inside:#?}");
    // Nothing of the page's own.
    for words in ["Kampala Coffee", "Say hello"] {
        assert!(
            read.iter()
                .all(|node| !node.title.contains(words) && !node.value.contains(words)),
            "{words} on the bus: {read:#?}"
        );
    }
    // Its address nowhere: neither its host, its port nor its path.
    let port = port.to_string();
    for said in ["127.0.0.1", port.as_str(), "checkout"] {
        let saying: Vec<_> = read
            .iter()
            .filter(|node| node.title.contains(said) || node.value.contains(said))
            .collect();
        assert!(saying.is_empty(), "{said} on the bus: {saying:#?}");
    }
    // The chat's name in the panel — the address's host — said as the same
    // words; and the rest of alo's interface there.
    assert!(found(&read, "AXLink", KEPT).is_some(), "{read:#?}");
    assert!(found(&read, "AXLink", "New chat").is_some(), "{read:#?}");
    alo.heard_that("the bus began listening", |said| {
        said.starts_with("alo: assistive technology: the accessibility bus began listening, ")
    })?;
    Ok(())
}
