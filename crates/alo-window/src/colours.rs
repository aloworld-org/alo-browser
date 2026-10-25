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

/// Behind the tab strip until its renderer has painted, and wherever its
/// frame does not reach: `--bg-rail`, the strip's own ground, so the band
/// is the strip's colour from the window's first moment.
pub fn rail() -> Rgba {
    Rgba::from_rgba8(0x1f, 0x25, 0x29, 0xff)
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

/// The ground of the line saying what an agent last did (queue item 623):
/// `--accent-soft`, the quiet terracotta ground alo gives what an AI wrote.
/// Not the warning's, because an agent doing what it was asked is not
/// something gone wrong.
pub fn agent_ground() -> Rgba {
    Rgba::from_rgba8(0xfc, 0xea, 0xe3, 0xff)
}

/// Its words: `--text-primary`, alo's navy.
pub fn agent_ink() -> Rgba {
    Rgba::from_rgba8(0x10, 0x2a, 0x43, 0xff)
}

/// The ground of a line saying why an agent's verb was refused (queue item
/// 625): `--danger-tint`. Not the agent's own ground, so a person can tell
/// at a glance that nothing was done, and not the warning's, because the
/// browser refusing a verb is not something gone wrong with a tab.
pub fn refused_ground() -> Rgba {
    Rgba::from_rgba8(0xfb, 0xe3, 0xe3, 0xff)
}

/// Its words: `--danger`, which alo's design system uses as both a danger
/// badge's mark and its ink on `--danger-tint`.
pub fn refused_ink() -> Rgba {
    Rgba::from_rgba8(0xdc, 0x26, 0x26, 0xff)
}

/// The ring around the box an agent's verb acted on (queue item 624):
/// `--accent`, alo's base terracotta, in which every control of its design
/// system draws its focus outline — so the mark reads as alo saying *this
/// one*, and never as a page's own border.
pub fn mark() -> Rgba {
    Rgba::from_rgba8(0xe7, 0x6f, 0x51, 0xff)
}

/// The three colours one of the browser's own lines is drawn in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineColours {
    /// Behind the words.
    pub ground: Rgba,
    /// The words.
    pub ink: Rgba,
    /// The rule along its top, between it and the page.
    pub rule: Rgba,
}

/// The line saying what happened to a tab or the strip.
pub fn notice_line() -> LineColours {
    LineColours {
        ground: notice_ground(),
        ink: notice_ink(),
        rule: notice_rule(),
    }
}

/// The line saying what an agent last did.
pub fn agent_line() -> LineColours {
    LineColours {
        ground: agent_ground(),
        ink: agent_ink(),
        rule: notice_rule(),
    }
}

/// The line saying why an agent's verb was refused.
pub fn refused_line() -> LineColours {
    LineColours {
        ground: refused_ground(),
        ink: refused_ink(),
        rule: notice_rule(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colour_is_solid() {
        for colour in [
            background(),
            rail(),
            notice_ground(),
            notice_ink(),
            notice_rule(),
            agent_ground(),
            agent_ink(),
            refused_ground(),
            refused_ink(),
            mark(),
        ] {
            assert!((colour.alpha - 1.0).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn the_words_are_darker_than_their_ground() {
        for line in [notice_line(), agent_line(), refused_line()] {
            let (ink, ground) = (line.ink.to_rgba8(), line.ground.to_rgba8());
            assert!(ink.0 < ground.0 && ink.1 < ground.1 && ink.2 < ground.2);
        }
    }

    #[test]
    fn an_agents_line_is_not_a_warnings() {
        assert_ne!(agent_line().ground, notice_line().ground);
        assert_ne!(agent_line().ink, notice_line().ink);
    }

    #[test]
    fn a_refusals_line_is_neither_an_agents_nor_a_warnings() {
        for other in [agent_line(), notice_line()] {
            assert_ne!(refused_line().ground, other.ground);
            assert_ne!(refused_line().ink, other.ink);
        }
    }
}
