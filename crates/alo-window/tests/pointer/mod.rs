/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The real pointer, read and moved as a mouse moves it, for the tests that
//! need a person's pointer somewhere on the screen (queue items 658, 731,
//! 734 and 809).
//!
//! # Moved, and there before the mover goes
//!
//! A move is posted at the window server's HID tap by an `osascript` of its
//! own, and **that script stays until the window server says the pointer is
//! there**. Measured for queue item 809 on macOS 26.5: a move posted by a
//! script that ended at once was lost in 23 of 300 posts — the pointer
//! never left where it was, however long it was waited for — and in none of
//! 300 when the script lived a tenth of a second after posting. The closed
//! list's tests waited for the pointer from outside, and failed in about one
//! run in eight with "the pointer never reached"; the host's tests did not
//! wait at all, and walked a menu with the pointer wherever it had been.
//! The move is posted once: a script still waiting when its patience ends
//! says so, and the move fails — never posted again.

use std::time::Duration;

/// The longest a move is waited for to reach the window server.
const AT_MOST: Duration = Duration::from_secs(10);

/// Run a script of JavaScript for Automation with `arguments`, and say what
/// it printed.
fn automation(script: &str, arguments: &[String]) -> Result<String, String> {
    // `--` ends osascript's own options: a point on a screen left of or
    // above the main one is negative, and "-1920" read as an option is
    // "illegal option -- 1".
    let ran = std::process::Command::new("osascript")
        .args(["-l", "JavaScript", "-e", script, "--"])
        .args(arguments)
        .output()
        .map_err(|why| why.to_string())?;
    if ran.status.success() {
        Ok(String::from_utf8_lossy(&ran.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&ran.stderr).into_owned())
    }
}

/// Where the pointer is, in the screen's points.
pub fn at() -> Result<(f64, f64), String> {
    let at = automation(
        "ObjC.import('CoreGraphics');\
         var at = $.CGEventGetLocation($.CGEventCreate($())); at.x + ' ' + at.y",
        &[],
    )?;
    let (x, y) = at.split_once(' ').ok_or(format!("no pointer in {at:?}"))?;
    let number = |text: &str| text.parse::<f64>().map_err(|why| why.to_string());
    Ok((number(x)?, number(y)?))
}

/// Move the pointer to `x`, `y`, in the screen's points, as a mouse does,
/// and come back once the window server has it there — or say it never
/// got there within [`AT_MOST`].
pub fn move_to((x, y): (f64, f64)) -> Result<(), String> {
    let said = automation(
        "ObjC.import('CoreGraphics'); function run(argv) {\
           var to = {x: Number(argv[0]), y: Number(argv[1])};\
           var until = Date.now() + Number(argv[2]);\
           $.CGEventPost($.kCGHIDEventTap, $.CGEventCreateMouseEvent($(),\
             $.kCGEventMouseMoved, to, $.kCGMouseButtonLeft));\
           while (true) {\
             var at = $.CGEventGetLocation($.CGEventCreate($()));\
             if (at.x === to.x && at.y === to.y) { return 'there'; }\
             if (Date.now() > until) { return 'at ' + at.x + ' ' + at.y; }\
             delay(0.01);\
           } }",
        &[
            x.to_string(),
            y.to_string(),
            AT_MOST.as_millis().to_string(),
        ],
    )?;
    if said == "there" {
        settled()
    } else {
        Err(format!("the pointer never reached {x}, {y}: it is {said}"))
    }
}

/// Come back once the pointer has been still for longer than a closed
/// list's menu asks before a key is posted to it
/// ([`alo_window::chrome_menu::AT_REST`]): a move, anywhere, moves the
/// menu's highlight, and the menu's walk refuses to go on so soon after one.
pub fn settled() -> Result<(), String> {
    let wanted = alo_window::chrome_menu::AT_REST.as_secs_f64() + 0.1;
    let said = automation(
        "ObjC.import('CoreGraphics'); function run(argv) {\
           var wanted = Number(argv[0]); var until = Date.now() + Number(argv[1]);\
           while (true) {\
             var still = $.CGEventSourceSecondsSinceLastEventType(\
               $.kCGEventSourceStateCombinedSessionState, $.kCGEventMouseMoved);\
             if (still >= wanted) { return 'still'; }\
             if (Date.now() > until) { return 'moved ' + still + ' s before'; }\
             delay(0.05);\
           } }",
        &[wanted.to_string(), AT_MOST.as_millis().to_string()],
    )?;
    if said == "still" {
        Ok(())
    } else {
        Err(format!("the pointer never came to rest: it {said}"))
    }
}

/// Where the pointer was, put back there when this is dropped, however the
/// test ends.
pub struct PutBack(pub (f64, f64));

impl Drop for PutBack {
    fn drop(&mut self) {
        if let Err(why) = move_to(self.0) {
            eprintln!("the pointer could not be put back: {why}");
        }
    }
}
