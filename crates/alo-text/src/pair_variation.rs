/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the rented shaper leaves out of a kerning pair's variation (queue
//! item 796, ADR 0097).
//!
//! A variable font varies a `GPOS` pair's adjustment along its axes with a
//! *variation index* table beside the value: Newsreader kerns `iv` −14
//! units at `opsz` 18 and adds a delta of up to +14 towards 72, so the pair
//! is not kerned at all there. `rustybuzz` 0.20.1 applies that delta only
//! as far as `ttf-parser` 0.25.1 hands it the table, and:
//!
//! - in a pair listed glyph by glyph (`PairPos` format 1), `ttf-parser`
//!   reads the value's table offsets from the pair records rather than from
//!   the start of their `PairSet`, as the specification and HarfBuzz have
//!   them — so the table is almost never found and the delta never applied
//!   (Newsreader: 21,795 of its 22,297 listed pairs vary; Instrument Sans:
//!   1,656 of 1,661);
//! - in a pair kerned by classes (format 2), where the offset is read
//!   right, the delta is rounded to a whole unit, as every position the
//!   shaper gives is; Chrome 156 keeps the fraction.
//!
//! So after shaping, [`corrections`] walks the glyphs as HarfBuzz's pair
//! positioning does — each lookup of the `kern` and `dist` features of the
//! run's script, in order, a glyph and the next one the lookup does not
//! skip, the first subtable holding the pair — and gives, for each glyph,
//! the exact delta less what the shaper applied of it: the shaper's own
//! reading of the value repeated here, fault and rounding included, so
//! that a pair it did apply is never counted twice.
//!
//! Only a varied face's left-to-right run is corrected; a right-to-left
//! run is left as the shaper set it (stage 2's bidirectional work). A font
//! whose `GPOS` swaps lookups along an axis (`FeatureVariations`) is left
//! alone too: which lookups ran there is the shaper's to say.

use crate::font::{GlyphKind, Varied};

/// What one glyph is moved by, beyond what the shaper moved it, in font
/// units: its advance and its offset across and up.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Moved {
    /// Added to the glyph's advance.
    pub advance: f32,
    /// Added to its offset across.
    pub across: f32,
    /// Added to its offset up.
    pub up: f32,
}

/// For each of `glyphs` — a run shaped left to right in `script` (its
/// OpenType tag), in the shaper's order — what its varied kerning adds
/// beyond what the shaper applied, in `face` at its instance. [`None`] when
/// there is nothing to add: no `GPOS`, or no pair in it that varies here.
pub(crate) fn corrections(
    face: &Varied<'_>,
    glyphs: &[u16],
    script: [u8; 4],
) -> Option<Vec<Moved>> {
    if glyphs.len() < 2 || !face.has_glyph_classes() {
        return None;
    }
    let gpos = face.gpos()?;
    let lookups = pair_lookups(gpos, script)?;
    let deltas = Deltas { face };
    let mut moved = vec![Moved::default(); glyphs.len()];
    for lookup in lookups {
        walk(gpos, lookup, glyphs, &deltas, &mut moved);
    }
    moved
        .iter()
        .any(|each| *each != Moved::default())
        .then_some(moved)
}

/// How a device table's delta is read: by the face's variation store, at
/// the face's coordinates.
struct Deltas<'v, 'a> {
    face: &'v Varied<'a>,
}

impl Deltas<'_, '_> {
    /// The delta of the device table at `at` in `data`: only a variation
    /// index table has one here — a hinting table is for a size in pixels,
    /// which neither the shaper nor Chrome is given.
    fn of(&self, data: &[u8], at: Option<usize>) -> f32 {
        let Some(at) = at else {
            return 0.0;
        };
        match (u16_at(data, at), u16_at(data, at + 2), u16_at(data, at + 4)) {
            (Some(outer), Some(inner), Some(0x8000)) => {
                self.face.delta(outer, inner).unwrap_or(0.0)
            }
            _ => 0.0,
        }
    }

    /// What a value's varied tables move its glyph by beyond what the
    /// shaper moved it: `exact` read where they are, less `shaped` — read
    /// where the shaper's reader looked, and rounded to a whole unit as the
    /// shaper does.
    fn beyond(&self, exact: Read<'_>, shaped: Read<'_>, format: u16) -> Moved {
        let exact = exact.devices(format);
        let shaped = shaped.devices(format);
        let more = |exact_at: Option<usize>,
                    exact_in: &[u8],
                    shaped_at: Option<usize>,
                    shaped_in: &[u8]| {
            self.of(exact_in, exact_at) - self.of(shaped_in, shaped_at).round()
        };
        Moved {
            advance: more(exact.1.advance, exact.0, shaped.1.advance, shaped.0),
            across: more(exact.1.across, exact.0, shaped.1.across, shaped.0),
            up: more(exact.1.up, exact.0, shaped.1.up, shaped.0),
        }
    }
}

/// Where one value record's device tables are read from: the bytes, where
/// in them the record is, and where in them its offsets count from.
#[derive(Clone, Copy)]
struct Read<'a> {
    data: &'a [u8],
    record: usize,
    base: usize,
}

/// A value record's device tables, by where each sits in the bytes read.
#[derive(Debug, Clone, Copy, Default)]
struct Devices {
    across: Option<usize>,
    up: Option<usize>,
    advance: Option<usize>,
}

impl<'a> Read<'a> {
    /// The record's device tables, formatted by `format`: an offset of
    /// nothing, or one that lands outside the bytes read, is no table.
    fn devices(self, format: u16) -> (&'a [u8], Devices) {
        // The four values before the tables, two bytes each where present.
        let mut cursor = self.record + 2 * (format & 0x0F).count_ones() as usize;
        let mut next = |bit: u16| -> Option<usize> {
            if format & bit == 0 {
                return None;
            }
            let offset = u16_at(self.data, cursor);
            cursor += 2;
            let offset = usize::from(offset?);
            (offset != 0 && self.base + offset < self.data.len()).then_some(self.base + offset)
        };
        let across = next(0x10);
        let up = next(0x20);
        let advance = next(0x40);
        (
            self.data,
            Devices {
                across,
                up,
                advance,
            },
        )
    }
}

/// How many bytes a value record of `format` is.
fn value_size(format: u16) -> usize {
    2 * (format & 0xFF).count_ones() as usize
}

/// The indices of the lookups the `kern` and `dist` features of `script`'s
/// default language hold, in order — or [`None`] where the table swaps
/// lookups along an axis, or cannot be read.
fn pair_lookups(gpos: &[u8], script: [u8; 4]) -> Option<Vec<u16>> {
    let minor = u16_at(gpos, 2)?;
    if minor >= 1 && u32_at(gpos, 10)? != 0 {
        return None;
    }
    let scripts = usize::from(u16_at(gpos, 4)?);
    let features = usize::from(u16_at(gpos, 6)?);
    let wanted = [script, *b"DFLT", *b"dflt", *b"latn"];
    let table = scripts
        + wanted
            .into_iter()
            .find_map(|tag| record_of(gpos, scripts, tag))?;
    let default = usize::from(u16_at(gpos, table)?);
    if default == 0 {
        return Some(Vec::new());
    }
    let language = table + default;
    let count = usize::from(u16_at(gpos, language + 4)?);
    let mut lookups = Vec::new();
    for index in 0..count {
        let feature = usize::from(u16_at(gpos, language + 6 + 2 * index)?);
        let record = features + 2 + 6 * feature;
        let tag = gpos.get(record..record + 4)?;
        if tag != b"kern" && tag != b"dist" {
            continue;
        }
        let at = features + usize::from(u16_at(gpos, record + 4)?);
        let held = usize::from(u16_at(gpos, at + 2)?);
        for lookup in 0..held {
            lookups.push(u16_at(gpos, at + 4 + 2 * lookup)?);
        }
    }
    lookups.sort_unstable();
    lookups.dedup();
    Some(lookups)
}

/// Where the script `wanted` is, from the script list at `list`.
fn record_of(gpos: &[u8], list: usize, wanted: [u8; 4]) -> Option<usize> {
    let count = usize::from(u16_at(gpos, list)?);
    (0..count).find_map(|index| {
        let record = list + 2 + 6 * index;
        let tag = gpos.get(record..record + 4)?;
        if tag == wanted {
            u16_at(gpos, record + 4).map(usize::from)
        } else {
            None
        }
    })
}

/// One lookup that positions pairs: its flags, its mark filtering set if it
/// names one, and where each of its subtables starts in `gpos`.
struct Lookup {
    flag: u16,
    filter: Option<u16>,
    subtables: Vec<usize>,
}

/// The lookup `index`, if it positions pairs.
fn pair_lookup(gpos: &[u8], index: u16) -> Option<Lookup> {
    let list = usize::from(u16_at(gpos, 8)?);
    if index >= u16_at(gpos, list)? {
        return None;
    }
    let at = list + usize::from(u16_at(gpos, list + 2 + 2 * usize::from(index))?);
    let kind = u16_at(gpos, at)?;
    let flag = u16_at(gpos, at + 2)?;
    let count = usize::from(u16_at(gpos, at + 4)?);
    let filter = if flag & 0x10 == 0 {
        None
    } else {
        Some(u16_at(gpos, at + 6 + 2 * count)?)
    };
    let mut subtables = Vec::with_capacity(count);
    for slot in 0..count {
        let subtable = at + usize::from(u16_at(gpos, at + 6 + 2 * slot)?);
        // An extension subtable holds its own type and a 32-bit offset.
        let (kind, subtable) = if kind == 9 {
            let further = usize::try_from(u32_at(gpos, subtable + 4)?).ok()?;
            (u16_at(gpos, subtable + 2)?, subtable + further)
        } else {
            (kind, subtable)
        };
        if kind == 2 {
            subtables.push(subtable);
        }
    }
    (!subtables.is_empty()).then_some(Lookup {
        flag,
        filter,
        subtables,
    })
}

/// Whether `lookup` passes over `glyph`, as HarfBuzz's glyph property check
/// does: by its class, and a mark by its filtering set or attachment class.
fn skips(face: &Varied<'_>, lookup: &Lookup, glyph: u16) -> bool {
    let flag = lookup.flag;
    match face.kind(glyph) {
        Some(GlyphKind::Base) => flag & 0x2 != 0,
        Some(GlyphKind::Ligature) => flag & 0x4 != 0,
        Some(GlyphKind::Mark) => {
            if flag & 0x8 != 0 {
                return true;
            }
            if let Some(set) = lookup.filter {
                return !face.in_mark_set(glyph, set);
            }
            let wanted = flag >> 8;
            wanted != 0 && face.attachment_class(glyph) != wanted
        }
        _ => false,
    }
}

/// One lookup run over the glyphs as the shaper runs it, adding to `moved`
/// what each pair it applies varies by beyond what the shaper applied.
fn walk(gpos: &[u8], index: u16, glyphs: &[u16], deltas: &Deltas<'_, '_>, moved: &mut [Moved]) {
    let Some(lookup) = pair_lookup(gpos, index) else {
        return;
    };
    let skipped = |at: usize| {
        glyphs
            .get(at)
            .is_none_or(|glyph| skips(deltas.face, &lookup, *glyph))
    };
    let mut at = 0;
    while at < glyphs.len() {
        if skipped(at) {
            at += 1;
            continue;
        }
        let Some(next) = (at + 1..glyphs.len()).find(|&later| !skipped(later)) else {
            break;
        };
        let (Some(first), Some(second)) = (glyphs.get(at), glyphs.get(next)) else {
            break;
        };
        let found = lookup
            .subtables
            .iter()
            .find_map(|&subtable| pair(gpos, subtable, (*first, *second), deltas));
        let Some(found) = found else {
            at += 1;
            continue;
        };
        for (index, values) in [(at, found.first), (next, found.second)] {
            if let Some(glyph) = moved.get_mut(index) {
                glyph.advance += values.advance;
                glyph.across += values.across;
                glyph.up += values.up;
            }
        }
        at = if found.second_has_a_value {
            next + 1
        } else {
            next
        };
    }
}

/// A pair found: what each glyph's value varies by beyond what the shaper
/// applied, and whether the second glyph had a value of its own — and so
/// is not the first of the next pair.
struct Found {
    first: Moved,
    second: Moved,
    second_has_a_value: bool,
}

/// The pair `glyphs` in the pair subtable at `at`, if it holds it.
fn pair<'a>(
    gpos: &'a [u8],
    at: usize,
    glyphs: (u16, u16),
    deltas: &Deltas<'_, '_>,
) -> Option<Found> {
    let (first, second) = glyphs;
    let format = u16_at(gpos, at)?;
    let covered = coverage_index(gpos, at + usize::from(u16_at(gpos, at + 2)?), first)?;
    let (one, two) = (u16_at(gpos, at + 4)?, u16_at(gpos, at + 6)?);
    let (exact, shaped) = match format {
        1 => {
            if covered >= usize::from(u16_at(gpos, at + 8)?) {
                return None;
            }
            let set = at + usize::from(u16_at(gpos, at + 10 + 2 * covered)?);
            let count = usize::from(u16_at(gpos, set)?);
            let size = 2 + value_size(one) + value_size(two);
            // The records are in the order of their second glyphs.
            let found = search(
                count,
                |index| u16_at(gpos, set + 2 + index * size).map(|listed| (listed, listed)),
                second,
            )?;
            let record = set + 2 + found * size;
            let exact = Read {
                data: gpos,
                record: record + 2,
                base: set,
            };
            // `ttf-parser` 0.25.1 counts the offsets from the records,
            // which start two bytes into the set, and reads only them.
            let records = gpos.get(set + 2..set + 2 + count * size)?;
            let shaped = Read {
                data: records,
                record: record + 2 - (set + 2),
                base: 0,
            };
            (exact, shaped)
        }
        2 => {
            let firsts = at + usize::from(u16_at(gpos, at + 8)?);
            let seconds = at + usize::from(u16_at(gpos, at + 10)?);
            let (rows, columns) = (u16_at(gpos, at + 12)?, u16_at(gpos, at + 14)?);
            let (row, column) = (
                class_of(gpos, firsts, first),
                class_of(gpos, seconds, second),
            );
            if row >= rows || column >= columns {
                return None;
            }
            let size = value_size(one) + value_size(two);
            let cell = usize::from(row) * usize::from(columns) + usize::from(column);
            let read = Read {
                data: gpos,
                record: at + 16 + cell * size,
                base: at,
            };
            (read, read)
        }
        _ => return None,
    };
    let after = |read: Read<'a>| Read {
        record: read.record + value_size(one),
        ..read
    };
    Some(Found {
        first: deltas.beyond(exact, shaped, one),
        second: deltas.beyond(after(exact), after(shaped), two),
        second_has_a_value: two != 0,
    })
}

/// Where `glyph` is in the coverage table at `at`, if it is there: its
/// glyphs, or its ranges, are in order, so it is searched by halves.
fn coverage_index(data: &[u8], at: usize, glyph: u16) -> Option<usize> {
    let count = usize::from(u16_at(data, at + 2)?);
    match u16_at(data, at)? {
        1 => search(
            count,
            |index| u16_at(data, at + 4 + 2 * index).map(|listed| (listed, listed)),
            glyph,
        ),
        2 => {
            let index = search(
                count,
                |index| {
                    let range = at + 4 + 6 * index;
                    Some((u16_at(data, range)?, u16_at(data, range + 2)?))
                },
                glyph,
            )?;
            let range = at + 4 + 6 * index;
            let (start, first) = (u16_at(data, range)?, u16_at(data, range + 4)?);
            Some(usize::from(first) + usize::from(glyph.checked_sub(start)?))
        }
        _ => None,
    }
}

/// Which of `count` entries in order — each `entry(index)` the first and
/// last glyph it covers — covers `glyph`, searched by halves.
fn search(count: usize, entry: impl Fn(usize) -> Option<(u16, u16)>, glyph: u16) -> Option<usize> {
    let (mut low, mut high) = (0, count);
    while low < high {
        let middle = low + (high - low) / 2;
        let (first, last) = entry(middle)?;
        if glyph < first {
            high = middle;
        } else if glyph > last {
            low = middle + 1;
        } else {
            return Some(middle);
        }
    }
    None
}

/// The class `glyph` is in by the class definition at `at`: 0 where it
/// names none.
fn class_of(data: &[u8], at: usize, glyph: u16) -> u16 {
    let found = match u16_at(data, at) {
        Some(1) => u16_at(data, at + 2).and_then(|start| {
            let count = u16_at(data, at + 4)?;
            let index = glyph.checked_sub(start)?;
            if index >= count {
                return None;
            }
            u16_at(data, at + 6 + 2 * usize::from(index))
        }),
        Some(2) => u16_at(data, at + 2).and_then(|count| {
            let index = search(
                usize::from(count),
                |index| {
                    let range = at + 4 + 6 * index;
                    Some((u16_at(data, range)?, u16_at(data, range + 2)?))
                },
                glyph,
            )?;
            u16_at(data, at + 4 + 6 * index + 4)
        }),
        _ => None,
    };
    found.unwrap_or(0)
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    let bytes = data.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes(bytes.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes(bytes.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{Font, Slant, Weight};

    fn newsreader() -> Option<Font> {
        let data = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../alo-renderer/interface/fonts/Newsreader.ttf"
        ))
        .ok()?;
        Font::load("Newsreader", Weight::NORMAL, Slant::Normal, data)
    }

    /// `iv`'s corrections in Newsreader at `size`, `opsz` following it, and
    /// the whole units the shaper is given for `i` there.
    fn iv_at(font: &Font, size: f32) -> Option<(Vec<Moved>, u16)> {
        let face = font.varied(size)?;
        let glyphs = [face.glyph('i')?, face.glyph('v')?];
        let (_, _, i) = face.advance(glyphs[0])?;
        let moved = corrections(&face, &glyphs, *b"latn")
            .unwrap_or_else(|| vec![Moved::default(); glyphs.len()]);
        Some((moved, i))
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 1e-3
    }

    #[test]
    fn a_listed_pair_is_varied_along_the_axis_as_the_shaper_did_not() {
        let Some(font) = newsreader() else {
            panic!("Newsreader is not a font");
        };
        // At 72 the pair's −14 is varied away by its +14: the shaper kerns
        // it −14 (`i` 553 unkerned), and +14 is given back.
        let Some((moved, i)) = iv_at(&font, 72.0) else {
            panic!("Newsreader at 72 could not be read");
        };
        assert_eq!(i, 553);
        assert!(
            close(moved.first().map_or(0.0, |m| m.advance), 14.0),
            "{moved:?}"
        );
        assert!(close(moved.get(1).map_or(0.0, |m| m.advance), 0.0));
        // At 40, 6,674 of the axis's 16,384 towards 72: 5.70 given back, its
        // fraction kept.
        let Some((moved, _)) = iv_at(&font, 40.0) else {
            panic!("Newsreader at 40 could not be read");
        };
        assert!(
            close(moved.first().map_or(0.0, |m| m.advance), 5.702_881),
            "{moved:?}"
        );
    }

    #[test]
    fn at_its_default_a_face_is_left_as_the_shaper_set_it() {
        // At 18 px, its default `opsz` and weight, nothing is varied.
        let Some(font) = newsreader() else {
            panic!("Newsreader is not a font");
        };
        assert!(font.varied(18.0).is_none());
    }

    #[test]
    fn coverage_and_classes_are_searched_by_halves() {
        // A coverage of glyphs 3, 7 and 9; then one of ranges 2–4 from
        // index 0 and 10–12 from index 3.
        let listed = [0, 1, 0, 3, 0, 3, 0, 7, 0, 9];
        assert_eq!(coverage_index(&listed, 0, 7), Some(1));
        assert_eq!(coverage_index(&listed, 0, 8), None);
        let ranges = [0, 2, 0, 2, 0, 2, 0, 4, 0, 0, 0, 10, 0, 12, 0, 3];
        assert_eq!(coverage_index(&ranges, 0, 11), Some(4));
        assert_eq!(coverage_index(&ranges, 0, 5), None);
        // Classes by range: 2–4 class 5, 10–12 class 6; anything else 0.
        let classes = [0, 2, 0, 2, 0, 2, 0, 4, 0, 5, 0, 10, 0, 12, 0, 6];
        assert_eq!(class_of(&classes, 0, 12), 6);
        assert_eq!(class_of(&classes, 0, 9), 0);
    }
}
