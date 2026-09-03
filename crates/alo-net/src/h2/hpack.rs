/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! HPACK: the header compression HTTP/2 carries in its header blocks.
//!
//! # The one thing to understand before reading
//!
//! HPACK is **stateful across frames**. Both ends keep a table that every block
//! adds to and reads from, and block number nine can only be decoded if blocks
//! one to eight were decoded identically. That single fact decides three
//! things in this file:
//!
//! - **A decoding failure kills the connection, never one stream.** After a
//!   block nobody could decode, the table is in a condition nobody can reason
//!   about, and every later block would be decoded against something quietly
//!   wrong. Resetting the stream and carrying on is the tempting, wrong answer.
//! - **Every bound is checked while decoding, not after.** An integer with
//!   enough continuation bytes is an arithmetic overflow; a size update is a
//!   number a stranger chose for how much we allocate.
//! - **Nothing is decoded speculatively.** The table is only changed by a
//!   representation that has been fully and correctly read.

use super::ErrorCode;
use super::frame::Broken;
use super::huffman;

/// The most bytes one header name or value may be.
pub const LONGEST_ONE: usize = 8 * 1024;

/// The most headers one block may carry.
///
/// A block is a list a stranger chooses the length of, and each entry costs an
/// allocation. Without a bound, one frame is as many as fit in a frame.
pub const MOST_HEADERS: usize = 200;

/// The largest dynamic table this end will keep, whatever the peer asks for.
///
/// The peer names a size in `SETTINGS_HEADER_TABLE_SIZE` and may raise it later
/// with a size update. This is the ceiling on both, because a peer that could
/// name any size could name a large one.
pub const LARGEST_TABLE: usize = 64 * 1024;

/// What every entry costs beyond its bytes, by definition.
///
/// The specification's own figure, meant to stand for the two strings and the
/// bookkeeping around them. It is what stops a table of ten thousand empty
/// headers being free.
const OVERHEAD: usize = 32;

/// The table both ends already have, and never send.
const STATIC: &[(&str, &str)] = &[
    (":authority", ""),
    (":method", "GET"),
    (":method", "POST"),
    (":path", "/"),
    (":path", "/index.html"),
    (":scheme", "http"),
    (":scheme", "https"),
    (":status", "200"),
    (":status", "204"),
    (":status", "206"),
    (":status", "304"),
    (":status", "400"),
    (":status", "404"),
    (":status", "500"),
    ("accept-charset", ""),
    ("accept-encoding", "gzip, deflate"),
    ("accept-language", ""),
    ("accept-ranges", ""),
    ("accept", ""),
    ("access-control-allow-origin", ""),
    ("age", ""),
    ("allow", ""),
    ("authorization", ""),
    ("cache-control", ""),
    ("content-disposition", ""),
    ("content-encoding", ""),
    ("content-language", ""),
    ("content-length", ""),
    ("content-location", ""),
    ("content-range", ""),
    ("content-type", ""),
    ("cookie", ""),
    ("date", ""),
    ("etag", ""),
    ("expect", ""),
    ("expires", ""),
    ("from", ""),
    ("host", ""),
    ("if-match", ""),
    ("if-modified-since", ""),
    ("if-none-match", ""),
    ("if-range", ""),
    ("if-unmodified-since", ""),
    ("last-modified", ""),
    ("link", ""),
    ("location", ""),
    ("max-forwards", ""),
    ("proxy-authenticate", ""),
    ("proxy-authorization", ""),
    ("range", ""),
    ("referer", ""),
    ("refresh", ""),
    ("retry-after", ""),
    ("server", ""),
    ("set-cookie", ""),
    ("strict-transport-security", ""),
    ("transfer-encoding", ""),
    ("user-agent", ""),
    ("vary", ""),
    ("via", ""),
    ("www-authenticate", ""),
];

/// One header, as HPACK carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The name, always lowercase on the wire.
    pub name: String,
    /// The value.
    pub value: String,
    /// Whether the sender asked that this never be put in any table, anywhere.
    ///
    /// Carried rather than dropped because it has to be **honoured onward**: it
    /// is how a proxy is told that a value is a secret, and a relay that forgot
    /// it would compress somebody's authorization token into a shared table.
    /// Nothing relays yet; the flag is kept so that when something does, the
    /// information is there rather than remembered.
    pub never_indexed: bool,
}

impl Field {
    /// A plain header.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            never_indexed: false,
        }
    }

    /// What it costs in a table.
    fn cost(&self) -> usize {
        self.name.len() + self.value.len() + OVERHEAD
    }
}

/// The table one end keeps while decoding, or while encoding.
///
/// One per direction per connection. Two ends of one connection have four
/// between them, and mixing any two up produces headers that decode to the
/// wrong thing rather than to an error — which is why this is a type rather
/// than a field somewhere convenient.
#[derive(Debug)]
pub struct Table {
    /// Newest first, which is the order the indices are in.
    entries: Vec<Field>,
    /// The size the other end has agreed to.
    allowed: usize,
    /// The size in force, which the peer may lower and raise within `allowed`.
    limit: usize,
    used: usize,
}

impl Default for Table {
    fn default() -> Self {
        Self::new(4096)
    }
}

impl Table {
    /// A table of this size, capped at [`LARGEST_TABLE`].
    pub fn new(allowed: usize) -> Self {
        let allowed = allowed.min(LARGEST_TABLE);
        Self {
            entries: Vec::new(),
            allowed,
            limit: allowed,
            used: 0,
        }
    }

    /// How many entries it holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it holds nothing.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many bytes it is counted as using.
    pub fn used(&self) -> usize {
        self.used
    }

    /// The size in force.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// What is at this index, static and dynamic together.
    ///
    /// Index zero is not an index — it is the value that means "a name that
    /// follows", and reading it as one is off-by-one into the static table.
    fn at(&self, index: usize) -> Option<Field> {
        if index == 0 {
            return None;
        }
        if let Some((name, value)) = STATIC.get(index - 1) {
            return Some(Field::new(*name, *value));
        }
        self.entries.get(index - 1 - STATIC.len()).cloned()
    }

    /// Add, evicting whatever no longer fits.
    fn add(&mut self, field: Field) {
        let cost = field.cost();
        while self.used + cost > self.limit {
            match self.entries.pop() {
                Some(gone) => self.used = self.used.saturating_sub(gone.cost()),
                None => break,
            }
        }
        // An entry larger than the whole table empties it and is not added.
        // That is the specification's rule, and it is not an error: it is how a
        // sender says "forget everything" without a separate way to say it.
        if cost > self.limit {
            self.entries.clear();
            self.used = 0;
            return;
        }
        self.used += cost;
        self.entries.insert(0, field);
    }

    /// Change the size in force.
    ///
    /// # Errors
    ///
    /// [`Broken`] when the peer asks for more than it agreed to. That is not a
    /// negotiation, it is a peer choosing how much memory this end spends.
    fn resize(&mut self, to: usize) -> Result<(), Broken> {
        if to > self.allowed {
            return Err(compression(&format!(
                "a header table of {to} bytes, when {} was agreed",
                self.allowed
            )));
        }
        self.limit = to;
        while self.used > self.limit {
            match self.entries.pop() {
                Some(gone) => self.used = self.used.saturating_sub(gone.cost()),
                None => break,
            }
        }
        Ok(())
    }

    /// Where this name and value already are, if anywhere.
    fn find(&self, name: &str, value: &str) -> (Option<usize>, Option<usize>) {
        let mut by_name = None;
        for (at, (known, held)) in STATIC.iter().enumerate() {
            if *known == name {
                if *held == value {
                    return (Some(at + 1), by_name);
                }
                by_name.get_or_insert(at + 1);
            }
        }
        for (at, field) in self.entries.iter().enumerate() {
            if field.name == name {
                let index = at + 1 + STATIC.len();
                if field.value == value {
                    return (Some(index), by_name);
                }
                by_name.get_or_insert(index);
            }
        }
        (None, by_name)
    }
}

/// Decode a header block.
///
/// # Errors
///
/// [`Broken`] with [`ErrorCode::CompressionError`], always fatal to the
/// connection — see this module's own note about why one stream is not enough.
pub fn decode(block: &[u8], table: &mut Table) -> Result<Vec<Field>, Broken> {
    let mut fields = Vec::new();
    let mut at = 0usize;
    // A size update may only appear before any header in the block. After one
    // has been read, an update is a sender doing something it must not.
    let mut still_at_the_start = true;

    while at < block.len() {
        let first = block.get(at).copied().unwrap_or(0);
        if first & 0x80 != 0 {
            let (index, next) = integer(block, at, 7)?;
            at = next;
            let field = table
                .at(usize::try_from(index).unwrap_or(usize::MAX))
                .ok_or_else(|| compression(&format!("index {index}, which is not in the table")))?;
            push(&mut fields, field)?;
            still_at_the_start = false;
        } else if first & 0x40 != 0 {
            let (field, next) = literal(block, at, 6, table)?;
            at = next;
            table.add(field.clone());
            push(&mut fields, field)?;
            still_at_the_start = false;
        } else if first & 0x20 != 0 {
            if !still_at_the_start {
                return Err(compression(
                    "a header table size update in the middle of a block, where it may not appear",
                ));
            }
            let (size, next) = integer(block, at, 5)?;
            at = next;
            table.resize(usize::try_from(size).unwrap_or(usize::MAX))?;
        } else {
            // `0001xxxx` is never-indexed, `0000xxxx` is without-indexing.
            // Both leave the table alone; only the flag differs.
            let never = first & 0x10 != 0;
            let (mut field, next) = literal(block, at, 4, table)?;
            at = next;
            field.never_indexed = never;
            push(&mut fields, field)?;
            still_at_the_start = false;
        }
    }
    Ok(fields)
}

fn push(fields: &mut Vec<Field>, field: Field) -> Result<(), Broken> {
    if fields.len() >= MOST_HEADERS {
        return Err(compression(&format!(
            "a header block of more than {MOST_HEADERS} headers"
        )));
    }
    fields.push(field);
    Ok(())
}

/// A literal, with its name either given by index or spelled out.
fn literal(block: &[u8], at: usize, prefix: u32, table: &Table) -> Result<(Field, usize), Broken> {
    let (index, mut next) = integer(block, at, prefix)?;
    let name = if index == 0 {
        let (name, after) = string(block, next)?;
        next = after;
        name
    } else {
        table
            .at(usize::try_from(index).unwrap_or(usize::MAX))
            .ok_or_else(|| compression(&format!("name index {index}, which is not in the table")))?
            .name
    };
    let (value, after) = string(block, next)?;
    Ok((
        Field {
            name,
            value,
            never_indexed: false,
        },
        after,
    ))
}

/// An integer with an `n`-bit prefix.
///
/// The bound is the point. Continuation bytes carry seven bits each and nothing
/// in the encoding says how many there are, so an integer is a place a peer can
/// send a hundred bytes that mean "overflow".
fn integer(block: &[u8], at: usize, prefix: u32) -> Result<(u64, usize), Broken> {
    let first = block
        .get(at)
        .copied()
        .ok_or_else(|| compression("a block that stops where an integer should be"))?;
    let mask = (1u64 << prefix) - 1;
    let mut value = u64::from(first) & mask;
    if value < mask {
        return Ok((value, at + 1));
    }
    let mut shift = 0u32;
    let mut next = at + 1;
    loop {
        let byte = block
            .get(next)
            .copied()
            .ok_or_else(|| compression("an integer that never ends"))?;
        next += 1;
        // Five groups of seven bits is thirty-five, which is already past
        // anything this protocol has a use for. Refusing here is what stops the
        // shift below from being an overflow.
        if shift > 28 {
            return Err(compression("an integer larger than this engine will read"));
        }
        value = value
            .checked_add(u64::from(byte & 0x7f) << shift)
            .ok_or_else(|| compression("an integer larger than this engine will read"))?;
        if byte & 0x80 == 0 {
            return Ok((value, next));
        }
        shift += 7;
    }
}

/// A string, Huffman-coded or not.
fn string(block: &[u8], at: usize) -> Result<(String, usize), Broken> {
    let first = block
        .get(at)
        .copied()
        .ok_or_else(|| compression("a block that stops where a string should be"))?;
    let compressed = first & 0x80 != 0;
    let (length, next) = integer(block, at, 7)?;
    let length = usize::try_from(length).unwrap_or(usize::MAX);
    if length > LONGEST_ONE {
        return Err(compression(&format!(
            "a header of {length} bytes, when {LONGEST_ONE} is the most this engine holds"
        )));
    }
    let bytes = block
        .get(next..next + length)
        .ok_or_else(|| compression("a string longer than the block it is in"))?;
    let bytes = if compressed {
        huffman::decode(bytes, LONGEST_ONE)?
    } else {
        bytes.to_vec()
    };
    // Headers are bytes on the wire and text everywhere above. A name or value
    // that is not UTF-8 is refused rather than replaced: a lossy conversion
    // here would let two different headers arrive as one.
    let text = String::from_utf8(bytes)
        .map_err(|_| compression("a header that is not text this engine can read"))?;
    Ok((text, next + length))
}

/// Encode a header block.
///
/// Indexes what it can and adds what it indexes, which is what makes the second
/// request on a connection small.
pub fn encode(fields: &[Field], table: &mut Table) -> Vec<u8> {
    let mut out = Vec::new();
    for field in fields {
        let (exact, by_name) = table.find(&field.name, &field.value);
        if let Some(index) = exact.filter(|_| !field.never_indexed) {
            write_integer(&mut out, index as u64, 7, 0x80);
            continue;
        }
        if field.never_indexed {
            // Never indexed means never *put in a table* — not by us and not by
            // anything downstream. So the name may still be referenced, but the
            // pair is not added.
            write_integer(&mut out, by_name.unwrap_or(0) as u64, 4, 0x10);
            if by_name.is_none() {
                write_string(&mut out, &field.name);
            }
            write_string(&mut out, &field.value);
            continue;
        }
        write_integer(&mut out, by_name.unwrap_or(0) as u64, 6, 0x40);
        if by_name.is_none() {
            write_string(&mut out, &field.name);
        }
        write_string(&mut out, &field.value);
        table.add(field.clone());
    }
    out
}

fn write_integer(out: &mut Vec<u8>, value: u64, prefix: u32, flags: u8) {
    let mask = (1u64 << prefix) - 1;
    if value < mask {
        out.push(flags | u8::try_from(value).unwrap_or(0));
        return;
    }
    out.push(flags | u8::try_from(mask).unwrap_or(0));
    let mut rest = value - mask;
    while rest >= 0x80 {
        out.push(u8::try_from((rest & 0x7f) | 0x80).unwrap_or(0));
        rest >>= 7;
    }
    out.push(u8::try_from(rest).unwrap_or(0));
}

fn write_string(out: &mut Vec<u8>, text: &str) {
    let bytes = text.as_bytes();
    if huffman::worth_it(bytes) {
        let coded = huffman::encode(bytes);
        write_integer(out, coded.len() as u64, 7, 0x80);
        out.extend_from_slice(&coded);
    } else {
        write_integer(out, bytes.len() as u64, 7, 0);
        out.extend_from_slice(bytes);
    }
}

fn compression(why: &str) -> Broken {
    Broken {
        why: why.to_owned(),
        error: ErrorCode::CompressionError,
        fatal: true,
    }
}
