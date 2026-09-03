/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The Huffman code HPACK uses for header names and values.
//!
//! # Why the codes are derived rather than written down
//!
//! The specification prints 257 rows of symbol, code and length. Copying them
//! is 257 chances to introduce a bug that only shows up on the one byte nobody
//! tested — and a wrong code is not a crash, it is a header that silently
//! decodes to something else.
//!
//! The code is **canonical**: sort the symbols by length, and the codes are
//! consecutive within each length, with each new length starting where the
//! previous one left off shifted along by one. So the only thing that has to be
//! written down is **which symbols have which length, in order** — the codes
//! follow. That is a third as much data and, more usefully, a transcription
//! error in it almost always breaks the structure rather than one entry, which
//! a test can see.
//!
//! Two tests check the structure itself: that the code space is exactly filled
//! (Kraft's equality, which catches a wrong length anywhere), and that every one
//! of the 256 bytes survives a round trip.

use super::ErrorCode;
use super::frame::Broken;

/// The symbol every table ends with, which may never appear in a decoded
/// string. A sender that emits it is either confused or probing.
const END_OF_STRING: u16 = 256;

/// Which symbols have which code length, in code order.
///
/// This is the whole table. Everything else is derived.
const BY_LENGTH: &[(u8, &[u16])] = &[
    (5, &[48, 49, 50, 97, 99, 101, 105, 111, 115, 116]),
    (
        6,
        &[
            32, 37, 45, 46, 47, 51, 52, 53, 54, 55, 56, 57, 61, 65, 95, 98, 100, 102, 103, 104,
            108, 109, 110, 112, 114, 117,
        ],
    ),
    (
        7,
        &[
            58, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86,
            87, 89, 106, 107, 113, 118, 119, 120, 121, 122,
        ],
    ),
    (8, &[38, 42, 44, 59, 88, 90]),
    (10, &[33, 34, 40, 41, 63]),
    (11, &[39, 43, 124]),
    (12, &[35, 62]),
    (13, &[0, 36, 64, 91, 93, 126]),
    (14, &[94, 125]),
    (15, &[60, 96, 123]),
    (19, &[92, 195, 208]),
    (20, &[128, 130, 131, 162, 184, 194, 224, 226]),
    (
        21,
        &[
            153, 161, 167, 172, 176, 177, 179, 209, 216, 217, 227, 229, 230,
        ],
    ),
    (
        22,
        &[
            129, 132, 133, 134, 136, 146, 154, 156, 160, 163, 164, 169, 170, 173, 178, 181, 185,
            186, 187, 189, 190, 196, 198, 228, 232, 233,
        ],
    ),
    (
        23,
        &[
            1, 135, 137, 138, 139, 140, 141, 143, 147, 149, 150, 151, 152, 155, 157, 158, 165, 166,
            168, 174, 175, 180, 182, 183, 188, 191, 197, 231, 239,
        ],
    ),
    (
        24,
        &[9, 142, 144, 145, 148, 159, 171, 206, 215, 225, 236, 237],
    ),
    (25, &[199, 207, 234, 235]),
    (
        26,
        &[
            192, 193, 200, 201, 202, 205, 210, 213, 218, 219, 238, 240, 242, 243, 255,
        ],
    ),
    (
        27,
        &[
            203, 204, 211, 212, 214, 221, 222, 223, 241, 244, 245, 246, 247, 248, 250, 251, 252,
            253, 254,
        ],
    ),
    (
        28,
        &[
            2, 3, 4, 5, 6, 7, 8, 11, 12, 14, 15, 16, 17, 18, 19, 20, 21, 23, 24, 25, 26, 27, 28,
            29, 30, 31, 127, 220, 249,
        ],
    ),
    (30, &[10, 13, 22, END_OF_STRING]),
];

/// A symbol's code and how many bits of it there are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Code {
    /// The code, right-aligned in a `u32`.
    pub bits: u32,
    /// How many bits of it count.
    pub length: u8,
}

/// The code for every symbol, by symbol.
///
/// Built once, from [`BY_LENGTH`], by walking the lengths in order and handing
/// out consecutive codes — which is the definition of a canonical code.
fn codes() -> [Code; 257] {
    let mut table = [Code { bits: 0, length: 0 }; 257];
    let mut next: u32 = 0;
    let mut previous: u8 = 0;
    for (length, symbols) in BY_LENGTH {
        // Each new length starts where the last one stopped, shifted along by
        // the difference. This is the whole of "canonical".
        next <<= u32::from(length - previous);
        previous = *length;
        for symbol in *symbols {
            if let Some(slot) = table.get_mut(usize::from(*symbol)) {
                *slot = Code {
                    bits: next,
                    length: *length,
                };
            }
            next += 1;
        }
    }
    table
}

/// The codes, built on first use.
fn table() -> &'static [Code; 257] {
    static TABLE: std::sync::OnceLock<[Code; 257]> = std::sync::OnceLock::new();
    TABLE.get_or_init(codes)
}

/// A node of the decoding tree, as an index into a flat vector.
///
/// A tree rather than a byte-at-a-time table: the longest code is thirty bits,
/// so a lookup table wide enough to decode a symbol in one step would be a
/// gigabyte. Walking bits is slower and fits in a cache line, and law 3 says
/// correct before fast.
#[derive(Debug, Clone, Copy)]
struct Branch {
    zero: Option<u32>,
    one: Option<u32>,
    symbol: Option<u16>,
}

fn tree() -> &'static Vec<Branch> {
    static TREE: std::sync::OnceLock<Vec<Branch>> = std::sync::OnceLock::new();
    TREE.get_or_init(|| {
        let mut nodes = vec![Branch {
            zero: None,
            one: None,
            symbol: None,
        }];
        for (symbol, code) in table().iter().enumerate() {
            if code.length == 0 {
                continue;
            }
            let mut at = 0usize;
            for step in (0..code.length).rev() {
                let bit = (code.bits >> step) & 1;
                let existing = nodes
                    .get(at)
                    .and_then(|node| if bit == 0 { node.zero } else { node.one });
                let next = if let Some(already) = existing {
                    already as usize
                } else {
                    nodes.push(Branch {
                        zero: None,
                        one: None,
                        symbol: None,
                    });
                    let made = u32::try_from(nodes.len() - 1).unwrap_or(0);
                    if let Some(node) = nodes.get_mut(at) {
                        if bit == 0 {
                            node.zero = Some(made);
                        } else {
                            node.one = Some(made);
                        }
                    }
                    made as usize
                };
                at = next;
            }
            if let Some(node) = nodes.get_mut(at) {
                node.symbol = u16::try_from(symbol).ok();
            }
        }
        nodes
    })
}

/// Encode bytes.
pub fn encode(bytes: &[u8]) -> Vec<u8> {
    let table = table();
    let mut out = Vec::new();
    let mut held: u64 = 0;
    let mut bits: u32 = 0;
    for byte in bytes {
        let code = table
            .get(usize::from(*byte))
            .copied()
            .unwrap_or(Code { bits: 0, length: 0 });
        held = (held << code.length) | u64::from(code.bits);
        bits += u32::from(code.length);
        while bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((held >> bits) & 0xff).unwrap_or(0));
        }
    }
    if bits > 0 {
        // The padding is the most significant bits of the end-of-string code,
        // which is all ones. It is what a decoder checks to tell padding from a
        // truncated symbol.
        let pad = 8 - bits;
        let last = (held << pad) | ((1u64 << pad) - 1);
        out.push(u8::try_from(last & 0xff).unwrap_or(0));
    }
    out
}

/// Decode bytes.
///
/// # Errors
///
/// [`Broken`] with [`ErrorCode::CompressionError`] for a string that is not a
/// valid encoding. Three ways it can fail, and all three are refusals rather
/// than best guesses:
///
/// - a symbol that runs off the end of the input,
/// - padding longer than seven bits, which means a whole symbol was left out,
/// - padding that is not all ones, or the end-of-string symbol appearing —
///   both of which mean the encoder was doing something other than encoding.
pub fn decode(bytes: &[u8], most: usize) -> Result<Vec<u8>, Broken> {
    let nodes = tree();
    let mut out = Vec::new();
    let mut at = 0usize;
    let mut since_a_symbol = 0u32;
    for byte in bytes {
        for step in (0..8).rev() {
            let bit = (byte >> step) & 1;
            let next = nodes
                .get(at)
                .and_then(|node| if bit == 0 { node.zero } else { node.one });
            let Some(next) = next else {
                return Err(compression("a Huffman code that is not in the table"));
            };
            at = next as usize;
            since_a_symbol += 1;
            if let Some(symbol) = nodes.get(at).and_then(|node| node.symbol) {
                if symbol == END_OF_STRING {
                    return Err(compression(
                        "an end-of-string symbol inside a Huffman string, which may never appear",
                    ));
                }
                if out.len() >= most {
                    return Err(compression(
                        "a Huffman string that decodes to more than this engine holds",
                    ));
                }
                out.push(u8::try_from(symbol).unwrap_or(0));
                at = 0;
                since_a_symbol = 0;
            }
        }
    }
    // What is left over must be padding: fewer than eight bits, and all ones.
    if since_a_symbol >= 8 {
        return Err(compression(
            "a Huffman string ending with a whole symbol's worth of padding",
        ));
    }
    if since_a_symbol > 0 && !is_all_ones(at) {
        return Err(compression(
            "a Huffman string padded with something other than ones",
        ));
    }
    Ok(out)
}

/// Whether every step from here down the all-ones path is still on the tree,
/// which is what "the padding is the top of the end-of-string code" means.
fn is_all_ones(at: usize) -> bool {
    let nodes = tree();
    let mut walk = at;
    loop {
        match nodes.get(walk) {
            // Reaching a symbol means the padding was long enough to be one,
            // which the length check above has already refused.
            Some(node) => match node.one {
                Some(next) => walk = next as usize,
                None => return false,
            },
            None => return false,
        }
        if nodes.get(walk).and_then(|node| node.symbol) == Some(END_OF_STRING) {
            return true;
        }
        // The end-of-string code is thirty bits; anything longer than a byte of
        // walking means this is not its prefix.
        if walk == at {
            return false;
        }
        if nodes.get(walk).and_then(|node| node.symbol).is_some() {
            return false;
        }
    }
}

/// Whether encoding these bytes would make them shorter.
///
/// A sender is allowed to send either, and sending the longer one is legal and
/// silly. Checked rather than assumed, because Huffman makes some strings —
/// anything mostly outside ASCII — longer.
pub fn worth_it(bytes: &[u8]) -> bool {
    let table = table();
    let bits: u32 = bytes
        .iter()
        .map(|byte| u32::from(table.get(usize::from(*byte)).map_or(8, |code| code.length)))
        .sum();
    bits.div_ceil(8) < u32::try_from(bytes.len()).unwrap_or(u32::MAX)
}

fn compression(why: &str) -> Broken {
    Broken {
        why: why.to_owned(),
        error: ErrorCode::CompressionError,
        // Always. The dynamic table carries state from one block to the next,
        // so a block nobody could decode leaves the table in a condition
        // nobody can reason about — every later block would be decoded against
        // something quietly wrong.
        fatal: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kraft's equality: a code that uses its space exactly sums to one. A
    /// single wrong length anywhere in `BY_LENGTH` breaks this, which is why
    /// deriving the codes is safer than copying them.
    #[test]
    fn the_code_space_is_exactly_filled() {
        // Counted in units of the shortest possible code rather than in
        // fractions: the longest code is thirty bits, so a code of length `n`
        // occupies `2^(30-n)` of them, and a full space is exactly `2^30`.
        // Integers, because a floating-point sum of 257 fractions is a test
        // that can fail for a reason that is not the table.
        let mut total: u64 = 0;
        let mut symbols = 0;
        for (length, list) in BY_LENGTH {
            total += list.len() as u64 * (1u64 << (30 - u32::from(*length)));
            symbols += list.len();
        }
        assert_eq!(symbols, 257, "the table does not have 257 symbols");
        assert_eq!(
            total,
            1u64 << 30,
            "the code space is not exactly filled — a length is wrong"
        );
    }

    /// The codes the specification prints, for symbols spread across every
    /// length. If deriving them were wrong, these would be the first to say so.
    #[test]
    fn the_derived_codes_are_the_ones_the_specification_prints() {
        let table = table();
        for (symbol, bits, length) in [
            (b'0' as usize, 0x0, 5),
            (b't' as usize, 0x9, 5),
            (b' ' as usize, 0x14, 6),
            (b'u' as usize, 0x2d, 6),
            (b':' as usize, 0x5c, 7),
            (b'z' as usize, 0x7b, 7),
            (b'&' as usize, 0xf8, 8),
            (b'!' as usize, 0x3f8, 10),
            (0, 0x1ff8, 13),
            (b'\\' as usize, 0x7_fff0, 19),
            (10, 0x3fff_fffc, 30),
            (256, 0x3fff_ffff, 30),
        ] {
            let code = table
                .get(symbol)
                .copied()
                .unwrap_or(Code { bits: 0, length: 0 });
            assert_eq!(
                (code.bits, code.length),
                (bits, length),
                "symbol {symbol} came out as {code:?}"
            );
        }
    }

    #[test]
    fn every_byte_survives_a_round_trip() {
        for byte in 0..=255u8 {
            let encoded = encode(&[byte]);
            assert_eq!(
                decode(&encoded, 16).unwrap_or_default(),
                vec![byte],
                "byte {byte} did not come back"
            );
        }
        let everything: Vec<u8> = (0..=255u8).collect();
        assert_eq!(
            decode(&encode(&everything), 1024).unwrap_or_default(),
            everything
        );
    }

    /// The specification's own examples.
    #[test]
    fn the_examples_from_the_specification_encode_as_printed() {
        for (text, expected) in [
            (
                "www.example.com",
                vec![
                    0xf1, 0xe3, 0xc2, 0xe5, 0xf2, 0x3a, 0x6b, 0xa0, 0xab, 0x90, 0xf4, 0xff,
                ],
            ),
            ("no-cache", vec![0xa8, 0xeb, 0x10, 0x64, 0x9c, 0xbf]),
            (
                "custom-key",
                vec![0x25, 0xa8, 0x49, 0xe9, 0x5b, 0xa9, 0x7d, 0x7f],
            ),
            (
                "custom-value",
                vec![0x25, 0xa8, 0x49, 0xe9, 0x5b, 0xb8, 0xe8, 0xb4, 0xbf],
            ),
        ] {
            assert_eq!(encode(text.as_bytes()), expected, "{text} encoded wrongly");
            assert_eq!(
                decode(&expected, 64).unwrap_or_default(),
                text.as_bytes(),
                "{text} decoded wrongly"
            );
        }
    }

    #[test]
    fn padding_that_is_not_ones_is_refused() {
        // "0" is 5 bits of zero; pad with zeroes instead of ones.
        assert!(decode(&[0x00], 16).is_err());
    }

    #[test]
    fn the_end_of_string_symbol_may_never_appear() {
        // Thirty ones, then padding.
        assert!(decode(&[0xff, 0xff, 0xff, 0xff], 16).is_err());
    }

    #[test]
    fn huffman_is_not_always_worth_it() {
        assert!(worth_it(b"www.example.com"));
        // Bytes outside ASCII cost twenty to thirty bits each.
        assert!(!worth_it(&[0xc3, 0xa9, 0xc3, 0xa8]));
    }
}
