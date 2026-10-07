/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a page may be allowed, and nothing else.
//!
//! ADR 0026 § 1: *"A capability is a Rust `enum`. Adding a variant changes this
//! table, in an amendment to this ADR, before any code changes. A capability
//! is never a string a page names, and there is no 'other'."*
//!
//! So a page's name for what it wants is read once, here, by
//! [`Capability::named`], and everything past this file holds a
//! [`Capability`]. A name not on the list is refused with the ADR's number
//! beside it, which is what a page is told through `NotAllowedError` or
//! `"denied"` once an API reaches this.
//!
//! # What is not a permission
//!
//! Writing to the clipboard, going full screen and starting playback with
//! sound need a person's recent gesture and nothing more. They are not
//! variants, and so they can never be a row in the table: a person who clicked
//! *copy* has already answered.

use core::fmt;

/// One thing a person may allow a site, from ADR 0026 § 1's closed list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// Video frames from a camera device.
    Camera,
    /// Audio frames from an input device.
    Microphone,
    /// The machine's position.
    Location,
    /// Showing a notification outside the page.
    Notifications,
    /// Protection of the site's bucket from eviction (ADR 0025 § 7).
    KeepData,
    /// The embedded site's unpartitioned cookies (ADR 0007).
    StorageAccess,
    /// Handing a person to a scheme another program owns (ADR 0020 § 3).
    OpenAnotherProgram,
    /// What the person last copied, from any program.
    ReadTheClipboard,
}

/// Every capability, in the ADR's order.
///
/// A ninth variant does not compile until [`Capability::tag`] gives it a byte,
/// and [`Capability::from_tag`] reads only what is listed here, so a variant
/// left out of this list can be written and never read back. The amendment
/// that adds one adds it in both places.
pub const EVERY: [Capability; 8] = [
    Capability::Camera,
    Capability::Microphone,
    Capability::Location,
    Capability::Notifications,
    Capability::KeepData,
    Capability::StorageAccess,
    Capability::OpenAnotherProgram,
    Capability::ReadTheClipboard,
];

/// A name a page used that is not on the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotOnTheList {
    /// What the page named.
    pub name: String,
}

impl fmt::Display for NotOnTheList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\"{}\" is not a capability this browser grants (ADR 0026 § 1)",
            self.name
        )
    }
}

impl std::error::Error for NotOnTheList {}

impl Capability {
    /// The capability a page names through the Permissions API, or why it is
    /// refused.
    ///
    /// The names are the Permissions Registry's. *Open another program* has
    /// none, because a page reaches it by navigating rather than by asking;
    /// a page that names one of the gesture-only things, or anything else, is
    /// refused here and never prompted.
    ///
    /// # Errors
    ///
    /// [`NotOnTheList`], for every name but the seven.
    pub fn named(name: &str) -> Result<Self, NotOnTheList> {
        match name {
            "camera" => Ok(Capability::Camera),
            "microphone" => Ok(Capability::Microphone),
            "geolocation" => Ok(Capability::Location),
            "notifications" => Ok(Capability::Notifications),
            "persistent-storage" => Ok(Capability::KeepData),
            "storage-access" => Ok(Capability::StorageAccess),
            "clipboard-read" => Ok(Capability::ReadTheClipboard),
            _ => Err(NotOnTheList {
                name: name.to_owned(),
            }),
        }
    }

    /// Whether using it holds a device, and so shows the indicator while it is
    /// in use (ADR 0026 § 6).
    pub fn holds_a_device(self) -> bool {
        matches!(
            self,
            Capability::Camera | Capability::Microphone | Capability::Location
        )
    }

    /// The byte it is written as in the table's file.
    pub fn tag(self) -> u8 {
        match self {
            Capability::Camera => 1,
            Capability::Microphone => 2,
            Capability::Location => 3,
            Capability::Notifications => 4,
            Capability::KeepData => 5,
            Capability::StorageAccess => 6,
            Capability::OpenAnotherProgram => 7,
            Capability::ReadTheClipboard => 8,
        }
    }

    /// The capability a byte from the file names, or [`None`] for a byte no
    /// table this engine wrote contains.
    pub fn from_tag(tag: u8) -> Option<Self> {
        EVERY.into_iter().find(|capability| capability.tag() == tag)
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Capability::Camera => "the camera",
            Capability::Microphone => "the microphone",
            Capability::Location => "the location",
            Capability::Notifications => "notifications",
            Capability::KeepData => "keeping data",
            Capability::StorageAccess => "storage access",
            Capability::OpenAnotherProgram => "opening another program",
            Capability::ReadTheClipboard => "reading the clipboard",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_capability_has_its_own_tag_and_reads_back() {
        for capability in EVERY {
            assert_eq!(Capability::from_tag(capability.tag()), Some(capability));
        }
        let mut tags: Vec<u8> = EVERY.iter().map(|capability| capability.tag()).collect();
        tags.dedup();
        assert_eq!(tags.len(), 8);
        assert_eq!(Capability::from_tag(0), None);
        assert_eq!(Capability::from_tag(9), None);
    }

    /// ADR 0026 § 1: anything not on the list is refused, naming the ADR.
    #[test]
    fn a_name_not_on_the_list_is_refused_and_the_refusal_names_the_adr() {
        for name in [
            "display-capture",
            "midi",
            "usb",
            "serial",
            "bluetooth",
            "hid",
            "idle-detection",
            "accelerometer",
            "clipboard-write",
            "fullscreen",
            "speaker-selection",
            "",
            "Camera",
        ] {
            let refused = Capability::named(name).expect_err("not on the list");
            assert!(refused.to_string().contains("ADR 0026"), "{refused}");
        }
        assert_eq!(Capability::named("camera"), Ok(Capability::Camera));
        assert_eq!(Capability::named("geolocation"), Ok(Capability::Location));
        assert_eq!(
            Capability::named("persistent-storage"),
            Ok(Capability::KeepData)
        );
    }

    #[test]
    fn only_the_camera_the_microphone_and_the_location_hold_a_device() {
        let holding: Vec<Capability> = EVERY
            .into_iter()
            .filter(|capability| capability.holds_a_device())
            .collect();
        assert_eq!(
            holding,
            vec![
                Capability::Camera,
                Capability::Microphone,
                Capability::Location
            ]
        );
    }
}
