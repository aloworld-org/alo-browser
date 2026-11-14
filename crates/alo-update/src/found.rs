/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a check found, and how the person is told (ADR 0116 § 4).
//!
//! *Settings › Updates* says it always, beside when alo last checked; the
//! window's own line — never inside a page — says it when an update is
//! ready ([`Found::ready_line`]) or alo cannot keep up
//! ([`missing_fixes`]).

use crate::instant::Instant;
use crate::version::Version;

/// How long alo may go without believing a manifest before it says its
/// Chromium may be missing security fixes: three days.
pub const STALE_AFTER: u64 = 3 * 24 * 60 * 60;

/// What a check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// The newest release is the one installed.
    UpToDate {
        /// The version installed.
        version: Version,
        /// Whether the release says CEF is behind Chrome's security fixes.
        behind: bool,
    },
    /// A newer one is downloaded, checked and unpacked: it installs at quit.
    Ready {
        /// Its version.
        version: Version,
        /// The Chromium it carries.
        chromium: String,
        /// Whether it carries Chrome's security fixes.
        security: bool,
        /// Whether the release says CEF is behind Chrome's security fixes.
        behind: bool,
    },
    /// A newer one is coming: its download stopped and resumes next time.
    Coming(String),
    /// Something was refused, or cannot be done here: said, and recorded.
    Refused(String),
    /// alo could not check: no network, or an expired manifest.
    CouldNotCheck(String),
}

impl Found {
    /// What it says, in a sentence or two.
    pub fn said(&self) -> String {
        match self {
            Found::UpToDate { version, behind } => {
                let mut said = format!("alo {version} is up to date.");
                if *behind {
                    said.push(' ');
                    said.push_str(BEHIND);
                }
                said
            }
            Found::Ready { behind, .. } => {
                let mut said = self.ready_line().unwrap_or_default();
                if *behind {
                    said.push(' ');
                    said.push_str(BEHIND);
                }
                said
            }
            Found::Coming(said) | Found::Refused(said) | Found::CouldNotCheck(said) => said.clone(),
        }
    }

    /// The line the window shows when an update is ready, if one is.
    pub fn ready_line(&self) -> Option<String> {
        let Found::Ready {
            version,
            chromium,
            security,
            ..
        } = self
        else {
            return None;
        };
        Some(if *security {
            format!(
                "alo {version}, a security update with Chromium {chromium}, is ready. It \
                 installs when you quit alo."
            )
        } else {
            format!("alo {version} is ready. It installs when you quit alo.")
        })
    }

    /// Whether a manifest was believed on the way to this.
    pub fn believed(&self) -> bool {
        matches!(
            self,
            Found::UpToDate { .. } | Found::Ready { .. } | Found::Coming(_)
        )
    }
}

/// What is said when a release says CEF is behind.
pub const BEHIND: &str = "alo\u{2019}s Chromium may be missing security fixes: Chromium\u{2019}s \
     newest security release has no build for alo yet.";

/// What is said when alo has not believed a manifest for more than three
/// days, if it has not: `believed` is when it last did, if ever, `since`
/// when it began trying (its first launch with checks on), and `now` now.
pub fn missing_fixes(believed: Option<Instant>, since: Instant, now: Instant) -> Option<String> {
    let from = believed.unwrap_or(since);
    (now.seconds().saturating_sub(from.seconds()) > STALE_AFTER).then(|| {
        "alo\u{2019}s Chromium may be missing security fixes: alo has not been able to check \
         for updates for more than three days."
            .to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ready_update_says_it_installs_at_quit_and_a_security_one_names_chromium() {
        let ready = Found::Ready {
            version: Version::of(0, 4, 2),
            chromium: "156.0.8078.96".into(),
            security: false,
            behind: false,
        };
        assert_eq!(
            ready.said(),
            "alo 0.4.2 is ready. It installs when you quit alo."
        );
        let security = Found::Ready {
            version: Version::of(0, 4, 2),
            chromium: "156.0.8078.96".into(),
            security: true,
            behind: true,
        };
        assert_eq!(
            security.ready_line().as_deref(),
            Some(
                "alo 0.4.2, a security update with Chromium 156.0.8078.96, is ready. It \
                 installs when you quit alo."
            )
        );
        assert!(security.said().ends_with(BEHIND));
        let up = Found::UpToDate {
            version: Version::of(0, 4, 2),
            behind: false,
        };
        assert_eq!(up.said(), "alo 0.4.2 is up to date.");
        assert_eq!(up.ready_line(), None);
    }

    #[test]
    fn more_than_three_days_without_believing_a_manifest_is_said() {
        let day = 24 * 60 * 60;
        let start = Instant::at(1_794_646_800);
        let at = |days: u64, extra: u64| Instant::at(start.seconds() + days * day + extra);
        assert_eq!(missing_fixes(Some(start), start, at(3, 0)), None);
        assert!(missing_fixes(Some(start), start, at(3, 1)).is_some());
        assert_eq!(missing_fixes(None, start, at(2, 0)), None);
        assert!(missing_fixes(None, start, at(4, 0)).is_some());
        assert_eq!(missing_fixes(Some(at(3, 0)), start, at(4, 0)), None);
    }
}
