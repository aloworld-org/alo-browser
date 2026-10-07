/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A background: a list of layers, and a colour beneath them all.
//!
//! `background: radial-gradient(…), var(--bg-app)` is two layers, and the
//! last one's colour is painted first, under every image. The images are
//! listed **first on top**, as the author wrote them, so a painter walks the
//! list backwards.
//!
//! # What a layer may hold
//!
//! An image — `none`, a gradient [`crate::parse_gradient`] reads, or a
//! `url()` — and, in the last layer only, a colour. Nothing else: a layer
//! that also says where its image sits, how big it is, whether it repeats or
//! which box it is clipped to is refused whole, because every one of those
//! moves pixels and a layer drawn without them is a wrong picture that looks
//! nearly right.
//!
//! A `url()` is read and kept rather than refused. The list around it is
//! valid CSS that this engine can draw all of except that one picture
//! (queue item 311), and saying "this layer was not drawn" is truer than
//! saying "this background could not be read".
//!
//! The text is read in [`crate::parse`], where the tokeniser is.

use crate::color::Color;
use crate::gradient::Gradient;

/// One layer's picture.
#[derive(Debug, Clone, PartialEq)]
pub enum Image {
    /// `none`: a layer with nothing in it, which is still a layer.
    None,
    /// A gradient.
    Gradient(Gradient),
    /// A picture fetched from somewhere, by the address as written.
    Url(String),
}

/// A whole `background`, or as much of one as a longhand says.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Background {
    /// The layers' pictures, first on top.
    pub images: Vec<Image>,
    /// The colour beneath them, if one was written.
    pub color: Option<Color>,
}
