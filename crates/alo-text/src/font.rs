/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A font, and the numbers that come out of one.
//!
//! `docs/features.md`: shaping and rasterisation are **rented**, as every
//! engine rents them. What is ours is which font is chosen, when one is given
//! up on, and how a line is put together — so a font here is a small thing:
//! some bytes, a name, and the handful of measurements the rest of the engine
//! asks for.
//!
//! Every measurement is in CSS pixels at a given size, rather than in the
//! font's own units. A caller that had to divide by `units_per_em` would be a
//! caller that could forget to.

use core::fmt;
use std::sync::Arc;

/// How heavy a face is, on CSS's nine-point scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Weight(u16);

impl Weight {
    /// `normal`.
    pub const NORMAL: Weight = Weight(400);
    /// `bold`.
    pub const BOLD: Weight = Weight(700);

    /// A weight, clamped to the range CSS allows.
    pub fn new(value: u16) -> Self {
        Self(value.clamp(1, 1000))
    }

    /// The number.
    pub fn value(self) -> u16 {
        self.0
    }
}

impl Default for Weight {
    fn default() -> Self {
        Weight::NORMAL
    }
}

impl fmt::Display for Weight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Whether a face is upright or slanted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Slant {
    /// Upright.
    #[default]
    Normal,
    /// Slanted.
    Italic,
}

/// The four bytes OpenType files a weight axis under.
///
/// Ours as a **value** rather than as a rented type, because two crates need
/// it: this one shapes with it and `alo-paint` outlines with it, and each
/// parses a face with the parser it is allowed to name (ADR 0001). A tag
/// written out in two places is two chances to write it differently.
pub const WEIGHT_AXIS: [u8; 4] = *b"wght";

/// The weights a variable font can be set to.
///
/// A variable font is one file holding a continuum rather than a face, and
/// `OS/2` still states a single weight in it: the instance the outlines are
/// when nobody has said otherwise. Reading only that number files the whole
/// family under it. macOS's `SFCompact.ttf` states 1000, so this engine had it
/// down as the heaviest thing CSS can name and every page asking for anything
/// lighter got black; `SFNSMono.ttf` states 295, so nothing could ask for its
/// bold. Neither number is *wrong* about the default instance and both are
/// wrong about the font.
///
/// The numbers here are CSS's own. The OpenType specification defines `wght` in
/// the same 1..=1000 scale `font-weight` uses, which is why there is no
/// conversion in this file and why there must not be one: a translation between
/// two scales that are already the same is somewhere for an off-by-one to live.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeightAxis {
    lightest: Weight,
    heaviest: Weight,
}

impl WeightAxis {
    /// The lightest weight this font can be set to.
    pub fn lightest(self) -> Weight {
        self.lightest
    }

    /// The heaviest.
    pub fn heaviest(self) -> Weight {
        self.heaviest
    }

    /// Whether this axis reaches a weight.
    pub fn covers(self, weight: Weight) -> bool {
        weight >= self.lightest && weight <= self.heaviest
    }

    /// The weight on this axis nearest the one asked for: the weight itself
    /// wherever the axis reaches it, and the end it is nearest otherwise.
    pub fn nearest(self, weight: Weight) -> Weight {
        weight.clamp(self.lightest, self.heaviest)
    }
}

impl fmt::Display for WeightAxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.lightest, self.heaviest)
    }
}

/// The weight and the slant a font states about **itself**.
///
/// The pair rather than either alone, because they are written side by side in
/// one table and read in one look: a caller asking twice would parse the same
/// file twice to learn two halves of one sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    /// How heavy, on CSS's scale.
    ///
    /// One number, and for a variable font it is the **default instance**
    /// rather than the whole answer — see `axis`.
    pub weight: Weight,
    /// Upright or slanted.
    pub slant: Slant,
    /// The weights this font can be set to, when it is a variable one.
    ///
    /// [`None`] for the ordinary face that is one weight, which is most fonts.
    /// It joins the pair above rather than being a question of its own for that
    /// pair's reason: `fvar` is read out of the same face `OS/2` is, and a
    /// caller asking separately would parse the file twice to learn two halves
    /// of one sentence about how heavy this font is.
    pub axis: Option<WeightAxis>,
}

/// What a caller is asking for when it asks for a font.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FontRequest {
    /// The families to try, in order, as `font-family` lists them.
    pub families: Vec<String>,
    /// How heavy.
    pub weight: Weight,
    /// Upright or slanted.
    pub slant: Slant,
}

impl FontRequest {
    /// A request for one family at the ordinary weight.
    pub fn family(name: &str) -> Self {
        Self {
            families: vec![name.to_owned()],
            ..Self::default()
        }
    }

    /// The families a `font-family` value lists, in order, with quotes removed.
    ///
    /// The generic families — `sans-serif`, `serif`, `monospace` — are kept as
    /// written rather than resolved here: which font is a sans-serif is the
    /// font database's business, and it is the one thing that differs between
    /// one machine and another.
    pub fn parse_families(value: &str) -> Vec<String> {
        value
            .split(',')
            .map(|part| {
                part.trim()
                    .trim_matches(|c| c == '"' || c == '\'')
                    .trim()
                    .to_owned()
            })
            .filter(|part| !part.is_empty())
            .collect()
    }
}

/// The measurements of a face, in CSS pixels at one size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceMetrics {
    /// How far above the baseline the face reaches.
    pub ascender: f32,
    /// How far below it, as a positive number.
    pub descender: f32,
    /// The extra room the face asks for between lines.
    pub line_gap: f32,
    /// The height of a lowercase `x`, for the `ex` unit.
    pub x_height: f32,
    /// The width of a `0`, for the `ch` unit.
    pub zero_width: f32,
    /// How far **below** the baseline an underline sits, as a positive number.
    ///
    /// The face's own figure, because where a line goes depends on how far the
    /// letters descend — a font with long descenders puts its underline lower
    /// so the line does not cut through them.
    pub underline_offset: f32,
    /// How thick that line is.
    pub underline_thickness: f32,
}

impl FaceMetrics {
    /// The line height this face suggests: everything it reaches plus the room
    /// it asks for between lines.
    pub fn suggested_line_height(self) -> f32 {
        self.ascender + self.descender + self.line_gap
    }
}

/// A loaded font.
///
/// The bytes are held behind an [`Arc`] because a face borrows from them and
/// the same face is used from several places; cloning a `Font` is cheap and
/// does not copy a megabyte of glyphs.
#[derive(Clone)]
pub struct Font {
    family: Arc<str>,
    weight: Weight,
    slant: Slant,
    data: Arc<Vec<u8>>,
    index: u32,
    axis: Option<WeightAxis>,
}

impl Font {
    /// Load a font from its bytes.
    ///
    /// Returns [`None`] if the bytes are not a font this engine can read —
    /// which is a real answer, not an error to be swallowed: a font that will
    /// not parse should be skipped and the next one tried.
    ///
    /// The weight axis is read here rather than taken from the caller, even
    /// though the caller supplies the weight and the slant. Those two are what
    /// a face is **filed under**, and a database may be told them; the axis
    /// decides what the bytes are *set to* when they are shaped, and a caller
    /// able to supply that could supply one the file does not have.
    pub fn load(family: &str, weight: Weight, slant: Slant, data: Vec<u8>) -> Option<Self> {
        let data = Arc::new(data);
        let index = 0;
        // Parsing once here means every later use can assume it parses.
        let axis = weight_axis_of(&ttf_parser::Face::parse(&data, index).ok()?);
        Some(Self {
            family: family.into(),
            weight,
            slant,
            data,
            index,
            axis,
        })
    }

    /// The family this font belongs to.
    pub fn family(&self) -> &str {
        &self.family
    }

    /// How heavy it is.
    ///
    /// For a variable font this is not a label but an **instruction**: it is
    /// the instance every face parsed out of these bytes is set to, which is
    /// what [`Font::at_weight`] changes.
    pub fn weight(&self) -> Weight {
        self.weight
    }

    /// The weights this font can be set to, when it is a variable one.
    pub fn weight_axis(&self) -> Option<WeightAxis> {
        self.axis
    }

    /// The weight this font can actually be set to, nearest the one asked for.
    ///
    /// For an ordinary face that is the weight it is, whatever was asked: a
    /// face has one, and a page asking for another is given it rather than
    /// nothing. For a variable one it is the weight itself wherever the axis
    /// reaches it — which is what makes such a font a candidate at every weight
    /// in its range rather than at the single one `OS/2` names.
    pub fn nearest_weight(&self, wanted: Weight) -> Weight {
        self.axis.map_or(self.weight, |axis| axis.nearest(wanted))
    }

    /// This font, set to the weight nearest the one asked for.
    ///
    /// Cheap: the bytes are shared, and for an ordinary face this is a clone
    /// and nothing else.
    #[must_use]
    pub fn at_weight(&self, wanted: Weight) -> Self {
        Self {
            weight: self.nearest_weight(wanted),
            ..self.clone()
        }
    }

    /// The coordinate this font's weight axis is set to, when it has one.
    ///
    /// `alo-paint` reads outlines with the parser **it** is allowed to name, so
    /// what crosses the crate boundary is this number and [`WEIGHT_AXIS`]
    /// rather than a face parsed here.
    pub fn variable_weight(&self) -> Option<f32> {
        self.axis.map(|_| f32::from(self.weight.value()))
    }

    /// Whether it is slanted.
    pub fn slant(&self) -> Slant {
        self.slant
    }

    /// The bytes, for whoever needs to shape or rasterise with them.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Which face within the bytes, for a collection.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// Whether this font has a glyph for a character.
    ///
    /// This is the question the fallback chain asks, and it is asked of the
    /// font rather than guessed from a language tag — a font either has the
    /// glyph or it does not, and there is nothing to infer.
    pub fn has_glyph(&self, character: char) -> bool {
        self.face()
            .and_then(|face| face.glyph_index(character))
            .is_some()
    }

    /// The measurements of this font at a size, in CSS pixels.
    pub fn metrics(&self, size: f32) -> FaceMetrics {
        let Some(face) = self.face() else {
            return FaceMetrics {
                ascender: size * 0.8,
                descender: size * 0.2,
                line_gap: 0.0,
                x_height: size * 0.5,
                zero_width: size * 0.5,
                underline_offset: size * 0.1,
                underline_thickness: size * 0.06,
            };
        };
        let units = f32::from(face.units_per_em());
        let scale = if units > 0.0 { size / units } else { 0.0 };
        FaceMetrics {
            ascender: f32::from(face.ascender()) * scale,
            descender: f32::from(face.descender()).abs() * scale,
            line_gap: f32::from(face.line_gap()) * scale,
            x_height: face
                .x_height()
                .map_or(size * 0.5, |height| f32::from(height) * scale),
            zero_width: face
                .glyph_index('0')
                .and_then(|glyph| face.glyph_hor_advance(glyph))
                .map_or(size * 0.5, |advance| f32::from(advance) * scale),
            // The face reports the position as a signed offset from the
            // baseline, negative for below it, which is where an underline
            // goes. Kept as a positive distance downwards, so that everything
            // reading it does not have to remember the sign.
            underline_offset: face
                .underline_metrics()
                .map_or(size * 0.1, |line| -f32::from(line.position) * scale),
            underline_thickness: face
                .underline_metrics()
                .map(|line| f32::from(line.thickness) * scale)
                // A face that reports no thickness, or a nonsensical one, gets
                // a line that is visible at every size rather than none at all.
                .filter(|thickness| *thickness > 0.0)
                .unwrap_or(size * 0.06),
        }
    }

    /// The parsed face, **at this font's instance**.
    ///
    /// Every measurement in this file goes through here, so a variable font is
    /// measured at the weight it was set to rather than at the one its `OS/2`
    /// names. That matters even where no outline has moved: `HVAR` varies the
    /// advance of a glyph, so a `ch` unit is a different number at 700 than at
    /// 400.
    fn face(&self) -> Option<ttf_parser::Face<'_>> {
        let mut face = ttf_parser::Face::parse(&self.data, self.index).ok()?;
        if let Some(value) = self.variable_weight() {
            // A font that declared the axis and will not be set to it is a
            // broken file rather than an error here: what comes back is the
            // default instance, which is what a font with no axis gives.
            let _ = face.set_variation(ttf_parser::Tag::from_bytes(&WEIGHT_AXIS), value);
        }
        Some(face)
    }

    /// The shaper's face, at this font's instance.
    ///
    /// In this file rather than in [`crate::shape`] because it is the same
    /// sentence as [`Font::face`]: *which instance of these bytes this font
    /// is*. Two files parsing the same bytes and setting the axis separately is
    /// how a line comes to be measured at one weight and drawn at another.
    pub(crate) fn shaper(&self) -> Option<rustybuzz::Face<'_>> {
        let mut face = rustybuzz::Face::from_slice(&self.data, self.index)?;
        if let Some(value) = self.variable_weight() {
            face.set_variations(&[rustybuzz::Variation {
                tag: ttf_parser::Tag::from_bytes(&WEIGHT_AXIS),
                value,
            }]);
        }
        Some(face)
    }
}

impl fmt::Debug for Font {
    /// The name and the weight — never the bytes, which are a megabyte of
    /// glyphs nobody wants in a test failure. A variable font names the range
    /// it covers as well, because "700" alone does not say whether that was a
    /// face or an instance.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Font({} {} {:?}", self.family, self.weight, self.slant)?;
        if let Some(axis) = self.axis {
            write!(f, " of {axis}")?;
        }
        f.write_str(")")
    }
}

/// `ttf-parser` is the font parser `rustybuzz` itself is built on, so reading a
/// face here and shaping with it later are the same parse of the same bytes.
use rustybuzz::ttf_parser;

/// The family a font file states about **itself**.
///
/// Everywhere else in this engine a family is a name somebody supplied: the
/// browser process reads a directory and guesses from the filename, because
/// `HelveticaNeue-Bold.ttf` is nearly always Helvetica Neue and opening every
/// font on a machine to ask would be most of a second at startup. A guess is
/// fine for filling a database.
///
/// It is **not** fine for answering *"does this machine have Inter"*. That
/// answer decides whether a page is drawn in the font its author asked for or
/// in one this engine chose, and an answer derived from how somebody named a
/// file is an answer that is wrong for every font named differently. So this
/// asks the font.
///
/// Two names are read because fonts carry two. A large family splits itself
/// into `Inter`, `Inter Light`, `Inter Semibold` under the older name so that
/// software which can only hold four faces per family still works; the
/// typographic name is the one that says `Inter` throughout, and it is the one
/// CSS means. So it wins where a font has both.
///
/// [`None`] for bytes that are not a font, and for one whose names are all
/// unreadable — both real answers about a file, and neither a reason to fail.
///
/// # A name written before Unicode
///
/// A `name` table holds the same name once per platform that was ever expected
/// to read it, and a font that carries **only** the Macintosh records — several
/// of the ones macOS ships do — used to answer [`None`] here. The machine had
/// the font, the engine said it did not, and a page asking for it by name got a
/// substitution nobody could explain. [`crate::macintosh`] reads those records
/// now, for the two encodings a rented table defines exactly and no others.
///
/// **A Unicode name still wins wherever a font has one**, which is what keeps
/// this from changing the answer for any font that already had a readable name:
/// a Macintosh record comes first in a well-formed table, and Mac OS Roman
/// cannot spell every family that UTF-16 can.
///
/// The alternative to all of this — falling back to the filename — was refused
/// deliberately. It would put a guess back inside the one answer that must be a
/// fact.
///
/// # A name is written in a language
///
/// A `name` table holds the same name once per **language** as well as once per
/// platform. macOS's system font states its family thirty-five times over —
/// `System Font`, `Police système`, `システムフォント` — and a rule that took the
/// first record of each kind files such a font under whichever language its
/// table happens to list first. On this machine that would be Catalan, and this
/// engine was saved from it only by the accident that the font's unlocalised
/// record comes before all of them. A family filed under a name no page will
/// ever ask for is [`crate::macintosh`]'s failure arriving by another road.
///
/// So where two records are of the same kind, the language decides between
/// them, and [`Spoken`] is that order: a name that states **no language** is the
/// font's own name, **English** is the language a `font-family` is nearly always
/// written in, and any **other** language is a translation of one of those. The
/// first record wins between two that say as much as each other, so a font
/// carrying only translations is filed under the first of them — a font in one
/// language is still a font somebody has.
///
/// # A name is text a person could type
///
/// The bytes were written by somebody else, so two rules apply to every record
/// whatever it is encoded in: a name longer than [`LONGEST_NAME`] is not a
/// family name, and neither is one carrying a control character. Both are
/// skipped rather than trimmed into shape — a name half-cleaned is a name that
/// matches something by accident.
pub fn family_in(data: &[u8]) -> Option<String> {
    let face = ttf_parser::Face::parse(data, 0).ok()?;
    let mut stated = Stated::default();
    for name in face.names() {
        if name.name.len() > LONGEST_NAME {
            continue;
        }
        // A name in an encoding nobody here reads is skipped rather than fatal:
        // a font commonly carries the same name several times over, and one
        // unreadable copy says nothing about the next.
        let (text, unicode) = match name.to_string() {
            Some(text) => (text, true),
            None => match name.platform_id {
                ttf_parser::PlatformId::Macintosh => {
                    match crate::macintosh::text(name.encoding_id, name.name) {
                        Some(text) => (text, false),
                        None => continue,
                    }
                }
                _ => continue,
            },
        };
        let text = text.trim();
        if text.is_empty() || text.chars().any(char::is_control) {
            continue;
        }
        let held = match (name.name_id, unicode) {
            (ttf_parser::name_id::TYPOGRAPHIC_FAMILY, true) => &mut stated.typographic,
            (ttf_parser::name_id::TYPOGRAPHIC_FAMILY, false) => &mut stated.typographic_legacy,
            (ttf_parser::name_id::FAMILY, true) => &mut stated.family,
            (ttf_parser::name_id::FAMILY, false) => &mut stated.family_legacy,
            _ => continue,
        };
        held.offer(text, spoken_in(&name));
    }
    stated
        .typographic
        .text
        .or(stated.typographic_legacy.text)
        .or(stated.family.text)
        .or(stated.family_legacy.text)
}

/// The most bytes a name record may be and still be a family name.
///
/// "Helvetica Neue" is fourteen and the longest family anybody ships is a small
/// multiple of that. This is room for all of them in UTF-16, and a bound
/// because the number is otherwise chosen by whoever wrote the font file.
pub const LONGEST_NAME: usize = 512;

/// What a font said about itself, before the order below is applied to it.
///
/// Four slots rather than two, because *which* name and *how readable* it is
/// are separate questions and each has its own answer. The typographic name
/// wins because it is the family CSS means; a Unicode record wins within each
/// because it can spell names the older encodings cannot.
///
/// The **language** decides inside a slot rather than between slots, which is
/// the difference between this and a fifth question. A font may state its
/// typographic name in one language and its older name in another, and the
/// typographic one is still the family CSS means; a language that outranked the
/// kind of name would file such a font under a name for four of its faces.
#[derive(Default)]
struct Stated {
    typographic: Held,
    typographic_legacy: Held,
    family: Held,
    family_legacy: Held,
}

/// The best name found for one slot so far, and how much its record's language
/// said.
///
/// [`Spoken`] is meaningless while there is no text: an empty slot takes
/// whatever is offered to it.
#[derive(Default)]
struct Held {
    text: Option<String>,
    spoken: Spoken,
}

impl Held {
    /// Take this name if its record said more about its language than the one
    /// held, and leave the slot alone otherwise.
    ///
    /// Strictly more, so that the **first** of two records saying as much as
    /// each other is kept — a font that states its family in Catalan and then in
    /// Croatian is filed under the Catalan, which is the order the file itself
    /// put them in and the only order there is anything to go on.
    fn offer(&mut self, text: &str, spoken: Spoken) {
        if self.text.is_some() && spoken <= self.spoken {
            return;
        }
        self.text = Some(text.to_owned());
        self.spoken = spoken;
    }
}

/// What a name record's language says about whether a page would ask for the
/// font by that name — weakest first, which is the order [`Held::offer`]
/// compares in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
enum Spoken {
    /// A language that is not English: a translation of the font's name, and a
    /// name a stylesheet is very unlikely to write. Weakest, and the default,
    /// because it is what a record says when it says nothing this engine can
    /// use.
    #[default]
    Translated,
    /// English, which is the language a `font-family` is nearly always written
    /// in and the language every other record here is a translation of.
    English,
    /// No language at all, which is the font's own name rather than a
    /// translation of it: the Unicode platform defines no language ids, so a
    /// record there is not written *in* anything.
    ///
    /// Above English deliberately, and macOS is the evidence rather than the
    /// specification: `SFNS.ttf` carries `.SF NS` on the Unicode platform and
    /// `System Font` in its English Windows record, and CoreText answers
    /// `.SF NS` for the family while keeping `System Font` as the name to show
    /// a person. Reading it the other way round would file the system font of
    /// this machine under a name the machine itself does not use.
    Unstated,
}

/// Which language a name record was written in, as far as this engine can tell.
///
/// The Windows language ids are a list of somebody else's — eighteen of them
/// are English — so the rented table answers, exactly as [`crate::macintosh`]
/// rents the tables for the encodings. It covers the Macintosh records too:
/// Apple's language code 0 is English, which is what nearly every Macintosh
/// record carries.
///
/// Two readings are decided here rather than left to that table's [`None`]:
///
/// - **The Unicode platform states no language**, so a record there is
///   [`Spoken::Unstated`] rather than a translation.
/// - **Except when it states one anyway.** A `name` table in format 1 may put a
///   language *tag* — `en`, `fr`, `ja` — behind an id of 0x8000 or more, and
///   this engine does not read those. Such a record has named a language we
///   cannot check, so it is treated as a translation: the weakest answer, which
///   is the one that cannot promote a localised name over an English one.
fn spoken_in(name: &ttf_parser::name::Name<'_>) -> Spoken {
    if name.platform_id == ttf_parser::PlatformId::Unicode && name.language_id == 0 {
        return Spoken::Unstated;
    }
    if name.language().primary_language() == ENGLISH {
        Spoken::English
    } else {
        Spoken::Translated
    }
}

/// What the rented table calls the language a `font-family` is written in.
const ENGLISH: &str = "English";

/// The weight and slant a font file states about **itself**.
///
/// The sibling of [`family_in`], and the same argument one table further on.
/// That function took the *family* off the font rather than off the name of the
/// file it was read out of; the weight and the slant were still guessed at by
/// looking for `bold` and `italic` in a filename, which is wrong for every file
/// somebody named by another convention — `Helvetica-Oblique`,
/// `InterDisplay-SemiBold`, a variable font with a weight axis and no word for
/// it in its name.
///
/// It is a smaller wrong than the family was, and that is why it came second: a
/// face filed under the wrong weight is still drawn in the right family, because
/// [`crate::FontDatabase`] chooses among the faces of the family it holds. A
/// family read off a filename is not drawn at all.
///
/// [`None`] for bytes that are not a font this engine can read — the same
/// answer, for the same reason, as [`family_in`] gives.
///
/// # A font that states nothing is still a font
///
/// `OS/2` is where both are written, and it is the one table a font may be
/// missing and still be a font — some of the older Macintosh ones are. Such a
/// face is **normal and upright** rather than nothing: a family of one
/// unlabelled face is most of the fonts on a machine, and refusing it would be
/// this engine losing a font over a table nobody needed. It is not quite
/// nothing, either: a face that leans says so in `post` as well, and that is
/// still read.
///
/// The alternative — reading the filename when the table says nothing — was
/// refused for [`family_in`]'s reason: it would put the guess back, in exactly
/// the files where nothing else could contradict it.
///
/// # A weight is a number somebody else wrote
///
/// `usWeightClass` runs from 1 to 1000 and a font may hold anything in two
/// bytes. Two readings are decided here rather than left to whatever the number
/// happens to clamp to:
///
/// - **Zero is not a statement.** It is what a font writes when it did not say,
///   and several do. Clamped into range it would become 1, the lightest face
///   CSS can name — a statement, and a wrong one. So the only other thing the
///   table says about heaviness is read instead: the **bold bit**, which is the
///   two-value shorthand older software went by. A font that states neither is
///   [`Weight::NORMAL`]. A font that states a number is taken at its word even
///   where the bit disagrees, because the number is the finer answer and CSS
///   asks its question as a number.
/// - **A number in 1..=9 is read as written.** Some fonts older than the
///   current specification meant that as the nine-point scale, so 9 was black;
///   today 9 is very nearly invisible. The two are the same bytes, nothing in
///   the file says which was meant, and a guess would draw somebody's page in a
///   face nobody chose.
///
/// # A font may be many weights
///
/// `OS/2` states one, and a variable font is one file holding a continuum. The
/// number above is still read and is still right — it is the **default
/// instance**, what the outlines are when nobody has set the axis — and
/// [`Style::axis`] is the rest of the answer. Reading only the number files
/// such a font under it and hides every other weight it has, which is what this
/// engine did to macOS's `SFCompact` (stated 1000) and `SFNSMono` (stated 295).
pub fn style_in(data: &[u8]) -> Option<Style> {
    let face = ttf_parser::Face::parse(data, 0).ok()?;
    let stated = face.weight().to_number();
    Some(Style {
        axis: weight_axis_of(&face),
        weight: if stated == 0 {
            if face.is_bold() {
                Weight::BOLD
            } else {
                Weight::NORMAL
            }
        } else {
            Weight::new(stated)
        },
        // Two questions, because `OS/2` answers them separately and a face is
        // slanted either way: the italic bit — or a face that leans without
        // having set it, which is an angle in the `post` table — and oblique,
        // which is the third value CSS has and this engine does not.
        slant: if face.is_italic() || face.is_oblique() {
            Slant::Italic
        } else {
            Slant::Normal
        },
    })
}

/// The weight axis a font declares, if it declares one this engine can use.
///
/// `fvar` lists the axes a variable font varies along and `wght` is the one CSS
/// asks about by number. The others — `wdth`, `slnt`, `opsz` — are read past
/// here rather than guessed at: each is a separate CSS property with its own
/// grammar, and a font is not narrower because this engine assumed an axis it
/// did not look at.
///
/// [`None`] in three cases, and each of them means *treat this as the one face
/// `OS/2` describes*, which is the answer that cannot draw a page in a weight
/// nobody asked for:
///
/// - **No `wght` axis.** Most fonts, including every variable font that only
///   varies its width.
/// - **An axis of no width**, where the two ends land on the same weight. Such
///   a font is a static face spelt at greater length, and calling it variable
///   would make it a candidate for every request while it can only ever draw
///   the one thing.
/// - **An axis that ends below [`THINNEST_CSS_NAMES`]**, which is not written
///   in CSS's scale at all — see below.
///
/// # An axis older than the scale it is supposed to be in
///
/// `wght` got its shared meaning when OpenType took variations on in 2016.
/// Apple's own earlier fonts had axes before that and wrote them in scales of
/// their own: this machine's `Skia.ttf` runs from **1 to 3**, and its `OS/2`
/// states 5, which is the very thing [`style_in`] already refuses to guess at
/// one table earlier.
///
/// Read as CSS numbers, such an axis is entirely hairline, so *every* request
/// lands on its heaviest end and a page of ordinary text is drawn in the
/// blackest thing the file has. Refusing it leaves the font exactly as it was
/// before this engine could read an axis at all, which is the safe direction
/// and the same one item 194 chose for the same reason.
///
/// The line is drawn at the lightest weight CSS has a *word* for. Nothing that
/// means the shared scale ends below `thin`; everything that ends below it
/// means something else.
fn weight_axis_of(face: &ttf_parser::Face<'_>) -> Option<WeightAxis> {
    let wanted = ttf_parser::Tag::from_bytes(&WEIGHT_AXIS);
    let axis = face
        .variation_axes()
        .into_iter()
        .find(|axis| axis.tag == wanted)?;
    let lightest = on_the_scale(axis.min_value);
    let heaviest = on_the_scale(axis.max_value);
    if heaviest < Weight::new(THINNEST_CSS_NAMES) {
        return None;
    }
    (lightest < heaviest).then_some(WeightAxis { lightest, heaviest })
}

/// The lightest weight CSS has a name for: `font-weight: thin`.
///
/// A bound on what counts as an axis written in CSS's scale, and not a bound on
/// what a page may ask for — [`Weight`] goes down to 1, because `font-weight:
/// 50` is a number an author may write and a font may answer.
pub const THINNEST_CSS_NAMES: u16 = 100;

/// A number out of somebody else's file, as a weight CSS can name.
///
/// The scale is shared — `wght` is defined in `font-weight`'s own numbers — so
/// this rounds and bounds rather than converting.
///
/// There is no case for "not a number": `fvar` writes an axis bound as 16.16
/// fixed point, which is four bytes read as an integer and divided, so every
/// value a file can hold is finite. A guard against a NaN here would be a
/// branch no font could reach and no test could reach either.
fn on_the_scale(value: f32) -> Weight {
    let bounded = value.clamp(1.0, 1000.0).round();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "bounded to 1..=1000 on the line above, so it fits and is positive"
    )]
    let number = bounded as u16;
    Weight::new(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dejavu() -> Font {
        Font::load(
            "DejaVu Sans",
            Weight::NORMAL,
            Slant::Normal,
            dejavu::sans::regular().to_vec(),
        )
        .expect("the DejaVu Sans this crate is tested with")
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 0.01
    }

    #[test]
    fn a_font_says_what_family_it_belongs_to() {
        assert_eq!(
            family_in(dejavu::sans::regular()).as_deref(),
            Some("DejaVu Sans"),
        );
        assert_eq!(
            family_in(dejavu::serif::regular()).as_deref(),
            Some("DejaVu Serif"),
        );
        assert_eq!(
            family_in(dejavu::sans::bold()).as_deref(),
            Some("DejaVu Sans"),
            "a bold face belongs to the same family as the regular one",
        );
    }

    #[test]
    fn bytes_that_are_not_a_font_name_no_family() {
        assert_eq!(family_in(&[]), None);
        assert_eq!(family_in(&[0; 64]), None);
        // A real font, truncated: the parser is handed something that starts
        // like a font and stops, which is what a half-copied file looks like.
        let cut = &dejavu::sans::regular()[..1024];
        assert_eq!(family_in(cut), None);
    }

    #[test]
    fn a_font_that_is_not_a_font_is_refused_rather_than_held() {
        assert!(Font::load("nonsense", Weight::NORMAL, Slant::Normal, vec![0; 64]).is_none());
        assert!(Font::load("empty", Weight::NORMAL, Slant::Normal, Vec::new()).is_none());
    }

    #[test]
    fn a_loaded_font_reports_what_it_was_loaded_as() {
        let font = dejavu();
        assert_eq!(font.family(), "DejaVu Sans");
        assert_eq!(font.weight(), Weight::NORMAL);
        assert_eq!(font.slant(), Slant::Normal);
        assert!(!font.data().is_empty());
        assert_eq!(font.index(), 0);
        assert_eq!(format!("{font:?}"), "Font(DejaVu Sans 400 Normal)");
    }

    #[test]
    fn a_font_knows_which_characters_it_has() {
        let font = dejavu();
        assert!(font.has_glyph('a'));
        assert!(font.has_glyph('é'));
        assert!(font.has_glyph('م'), "DejaVu Sans covers Arabic");
        assert!(
            !font.has_glyph('क'),
            "and not Devanagari, which is what the fallback chain is for",
        );
    }

    #[test]
    fn the_metrics_scale_with_the_size_asked_for() {
        let font = dejavu();
        let small = font.metrics(16.0);
        let large = font.metrics(32.0);
        assert!(close(large.ascender, small.ascender * 2.0));
        assert!(close(large.x_height, small.x_height * 2.0));
        assert!(close(large.zero_width, small.zero_width * 2.0));
        assert!(small.ascender > 0.0 && small.descender > 0.0);
        assert!(close(
            small.suggested_line_height(),
            small.ascender + small.descender + small.line_gap,
        ),);
    }

    #[test]
    fn a_font_family_list_is_read_in_order_and_unquoted() {
        assert_eq!(
            FontRequest::parse_families("Inter, \"Helvetica Neue\", system-ui, sans-serif"),
            vec!["Inter", "Helvetica Neue", "system-ui", "sans-serif"],
        );
        assert_eq!(
            FontRequest::parse_families("  'One Font'  "),
            vec!["One Font"]
        );
        assert_eq!(
            FontRequest::parse_families(",,"),
            Vec::<String>::new(),
            "a list of nothing is a list of nothing",
        );
    }

    #[test]
    fn a_weight_is_kept_inside_the_range_css_allows() {
        assert_eq!(Weight::new(0).value(), 1);
        assert_eq!(Weight::new(5000).value(), 1000);
        assert_eq!(Weight::new(600).value(), 600);
        assert_eq!(Weight::default(), Weight::NORMAL);
        assert!(Weight::BOLD > Weight::NORMAL);
        assert_eq!(Weight::BOLD.to_string(), "700");
    }

    #[test]
    fn a_request_for_one_family_is_a_request_for_one_family() {
        let request = FontRequest::family("Inter");
        assert_eq!(request.families, vec!["Inter"]);
        assert_eq!(request.weight, Weight::NORMAL);
        assert_eq!(request.slant, Slant::Normal);
    }
}
