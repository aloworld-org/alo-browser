/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! alo's own host program for CEF's Chromium (ADR 0079 § 3 option H, queue
//! item 725).
//!
//! The alo browser draws the web with a Chromium it carries (ADR 0077), and
//! the Chromium it carries is CEF's published build (ADR 0079). CEF is a
//! library, so something has to load it: this program, and nothing else. It
//! is started and driven by `alo-window` exactly as `chrome.rs` starts and
//! drives Chrome — descriptors 3 and 4 with `--remote-debugging-pipe`, the
//! profile as `--user-data-dir`, ADR 0073's switches — and every window it
//! shows is one alo asked for over that pipe. alo's own process links
//! nothing of Chromium's and holds no `unsafe` (ADR 0058). This one holds
//! one boundary, which the owner allowed (ADR 0080): on macOS CEF requires
//! an `NSApplication` subclass of the host's own (`CefAppProtocol`), and
//! defining one is `unsafe` in Rust. It is [`application_mac`], the one
//! module the lint below is lifted for; every other module of the host
//! holds no `unsafe`, and `scripts/gate.sh` refuses it anywhere else.
//! CEF's own is the `cef` crate's, behind the files that name it: [`host`],
//! which loads and runs it, [`window`], the window it shows a page in for
//! `--app=` (queue item 727) and the pages it closes when it ends, and
//! [`application_mac`].
//!
//! It is one program in two roles ([`switches::Role`]): the browser process
//! alo starts, and each helper CEF starts from the copies of it in the
//! bundle's `Contents/Frameworks` (`alo-window`'s `cef_host.rs` lays the
//! bundle out). The browser first makes sure that everything Chromium
//! writes goes inside the profile it was given, starting itself again in
//! its own place to do it ([`switches`]), then runs CEF until Chromium ends
//! the process.
//!
//! Built on macOS only so far: Windows and Linux are queue item 728.

#![deny(unsafe_code)]

#[cfg(not(target_os = "macos"))]
compile_error!("alo's CEF host is built for macOS only so far (queue item 728)");

#[expect(
    unsafe_code,
    reason = "the `NSApplication` subclass CEF requires on macOS, the one boundary ADR 0080 allows"
)]
mod application_mac;
mod host;
mod menu_mac;
mod switches;
mod window;

use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};

/// The exit status when alo gave no profile, or not an absolute one.
const NO_PROFILE: u8 = 2;

/// The exit status when the host could not start itself again.
const NOT_AGAIN: u8 = 5;

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let program = match std::env::current_exe() {
        Ok(program) => program,
        Err(why) => {
            eprintln!("alo's CEF host: where this program is is not known: {why}");
            return ExitCode::from(host_status(host::MISSING));
        }
    };
    if switches::role(&args) == switches::Role::Helper {
        return ExitCode::from(host_status(host::helper(&program)));
    }
    let Some(profile) = switches::profile(&args) else {
        eprintln!(
            "alo's CEF host: no profile was given as an absolute --user-data-dir, \
             and the host writes nowhere else"
        );
        return ExitCode::from(NO_PROFILE);
    };
    let variable = std::env::var_os(switches::TEMPORARY_VARIABLE);
    if let Some(again) = switches::again(&args, &profile, variable.as_deref()) {
        if let Err(why) = std::fs::create_dir_all(&again.temporary) {
            eprintln!(
                "alo's CEF host: no temporary directory at {}: {why}",
                again.temporary.display()
            );
            return ExitCode::from(NOT_AGAIN);
        }
        // In this process's own place: the same process, holding the same
        // descriptors. `exec` returns only when it failed.
        let why = Command::new(&program)
            .args(&args)
            .args(&again.added)
            .env(switches::TEMPORARY_VARIABLE, &again.temporary)
            .exec();
        eprintln!("alo's CEF host: could not start again: {why}");
        return ExitCode::from(NOT_AGAIN);
    }
    ExitCode::from(host_status(host::browser(
        &program,
        &profile,
        switches::app(&args),
    )))
}

/// A status CEF or the host returned, as a process's exit status: one that
/// does not fit is said as 1, never as a status it is not.
fn host_status(code: i32) -> u8 {
    u8::try_from(code).unwrap_or(1)
}
