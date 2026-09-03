/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The boundary's messages, as bytes.
//!
//! ADR 0005 built the boundary as a *type* so that the process split would be a
//! change of **transport** rather than a redesign. This is that transport's
//! encoding, and it is the piece that had to be right before anything is
//! spawned.
//!
//! # Which direction is untrusted
//!
//! Both, and the second one is the one people forget.
//!
//! A renderer must not trust the browser process blindly either, but the
//! browser process holds the network, the disk and the profile — so the message
//! that matters is the one coming **back**. A renderer is the process that
//! parsed a hostile page; if that page found a way to steer it, everything the
//! renderer says afterwards is the page talking. So the decoder below treats
//! every length as a number a stranger chose, and every id as a claim rather
//! than a fact.
//!
//! # Why the encoding is written out rather than derived
//!
//! Deriving it would mean a serialisation crate reaching into `alo-box`,
//! `alo-agent`, `alo-layout` and `alo-paint` — four crates gaining a dependency
//! and a set of derives for the benefit of one boundary. ADR 0005 says the
//! protocol has to be **coarse**, and a coarse protocol is small enough to
//! write down. Writing it down also means the wire format is a thing somebody
//! can read, which matters for a boundary that is a security boundary.

use crate::face::Face;
use crate::frame::Frame;
use crate::message::{Failure, FromRenderer, ToRenderer};
use crate::page::Page;
use crate::snapshot::{Snapshot, SnapshotNode};
use alo_agent::verb::{Outcome, Refusal, ScrollBy, Target, Verb};
use alo_box::role::{KnownRole, Role};
use alo_box::state::{Checked, Current, States};
use alo_box::tree::BoxId;
use alo_css::media::ColorScheme;
use alo_layout::geometry::{Point, Rect, Size};
use alo_text::{Slant, Weight};

/// The most bytes one message may be.
///
/// A frame of pixels is the largest thing that crosses, and a very large window
/// is a few tens of megabytes. Sixty-four is room for that and a bound on
/// everything else — without one, four bytes on a pipe are an allocation
/// somebody else chose the size of.
pub const LARGEST_MESSAGE: usize = 64 * 1024 * 1024;

/// The most nodes deep a snapshot may be.
///
/// A tree arrives as a recursive structure, and a decoder that recursed as
/// deeply as it was told would run out of stack on a message rather than
/// returning an error. This is what makes the depth a refusal instead.
pub const DEEPEST_TREE: usize = 512;

/// Why a message could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unreadable {
    /// In words.
    pub why: String,
}

impl core::fmt::Display for Unreadable {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.why)
    }
}

impl std::error::Error for Unreadable {}

fn unreadable(why: impl Into<String>) -> Unreadable {
    Unreadable { why: why.into() }
}

// --- Writing -----------------------------------------------------------------

/// Bytes being built.
#[derive(Debug, Default)]
struct Writer {
    out: Vec<u8>,
}

impl Writer {
    fn tag(&mut self, tag: u8) {
        self.out.push(tag);
    }
    fn bool(&mut self, yes: bool) {
        self.out.push(u8::from(yes));
    }
    fn number(&mut self, value: u64) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }
    fn float(&mut self, value: f32) {
        self.out.extend_from_slice(&value.to_be_bytes());
    }
    fn bytes(&mut self, value: &[u8]) {
        self.number(value.len() as u64);
        self.out.extend_from_slice(value);
    }
    fn text(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }
    fn maybe_text(&mut self, value: Option<&str>) {
        match value {
            Some(text) => {
                self.bool(true);
                self.text(text);
            }
            None => self.bool(false),
        }
    }
    fn maybe_bool(&mut self, value: Option<bool>) {
        match value {
            Some(yes) => {
                self.bool(true);
                self.bool(yes);
            }
            None => self.bool(false),
        }
    }
    fn id(&mut self, value: BoxId) {
        self.number(value.as_usize() as u64);
    }
    fn role(&mut self, role: &Role) {
        match role {
            Role::Generic => self.tag(0),
            Role::Presentational => self.tag(1),
            Role::Known(known) => {
                self.tag(2);
                // By name rather than by number, so that adding a role never
                // renumbers the others — and so the wire is readable.
                self.text(known.as_str());
            }
            Role::Declared(name) => {
                self.tag(3);
                self.text(name);
            }
        }
    }
    fn rect(&mut self, rect: Rect) {
        self.float(rect.origin.x);
        self.float(rect.origin.y);
        self.float(rect.size.width);
        self.float(rect.size.height);
    }
    fn states(&mut self, states: &States) {
        self.bool(states.disabled);
        match states.checked {
            None => self.tag(0),
            Some(Checked::No) => self.tag(1),
            Some(Checked::Yes) => self.tag(2),
            Some(Checked::Mixed) => self.tag(3),
        }
        self.maybe_bool(states.selected);
        self.maybe_bool(states.expanded);
        self.maybe_bool(states.pressed);
        self.bool(states.required);
        self.bool(states.read_only);
        self.bool(states.busy);
        self.bool(states.invalid);
        self.bool(states.hidden);
        match states.level {
            Some(level) => {
                self.bool(true);
                self.out.push(level);
            }
            None => self.bool(false),
        }
        match states.current {
            None => self.tag(0),
            Some(Current::Yes) => self.tag(1),
            Some(Current::Page) => self.tag(2),
            Some(Current::Step) => self.tag(3),
            Some(Current::Location) => self.tag(4),
            Some(Current::Date) => self.tag(5),
            Some(Current::Time) => self.tag(6),
        }
        self.bool(states.takes_text);
    }
    fn target(&mut self, target: &Target) {
        match target {
            Target::Named(name) => {
                self.tag(0);
                self.text(name);
            }
            Target::OfRole(role) => {
                self.tag(1);
                self.role(role);
            }
            Target::NamedOfRole { role, name } => {
                self.tag(2);
                self.role(role);
                self.text(name);
            }
            Target::Node(id) => {
                self.tag(3);
                self.id(*id);
            }
        }
    }
    fn scroll(&mut self, by: ScrollBy) {
        match by {
            ScrollBy::Pixels(amount) => {
                self.tag(0);
                self.float(amount);
            }
            ScrollBy::ToStart => self.tag(1),
            ScrollBy::ToEnd => self.tag(2),
        }
    }
    fn node(&mut self, node: &SnapshotNode) {
        self.id(node.id);
        self.role(&node.role);
        self.maybe_text(node.name.as_deref());
        self.states(&node.states);
        self.rect(node.rect);
        self.number(node.rects.len() as u64);
        for rect in &node.rects {
            self.rect(*rect);
        }
        self.bool(node.offscreen);
        self.bool(node.scrolls);
        self.number(node.children.len() as u64);
        for child in &node.children {
            self.node(child);
        }
    }
}

/// One message from the browser process, as bytes.
pub fn write_to_renderer(message: &ToRenderer) -> Vec<u8> {
    let mut writer = Writer::default();
    match message {
        ToRenderer::UseFont(face) => {
            writer.tag(5);
            writer.text(&face.family);
            writer.number(u64::from(face.weight));
            writer.tag(match face.slant {
                Slant::Normal => 0,
                Slant::Italic => 1,
            });
            writer.bytes(&face.bytes);
        }
        ToRenderer::Load(page) => {
            writer.tag(0);
            writer.text(&page.html);
            writer.number(page.sheets.len() as u64);
            for sheet in &page.sheets {
                writer.text(sheet);
            }
            writer.float(page.viewport.width);
            writer.float(page.viewport.height);
            writer.tag(match page.scheme {
                ColorScheme::Light => 0,
                ColorScheme::Dark => 1,
            });
        }
        ToRenderer::Resize(size) => {
            writer.tag(1);
            writer.float(size.width);
            writer.float(size.height);
        }
        ToRenderer::Paint => writer.tag(2),
        ToRenderer::ReadTree => writer.tag(3),
        ToRenderer::Act { target, verb } => {
            writer.tag(4);
            writer.target(target);
            match verb {
                Verb::Activate => writer.tag(0),
                Verb::PutText(text) => {
                    writer.tag(1);
                    writer.text(text);
                }
                Verb::Scroll(by) => {
                    writer.tag(2);
                    writer.scroll(*by);
                }
            }
        }
    }
    writer.out
}

/// One message from a renderer, as bytes.
pub fn write_from_renderer(message: &FromRenderer) -> Vec<u8> {
    let mut writer = Writer::default();
    match message {
        FromRenderer::UsingFont { family } => {
            writer.tag(6);
            writer.text(family);
        }
        FromRenderer::Loaded { issues } => {
            writer.tag(0);
            writer.number(issues.len() as u64);
            for issue in issues {
                writer.text(issue);
            }
        }
        FromRenderer::Painted(frame) => {
            writer.tag(1);
            writer.number(u64::from(frame.width));
            writer.number(u64::from(frame.height));
            writer.bytes(&frame.pixels);
        }
        FromRenderer::Tree(snapshot) => {
            writer.tag(2);
            match &snapshot.root {
                Some(root) => {
                    writer.bool(true);
                    writer.node(root);
                }
                None => writer.bool(false),
            }
        }
        FromRenderer::Acted(outcome) => {
            writer.tag(3);
            writer.outcome(outcome);
        }
        FromRenderer::Refused(refusal) => {
            writer.tag(4);
            writer.refusal(refusal);
        }
        FromRenderer::Failed(failure) => {
            writer.tag(5);
            match failure {
                Failure::NothingLoaded => writer.tag(0),
                Failure::Unpaintable { why } => {
                    writer.tag(1);
                    writer.text(why);
                }
                Failure::NotAFont { family } => {
                    writer.tag(2);
                    writer.text(family);
                }
            }
        }
    }
    writer.out
}

impl Writer {
    fn outcome(&mut self, outcome: &Outcome) {
        let writer = self;
        match outcome {
            Outcome::Activated { node, name } => {
                writer.tag(0);
                writer.id(*node);
                writer.maybe_text(name.as_deref());
            }
            Outcome::Followed { node, to } => {
                writer.tag(1);
                writer.id(*node);
                writer.text(to);
            }
            Outcome::TextPut { node, text } => {
                writer.tag(2);
                writer.id(*node);
                writer.text(text);
            }
            Outcome::Scrolled { node, by } => {
                writer.tag(3);
                writer.id(*node);
                writer.scroll(*by);
            }
        }
    }

    fn refusal(&mut self, refusal: &Refusal) {
        let writer = self;
        match refusal {
            Refusal::NotFound { target } => {
                writer.tag(0);
                writer.target(target);
            }
            Refusal::Ambiguous { target, candidates } => {
                writer.tag(1);
                writer.target(target);
                writer.number(candidates.len() as u64);
                for id in candidates {
                    writer.id(*id);
                }
            }
            Refusal::NotOperable { node, role } => {
                writer.tag(2);
                writer.id(*node);
                writer.role(role);
            }
            Refusal::Disabled { node } => {
                writer.tag(3);
                writer.id(*node);
            }
            Refusal::NotAField { node, role } => {
                writer.tag(4);
                writer.id(*node);
                writer.role(role);
            }
            Refusal::ReadOnly { node } => {
                writer.tag(5);
                writer.id(*node);
            }
            Refusal::DoesNotScroll { node } => {
                writer.tag(6);
                writer.id(*node);
            }
        }
    }
}

// --- Reading -----------------------------------------------------------------

/// Bytes being taken apart, none of which are trusted.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    depth: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            at: 0,
            depth: 0,
        }
    }

    fn tag(&mut self) -> Result<u8, Unreadable> {
        let byte = self
            .bytes
            .get(self.at)
            .copied()
            .ok_or_else(|| unreadable("a message that stops where a tag should be"))?;
        self.at += 1;
        Ok(byte)
    }

    fn bool(&mut self) -> Result<bool, Unreadable> {
        Ok(self.tag()? != 0)
    }

    fn number(&mut self) -> Result<u64, Unreadable> {
        let slice = self
            .bytes
            .get(self.at..self.at + 8)
            .ok_or_else(|| unreadable("a message that stops where a number should be"))?;
        let mut eight = [0u8; 8];
        eight.copy_from_slice(slice);
        self.at += 8;
        Ok(u64::from_be_bytes(eight))
    }

    fn float(&mut self) -> Result<f32, Unreadable> {
        let slice = self
            .bytes
            .get(self.at..self.at + 4)
            .ok_or_else(|| unreadable("a message that stops where a number should be"))?;
        let mut four = [0u8; 4];
        four.copy_from_slice(slice);
        self.at += 4;
        let value = f32::from_be_bytes(four);
        // A NaN or an infinity in a rect is a number every comparison answers
        // "false" to, which turns a bounds check into a thing that passes.
        if !value.is_finite() {
            return Err(unreadable("a number that is not a number"));
        }
        Ok(value)
    }

    /// A length, checked against what is actually left.
    ///
    /// The one line this whole file is built around: a length is a number
    /// somebody else chose, and reserving before checking is how four bytes on
    /// a pipe become a gigabyte of allocation.
    fn length(&mut self) -> Result<usize, Unreadable> {
        let said = self.number()?;
        let said = usize::try_from(said)
            .map_err(|_| unreadable("a length larger than this machine can hold"))?;
        let left = self.bytes.len().saturating_sub(self.at);
        if said > left {
            return Err(unreadable(format!(
                "a length of {said} with {left} bytes left"
            )));
        }
        Ok(said)
    }

    fn bytes(&mut self) -> Result<Vec<u8>, Unreadable> {
        let how_many = self.length()?;
        let taken = self
            .bytes
            .get(self.at..self.at + how_many)
            .ok_or_else(|| unreadable("a run of bytes longer than the message"))?
            .to_vec();
        self.at += how_many;
        Ok(taken)
    }

    fn text(&mut self) -> Result<String, Unreadable> {
        String::from_utf8(self.bytes()?)
            .map_err(|_| unreadable("text that is not text this engine can read"))
    }

    fn maybe_text(&mut self) -> Result<Option<String>, Unreadable> {
        if self.bool()? {
            Ok(Some(self.text()?))
        } else {
            Ok(None)
        }
    }

    fn maybe_bool(&mut self) -> Result<Option<bool>, Unreadable> {
        if self.bool()? {
            Ok(Some(self.bool()?))
        } else {
            Ok(None)
        }
    }

    fn id(&mut self) -> Result<BoxId, Unreadable> {
        let number = self.number()?;
        Ok(BoxId::from_wire(usize::try_from(number).map_err(|_| {
            unreadable("an id larger than this machine can hold")
        })?))
    }

    /// A count of things, each of which costs at least one byte.
    ///
    /// So a count larger than the bytes remaining is a count that cannot be
    /// honest, and refusing it here is what stops a `Vec::with_capacity` of
    /// four billion.
    fn count(&mut self) -> Result<usize, Unreadable> {
        let said = self.number()?;
        let said = usize::try_from(said)
            .map_err(|_| unreadable("a count larger than this machine can hold"))?;
        if said > self.bytes.len().saturating_sub(self.at) {
            return Err(unreadable(format!(
                "a count of {said} in a message with no room for them"
            )));
        }
        Ok(said)
    }

    fn role(&mut self) -> Result<Role, Unreadable> {
        match self.tag()? {
            0 => Ok(Role::Generic),
            1 => Ok(Role::Presentational),
            2 => {
                let name = self.text()?;
                // A name this engine does not know becomes a declared role
                // rather than an error: the other side may be a version that
                // knows one more, and losing the name would lose it for the
                // agent too.
                Ok(match KnownRole::named(&name) {
                    Some(known) => Role::Known(known),
                    None => Role::Declared(name.into_boxed_str()),
                })
            }
            3 => Ok(Role::Declared(self.text()?.into_boxed_str())),
            other => Err(unreadable(format!("a role tagged {other}"))),
        }
    }

    fn rect(&mut self) -> Result<Rect, Unreadable> {
        Ok(Rect {
            origin: Point {
                x: self.float()?,
                y: self.float()?,
            },
            size: Size {
                width: self.float()?,
                height: self.float()?,
            },
        })
    }

    fn states(&mut self) -> Result<States, Unreadable> {
        Ok(States {
            disabled: self.bool()?,
            checked: match self.tag()? {
                0 => None,
                1 => Some(Checked::No),
                2 => Some(Checked::Yes),
                3 => Some(Checked::Mixed),
                other => return Err(unreadable(format!("a checked state tagged {other}"))),
            },
            selected: self.maybe_bool()?,
            expanded: self.maybe_bool()?,
            pressed: self.maybe_bool()?,
            required: self.bool()?,
            read_only: self.bool()?,
            busy: self.bool()?,
            invalid: self.bool()?,
            hidden: self.bool()?,
            level: if self.bool()? {
                Some(self.tag()?)
            } else {
                None
            },
            current: match self.tag()? {
                0 => None,
                1 => Some(Current::Yes),
                2 => Some(Current::Page),
                3 => Some(Current::Step),
                4 => Some(Current::Location),
                5 => Some(Current::Date),
                6 => Some(Current::Time),
                other => return Err(unreadable(format!("a current state tagged {other}"))),
            },
            takes_text: self.bool()?,
        })
    }

    fn target(&mut self) -> Result<Target, Unreadable> {
        match self.tag()? {
            0 => Ok(Target::Named(self.text()?)),
            1 => Ok(Target::OfRole(self.role()?)),
            2 => Ok(Target::NamedOfRole {
                role: self.role()?,
                name: self.text()?,
            }),
            3 => Ok(Target::Node(self.id()?)),
            other => Err(unreadable(format!("a target tagged {other}"))),
        }
    }

    fn scroll(&mut self) -> Result<ScrollBy, Unreadable> {
        match self.tag()? {
            0 => Ok(ScrollBy::Pixels(self.float()?)),
            1 => Ok(ScrollBy::ToStart),
            2 => Ok(ScrollBy::ToEnd),
            other => Err(unreadable(format!("a scroll tagged {other}"))),
        }
    }

    fn outcome(&mut self) -> Result<Outcome, Unreadable> {
        match self.tag()? {
            0 => Ok(Outcome::Activated {
                node: self.id()?,
                name: self.maybe_text()?,
            }),
            1 => Ok(Outcome::Followed {
                node: self.id()?,
                to: self.text()?,
            }),
            2 => Ok(Outcome::TextPut {
                node: self.id()?,
                text: self.text()?,
            }),
            3 => Ok(Outcome::Scrolled {
                node: self.id()?,
                by: self.scroll()?,
            }),
            other => Err(unreadable(format!("an outcome tagged {other}"))),
        }
    }

    fn refusal(&mut self) -> Result<Refusal, Unreadable> {
        match self.tag()? {
            0 => Ok(Refusal::NotFound {
                target: self.target()?,
            }),
            1 => {
                let target = self.target()?;
                let how_many = self.count()?;
                let mut candidates = Vec::new();
                for _ in 0..how_many {
                    candidates.push(self.id()?);
                }
                Ok(Refusal::Ambiguous { target, candidates })
            }
            2 => Ok(Refusal::NotOperable {
                node: self.id()?,
                role: self.role()?,
            }),
            3 => Ok(Refusal::Disabled { node: self.id()? }),
            4 => Ok(Refusal::NotAField {
                node: self.id()?,
                role: self.role()?,
            }),
            5 => Ok(Refusal::ReadOnly { node: self.id()? }),
            6 => Ok(Refusal::DoesNotScroll { node: self.id()? }),
            other => Err(unreadable(format!("a refusal tagged {other}"))),
        }
    }

    fn node(&mut self) -> Result<SnapshotNode, Unreadable> {
        self.depth += 1;
        if self.depth > DEEPEST_TREE {
            return Err(unreadable(format!(
                "a tree deeper than the {DEEPEST_TREE} this engine will read"
            )));
        }
        let id = self.id()?;
        let role = self.role()?;
        let name = self.maybe_text()?;
        let states = self.states()?;
        let rect = self.rect()?;
        let how_many = self.count()?;
        let mut rects = Vec::new();
        for _ in 0..how_many {
            rects.push(self.rect()?);
        }
        let offscreen = self.bool()?;
        let scrolls = self.bool()?;
        let how_many = self.count()?;
        let mut children = Vec::new();
        for _ in 0..how_many {
            children.push(self.node()?);
        }
        self.depth -= 1;
        Ok(SnapshotNode {
            id,
            role,
            name,
            states,
            rect,
            rects,
            offscreen,
            scrolls,
            children,
        })
    }

    /// Nothing may be left over.
    ///
    /// Trailing bytes mean the two ends disagree about the message, and a
    /// decoder that ignored them would let a sender append something a later
    /// version would read.
    fn finished(&self) -> Result<(), Unreadable> {
        if self.at == self.bytes.len() {
            return Ok(());
        }
        Err(unreadable(format!(
            "{} bytes left over after the message",
            self.bytes.len() - self.at
        )))
    }
}

/// Read a message meant for a renderer.
///
/// # Errors
///
/// [`Unreadable`] for anything that is not one, which includes everything a
/// hostile sender could put on a pipe.
pub fn read_to_renderer(bytes: &[u8]) -> Result<ToRenderer, Unreadable> {
    if bytes.len() > LARGEST_MESSAGE {
        return Err(unreadable("a message larger than this engine will read"));
    }
    let mut reader = Reader::new(bytes);
    let message = match reader.tag()? {
        0 => {
            let html = reader.text()?;
            let how_many = reader.count()?;
            let mut sheets = Vec::new();
            for _ in 0..how_many {
                sheets.push(reader.text()?);
            }
            let viewport = Size {
                width: reader.float()?,
                height: reader.float()?,
            };
            let scheme = match reader.tag()? {
                0 => ColorScheme::Light,
                1 => ColorScheme::Dark,
                other => return Err(unreadable(format!("a colour scheme tagged {other}"))),
            };
            ToRenderer::Load(Box::new(Page {
                html,
                sheets,
                viewport,
                scheme,
            }))
        }
        1 => ToRenderer::Resize(Size {
            width: reader.float()?,
            height: reader.float()?,
        }),
        2 => ToRenderer::Paint,
        3 => ToRenderer::ReadTree,
        4 => {
            let target = reader.target()?;
            let verb = match reader.tag()? {
                0 => Verb::Activate,
                1 => Verb::PutText(reader.text()?),
                2 => Verb::Scroll(reader.scroll()?),
                other => return Err(unreadable(format!("a verb tagged {other}"))),
            };
            ToRenderer::Act { target, verb }
        }
        5 => {
            let family = reader.text()?;
            let weight = u16::try_from(reader.number()?)
                .map_err(|_| unreadable("a font weight larger than any weight"))?;
            let slant = match reader.tag()? {
                0 => Slant::Normal,
                1 => Slant::Italic,
                other => return Err(unreadable(format!("a slant tagged {other}"))),
            };
            let bytes = reader.bytes()?;
            let face = Face::new(family, Weight::new(weight), slant, bytes).ok_or_else(|| {
                unreadable("a font of no bytes, or more than this engine carries")
            })?;
            ToRenderer::UseFont(Box::new(face))
        }
        other => return Err(unreadable(format!("a message tagged {other}"))),
    };
    reader.finished()?;
    Ok(message)
}

/// Read a message from a renderer.
///
/// # Errors
///
/// [`Unreadable`]. This is the direction that matters most: a renderer is the
/// process that parsed a hostile page, and everything it says afterwards may be
/// the page talking.
pub fn read_from_renderer(bytes: &[u8]) -> Result<FromRenderer, Unreadable> {
    if bytes.len() > LARGEST_MESSAGE {
        return Err(unreadable("a message larger than this engine will read"));
    }
    let mut reader = Reader::new(bytes);
    let message = match reader.tag()? {
        0 => {
            let how_many = reader.count()?;
            let mut issues = Vec::new();
            for _ in 0..how_many {
                issues.push(reader.text()?);
            }
            FromRenderer::Loaded { issues }
        }
        1 => {
            let width = u32::try_from(reader.number()?)
                .map_err(|_| unreadable("a frame wider than this engine will hold"))?;
            let height = u32::try_from(reader.number()?)
                .map_err(|_| unreadable("a frame taller than this engine will hold"))?;
            let pixels = reader.bytes()?;
            // The one cross-check the encoding cannot do on its own: a frame
            // whose size and pixels disagree is a frame something above would
            // read past the end of.
            let wanted = u64::from(width)
                .checked_mul(u64::from(height))
                .and_then(|area| area.checked_mul(4))
                .ok_or_else(|| unreadable("a frame larger than any picture"))?;
            if wanted != pixels.len() as u64 {
                return Err(unreadable(format!(
                    "a {width}×{height} frame carrying {} bytes rather than {wanted}",
                    pixels.len()
                )));
            }
            FromRenderer::Painted(Frame {
                width,
                height,
                pixels,
            })
        }
        2 => {
            let root = if reader.bool()? {
                Some(reader.node()?)
            } else {
                None
            };
            FromRenderer::Tree(Box::new(Snapshot { root }))
        }
        3 => FromRenderer::Acted(reader.outcome()?),
        4 => FromRenderer::Refused(reader.refusal()?),
        5 => {
            let failure = match reader.tag()? {
                0 => Failure::NothingLoaded,
                1 => Failure::Unpaintable {
                    why: reader.text()?,
                },
                2 => Failure::NotAFont {
                    family: reader.text()?,
                },
                other => return Err(unreadable(format!("a failure tagged {other}"))),
            };
            FromRenderer::Failed(failure)
        }
        6 => FromRenderer::UsingFont {
            family: reader.text()?,
        },
        other => return Err(unreadable(format!("a message tagged {other}"))),
    };
    reader.finished()?;
    Ok(message)
}
