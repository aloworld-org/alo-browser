/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What this browser says it is: the default `User-Agent` value, and the
//! platform beside it (ADR 0030).
//!
//! **This is the one place either is composed.** The header is sent from here
//! by [`crate::http::write_request`] and HTTP/2's header block, and
//! `navigator.userAgent` will answer the same string because the renderer is
//! told it rather than composing its own (ADR 0030 § 4). A page that reads one
//! value while its server receives another is a page whose two halves disagree
//! about which browser they are on.
//!
//! **It names this browser and claims no other.** `Mozilla/5.0 (` stays only
//! because HTML reads the string through it: `appVersion` is empty for a string
//! that does not start that way. After it comes the system's frozen token and
//! `alo/` with the release's first two numbers. No `AppleWebKit`, no `Gecko/`,
//! no `Chrome`: each is a claim to be an engine this is not, and a server that
//! believes one sends that engine's workarounds.
//!
//! **Nothing here is measured.** The token is chosen by the system the binary
//! is built for, at compile time, from § 2's table, and it is the frozen one
//! the major browsers already send — a kind of system, not a description of a
//! machine. A Mac with an Apple processor says `Intel` here for the same reason
//! every other browser does: a truer token splits the crowd by processor, which
//! is exactly what HTML's privacy note asks implementers not to do.
//!
//! A build for a system with no row does not compile. A port adds its row by
//! amending the ADR, and a missing one is found when the port is built rather
//! than by a server receiving a guess.

/// One row of ADR 0030 § 2's table: what a kind of system is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct System {
    /// What goes between the parentheses of the string.
    pub token: &'static str,
    /// What `navigator.platform` answers.
    pub platform: &'static str,
}

impl System {
    /// The default `User-Agent` value for this kind of system.
    ///
    /// The release's major and minor numbers and never its patch: a patch
    /// release would split the people running a release into smaller groups
    /// for the days an update takes to reach them, and no server needs to tell
    /// two patches apart.
    pub fn user_agent(self) -> String {
        format!(
            "Mozilla/5.0 ({}) alo/{}.{}",
            self.token,
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
        )
    }
}

/// A build for macOS.
pub const MACOS: System = System {
    token: "Macintosh; Intel Mac OS X 10_15_7",
    platform: "MacIntel",
};

/// A build for Windows.
pub const WINDOWS: System = System {
    token: "Windows NT 10.0; Win64; x64",
    platform: "Win32",
};

/// A build for Linux.
pub const LINUX: System = System {
    token: "X11; Linux x86_64",
    platform: "Linux x86_64",
};

/// The row for the system this binary was built for.
#[cfg(target_os = "macos")]
pub const THIS: System = MACOS;

/// The row for the system this binary was built for.
#[cfg(target_os = "windows")]
pub const THIS: System = WINDOWS;

/// The row for the system this binary was built for.
#[cfg(target_os = "linux")]
pub const THIS: System = LINUX;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
compile_error!(
    "ADR 0030 § 2 has no row for this operating system, so this browser has no way to say \
     what it is. A port adds its row by amending the ADR, never by guessing."
);

/// What this build sends as `User-Agent` and answers as `navigator.userAgent`.
pub fn user_agent() -> String {
    THIS.user_agent()
}

/// What this build answers as `navigator.platform`.
pub fn platform() -> &'static str {
    THIS.platform
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_ROW: [System; 3] = [MACOS, WINDOWS, LINUX];

    /// Each a claim to be an engine this is not.
    const ANOTHER_ENGINES: [&str; 6] = [
        "AppleWebKit",
        "KHTML",
        "Chrome",
        "Safari",
        "Gecko/",
        "Firefox",
    ];

    #[test]
    fn the_string_names_this_browser_and_its_system() {
        for row in EVERY_ROW {
            let said = row.user_agent();
            assert_eq!(
                said,
                format!(
                    "Mozilla/5.0 ({}) alo/{}.{}",
                    row.token,
                    env!("CARGO_PKG_VERSION_MAJOR"),
                    env!("CARGO_PKG_VERSION_MINOR")
                )
            );
            assert!(said.starts_with("Mozilla/5.0 ("), "{said}");
        }
        assert!(
            MACOS
                .user_agent()
                .starts_with("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) alo/"),
            "{}",
            MACOS.user_agent()
        );
    }

    #[test]
    fn no_row_claims_another_engine() {
        for row in EVERY_ROW {
            let said = row.user_agent();
            for token in ANOTHER_ENGINES {
                assert!(!said.contains(token), "{said} claims {token}");
            }
        }
    }

    #[test]
    fn the_version_is_two_numbers_and_never_the_patch() {
        for row in EVERY_ROW {
            let said = row.user_agent();
            let version = said.rsplit_once("alo/").map(|(_, version)| version);
            assert_eq!(
                version.map(|version| version.split('.').count()),
                Some(2),
                "{said}"
            );
        }
    }

    #[test]
    fn the_rows_are_the_adrs_table() {
        assert_eq!(MACOS.platform, "MacIntel");
        assert_eq!(WINDOWS.platform, "Win32");
        assert_eq!(LINUX.platform, "Linux x86_64");
        assert_eq!(WINDOWS.token, "Windows NT 10.0; Win64; x64");
        assert_eq!(LINUX.token, "X11; Linux x86_64");
    }

    /// A value a header can carry as it is: visible ASCII and spaces, so
    /// neither protocol has anything to escape and no row can end a line.
    #[test]
    fn every_row_is_a_header_value_as_written() {
        for row in EVERY_ROW {
            let said = row.user_agent();
            assert!(
                said.bytes()
                    .all(|byte| byte == b' ' || byte.is_ascii_graphic()),
                "{said:?}"
            );
        }
    }

    #[test]
    fn this_build_answers_its_own_row() {
        assert_eq!(user_agent(), THIS.user_agent());
        assert_eq!(platform(), THIS.platform);
        assert!(EVERY_ROW.contains(&THIS));
    }
}
