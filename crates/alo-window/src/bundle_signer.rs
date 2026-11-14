/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Who signs the macOS application, and what `codesign` is told for each
//! signature (queue item 735; ADR 0079 § 4).
//!
//! Without `alo-bundle --sign` the application is sealed **ad hoc** —
//! nobody's identity, `codesign --sign - --force` — exactly as every build
//! before 735: valid, opened on the Mac that made it, and refused by
//! `spctl` anywhere else.
//!
//! `alo-bundle --sign "Developer ID Application: NAME (TEAMID)"` names the
//! owner's **Developer ID Application** identity in the login keychain,
//! by the certificate's whole common name, as Keychain Access and
//! `security find-identity -v -p codesigning` show it. The whole name and
//! nothing shorter, because it carries the Team ID every signature is then
//! checked against ([`Signer::team`]), and because `codesign` takes any
//! part of a name and so could quietly take another certificate. An
//! identity not for distribution — "Apple Development", "Mac Developer" —
//! is refused, as is `-` (ad hoc is what naming nothing gives). Every
//! signature made with it is under the hardened runtime (`--options
//! runtime`) with a secure timestamp from Apple (`--timestamp`), which
//! notarisation requires of each.

use alo_update::team::is_team_id;
use std::path::Path;

/// The start of a Developer ID Application certificate's common name.
pub const DEVELOPER_ID: &str = "Developer ID Application: ";

/// Who signs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signer {
    /// Nobody: sealed ad hoc.
    AdHoc,
    /// A Developer ID Application identity in the keychain.
    Identity {
        /// The certificate's whole common name.
        name: String,
        /// The Team ID at its end.
        team: String,
    },
}

impl Signer {
    /// The identity `named`, as `--sign` names it.
    ///
    /// # Errors
    ///
    /// Why it is refused, in words.
    pub fn named(named: &str) -> Result<Signer, String> {
        if named.is_empty() {
            return Err("--sign names no identity".to_owned());
        }
        if named == "-" {
            return Err(
                "--sign - is ad hoc, which is what not naming --sign gives: name a Developer ID \
                 Application identity"
                    .to_owned(),
            );
        }
        if named.chars().any(char::is_control) {
            return Err("--sign names an identity with a line break or a control in it".to_owned());
        }
        if !named.starts_with(DEVELOPER_ID) {
            return Err(format!(
                "--sign names {named:?}, which is not a Developer ID Application identity: only \
                 one of those signs an application for other Macs"
            ));
        }
        let team = named
            .strip_suffix(')')
            .and_then(|rest| rest.rsplit_once(" ("))
            .map(|(_, team)| team)
            .filter(|team| is_team_id(team))
            .ok_or_else(|| {
                format!(
                    "--sign names {named:?}, which does not end in its Team ID: name the \
                     certificate's whole common name, \"{DEVELOPER_ID}NAME (TEAMID)\""
                )
            })?;
        let holder = named
            .strip_prefix(DEVELOPER_ID)
            .and_then(|rest| rest.strip_suffix(&format!(" ({team})")))
            .unwrap_or_default();
        if holder.trim().is_empty() {
            return Err(format!("--sign names {named:?}, which names nobody"));
        }
        Ok(Signer::Identity {
            team: team.to_owned(),
            name: named.to_owned(),
        })
    }

    /// The Team ID every signature must name: none for ad hoc.
    pub fn team(&self) -> Option<&str> {
        match self {
            Signer::AdHoc => None,
            Signer::Identity { team, .. } => Some(team),
        }
    }

    /// What `codesign` is told to sign one piece of code, before its path:
    /// with the entitlements at `entitlements`, if any.
    pub fn options(&self, entitlements: Option<&Path>) -> Vec<String> {
        let mut options = match self {
            Signer::AdHoc => vec!["--sign".to_owned(), "-".to_owned(), "--force".to_owned()],
            Signer::Identity { name, .. } => vec![
                "--sign".to_owned(),
                name.clone(),
                "--force".to_owned(),
                "--options".to_owned(),
                "runtime".to_owned(),
                "--timestamp".to_owned(),
            ],
        };
        if let Some(entitlements) = entitlements {
            options.push("--entitlements".to_owned());
            options.push(entitlements.to_string_lossy().into_owned());
        }
        options
    }

    /// How it was sealed, as `alo-bundle` says at the end.
    pub fn said(&self) -> String {
        match self {
            Signer::AdHoc => "sealed ad hoc".to_owned(),
            Signer::Identity { team, .. } => format!(
                "signed with the Developer ID of the team {team} under the hardened runtime"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stranger's identity: never the owner's, and never in a keychain.
    const STRANGER: &str = "Developer ID Application: A Stranger (ABCDE12345)";

    #[test]
    fn a_developer_id_is_taken_with_the_team_id_at_its_end() {
        let signer = Signer::named(STRANGER).expect("taken");
        assert_eq!(signer.team(), Some("ABCDE12345"));
        assert_eq!(
            signer,
            Signer::Identity {
                name: STRANGER.to_owned(),
                team: "ABCDE12345".to_owned(),
            }
        );
        let parenthesised = "Developer ID Application: A (Small) Company Ltd (ZYXWV98765)";
        assert_eq!(
            Signer::named(parenthesised).expect("taken").team(),
            Some("ZYXWV98765")
        );
    }

    #[test]
    fn what_is_not_a_whole_developer_id_name_is_refused() {
        for (named, said) in [
            ("", "names no identity"),
            ("-", "is ad hoc"),
            (
                "Apple Development: A Stranger (ABCDE12345)",
                "not a Developer ID Application identity",
            ),
            ("ABCDE12345", "not a Developer ID Application identity"),
            (
                "Developer ID Application: A Stranger",
                "does not end in its Team ID",
            ),
            (
                "Developer ID Application: A Stranger (abcde12345)",
                "does not end in its Team ID",
            ),
            (
                "Developer ID Application: A Stranger (ABCDE1234)",
                "does not end in its Team ID",
            ),
            (
                "Developer ID Application: A Stranger ABCDE12345",
                "does not end in its Team ID",
            ),
            ("Developer ID Application:  (ABCDE12345)", "names nobody"),
            (
                "Developer ID Application: A\nStranger (ABCDE12345)",
                "a line break",
            ),
        ] {
            let refused = Signer::named(named);
            assert!(
                refused.as_ref().is_err_and(|why| why.contains(said)),
                "{named:?}: {refused:?}"
            );
        }
    }

    #[test]
    fn ad_hoc_is_told_exactly_what_it_was_told_before() {
        assert_eq!(Signer::AdHoc.options(None), ["--sign", "-", "--force"]);
        assert_eq!(
            Signer::AdHoc.options(Some(Path::new("/s/allow-jit.entitlements"))),
            [
                "--sign",
                "-",
                "--force",
                "--entitlements",
                "/s/allow-jit.entitlements"
            ]
        );
        assert_eq!(Signer::AdHoc.team(), None);
        assert_eq!(Signer::AdHoc.said(), "sealed ad hoc");
    }

    #[test]
    fn an_identity_signs_under_the_hardened_runtime_with_a_timestamp() {
        let signer = Signer::named(STRANGER).expect("taken");
        assert_eq!(
            signer.options(None),
            [
                "--sign",
                STRANGER,
                "--force",
                "--options",
                "runtime",
                "--timestamp"
            ]
        );
        assert_eq!(
            signer.options(Some(Path::new("/s/allow-jit.entitlements"))),
            [
                "--sign",
                STRANGER,
                "--force",
                "--options",
                "runtime",
                "--timestamp",
                "--entitlements",
                "/s/allow-jit.entitlements"
            ]
        );
        assert!(!signer.options(None).iter().any(|option| option == "--deep"));
        assert_eq!(
            signer.said(),
            "signed with the Developer ID of the team ABCDE12345 under the hardened runtime"
        );
    }
}
