/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! One check, the whole road (ADR 0116 § 3): the manifest and its
//! signature fetched; checked in order ([`crate::check`]); believed; the
//! package downloaded and checked ([`crate::package`]); unpacked beside
//! the application and checked again ([`crate::stage`]) — ready to be
//! swapped in at quit ([`crate::quitting`]). Each step refuses on its
//! first failure, and **nothing of the installed application is touched**
//! by any of it.
//!
//! When it checks is its caller's: at launch and every 24 hours while the
//! switch in *Settings* is on, or when *Check now* is pressed — each
//! request carrying that cause. What it found is kept with when
//! ([`crate::kept`]), for *Settings* to say.

use crate::address::Address;
use crate::check::{self, Installed, Refused};
use crate::found::Found;
use crate::instant::Instant;
use crate::kept::Kept;
use crate::manifest;
use crate::package::{self, Unfinished};
use crate::signature::{self, Keys};
use crate::stage::{self, Unstaged};
use crate::version::Version;
use alo_net::{Cause, Pool};
use std::fs;
use std::path::PathBuf;

/// What one alo updates, and from where.
#[derive(Debug, Clone)]
pub struct Updater {
    /// Where the feed is.
    pub address: Address,
    /// The two keys either of which may sign.
    pub keys: Keys,
    /// The updater's own folder.
    pub kept: Kept,
    /// The installed application, `alo browser.app`.
    pub application: PathBuf,
    /// The version running.
    pub version: Version,
    /// This platform's word.
    pub platform: String,
    /// The most bytes one piece of the package asks for.
    pub piece: u64,
}

impl Updater {
    /// Check once, `now`, every request caused by `cause`: what was found,
    /// kept with when.
    pub fn check(&self, pool: &mut Pool, cause: &Cause, now: Instant) -> Found {
        let found = self.road(pool, cause, now);
        let _ = self.kept.check_ended(now, &found.said());
        found
    }

    /// The road, refusing on its first failure.
    fn road(&self, pool: &mut Pool, cause: &Cause, now: Instant) -> Found {
        let (bytes, signed) = match self.fetched(pool, cause) {
            Ok(both) => both,
            Err(why) => {
                return Found::CouldNotCheck(format!("alo could not check for updates: {why}."));
            }
        };
        let installed = Installed {
            version: self.version,
            platform: &self.platform,
        };
        let checked = match check::check(
            (&bytes, &signed),
            &self.keys,
            self.kept.serial(),
            installed,
            now,
        ) {
            Ok(checked) => checked,
            Err(expired @ Refused::Expired { .. }) => {
                return Found::CouldNotCheck(expired.to_string());
            }
            Err(refused) => return Found::Refused(refused.to_string()),
        };
        if let Err(why) = self.kept.believe(checked.believed(), now) {
            return Found::Refused(format!("The update could not be kept: {why}."));
        }
        let manifest = &checked.manifest;
        match check::offered(&checked, self.version) {
            Ok(()) => {}
            Err(same) if same.is_up_to_date() => {
                return Found::UpToDate {
                    version: self.version,
                    behind: manifest.behind,
                };
            }
            Err(refused) => return Found::Refused(refused.to_string()),
        }
        let ready = Found::Ready {
            version: manifest.version,
            chromium: manifest.chromium.clone(),
            security: manifest.security,
            behind: manifest.behind,
        };
        if stage::ready_beside(&self.application)
            .is_some_and(|staged| staged.version == manifest.version)
        {
            return ready;
        }
        let folder = match self.kept.made() {
            Ok(folder) => folder.to_path_buf(),
            Err(why) => return Found::Refused(format!("The update could not be kept: {why}.")),
        };
        if let Err(no_room) = check::room(manifest.size, alo_storage::volume::free_space(&folder)) {
            return Found::Refused(no_room.to_string());
        }
        let zip = match package::download(pool, manifest, &folder, (cause, self.piece)) {
            Ok(zip) => zip,
            Err(cut @ Unfinished::Cut { .. }) => return Found::Coming(cut.to_string()),
            Err(refused) => return Found::Refused(refused.to_string()),
        };
        let staged = stage::unpack(&zip, &self.application, manifest.version);
        let _ = fs::remove_file(&zip);
        match staged {
            Ok(_) => ready,
            Err(Unstaged::NotWritable { why }) => Found::Refused(why),
            Err(refused) => Found::Refused(refused.to_string()),
        }
    }

    /// The manifest and its signature, as the feed sent them.
    fn fetched(&self, pool: &mut Pool, cause: &Cause) -> Result<(Vec<u8>, Vec<u8>), String> {
        let manifest_at = self.address.manifest(&self.platform)?;
        let signature_at = self.address.signature(&self.platform)?;
        let bytes = crate::feed::small(
            pool,
            (&manifest_at, cause),
            u64::try_from(manifest::LARGEST + 1).unwrap_or(u64::MAX),
        )?;
        let signed = crate::feed::small(
            pool,
            (&signature_at, cause),
            u64::try_from(signature::LARGEST + 1).unwrap_or(u64::MAX),
        )?;
        Ok((bytes, signed))
    }
}
