/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The one CEF build the macOS application carries, pinned, and a
//! distribution checked against the pin before anything of it is copied
//! (queue item 720; ADR 0077, ADR 0079).
//!
//! CEF publishes its builds with a SHA-1 only. So alo pins, for the macOS
//! arm64 *minimal* build of CEF 156.0.14 (Chromium 156.0.8078.48):
//!
//! - the archive's name and CEF's SHA-1 of it ([`ARCHIVE`], [`ARCHIVE_SHA1`]),
//!   which an unpacked distribution names in its `archive.json`;
//! - a SHA-256 of our own of that archive ([`ARCHIVE_SHA256`]), computed
//!   when it was downloaded and recorded here as its provenance;
//! - and what is checked, because what is carried is the unpacked
//!   distribution and not the archive: a SHA-256 of our own of **CEF's
//!   framework** ([`FRAMEWORK_SHA256`], [`framework_digest`]) and of each
//!   licence file carried with it ([`LICENCES`]), computed from a copy
//!   unpacked afresh from the archive whose SHA-256 matched.
//!
//! A framework is a directory, so its digest is over a listing of it: one
//! line per file, `<the file's SHA-256 in hex>  <its path inside the
//! framework>`, in byte order of the path, and the SHA-256 of those lines —
//! what `find . -type f | LC_ALL=C sort` with `shasum -a 256` on each gives,
//! so the number can be made again without this program. A link inside it
//! is refused rather than followed.
//!
//! [`check`] refuses a distribution that is not the pinned one — its
//! `archive.json` naming another archive, or a byte of its framework or a
//! licence different — with a sentence saying what differs. A new CEF is a
//! change to this file, said in a diff (queue item 721 keeps it updated).

use alo_net::digest::Digest;
use core::fmt;
use core::fmt::Write as _;
use std::path::{Path, PathBuf};

/// CEF's version, and the Chromium it is built from.
pub const VERSION: &str = "CEF 156.0.14 (Chromium 156.0.8078.48)";

/// The archive CEF published, for macOS on Apple silicon.
pub const ARCHIVE: &str =
    "cef_binary_156.0.14+g4fa4ab5+chromium-156.0.8078.48_macosarm64_minimal.tar.bz2";

/// CEF's own SHA-1 of [`ARCHIVE`], as its `index.json` and `.sha1` give it.
pub const ARCHIVE_SHA1: &str = "7359a9d293dd5c480bec3f18b762731b483b0e13";

/// Our SHA-256 of [`ARCHIVE`], taken when it was downloaded: its provenance.
pub const ARCHIVE_SHA256: &str = "ed994b907ad596ca609a0542a1e2b22aa97e93e98b8141fed4c24aef39b9ccf6";

/// Our SHA-256 of CEF's framework as [`ARCHIVE`] unpacks it, by
/// [`framework_digest`].
pub const FRAMEWORK_SHA256: &str =
    "aba8ead9a4b1ceae97b5e41da54c7c1573eda991485af0d89d2ccaca440d5899";

/// Each licence file the distribution holds and the application carries,
/// and our SHA-256 of it: CEF's licence, and Chromium's credits — the
/// licence of every project inside it.
pub const LICENCES: [(&str, &str); 2] = [
    (
        "LICENSE.txt",
        "058c3827ffb827ff3edda471ae7e1bb1d1aa5931985f0126043ccd33409e792f",
    ),
    (
        "CREDITS.html",
        "8de6e17eb43cce00aea049179433608219423bb2bf41d34c6e651015a88e6d6a",
    ),
];

/// The file an unpacked distribution names its archive in.
pub const ARCHIVE_JSON: &str = "archive.json";

/// Why a distribution is not the one alo carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unpinned {
    /// No unpacked distribution is there: no framework, or no
    /// [`ARCHIVE_JSON`].
    NoDistribution(PathBuf),
    /// Its [`ARCHIVE_JSON`] names another archive, or another SHA-1: what it
    /// names.
    OtherArchive(PathBuf, String),
    /// A file's bytes, or the framework's, are not the pinned ones: what,
    /// the SHA-256 pinned, and the one found.
    Changed(PathBuf, &'static str, String),
    /// A link inside the framework, which is not followed.
    Link(PathBuf),
    /// The disk refused: what, and its words.
    Unread(PathBuf, String),
}

impl fmt::Display for Unpinned {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDistribution(at) => write!(
                f,
                "{} holds no unpacked CEF distribution: CEF's framework and its {ARCHIVE_JSON} \
                 are not both there",
                at.display()
            ),
            Self::OtherArchive(at, named) => write!(
                f,
                "the CEF distribution at {} is {named}, not the one alo carries, {ARCHIVE} \
                 with SHA-1 {ARCHIVE_SHA1}",
                at.display()
            ),
            Self::Changed(what, pinned, found) => write!(
                f,
                "{} is not the CEF alo pinned: its SHA-256 is {found}, not {pinned}, so it is \
                 not carried",
                what.display()
            ),
            Self::Link(at) => write!(
                f,
                "{} is a link inside CEF's framework, which is not followed, so it is not carried",
                at.display()
            ),
            Self::Unread(what, why) => write!(f, "{} could not be read: {why}", what.display()),
        }
    }
}

impl std::error::Error for Unpinned {}

/// Check the unpacked distribution at `distribution` against the pin: its
/// archive's name and SHA-1, each licence, then the framework — the
/// cheapest first.
///
/// # Errors
///
/// [`Unpinned`], saying the first thing that differs.
pub fn check(distribution: &Path) -> Result<(), Unpinned> {
    let framework = distribution.join(crate::cef_host::FRAMEWORK);
    let named = distribution.join(ARCHIVE_JSON);
    if !framework.is_dir() || !named.is_file() {
        return Err(Unpinned::NoDistribution(distribution.to_path_buf()));
    }
    let text = std::fs::read_to_string(&named).map_err(|why| unread(&named, &why))?;
    let archive = field(&text, "name");
    let sha1 = field(&text, "sha1");
    if archive != Some(ARCHIVE) || sha1 != Some(ARCHIVE_SHA1) {
        let said = format!(
            "{} with SHA-1 {}",
            archive.unwrap_or("an archive it does not name"),
            sha1.unwrap_or("not given")
        );
        return Err(Unpinned::OtherArchive(distribution.to_path_buf(), said));
    }
    for (name, pinned) in LICENCES {
        let file = distribution.join(name);
        let found = file_digest(&file)?;
        if found != pinned {
            return Err(Unpinned::Changed(file, pinned, found));
        }
    }
    let found = framework_digest(&framework)?;
    if found == FRAMEWORK_SHA256 {
        Ok(())
    } else {
        Err(Unpinned::Changed(framework, FRAMEWORK_SHA256, found))
    }
}

/// The SHA-256 of the framework at `framework`, in hex, as this module's
/// header defines it.
///
/// # Errors
///
/// [`Unpinned::Link`] for a link inside it, [`Unpinned::Unread`] when the
/// disk refuses.
pub fn framework_digest(framework: &Path) -> Result<String, Unpinned> {
    let mut files = Vec::new();
    let mut left = vec![framework.to_path_buf()];
    while let Some(directory) = left.pop() {
        let entries = std::fs::read_dir(&directory).map_err(|why| unread(&directory, &why))?;
        for entry in entries {
            let entry = entry.map_err(|why| unread(&directory, &why))?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|why| unread(&path, &why))?;
            if kind.is_symlink() {
                return Err(Unpinned::Link(path));
            } else if kind.is_dir() {
                left.push(path);
            } else {
                let inside = path
                    .strip_prefix(framework)
                    .map_err(|_| Unpinned::Link(path.clone()))?;
                let Some(inside) = inside.to_str() else {
                    return Err(Unpinned::Unread(path, "not a name in UTF-8".to_owned()));
                };
                files.push((inside.to_owned(), path));
            }
        }
    }
    files.sort_by(|(one, _), (other, _)| one.as_bytes().cmp(other.as_bytes()));
    let mut listing = String::new();
    for (inside, path) in &files {
        listing.push_str(&file_digest(path)?);
        listing.push_str("  ");
        listing.push_str(inside);
        listing.push('\n');
    }
    Ok(hex(&Digest::Sha256.of(listing.as_bytes())))
}

/// The SHA-256 of the file at `path`, in hex.
fn file_digest(path: &Path) -> Result<String, Unpinned> {
    let file = std::fs::File::open(path).map_err(|why| unread(path, &why))?;
    Digest::Sha256
        .of_read(file)
        .map(|digest| hex(&digest))
        .map_err(|why| unread(path, &why))
}

/// Why the disk refused `path`.
fn unread(path: &Path, why: &std::io::Error) -> Unpinned {
    Unpinned::Unread(path.to_path_buf(), why.to_string())
}

/// `bytes` in lower-case hex, as a SHA-256 is pinned here.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        // Writing to a `String` cannot fail.
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// The string `key` holds in `archive.json`'s one flat object, if it is a
/// plain one: nothing escaped, which no archive's name or SHA-1 needs.
fn field<'t>(text: &'t str, key: &str) -> Option<&'t str> {
    let (_, after) = text.split_once(&format!("\"{key}\""))?;
    let after = after.trim_start().strip_prefix(':')?.trim_start();
    let (value, _) = after.strip_prefix('"')?.split_once('"')?;
    (!value.contains('\\')).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of this test's own, empty.
    fn directory(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("alo-cef-pin-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::create_dir_all(&root);
        root
    }

    #[test]
    fn a_framework_is_digested_as_its_listing_is_by_shasum() {
        let root = directory("listing");
        let framework = root.join("F.framework");
        let _ = std::fs::create_dir_all(framework.join("Resources"));
        let _ = std::fs::write(framework.join("F"), "abc");
        let _ = std::fs::write(framework.join("Resources/a.b"), "");
        let _ = std::fs::write(framework.join("Resources/a"), "");
        // By `find`, `LC_ALL=C sort` and `shasum -a 256` in a shell, not by
        // the code under test, over the lines
        //   ba78…15ad  F
        //   e3b0…b855  Resources/a
        //   e3b0…b855  Resources/a.b
        assert_eq!(
            framework_digest(&framework).ok().as_deref(),
            Some("07095da7950a681ce85b0dfb0b1c9d8d60bd6285ada841c65a1f9ac7a3fa42ed")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_link_inside_a_framework_is_refused() {
        let root = directory("link");
        let framework = root.join("F.framework");
        let _ = std::fs::create_dir_all(&framework);
        let _ = std::os::unix::fs::symlink("/etc/hosts", framework.join("hosts"));
        assert_eq!(
            framework_digest(&framework),
            Err(Unpinned::Link(framework.join("hosts")))
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn archive_json_is_read_for_the_archive_and_its_sha1() {
        let text =
            "{\n  \"type\": \"minimal\",\n  \"name\": \"cef.tar.bz2\",\n  \"sha1\": \"7359\"\n}";
        assert_eq!(field(text, "name"), Some("cef.tar.bz2"));
        assert_eq!(field(text, "sha1"), Some("7359"));
        assert_eq!(field(text, "size"), None);
        assert_eq!(field(r#"{"name": "a\"b"}"#, "name"), None);
    }

    #[test]
    fn a_distribution_not_the_pinned_one_is_refused_saying_what_differs() {
        let root = directory("other");
        assert_eq!(check(&root), Err(Unpinned::NoDistribution(root.clone())));
        let _ = std::fs::create_dir_all(root.join(crate::cef_host::FRAMEWORK));
        let _ = std::fs::write(
            root.join(ARCHIVE_JSON),
            format!("{{\"name\": \"cef_binary_155.tar.bz2\", \"sha1\": \"{ARCHIVE_SHA1}\"}}"),
        );
        let refused = check(&root);
        assert_eq!(
            refused,
            Err(Unpinned::OtherArchive(
                root.clone(),
                format!("cef_binary_155.tar.bz2 with SHA-1 {ARCHIVE_SHA1}")
            ))
        );
        // The pinned archive named, and a licence that is not its own.
        let _ = std::fs::write(
            root.join(ARCHIVE_JSON),
            format!("{{\"name\": \"{ARCHIVE}\", \"sha1\": \"{ARCHIVE_SHA1}\"}}"),
        );
        let _ = std::fs::write(root.join("LICENSE.txt"), "abc");
        let refused = check(&root);
        let [(_, pinned), _] = LICENCES;
        assert_eq!(
            refused,
            Err(Unpinned::Changed(
                root.join("LICENSE.txt"),
                pinned,
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_owned()
            ))
        );
        assert!(refused.err().is_some_and(|said| {
            said.to_string()
                .ends_with(&format!("not {pinned}, so it is not carried"))
        }),);
        let _ = std::fs::remove_dir_all(&root);
    }
}
