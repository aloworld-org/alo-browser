/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The package, downloaded: written to `<version>.partial` in the
//! updater's folder piece by piece, resumed where it stopped, and renamed
//! `<version>.zip` only once it is exactly `size` bytes and its SHA-256 is
//! the manifest's (ADR 0116 § 3). Nothing of the installed application is
//! touched.
//!
//! # In pieces, each a range
//!
//! Each request asks for the next piece, at most [`PIECE`] bytes, by
//! `Range: bytes=FIRST-LAST`, and what comes back is placed by rules that
//! protect the file:
//!
//! - **A `206` must start exactly where the file stops**, carry at most what
//!   was asked, and say the whole is `size` — a server whose file is a byte
//!   longer or shorter than the manifest says is refused at once, and the
//!   `.partial` removed.
//! - **A `200` is the file from its first byte**, whatever was asked: a
//!   server that ignores `Range` sends the whole thing, which must then be
//!   exactly `size` bytes, and replaces what was held.
//! - **A cut** — the connection closed half way, the network gone — ends
//!   this try and keeps every piece already written. The next try resumes
//!   from there. A piece is written only whole, so the `.partial` is always
//!   a prefix of what the server sent.
//!
//! # The signed digest is the validator
//!
//! A resumed download usually needs an `ETag` to know the file did not
//! change between pieces. Here the manifest names the file's SHA-256, and
//! the manifest is signed: a `.partial` spliced from two files is a file
//! whose digest is not the manifest's, refused and removed, and the next
//! try starts again. So a splice can cost a download, never an install.

use crate::check::Refused;
use crate::feed;
use crate::manifest::Manifest;
use alo_net::digest::Digest;
use alo_net::{Cause, Pool};
use core::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// The most bytes one piece asks for: 16 MiB, so a cut costs at most that.
pub const PIECE: u64 = 16 * 1024 * 1024;

/// Why a download did not finish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unfinished {
    /// It was cut; the `.partial` keeps what came, and the next try resumes.
    Cut {
        /// Why, in words.
        why: String,
        /// The bytes kept.
        kept: u64,
    },
    /// What came is not the package the manifest names; the `.partial` is
    /// removed.
    Refused {
        /// Why, in words.
        why: String,
    },
    /// The folder could not be written.
    Unwritten {
        /// Why, in words.
        why: String,
    },
}

impl fmt::Display for Unfinished {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unfinished::Cut { why, kept } => write!(
                f,
                "The update\u{2019}s download stopped after {kept} bytes ({why}); it resumes \
                 at the next check."
            ),
            Unfinished::Refused { why } => write!(f, "The update was refused: {why}."),
            Unfinished::Unwritten { why } => {
                write!(f, "The update could not be kept: {why}.")
            }
        }
    }
}

impl From<Refused> for Unfinished {
    fn from(refused: Refused) -> Self {
        Unfinished::Refused {
            why: refused.to_string(),
        }
    }
}

/// Where `manifest`'s package is kept while it comes.
pub fn partial_in(folder: &Path, manifest: &Manifest) -> PathBuf {
    folder.join(format!("{}.partial", manifest.version))
}

/// Where `manifest`'s package is kept once it is checked.
pub fn whole_in(folder: &Path, manifest: &Manifest) -> PathBuf {
    folder.join(format!("{}.zip", manifest.version))
}

/// Download `manifest`'s package into `folder`, in pieces of at most
/// `piece` bytes, each request caused by `cause`: the checked file.
///
/// # Errors
///
/// [`Unfinished`], saying whether the next try resumes.
pub fn download(
    pool: &mut Pool,
    manifest: &Manifest,
    folder: &Path,
    (cause, piece): (&Cause, u64),
) -> Result<PathBuf, Unfinished> {
    let whole = whole_in(folder, manifest);
    if whole.is_file() && checked(&whole, manifest).is_ok() {
        return Ok(whole);
    }
    let _ = fs::remove_file(&whole);
    let partial = partial_in(folder, manifest);
    let piece = piece.max(1);
    // Every answer moves the file on by at least a byte or ends the try, so
    // this is a backstop, never the bound a download meets.
    let most = manifest.size.div_ceil(piece).saturating_add(8);
    for _ in 0..most {
        let have = length(&partial);
        if have == manifest.size {
            return finished(&partial, &whole, manifest);
        }
        if have > manifest.size {
            return Err(refused(&partial, "it is longer than the manifest says"));
        }
        let last = have.saturating_add(piece).min(manifest.size) - 1;
        let range = format!("bytes={have}-{last}");
        let answer = feed::get(
            pool,
            (&manifest.package, cause),
            // One byte past the whole, so a `200` that is longer is read far
            // enough to be refused as longer, not cut as if the network had.
            manifest
                .size
                .saturating_add(1)
                .min(alo_net::body::LARGEST_BODY),
            Some(&range),
        );
        let response = match answer {
            Ok(response) => response,
            Err(why) => return Err(Unfinished::Cut { why, kept: have }),
        };
        place(&partial, manifest, (have, last), &response)?;
    }
    Err(Unfinished::Cut {
        why: "the server stopped giving more".to_owned(),
        kept: length(&partial),
    })
}

/// Put `response`, the answer to a range of `asked`, in `partial`.
fn place(
    partial: &Path,
    manifest: &Manifest,
    (have, last): (u64, u64),
    response: &alo_net::Response,
) -> Result<(), Unfinished> {
    let arrived = u64::try_from(response.body.len()).unwrap_or(u64::MAX);
    match response.status.0 {
        206 => {
            let sent = alo_net::range::what_was_sent(&response.headers)
                .ok()
                .flatten()
                .ok_or_else(|| refused(partial, "a piece came with no range it says"))?;
            if sent.complete != Some(manifest.size) {
                return Err(refused(
                    partial,
                    "the package is not the size the manifest says",
                ));
            }
            if sent.first != have || sent.last > last || arrived != sent.count() {
                return Err(refused(
                    partial,
                    "a piece came for another place in the file",
                ));
            }
            append(partial, &response.body)
        }
        200 => {
            if arrived != manifest.size {
                return Err(refused(
                    partial,
                    "the package is not the size the manifest says",
                ));
            }
            let _ = fs::remove_file(partial);
            append(partial, &response.body)
        }
        status => Err(Unfinished::Cut {
            why: format!("the server answered {status}"),
            kept: have,
        }),
    }
}

/// Add `bytes` to the end of `partial`, flushed.
fn append(partial: &Path, bytes: &[u8]) -> Result<(), Unfinished> {
    let unwritten = |why: std::io::Error| Unfinished::Unwritten {
        why: format!("{} could not be written: {why}", partial.display()),
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(partial)
        .map_err(unwritten)?;
    file.write_all(bytes).map_err(unwritten)?;
    file.sync_all().map_err(unwritten)
}

/// The `.partial`, all of it here: checked and renamed whole, or refused
/// and removed.
fn finished(partial: &Path, whole: &Path, manifest: &Manifest) -> Result<PathBuf, Unfinished> {
    if let Err(why) = checked(partial, manifest) {
        return Err(refused(partial, why));
    }
    fs::rename(partial, whole).map_err(|why| Unfinished::Unwritten {
        why: format!("{} could not be renamed: {why}", partial.display()),
    })?;
    Ok(whole.to_path_buf())
}

/// Whether `file` is exactly `manifest`'s package: its size and SHA-256.
///
/// # Errors
///
/// Why not, in words.
pub fn checked(file: &Path, manifest: &Manifest) -> Result<(), &'static str> {
    if length(file) != manifest.size {
        return Err("the package is not the size the manifest says");
    }
    let digest = File::open(file)
        .and_then(|opened| Digest::Sha256.of_read(opened))
        .map_err(|_| "the package could not be read back")?;
    if digest.as_slice() != manifest.sha256 {
        return Err("the package\u{2019}s SHA-256 is not the manifest\u{2019}s");
    }
    Ok(())
}

/// `partial` removed, and why.
fn refused(partial: &Path, why: &str) -> Unfinished {
    let _ = fs::remove_file(partial);
    Unfinished::Refused {
        why: why.to_owned(),
    }
}

/// How many bytes `file` holds; none if it is not there.
fn length(file: &Path) -> u64 {
    fs::metadata(file).map_or(0, |meta| meta.len())
}
