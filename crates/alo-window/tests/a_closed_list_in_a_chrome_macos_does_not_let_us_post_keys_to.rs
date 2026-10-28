/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A closed list's option asked for by a browser macOS does not let post
//! keys: no menu is opened, the page hears nothing, and the answer says
//! where a person allows it (queue item 692, ADR 0072).
//!
//! The tests run in a process macOS already trusts, so this one makes a
//! process it does not: a **new application bundle**, sealed ad hoc as
//! `alo-bundle` seals `alo browser.app` and started by `open`, as ADR 0072
//! § 1 measured — under an identifier of its own, holding a copy of this
//! test binary as its program. Started with [`PLAYING`] set, the copy plays
//! the browser: it asks macOS whether it may post keys, starts a shown
//! Chrome, asks for an option of a closed list, reads the window server for
//! a menu and the page for what it heard, asks the posting script for an
//! Escape — which asks macOS again on its own — and writes each down. This
//! test reads what it wrote.
//!
//! If macOS has been told to trust that identifier, the copy says so first
//! and the test fails saying why, rather than passing on a walk it never
//! made.

#![cfg(target_os = "macos")]

use alo_agent::verb::{Target, Verb};
use alo_window::bundle_seal;
use alo_window::chrome::Chrome;
use alo_window::chrome_act::Unacted;
use alo_window::chrome_choose::Unwalked;
use alo_window::chrome_menu;
use alo_window::chrome_tab::Tab;
use alo_window::window_server;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// Set in the copy of this binary that plays the browser.
const PLAYING: &str = "ALO_TEST_PLAYS_A_BROWSER_NOT_ALLOWED_TO_POST_KEYS";

/// This test's name, which the copy is asked to run.
const NAME: &str = "a_closed_lists_menu_is_not_opened_by_a_browser_macos_does_not_let_post_keys";

/// The application's identifier: never `alo browser.app`'s, so nothing a
/// person allowed that application is lent to this one.
const IDENTIFIER: &str = "org.aloworld.test.not-allowed-to-post-keys";

/// What every line the copy writes down is read after, so that its own
/// test harness's lines, and the harness's words before its first, are
/// passed over.
const SAID: &str = "played:";

/// A closed list, and a line that writes down what it hears — as in
/// `a_closed_list_chosen_through_a_shown_chrome.rs`, whose walk to
/// *Elderberries* this would be.
const FRUIT: &str = r#"<!doctype html>
<html lang=en><title>Basket</title>
<p id=heard>Heard:</p>
<select aria-label="Fruit">
<option>Apples</option><option disabled>Bananas</option>
<optgroup label="Late"><option>Dates</option><option>Elderberries</option></optgroup>
</select>
<script>
const list = document.querySelector("select");
const heard = document.getElementById("heard");
for (const type of ["focus", "input", "change"]) {
  list.addEventListener(type, () => { heard.textContent += " " + type + ":" + list.value; });
}
</script>
"#;

/// The longest the copy is waited for. It starts one Chrome and acts
/// once, and every wait inside it is bounded: an instrument, not the claim.
const AT_MOST: Duration = Duration::from_secs(180);

/// The browser this binary plays when [`PLAYING`] is set: each line it
/// writes is the claim's evidence, and nothing it meets is panicked on.
fn play() {
    println!("{SAID} process {}", std::process::id());
    match chrome_menu::may_post() {
        Ok(allowed) => println!("{SAID} may post {allowed}"),
        Err(why) => println!("{SAID} unasked {why}"),
    }
    if let Err(why) = choose_elderberries() {
        println!("{SAID} failed {why}");
    }
    println!("{SAID} done");
}

/// Ask for *Elderberries* through a shown Chrome, and write down the
/// answer, any menu shown after it, and what the page heard.
fn choose_elderberries() -> Result<(), String> {
    let mut chrome = Chrome::start_shown().map_err(|why| why.to_string())?;
    let owner = chrome.process();
    let mut tab = Tab::open_frozen(
        &mut chrome,
        "https://example.test/fruit",
        FRUIT.as_bytes().to_vec(),
        (600, 500),
    )
    .map_err(|why| why.to_string())?;
    let before = window_server::on_the_screen(owner).map_err(|why| why.to_string())?;
    match tab.act(&Target::Named("Elderberries".to_owned()), &Verb::Activate) {
        Err(
            refused @ Unacted::Unwalked {
                why: Unwalked::NotAllowed,
                ..
            },
        ) => println!("{SAID} refused {refused}"),
        other => println!("{SAID} answered {other:?}"),
    }
    // A menu is shown a moment after the click that opens it: waited for
    // as long as the browser waits for one.
    let menu = chrome_menu::shown(owner, &before).map_err(|why| why.to_string())?;
    println!("{SAID} menu {menu:?}");
    // The posting script asks again before every key (ADR 0072 § 2.2): an
    // Escape asked for now, with no menu shown, is refused for the same
    // reason before any window is read.
    let now = window_server::on_the_screen(owner).map_err(|why| why.to_string())?;
    println!("{SAID} escape {:?}", chrome_menu::close_any(owner, &now));
    let read = tab.read().map_err(|why| why.to_string())?;
    let heard = read
        .nodes()
        .into_iter()
        .filter_map(|node| node.name.clone())
        .find(|name| name.starts_with("Heard:"))
        .ok_or_else(|| format!("nothing heard in {}", read.to_outline()))?;
    println!("{SAID} heard {heard}");
    Ok(())
}

/// A directory of this test's own, removed when it is done with.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A new application in `directory` whose program is a copy of this test
/// binary, sealed ad hoc.
fn application(directory: &Path) -> Result<PathBuf, String> {
    let application = directory.join("Not Allowed.app");
    let programs = application.join("Contents").join("MacOS");
    std::fs::create_dir_all(&programs).map_err(|why| why.to_string())?;
    let this = std::env::current_exe().map_err(|why| why.to_string())?;
    std::fs::copy(this, programs.join("played")).map_err(|why| why.to_string())?;
    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\"><dict>\n\
         <key>CFBundleExecutable</key><string>played</string>\n\
         <key>CFBundleIdentifier</key><string>{IDENTIFIER}</string>\n\
         <key>CFBundleName</key><string>Not Allowed</string>\n\
         <key>CFBundlePackageType</key><string>APPL</string>\n\
         <key>LSUIElement</key><true/>\n\
         </dict></plist>\n"
    );
    std::fs::write(application.join("Contents").join("Info.plist"), plist)
        .map_err(|why| why.to_string())?;
    bundle_seal::seal(&application, &[]).map_err(|why| why.to_string())?;
    Ok(application)
}

/// Start `application` by `open`, as the Finder would, with what it writes
/// to `written`; wait for it to end, and say what it wrote.
fn run(application: &Path, written: &Path) -> Result<String, String> {
    let mut open = Command::new("open")
        .arg("-W")
        .arg("-n")
        .arg("--stdout")
        .arg(written)
        .arg("--stderr")
        .arg(written)
        .arg("--env")
        .arg(format!("{PLAYING}=1"))
        .arg(application)
        .args(["--args", "--exact", NAME, "--nocapture", "--test-threads=1"])
        .spawn()
        .map_err(|why| why.to_string())?;
    let deadline = Instant::now() + AT_MOST;
    loop {
        if let Some(status) = open.try_wait().map_err(|why| why.to_string())? {
            let said = std::fs::read_to_string(written).map_err(|why| why.to_string())?;
            return if status.success() {
                Ok(said)
            } else {
                Err(format!("open ended {status}: {said}"))
            };
        }
        if Instant::now() >= deadline {
            let said = std::fs::read_to_string(written).unwrap_or_default();
            // The copy, by the process it wrote down, and `open` waiting on
            // it: neither outlives the test.
            if let Some(process) = said
                .lines()
                .find_map(|line| line.split_once(&format!("{SAID} process ")))
                .map(|(_, process)| process)
            {
                let _ = Command::new("kill").args(["-9", process]).status();
            }
            let _ = open.kill();
            let _ = open.wait();
            return Err(format!("the application ran past {AT_MOST:?}: {said}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn a_closed_lists_menu_is_not_opened_by_a_browser_macos_does_not_let_post_keys()
-> Result<(), String> {
    if std::env::var_os(PLAYING).is_some() {
        play();
        return Ok(());
    }
    let scratch =
        Scratch(std::env::temp_dir().join(format!("alo-not-allowed-test-{}", std::process::id())));
    let _ = std::fs::remove_dir_all(&scratch.0);
    std::fs::create_dir(&scratch.0).map_err(|why| why.to_string())?;
    let application = application(&scratch.0)?;
    let said = run(&application, &scratch.0.join("written.txt"))?;
    let lines: Vec<&str> = said
        .lines()
        .filter_map(|line| line.split_once(SAID).map(|(_, said)| said))
        .map(str::trim)
        .collect();
    assert!(
        lines.contains(&"may post false"),
        "macOS lets {IDENTIFIER} post keys — it was allowed in System Settings, \
         so this test cannot see a browser that is not: {said}"
    );
    let refused = lines
        .iter()
        .find_map(|line| line.strip_prefix("refused "))
        .ok_or_else(|| format!("not refused as not allowed: {said}"))?;
    assert!(
        refused.ends_with(
            "was not chosen in its menu: macOS does not let this browser post keys to Chrome, \
             so its menu was not opened; a person allows it in System Settings, \
             under Privacy & Security, Accessibility, for the application this browser runs in"
        ),
        "{refused}"
    );
    assert!(lines.contains(&"menu None"), "no menu was shown: {said}");
    assert!(
        lines.contains(&"escape Ok(NotAllowed)"),
        "the posting script refuses on its own: {said}"
    );
    assert!(
        lines.contains(&"heard Heard:"),
        "the page heard nothing — not even the focus a click gives: {said}"
    );
    assert!(lines.contains(&"done"), "{said}");
    Ok(())
}
