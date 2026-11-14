/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Whose signature a bundle carries: the Team ID `codesign -dv` names
//! (queue item 735; ADR 0079 § 4, ADR 0116 § 3).
//!
//! A Developer ID signature names the team the certificate was issued to,
//! as a line `TeamIdentifier=55G4593FM8` among what `codesign -dv` writes
//! on its error output; a signature with nobody's identity (`codesign
//! --sign -`) and Apple's own programs say `TeamIdentifier=not set`. The
//! same text says whether the code is signed under the hardened runtime:
//! `runtime` among the `flags=` of its `CodeDirectory` line.
//!
//! Two things read it: `alo-bundle`, which checks that every signature it
//! made names the one team it signed with, so library validation passes;
//! and the updater, which takes a staged update only if it names the same
//! team as the application running ([`same_team`]).

use std::path::Path;
use std::process::Command;

/// Apple's signing tool, where macOS puts it.
pub const CODESIGN: &str = "/usr/bin/codesign";

/// What a signature says of itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signed {
    /// The Team ID it names, if any: none for an ad hoc signature.
    pub team: Option<String>,
    /// Whether it is signed under the hardened runtime.
    pub runtime: bool,
}

/// Whether `text` is a Team ID: ten capital letters and digits.
pub fn is_team_id(text: &str) -> bool {
    text.len() == 10
        && text
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
}

/// What `said`, the text `codesign -dv` wrote, says of the signature; none
/// if it is not such a text — no `TeamIdentifier=` line, or one that is
/// neither a Team ID nor `not set`.
pub fn read(said: &str) -> Option<Signed> {
    let mut team = None;
    let mut runtime = false;
    for line in said.lines() {
        if let Some(named) = line.strip_prefix("TeamIdentifier=") {
            team = Some(match named.trim() {
                "not set" => None,
                id if is_team_id(id) => Some(id.to_owned()),
                _ => return None,
            });
        } else if let Some(rest) = line.strip_prefix("CodeDirectory ") {
            runtime = rest
                .split_whitespace()
                .filter_map(|word| word.strip_prefix("flags="))
                .any(|flags| {
                    flags
                        .split_once('(')
                        .and_then(|(_, named)| named.strip_suffix(')'))
                        .is_some_and(|named| named.split(',').any(|flag| flag == "runtime"))
                });
        }
    }
    Some(Signed {
        team: team?,
        runtime,
    })
}

/// The signature of the code at `path`, as `codesign -dv` reads it.
///
/// # Errors
///
/// `codesign`'s words, when it could not be run, refused (code not signed
/// at all), or wrote nothing this reads.
pub fn of(path: &Path) -> Result<Signed, String> {
    let ran = Command::new(CODESIGN)
        .arg("-dv")
        .arg(path)
        .output()
        .map_err(|why| format!("{CODESIGN} could not be run: {why}"))?;
    let said = String::from_utf8_lossy(&ran.stderr);
    if !ran.status.success() {
        return Err(said.lines().next().unwrap_or("it failed").to_owned());
    }
    read(&said).ok_or_else(|| "codesign named no Team ID".to_owned())
}

/// Whether `staged` may replace `installed`, by whose signature each one
/// carries: when the installed application names a team, the staged one
/// must name the same. One that names none — sealed ad hoc, as every alo
/// built from this repository without `--sign` is, or not signed at all —
/// has no team to hold an update to, and this check is not made, nor
/// pretended (ADR 0116 § 3).
///
/// # Errors
///
/// Why not, in words.
pub fn same_team(installed: &Path, staged: &Path) -> Result<(), String> {
    let Some(running) = of(installed).ok().and_then(|signed| signed.team) else {
        return Ok(());
    };
    held(&running, of(staged)?.team.as_deref())
}

/// Whether an update naming the team `staged` may replace an application
/// signed by the team `running`.
///
/// # Errors
///
/// Why not, in words.
pub fn held(running: &str, staged: Option<&str>) -> Result<(), String> {
    match staged {
        Some(team) if team == running => Ok(()),
        Some(team) => Err(format!(
            "it is signed by the team {team}, and this alo by {running}"
        )),
        None => Err(format!(
            "it names no team, and this alo is signed by {running}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What `codesign -dv` wrote for an application signed with a
    /// Developer ID under the hardened runtime (a stranger's Team ID).
    const DEVELOPER_ID: &str = "Executable=/A/alo browser.app/Contents/MacOS/alo
Identifier=org.alo.browser
Format=app bundle with Mach-O thin (arm64)
CodeDirectory v=20500 size=765 flags=0x10000(runtime) hashes=13+7 location=embedded
Signature size=8990
Timestamp=14 Nov 2026 at 10:04:08
Info.plist entries=12
TeamIdentifier=ABCDE12345
Runtime Version=26.0.0
Sealed Resources version=2 rules=13 files=63
Internal requirements count=1 size=200
";

    /// What it wrote for one sealed ad hoc, as `alo-bundle` seals today.
    const AD_HOC: &str = "Executable=/A/alo browser.app/Contents/MacOS/alo
Identifier=alo
Format=app bundle with Mach-O thin (arm64)
CodeDirectory v=20400 size=500 flags=0x2(adhoc) hashes=9+7 location=embedded
Signature=adhoc
Info.plist entries=12
TeamIdentifier=not set
Sealed Resources version=2 rules=13 files=63
Internal requirements count=0 size=12
";

    #[test]
    fn a_developer_id_signature_names_its_team_and_the_runtime() {
        assert_eq!(
            read(DEVELOPER_ID),
            Some(Signed {
                team: Some("ABCDE12345".to_owned()),
                runtime: true,
            })
        );
    }

    #[test]
    fn an_ad_hoc_signature_names_no_team_and_no_runtime() {
        assert_eq!(
            read(AD_HOC),
            Some(Signed {
                team: None,
                runtime: false,
            })
        );
    }

    #[test]
    fn the_runtime_is_read_among_other_flags_and_only_as_a_whole_word() {
        let several = DEVELOPER_ID.replace(
            "flags=0x10000(runtime)",
            "flags=0x12a00(kill,restrict,library-validation,runtime)",
        );
        assert!(read(&several).is_some_and(|signed| signed.runtime));
        let other = DEVELOPER_ID.replace("flags=0x10000(runtime)", "flags=0x2000(runtimes)");
        assert!(read(&other).is_some_and(|signed| !signed.runtime));
    }

    #[test]
    fn a_text_without_a_team_or_with_a_wrong_one_is_not_read() {
        assert_eq!(read(""), None);
        assert_eq!(read("code object is not signed at all\n"), None);
        for wrong in ["abcde12345", "ABCDE1234", "ABCDE123456", "ABCDE-1234"] {
            let text = DEVELOPER_ID.replace("ABCDE12345", wrong);
            assert_eq!(read(&text), None, "{wrong}");
        }
    }

    #[test]
    fn a_team_id_is_ten_capitals_and_digits() {
        assert!(is_team_id("ABCDE12345"));
        assert!(is_team_id("0123456789"));
        assert!(!is_team_id(""));
        assert!(!is_team_id("ABCDE 2345"));
    }

    #[test]
    fn an_update_is_held_to_the_running_applications_team() {
        assert_eq!(held("ABCDE12345", Some("ABCDE12345")), Ok(()));
        assert_eq!(
            held("ABCDE12345", Some("ZYXWV98765")),
            Err("it is signed by the team ZYXWV98765, and this alo by ABCDE12345".to_owned())
        );
        assert_eq!(
            held("ABCDE12345", None),
            Err("it names no team, and this alo is signed by ABCDE12345".to_owned())
        );
    }

    #[test]
    fn an_unsigned_installed_application_holds_an_update_to_no_team() {
        // A path with no code at all: `codesign` refuses it, so the
        // installed application names no team and no check is made.
        let nowhere = Path::new("/nowhere/alo browser.app");
        assert_eq!(same_team(nowhere, nowhere), Ok(()));
        assert!(of(nowhere).is_err());
    }
}
