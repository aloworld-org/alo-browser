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
//! So this finds a small set — enough to render text at all — and finding the
//! rest **on demand, when a page asks for a family by name**, is the shape that
//! follows. That is not built here; what is built is the part that makes it
//! possible, which is that a renderer receives fonts rather than opening them.

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

/// One font file, read and named.
///
/// The family is taken from the filename, which is a **guess** — and it is why
/// a renderer answers with the family it actually found rather than echoing
/// this back.
pub fn from_file(path: &Path) -> Option<Face> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > LARGEST_FONT {
        return None;
    }
    let name = path.file_stem()?.to_str()?;
    let slant = if name.to_ascii_lowercase().contains("italic") {
        Slant::Italic
    } else {
        Slant::Normal
    };
    let weight = if name.to_ascii_lowercase().contains("bold") {
        Weight::BOLD
    } else {
        Weight::NORMAL
    };
    Face::new(name, weight, slant, bytes)
}

fn collect(directory: &Path, into: &mut Vec<Face>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
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
    for path in paths {
        if into.len() >= MOST_FONTS {
            return;
        }
        if let Some(face) = from_file(&path) {
            into.push(face);
        }
    }
}
