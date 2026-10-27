/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Apple's icon container, `.icns`, written with PNG pictures in it (queue
//! item 689; ADR 0067 § 6).
//!
//! The format is a header and a run of entries, each a four-letter type, a
//! length and its bytes, every length big-endian and counting its own eight
//! bytes. An entry of a type that holds a PNG holds the file as it is, so the
//! container is a few lines of ours rather than Apple's `iconutil`. Which
//! picture goes in which type is [`crate::bundle_icon`]'s.
//!
//! A length has four bytes, so a container over 4 GiB cannot be said and is
//! refused rather than written wrong.

use core::fmt;

/// The container was too large for its lengths to say.
#[derive(Debug, PartialEq, Eq)]
pub struct TooLarge;

impl fmt::Display for TooLarge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the icon is too large for an .icns to hold")
    }
}

impl std::error::Error for TooLarge {}

/// The bytes before an entry's own: its type and its length.
const HEADER: usize = 8;

/// The container holding each `(type, bytes)` in `entries`, in that order.
///
/// # Errors
///
/// [`TooLarge`] when the whole would be 4 GiB or more.
pub fn written(entries: &[([u8; 4], &[u8])]) -> Result<Vec<u8>, TooLarge> {
    let whole = entries.iter().try_fold(HEADER, |sum, (_, bytes)| {
        sum.checked_add(HEADER)?.checked_add(bytes.len())
    });
    let whole = whole.ok_or(TooLarge)?;
    let mut icns = Vec::with_capacity(whole);
    icns.extend_from_slice(b"icns");
    icns.extend_from_slice(&length(whole)?);
    for (kind, bytes) in entries {
        icns.extend_from_slice(kind);
        icns.extend_from_slice(&length(HEADER + bytes.len())?);
        icns.extend_from_slice(bytes);
    }
    Ok(icns)
}

/// `count` as the four big-endian bytes a length is.
fn length(count: usize) -> Result<[u8; 4], TooLarge> {
    u32::try_from(count)
        .map(u32::to_be_bytes)
        .map_err(|_| TooLarge)
}

/// Each entry of a container, as `(type, bytes)`, or [`None`] when `icns` is
/// not one: a wrong header, a length that is not its own, or an entry cut
/// short. For reading back what [`written`] wrote.
pub fn entries(icns: &[u8]) -> Option<Vec<([u8; 4], &[u8])>> {
    let (head, mut rest) = icns.split_at_checked(HEADER)?;
    let (magic, whole) = head.split_at_checked(4)?;
    if magic != b"icns" || said(whole)? != icns.len() {
        return None;
    }
    let mut found = Vec::new();
    while !rest.is_empty() {
        let (head, after) = rest.split_at_checked(HEADER)?;
        let (kind, count) = head.split_at_checked(4)?;
        let (bytes, after) = after.split_at_checked(said(count)?.checked_sub(HEADER)?)?;
        found.push((kind.try_into().ok()?, bytes));
        rest = after;
    }
    Some(found)
}

/// The length four big-endian bytes say.
fn said(bytes: &[u8]) -> Option<usize> {
    let bytes: [u8; 4] = bytes.try_into().ok()?;
    usize::try_from(u32::from_be_bytes(bytes)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_container_is_its_header_and_each_entry_with_its_length() {
        let icns = written(&[(*b"ic07", b"abc"), (*b"ic08", b"")]);
        assert_eq!(
            icns,
            Ok([
                b"icns".as_slice(),
                &[0, 0, 0, 27],
                b"ic07",
                &[0, 0, 0, 11],
                b"abc",
                b"ic08",
                &[0, 0, 0, 8],
            ]
            .concat())
        );
    }

    #[test]
    fn what_is_written_reads_back() {
        let entries_in: [([u8; 4], &[u8]); 3] = [
            (*b"icp4", b"\x89PNG sixteen"),
            (*b"ic10", b"\x89PNG a thousand"),
            (*b"ic11", b""),
        ];
        let icns = written(&entries_in).unwrap_or_default();
        assert_eq!(entries(&icns), Some(entries_in.to_vec()));
        assert_eq!(written(&[]), Ok(b"icns\0\0\0\x08".to_vec()));
        assert_eq!(entries(b"icns\0\0\0\x08"), Some(Vec::new()));
    }

    #[test]
    fn what_is_not_a_container_is_not_read() {
        let icns = written(&[(*b"ic07", b"abc")]).unwrap_or_default();
        assert_eq!(entries(&icns[..icns.len() - 1]), None, "cut short");
        let mut long = icns.clone();
        long.push(0);
        assert_eq!(entries(&long), None, "longer than it says");
        let mut wrong = icns.clone();
        wrong[0] = b'x';
        assert_eq!(entries(&wrong), None, "not icns");
        let mut short_entry = icns;
        short_entry[15] = 7;
        assert_eq!(
            entries(&short_entry),
            None,
            "an entry shorter than its head"
        );
        assert_eq!(entries(b"icn"), None);
        assert_eq!(entries(b""), None);
    }

    #[test]
    fn a_length_over_four_bytes_is_refused() {
        assert_eq!(length(usize::try_from(u32::MAX).unwrap_or(0)), Ok([255; 4]));
        if let Some(over) = usize::try_from(u32::MAX)
            .ok()
            .and_then(|m| m.checked_add(1))
        {
            assert_eq!(length(over), Err(TooLarge));
        }
        assert_eq!(
            TooLarge.to_string(),
            "the icon is too large for an .icns to hold"
        );
    }
}
