/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! ★ A closed list's menu in a shown Chrome: whether the window server shows
//! it, and a key posted to Chrome's process while it does (ADR 0071 § 2,
//! queue item 658).
//!
//! On macOS a shown Chrome draws a closed `<select>`'s menu as a menu of the
//! system's own: a window of Chrome's process at layer [`MENU_LAYER`]. Keys
//! sent over Chrome's protocol reach only the page, never that menu, so a
//! key is posted to Chrome's process with `CGEventPostToPid` — to that
//! process and to nothing else, never to whichever application a person has
//! in front.
//!
//! # Witnessed before every key, in the same call
//!
//! The menu is a window of Chrome's process at layer [`MENU_LAYER`] that
//! was not on the screen before the click that opened it ([`menu_of`]), and
//! a key is posted **only while the window server shows that window**, by
//! its number. Measured for ADR 0071, a menu closed by another application
//! brought in front leaves the list focused, and the same keys then reach
//! the list and choose an option nobody asked for. And measured for item
//! 658, a Down that reaches the list then opens its menu again, as a window
//! with **another number**, its highlight back at the option the list
//! holds: a guard that took any new menu for the one witnessed walked on in
//! it and chose an option nobody asked for. So the number is the witness.
//!
//! The window server is read and the key posted **in one `osascript`
//! call** ([`post_while_shown`]): the reading and the key are one script's
//! two statements apart, rather than two processes apart as a reading
//! followed by a posting would be. The race ADR 0071 § 4 names is narrowed
//! by that, and not closed.
//!
//! # And never under the pointer
//!
//! Measured for item 658: a pointer resting over the place the menu opens,
//! or moved there while it is open, puts the highlight on the item under
//! it, and the arrows then walk from there — from Apples to Dates, three
//! Downs chose Elderberries or Figs instead. So a key is posted only while
//! the pointer is **not over the menu's window**, read in the same script,
//! and otherwise nothing is posted and the script says `pointer`. A person
//! whose pointer is over the menu is choosing in it themselves.
//!
//! A menu opened again by a key that raced, or left open under a pointer, is
//! closed with Escape, which chooses nothing ([`close_any`]).
//!
//! # And only when macOS lets this browser post keys
//!
//! Measured for ADR 0072: a process macOS has not been told to trust — a
//! freshly installed application, and the `osascript` it starts — has its
//! posted keys **dropped without a word**, and `CGEventPostToPid` says
//! nothing of it. So before the click that opens a menu, the browser asks
//! macOS whether it may post events at all ([`may_post`]), and the posting
//! script asks again before every key, printing `refused` and posting
//! nothing when it may not. The question asks the person nothing:
//! `CGPreflightPostEventAccess`, never `CGRequestPostEventAccess`, which
//! would put macOS's prompt before them in the middle of an agent's verb
//! (ADR 0072 § 2.3).
//!
//! # Waiting for it
//!
//! The menu is shown a moment after the click that opens it, and goes a
//! moment after the key that closes it, in Chrome's own time. So the window
//! server is read every [`MENU_POLL`], for at most [`MENU_PATIENCE`]: until
//! the menu is shown ([`shown`]), until the one witnessed has gone
//! ([`gone`]), or until no new one is shown ([`closed`]).
//!
//! # Through `osascript`, as the window server is read
//!
//! As [`crate::window_server`] says, the crate that would call these
//! functions directly reaches the window list only through `unsafe`, which
//! law 4 forbids here without an ADR; ADR 0071 § 2.4 takes the same route
//! for the key. The process, the key and the windows on the screen before
//! are the script's arguments, never written into its text, and the key is
//! one of [`Key`]'s, never a number from outside.
//!
//! What `osascript` prints is read strictly: `posted`, `gone`, `pointer`
//! or `refused`, and `allowed` or `refused` to the question alone; anything
//! else is refused.

use crate::window_server::{self, Listed, Unlisted};
use core::fmt;
use std::io::ErrorKind;
use std::time::{Duration, Instant};

/// The layer of the window macOS draws a pop-up menu in, as the window
/// server listed a closed list's menu of Chrome 156 (ADR 0071 § 1).
pub const MENU_LAYER: i64 = 101;

/// The script, in JavaScript for Automation, that asks macOS whether this
/// process may post events, asking the person nothing, and prints `allowed`
/// or `refused`. The function is not in the scripting bridge's own list, so
/// it is bound by its name and its type (ADR 0072 § 1).
#[cfg(target_os = "macos")]
const ASKING: &str = "ObjC.import('CoreGraphics');\
    ObjC.bindFunction('CGPreflightPostEventAccess', ['bool', []]);\
    $.CGPreflightPostEventAccess() ? 'allowed' : 'refused'";

/// The script, in JavaScript for Automation: when macOS does not let this
/// process post events, post nothing and print `refused`. Otherwise, when a
/// window of the process in its first argument is on the screen at
/// [`MENU_LAYER`] — the window
/// numbered in its third argument, or, when that is nought, any window
/// numbered in none of its arguments after it — post the key whose code is
/// its second argument, pressed then let go, to that process and print
/// `posted`; otherwise post nothing and print `gone`. No window is
/// numbered nought. For a window by its number, the pointer is read first,
/// in the same points as the window's bounds, and over the window nothing is
/// posted and `pointer` printed.
#[cfg(target_os = "macos")]
const POSTING: &str = "ObjC.import('CoreGraphics');\
    function run(argv) {\
      var owner = Number(argv[0]);\
      var key = Number(argv[1]);\
      var menu = Number(argv[2]);\
      var before = argv.slice(3).map(Number);\
      ObjC.bindFunction('CGPreflightPostEventAccess', ['bool', []]);\
      if (!$.CGPreflightPostEventAccess()) { return 'refused'; }\
      var all = ObjC.deepUnwrap(ObjC.castRefToObject(\
        $.CGWindowListCopyWindowInfo($.kCGWindowListOptionOnScreenOnly, 0))) || [];\
      var shown = all.find(w => w.kCGWindowOwnerPID === owner\
        && w.kCGWindowLayer === 101 && (menu === 0\
          ? before.indexOf(w.kCGWindowNumber) < 0 : w.kCGWindowNumber === menu));\
      if (!shown) { return 'gone'; }\
      var at = $.CGEventGetLocation($.CGEventCreate($()));\
      var b = shown.kCGWindowBounds;\
      if (menu !== 0 && at.x >= b.X && at.x < b.X + b.Width\
        && at.y >= b.Y && at.y < b.Y + b.Height) { return 'pointer'; }\
      [true, false].forEach(down =>\
        $.CGEventPostToPid(owner, $.CGEventCreateKeyboardEvent($(), key, down)));\
      return 'posted';\
    }";

/// The longest a closed list's menu is waited for to be shown after the
/// click that opens it, and to go after the key that closes it. Against
/// Chrome 156 on macOS 26, in every run of item 658's tests, the menu was
/// shown and gone well inside it: no run waited it out.
pub const MENU_PATIENCE: Duration = Duration::from_secs(3);

/// How long between two readings of the window server while waiting.
pub const MENU_POLL: Duration = Duration::from_millis(50);

/// A key a closed list's menu is walked and chosen in with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// The highlight one item down. Chooses nothing.
    Down,
    /// The highlight one item up. Chooses nothing.
    Up,
    /// Choose the highlighted item, and close the menu.
    Return,
    /// Close the menu, choosing nothing.
    Escape,
}

impl Key {
    /// Its virtual key code on macOS: the key's place on the keyboard, the
    /// same on every layout for these four.
    pub const fn code(self) -> u16 {
        match self {
            Self::Down => 125,
            Self::Up => 126,
            Self::Return => 36,
            Self::Escape => 53,
        }
    }
}

/// What came of a key asked to be posted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posted {
    /// The menu was shown, and the key was posted to Chrome's process.
    Posted,
    /// The menu was not shown, and nothing was posted.
    Gone,
    /// The menu was shown with the pointer over it, and nothing was posted.
    UnderPointer,
    /// macOS does not let this browser post events, and nothing was posted
    /// (ADR 0072).
    NotAllowed,
}

/// Why it is not known whether a key was posted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unposted {
    /// `osascript` could not be run.
    NotRun(ErrorKind),
    /// `osascript` ran and failed, and said this.
    Refused {
        /// What it wrote to its error output, cut short.
        said: String,
    },
    /// What it printed was none of the answers it may print.
    Unread,
    /// This system's menus are not ones this browser posts keys to.
    Elsewhere,
}

impl fmt::Display for Unposted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRun(kind) => write!(f, "the key could not be posted: {kind}"),
            Self::Refused { said } => write!(f, "posting the key failed: {said}"),
            Self::Unread => f.write_str("osascript printed none of the answers it may print"),
            Self::Elsewhere => {
                f.write_str("this system's menus are not ones this browser posts keys to")
            }
        }
    }
}

impl std::error::Error for Unposted {}

/// The menu that `now`, a reading of a process's windows on the screen,
/// shows and `before`, read before the click that opened it, did not — by
/// its number, the frontmost when there are several.
pub fn menu_of(before: &[Listed], now: &[Listed]) -> Option<u64> {
    now.iter()
        .find(|window| {
            window.layer == MENU_LAYER && !before.iter().any(|old| old.number == window.number)
        })
        .map(|window| window.number)
}

/// The menu of the process `owner` that `before` did not hold, by its
/// number, once the window server shows it — waited for at most
/// [`MENU_PATIENCE`].
///
/// # Errors
///
/// [`Unlisted`], when the window server could not be read.
pub fn shown(owner: u32, before: &[Listed]) -> Result<Option<u64>, Unlisted> {
    let deadline = Instant::now() + MENU_PATIENCE;
    loop {
        let menu = menu_of(before, &window_server::on_the_screen(owner)?);
        if menu.is_some() || Instant::now() >= deadline {
            return Ok(menu);
        }
        std::thread::sleep(MENU_POLL);
    }
}

/// Whether the window server shows no menu of the process `owner` that
/// `before` did not hold — waited for at most [`MENU_PATIENCE`].
///
/// # Errors
///
/// [`Unlisted`], when the window server could not be read.
pub fn closed(owner: u32, before: &[Listed]) -> Result<bool, Unlisted> {
    let deadline = Instant::now() + MENU_PATIENCE;
    loop {
        let open = menu_of(before, &window_server::on_the_screen(owner)?).is_some();
        if !open || Instant::now() >= deadline {
            return Ok(!open);
        }
        std::thread::sleep(MENU_POLL);
    }
}

/// Whether the window server has stopped showing the window numbered
/// `menu` of the process `owner` — waited for at most [`MENU_PATIENCE`].
///
/// # Errors
///
/// [`Unlisted`], when the window server could not be read.
pub fn gone(owner: u32, menu: u64) -> Result<bool, Unlisted> {
    let deadline = Instant::now() + MENU_PATIENCE;
    loop {
        let shown = window_server::on_the_screen(owner)?
            .iter()
            .any(|window| window.number == menu);
        if !shown || Instant::now() >= deadline {
            return Ok(!shown);
        }
        std::thread::sleep(MENU_POLL);
    }
}

/// Whether macOS lets this browser's process post events to another's —
/// asked without asking the person (ADR 0072 § 2.1).
///
/// # Errors
///
/// [`Unposted`], when `osascript` could not be run, failed, or printed
/// something else, or [`Unposted::Elsewhere`] off macOS.
#[cfg(target_os = "macos")]
pub fn may_post() -> Result<bool, Unposted> {
    allowed(&osascript(ASKING, &[])?)
}

/// No event is posted off macOS (ADR 0071 § 4).
///
/// # Errors
///
/// [`Unposted::Elsewhere`], always.
#[cfg(not(target_os = "macos"))]
pub fn may_post() -> Result<bool, Unposted> {
    Err(Unposted::Elsewhere)
}

/// Post `key` to the process `owner` if the window server shows the menu
/// numbered `menu` now, the pointer is not over it and macOS lets this
/// browser post events, and say whether it did.
///
/// # Errors
///
/// [`Unposted`], when `osascript` could not be run, failed, or printed
/// something else — in which case whether the key was posted is not known
/// — or [`Unposted::Elsewhere`] off macOS.
pub fn post_while_shown(owner: u32, key: Key, menu: u64) -> Result<Posted, Unposted> {
    if menu == 0 {
        // No window is numbered nought, so none is shown; and nought would
        // be read by the script as any menu at all.
        return Ok(Posted::Gone);
    }
    post(owner, key, menu, &[])
}

/// Post Escape to the process `owner` if the window server shows any menu
/// of that process that `before` did not hold — closing it, choosing
/// nothing, wherever the pointer is — and say whether it did.
///
/// # Errors
///
/// As [`post_while_shown`].
pub fn close_any(owner: u32, before: &[Listed]) -> Result<Posted, Unposted> {
    post(owner, Key::Escape, 0, before)
}

/// Post `key` to the process `owner` while it shows the menu numbered
/// `menu` — or, for nought, any menu not in `before`.
#[cfg(target_os = "macos")]
fn post(owner: u32, key: Key, menu: u64, before: &[Listed]) -> Result<Posted, Unposted> {
    let mut arguments = vec![owner.to_string(), key.code().to_string(), menu.to_string()];
    arguments.extend(before.iter().map(|window| window.number.to_string()));
    read(&osascript(POSTING, &arguments)?)
}

/// Run `script`, in JavaScript for Automation, with `arguments`, and say
/// what it printed.
#[cfg(target_os = "macos")]
fn osascript(script: &str, arguments: &[String]) -> Result<Vec<u8>, Unposted> {
    // `--` ends osascript's own options, so no argument is ever read as
    // one: a number written with a minus sign would be.
    let ran = std::process::Command::new("osascript")
        .args(["-l", "JavaScript", "-e", script, "--"])
        .args(arguments)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|why| Unposted::NotRun(why.kind()))?;
    if !ran.status.success() {
        let said = String::from_utf8_lossy(&ran.stderr);
        return Err(Unposted::Refused {
            said: said.trim().chars().take(200).collect(),
        });
    }
    Ok(ran.stdout)
}

/// Nothing is posted off macOS: how another system's menus are reached is
/// not measured (ADR 0071 § 4).
#[cfg(not(target_os = "macos"))]
fn post(_owner: u32, _key: Key, _menu: u64, _before: &[Listed]) -> Result<Posted, Unposted> {
    Err(Unposted::Elsewhere)
}

/// What the question printed, read.
fn allowed(printed: &[u8]) -> Result<bool, Unposted> {
    match printed {
        b"allowed\n" => Ok(true),
        b"refused\n" => Ok(false),
        _ => Err(Unposted::Unread),
    }
}

/// What the posting script printed, read.
fn read(printed: &[u8]) -> Result<Posted, Unposted> {
    match printed {
        b"posted\n" => Ok(Posted::Posted),
        b"gone\n" => Ok(Posted::Gone),
        b"pointer\n" => Ok(Posted::UnderPointer),
        b"refused\n" => Ok(Posted::NotAllowed),
        _ => Err(Unposted::Unread),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(number: u64, layer: i64) -> Listed {
        Listed {
            number,
            owner: 7,
            layer,
            bounds: None,
        }
    }

    #[test]
    fn a_menu_is_a_window_at_the_menus_layer_that_was_not_there_before() {
        let page = window(40, 0);
        let menu = window(41, MENU_LAYER);
        assert_eq!(menu_of(&[page], &[menu, page]), Some(41));
        assert_eq!(menu_of(&[page], &[page]), None, "no menu");
        assert_eq!(
            menu_of(&[page, menu], &[menu, page]),
            None,
            "a menu already shown before the click is not the list's"
        );
        assert_eq!(
            menu_of(&[page], &[window(42, 3), page]),
            None,
            "a floating window is not a menu"
        );
        assert_eq!(menu_of(&[menu], &[menu, window(43, MENU_LAYER)]), Some(43));
    }

    #[test]
    fn no_menu_numbered_nought_is_ever_shown() {
        assert_eq!(post_while_shown(7, Key::Down, 0), Ok(Posted::Gone));
    }

    #[test]
    fn only_the_scripts_own_answers_are_read() {
        assert_eq!(read(b"posted\n"), Ok(Posted::Posted));
        assert_eq!(read(b"gone\n"), Ok(Posted::Gone));
        assert_eq!(read(b"pointer\n"), Ok(Posted::UnderPointer));
        assert_eq!(read(b"refused\n"), Ok(Posted::NotAllowed));
        for other in [
            &b""[..],
            b"posted",
            b"gone",
            b"refused",
            b"Refused\n",
            b"allowed\n",
            b"posted\nposted\n",
            b"Posted\n",
            b" gone\n",
            b"\xff\n",
        ] {
            assert_eq!(read(other), Err(Unposted::Unread), "{other:?}");
        }
    }

    #[test]
    fn only_allowed_or_refused_answers_the_question() {
        assert_eq!(allowed(b"allowed\n"), Ok(true));
        assert_eq!(allowed(b"refused\n"), Ok(false));
        for other in [
            &b""[..],
            b"allowed",
            b"true\n",
            b"posted\n",
            b"allowed\nrefused\n",
            b" refused\n",
        ] {
            assert_eq!(allowed(other), Err(Unposted::Unread), "{other:?}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn both_scripts_run_and_answer_in_their_own_words() -> Result<(), Unposted> {
        // Whatever this process is allowed, the question is answered with
        // one of its two words, and asks nobody anything; and the posting
        // script answers it the same way before it reads any window. No
        // process is numbered nought, so none of its menus is shown.
        let allowed = may_post()?;
        let posted = post_while_shown(0, Key::Escape, 1)?;
        assert_eq!(
            posted,
            if allowed {
                Posted::Gone
            } else {
                Posted::NotAllowed
            }
        );
        Ok(())
    }

    #[test]
    fn each_key_is_its_place_on_the_keyboard() {
        assert_eq!(
            [Key::Down, Key::Up, Key::Return, Key::Escape].map(Key::code),
            [125, 126, 36, 53]
        );
    }
}
