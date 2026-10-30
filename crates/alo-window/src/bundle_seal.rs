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
//!
//! # The Chromium it carries, sealed inside out (queue item 720)
//!
//! An application carrying alo's CEF host ([`crate::bundle_carried`]) has
//! code nested three deep, and `codesign` seals a bundle only over nested
//! code already signed, so [`seal_carried`] signs it from the inside out,
//! as CEF's guide to signing says (ADR 0079 § 4): each library in CEF's
//! framework's `Libraries`, the framework, each helper, then the host's
//! own application — before [`seal`] signs the application around it. CEF's
//! framework comes signed only by its linker, which `codesign --verify
//! --strict` refuses ("code has no resources but signature indicates they
//! must be present"), so it is signed again here, ad hoc like the rest.
//!
//! The GPU and renderer helpers are given the one entitlement CEF's guide
//! gives them, `com.apple.security.cs.allow-jit` ([`JIT`]), and the others
//! none. An entitlement of the hardened runtime is enforced only under the
//! hardened runtime, which a seal with nobody's identity is not given here:
//! it is recorded in the seal, and has its effect once the application is
//! signed with the owner's Developer ID and notarised — queue item 735's,
//! blocked on that certificate.

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

/// The helpers CEF's guide gives the JIT entitlement: the GPU's and the
/// renderer's, as the kind after ` Helper` in their names.
pub const JIT_HELPERS: [&str; 2] = [" (GPU)", " (Renderer)"];

/// The entitlements of a helper that compiles code as it runs.
pub const JIT: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
\t<key>com.apple.security.cs.allow-jit</key>
\t<true/>
</dict>
</plist>
";

/// Seal alo's CEF host's application at `host`, inside out, before the
/// application around it is sealed; `scratch` is a directory outside the
/// application to write [`JIT`] in.
///
/// # Errors
///
/// [`NotSealed`]: `codesign` not run or refusing to sign, or the
/// entitlements or the framework's libraries not written or read.
pub fn seal_carried(host: &Path, scratch: &Path) -> Result<(), NotSealed> {
    let frameworks = host.join("Contents").join("Frameworks");
    let framework = frameworks.join(crate::cef_host::FRAMEWORK);
    let libraries = framework.join("Libraries");
    let mut held: Vec<PathBuf> = std::fs::read_dir(&libraries)
        .map_err(|why| NotSealed::Signing(libraries.clone(), why.to_string()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "dylib")
        })
        .collect();
    held.sort();
    for library in &held {
        sign(library)?;
    }
    sign(&framework)?;
    let jit = scratch.join("allow-jit.entitlements");
    std::fs::write(&jit, JIT).map_err(|why| NotSealed::Signing(jit.clone(), why.to_string()))?;
    for (kind, _) in crate::cef_host::HELPERS {
        let helper = frameworks.join(format!("{}.app", crate::cef_host::helper(kind)));
        if JIT_HELPERS.contains(&kind) {
            let jit = jit.to_string_lossy();
            let said = codesign(&["--sign", "-", "--force", "--entitlements", &jit], &helper)?;
            said.map_err(|said| NotSealed::Signing(helper.clone(), said))?;
        } else {
            sign(&helper)?;
        }
    }
    sign(host)
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
    fn only_the_gpu_and_renderer_helpers_may_compile_code_as_they_run() {
        let kinds: Vec<&str> = crate::cef_host::HELPERS
            .iter()
            .map(|(kind, _)| *kind)
            .filter(|kind| JIT_HELPERS.contains(kind))
            .collect();
        assert_eq!(kinds, [" (GPU)", " (Renderer)"], "each one a helper's kind");
        assert!(JIT.contains("<key>com.apple.security.cs.allow-jit</key>\n\t<true/>"));
        assert_eq!(JIT.matches("<key>").count(), 1, "that one, and no other");
    }

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
