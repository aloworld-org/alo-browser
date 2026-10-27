/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Why a placed window was not where a person sees it (queue item 643).
//!
//! With Stage Manager on, a window that is not in front is shrunk into the
//! strip at the screen's left edge, and the window server lists it there,
//! about a hundred points square, while Chrome still gives the bounds it
//! put the window at. Under the suite's concurrent shown Chromes, a window
//! placed and brought in front was sometimes found in the strip, and
//! whether macOS had refused Chrome's activation was not known.
//!
//! It had not. Measured on 2026-10-27 against Chrome 156.0.8078.48 on
//! macOS 26.5.2, screen unlocked: `Target.activateTarget` brought the
//! window in front, at its bounds, after Finder or Terminal was activated
//! by a script, after a click on Terminal, while keys were pressed in
//! Terminal, and after another shown Chrome had been brought in front. In
//! every run the window in the strip was the one another window had been
//! brought in front of, and when two Chromes took turns, the one brought
//! in front last was the one in front. So it was the competing windows: the
//! suite's other shown Chromes, each bringing its own window in front.
//! Shown Chromes now take turns (`.config/nextest.toml`), and
//! `a_page_in_chromes_own_window` asserts the window server's bounds.
//!
//! That cause is made to happen here, on purpose, with two shown Chromes in
//! one test: the first placed, the second placed after it, and the first
//! brought in front again. The window server is asked each time. A window
//! sent into the strip is asserted to be listed elsewhere than Chrome's
//! bounds only while Stage Manager is on; with it off, the window behind is
//! still at its bounds, and that is asserted instead.

use alo_window::chrome::Chrome;
use alo_window::chrome_protocol::Bounds;
use alo_window::chrome_tab::Tab;
use alo_window::chrome_window::{self, Placed};
use alo_window::window_server::{self, Listed};
use std::time::{Duration, Instant};

/// The first page. Frozen: nothing reaches a server.
const FIRST: &str = "<!doctype html><title>First</title><h1>First</h1>";

/// The second page.
const SECOND: &str = "<!doctype html><title>Second</title><h1>Second</h1>";

/// The longest the window server is waited on to move a window into the
/// strip or out of it. Measured on 2026-10-27: out at the first reading
/// after, in within 700 ms.
const AT_MOST: Duration = Duration::from_secs(10);

/// Whether Stage Manager is on, by its own setting.
fn stage_manager_is_on() -> Result<bool, String> {
    let read = std::process::Command::new("defaults")
        .args(["read", "com.apple.WindowManager", "GloballyEnabled"])
        .output()
        .map_err(|why| format!("defaults did not run: {why}"))?;
    // Never set is off: the setting is written the first time it is turned
    // on.
    Ok(read.status.success() && String::from_utf8_lossy(&read.stdout).trim() == "1")
}

/// The one ordinary window the window server lists for the process `owner`.
fn listed(owner: u32) -> Result<Option<Bounds>, String> {
    let listed = window_server::on_the_screen(owner)
        .map_err(|why| format!("the window server was not read: {why}"))?;
    let ordinary: Vec<&Listed> = listed.iter().filter(|window| window.layer == 0).collect();
    let [window] = ordinary.as_slice() else {
        return Err(format!(
            "not one window of {owner} on the screen: {listed:?}"
        ));
    };
    Ok(window.bounds)
}

/// Wait, at most [`AT_MOST`], for the window of `owner` to be listed at
/// `bounds` (`at`) or anywhere else (`!at`), and say where it was last
/// listed if it never was.
fn until_listed(owner: u32, bounds: Bounds, at: bool, what: &str) -> Result<(), String> {
    let until = Instant::now() + AT_MOST;
    loop {
        let now = listed(owner)?;
        if (now == Some(bounds)) == at {
            return Ok(());
        }
        if Instant::now() > until {
            return Err(format!(
                "{what}: Chrome says {bounds:?}, the window server {now:?}"
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Open `page` in a shown Chrome's window at `size`.
fn opened<'a>(
    chrome: &'a mut Chrome,
    address: &str,
    page: &str,
    size: (u32, u32),
) -> Result<Tab<'a>, String> {
    Tab::open_frozen(chrome, address, page.as_bytes().to_vec(), size).map_err(|why| why.to_string())
}

/// Put the window of `tab` at `left`, `top`, brought in front.
fn placed(tab: &mut Tab<'_>, left: i32, top: i32) -> Result<Placed, String> {
    chrome_window::place(tab, left, top).map_err(|why| why.to_string())
}

#[test]
fn the_window_in_the_strip_is_the_one_another_was_brought_in_front_of() -> Result<(), String> {
    let strips = stage_manager_is_on()?;

    let mut first = Chrome::start_shown().map_err(|why| why.to_string())?;
    let first_process = first.process();
    let mut first_tab = opened(
        &mut first,
        "https://example.com/first.html",
        FIRST,
        (700, 400),
    )?;
    let first_at = placed(&mut first_tab, 160, 120)?.window.bounds;
    assert_eq!(
        (first_at.left, first_at.top, first_at.width),
        (160, 120, 700),
        "{first_at:?}"
    );
    until_listed(first_process, first_at, true, "the first, placed")?;

    // A second Chrome's window put on the screen after it, and brought in
    // front: the first goes into the strip, and Chrome's word for where it
    // is does not change.
    let mut second = Chrome::start_shown().map_err(|why| why.to_string())?;
    let second_process = second.process();
    let mut second_tab = opened(
        &mut second,
        "https://example.com/second.html",
        SECOND,
        (500, 300),
    )?;
    let second_at = placed(&mut second_tab, 420, 260)?.window.bounds;
    until_listed(second_process, second_at, true, "the second, placed")?;
    until_listed(
        first_process,
        first_at,
        !strips,
        if strips {
            "the first, in the strip behind the second"
        } else {
            "the first, behind the second"
        },
    )?;
    assert_eq!(
        chrome_window::placed(&mut first_tab)
            .map_err(|why| why.to_string())?
            .window
            .bounds,
        first_at,
        "Chrome still says the first is where it was put"
    );

    // The first brought in front again comes out of the strip, at its
    // bounds, and the second goes in: the one brought in front last is the
    // one a person sees.
    chrome_window::bring_to_front(&mut first_tab).map_err(|why| why.to_string())?;
    until_listed(first_process, first_at, true, "the first, in front again")?;
    until_listed(
        second_process,
        second_at,
        !strips,
        if strips {
            "the second, in the strip behind the first"
        } else {
            "the second, behind the first"
        },
    )?;
    Ok(())
}
