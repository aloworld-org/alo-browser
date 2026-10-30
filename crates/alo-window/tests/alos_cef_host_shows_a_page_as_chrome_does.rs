/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's own CEF host shown, a page in a window of its own as Chrome's
//! `--app` window is (queue item 727, ADR 0079 § 3.2).
//!
//! `Chrome::start_from(&Chromium::Carried(…), true)` — the very call the
//! application makes to show a page with the Chromium it carries — starts
//! the host with Chrome's shown switches, `--app=data:,` among them, and
//! the host opens that page in a window of its own (`window.rs`). Then
//! everything is as for Chrome's own window
//! (`tests/a_page_in_chromes_own_window.rs`): the window's page is the tab,
//! opened on a page of this test's own server; the window is sized so the
//! page is laid out at the size asked and put where it is asked, both by
//! Chromium's word (`Browser.getWindowForTarget`) and by the window
//! server's, which lists one window of the host's, titled by the page,
//! opaque, at exactly those bounds; and the tree an agent reads is that
//! page at that size.
//!
//! **A person's keys and clicks reach the page** (727): the host is
//! brought in front as a person brings an application in front, keys are
//! typed by `System Events`, and a press is posted where a mouse posts
//! one, on a button of the page — and the page's title, which it changes
//! as it hears each, is read back from the window server's list.
//!
//! **Closed by alo, and by a person.** A host shown and ended by alo the
//! way alo ends every Chrome it started — its handle dropped,
//! `Browser.close` — takes its window with it. A host whose window a person
//! closes by its close button closes its page, and alo hears the page go
//! (`chrome_frame::gone`) and the window leave the screen. Each host is then
//! gone, its profile removed, nothing of the bundle left running, **no
//! crash report** made, and nothing written outside the profile but what
//! macOS itself keeps for a program that draws with the GPU, which is said
//! and removed (queue item 729).
//!
//! **What crashed it before it had an application object of its own**
//! (queue item 731, ADR 0080), and now leaves it running: a context menu
//! opened by a person's right click and dismissed with Escape; a closed
//! list's menu, an option chosen in it by an agent (`Activate`) and heard
//! by the page; and a page closed from inside — by alo
//! (`chrome_window::close`, `Target.closeTarget`) and by the page's own
//! `window.close()`. alo hears each page go, and the host lives on.
//!
//! **Its menu bar and ⌘Q** (queue items 731, 732): the menu bar holds
//! *alo*, *Edit* and *Window*; a person's ⌘A, ⌘C and ⌘V copy what was
//! typed and paste it again; and ⌘Q ends the host **in order** — the pipe
//! closes, the process ends, and Chromium's own clean-up has removed its
//! process singleton's directory from the profile, which the host's
//! earlier exit, inside `AppKit`'s `-terminate:`, left behind — both with
//! the page's window in front and with no window at all. A key with ⌘ is
//! typed only once the host is checked to be the application in front, in
//! the same script, so that a ⌘Q never reaches another application.
//!
//! Not asserted, because it still crashes the host (queue item 734): a
//! page closed while its context menu is open.
//!
//! # When there is no CEF distribution here
//!
//! As `tests/alos_cef_host_driven_as_chrome.rs`: with `CEF_PATH` unset, or
//! not naming an unpacked distribution, it says why on its error output
//! and returns.

#![cfg(target_os = "macos")]

use alo_agent::Outcome;
use alo_agent::verb::{Target, Verb};
use alo_box::BoxId;
use alo_url::Url;
use alo_window::beside::Chromium;
use alo_window::cef_host::{self, EXECUTABLE, FRAMEWORK, IDENTIFIER};
use alo_window::chrome::Chrome;
use alo_window::chrome_frame::{self, Closed};
use alo_window::chrome_protocol::WindowState;
use alo_window::chrome_tab::Tab;
use alo_window::chrome_window::{self, Placed};
use alo_window::window_server;
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// The size the page is asked for at, in CSS pixels.
const SIZE: (u32, u32) = (900, 600);

/// What the server's page is titled and says.
const SAID: &str = "Shown by the Chromium alo carries";

/// The page: a field that titles the page with what was typed into it, and
/// a button, low and wide, that titles it as pressed.
const PAGE: &str = "<!doctype html><title>Shown by the Chromium alo carries</title>\
    <h1>Shown by the Chromium alo carries</h1>\
    <input aria-label=\"Words\" autofocus \
      oninput=\"document.title = 'Typed ' + this.value\">\
    <button style=\"position: fixed; left: 0; top: 300px; width: 900px; height: 300px\" \
      onclick=\"document.title = 'Pressed by ' + (event.isTrusted ? 'a person' : 'a script')\">\
      Press</button>";

/// A page with a closed list, whose change it writes down and titles
/// itself with, and a button, low and wide, that closes the page from
/// inside.
const MENUS: &str = "<!doctype html><title>Menus</title>\
    <p id=heard>Heard:</p>\
    <select aria-label=\"Fruit\"><option>Apples</option><option>Dates</option>\
      <option>Honeydew</option></select>\
    <button style=\"position: fixed; left: 0; top: 300px; width: 900px; height: 300px\" \
      onclick=\"window.close()\">Close</button>\
    <script>const list = document.querySelector('select');\
      list.addEventListener('change', (event) => {\
        document.getElementById('heard').textContent += ' change:' + list.value\
          + (event.isTrusted ? '' : '(by script)'); });</script>";

/// Escape and the right arrow, as macOS numbers them.
const KEY_ESCAPE: u16 = 53;
const KEY_RIGHT: u16 = 124;

/// The longest the window server is waited on to list what is wanted.
const AT_MOST: Duration = Duration::from_secs(10);

/// The CEF distribution to build against, or why there is none.
fn distribution() -> Result<PathBuf, String> {
    let Some(named) = std::env::var_os("CEF_PATH") else {
        return Err("CEF_PATH is not set".to_owned());
    };
    let named = PathBuf::from(named);
    if named.join(FRAMEWORK).is_dir() && named.join("archive.json").is_file() {
        Ok(named)
    } else {
        Err(format!(
            "CEF_PATH names {}, which holds no unpacked CEF distribution",
            named.display()
        ))
    }
}

/// The host built against the distribution at `cef`: its program.
fn built(cef: &Path) -> Result<PathBuf, String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = repository.join("target/alo-cef-host");
    let status = Command::new(env!("CARGO"))
        .arg("build")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(repository.join("crates/alo-cef-host/Cargo.toml"))
        .arg("--target-dir")
        .arg(&target)
        .env("CEF_PATH", cef)
        .status()
        .map_err(|why| format!("cargo did not start: {why}"))?;
    if !status.success() {
        return Err(format!("the host did not build: {status}"));
    }
    Ok(target.join("debug/alo-cef-host"))
}

/// A server on this machine that answers every request with [`PAGE`]: its
/// port.
fn serve() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|why| why.to_string())?;
    let port = listener.local_addr().map_err(|why| why.to_string())?.port();
    std::thread::spawn(move || {
        for socket in listener.incoming() {
            let Ok(mut socket) = socket else {
                return;
            };
            std::thread::spawn(move || {
                let _ = socket.set_read_timeout(Some(Duration::from_secs(5)));
                let mut asked = Vec::new();
                let mut block = [0u8; 1024];
                while let Ok(got) = socket.read(&mut block) {
                    asked.extend_from_slice(block.get(..got).unwrap_or_default());
                    if got == 0 || asked.windows(4).any(|end| end == b"\r\n\r\n") {
                        break;
                    }
                }
                // A connection Chromium opened ahead of need asked for
                // nothing.
                if asked.is_empty() {
                    return;
                }
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nCache-Control: no-store\r\n\
                     Connection: close\r\n\r\n{PAGE}",
                    PAGE.len()
                );
                let _ = socket.write_all(reply.as_bytes());
            });
        }
    });
    Ok(port)
}

/// Run a script of JavaScript for Automation with `arguments` after `--`,
/// so that none is read as one of `osascript`'s own options, and say what
/// it printed.
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

/// The windows the window server lists for the process `pid` that are on
/// the screen, a line each: title, then bounds as [`bounds`] writes them,
/// then opacity.
fn on_the_screen(pid: u32) -> Result<Vec<String>, String> {
    const LISTED: &str = "ObjC.import('CoreGraphics'); function run(argv) {\
        var all = ObjC.deepUnwrap(ObjC.castRefToObject(\
          $.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly, 0)));\
        return all.filter(w => w.kCGWindowOwnerPID == Number(argv[0]) && w.kCGWindowLayer == 0)\
          .map(w => (w.kCGWindowName || '') + ' ' + w.kCGWindowBounds.X + ','\
            + w.kCGWindowBounds.Y + ' ' + w.kCGWindowBounds.Width + '×'\
            + w.kCGWindowBounds.Height + ' ' + w.kCGWindowAlpha).join('\\n'); }";
    Ok(automation(LISTED, &[pid.to_string()])?
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

/// What the window server lists for the process `pid` once it is `wanted`,
/// or what it listed last when [`AT_MOST`] has passed first.
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

/// Type `keys` into whatever application is in front, as a person types.
fn type_keys(keys: &str) -> Result<(), String> {
    automation(
        "function run(argv) { Application('System Events').keystroke(argv[0]); }",
        &[keys.to_owned()],
    )
    .map(drop)
}

/// Press and let go the mouse's button at `x`, `y`, in the screen's points,
/// as a mouse does.
fn press_at(x: i64, y: i64) -> Result<(), String> {
    automation(
        "ObjC.import('CoreGraphics'); function run(argv) {\
           var at = {x: Number(argv[0]), y: Number(argv[1])};\
           [$.kCGEventMouseMoved, $.kCGEventLeftMouseDown, $.kCGEventLeftMouseUp]\
             .forEach(kind => { $.CGEventPost($.kCGHIDEventTap,\
               $.CGEventCreateMouseEvent($(), kind, at, $.kCGMouseButtonLeft)); delay(0.05); }); }",
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

/// A page's size as text, so it is compared whole rather than as floats.
fn size(placed: &Placed) -> String {
    format!("{}×{}", placed.page.0, placed.page.1)
}

/// A window's bounds as text: left, top, width, height.
fn bounds(placed: &Placed) -> String {
    let at = placed.window.bounds;
    format!("{},{} {}×{}", at.left, at.top, at.width, at.height)
}

/// How alo hears the window of `tab` gone, waiting at most [`AT_MOST`].
fn heard_gone(tab: &mut Tab<'_>) -> Result<Option<Closed>, String> {
    let until = Instant::now() + AT_MOST;
    loop {
        let gone = chrome_frame::gone(tab).map_err(|why| why.to_string())?;
        if gone.is_some() || Instant::now() > until {
            return Ok(gone);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The system's directory of this person's `name` (`DARWIN_USER_TEMP_DIR`,
/// `DARWIN_USER_CACHE_DIR`).
fn per_person(name: &str) -> Result<PathBuf, String> {
    let said = Command::new("/usr/bin/getconf")
        .arg(name)
        .output()
        .map_err(|why| why.to_string())?;
    let said = String::from_utf8_lossy(&said.stdout).trim().to_owned();
    if said.is_empty() {
        return Err(format!("getconf {name} said nothing"));
    }
    Ok(PathBuf::from(said))
}

/// Every place outside the profile a CEF program was seen to write on this
/// Mac, with what is there now, as `alos_cef_host_driven_as_chrome.rs`
/// reads them: each entry's path.
fn written_outside() -> Result<BTreeSet<PathBuf>, String> {
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("no HOME")?);
    let temporary = per_person("DARWIN_USER_TEMP_DIR")?;
    let clones = temporary
        .parent()
        .ok_or("the temporary directory is the root")?
        .join("X");
    let mut found = BTreeSet::new();
    for directory in [
        temporary,
        clones,
        per_person("DARWIN_USER_CACHE_DIR")?,
        home.join("Library/Saved Application State"),
    ] {
        for entry in std::fs::read_dir(&directory)
            .into_iter()
            .flatten()
            .flatten()
        {
            if entry.file_name().to_string_lossy().starts_with(IDENTIFIER) {
                found.insert(entry.path());
            }
        }
    }
    for cefs in [
        home.join("Library/Application Support/CEF"),
        home.join("Library/Caches/CEF"),
    ] {
        if cefs.exists() {
            found.insert(cefs);
        }
    }
    let working = std::env::current_dir().map_err(|why| why.to_string())?;
    for entry in std::fs::read_dir(&working)
        .map_err(|why| why.to_string())?
        .flatten()
    {
        found.insert(entry.path());
    }
    Ok(found)
}

/// Whether `path` is what macOS itself keeps for a program that draws with
/// the GPU, outside the profile and not Chromium's own writing: a directory
/// in the per-person cache directory named by the host's identifier, or a
/// helper's, holding only Metal's and the GPU archiver's caches
/// (`com.apple.metal…`, `com.apple.gpuarchiver`), measured on this Mac with
/// CEF 156.0.14 shown. Whether alo keeps even that is queue item 729.
fn is_macos_gpu_cache(path: &Path) -> bool {
    let Ok(caches) = per_person("DARWIN_USER_CACHE_DIR") else {
        return false;
    };
    let Ok(held) = std::fs::read_dir(path) else {
        return false;
    };
    path.parent() == Some(caches.as_path())
        && held.flatten().all(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with("com.apple.metal") || name == "com.apple.gpuarchiver"
        })
}

/// Whether any process is running a program inside `application`.
fn running_inside(application: &Path) -> bool {
    Command::new("/usr/bin/pgrep")
        .arg("-f")
        .arg(application)
        .output()
        .is_ok_and(|found| !found.stdout.is_empty())
}

/// Whether the process `pid` is still running.
fn is_running(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
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

/// Press and let go the key `key` in the process `pid` alone — posted to
/// that process: a menu's Escape.
fn press_key_in(pid: u32, key: u16) -> Result<(), String> {
    automation(
        "ObjC.import('CoreGraphics'); function run(argv) {\
           [true, false].forEach(down => { $.CGEventPostToPid(Number(argv[0]),\
             $.CGEventCreateKeyboardEvent($(), Number(argv[1]), down)); delay(0.05); }); }",
        &[pid.to_string(), key.to_string()],
    )
    .map(drop)
}

/// Type `key` as a person does — with ⌘ held when `command` — **only if**
/// the process `pid` is the application in front, checked in the same
/// script just before: a key posted to one process is not read as a
/// shortcut (measured: ⌘Q posted to the host's process did nothing), so
/// these go where a person's keys go, and a ⌘Q must never reach another
/// application that happens to be in front.
fn type_key_in_front(pid: u32, key: Key, command: bool) -> Result<(), String> {
    let (how, which) = match key {
        Key::Letter(letter) => ("letter", letter.to_string()),
        Key::Code(code) => ("code", code.to_string()),
    };
    automation(
        "function run(argv) { var events = Application('System Events');\
           var front = events.processes.whose({frontmost: true})[0].unixId();\
           if (front !== Number(argv[0])) { throw new Error('not in front: ' + front); }\
           var held = argv[3] === 'command' ? {using: 'command down'} : {};\
           if (argv[1] === 'letter') { events.keystroke(argv[2], held); }\
           else { events.keyCode(Number(argv[2]), held); } }",
        &[
            pid.to_string(),
            how.to_owned(),
            which,
            if command { "command" } else { "plain" }.to_owned(),
        ],
    )
    .map(drop)
}

/// A key [`type_key_in_front`] types: a letter, or a key by macOS's number.
#[derive(Debug, Clone, Copy)]
enum Key {
    /// A letter, as typed.
    Letter(char),
    /// A key by its number.
    Code(u16),
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

/// Where the pointer is, in the screen's points.
fn pointer() -> Result<(f64, f64), String> {
    let at = automation(
        "ObjC.import('CoreGraphics');\
         var at = $.CGEventGetLocation($.CGEventCreate($())); at.x + ' ' + at.y",
        &[],
    )?;
    let (x, y) = at.split_once(' ').ok_or(format!("no pointer in {at:?}"))?;
    let number = |text: &str| text.parse::<f64>().map_err(|why| why.to_string());
    Ok((number(x)?, number(y)?))
}

/// Where the pointer was, put back there when this is dropped.
struct PointerPutBack((f64, f64));

impl Drop for PointerPutBack {
    fn drop(&mut self) {
        if let Err(why) = move_pointer(self.0) {
            eprintln!("the pointer could not be put back: {why}");
        }
    }
}

/// Move the pointer to `x`, `y`, in the screen's points, as a mouse does.
fn move_pointer((x, y): (f64, f64)) -> Result<(), String> {
    automation(
        "ObjC.import('CoreGraphics'); function run(argv) {\
           $.CGEventPost($.kCGHIDEventTap, $.CGEventCreateMouseEvent($(),\
             $.kCGEventMouseMoved, {x: Number(argv[0]), y: Number(argv[1])},\
             $.kCGMouseButtonLeft)); }",
        &[x.to_string(), y.to_string()],
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

/// The names of the menu bar's items of the process `pid`, and of the items
/// in its *Edit* menu, as `System Events` reads them.
fn menu_bar(pid: u32) -> Result<String, String> {
    automation(
        "function run(argv) { var bar = Application('System Events').processes\
           .whose({unixId: Number(argv[0])})[0].menuBars[0];\
         return bar.menuBarItems.name().join(', ') + ' | '\
           + bar.menuBarItems.byName('Edit').menus[0].menuItems.name().join(', ') + ' | '\
           + bar.menuBarItems.byName('Window').menus[0].menuItems.name().join(', '); }",
        &[pid.to_string()],
    )
}

/// Whether the process `pid` has ended — gone, or ended and not yet
/// waited for by the handle that started it.
fn has_ended(pid: u32) -> bool {
    Command::new("/bin/ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .is_ok_and(|said| {
            let state = String::from_utf8_lossy(&said.stdout);
            state.trim().is_empty() || state.trim_start().starts_with('Z')
        })
}

/// Wait at most [`AT_MOST`] for the process `pid` to end: whether it did.
fn ends(pid: u32) -> bool {
    let until = Instant::now() + AT_MOST;
    while !has_ended(pid) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    has_ended(pid)
}

/// The temporary directories in the profile `profile` (`switches.rs` in
/// the host puts Chromium's there) that hold Chromium's process singleton's
/// socket: each one's name. Chromium's orderly clean-up removes them; an
/// exit that skips it — `AppKit`'s own `-terminate:`, or a crash — leaves
/// them (measured). Its URL fetcher's directory is not one: it was left
/// after an orderly exit too.
fn singletons(profile: &Path) -> Vec<String> {
    std::fs::read_dir(profile.join("tmp"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().join("SingletonSocket").exists())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

/// The node an agent names `name` by, on the page as it is now.
fn id_of(tab: &mut Tab<'_>, name: &str) -> Result<BoxId, String> {
    let read = tab.read().map_err(|why| why.to_string())?;
    read.nodes()
        .into_iter()
        .find(|node| node.name.as_deref() == Some(name))
        .map(|node| node.id)
        .ok_or_else(|| format!("no {name} in {}", read.to_outline()))
}

/// What the page has written down that it heard.
fn heard(tab: &mut Tab<'_>) -> Result<String, String> {
    let read = tab.read().map_err(|why| why.to_string())?;
    read.nodes()
        .into_iter()
        .filter_map(|node| node.name.clone())
        .find(|name| name.starts_with("Heard:"))
        .ok_or_else(|| format!("nothing heard in {}", read.to_outline()))
}

/// The menus page in a shown host, placed at 120,80 and in front: the tab,
/// and the window's bounds and title bar's height.
fn menus_page(chrome: &mut Chrome) -> Result<(Tab<'_>, Placed, i64), String> {
    let process = chrome.process();
    let mut tab = Tab::open_frozen(
        chrome,
        "https://example.test/menus",
        MENUS.as_bytes().to_vec(),
        SIZE,
    )
    .map_err(|why| why.to_string())?;
    let placed = chrome_window::place(&mut tab, 120, 80).map_err(|why| why.to_string())?;
    let title_bar = placed.window.bounds.height - 600;
    bring_in_front(process)?;
    Ok((tab, placed, title_bar))
}

/// Shown on the menus page: a context menu opened by a person and
/// dismissed, an option of a closed list chosen through its menu, and the
/// page closed by alo from inside — the host running through each, then
/// ended by alo.
fn through_menus_then_closed_from_inside(carried: &Path) -> Result<(), String> {
    let profile;
    let process;
    {
        let mut chrome = Chrome::start_from(&Chromium::Carried(carried.to_path_buf()), true)
            .map_err(|why| why.to_string())?;
        (process, profile) = (chrome.process(), chrome.profile().to_path_buf());
        let (mut tab, placed, title_bar) = menus_page(&mut chrome)?;
        let (left, top) = (placed.window.bounds.left, placed.window.bounds.top);
        let was = pointer()?;

        // A context menu, opened by a person's right click on the page and
        // dismissed with Escape: the host lives on, and the page answers.
        right_press_at(left + 450, top + title_bar + 200)?;
        assert_eq!(menus_once(process, 1)?, 1, "a context menu is shown");
        press_key_in(process, KEY_ESCAPE)?;
        assert_eq!(menus_once(process, 0)?, 0, "the context menu is gone");
        assert!(!has_ended(process), "the host lived through a context menu");
        assert_eq!(heard(&mut tab)?, "Heard:");

        // An option of the closed list chosen in its menu, the pointer at
        // the top of the window first, where no menu of the page opens.
        #[expect(
            clippy::cast_precision_loss,
            reason = "a window's bounds are a few thousand points"
        )]
        move_pointer(((left + 860) as f64, (top + 8) as f64))?;
        let wanted = Outcome::Chosen {
            node: id_of(&mut tab, "Dates")?,
            list: id_of(&mut tab, "Fruit")?,
            chosen: true,
        };
        let chosen = tab.act(&Target::Named("Dates".to_owned()), &Verb::Activate);
        move_pointer(was)?;
        assert_eq!(chosen, Ok(wanted), "chosen through the list's menu");
        assert_eq!(heard(&mut tab)?, "Heard: change:Dates");
        assert!(!has_ended(process), "the host lived through a list's menu");

        // Closed from inside by alo: the page goes, alo hears it, and the
        // window leaves the screen; the host lives on.
        chrome_window::close(&mut tab).map_err(|why| format!("{why:?}"))?;
        assert_eq!(heard_gone(&mut tab)?, Some(Closed::Page));
        assert_eq!(once_listed(process, &[])?, Vec::<String>::new());
        assert!(
            !has_ended(process),
            "the host lived through its page closed"
        );
    }
    assert!(ends(process), "the host {process} outlived its handle");
    assert!(!profile.exists(), "{}", profile.display());
    Ok(())
}

/// Shown on the menus page, closed by the page's own `window.close()`, and
/// then quit with ⌘Q with no window left: ended in order.
fn closed_by_its_page_then_quit(carried: &Path) -> Result<(), String> {
    let mut chrome = Chrome::start_from(&Chromium::Carried(carried.to_path_buf()), true)
        .map_err(|why| why.to_string())?;
    let (process, profile) = (chrome.process(), chrome.profile().to_path_buf());
    let (mut tab, placed, title_bar) = menus_page(&mut chrome)?;
    let (left, top) = (placed.window.bounds.left, placed.window.bounds.top);

    // A person presses the page's button, whose script closes the page.
    press_at(left + 450, top + title_bar + 450)?;
    assert_eq!(heard_gone(&mut tab)?, Some(Closed::Page));
    assert_eq!(once_listed(process, &[])?, Vec::<String>::new());
    assert!(!has_ended(process), "the host lived through window.close()");
    drop(tab);

    // ⌘Q with no window: the menu bar's Quit, and the host ends in order.
    bring_in_front(process)?;
    assert_eq!(
        singletons(&profile).len(),
        1,
        "Chromium's own, while it runs"
    );
    type_key_in_front(process, Key::Letter('q'), true)?;
    assert!(ends(process), "⌘Q ended the host");
    assert_eq!(singletons(&profile), Vec::<String>::new(), "cleaned up");
    assert!(chrome.version().is_err(), "its pipe is closed");
    drop(chrome);
    assert!(!profile.exists(), "{}", profile.display());
    Ok(())
}

/// Shown on the page at `address`: its menu bar read, what a person typed
/// copied and pasted with ⌘A, ⌘C and ⌘V, and then quit with ⌘Q with the
/// page's window in front: ended in order.
fn edited_then_quit_from_its_window(carried: &Path, address: &Url) -> Result<(), String> {
    let mut chrome = Chrome::start_from(&Chromium::Carried(carried.to_path_buf()), true)
        .map_err(|why| why.to_string())?;
    let (process, profile) = (chrome.process(), chrome.profile().to_path_buf());
    let mut tab = Tab::open(&mut chrome, address, SIZE).map_err(|why| why.to_string())?;
    let placed = chrome_window::place(&mut tab, 120, 80).map_err(|why| why.to_string())?;
    bring_in_front(process)?;
    // The host's menus, each item of its own first: macOS adds its own
    // after them (measured: Writing Tools, AutoFill, Start Dictation and
    // Emoji & Symbols in Edit, Close All in Window).
    let bar = menu_bar(process)?;
    let menus: Vec<&str> = bar.split(" | ").collect();
    assert_eq!(menus.first(), Some(&"Apple, alo, Edit, Window"), "{bar}");
    assert!(
        menus
            .get(1)
            .is_some_and(|edit| edit.starts_with("Cut, Copy, Paste, Select All,")),
        "{bar}"
    );
    assert!(
        menus
            .get(2)
            .is_some_and(|window| window.starts_with("Close Window")),
        "{bar}"
    );

    // What a person typed, selected, copied, and pasted after itself.
    type_keys("alo")?;
    let typed = vec![format!("Typed alo {} 1", bounds(&placed))];
    assert_eq!(once_listed(process, &typed)?, typed, "keys typed");
    type_key_in_front(process, Key::Letter('a'), true)?;
    type_key_in_front(process, Key::Letter('c'), true)?;
    type_key_in_front(process, Key::Code(KEY_RIGHT), false)?;
    type_key_in_front(process, Key::Letter('v'), true)?;
    let pasted = vec![format!("Typed aloalo {} 1", bounds(&placed))];
    assert_eq!(once_listed(process, &pasted)?, pasted, "copied and pasted");

    // ⌘Q with the page's window in front, which takes it as Chrome's Exit.
    assert_eq!(
        singletons(&profile).len(),
        1,
        "Chromium's own, while it runs"
    );
    type_key_in_front(process, Key::Letter('q'), true)?;
    assert!(ends(process), "⌘Q ended the host");
    drop(tab);
    assert_eq!(singletons(&profile), Vec::<String>::new(), "cleaned up");
    assert!(chrome.version().is_err(), "its pipe is closed");
    drop(chrome);
    assert!(!profile.exists(), "{}", profile.display());
    Ok(())
}

/// Shown, then ended by alo with its handle: its window goes with it.
fn shown_then_ended_by_alo(carried: &Path) -> Result<(), String> {
    let profile;
    let process;
    {
        let mut chrome = Chrome::start_from(&Chromium::Carried(carried.to_path_buf()), true)
            .map_err(|why| why.to_string())?;
        assert!(chrome.is_shown());
        (process, profile) = (chrome.process(), chrome.profile().to_path_buf());
        let version = chrome.version().map_err(|why| why.to_string())?;
        assert!(version.product.starts_with("Chrome/156."), "{version:?}");
        assert!(
            !version.user_agent.contains("HeadlessChrome"),
            "shown, not headless: {version:?}"
        );
        let until = Instant::now() + AT_MOST;
        let mut listed = on_the_screen(process)?;
        while listed.is_empty() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(100));
            listed = on_the_screen(process)?;
        }
        assert_eq!(listed.len(), 1, "one window on the screen: {listed:?}");
    }
    assert!(
        !is_running(process),
        "the host {process} outlived its handle"
    );
    assert!(!profile.exists(), "{}", profile.display());
    assert_eq!(on_the_screen(process)?, Vec::<String>::new());
    Ok(())
}

/// Shown on the page at `address`, placed, typed into and pressed by a
/// person, and closed by a person.
fn shown_on_a_page_then_closed_by_a_person(carried: &Path, address: &Url) -> Result<(), String> {
    let profile;
    let process;
    {
        let mut chrome = Chrome::start_from(&Chromium::Carried(carried.to_path_buf()), true)
            .map_err(|why| why.to_string())?;
        (process, profile) = (chrome.process(), chrome.profile().to_path_buf());
        let mut tab = Tab::open(&mut chrome, address, SIZE).map_err(|why| why.to_string())?;

        // The page is the size asked, and its window is that and a title
        // bar, with no sides: no tab strip, no toolbar.
        let fitted = chrome_window::placed(&mut tab).map_err(|why| why.to_string())?;
        assert_eq!(size(&fitted), "900×600");
        assert_eq!(fitted.window.state, WindowState::Normal);
        assert_eq!(fitted.window.bounds.width, 900, "{}", bounds(&fitted));
        let title_bar = fitted.window.bounds.height - 600;
        assert!(
            (1..=64).contains(&title_bar),
            "a title bar and nothing else above the page: {}",
            bounds(&fitted)
        );

        // Put where alo asks, at the size it was: exactly, by Chromium's
        // word and by the window server's, which names it by the page.
        let placed = chrome_window::place(&mut tab, 120, 80).map_err(|why| why.to_string())?;
        assert_eq!(bounds(&placed), format!("120,80 900×{}", 600 + title_bar));
        assert_eq!(size(&placed), "900×600");
        assert_eq!(placed.window.id, fitted.window.id, "the same window, moved");
        let wanted = vec![format!("{SAID} {} 1", bounds(&placed))];
        assert_eq!(
            once_listed(process, &wanted)?,
            wanted,
            "the window server's own word"
        );
        let moved = chrome_window::place(&mut tab, 160, 120).map_err(|why| why.to_string())?;
        assert_eq!(bounds(&moved), format!("160,120 900×{}", 600 + title_bar));
        let wanted = vec![format!("{SAID} {} 1", bounds(&moved))];
        assert_eq!(once_listed(process, &wanted)?, wanted, "moved");

        // The tree an agent reads is the page at that size.
        let tree = tab.read().map_err(|why| why.to_string())?;
        let root = tree.root.clone().ok_or("the page has no tree")?;
        assert_eq!(
            format!("{}×{}", root.rect.size.width, root.rect.size.height),
            "900×600"
        );
        assert!(
            tree.nodes()
                .iter()
                .any(|node| node.to_string().contains(SAID)),
            "{tree:?}"
        );
        assert_eq!(tab.refusals(), &[], "nothing here breaks a rule");

        // A person's keys reach the field the page focused, and a person's
        // press the button: each retitles the page, and the window with it.
        bring_in_front(process)?;
        type_keys("alo")?;
        let typed = vec![format!("Typed alo {} 1", bounds(&moved))];
        assert_eq!(once_listed(process, &typed)?, typed, "keys typed");
        let (left, top) = (moved.window.bounds.left, moved.window.bounds.top);
        press_at(left + 450, top + title_bar + 450)?;
        let pressed = vec![format!("Pressed by a person {} 1", bounds(&moved))];
        assert_eq!(once_listed(process, &pressed)?, pressed, "a press posted");

        // Closed by a person, by its close button: alo hears its page go,
        // and the window is gone from the screen.
        close_as_a_person(process)?;
        assert_eq!(heard_gone(&mut tab)?, Some(Closed::Page));
        assert_eq!(
            once_listed(process, &[])?,
            Vec::<String>::new(),
            "no window left on the screen"
        );
    }
    assert!(
        !is_running(process),
        "the host {process} outlived its handle"
    );
    assert!(!profile.exists(), "{}", profile.display());
    Ok(())
}

#[test]
fn alos_cef_host_shows_a_page_in_its_own_window_placed_and_closed_as_chromes() {
    let cef = match distribution() {
        Ok(cef) => cef,
        Err(why) => {
            eprintln!(
                "skipped: alo's CEF host needs a CEF distribution to be built against, \
                 and {why} (queue item 727)"
            );
            return;
        }
    };
    // The test presses and right-presses with the real pointer; it is put
    // back where it was when the test ends, passed or failed, so that a
    // test after this one does not find it resting over its window — a
    // pointer left over a link in Chrome's page kept the next shown-Chrome
    // test from seeing its page.
    let _pointer = pointer().map(PointerPutBack).unwrap();
    let program = built(&cef).unwrap();
    let scratch = std::env::temp_dir().join(format!("alo-cef-shown-test-{}", std::process::id()));
    let application = scratch.join(format!("{EXECUTABLE}.app"));
    let carried = cef_host::assemble(&program, &cef.join(FRAMEWORK), &application).unwrap();
    let before = written_outside().unwrap();
    let crashed = crash_reports().unwrap();
    let port = serve().unwrap();
    let address = alo_url::parse(&format!("http://127.0.0.1:{port}/shown.html")).unwrap();

    shown_then_ended_by_alo(&carried).unwrap();
    shown_on_a_page_then_closed_by_a_person(&carried, &address).unwrap();
    through_menus_then_closed_from_inside(&carried).unwrap();
    closed_by_its_page_then_quit(&carried).unwrap();
    edited_then_quit_from_its_window(&carried, &address).unwrap();

    // Nothing of the bundle left running, no crash, and nothing written
    // outside the profiles but macOS's own caches, which are removed.
    let until = Instant::now() + AT_MOST;
    while running_inside(&application) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!running_inside(&application), "a helper outlived the host");
    assert_eq!(crash_reports().unwrap(), crashed, "the host crashed");
    let after = written_outside().unwrap();
    let (macos, new): (Vec<_>, Vec<_>) = after
        .difference(&before)
        .partition(|path| is_macos_gpu_cache(path));
    assert!(new.is_empty(), "written outside the profile: {new:?}");
    for made in macos {
        eprintln!("macOS's own GPU cache, removed: {}", made.display());
        std::fs::remove_dir_all(made).unwrap();
    }
    std::fs::remove_dir_all(&scratch).unwrap();
}
