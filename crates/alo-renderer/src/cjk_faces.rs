/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The machine's Chinese, Japanese and Korean faces, for the interface's
//! renderers (queue item 767, ADR 0088 § 5).
//!
//! The faces a renderer is handed from the machine ([`crate::fonts`]) are
//! two dozen `.ttf` and `.otf` files, and on macOS none of them has a Han
//! character, a kana or a hangul: those faces are collections. So a
//! Chinese, Japanese or Korean title among a search's results was drawn in
//! `LastResort`, its block's box. This names one face of each script, read
//! out of the collection that holds it ([`crate::font_collection`]), for the
//! browser process to hand the conversation's two renderers after every
//! other face — never a page's, whose fonts are the machine's two dozen and
//! what it asks for by name.
//!
//! **Which, and in what order.** The regular face of each, each under
//! [`crate::face::LARGEST_FONT`]: Hiragino Sans W3 (Japanese, 7.3 MB of a 7.9 MB file),
//! then Hiragino Sans GB W3 (Chinese, 11.6 MB of 23.5 MB), then Apple SD
//! Gothic Neo Regular (Korean, 3.2 MB of 55.4 MB) — 22 MB a renderer. A Han
//! character all three have is drawn in the first; which one Chrome would
//! choose depends on the language a person reads, and is not measured here.
//! A face whose file is not there, does not read, or does not state the
//! family it is listed under, is left out, and the rest are handed.
//!
//! A face is read once for the window's life, when its renderers start, and
//! only where its tables are: the files are 86 MB together.

use crate::face::Face;
use crate::font_collection;
use std::fs::File;
use std::path::Path;

/// Each face handed, as `(file, face in it, the family it must state)`.
#[cfg(target_os = "macos")]
pub const FACES: [(&str, u32, &str); 3] = [
    (
        "/System/Library/Fonts/\u{30d2}\u{30e9}\u{30ae}\u{30ce}\u{89d2}\u{30b4}\u{30b7}\u{30c3}\u{30af} W3.ttc",
        0,
        "Hiragino Sans",
    ),
    (
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        0,
        "Hiragino Sans GB",
    ),
    (
        "/System/Library/Fonts/AppleSDGothicNeo.ttc",
        0,
        "Apple SD Gothic Neo",
    ),
];

/// None on a machine whose faces are not named here yet.
#[cfg(not(target_os = "macos"))]
pub const FACES: [(&str, u32, &str); 0] = [];

/// The faces of [`FACES`] this machine has, in order: each read out of its
/// collection and described by itself.
pub fn of_this_machine() -> Vec<Face> {
    FACES
        .iter()
        .filter_map(|&(file, index, family)| face(Path::new(file), index, family))
        .collect()
}

/// The face at `index` of the collection at `path`, if it reads and states
/// `family`.
pub fn face(path: &Path, index: u32, family: &str) -> Option<Face> {
    let mut file = File::open(path).ok()?;
    let bytes = font_collection::face_of(&mut file, index).ok()?;
    Face::described(bytes).filter(|face| face.family == family)
}
