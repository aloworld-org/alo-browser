/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The macOS application's icon: the pictures this engine drew, in the
//! entries of an `.icns` (queue item 689; ADR 0067 § 6).
//!
//! The pictures are the corpus case `the-applications-icon`'s references,
//! `icon-16.png` to `icon-1024.png`, each the case's page drawn in a window
//! that many pixels a side and compared by the gate
//! (`alo-corpus/tests/the_applications_icon.rs`). They are taken into the
//! command as it is compiled, so the icon an application carries is the
//! bytes the gate compared — never a copy that drifted.
//!
//! Each entry type names a size in points and a scale, and a picture of
//! some number of pixels serves every type that many pixels wide: the
//! 32-pixel picture is both 32 points at one and 16 points at two. The
//! types are the ones that hold a PNG; Apple's `iconutil` writes the two
//! smallest as its own run-length format instead, and reads these back as
//! the same sizes (measured, macOS 26.5.2).

/// The icon's file in `Contents/Resources`, which `CFBundleIconFile` names.
pub const FILE: &str = "icon.icns";

/// Each picture, by the pixels a side it is drawn at.
const PICTURES: [(u32, &[u8]); 7] = [
    (
        16,
        include_bytes!("../../alo-corpus/cases/the-applications-icon/icon-16.png"),
    ),
    (
        32,
        include_bytes!("../../alo-corpus/cases/the-applications-icon/icon-32.png"),
    ),
    (
        64,
        include_bytes!("../../alo-corpus/cases/the-applications-icon/icon-64.png"),
    ),
    (
        128,
        include_bytes!("../../alo-corpus/cases/the-applications-icon/icon-128.png"),
    ),
    (
        256,
        include_bytes!("../../alo-corpus/cases/the-applications-icon/icon-256.png"),
    ),
    (
        512,
        include_bytes!("../../alo-corpus/cases/the-applications-icon/icon-512.png"),
    ),
    (
        1024,
        include_bytes!("../../alo-corpus/cases/the-applications-icon/icon-1024.png"),
    ),
];

/// Each entry the `.icns` holds, in the order written, and the pixels a
/// side of the picture it holds.
pub const ENTRIES: [([u8; 4], u32); 10] = [
    (*b"icp4", 16),
    (*b"icp5", 32),
    (*b"ic11", 32),
    (*b"ic12", 64),
    (*b"ic07", 128),
    (*b"ic13", 256),
    (*b"ic08", 256),
    (*b"ic14", 512),
    (*b"ic09", 512),
    (*b"ic10", 1024),
];

/// The picture drawn `side` pixels a side, or [`None`] for a size not drawn.
pub fn picture(side: u32) -> Option<&'static [u8]> {
    PICTURES
        .iter()
        .find(|(drawn, _)| *drawn == side)
        .map(|(_, bytes)| *bytes)
}

/// The `.icns`'s bytes.
///
/// # Errors
///
/// [`crate::icns::TooLarge`] were the pictures 4 GiB together, which seven
/// committed PNGs are not.
pub fn icns() -> Result<Vec<u8>, crate::icns::TooLarge> {
    let entries: Vec<([u8; 4], &[u8])> = ENTRIES
        .iter()
        .filter_map(|(kind, side)| Some((*kind, picture(*side)?)))
        .collect();
    crate::icns::written(&entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PNG's width and height, from its header.
    fn sides(png: &[u8]) -> Option<(u32, u32)> {
        let width = png.get(16..20)?.try_into().ok()?;
        let height = png.get(20..24)?.try_into().ok()?;
        Some((u32::from_be_bytes(width), u32::from_be_bytes(height)))
    }

    #[test]
    fn every_picture_is_a_png_as_wide_as_it_says() {
        for (side, png) in PICTURES {
            assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "{side}");
            assert_eq!(sides(png), Some((side, side)), "{side}");
        }
    }

    #[test]
    fn every_entry_holds_a_picture_drawn_and_every_picture_is_held() {
        for (kind, side) in ENTRIES {
            assert!(
                picture(side).is_some(),
                "{}",
                String::from_utf8_lossy(&kind)
            );
        }
        for (side, _) in PICTURES {
            assert!(ENTRIES.iter().any(|(_, held)| *held == side), "{side}");
        }
        assert_eq!(picture(48), None);
    }

    #[test]
    fn the_icns_holds_each_entry_with_its_picture_as_drawn() {
        let icns = icns().unwrap_or_default();
        let read = crate::icns::entries(&icns).unwrap_or_default();
        assert_eq!(read.len(), ENTRIES.len());
        for ((kind, bytes), (wanted, side)) in read.iter().zip(ENTRIES) {
            assert_eq!(*kind, wanted);
            assert_eq!(
                Some(*bytes),
                picture(side),
                "{}",
                String::from_utf8_lossy(kind)
            );
        }
    }
}
