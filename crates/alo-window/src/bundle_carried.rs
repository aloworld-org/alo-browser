/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! CEF's Chromium put in the macOS application, inside alo's own host,
//! with its licences (queue item 720; ADR 0077, ADR 0079).
//!
//! What `alo-bundle` carries is two things built elsewhere: alo's CEF host,
//! `alo-cef-host` (`crates/alo-cef-host`, built against the pinned CEF), and
//! CEF's framework from the distribution `CEF_PATH` names. Both are looked
//! at before anything is made ([`checked`]): the host is a program there,
//! and the distribution is the pinned one ([`crate::cef_pin::check`]) —
//! so a distribution that is not is refused before a byte of it is copied.
//!
//! [`Carried::put_in`] then lays the host out as [`crate::cef_host::assemble`]
//! does, at the place the application starts its Chromium
//! ([`crate::bundle::CHROMIUM`]):
//!
//! ```text
//! alo browser.app/Contents/
//!   Frameworks/alo Chromium.app/          the host, its helpers and CEF's framework
//!   Resources/licences/LICENSE.txt        CEF's licence
//!   Resources/licences/CREDITS.html       Chromium's credits: every project in it
//! ```
//!
//! and the application's page links both licence files ([`LICENCES`]). It
//! is sealed after, inside out, by [`crate::bundle_seal::seal_carried`].

use crate::cef_host::{self, FRAMEWORK, Unassembled};
use crate::cef_pin::{self, Unpinned};
use core::fmt;
use std::path::{Path, PathBuf};

/// The directory in the application's `Contents/Resources` holding the
/// licence files of the Chromium it carries, beside its page — and those of
/// the interface's typefaces, which every application carries
/// ([`crate::bundle_made`]).
pub const LICENCES: &str = "licences";

/// The command that builds the host, said when it is not there.
pub const BUILD_THE_HOST: &str = "CEF_PATH=… cargo build --release --locked \
     --manifest-path crates/alo-cef-host/Cargo.toml --target-dir target/alo-cef-host";

/// Why the Chromium was not carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Uncarried {
    /// The distribution is not the pinned one.
    Unpinned(Unpinned),
    /// The host is not a program at this path.
    NoHost(PathBuf),
    /// The host was not laid out.
    Unassembled(Unassembled),
    /// The disk refused: what was being done, and its words.
    Disk(String, String),
}

impl fmt::Display for Uncarried {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unpinned(why) => write!(f, "{why}"),
            Self::NoHost(at) => write!(
                f,
                "alo's CEF host is not at {}: build it first, with {BUILD_THE_HOST}",
                at.display()
            ),
            Self::Unassembled(why) => write!(f, "{why}"),
            Self::Disk(doing, why) => write!(f, "{doing}: {why}"),
        }
    }
}

impl std::error::Error for Uncarried {}

/// The host and the distribution, both checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carried {
    /// The unpacked CEF distribution, the pinned one.
    distribution: PathBuf,
    /// alo's CEF host's program.
    host: PathBuf,
}

/// The host at `host` and the CEF distribution at `distribution`, checked:
/// the host first, the cheaper, then the distribution against its pin.
///
/// # Errors
///
/// [`Uncarried::NoHost`], or [`Uncarried::Unpinned`] saying what differs.
pub fn checked(distribution: &Path, host: &Path) -> Result<Carried, Uncarried> {
    if !crate::beside::is_program(host) {
        return Err(Uncarried::NoHost(host.to_path_buf()));
    }
    cef_pin::check(distribution).map_err(Uncarried::Unpinned)?;
    Ok(Carried {
        distribution: distribution.to_path_buf(),
        host: host.to_path_buf(),
    })
}

/// The host's own application inside the application whose directory is
/// `application`: `Contents/Frameworks/alo Chromium.app`.
pub fn host_application(application: &Path) -> PathBuf {
    application
        .join("Contents/Frameworks")
        .join(format!("{}.app", cef_host::EXECUTABLE))
}

impl Carried {
    /// Put the host, CEF's framework and the licences in the application
    /// being made at `application`: the host's program, where the
    /// application starts it.
    ///
    /// # Errors
    ///
    /// [`Uncarried::Unassembled`] or [`Uncarried::Disk`], naming the step.
    pub fn put_in(&self, application: &Path) -> Result<PathBuf, Uncarried> {
        let program = cef_host::assemble(
            &self.host,
            &self.distribution.join(FRAMEWORK),
            &host_application(application),
        )
        .map_err(Uncarried::Unassembled)?;
        let licences = application.join("Contents/Resources").join(LICENCES);
        std::fs::create_dir_all(&licences).map_err(|why| {
            Uncarried::Disk(
                format!("{} could not be made", licences.display()),
                why.to_string(),
            )
        })?;
        for (name, _) in cef_pin::LICENCES {
            let from = self.distribution.join(name);
            std::fs::copy(&from, licences.join(name)).map_err(|why| {
                Uncarried::Disk(
                    format!("{} could not be copied", from.display()),
                    why.to_string(),
                )
            })?;
        }
        Ok(program)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_is_where_the_application_starts_its_chromium() {
        let application = Path::new("/A/alo browser.app");
        assert_eq!(
            host_application(application)
                .join("Contents/MacOS")
                .join(cef_host::EXECUTABLE),
            application.join("Contents").join(crate::bundle::CHROMIUM)
        );
    }

    #[test]
    fn a_host_not_built_is_named_with_the_command_that_builds_it() {
        let refused = checked(Path::new("/nowhere"), Path::new("/nowhere/alo-cef-host"));
        assert_eq!(
            refused,
            Err(Uncarried::NoHost(PathBuf::from("/nowhere/alo-cef-host")))
        );
        assert!(refused.is_err_and(|why| why.to_string().ends_with(
            "--manifest-path crates/alo-cef-host/Cargo.toml --target-dir target/alo-cef-host"
        )));
    }

    #[test]
    fn with_the_host_there_the_distribution_is_checked() {
        let refused = checked(Path::new("/nowhere"), Path::new("/bin/sh"));
        assert_eq!(
            refused,
            Err(Uncarried::Unpinned(Unpinned::NoDistribution(
                PathBuf::from("/nowhere")
            )))
        );
    }
}
