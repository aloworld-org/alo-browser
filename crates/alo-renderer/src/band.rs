/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The band of the frame around Chromium's window: what the browser process
//! tells the strip's renderer, as data (queue item 991).
//!
//! The frame of ours above Chromium's window is drawn as alo OS's window band
//! — the page's title, then the application's name — and the title is a
//! stranger's string, so it is drawn where the tab strip's titles are: by a
//! sandboxed renderer, at the strip's own site, from data, the title a text
//! node (ADR 0024 § 4; [`crate::band_document`]). Never shaped in the browser
//! process, and never markup.
//!
//! The title and the name are laid out together here, in one line, because
//! only the layout that holds both can let the title give way first: the
//! browser process cannot measure a title it never shapes. So the browser
//! process says only how wide the band is, and the stylesheet does the rest
//! — the title cut with an ellipsis, the name kept whole after it, and the
//! name drawn once when the title already says it.

use crate::strip::longest;
use alo_layout::Size;
use core::fmt;

/// The application's name, as the band says it after the page's title.
pub const APPLICATION: &str = "alo browser";

/// The band, as the browser process sees it at one instant.
#[derive(Debug, Clone, PartialEq)]
pub struct Band {
    /// The page's title, as the page wrote it. A stranger's string: never
    /// markup, carried whole, drawn to
    /// [`crate::strip::LONGEST_TITLE`] characters, and cut by the
    /// stylesheet where it does not fit.
    pub title: String,
    /// Where the page is: what the band shows when its title is nothing.
    pub address: String,
    /// How big the band is drawn, in CSS pixels.
    pub viewport: Size,
    /// The scale of the screen the frame is on, which the band is laid out
    /// and drawn at: any finite factor above zero, and anything else is one.
    pub scale: f32,
}

impl Band {
    /// The band for a page called `title` at `address`, `viewport` big, on a
    /// screen of one (the caller says another with [`Band::at_scale`]).
    pub fn new(title: impl Into<String>, address: impl Into<String>, viewport: Size) -> Self {
        Self {
            title: title.into(),
            address: address.into(),
            viewport,
            scale: 1.0,
        }
    }

    /// The same band, drawn for a screen of `scale` device pixels to the
    /// CSS pixel. Anything that is not a finite factor above zero is one.
    #[must_use]
    pub fn at_scale(mut self, scale: f32) -> Self {
        self.scale = alo_value::device_scale(scale);
        self
    }

    /// What the band shows of the page: its title, or its address when the
    /// title is nothing but white space — at most
    /// [`crate::strip::LONGEST_TITLE`] characters of either, as a tab does.
    pub fn label(&self) -> &str {
        let whole = if self.title.trim().is_empty() {
            &self.address
        } else {
            &self.title
        };
        longest(whole)
    }

    /// Whether the application's name is drawn after the label: not when
    /// the label already is the name, which the band then says once.
    pub fn names_the_application(&self) -> bool {
        self.label().trim() != APPLICATION
    }
}

impl fmt::Display for Band {
    /// What a renderer is asked, in words, as every message is said: the
    /// band's size, never its title.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "hold the frame's band, at {}×{}",
            self.viewport.width, self.viewport.height
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strip::LONGEST_TITLE;

    fn band(title: &str) -> Band {
        Band::new(title, "https://example.com/", Size::new(600.0, 54.0))
    }

    #[test]
    fn a_band_with_no_title_shows_its_address() {
        assert_eq!(band(" \t").label(), "https://example.com/");
        assert_eq!(band("Invoices").label(), "Invoices");
    }

    #[test]
    fn a_long_title_is_kept_to_the_longest_as_a_tabs_is() {
        let long = band(&"é".repeat(LONGEST_TITLE + 3));
        assert_eq!(long.label().chars().count(), LONGEST_TITLE);
    }

    #[test]
    fn the_name_is_said_once_when_the_title_is_the_name() {
        assert!(band("Invoices").names_the_application());
        assert!(!band(APPLICATION).names_the_application());
        assert!(!band(" alo browser ").names_the_application());
        assert!(band("alo").names_the_application());
    }

    #[test]
    fn a_scale_that_is_no_scale_is_one() {
        assert!((band("a").at_scale(f32::NAN).scale - 1.0).abs() < f32::EPSILON);
        assert!((band("a").at_scale(2.0).scale - 2.0).abs() < f32::EPSILON);
    }
}
