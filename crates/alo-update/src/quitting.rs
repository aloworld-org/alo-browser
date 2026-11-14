/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An update installs at quit, and only then (ADR 0116 § 4).
//!
//! # Never a restart by itself
//!
//! alo does not restart at night, when idle, or when an update is critical.
//! A person's quit is the only start: [`install`] is called after every
//! card's host has ended and before alo exits — CEF starts its helpers
//! from the bundle on demand, so a bundle swapped under a running alo would
//! start a new helper against an old framework. Nothing here starts a
//! program.
//!
//! # *Quit and update* waits for an agent
//!
//! While an agent is working in any chat, the button does not quit
//! ([`quit_and_update`]): it says *An agent is working. alo will update
//! when you quit.* — and a person's own quit still installs. No ask of the
//! agent socket starts, delays, refuses or reads an update.
//!
//! # The swap
//!
//! The staged bundle is checked again — `codesign --verify --strict` and
//! its version — for it has waited on the disk since it was unpacked. Its
//! `ready` mark is removed **before** the exchange, so a crash between the
//! two leaves an unready folder that the next launch sweeps, never a mark
//! beside the old application that would swap it back. An exchange
//! refused (a folder that cannot be written) leaves the old one in place,
//! puts the mark back for the next quit, and says why.

use crate::stage::{self, READY};
use crate::swap;
use crate::version::Version;
use std::fs;
use std::path::Path;

/// What the button says, and is named.
pub const QUIT_AND_UPDATE: &str = "Quit and update";

/// What *Quit and update* says while an agent works.
pub const AGENT_WORKING: &str = "An agent is working. alo will update when you quit.";

/// What *Quit and update* does when pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressed {
    /// alo quits, and the update installs as it does.
    Quits,
    /// alo stays, saying why.
    Waits(&'static str),
}

/// *Quit and update* pressed, an agent `working` in some chat or not.
pub fn quit_and_update(working: bool) -> Pressed {
    if working {
        Pressed::Waits(AGENT_WORKING)
    } else {
        Pressed::Quits
    }
}

/// What happened at quit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Installed {
    /// Nothing was ready.
    Nothing,
    /// This version is now at the application's path.
    Swapped(Version),
    /// The ready one was not installed, and why; the old one is in place.
    Refused(String),
}

/// Install what is ready beside `installed`, the application now `running`
/// this version — at quit, after every card's host has ended.
pub fn install(installed: &Path, running: Version) -> Installed {
    let Some(staged) = stage::ready_beside(installed) else {
        return Installed::Nothing;
    };
    let Some(folder) = staged.bundle.parent().map(Path::to_path_buf) else {
        return Installed::Nothing;
    };
    // Never an older or equal one, whatever was left on the disk.
    if staged.version <= running {
        let _ = fs::remove_dir_all(&folder);
        return Installed::Nothing;
    }
    if let Err(why) = stage::verified(&staged.bundle, staged.version) {
        let _ = fs::remove_dir_all(&folder);
        return Installed::Refused(format!("The update was refused: {why}."));
    }
    let mark = folder.join(READY);
    if let Err(why) = fs::remove_file(&mark) {
        return Installed::Refused(format!("The update could not be installed: {why}."));
    }
    match swap::exchange(&staged.bundle, installed) {
        Ok(()) => Installed::Swapped(staged.version),
        Err(why) => {
            let _ = fs::write(&mark, format!("{}\n", staged.version));
            Installed::Refused(format!(
                "The update could not be installed, and alo {running} is as it was: {why}."
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_button_waits_while_an_agent_works() {
        assert_eq!(
            quit_and_update(true),
            Pressed::Waits("An agent is working. alo will update when you quit.")
        );
        assert_eq!(quit_and_update(false), Pressed::Quits);
    }

    #[test]
    fn nothing_ready_installs_nothing() {
        let root = std::env::temp_dir().join(format!("alo-update-quit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let app = root.join("alo browser.app");
        fs::create_dir_all(&app).expect("made");
        assert_eq!(install(&app, Version::of(0, 4, 1)), Installed::Nothing);
        // A folder that says it is ready for an older version is removed,
        // never swapped in.
        let older = root.join(".alo-update/0.4.0");
        fs::create_dir_all(older.join("alo browser.app")).expect("made");
        fs::write(older.join(READY), "0.4.0\n").expect("written");
        assert_eq!(install(&app, Version::of(0, 4, 1)), Installed::Nothing);
        assert!(!older.exists());
        let _ = fs::remove_dir_all(&root);
    }
}
