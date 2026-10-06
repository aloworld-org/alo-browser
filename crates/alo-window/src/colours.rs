/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The window's own colours, which are alo's.
//!
//! Each is a token from `alo-workplace`'s `web/src/ds/tokens.css` — the
//! specification for what alo looks like (`QUEUE.md`, *Before the first
//! item*) — named here by the token it is. That file is read, never written.
//! A colour of the window's that is not one of alo's would be the browser
//! inventing a design beside the one it renders.

use alo_value::Rgba;

/// Behind everything a page has not covered: `--bg-app`, alo's warm porcelain
/// canvas. What a window shows before its first frame, and around a frame
/// smaller than it.
pub fn background() -> Rgba {
    Rgba::from_rgba8(0xf4, 0xf1, 0xec, 0xff)
}

/// The ground of the line saying what happened to a tab: `--warning-tint`.
pub fn notice_ground() -> Rgba {
    Rgba::from_rgba8(0xfd, 0xf0, 0xd8, 0xff)
}

/// Its words: `--warning-ink`.
pub fn notice_ink() -> Rgba {
    Rgba::from_rgba8(0x8a, 0x5a, 0x08, 0xff)
}

/// The rule between it and the page: `--border-default`.
pub fn notice_rule() -> Rgba {
    Rgba::from_rgba8(0xde, 0xd7, 0xcd, 0xff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colour_is_solid() {
        for colour in [background(), notice_ground(), notice_ink(), notice_rule()] {
            assert!((colour.alpha - 1.0).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn the_words_are_darker_than_their_ground() {
        let (ink, ground) = (notice_ink().to_rgba8(), notice_ground().to_rgba8());
        assert!(ink.0 < ground.0 && ink.1 < ground.1 && ink.2 < ground.2);
    }
}
