/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A font, as it crosses the boundary.
//!
//! # Why a font is a message rather than a path
//!
//! ADR 0010 confines a renderer, and a confined renderer cannot open a font
//! file. There were two ways out of that and the ADR chose one:
//!
//! - permit the font directories in the sandbox policy, or
//! - **have the browser process read them and hand over the bytes.**
//!
//! The first is easier and it is the one to resist. It puts a filesystem path
//! into a security policy for one kind of resource, and the next kind — images,
//! then whatever follows — arrives with the same argument and no way to refuse
//! it. One rule that holds for everything beats a policy that grows a hole per
//! resource type.
//!
//! So a font is bytes on a pipe, and this is what those bytes are wrapped in.

use alo_text::{Slant, Weight};

/// The most bytes one font may be.
///
/// Some system font collections are tens of megabytes; a bound because a
/// browser process reading a directory should not be able to hand a renderer
/// something the renderer then has to hold.
pub const LARGEST_FONT: usize = 16 * 1024 * 1024;

/// The most fonts handed to one renderer.
pub const MOST_FONTS: usize = 24;

/// One font, as bytes and what to file them under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Face {
    /// The family the browser process believes this is.
    ///
    /// A belief rather than a fact — it usually comes from a filename — which
    /// is why [`crate::message::FromRenderer::UsingFont`] says what the family
    /// turned out to be rather than echoing this back.
    pub family: String,
    /// How heavy.
    pub weight: u16,
    /// Upright or slanted.
    pub slant: Slant,
    /// The font file itself.
    pub bytes: Vec<u8>,
}

impl Face {
    /// A face, if the bytes are within what this engine will carry.
    pub fn new(
        family: impl Into<String>,
        weight: Weight,
        slant: Slant,
        bytes: Vec<u8>,
    ) -> Option<Self> {
        if bytes.is_empty() || bytes.len() > LARGEST_FONT {
            return None;
        }
        Some(Self {
            family: family.into(),
            weight: weight.value(),
            slant,
            bytes,
        })
    }

    /// The weight as the text engine wants it.
    pub fn weight(&self) -> Weight {
        Weight::new(self.weight)
    }
}
