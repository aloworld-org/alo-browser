/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What alo checks of a manifest, in order, refusing on the first failure
//! (ADR 0116 § 3):
//!
//! 1. the signature, by one of the two built-in keys ([`crate::signature`]);
//! 2. that it reads ([`crate::manifest`]) — `alo-update 1` — and its
//!    `platform` is this one's;
//! 3. its `serial` is not lower than the highest believed
//!    ([`crate::kept`]): a replayed older manifest is refused. The same
//!    serial again is the same release, checked again;
//! 4. it has not expired: a manifest kept back by a hostile network stops
//!    being believed;
//! 5. its `version` is higher than the installed one: **an older or equal
//!    version is never installed**, whatever is signed;
//! 6. the installed version is at least its `minimum`;
//! 7. the disk has room for twice its `size` ([`room`], asked separately,
//!    for it needs the disk).
//!
//! Nothing here touches a file or the network: every fact is an argument,
//! so each rule is asserted with nothing moving. A manifest that passes 1
//! to 4 is believed — its serial is the highest now — even when it offers
//! nothing newer ([`Checked::believed`]).

use crate::instant::Instant;
use crate::kept::Serial;
use crate::manifest::{self, Manifest, Unread};
use crate::signature::{self, Keys, Signer, Unsigned};
use crate::version::Version;
use core::fmt;

/// What is installed, as the checks ask it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Installed<'a> {
    /// Its version.
    pub version: Version,
    /// Its platform's word.
    pub platform: &'a str,
}

/// A manifest that passed the checks: what it offers, and who signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// The manifest.
    pub manifest: Manifest,
    /// Which key signed it.
    pub signer: Signer,
}

impl Checked {
    /// The serial to keep as the highest believed.
    pub fn believed(&self) -> u64 {
        self.manifest.serial
    }
}

/// Why a manifest was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// 1: not signed by either key.
    Unsigned(Unsigned),
    /// 2: signed, and does not read.
    Unread(Unread),
    /// 2: for another platform.
    OtherPlatform {
        /// The platform it names.
        named: String,
    },
    /// 3: a serial lower than the highest believed — or the highest kept
    /// does not read, so nothing is believed.
    Replayed {
        /// Its serial.
        serial: u64,
        /// The highest believed, if it reads.
        highest: Option<u64>,
    },
    /// 4: it has expired.
    Expired {
        /// When.
        expires: Instant,
    },
    /// 5: the version it offers is not higher than the installed one.
    NotNewer {
        /// The version offered.
        offered: Version,
        /// The version installed.
        installed: Version,
    },
    /// 6: the installed version is below its minimum.
    BelowMinimum {
        /// Its minimum.
        minimum: Version,
        /// The version installed.
        installed: Version,
    },
    /// 7: not room on the disk for twice its size.
    NoRoom {
        /// The bytes needed: twice its size.
        needed: u64,
        /// The bytes free, if the disk could be asked.
        free: Option<u64>,
    },
}

impl Refused {
    /// Whether this is the ordinary answer of a feed with nothing newer —
    /// the same version again — rather than a refusal worth recording.
    pub fn is_up_to_date(&self) -> bool {
        matches!(self, Refused::NotNewer { offered, installed } if offered == installed)
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Refused::Unsigned(why) => write!(f, "The update was refused: {why}."),
            Refused::Unread(why) => write!(f, "The update was refused: {why}."),
            Refused::OtherPlatform { named } => write!(
                f,
                "The update was refused: it is for {named:?}, not this computer."
            ),
            Refused::Replayed {
                serial,
                highest: Some(highest),
            } => write!(
                f,
                "The update was refused: it is an older release ({serial}, after {highest}), \
                 sent again."
            ),
            Refused::Replayed { highest: None, .. } => f.write_str(
                "The update was refused: what alo kept of the last update does not read.",
            ),
            Refused::Expired { expires } => write!(
                f,
                "alo could not check for updates: the update\u{2019}s manifest expired at \
                 {expires}."
            ),
            Refused::NotNewer { offered, installed } if offered == installed => {
                write!(f, "alo {installed} is up to date.")
            }
            Refused::NotNewer { offered, installed } => write!(
                f,
                "The update was refused: alo {offered} is older than alo {installed}."
            ),
            Refused::BelowMinimum { minimum, .. } => write!(
                f,
                "This alo is too old to update itself to the newest. Download alo again from \
                 where you got it (it needs {minimum} or later)."
            ),
            Refused::NoRoom { needed, .. } => write!(
                f,
                "There is not room on the disk for the update: it needs {} MB free.",
                needed.div_ceil(1_000_000)
            ),
        }
    }
}

/// Checks 1 to 6 of `bytes`, signed with `signature`, against `keys`, the
/// highest serial believed, what is `installed` and the moment `now`.
///
/// # Errors
///
/// [`Refused`], the first check that failed.
pub fn check(
    (bytes, signature): (&[u8], &[u8]),
    keys: &Keys,
    highest: Serial,
    installed: Installed<'_>,
    now: Instant,
) -> Result<Checked, Refused> {
    let signer = signature::verify(bytes, signature, keys).map_err(Refused::Unsigned)?;
    let manifest = manifest::read(bytes).map_err(Refused::Unread)?;
    if manifest.platform != installed.platform {
        return Err(Refused::OtherPlatform {
            named: manifest.platform,
        });
    }
    match highest {
        Serial::None => {}
        Serial::Highest(highest) if manifest.serial >= highest => {}
        Serial::Highest(highest) => {
            return Err(Refused::Replayed {
                serial: manifest.serial,
                highest: Some(highest),
            });
        }
        Serial::Unread => {
            return Err(Refused::Replayed {
                serial: manifest.serial,
                highest: None,
            });
        }
    }
    if now >= manifest.expires {
        return Err(Refused::Expired {
            expires: manifest.expires,
        });
    }
    Ok(Checked { manifest, signer })
}

/// Checks 5 and 6: whether what `checked` offers may be installed over
/// `installed`.
///
/// # Errors
///
/// [`Refused::NotNewer`] or [`Refused::BelowMinimum`].
pub fn offered(checked: &Checked, installed: Version) -> Result<(), Refused> {
    let manifest = &checked.manifest;
    if manifest.version <= installed {
        return Err(Refused::NotNewer {
            offered: manifest.version,
            installed,
        });
    }
    if installed < manifest.minimum {
        return Err(Refused::BelowMinimum {
            minimum: manifest.minimum,
            installed,
        });
    }
    Ok(())
}

/// Check 7: whether `free` bytes are room for twice `size`.
///
/// # Errors
///
/// [`Refused::NoRoom`]; a disk that could not be asked has no room.
pub fn room(size: u64, free: Option<u64>) -> Result<(), Refused> {
    let needed = size.saturating_mul(2);
    match free {
        Some(free) if free >= needed => Ok(()),
        _ => Err(Refused::NoRoom { needed, free }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signature::Key;
    use crate::test_key;
    use crate::worthless_test_key::{IN_USE, SPARE, STRANGER};

    const NOW: Instant = Instant::at(1_794_650_000);
    const INSTALLED: Installed<'static> = Installed {
        version: Version::of(0, 4, 1),
        platform: "macos-arm64",
    };

    fn keys() -> Keys {
        Keys {
            in_use: Key::read(test_key::PUBLIC).expect("a key"),
            spare: Key::read(test_key::SPARE).expect("a key"),
        }
    }

    fn manifest(changes: &[(&str, &str)]) -> String {
        let mut text = "alo-update 1\nplatform macos-arm64\nversion 0.4.2\nserial 118\n\
             published 2026-11-14T09:00:00Z\nexpires 2026-11-28T09:00:00Z\nminimum 0.3.0\n\
             chromium 156.0.8078.96\npackage https://updates.example.org/alo-0.4.2.zip\n\
             size 1000\nsha256 0000000000000000000000000000000000000000000000000000000000000000\n\
             security yes\nbehind no\n"
            .to_owned();
        for (from, to) in changes {
            text = text.replacen(from, to, 1);
        }
        text
    }

    fn checked(text: &str, signature: &str, highest: Serial) -> Result<Checked, Refused> {
        check(
            (text.as_bytes(), signature.as_bytes()),
            &keys(),
            highest,
            INSTALLED,
            NOW,
        )
    }

    #[test]
    fn a_manifest_signed_by_either_key_passes_and_says_which() {
        let text = manifest(&[]);
        let by_use = checked(&text, &IN_USE.sign(text.as_bytes()), Serial::None).expect("passes");
        assert_eq!((by_use.signer, by_use.believed()), (Signer::InUse, 118));
        let by_spare =
            checked(&text, &SPARE.sign(text.as_bytes()), Serial::Highest(118)).expect("passes");
        assert_eq!(by_spare.signer, Signer::Spare);
        assert_eq!(offered(&by_use, INSTALLED.version), Ok(()));
        // The public halves the tests sign with are the ones the crate
        // knows as the worthless test keys.
        assert_eq!(IN_USE.public(), test_key::PUBLIC);
        assert_eq!(SPARE.public(), test_key::SPARE);
    }

    #[test]
    fn each_check_refuses_in_its_order() {
        let text = manifest(&[]);
        let signed = IN_USE.sign(text.as_bytes());
        // 1: a stranger's key; a byte changed; the trusted comment changed.
        assert_eq!(
            checked(&text, &STRANGER.sign(text.as_bytes()), Serial::None),
            Err(Refused::Unsigned(Unsigned::NotByAlosKeys))
        );
        let changed = text.replacen("serial 118", "serial 119", 1);
        assert_eq!(
            checked(&changed, &signed, Serial::None),
            Err(Refused::Unsigned(Unsigned::NotByAlosKeys))
        );
        let comment = IN_USE.sign_saying(text.as_bytes(), "timestamp:1\tfile:manifest");
        let swapped = signed.replace(
            &format!("trusted comment: {}", crate::worthless_test_key::TRUSTED),
            "trusted comment: timestamp:1\tfile:manifest",
        );
        assert!(checked(&text, &comment, Serial::None).is_ok());
        assert_eq!(
            checked(&text, &swapped, Serial::None),
            Err(Refused::Unsigned(Unsigned::NotByAlosKeys))
        );
        // 2: signed, and of another format or platform.
        let later = manifest(&[("alo-update 1", "alo-update 2")]);
        assert!(matches!(
            checked(&later, &IN_USE.sign(later.as_bytes()), Serial::None),
            Err(Refused::Unread(_))
        ));
        let other = manifest(&[("platform macos-arm64", "platform linux-x64")]);
        assert_eq!(
            checked(&other, &IN_USE.sign(other.as_bytes()), Serial::None),
            Err(Refused::OtherPlatform {
                named: "linux-x64".into()
            })
        );
        // 3: replayed; the same serial again is the same release.
        assert_eq!(
            checked(&text, &signed, Serial::Highest(119)),
            Err(Refused::Replayed {
                serial: 118,
                highest: Some(119)
            })
        );
        assert!(checked(&text, &signed, Serial::Highest(118)).is_ok());
        assert_eq!(
            checked(&text, &signed, Serial::Unread),
            Err(Refused::Replayed {
                serial: 118,
                highest: None
            })
        );
        // 4: expired, to the second.
        let expiry = Instant::read("2026-11-28T09:00:00Z").expect("a moment");
        assert!(
            check(
                (text.as_bytes(), signed.as_bytes()),
                &keys(),
                Serial::None,
                INSTALLED,
                Instant::at(expiry.seconds() - 1)
            )
            .is_ok()
        );
        assert_eq!(
            check(
                (text.as_bytes(), signed.as_bytes()),
                &keys(),
                Serial::None,
                INSTALLED,
                expiry
            ),
            Err(Refused::Expired { expires: expiry })
        );
    }

    #[test]
    fn an_older_or_equal_version_is_never_offered_nor_one_past_its_minimum() {
        let text = manifest(&[]);
        let passed = checked(&text, &IN_USE.sign(text.as_bytes()), Serial::None).expect("passes");
        let equal = offered(&passed, Version::of(0, 4, 2));
        assert_eq!(
            equal,
            Err(Refused::NotNewer {
                offered: Version::of(0, 4, 2),
                installed: Version::of(0, 4, 2)
            })
        );
        assert!(equal.as_ref().is_err_and(Refused::is_up_to_date));
        let older = offered(&passed, Version::of(0, 5, 0));
        assert!(older.as_ref().is_err_and(|why| !why.is_up_to_date()));
        assert_eq!(
            older.map_err(|why| why.to_string()),
            Err("The update was refused: alo 0.4.2 is older than alo 0.5.0.".to_owned())
        );
        assert_eq!(
            offered(&passed, Version::of(0, 2, 9)),
            Err(Refused::BelowMinimum {
                minimum: Version::of(0, 3, 0),
                installed: Version::of(0, 2, 9)
            })
        );
        assert_eq!(offered(&passed, Version::of(0, 3, 0)), Ok(()));
    }

    #[test]
    fn room_is_twice_the_size_and_an_unasked_disk_has_none() {
        assert_eq!(room(1000, Some(2000)), Ok(()));
        assert_eq!(
            room(1000, Some(1999)),
            Err(Refused::NoRoom {
                needed: 2000,
                free: Some(1999)
            })
        );
        assert_eq!(
            room(1000, None),
            Err(Refused::NoRoom {
                needed: 2000,
                free: None
            })
        );
        assert_eq!(room(u64::MAX, Some(u64::MAX)), Ok(()));
    }
}
