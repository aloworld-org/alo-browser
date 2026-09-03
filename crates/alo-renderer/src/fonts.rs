/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The fonts the browser process finds, so a renderer never has to look.
//!
//! ADR 0010 confines renderers, and this is the side of that decision nobody
//! sees: **somebody still has to open the font files**, and it is the process
//! that is allowed to open files. This module is that side.
//!
//! # Why a short list rather than everything installed
//!
//! A machine has hundreds of fonts and some collections are tens of megabytes.
//! Handing all of them to every renderer would be most of a second and most of
//! a gigabyte per tab, for fonts no page asks for.
//!
//! So this finds a small set — enough to render text at all — and finds the
//! rest **on demand, when a page asks for a family by name**, which is
//! [`named`].
//!
//! # A font is called what it says it is called
//!
//! Every family here comes from the **font**, via [`alo_text::family_in`], and
//! never from the name of the file it was read out of. A filename is a guess:
//! `HelveticaNeue-Bold.ttf` is nearly always Helvetica Neue and
//! `ZapfDingbats.dfont` is nearly never what its author called it, and the
//! difference between those two is invisible until somebody's page is drawn in
//! the wrong font.
//!
//! This file did take the filename once, for [`from_this_machine`], on the
//! argument that a database is only a guess about what to look at and that
//! opening every font on the machine to ask would be most of a second before
//! the first page. The second half of that was **wrong**: [`from_file`] reads
//! the whole file already, because a face is bytes rather than a path
//! (ADR 0010), so asking costs a `name` table rather than an open.
//!
//! A font that states no readable family is skipped. That is a real answer
//! about a file — nothing can ask for it by name, so a renderer holding it
//! would be holding a font nothing could choose — and it is rarer than it
//! sounds now that [`alo_text::family_in`] reads the Macintosh records too.

use crate::face::{Face, LARGEST_FONT, MOST_FONTS};
use alo_text::{Slant, Weight};
use std::path::{Path, PathBuf};

/// Where a machine keeps its fonts.
#[cfg(target_os = "macos")]
const DIRECTORIES: [&str; 2] = ["/System/Library/Fonts", "/Library/Fonts"];

/// Where a machine keeps its fonts.
#[cfg(not(target_os = "macos"))]
const DIRECTORIES: [&str; 3] = [
    "/usr/share/fonts",
    "/usr/local/share/fonts",
    "/System/Library/Fonts",
];

/// A font file this engine can read, by extension.
///
/// `.ttc` collections are deliberately absent: a collection holds several fonts
/// and picking one out of it is a thing `alo-text` does not do yet. Taking the
/// first face of a collection and calling it the family would be a font that
/// renders and is not the one anybody asked for.
const READABLE: [&str; 3] = ["ttf", "otf", "TTF"];

/// The fonts this machine has, up to what one renderer will be given.
///
/// Sorted by name so that two runs on the same machine hand a renderer the same
/// fonts in the same order — which is what makes a rendering difference between
/// runs mean something.
pub fn from_this_machine() -> Vec<Face> {
    let mut found = Vec::new();
    for directory in DIRECTORIES {
        collect(Path::new(directory), &mut found);
    }
    found.sort_by(|one, two| one.family.cmp(&two.family));
    found.truncate(MOST_FONTS);
    found
}

/// The most faces of one family this looks for.
///
/// A large family on a well-stocked machine is a dozen or so files; this is
/// room for that and a bound on a directory somebody filled with two thousand
/// weights of one name.
pub const MOST_FACES_OF_A_FAMILY: usize = 16;

/// Every face on this machine belonging to a family, by the name the **fonts**
/// give themselves.
///
/// This is the browser process's half of queue item 170: a renderer says which
/// family a page asked for and did not have, and this is how the side that may
/// open a file goes and looks. An empty answer is a real one — *this machine
/// does not have that family* — and it is what turns a silent substitution into
/// a named one, because the renderer then reports what it drew instead and
/// nothing anywhere pretended the page got what it asked for.
///
/// # The name is a stranger's
///
/// It came off a page, through a renderer that parsed that page. So it is used
/// **only** to compare against the family a font states about itself: nothing
/// here joins it to a path, opens it, or lets it choose a directory. A family
/// called `../../etc/passwd` finds no font, because no font is called that.
pub fn named(family: &str) -> Vec<Face> {
    let wanted = family.trim();
    if wanted.is_empty() {
        return Vec::new();
    }
    let mut found = Vec::new();
    for directory in DIRECTORIES {
        for path in readable_in(Path::new(directory)) {
            if found.len() >= MOST_FACES_OF_A_FAMILY {
                return found;
            }
            let Some(face) = from_file(&path) else {
                continue;
            };
            // `from_file` has already asked the font what it is called, and a
            // font that would not say is not here at all — which is why this
            // compares one name rather than deriving a second. Two derivations
            // of one fact is two chances for them to disagree.
            if face.family.eq_ignore_ascii_case(wanted) {
                found.push(face);
            }
        }
    }
    found
}

/// One font file, read and named by the font.
///
/// [`None`] for a file that is not a font this engine can read, that is larger
/// than one may be, or that **states no family** — the last being a file whose
/// name nothing could ever ask for.
///
/// The weight and the slant are still read off the filename, and they are still
/// a guess. It is a smaller one and it is answered elsewhere: which of a
/// family's faces to draw with is chosen by `alo_text::FontDatabase` among the
/// faces it holds, so a face filed under the wrong weight is drawn in the right
/// family. A family read off a filename is drawn in the wrong font entirely,
/// which is why only that half was owed here. The face's own `OS/2` table would
/// answer both properly, and that is queue item 194 rather than this one.
pub fn from_file(path: &Path) -> Option<Face> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > LARGEST_FONT {
        return None;
    }
    // The font's own answer, never the filename. This is the one fact in the
    // module that decides whether a page is drawn as its author wrote it.
    let family = alo_text::family_in(&bytes)?;
    let name = path.file_stem()?.to_str()?.to_ascii_lowercase();
    let slant = if name.contains("italic") {
        Slant::Italic
    } else {
        Slant::Normal
    };
    let weight = if name.contains("bold") {
        Weight::BOLD
    } else {
        Weight::NORMAL
    };
    Face::new(family, weight, slant, bytes)
}

fn collect(directory: &Path, into: &mut Vec<Face>) {
    for path in readable_in(directory) {
        if into.len() >= MOST_FONTS {
            return;
        }
        if let Some(face) = from_file(&path) {
            into.push(face);
        }
    }
}

/// The font files in a directory, sorted, so two runs read the same ones in the
/// same order.
///
/// A directory that cannot be read is no files rather than an error: a font
/// path that does not exist on this machine is the ordinary case on three
/// operating systems out of three.
fn readable_in(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| READABLE.contains(&extension))
        })
        .collect();
    paths.sort();
    paths
}
