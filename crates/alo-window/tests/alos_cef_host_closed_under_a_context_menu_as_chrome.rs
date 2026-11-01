/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 734: a page closed while its context menu is still open ends
//! alo's CEF host in order, as it ends Chrome — where it ended the host with
//! `SIGSEGV` inside CEF's framework.
//!
//! A shown host on a page of its own, placed and in front; a person's right
//! click on the page opens Chromium's context menu, and it is **left
//! open**. Then the page is closed each of the ways that crashed it:
//!
//! 1. by alo ending the host — `Browser.close`, as alo's ordinary close of a
//!    card's Chromium is ([`Chrome::close`]);
//! 2. by alo closing the page from inside first — `Target.closeTarget`
//!    (`chrome_window::close`), which Chromium does not carry out while the
//!    menu is open — and then ending it;
//! 3. by a person pressing the window's close button.
//!
//! Each: the menu is gone, the host ends by itself with status 0 —
//! asserted from the process's own status — its profile is removed, and
//! macOS keeps no new crash report for it.
//!
//! The test right-presses with the real pointer, as 731's does, and puts it
//! back where it was however the test ends. With `CEF_PATH` unset, or not
//! naming an unpacked distribution, it says why on its error output and
//! returns (`carried`).

#![cfg(target_os = "macos")]

mod carried;
mod pointer;

use alo_window::beside::Chromium;
use alo_window::cef_host::EXECUTABLE;
use alo_window::chrome::Chrome;
use alo_window::chrome_tab::Tab;
use alo_window::chrome_window;
use alo_window::window_server;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

/// The size the page is asked for at, in CSS pixels.
const SIZE: (u32, u32) = (900, 600);

/// A page with nothing on it to press but itself.
const PAGE: &str = "<!doctype html><title>A menu left open</title>\
    <p>Right-click anywhere.</p>";

/// The longest the window server is waited on to list what is wanted.
const AT_MOST: Duration = Duration::from_secs(10);

/// How long macOS is given to write a crash report once a process has
/// ended: its `ReportCrash` writes one within a few seconds.
const REPORTED_WITHIN: Duration = Duration::from_secs(10);

/// Run a script of JavaScript for Automation with `arguments` after `--`,
/// and say what it printed.
fn automation(script: &str, arguments: &[String]) -> Result<String, String> {
    let ran = Command::new("osascript")
        .args(["-l", "JavaScript", "-e", script, "--"])
        .args(arguments)
        .output()
        .map_err(|why| format!("osascript did not run: {why}"))?;
    if ran.status.success() {
        Ok(String::from_utf8_lossy(&ran.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&ran.stderr).into_owned())
    }
}

/// Bring the application of the process `pid` in front, as a person does
/// by clicking it in the Dock.
fn bring_in_front(pid: u32) -> Result<(), String> {
    automation(
        "function run(argv) { Application('System Events').processes\
           .whose({unixId: Number(argv[0])})[0].frontmost = true; }",
        &[pid.to_string()],
    )
    .map(drop)
}

/// Press and let go the mouse's right button at `x`, `y`, in the screen's
/// points, as a mouse does.
fn right_press_at(x: i64, y: i64) -> Result<(), String> {
    automation(
        "ObjC.import('CoreGraphics'); function run(argv) {\
           var at = {x: Number(argv[0]), y: Number(argv[1])};\
           [$.kCGEventMouseMoved, $.kCGEventRightMouseDown, $.kCGEventRightMouseUp]\
             .forEach(kind => { $.CGEventPost($.kCGHIDEventTap,\
               $.CGEventCreateMouseEvent($(), kind, at, $.kCGMouseButtonRight)); delay(0.05); }); }",
        &[x.to_string(), y.to_string()],
    )
    .map(drop)
}

/// Close the front window of the process `pid` by its close button, as a
/// person closes it.
fn close_as_a_person(pid: u32) -> Result<(), String> {
    automation(
        "function run(argv) { var process = Application('System Events').processes\
           .whose({unixId: Number(argv[0])})[0];\
         process.windows[0].buttons.whose({subrole: 'AXCloseButton'})[0].click(); }",
        &[pid.to_string()],
    )
    .map(drop)
}

/// How many menus — windows above the ordinary layer — the process `pid`
/// has on the screen once there are `wanted`, or when [`AT_MOST`] has
/// passed first.
fn menus_once(pid: u32, wanted: usize) -> Result<usize, String> {
    let until = Instant::now() + AT_MOST;
    loop {
        let menus = window_server::on_the_screen(pid)
            .map_err(|why| format!("{why:?}"))?
            .iter()
            .filter(|window| window.layer > 0)
            .count();
        if menus == wanted || Instant::now() > until {
            return Ok(menus);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The crash reports macOS keeps for the host's program: each one's path.
fn crash_reports() -> Result<BTreeSet<PathBuf>, String> {
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("no HOME")?);
    Ok(
        std::fs::read_dir(home.join("Library/Logs/DiagnosticReports"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(EXECUTABLE))
            .map(|entry| entry.path())
            .collect(),
    )
}

/// Whether macOS writes a crash report beside `before` within
/// [`REPORTED_WITHIN`]: the new ones.
fn reported_after(before: &BTreeSet<PathBuf>) -> Result<Vec<PathBuf>, String> {
    let until = Instant::now() + REPORTED_WITHIN;
    loop {
        let new: Vec<PathBuf> = crash_reports()?.difference(before).cloned().collect();
        if !new.is_empty() || Instant::now() > until {
            return Ok(new);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// How the page is closed with its menu open.
#[derive(Debug, Clone, Copy)]
enum Closing {
    /// The host ended by alo: `Browser.close`.
    Ended,
    /// The page closed from inside by alo first, then the host ended.
    ClosedFromInsideThenEnded,
    /// The window's close button, pressed by a person, then the host ended.
    ByItsCloseButton,
}

/// A shown host on the page, a context menu left open on it, closed as
/// `closing` says: how the host ended.
fn closed_under_a_menu(carried: &Chromium, closing: Closing) -> Result<(), String> {
    let mut chrome = Chrome::start_from(carried, true).map_err(|why| why.to_string())?;
    let (process, profile) = (chrome.process(), chrome.profile().to_path_buf());
    let crashed = crash_reports()?;
    {
        let mut tab = Tab::open_frozen(
            &mut chrome,
            "https://example.test/menu",
            PAGE.as_bytes().to_vec(),
            SIZE,
        )
        .map_err(|why| why.to_string())?;
        let placed = chrome_window::place(&mut tab, 120, 80).map_err(|why| why.to_string())?;
        let title_bar = placed.window.bounds.height - 600;
        bring_in_front(process)?;
        let (left, top) = (placed.window.bounds.left, placed.window.bounds.top);
        right_press_at(left + 450, top + title_bar + 200)?;
        assert_eq!(
            menus_once(process, 1)?,
            1,
            "{closing:?}: a context menu is shown"
        );
        match closing {
            Closing::Ended => {}
            Closing::ClosedFromInsideThenEnded => {
                chrome_window::close(&mut tab).map_err(|why| format!("{why:?}"))?;
            }
            Closing::ByItsCloseButton => close_as_a_person(process)?,
        }
    }
    let ended = chrome.close();
    let new = reported_after(&crashed)?;
    assert!(
        new.is_empty(),
        "{closing:?}: the host crashed, its report at {new:?}"
    );
    assert!(
        ended.is_some_and(|status| status.success()),
        "{closing:?}: the host ended {ended:?}, not in order"
    );
    assert_eq!(
        menus_once(process, 0)?,
        0,
        "{closing:?}: a menu outlived it"
    );
    assert!(!profile.exists(), "{closing:?}: {}", profile.display());
    eprintln!("{closing:?}: the host ended {ended:?}, no crash report");
    Ok(())
}

#[test]
fn a_page_closed_under_its_open_context_menu_ends_alos_cef_host_in_order() -> Result<(), String> {
    let Some(host) = carried::host("734")? else {
        return Ok(());
    };
    let _pointer = pointer::at().map(pointer::PutBack)?;
    for closing in [
        Closing::Ended,
        Closing::ClosedFromInsideThenEnded,
        Closing::ByItsCloseButton,
    ] {
        closed_under_a_menu(&host.chromium(), closing)?;
    }
    Ok(())
}
