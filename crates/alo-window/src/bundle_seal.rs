/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The macOS application sealed ad hoc, and checked (queue item 687; ADR
//! 0067 § 6).
//!
//! Unsealed, a bundle fails `codesign --verify` — `ld` signs each program
//! and nothing signs the bundle around them (ADR 0067 § 1, measured). So
//! each program the application holds beside the one it runs is signed
//! first, as code nested in it, and then the application whole, with
//! Apple's own `codesign` and nobody's identity (`-s -`): that seals its
//! files, so a page or a program changed after is known to be. It is
//! **valid, not trusted**: `spctl` rejects it, and a Developer ID and
//! notarisation are item 478's.
//!
//! Then it is checked as strictly as `codesign` checks — every nested
//! program and every sealed file — and a refusal is said in `codesign`'s
//! own words.

use core::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Apple's signing tool, where macOS puts it; never looked for on `PATH`,
/// for the reason [`crate::beside`] gives for Chrome.
pub const CODESIGN: &str = "/usr/bin/codesign";

/// Why the application is not sealed.
#[derive(Debug, PartialEq, Eq)]
pub enum NotSealed {
    /// `codesign` could not be run: its words.
    NoCodesign(String),
    /// `codesign` refused to sign this: what, and what it said.
    Signing(PathBuf, String),
    /// `codesign` signed it and its check refused it: what it said.
    Checked(String),
}

impl fmt::Display for NotSealed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCodesign(why) => write!(f, "{CODESIGN} could not be run: {why}"),
            Self::Signing(what, said) => {
                write!(f, "{} could not be signed: {said}", what.display())
            }
            Self::Checked(said) => write!(f, "the sealed application was refused: {said}"),
        }
    }
}

impl std::error::Error for NotSealed {}

/// Seal `application`, holding `nested` beside the program it runs in
/// `Contents/MacOS`, and check it.
///
/// # Errors
///
/// [`NotSealed`]: `codesign` not run, refusing to sign, or refusing what it
/// signed.
pub fn seal(application: &Path, nested: &[&str]) -> Result<(), NotSealed> {
    let held = application.join("Contents").join("MacOS");
    for program in nested {
        sign(&held.join(program))?;
    }
    sign(application)?;
    check(application)
}

/// Sign `what` ad hoc, replacing the signature `ld` gave it.
fn sign(what: &Path) -> Result<(), NotSealed> {
    let said = codesign(&["--sign", "-", "--force"], what)?;
    said.map_err(|said| NotSealed::Signing(what.to_path_buf(), said))
}

/// Check `application`'s seal, its nested programs' among it.
///
/// # Errors
///
/// [`NotSealed`]: `codesign` not run, or what it said refusing it.
pub fn check(application: &Path) -> Result<(), NotSealed> {
    codesign(&["--verify", "--deep", "--strict"], application)?.map_err(NotSealed::Checked)
}

/// Run `codesign` with `options` on `path`: whether it agreed, or what it
/// said refusing.
fn codesign(options: &[&str], path: &Path) -> Result<Result<(), String>, NotSealed> {
    let ran = Command::new(CODESIGN)
        .args(options)
        .arg(path)
        .output()
        .map_err(|why| NotSealed::NoCodesign(why.to_string()))?;
    Ok(if ran.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&ran.stderr).trim().to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refusal_names_what_and_says_codesigns_words() {
        assert_eq!(
            NotSealed::Signing(PathBuf::from("/a.app"), "no".to_owned()).to_string(),
            "/a.app could not be signed: no"
        );
        assert_eq!(
            NotSealed::Checked("a sealed resource is missing".to_owned()).to_string(),
            "the sealed application was refused: a sealed resource is missing"
        );
        assert!(
            NotSealed::NoCodesign("gone".to_owned())
                .to_string()
                .starts_with("/usr/bin/codesign could not be run")
        );
    }
}
