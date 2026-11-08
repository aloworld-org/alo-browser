/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What `backdrop-filter` asks for (queue item 885, ADR 0104).
//!
//! Filter Effects 2 lets `backdrop-filter` hold a list of filter functions,
//! applied in turn to what is drawn behind a box. This engine draws one of
//! them, **`blur()`** — what alo's own *Settings* stands over — and refuses
//! the others **by name** until a page needs one: a value naming
//! `brightness()` is read as valid CSS, so it is not mistaken for a typo,
//! and is said not drawn ([`BackdropFilter::Unbuilt`]). A list of more than
//! one function is refused the same way, since two blurs one after the
//! other are not one blur of their radii added.
//!
//! The blur's length is kept as written, `0.5em` included, as a shadow's is
//! ([`crate::shadow`]): it is turned into pixels where it is drawn.

use crate::length::Length;

/// A `backdrop-filter` value, as read.
#[derive(Debug, Clone, PartialEq)]
pub enum BackdropFilter {
    /// `none`: nothing is done to what is behind the box.
    None,
    /// `blur(<length>)`: what is behind the box blurred, the length the
    /// Gaussian's standard deviation (Filter Effects 1 § 13.2.1) — not half
    /// of it, as a shadow's blur radius is.
    Blur(Length),
    /// A value CSS allows that this engine does not draw — another filter
    /// function, or more than one — named by what was written, so a refusal
    /// can say which.
    Unbuilt(String),
}

/// The filter functions Filter Effects 1 § 13 names, and `url()`: a value
/// built of these is CSS, whether or not this engine draws it.
pub const FUNCTIONS: [&str; 11] = [
    "blur",
    "brightness",
    "contrast",
    "drop-shadow",
    "grayscale",
    "hue-rotate",
    "invert",
    "opacity",
    "saturate",
    "sepia",
    "url",
];

impl BackdropFilter {
    /// Whether it asks for anything to be done: anything but `none`. A box
    /// whose `backdrop-filter` is not `none` is a stacking context, as
    /// Filter Effects 2 § 3 says, whether or not this engine draws it.
    pub fn is_some(&self) -> bool {
        !matches!(self, BackdropFilter::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_none_asks_for_nothing() {
        assert!(!BackdropFilter::None.is_some());
        assert!(BackdropFilter::Blur(Length::ZERO).is_some());
        assert!(BackdropFilter::Unbuilt("sepia(1)".to_owned()).is_some());
    }

    #[test]
    fn the_functions_are_sorted_and_name_blur() {
        let mut sorted = FUNCTIONS;
        sorted.sort_unstable();
        assert_eq!(sorted, FUNCTIONS);
        assert!(FUNCTIONS.contains(&"blur"));
    }
}
