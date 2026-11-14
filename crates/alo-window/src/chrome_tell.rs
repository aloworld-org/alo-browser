/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What alo tells a windowless page's host beside the DevTools pipe (queue
//! item 979, ADR 0113): the view's size and scale, whether the card is
//! hidden, and whether the page has the focus — what a window would have
//! told Chromium, where a windowless page has none.
//!
//! The pipe carries only Chromium's protocol, and [`crate::chrome_told`]
//! reads what the host tells alo on descriptor 5; for a windowless host
//! that descriptor is a socket ([`crate::chrome::Chrome::start_windowless`])
//! and these lines go the other way on it. The host reads them as a
//! boundary (its `heard.rs`): a line is a word and one field after a tab,
//! at most [`LONGEST_LINE`] bytes with its end, at most [`MOST_A_SECOND`] a
//! second, and anything else — a size or scale out of its bounds included —
//! is refused there and nothing done. alo writes nothing else.

use crate::scale::Scale;

/// The longest line the host hears, its end included (the host's
/// `heard.rs`, `LONGEST_LINE`).
pub const LONGEST_LINE: usize = 64;

/// The most lines the host hears in any one second (`heard.rs`,
/// `MOST_A_SECOND`); past it each is refused.
pub const MOST_A_SECOND: usize = 60;

/// What alo tells a windowless page's host.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToHost {
    /// The view laid out at `size` CSS pixels, drawn at `scale`.
    View {
        /// In CSS pixels.
        size: (u32, u32),
        /// Device pixels to a CSS pixel.
        scale: Scale,
    },
    /// The card off the screen, or on it again.
    Hidden(bool),
    /// The page given the focus, or it taken away.
    Focused(bool),
}

impl ToHost {
    /// The line written for this, its end included.
    pub fn line(self) -> String {
        match self {
            Self::View {
                size: (width, height),
                scale,
            } => format!("view\t{width}x{height}@{}\n", scale.factor()),
            Self::Hidden(hidden) => format!("hidden\t{}\n", u8::from(hidden)),
            Self::Focused(focused) => format!("focus\t{}\n", u8::from(focused)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_is_a_word_and_one_field_as_the_host_reads_it() {
        assert_eq!(
            ToHost::View {
                size: (800, 500),
                scale: Scale::TWO
            }
            .line(),
            "view\t800x500@2\n"
        );
        assert_eq!(
            ToHost::View {
                size: (600, 400),
                scale: Scale::ONE_AND_A_HALF
            }
            .line(),
            "view\t600x400@1.5\n"
        );
        assert_eq!(ToHost::Hidden(true).line(), "hidden\t1\n");
        assert_eq!(ToHost::Focused(false).line(), "focus\t0\n");
    }

    #[test]
    fn the_longest_view_fits_a_line() {
        let longest = ToHost::View {
            size: (u32::MAX, u32::MAX),
            scale: Scale::ONE_AND_A_HALF,
        }
        .line();
        assert!(longest.len() <= LONGEST_LINE, "{longest:?}");
    }
}
