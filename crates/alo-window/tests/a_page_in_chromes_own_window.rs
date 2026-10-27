/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a person looks at on the product path: a page in the machine's
//! Chrome, drawn in a window of Chrome's own on the screen and placed there
//! by this process (queue item 627, ADR 0061).
//!
//! The Chrome is shown, not headless, in `--app` mode. A frozen page is
//! opened in its window, and the window is sized so the page is laid out at
//! the size asked — the same size a headless Chrome is told to pretend —
//! and put where it is asked to be. Each is asserted in numbers: the
//! window's bounds on the screen as Chrome reads them, the page's size
//! inside it, and the document's box in the tree an agent reads. Where the
//! system overrules a place, what is answered is where the window went.
//!
//! Chrome's word for where its window is is not the only witness. On macOS
//! the window server is asked too, through `CGWindowListCopyWindowInfo` in
//! `osascript`: the Chrome this test started has one window on the screen,
//! titled by the page and opaque.
//!
//! And where: the window server lists it at Chrome's bounds exactly. With
//! Stage Manager on, a window that is not in front is shrunk into its strip
//! — listed at 16,773 and 97×115 while Chrome says 160,120 and 900×632 —
//! and the window that sends it there is the one brought in front after it
//! (item 643, `a_chrome_window_brought_in_front_of_another`). So this test
//! shows its window with no other Chrome's beside it (`.config/nextest.toml`)
//! and waits, at most [`AT_MOST`], for the window server to finish moving it.

use alo_agent::Outcome;
use alo_agent::verb::{Target, Verb};
use alo_window::chrome::Chrome;
use alo_window::chrome_protocol::WindowState;
use alo_window::chrome_tab::{Tab, Unopened};
use alo_window::chrome_window::{self, LARGEST, Placed, Unplaced};
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};

/// Where the page says it was taken from. Frozen: nothing reaches a server.
const ADDRESS: &str = "https://example.com/window.html";

/// A heading, and a link to a part of the page far below it.
const PAGE: &str = "<!doctype html><title>Window</title><h1>Window</h1>\
                    <a href=\"#below\">Further down</a>\
                    <div style=\"height:3000px\"></div><h2 id=\"below\">Below</h2>";

/// The size the page is asked for at, in CSS pixels.
const SIZE: (u32, u32) = (900, 600);

/// The longest the window server is waited on to list the window where
/// Chrome put it. Measured on 2026-10-27, a window brought out of Stage
/// Manager's strip was listed at its bounds at the first reading after, and
/// one sent into it had shrunk within 700 ms.
#[cfg(target_os = "macos")]
const AT_MOST: Duration = Duration::from_secs(10);

/// The windows the window server lists for the process `pid` that are on
/// the screen, a line each: title, then bounds as [`bounds`] writes them,
/// then opacity.
#[cfg(target_os = "macos")]
fn on_the_screen(pid: u32) -> Result<Vec<String>, String> {
    const LISTED: &str = "ObjC.import('CoreGraphics');\
        var all = ObjC.deepUnwrap(ObjC.castRefToObject(\
          $.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly, 0)));\
        all.filter(w => w.kCGWindowOwnerPID == THE_PROCESS && w.kCGWindowLayer == 0)\
          .map(w => (w.kCGWindowName || '') + ' ' + w.kCGWindowBounds.X + ','\
            + w.kCGWindowBounds.Y + ' ' + w.kCGWindowBounds.Width + '×'\
            + w.kCGWindowBounds.Height + ' ' + w.kCGWindowAlpha).join('\\n')";
    let listed = std::process::Command::new("osascript")
        .args([
            "-l",
            "JavaScript",
            "-e",
            &LISTED.replace("THE_PROCESS", &pid.to_string()),
        ])
        .output()
        .map_err(|why| format!("osascript did not run: {why}"))?;
    if !listed.status.success() {
        return Err(String::from_utf8_lossy(&listed.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&listed.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

/// What the window server lists for the process `pid` once it is `wanted`,
/// or what it listed last when [`AT_MOST`] has passed first.
#[cfg(target_os = "macos")]
fn once_listed(pid: u32, wanted: &[String]) -> Result<Vec<String>, String> {
    let until = Instant::now() + AT_MOST;
    loop {
        let listed = on_the_screen(pid)?;
        if listed.as_slice() == wanted || Instant::now() > until {
            return Ok(listed);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A page's size as text, so it is compared whole rather than as floats.
fn size(placed: &Placed) -> String {
    format!("{}×{}", placed.page.0, placed.page.1)
}

/// A window's bounds as text: left, top, width, height.
fn bounds(placed: &Placed) -> String {
    let at = placed.window.bounds;
    format!("{},{} {}×{}", at.left, at.top, at.width, at.height)
}

#[test]
fn a_page_is_laid_out_in_its_window_at_the_size_asked_and_placed() -> Result<(), String> {
    let mut chrome = Chrome::start_shown().map_err(|why| why.to_string())?;
    assert!(chrome.is_shown());
    #[cfg(target_os = "macos")]
    let process = chrome.process();
    let mut tab = Tab::open_frozen(&mut chrome, ADDRESS, PAGE.as_bytes().to_vec(), SIZE)
        .map_err(|why| why.to_string())?;

    // The page is the size asked, and its window is that and a title bar.
    let fitted = chrome_window::placed(&mut tab).map_err(|why| why.to_string())?;
    assert_eq!(size(&fitted), "900×600");
    assert_eq!(fitted.window.state, WindowState::Normal);
    assert_eq!(
        fitted.window.bounds.width, 900,
        "an app window has no sides"
    );
    let title_bar = fitted.window.bounds.height - 600;
    assert!(
        (1..=64).contains(&title_bar),
        "a title bar and nothing else above the page: {}",
        bounds(&fitted)
    );

    // Put where it is asked, at the size it was: exactly.
    let placed = chrome_window::place(&mut tab, 120, 80).map_err(|why| why.to_string())?;
    assert_eq!(bounds(&placed), format!("120,80 900×{}", 600 + title_bar));
    assert_eq!(size(&placed), "900×600");
    assert_eq!(placed.window.id, fitted.window.id, "the same window, moved");
    let moved = chrome_window::place(&mut tab, 160, 120).map_err(|why| why.to_string())?;
    assert_eq!(bounds(&moved), format!("160,120 900×{}", 600 + title_bar));
    assert_eq!(
        chrome_window::placed(&mut tab).map_err(|why| why.to_string())?,
        moved,
        "read again, it is where it was said to be"
    );
    #[cfg(target_os = "macos")]
    {
        // One window on the screen, titled by the page, opaque, and where
        // Chrome says it is: in front, not in Stage Manager's strip.
        let wanted = vec![format!("Window {} 1", bounds(&moved))];
        assert_eq!(
            once_listed(process, &wanted)?,
            wanted,
            "the window server's own word"
        );
    }

    // The tree an agent reads is the page at that size, in numbers.
    let tree = tab.read().map_err(|why| why.to_string())?;
    let root = tree.root.ok_or("the page has no tree")?;
    assert_eq!(
        format!(
            "{} {} {}×{}",
            root.rect.origin.x, root.rect.origin.y, root.rect.size.width, root.rect.size.height
        ),
        "0 0 900×600"
    );

    // And a verb carried out in the window is carried out as headless.
    let followed = tab
        .act(&Target::Named("Further down".to_owned()), &Verb::Activate)
        .map_err(|why| why.to_string())?;
    assert!(
        matches!(&followed, Outcome::Followed { to, .. } if to == "#below"),
        "{followed:?}"
    );
    Ok(())
}

#[test]
fn a_place_the_system_overrules_is_answered_with_where_the_window_went() -> Result<(), String> {
    let mut chrome = Chrome::start_shown().map_err(|why| why.to_string())?;
    let mut tab = Tab::open_frozen(&mut chrome, ADDRESS, PAGE.as_bytes().to_vec(), SIZE)
        .map_err(|why| why.to_string())?;
    // Far above and to the left of every screen: macOS pulls a window back
    // under the menu bar and onto the screen.
    let went = chrome_window::place(&mut tab, -5000, -5000).map_err(|why| why.to_string())?;
    assert!(
        went.window.bounds.left > -5000 && went.window.bounds.top >= 0,
        "pulled back onto the screen: {}",
        bounds(&went)
    );
    assert_eq!(
        chrome_window::placed(&mut tab).map_err(|why| why.to_string())?,
        went,
        "what is answered is where it is, not where it was sent"
    );
    assert_eq!(size(&went), "900×600", "moved, not resized");
    Ok(())
}

#[test]
fn a_size_nobody_could_look_at_is_refused_before_chrome_hears_of_it() -> Result<(), String> {
    let mut chrome = Chrome::start_shown().map_err(|why| why.to_string())?;
    let written = chrome.written();
    match Tab::open_frozen(&mut chrome, ADDRESS, PAGE.as_bytes().to_vec(), (0, 600)) {
        Err(Unopened::Unfitted(why)) => assert_eq!(
            *why,
            Unplaced::Unsized {
                width: 0,
                height: 600
            }
        ),
        Err(other) => return Err(format!("not refused as unsized: {other}")),
        Ok(_) => return Err("a page of no width was opened".to_owned()),
    }
    assert!(chrome.written() > written, "the tab was begun");
    drop(chrome);

    let mut again = Chrome::start_shown().map_err(|why| why.to_string())?;
    let mut tab = Tab::open_frozen(&mut again, ADDRESS, PAGE.as_bytes().to_vec(), SIZE)
        .map_err(|why| why.to_string())?;
    for (width, height) in [(LARGEST + 1, 600), (900, 0), (u32::MAX, u32::MAX)] {
        assert_eq!(
            chrome_window::fit(&mut tab, width, height),
            Err(Unplaced::Unsized { width, height })
        );
    }
    assert_eq!(
        size(&chrome_window::placed(&mut tab).map_err(|why| why.to_string())?),
        "900×600",
        "left as it was"
    );
    Ok(())
}

#[test]
fn a_headless_chrome_has_no_window_to_place() -> Result<(), String> {
    let mut chrome = Chrome::start().map_err(|why| why.to_string())?;
    assert!(!chrome.is_shown());
    let mut tab = Tab::open_frozen(&mut chrome, ADDRESS, PAGE.as_bytes().to_vec(), SIZE)
        .map_err(|why| why.to_string())?;
    assert_eq!(chrome_window::placed(&mut tab), Err(Unplaced::Headless));
    assert_eq!(
        chrome_window::place(&mut tab, 0, 0),
        Err(Unplaced::Headless)
    );
    assert_eq!(
        chrome_window::fit(&mut tab, 900, 600),
        Err(Unplaced::Headless)
    );
    assert_eq!(
        chrome_window::bring_to_front(&mut tab),
        Err(Unplaced::Headless)
    );
    Ok(())
}

#[test]
fn a_shown_chrome_ends_with_its_handle() -> Result<(), String> {
    let chrome = Chrome::start_shown().map_err(|why| why.to_string())?;
    let (process, profile) = (chrome.process(), chrome.profile().to_path_buf());
    drop(chrome);
    let alive = std::process::Command::new("kill")
        .args(["-0", &process.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|why| why.to_string())?;
    assert!(!alive.success(), "Chrome {process} is still running");
    assert!(!profile.exists(), "{} is left", profile.display());
    Ok(())
}
