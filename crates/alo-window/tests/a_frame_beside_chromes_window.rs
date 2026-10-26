/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the frame of ours stands around Chrome's window, read from the
//! two witnesses ADR 0062 names: Chrome's word for where its window is,
//! and the window server's for whether it is on the screen there (queue
//! item 672, cut from 642).
//!
//! A shown Chrome's window is put, moved and resized over the pipe — which
//! Chrome reports exactly as it reports a person's drag of the same window,
//! measured on 2026-10-26 (`chrome_frame`'s documentation) — and closed,
//! and at each step the frame's stance is read and asserted in numbers.
//!
//! **What is not assumed.** Under the suite's other shown Chromes, a window
//! placed is left in Stage Manager's strip in about one run in four (item
//! 643), and the window server then shows it nowhere near its bounds. So
//! this test does not assert that Chrome's window is in front: it asserts
//! the rule. Either the window server lists Chrome's window at exactly the
//! bounds Chrome gives and the frame stands directly above them, or it
//! lists none there and the frame stands aside — each reading is one or the
//! other, in numbers, and is settled within the time a window takes to come
//! out of the strip.

#![cfg(target_os = "macos")]

use alo_window::chrome::Chrome;
use alo_window::chrome_frame::{self, Aside, Closed, FRAME_HEIGHT, Reading, Stance};
use alo_window::chrome_protocol::Bounds;
use alo_window::chrome_tab::Tab;
use alo_window::chrome_window;
use std::time::{Duration, Instant};

/// Where the page says it was taken from. Frozen: nothing reaches a server.
const ADDRESS: &str = "https://example.com/frame.html";

/// A page with nothing to wait for.
const PAGE: &str = "<!doctype html><title>Frame</title><h1>Framed</h1>";

/// How long a reading is given to settle: a window brought out of the
/// strip, or one still moving, within Chrome's own patience.
const SETTLE: Duration = Duration::from_secs(10);

/// A page's side, which Chrome gives as a float, as the whole number it is.
fn whole(side: f64) -> Result<i64, String> {
    side.to_string()
        .parse()
        .map_err(|_| format!("a page's side is not whole: {side}"))
}

/// Read until the frame's stance agrees with itself twice running, so that
/// a window caught between Chrome's reading and the window server's is
/// read again rather than believed.
fn settled(tab: &mut Tab<'_>) -> Result<Reading, String> {
    let until = Instant::now() + SETTLE;
    let mut last = chrome_frame::read(tab).map_err(|why| why.to_string())?;
    loop {
        std::thread::sleep(Duration::from_millis(150));
        let now = chrome_frame::read(tab).map_err(|why| why.to_string())?;
        let beside = matches!(now.stance, Stance::Beside { .. });
        if now == last && (beside || Instant::now() >= until) {
            return Ok(now);
        }
        if Instant::now() >= until {
            return Err(format!("never settled: {last:?} then {now:?}"));
        }
        last = now;
    }
}

/// The rule, checked on one reading of a window Chrome says is at
/// `chrome`: beside it at exactly its top, or aside with nothing of
/// Chrome's shown there. Answers whether it was beside, or how it broke.
fn the_rule_holds(reading: &Reading, owner: u32, chrome: Bounds) -> Result<bool, String> {
    let placed = reading
        .placed
        .ok_or_else(|| format!("a window not closed is read: {reading:?}"))?;
    if placed.window.bounds != chrome {
        return Err(format!("Chrome's word is not {chrome:?}: {reading:?}"));
    }
    let shown = reading
        .listed
        .iter()
        .any(|seen| seen.owner == owner && seen.layer == 0 && seen.bounds == Some(chrome));
    let above = Bounds {
        left: chrome.left,
        top: chrome.top - FRAME_HEIGHT,
        width: chrome.width,
        height: FRAME_HEIGHT,
    };
    match reading.stance {
        Stance::Beside { frame } if shown && frame == above => Ok(true),
        Stance::Aside(Aside::NotOnTheScreen) if !shown => Ok(false),
        _ => Err(format!(
            "neither directly above a window the screen shows, nor aside from one it does not: \
             {reading:?}"
        )),
    }
}

#[test]
fn the_frame_stands_above_chromes_window_where_the_screen_shows_it_and_goes_when_it_closes()
-> Result<(), String> {
    let mut chrome = Chrome::start_shown().map_err(|why| why.to_string())?;
    let owner = chrome.process();
    let mut tab = Tab::open_frozen(&mut chrome, ADDRESS, PAGE.as_bytes().to_vec(), (900, 600))
        .map_err(|why| why.to_string())?;
    let mut beside = 0;

    // Put, and read: the frame is above it or aside, by the screen's word.
    // The title bar is the window less its page, read rather than assumed:
    // a screen may give the page less than the 600 asked (item 671).
    let placed = chrome_window::place(&mut tab, 160, 180).map_err(|why| why.to_string())?;
    let title_bar = placed.window.bounds.height - whole(placed.page.1)?;
    let first = Bounds {
        left: 160,
        top: 180,
        width: 900,
        ..placed.window.bounds
    };
    assert_eq!(placed.window.bounds, first);
    let reading = settled(&mut tab)?;
    beside += usize::from(the_rule_holds(&reading, owner, first)?);

    // Moved, as a person's drag is reported: the frame follows.
    let moved = chrome_window::place(&mut tab, 300, 240).map_err(|why| why.to_string())?;
    let there = Bounds {
        left: 300,
        top: 240,
        ..first
    };
    assert_eq!(moved.window.bounds, there);
    let reading = settled(&mut tab)?;
    beside += usize::from(the_rule_holds(&reading, owner, there)?);

    // Resized: the page is laid out again at the new size, and the frame
    // is as wide as the window.
    let fitted = chrome_window::fit(&mut tab, 700, 500).map_err(|why| why.to_string())?;
    assert_eq!(fitted.page, (700.0, 500.0), "the page laid out again");
    let resized = Bounds {
        width: 700,
        height: 500 + title_bar,
        ..there
    };
    assert_eq!(fitted.window.bounds, resized);
    let reading = settled(&mut tab)?;
    beside += usize::from(the_rule_holds(&reading, owner, resized)?);
    eprintln!("beside in {beside} of 3 readings; aside, as the rule says, in the rest");

    // Closed: the frame goes, and the screen shows nothing of Chrome's. A
    // reading sent as the page's session is detached is never answered, so
    // each is bounded well inside Chrome's patience: the tab hears its
    // session go and says so rather than waiting it out.
    chrome_window::close(&mut tab).map_err(|why| why.to_string())?;
    let until = Instant::now() + SETTLE;
    let reading = loop {
        let asked = Instant::now();
        let reading = chrome_frame::read(&mut tab).map_err(|why| why.to_string())?;
        if asked.elapsed() >= SETTLE {
            return Err(format!(
                "a reading of a closed window took {} ms",
                asked.elapsed().as_millis()
            ));
        }
        if reading.stance == Stance::Closed(Closed::Page) || Instant::now() >= until {
            break reading;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(reading.stance, Stance::Closed(Closed::Page));
    assert_eq!(reading.placed, None);
    let until = Instant::now() + SETTLE;
    loop {
        let shown = alo_window::window_server::on_the_screen(owner)
            .map_err(|why| why.to_string())?
            .into_iter()
            .filter(|seen| seen.layer == 0)
            .count();
        if shown == 0 {
            break;
        }
        assert!(
            Instant::now() < until,
            "Chrome's window still on the screen: {shown}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

#[test]
fn a_headless_chrome_has_no_window_to_frame() -> Result<(), String> {
    let mut chrome = Chrome::start().map_err(|why| why.to_string())?;
    let mut tab = Tab::open_frozen(&mut chrome, ADDRESS, PAGE.as_bytes().to_vec(), (900, 600))
        .map_err(|why| why.to_string())?;
    let read = chrome_frame::read(&mut tab);
    assert_eq!(
        read.map_err(|why| why.to_string()),
        Err("Chrome is headless, and has no window to frame".to_owned())
    );
    Ok(())
}
