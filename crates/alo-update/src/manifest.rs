/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The manifest: what a release says of itself, in plain lines (ADR 0116 §
//! 3), and the one file of alo's own that reads it — no JSON, no TOML.
//!
//! ```text
//! alo-update 1
//! platform macos-arm64
//! version 0.4.2
//! serial 118
//! published 2026-11-14T09:00:00Z
//! expires 2026-11-28T09:00:00Z
//! minimum 0.3.0
//! chromium 156.0.8078.96
//! package https://…/alo-browser-0.4.2-macos-arm64.zip
//! size 158204416
//! sha256 3f6c…
//! security yes
//! behind no
//! notes Chromium 156.0.8078.96, with Chrome's security fixes of 12 November.
//! ```
//!
//! # Read only after its signature
//!
//! The bytes this reads are a stranger's until [`crate::signature`] has
//! said one of the built-in keys signed them; [`crate::check`] asks that
//! first. Even so this file trusts nothing it reads: a signed manifest
//! that does not read as written here is refused, not half used.
//!
//! # Bounded, and one spelling
//!
//! - At most [`LARGEST`] bytes, UTF-8, lines ending in `\n` alone, the last
//!   one too or not; no carriage return, no tab, no other control
//!   character anywhere.
//! - The first line is `alo-update 1`. A later format is another number,
//!   refused here rather than half read.
//! - Every other line is a name, one space and its value. Each name once;
//!   each but `notes` required; a name alo does not know refused — the
//!   manifest is signed whole, so nothing is in it that its signer did not
//!   mean and this reader does not read.
//! - Each value as its own file reads it: [`Version`], [`Instant`],
//!   numbers in decimal with no leading zero, the digest as 64 lower-case
//!   hexadecimal digits, `yes` or `no`, the package's address as
//!   [`crate::address::allowed`] allows, `notes` at most [`LONGEST_NOTES`]
//!   characters.

use crate::address;
use crate::instant::Instant;
use crate::version::Version;
use alo_url::Url;
use core::fmt;
use core::fmt::Write as _;

/// The most bytes a manifest is read to: far past any a release writes.
pub const LARGEST: usize = 4096;

/// The most characters of `notes`.
pub const LONGEST_NOTES: usize = 400;

/// The most bytes of a package, as any manifest may say it: 2 GiB, more
/// than ten times what alo is (ADR 0116 § 1, about 150 MB).
pub const LARGEST_PACKAGE: u64 = 2 * 1024 * 1024 * 1024;

/// The first line, and the only format this reads.
pub const FORMAT: &str = "alo-update 1";

/// A manifest, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// The platform it is for: `macos-arm64`.
    pub platform: String,
    /// The version it offers.
    pub version: Version,
    /// The release's own count, which only rises.
    pub serial: u64,
    /// When it was signed.
    pub published: Instant,
    /// When it stops being believed.
    pub expires: Instant,
    /// The oldest installed version it may update.
    pub minimum: Version,
    /// The Chromium the version carries, as CEF names it.
    pub chromium: String,
    /// Where the package is.
    pub package: Url,
    /// The package's length in bytes.
    pub size: u64,
    /// The package's SHA-256.
    pub sha256: [u8; 32],
    /// Whether it carries Chrome's security fixes.
    pub security: bool,
    /// Whether CEF has no build for Chrome's latest security release.
    pub behind: bool,
    /// A line for the person, if any.
    pub notes: Option<String>,
}

/// Why a manifest did not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unread {
    /// Why, in words.
    pub why: String,
}

impl fmt::Display for Unread {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the manifest does not read: {}", self.why)
    }
}

/// The fields as they are found, each once.
#[derive(Default)]
struct Found {
    platform: Option<String>,
    version: Option<Version>,
    serial: Option<u64>,
    published: Option<Instant>,
    expires: Option<Instant>,
    minimum: Option<Version>,
    chromium: Option<String>,
    package: Option<Url>,
    size: Option<u64>,
    sha256: Option<[u8; 32]>,
    security: Option<bool>,
    behind: Option<bool>,
    notes: Option<String>,
}

/// The manifest `bytes` are, or why they are not one.
///
/// # Errors
///
/// [`Unread`], saying which rule of this file's note was broken.
pub fn read(bytes: &[u8]) -> Result<Manifest, Unread> {
    let unread = |why: String| Unread { why };
    if bytes.len() > LARGEST {
        return Err(unread(format!("it is longer than {LARGEST} bytes")));
    }
    let text = core::str::from_utf8(bytes).map_err(|_| unread("it is not text".to_owned()))?;
    if text.chars().any(|each| each.is_control() && each != '\n') {
        return Err(unread(
            "it holds a control character other than a line's end".to_owned(),
        ));
    }
    let body = text.strip_suffix('\n').unwrap_or(text);
    let mut lines = body.split('\n');
    if lines.next() != Some(FORMAT) {
        return Err(unread(format!("its first line is not {FORMAT:?}")));
    }
    let mut found = Found::default();
    for line in lines {
        let (name, value) = line
            .split_once(' ')
            .ok_or_else(|| unread(format!("a line is not a name and a value: {}", cut(line))))?;
        found.take(name, value).map_err(unread)?;
    }
    found.finished().map_err(unread)
}

impl Found {
    /// Read `value` as `name`'s, once.
    fn take(&mut self, name: &str, value: &str) -> Result<(), String> {
        let twice = || format!("it names {name} twice");
        let bad = || format!("its {name} does not read: {}", cut(value));
        if value.is_empty() || value.starts_with(' ') || value.ends_with(' ') {
            return Err(bad());
        }
        macro_rules! once {
            ($field:ident, $read:expr) => {{
                let read = $read.ok_or_else(bad)?;
                if self.$field.replace(read).is_some() {
                    return Err(twice());
                }
            }};
        }
        match name {
            "platform" => once!(platform, platform(value)),
            "version" => once!(version, Version::read(value)),
            "serial" => once!(serial, decimal(value)),
            "published" => once!(published, Instant::read(value)),
            "expires" => once!(expires, Instant::read(value)),
            "minimum" => once!(minimum, Version::read(value)),
            "chromium" => once!(chromium, chromium(value)),
            "package" => once!(package, package(value)),
            "size" => once!(
                size,
                decimal(value).filter(|size| (1..=LARGEST_PACKAGE).contains(size))
            ),
            "sha256" => once!(sha256, digest(value)),
            "security" => once!(security, yes_or_no(value)),
            "behind" => once!(behind, yes_or_no(value)),
            "notes" => once!(
                notes,
                Some(value.to_owned()).filter(|notes| notes.chars().count() <= LONGEST_NOTES)
            ),
            other => return Err(format!("it names {}, which alo does not read", cut(other))),
        }
        Ok(())
    }

    /// The manifest, if every field it must name is named.
    fn finished(self) -> Result<Manifest, String> {
        let missing = |name: &str| format!("it names no {name}");
        let manifest = Manifest {
            platform: self.platform.ok_or_else(|| missing("platform"))?,
            version: self.version.ok_or_else(|| missing("version"))?,
            serial: self.serial.ok_or_else(|| missing("serial"))?,
            published: self.published.ok_or_else(|| missing("published"))?,
            expires: self.expires.ok_or_else(|| missing("expires"))?,
            minimum: self.minimum.ok_or_else(|| missing("minimum"))?,
            chromium: self.chromium.ok_or_else(|| missing("chromium"))?,
            package: self.package.ok_or_else(|| missing("package"))?,
            size: self.size.ok_or_else(|| missing("size"))?,
            sha256: self.sha256.ok_or_else(|| missing("sha256"))?,
            security: self.security.ok_or_else(|| missing("security"))?,
            behind: self.behind.ok_or_else(|| missing("behind"))?,
            notes: self.notes,
        };
        if manifest.expires <= manifest.published {
            return Err("it expires before it was published".to_owned());
        }
        if manifest.minimum > manifest.version {
            return Err("its minimum is above its own version".to_owned());
        }
        Ok(manifest)
    }
}

impl Manifest {
    /// The manifest as its lines, as [`read`] reads them back: what the
    /// small tool of ADR 0116 § 7 writes before its owner signs it.
    pub fn lines(&self) -> String {
        let mut hex = String::with_capacity(64);
        for byte in self.sha256 {
            let _ = write!(hex, "{byte:02x}");
        }
        let word = |yes: bool| if yes { "yes" } else { "no" };
        let mut lines = format!(
            "{FORMAT}\nplatform {}\nversion {}\nserial {}\npublished {}\nexpires {}\n\
             minimum {}\nchromium {}\npackage {}\nsize {}\nsha256 {hex}\nsecurity {}\nbehind {}\n",
            self.platform,
            self.version,
            self.serial,
            self.published,
            self.expires,
            self.minimum,
            self.chromium,
            self.package.serialised,
            self.size,
            word(self.security),
            word(self.behind),
        );
        if let Some(notes) = &self.notes {
            lines.push_str("notes ");
            lines.push_str(notes);
            lines.push('\n');
        }
        lines
    }
}

/// A platform's word: lower-case letters, digits and `-`, at most 32.
fn platform(value: &str) -> Option<String> {
    let fits = value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    fits.then(|| value.to_owned())
}

/// Chromium's version as CEF names it: four numbers.
fn chromium(value: &str) -> Option<String> {
    let parts: Vec<&str> = value.split('.').collect();
    let fits = parts.len() == 4 && parts.iter().all(|part| decimal(part).is_some());
    fits.then(|| value.to_owned())
}

/// A number in decimal, no sign and no leading zero but a lone `0`.
fn decimal(value: &str) -> Option<u64> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return None;
    }
    value.parse().ok()
}

/// A SHA-256 as 64 lower-case hexadecimal digits.
fn digest(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut digest = [0u8; 32];
    let nibble = |byte: u8| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    };
    for (into, pair) in digest.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        let (high, low) = match pair {
            [high, low] => (nibble(*high)?, nibble(*low)?),
            _ => return None,
        };
        *into = high << 4 | low;
    }
    Some(digest)
}

/// `yes` or `no`.
fn yes_or_no(value: &str) -> Option<bool> {
    match value {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

/// The package's address, as an update's may be.
fn package(value: &str) -> Option<Url> {
    if value.len() > address::LONGEST {
        return None;
    }
    let url = alo_url::parse(value).ok()?;
    address::allowed(&url).ok()?;
    Some(url)
}

/// At most 40 characters of something read, said back.
fn cut(text: &str) -> String {
    format!("{:?}", text.chars().take(40).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "3f6c00112233445566778899aabbccddeeff00112233445566778899aabbccdd";

    fn written() -> String {
        format!(
            "alo-update 1\nplatform macos-arm64\nversion 0.4.2\nserial 118\n\
             published 2026-11-14T09:00:00Z\nexpires 2026-11-28T09:00:00Z\nminimum 0.3.0\n\
             chromium 156.0.8078.96\npackage https://updates.example.org/alo-browser-0.4.2-macos-arm64.zip\n\
             size 158204416\nsha256 {SHA}\nsecurity yes\nbehind no\n\
             notes Chromium 156.0.8078.96, with Chrome\u{2019}s security fixes of 12 November.\n"
        )
    }

    #[test]
    fn the_adrs_manifest_reads_and_writes_back_the_same_bytes() {
        let manifest = read(written().as_bytes()).expect("read");
        assert_eq!(manifest.platform, "macos-arm64");
        assert_eq!(manifest.version, Version::of(0, 4, 2));
        assert_eq!(manifest.serial, 118);
        assert_eq!(manifest.published, Instant::at(1_794_646_800));
        assert_eq!(manifest.minimum, Version::of(0, 3, 0));
        assert_eq!(manifest.chromium, "156.0.8078.96");
        assert_eq!(manifest.size, 158_204_416);
        assert_eq!(manifest.sha256.first(), Some(&0x3f));
        assert_eq!(manifest.sha256.last(), Some(&0xdd));
        assert!(manifest.security && !manifest.behind);
        assert_eq!(manifest.lines(), written());
        // The last line's end may be left off.
        let unended = written();
        assert_eq!(
            read(unended.trim_end_matches('\n').as_bytes()),
            Ok(manifest)
        );
    }

    #[test]
    fn every_broken_rule_is_refused_and_said() {
        let with = |from: &str, to: &str| written().replacen(from, to, 1);
        let cases: Vec<(String, &str)> = vec![
            (with("alo-update 1", "alo-update 2"), "its first line"),
            (with("serial 118\n", ""), "it names no serial"),
            (
                with("serial 118", "serial 118\nserial 119"),
                "it names serial twice",
            ),
            (
                with("serial 118", "serial 0118"),
                "its serial does not read",
            ),
            (with("serial 118", "serial -1"), "its serial does not read"),
            (
                with("serial 118", "serial  118"),
                "its serial does not read",
            ),
            (
                with("version 0.4.2", "version 0.4"),
                "its version does not read",
            ),
            (with("size 158204416", "size 0"), "its size does not read"),
            (
                with("size 158204416", "size 2147483649"),
                "its size does not read",
            ),
            (with(SHA, &SHA.to_uppercase()), "its sha256 does not read"),
            (with(SHA, &SHA[..62]), "its sha256 does not read"),
            (
                with("security yes", "security true"),
                "its security does not read",
            ),
            (
                with("chromium 156.0.8078.96", "chromium 156.0"),
                "its chromium does not read",
            ),
            (
                with("https://updates.example.org", "http://updates.example.org"),
                "its package does not read",
            ),
            (with(".zip", ".zip?who=me"), "its package does not read"),
            (
                with("platform macos-arm64", "platform macOS"),
                "its platform does not read",
            ),
            (
                with("behind no", "behind no\nkey RWQ"),
                "which alo does not read",
            ),
            (
                with("behind no", "behind no\n"),
                "a line is not a name and a value",
            ),
            (with("behind no", "behind\tno"), "control character"),
            (with("behind no", "behind no\r"), "control character"),
            (
                with("expires 2026-11-28", "expires 2026-11-13"),
                "expires before",
            ),
            (with("minimum 0.3.0", "minimum 0.5.0"), "minimum is above"),
            (
                with("notes ", &format!("notes {}", "n".repeat(LONGEST_NOTES))),
                "its notes does not read",
            ),
            (
                format!("{}{}", written(), "#".repeat(LARGEST)),
                "longer than 4096",
            ),
        ];
        for (bytes, why) in cases {
            let refused = read(bytes.as_bytes()).expect_err(why);
            assert!(refused.why.contains(why), "{why}: {refused}");
        }
        assert!(read(b"alo-update 1\nplatform \xff\n").is_err());
        // Notes are the one field that may be left out.
        let without: String = written()
            .lines()
            .filter(|line| !line.starts_with("notes "))
            .flat_map(|line| [line, "\n"])
            .collect();
        assert!(read(without.as_bytes()).is_ok_and(|read| read.notes.is_none()));
    }
}
