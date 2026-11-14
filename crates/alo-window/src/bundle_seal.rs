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
//! hardened runtime, which a seal with nobody's identity is not given: it
//! is recorded in the seal, and has its effect once the application is
//! signed with a Developer ID.
//!
//! # With the owner's Developer ID (queue item 735)
//!
//! The same signatures in the same order, each made by the identity
//! `alo-bundle --sign` names ([`Signer`]) under the hardened runtime with a
//! timestamp, and never `--deep`. No other piece is given an entitlement —
//! not the host, not alo's own programs: each of Chromium's
//! `app-entitlements.plist` is decided in ADR 0079 § 4, and none is
//! carried yet. After `codesign --verify --deep --strict` accepts it,
//! every signature made is read back ([`alo_update::team`]): each must
//! name the identity's Team ID, so library validation passes with no
//! `disable-library-validation`, and each must be under the hardened
//! runtime ([`refusal`]).

use crate::bundle_signer::Signer;
use alo_update::team::{self, Signed};
use core::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Apple's signing tool, where macOS puts it; never looked for on `PATH`,
/// for the reason [`crate::beside`] gives for Chrome.
pub const CODESIGN: &str = team::CODESIGN;

/// Why the application is not sealed.
#[derive(Debug, PartialEq, Eq)]
pub enum NotSealed {
    /// `codesign` could not be run: its words.
    NoCodesign(String),
    /// `codesign` refused to sign this: what, and what it said.
    Signing(PathBuf, String),
    /// `codesign` signed it and its check refused it: what it said.
    Checked(String),
    /// A signature made names another team than the identity's, or none:
    /// what, the team it names, and the identity's.
    OtherTeam(PathBuf, Option<String>, String),
    /// A signature made with an identity is not under the hardened runtime.
    NoRuntime(PathBuf),
}

impl fmt::Display for NotSealed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCodesign(why) => write!(f, "{CODESIGN} could not be run: {why}"),
            Self::Signing(what, said) => {
                write!(f, "{} could not be signed: {said}", what.display())
            }
            Self::Checked(said) => write!(f, "the sealed application was refused: {said}"),
            Self::OtherTeam(what, named, wanted) => write!(
                f,
                "{} names the team {}, not {wanted}",
                what.display(),
                named.as_deref().unwrap_or("of nobody")
            ),
            Self::NoRuntime(what) => {
                write!(
                    f,
                    "{} is not signed under the hardened runtime",
                    what.display()
                )
            }
        }
    }
}

impl std::error::Error for NotSealed {}

/// One signature to make: the code, and whether it is given [`JIT`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signing {
    /// The program, library or bundle.
    pub code: PathBuf,
    /// Whether it may compile code as it runs.
    pub jit: bool,
}

impl Signing {
    fn plain(code: PathBuf) -> Self {
        Self { code, jit: false }
    }
}

/// The signatures `application` is given around what it carries, in
/// order: each program in `nested`, held beside the one it runs in
/// `Contents/MacOS`, then the application.
pub fn application_order(application: &Path, nested: &[&str]) -> Vec<Signing> {
    let held = application.join("Contents").join("MacOS");
    nested
        .iter()
        .map(|program| Signing::plain(held.join(program)))
        .chain([Signing::plain(application.to_path_buf())])
        .collect()
}

/// Seal `application`, holding `nested` beside the program it runs in
/// `Contents/MacOS`, ad hoc, and check it.
///
/// # Errors
///
/// [`NotSealed`]: `codesign` not run, refusing to sign, or refusing what it
/// signed.
pub fn seal(application: &Path, nested: &[&str]) -> Result<(), NotSealed> {
    seal_by(&Signer::AdHoc, application, nested, &[])
}

/// Seal `application` as [`seal`] does, signed by `signer`, and check it;
/// with an identity, check too that every signature made — `carried`'s,
/// made before by [`seal_carried`], and these — names its team and the
/// hardened runtime.
///
/// # Errors
///
/// [`NotSealed`]: `codesign` not run, refusing to sign, or refusing what it
/// signed, or a signature naming another team or no hardened runtime.
pub fn seal_by(
    signer: &Signer,
    application: &Path,
    nested: &[&str],
    carried: &[Signing],
) -> Result<(), NotSealed> {
    let order = application_order(application, nested);
    for signing in &order {
        sign(signer, signing, None)?;
    }
    check(application)?;
    let Some(wanted) = signer.team() else {
        return Ok(());
    };
    for signing in carried.iter().chain(&order) {
        let read = team::of(&signing.code)
            .map_err(|said| NotSealed::Checked(format!("{}: {said}", signing.code.display())))?;
        if let Some(refused) = refusal(wanted, &signing.code, &read) {
            return Err(refused);
        }
    }
    Ok(())
}

/// Why the signature `signed` of `code` is not one the identity of the
/// team `wanted` made under the hardened runtime, if it is not.
pub fn refusal(wanted: &str, code: &Path, signed: &Signed) -> Option<NotSealed> {
    if signed.team.as_deref() != Some(wanted) {
        return Some(NotSealed::OtherTeam(
            code.to_path_buf(),
            signed.team.clone(),
            wanted.to_owned(),
        ));
    }
    (!signed.runtime).then(|| NotSealed::NoRuntime(code.to_path_buf()))
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

/// The signatures alo's CEF host's application at `host` is given, in
/// order, inside out: each of `libraries` (the framework's), the
/// framework, each helper — the GPU's and the renderer's with [`JIT`] —
/// and the host's application.
pub fn carried_order(host: &Path, libraries: &[PathBuf]) -> Vec<Signing> {
    let frameworks = host.join("Contents").join("Frameworks");
    let mut order: Vec<Signing> = libraries.iter().cloned().map(Signing::plain).collect();
    order.push(Signing::plain(frameworks.join(crate::cef_host::FRAMEWORK)));
    for (kind, _) in crate::cef_host::HELPERS {
        order.push(Signing {
            code: frameworks.join(format!("{}.app", crate::cef_host::helper(kind))),
            jit: JIT_HELPERS.contains(&kind),
        });
    }
    order.push(Signing::plain(host.to_path_buf()));
    order
}

/// Seal alo's CEF host's application at `host`, inside out, signed by
/// `signer`, before the application around it is sealed; `scratch` is a
/// directory outside the application to write [`JIT`] in. The signatures
/// made, in order.
///
/// # Errors
///
/// [`NotSealed`]: `codesign` not run or refusing to sign, or the
/// entitlements or the framework's libraries not written or read.
pub fn seal_carried(
    signer: &Signer,
    host: &Path,
    scratch: &Path,
) -> Result<Vec<Signing>, NotSealed> {
    let libraries = host
        .join("Contents")
        .join("Frameworks")
        .join(crate::cef_host::FRAMEWORK)
        .join("Libraries");
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
    let jit = scratch.join("allow-jit.entitlements");
    std::fs::write(&jit, JIT).map_err(|why| NotSealed::Signing(jit.clone(), why.to_string()))?;
    let order = carried_order(host, &held);
    for signing in &order {
        sign(signer, signing, signing.jit.then_some(jit.as_path()))?;
    }
    Ok(order)
}

/// Sign `signing`'s code by `signer`, replacing the signature `ld` gave it,
/// with the entitlements at `entitlements`, if any.
fn sign(signer: &Signer, signing: &Signing, entitlements: Option<&Path>) -> Result<(), NotSealed> {
    let options = signer.options(entitlements);
    let options: Vec<&str> = options.iter().map(String::as_str).collect();
    let said = codesign(&options, &signing.code)?;
    said.map_err(|said| NotSealed::Signing(signing.code.clone(), said))
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
    fn the_carried_host_is_signed_inside_out_with_jit_for_the_gpu_and_renderer_only() {
        let host = Path::new("/A/alo browser.app/Contents/Frameworks/alo Chromium.app");
        let frameworks = host.join("Contents/Frameworks");
        let framework = frameworks.join(crate::cef_host::FRAMEWORK);
        let libraries = [
            framework.join("Libraries/libcef_sandbox.dylib"),
            framework.join("Libraries/libvulkan.dylib"),
        ];
        let order = carried_order(host, &libraries);
        let said: Vec<(String, bool)> = order
            .iter()
            .map(|signing| {
                let code = signing
                    .code
                    .strip_prefix(host)
                    .map_or_else(|_| "?".to_owned(), |code| code.display().to_string());
                (code, signing.jit)
            })
            .collect();
        let framework = format!("Contents/Frameworks/{}", crate::cef_host::FRAMEWORK);
        let wanted: Vec<(String, bool)> = [
            (format!("{framework}/Libraries/libcef_sandbox.dylib"), false),
            (format!("{framework}/Libraries/libvulkan.dylib"), false),
            (framework.clone(), false),
            (
                "Contents/Frameworks/alo Chromium Helper.app".to_owned(),
                false,
            ),
            (
                "Contents/Frameworks/alo Chromium Helper (GPU).app".to_owned(),
                true,
            ),
            (
                "Contents/Frameworks/alo Chromium Helper (Renderer).app".to_owned(),
                true,
            ),
            (
                "Contents/Frameworks/alo Chromium Helper (Plugin).app".to_owned(),
                false,
            ),
            (
                "Contents/Frameworks/alo Chromium Helper (Alerts).app".to_owned(),
                false,
            ),
            (String::new(), false),
        ]
        .into_iter()
        .collect();
        assert_eq!(said, wanted, "libraries, framework, helpers, then the host");
    }

    #[test]
    fn alos_own_programs_are_signed_before_the_application_and_none_with_jit() {
        let application = Path::new("/A/alo browser.app");
        assert_eq!(
            application_order(application, &["alo-render", "alo-ask"]),
            [
                Signing::plain(application.join("Contents/MacOS/alo-render")),
                Signing::plain(application.join("Contents/MacOS/alo-ask")),
                Signing::plain(application.to_path_buf()),
            ]
        );
    }

    #[test]
    fn each_signature_made_must_name_the_identitys_team_and_the_runtime() {
        let code = Path::new("/A/alo browser.app");
        let read = |team: &str, flags: &str| {
            team::read(&format!(
                "Executable=/A/alo browser.app/Contents/MacOS/alo\n\
                 CodeDirectory v=20500 size=765 flags={flags} hashes=13+7 location=embedded\n\
                 TeamIdentifier={team}\n"
            ))
            .expect("read")
        };
        assert_eq!(
            refusal("ABCDE12345", code, &read("ABCDE12345", "0x10000(runtime)")),
            None
        );
        assert_eq!(
            refusal("ABCDE12345", code, &read("ZYXWV98765", "0x10000(runtime)")),
            Some(NotSealed::OtherTeam(
                code.to_path_buf(),
                Some("ZYXWV98765".to_owned()),
                "ABCDE12345".to_owned()
            ))
        );
        let ad_hoc = refusal("ABCDE12345", code, &read("not set", "0x2(adhoc)"));
        assert_eq!(
            ad_hoc.map(|refused| refused.to_string()),
            Some("/A/alo browser.app names the team of nobody, not ABCDE12345".to_owned())
        );
        assert_eq!(
            refusal("ABCDE12345", code, &read("ABCDE12345", "0x0(none)")),
            Some(NotSealed::NoRuntime(code.to_path_buf()))
        );
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
