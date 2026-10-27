/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Where the renderer programs are: ours beside the browser's own, and
//! Chrome where Chrome installs itself.
//!
//! `alo` and `alo-render` are built into the same directory, and are shipped
//! that way. Looking beside the running program, rather than on the search
//! path, means the renderer started is the one built with this browser — a
//! renderer from somewhere else on `PATH` would speak a message format this
//! process may not.
//!
//! Chrome, the other renderer (ADR 0058), is the one already installed on
//! the machine, and is found the same way: at the places its own installer
//! puts it, never on `PATH`, where anybody who can write one directory on it
//! could put a program called `google-chrome`. One that is not at any of them
//! is refused by name, saying where it was looked for ([`NotInstalled`]).

use core::fmt;
use std::path::{Path, PathBuf};

/// The renderer's file name.
pub const RENDERER: &str = "alo-render";

/// The renderer that belongs with the program at `program`.
pub fn renderer_beside(program: &Path) -> PathBuf {
    program.with_file_name(RENDERER)
}

/// Where Google Chrome's installer puts the program on this kind of machine,
/// in the order they are tried. `home` is the person's home directory, if
/// there is one, for an installation of their own.
pub fn chrome_places(home: Option<&Path>) -> Vec<PathBuf> {
    let mut places = Vec::new();
    if cfg!(target_os = "macos") {
        let program = "Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
        places.push(Path::new("/").join(program));
        if let Some(home) = home {
            places.push(home.join(program));
        }
    } else if cfg!(unix) {
        places.push(PathBuf::from("/opt/google/chrome/chrome"));
        places.push(PathBuf::from("/usr/bin/google-chrome-stable"));
        places.push(PathBuf::from("/usr/bin/google-chrome"));
    }
    // On Windows how Chrome would be spoken to is undecided, as the agent's
    // socket is (ADR 0055 § 1), so it is looked for nowhere.
    places
}

/// The Chrome installed on this machine: the first place its installer puts
/// it, for the person whose home is `HOME`, that holds a program.
///
/// # Errors
///
/// [`NotInstalled`], naming every place looked at, when none does.
pub fn chrome_on_this_machine() -> Result<PathBuf, NotInstalled> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    chrome_in(&chrome_places(home.as_deref()))
}

/// The first of `places` that holds a program.
///
/// # Errors
///
/// [`NotInstalled`], naming every place looked at, when none does.
pub fn chrome_in(places: &[PathBuf]) -> Result<PathBuf, NotInstalled> {
    places
        .iter()
        .find(|place| is_program(place))
        .cloned()
        .ok_or_else(|| NotInstalled {
            looked: places.to_vec(),
        })
}

/// Whether `path` is a file this machine would run.
pub fn is_program(path: &Path) -> bool {
    let Ok(about) = std::fs::metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        about.is_file() && about.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        about.is_file()
    }
}

/// Chrome is not installed where it would be: a refusal of its own, rather
/// than a pipe that never answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotInstalled {
    /// Every place it was looked for, in order.
    pub looked: Vec<PathBuf>,
}

impl fmt::Display for NotInstalled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Chrome is not installed")?;
        let mut places = self.looked.iter();
        if let Some(first) = places.next() {
            write!(f, ": it was looked for at {}", first.display())?;
            for place in places {
                write!(f, ", {}", place.display())?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_renderer_is_in_the_same_directory() {
        assert_eq!(
            renderer_beside(Path::new("/opt/alo/bin/alo")),
            PathBuf::from("/opt/alo/bin/alo-render")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn chrome_is_looked_for_where_its_installer_puts_it() {
        assert_eq!(
            chrome_places(Some(Path::new("/Users/someone"))),
            [
                PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
                PathBuf::from(
                    "/Users/someone/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
                ),
            ]
        );
        assert_eq!(chrome_places(None).len(), 1, "no home, no place in it");
    }

    #[test]
    fn chrome_nowhere_is_refused_naming_every_place_looked() {
        let places = [
            PathBuf::from("/nowhere/a/chrome"),
            PathBuf::from("/nowhere/b/chrome"),
        ];
        let refused = chrome_in(&places).unwrap_err();
        assert_eq!(refused.looked, places);
        assert_eq!(
            refused.to_string(),
            "Chrome is not installed: it was looked for at /nowhere/a/chrome, /nowhere/b/chrome"
        );
        assert_eq!(
            chrome_in(&[]).unwrap_err().to_string(),
            "Chrome is not installed"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_or_a_file_nobody_may_run_is_not_chrome() {
        // A directory, and this source, which is not executable.
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let source = directory.join("src/beside.rs");
        let shell = PathBuf::from("/bin/sh");
        assert!(!is_program(&directory));
        assert!(!is_program(&source));
        assert_eq!(chrome_in(&[directory, source, shell.clone()]), Ok(shell));
    }
}
