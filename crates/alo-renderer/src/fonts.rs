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
//! # A font is what it says it is
//!
//! Everything a face is filed under here comes from the **font**: the family
//! from [`alo_text::family_in`], the weight and the slant from
//! [`alo_text::style_in`]. Never from the name of the file it was read out of.
//! A filename is a guess: `HelveticaNeue-Bold.ttf` is nearly always Helvetica
//! Neue and `ZapfDingbats.dfont` is nearly never what its author called it, and
//! the difference between those two is invisible until somebody's page is drawn
//! in the wrong font.
//!
//! This file did take the filename once, for [`from_this_machine`], on the
//! argument that a database is only a guess about what to look at and that
//! opening every font on the machine to ask would be most of a second before
//! the first page. The second half of that was **wrong**: [`from_file`] reads
//! the whole file already, because a face is bytes rather than a path
//! (ADR 0010), so asking costs two tables rather than an open.
//!
//! A font that states no readable family is skipped. That is a real answer
//! about a file — nothing can ask for it by name, so a renderer holding it
//! would be holding a font nothing could choose — and it is rarer than it
//! sounds now that [`alo_text::family_in`] reads the Macintosh records too. A
//! font that states no *weight* is kept and is normal: nothing needs to name a
//! weight to be asked for, and a family of one unlabelled face is most of the
//! fonts on a machine.
//!
//! # And what the generics mean
//!
//! [`from_this_machine`] answers two questions rather than one, because they are
//! the same look through the same directories: *what fonts are here*, and *which
//! of them is this machine's `sans-serif`*. [`crate::generic`] decides the
//! second from the families the first found — deriving it twice from the same
//! scan would be two chances for the two answers to disagree, which is the
//! argument [`named`] already makes about a filename.

use crate::face::{Face, LARGEST_FONT, MOST_FONTS};
use crate::generic::Generics;
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

/// The most font files this will open to find out what a machine has.
///
/// A bound on the *look* rather than on what is kept, and it has to be larger
/// than [`MOST_FONTS`] now: a machine's `sans-serif` is a particular family, and
/// stopping at the first two dozen files alphabetically is how a machine with
/// Helvetica ends up with no answer for `sans-serif` because `Apple Braille`
/// sorted earlier.
pub const MOST_LOOKED_AT: usize = 256;

/// The most faces held at once while a machine is being read.
///
/// Room for the short list and for the generic families that would otherwise
/// have been cut out of it, and a ceiling: a directory holding two hundred faces
/// all calling themselves `Helvetica` would otherwise be two hundred font files
/// in memory at once, before anything was truncated.
const MOST_KEPT: usize = MOST_FONTS * 2;

/// What a machine turned out to have.
///
/// The two answers together rather than separately, because the second is read
/// out of the first and a caller deriving it again would be deriving a fact
/// this module already knows.
#[derive(Debug, Clone, Default)]
pub struct Machine {
    /// The fonts to hand a renderer, up to [`MOST_FONTS`].
    pub faces: Vec<Face>,
    /// What the generic families mean here, naming only families in `faces`.
    pub generics: Generics,
}

/// The fonts this machine has, up to what one renderer will be given, and what
/// its generic families mean.
///
/// Ordered so that two runs on the same machine hand a renderer the same fonts
/// in the same order — which is what makes a rendering difference between runs
/// mean something. The families a generic names come **first**, because the cut
/// down to [`MOST_FONTS`] must not be what decides whether every page on this
/// machine has a `sans-serif`; the rest follow by name.
pub fn from_this_machine() -> Machine {
    let mut found = Vec::new();
    let mut looked_at = 0usize;
    for directory in DIRECTORIES {
        collect(Path::new(directory), &mut found, &mut looked_at);
    }
    let wanted = Generics::of(&families_of(&found));
    found.sort_by(|one, two| {
        let after = |face: &Face| !names_a_generic(&wanted, &face.family);
        (after(one), one.family.clone()).cmp(&(after(two), two.family.clone()))
    });
    found.truncate(MOST_FONTS);
    // Decided again, over what actually survived the cut. A mapping naming a
    // family the renderer was not given is a generic that resolves to nothing,
    // reported as a family this machine does not have — which would be this
    // module saying one thing and the fonts it sent saying another.
    let generics = Generics::of(&families_of(&found));
    Machine {
        faces: found,
        generics,
    }
}

/// The family of each face, in order.
fn families_of(faces: &[Face]) -> Vec<String> {
    faces.iter().map(|face| face.family.clone()).collect()
}

/// Whether a generic was decided to mean this family.
fn names_a_generic(generics: &Generics, family: &str) -> bool {
    generics
        .pairs()
        .iter()
        .any(|(_, named)| named.eq_ignore_ascii_case(family))
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

/// One font file, read and described by the font.
///
/// [`None`] for a file that is not a font this engine can read, that is larger
/// than one may be, or that **states no family** — the last being a file whose
/// name nothing could ever ask for.
///
/// Nothing here reads the path except to open it. The family comes from the
/// `name` table and the weight and the slant from `OS/2`, so a file called
/// `Helvetica-Oblique.ttf` and one called `f2.ttf` describe themselves equally
/// well — which was queue item 192 and then queue item 194, in that order,
/// because the family is the half where being wrong means the page is drawn in
/// another font entirely.
pub fn from_file(path: &Path) -> Option<Face> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > LARGEST_FONT {
        return None;
    }
    // The font's own answers, never the filename. Two tables and two questions:
    // what a page asks for by name, and which face of it to draw with.
    let family = alo_text::family_in(&bytes)?;
    let style = alo_text::style_in(&bytes)?;
    Face::new(family, style.weight, style.slant, bytes)
}

/// Read a directory's fonts, keeping the ones worth keeping.
///
/// Two reasons to keep a face and they are different reasons: there is still
/// room in the short list, or it belongs to a family some generic would like to
/// mean. Without the second, the short list is alphabetical and whether a
/// machine has a `sans-serif` at all is decided by how its fonts sort.
///
/// `looked_at` counts **files opened**, across every directory, so the bound is
/// on the work rather than on any one directory — a machine that keeps two
/// thousand fonts in one place and a machine that spreads them over three cost
/// the same.
fn collect(directory: &Path, into: &mut Vec<Face>, looked_at: &mut usize) {
    for path in readable_in(directory) {
        if *looked_at >= MOST_LOOKED_AT {
            return;
        }
        *looked_at += 1;
        let Some(face) = from_file(&path) else {
            continue;
        };
        let keep = into.len() < MOST_FONTS
            || (into.len() < MOST_KEPT && crate::generic::is_a_candidate(&face.family));
        if keep {
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
