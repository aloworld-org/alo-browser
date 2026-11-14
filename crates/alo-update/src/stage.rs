/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The checked package, unpacked beside the installed application and
//! checked again, ready to be swapped in at quit (ADR 0116 § 3).
//!
//! # Beside it, on the same volume
//!
//! Into a hidden folder in the folder that holds `alo browser.app` —
//! [`HIDDEN`], then the version — so the swap is a rename on one volume.
//! The archive is unpacked with `/usr/bin/ditto -x -k`, macOS's own, which
//! keeps a bundle's symbolic links and its signature.
//!
//! # What the unpacked bundle must be
//!
//! - one bundle of the installed one's own name, a folder and not a link;
//! - one that answers `/usr/bin/codesign --verify --strict`;
//! - one whose `CFBundleShortVersionString` is the manifest's `version`,
//!   read with `/usr/bin/plutil`.
//!
//! Once Developer ID is done (queue item 735), its Team ID must also be the
//! running application's; until then every alo is sealed ad hoc and has
//! none, so that check is not made — and not pretended.
//!
//! Only then is a `ready` file written in its folder, last, saying the
//! version: what [`crate::quitting`] swaps in. A folder without one is an
//! unpacking that did not finish, or the old application after a swap, and
//! is removed at the next launch ([`sweep`]).
//!
//! # Never more rights than the person has
//!
//! A folder the person cannot write (a standard account and
//! `/Applications`), or an application macOS runs from a translocated copy,
//! is not updated: alo says why and what to do. There is no privileged
//! helper and no password prompt.

use crate::version::Version;
use core::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The hidden folder beside the installed application.
pub const HIDDEN: &str = ".alo-update";

/// The file whose presence says an unpacked bundle is checked.
pub const READY: &str = "ready";

/// Why a package was not made ready.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unstaged {
    /// The application is where alo may not write, or is translocated.
    NotWritable {
        /// What the person is told to do.
        why: String,
    },
    /// The package, unpacked, is not what an update must be.
    Refused {
        /// Why, in words.
        why: String,
    },
}

impl fmt::Display for Unstaged {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unstaged::NotWritable { why } => f.write_str(why),
            Unstaged::Refused { why } => write!(f, "The update was refused: {why}."),
        }
    }
}

/// A bundle unpacked and checked, ready to be swapped in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staged {
    /// The unpacked bundle.
    pub bundle: PathBuf,
    /// Its version.
    pub version: Version,
}

/// The folder updates for `installed` are unpacked in.
pub fn hidden_beside(installed: &Path) -> Option<PathBuf> {
    Some(installed.parent()?.join(HIDDEN))
}

/// Unpack `package` beside `installed` and check it is `version`: the
/// staged bundle.
///
/// # Errors
///
/// [`Unstaged`], saying why.
pub fn unpack(package: &Path, installed: &Path, version: Version) -> Result<Staged, Unstaged> {
    let (hidden, name) = writable(installed)?;
    let folder = hidden.join(version.to_string());
    let refused = |why: String| {
        let _ = fs::remove_dir_all(&folder);
        Unstaged::Refused { why }
    };
    let _ = fs::remove_dir_all(&folder);
    fs::create_dir_all(&folder).map_err(|why| Unstaged::NotWritable {
        why: cannot_write(&hidden, &why.to_string()),
    })?;
    run(Command::new("/usr/bin/ditto")
        .arg("-x")
        .arg("-k")
        .arg(package)
        .arg(&folder))
    .map_err(|why| refused(format!("the package does not unpack ({why})")))?;
    let bundle = folder.join(&name);
    let meta = fs::symlink_metadata(&bundle)
        .map_err(|_| refused(format!("the package holds no {}", name.to_string_lossy())))?;
    if !meta.is_dir() {
        return Err(refused(format!(
            "the package\u{2019}s {} is not a folder",
            name.to_string_lossy()
        )));
    }
    let entries = fs::read_dir(&folder).map_or(0, Iterator::count);
    if entries != 1 {
        return Err(refused(
            "the package holds more than the application".to_owned(),
        ));
    }
    verified(&bundle, version).map_err(refused)?;
    fs::write(folder.join(READY), format!("{version}\n"))
        .map_err(|why| refused(format!("its ready mark could not be written ({why})")))?;
    Ok(Staged { bundle, version })
}

/// Whether `bundle` answers `codesign --verify --strict` and is `version`.
///
/// # Errors
///
/// Why not, in words.
pub fn verified(bundle: &Path, version: Version) -> Result<(), String> {
    run(Command::new("/usr/bin/codesign")
        .arg("--verify")
        .arg("--strict")
        .arg(bundle))
    .map_err(|why| format!("its signature does not verify ({why})"))?;
    let said = run(Command::new("/usr/bin/plutil")
        .arg("-extract")
        .arg("CFBundleShortVersionString")
        .arg("raw")
        .arg("-o")
        .arg("-")
        .arg(bundle.join("Contents").join("Info.plist")))
    .map_err(|why| format!("it says no version ({why})"))?;
    match Version::read(said.trim()) {
        Some(found) if found == version => Ok(()),
        _ => Err(format!(
            "it says it is {:?}, and the manifest {version}",
            said.trim().chars().take(40).collect::<String>()
        )),
    }
}

/// The staged bundle beside `installed` marked ready, if there is one: the
/// newest, if there are several.
pub fn ready_beside(installed: &Path) -> Option<Staged> {
    let hidden = hidden_beside(installed)?;
    let name = installed.file_name()?;
    fs::read_dir(&hidden)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let folder = entry.path();
            let said = fs::read_to_string(folder.join(READY)).ok()?;
            let version = Version::read(said.trim())?;
            let named = Version::read(&entry.file_name().to_string_lossy())?;
            (named == version).then(|| Staged {
                bundle: folder.join(name),
                version,
            })
        })
        .max_by_key(|staged| staged.version)
}

/// Remove what is beside `installed` and not ready: an unpacking that did
/// not finish, or the old application after a swap. Whether anything was.
pub fn sweep(installed: &Path) -> bool {
    let Some(hidden) = hidden_beside(installed) else {
        return false;
    };
    let Ok(entries) = fs::read_dir(&hidden) else {
        return false;
    };
    let mut swept = false;
    for entry in entries.filter_map(Result::ok) {
        let folder = entry.path();
        if !folder.join(READY).is_file() {
            swept |= fs::remove_dir_all(&folder).is_ok();
        }
    }
    // Empty now? Then the hidden folder goes too.
    let _ = fs::remove_dir(&hidden);
    swept
}

/// The hidden folder and the application's name, if alo may write beside
/// `installed`.
fn writable(installed: &Path) -> Result<(PathBuf, std::ffi::OsString), Unstaged> {
    let text = installed.to_string_lossy();
    if text.contains("/AppTranslocation/") {
        return Err(Unstaged::NotWritable {
            why: "alo cannot update itself while macOS runs it from a temporary copy. Move alo \
                  to your Applications folder, or another folder you can write, and open it \
                  again."
                .to_owned(),
        });
    }
    let (Some(hidden), Some(name)) = (hidden_beside(installed), installed.file_name()) else {
        return Err(Unstaged::NotWritable {
            why: "alo does not know which folder it is in.".to_owned(),
        });
    };
    fs::create_dir_all(&hidden).map_err(|why| Unstaged::NotWritable {
        why: cannot_write(&hidden, &why.to_string()),
    })?;
    Ok((hidden, name.to_owned()))
}

/// What the person is told when the folder cannot be written.
fn cannot_write(hidden: &Path, why: &str) -> String {
    let folder = hidden.parent().unwrap_or(hidden);
    format!(
        "alo cannot update itself: it cannot write in {} ({why}). Move alo to a folder you \
         can write, or ask the computer\u{2019}s administrator.",
        folder.display()
    )
}

/// Run `command`, its standard output if it succeeded.
fn run(command: &mut Command) -> Result<String, String> {
    let output = command.output().map_err(|why| why.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let said = String::from_utf8_lossy(&output.stderr);
        Err(said
            .lines()
            .next()
            .unwrap_or("it failed")
            .chars()
            .take(200)
            .collect())
    }
}
