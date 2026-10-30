/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! CEF, loaded and run: one of the three files that name the `cef` crate,
//! with [`crate::window`] and [`crate::application_mac`] (ADR 0079 § 3
//! option H, queue items 725, 727, 731).
//!
//! CEF is a library, not a browser: the program that loads it **is**
//! Chromium's browser process, and the same program started again by CEF
//! with `--type=` is each helper. This file does what each needs and
//! nothing more. A helper enters macOS's sandbox through CEF's own
//! `libcef_sandbox.dylib`, loads the framework from three directories up,
//! and runs as CEF tells it ([`helper`]). The browser loads the framework
//! from its bundle's `Contents/Frameworks`, starts Chromium on the profile
//! alo gave it, and runs until Chromium ends it ([`browser`]).
//!
//! # One window of its own, and an `NSApplication` of its own
//!
//! The host opens one window itself: the page alo names with `--app=`,
//! shown as Chrome shows its own `--app` window ([`crate::window`], queue
//! item 727). Any other is one alo asks for over the pipe
//! (`Target.createTarget`). Both are Chrome's own style — CEF's *Chrome
//! style*, the only one in which ADR 0060's held requests work (CEF issue
//! #4308, ADR 0079 § 3.2) — and `--headless` shows neither.
//!
//! Before CEF starts, the host makes its own `NSApplication` subclass the
//! shared application ([`crate::application_mac`], ADR 0080, queue item
//! 731), as CEF's own samples do: with `AppKit`'s own, a shown host
//! crashed on a page closed from inside, a context menu and a closed
//! list's menu (727). Its `-terminate:` — reached by `Browser.close` and
//! by Quit in the menu bar ([`crate::menu_mac`], put in place once CEF
//! has started) — closes every page and leaves the message loop, so CEF
//! shuts down and [`browser`] returns 0: an orderly exit, where `AppKit`'s
//! own ended the process at once and Chromium's clean-up never ran. What
//! that clean-up would remove outside the profile is still kept inside it
//! (`switches.rs`).
//!
//! # Refused, not panicked
//!
//! The `cef` crate panics when the framework or the sandbox library is not
//! where it looks, so both are looked for first and their absence is said
//! and the process ends with [`MISSING`].

use crate::{application_mac, menu_mac, switches, window};
use cef::args::Args;
use cef::library_loader::LibraryLoader;
use cef::sandbox::Sandbox;
use cef::{CefString, LogSeverity, Settings};
use std::path::{Path, PathBuf};

/// The exit status when the framework or the sandbox library is missing.
pub const MISSING: i32 = 3;

/// The exit status when CEF would not start.
pub const UNSTARTED: i32 = 4;

/// The exit status when the host's application object could not be made.
pub const NO_APPLICATION: i32 = 6;

/// The framework's program, as CEF lays it out.
const FRAMEWORK: &str = "Chromium Embedded Framework.framework/Chromium Embedded Framework";

/// CEF's sandbox library, inside the framework.
const SANDBOX: &str = "Chromium Embedded Framework.framework/Libraries/libcef_sandbox.dylib";

/// Where the framework's directory is from a program at `program`: the
/// browser's is `Contents/Frameworks` beside its `Contents/MacOS`; a
/// helper's, three directories above its own `Contents/MacOS`, since each
/// helper is an application inside that same `Contents/Frameworks`.
pub fn frameworks(program: &Path, role: switches::Role) -> Option<PathBuf> {
    let programs = program.parent()?;
    match role {
        switches::Role::Browser => Some(programs.parent()?.join("Frameworks")),
        switches::Role::Helper => Some(programs.parent()?.parent()?.parent()?.to_path_buf()),
    }
}

/// Run as the helper CEF started: the process's exit status.
pub fn helper(program: &Path) -> i32 {
    let Some(frameworks) = frameworks(program, switches::Role::Helper) else {
        return MISSING;
    };
    let args = Args::new();
    if !frameworks.join(SANDBOX).is_file() {
        eprintln!(
            "alo's CEF host: no sandbox library at {}",
            frameworks.join(SANDBOX).display()
        );
        return MISSING;
    }
    // Before the framework is loaded, as CEF asks; it is left when this
    // returns, after the helper's work.
    let mut sandbox = Sandbox::new();
    sandbox.initialize(args.as_main_args());
    let Some(_framework) = loaded(program, &frameworks, true) else {
        return MISSING;
    };
    cef::execute_process(Some(args.as_main_args()), None, std::ptr::null_mut())
}

/// Run as the browser process, on `profile`, showing `page` in a window of
/// its own when alo asked for one (`--app=`, [`crate::window`]): the
/// process's exit status: 0 once the host has ended in order (Quit,
/// `Browser.close`), or why it would not start.
pub fn browser(program: &Path, profile: &Path, page: Option<String>) -> i32 {
    let Some(frameworks) = frameworks(program, switches::Role::Browser) else {
        return MISSING;
    };
    let args = Args::new();
    let Some(_framework) = loaded(program, &frameworks, false) else {
        return MISSING;
    };
    // A browser process is never told `--type=`, so this starts no other
    // process's work; it is asked anyway, as CEF asks every process to.
    let code = cef::execute_process(Some(args.as_main_args()), None, std::ptr::null_mut());
    if code >= 0 {
        return code;
    }
    // After the framework is loaded, whose protocols the class conforms
    // to, and before CEF starts, which would make `AppKit`'s own.
    if let Err(why) = application_mac::install(window::close_all) {
        eprintln!("alo's CEF host: no application object of its own: {why:?}");
        return NO_APPLICATION;
    }
    let profile = CefString::from(profile.to_string_lossy().as_ref());
    let settings = Settings {
        root_cache_path: profile.clone(),
        cache_path: profile,
        // CEF's own log would be `debug.log` in the working directory.
        log_severity: LogSeverity::DISABLE,
        ..Settings::default()
    };
    let mut app = window::app(page);
    if cef::initialize(
        Some(args.as_main_args()),
        Some(&settings),
        Some(&mut app),
        std::ptr::null_mut(),
    ) != 1
    {
        eprintln!("alo's CEF host: CEF would not start");
        return UNSTARTED;
    }
    menu_mac::install();
    cef::run_message_loop();
    cef::shutdown();
    0
}

/// CEF's framework, loaded for a program at `program` whose framework
/// directory is `frameworks`, and its API version declared — or [`None`],
/// said, when it is not there.
fn loaded(program: &Path, frameworks: &Path, helper: bool) -> Option<LibraryLoader> {
    if !frameworks.join(FRAMEWORK).is_file() {
        eprintln!(
            "alo's CEF host: no Chromium Embedded Framework at {}",
            frameworks.join(FRAMEWORK).display()
        );
        return None;
    }
    let framework = LibraryLoader::new(program, helper);
    if !framework.load() {
        eprintln!(
            "alo's CEF host: the framework at {} would not load",
            frameworks.join(FRAMEWORK).display()
        );
        return None;
    }
    // The API version this host was built against, which CEF checks its own
    // against before anything else is called.
    let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);
    Some(framework)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_framework_is_beside_the_browser_and_above_a_helper() {
        assert_eq!(
            frameworks(
                Path::new("/A/alo Chromium.app/Contents/MacOS/alo Chromium"),
                switches::Role::Browser
            ),
            Some(PathBuf::from("/A/alo Chromium.app/Contents/Frameworks"))
        );
        assert_eq!(
            frameworks(
                Path::new(
                    "/A/alo Chromium.app/Contents/Frameworks/alo Chromium Helper (Renderer).app\
                     /Contents/MacOS/alo Chromium Helper (Renderer)"
                ),
                switches::Role::Helper
            ),
            Some(PathBuf::from("/A/alo Chromium.app/Contents/Frameworks"))
        );
        assert_eq!(
            frameworks(Path::new("alone"), switches::Role::Browser),
            None
        );
    }
}
