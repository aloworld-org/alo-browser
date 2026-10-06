/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What the window was last sent, which is all it ever shows.
//!
//! ADR 0024 § 2: *"The window shows what it was last sent. A frame, once it
//! arrives, is kept; a resize shows the old frame at its old size, against
//! the window's background, until the new one arrives. … Nothing is drawn as
//! though it were current that is not."*
//!
//! So this holds a frame and a sentence and changes only when [`News`]
//! arrives. A resize changes nothing here: the window's size is the window's,
//! and the frame stays the size it was painted until a new one replaces it.

use crate::compose::Scene;
use crate::message::News;
use alo_renderer::Frame;

/// The selected tab, as far as the window has been told.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Showing {
    frame: Option<Frame>,
    sentence: Option<String>,
}

impl Showing {
    /// Take in what the conductor said. Whether the window has to be drawn
    /// again because of it.
    ///
    /// A new frame clears the sentence, because a renderer that painted has
    /// answered; a sentence leaves the frame, because a tab keeps the last
    /// frame it painted (ADR 0005).
    pub fn hear(&mut self, news: News) -> bool {
        match news {
            News::Painted(frame) => {
                self.frame = Some(frame);
                self.sentence = None;
                true
            }
            News::Said(sentence) => {
                self.sentence = Some(sentence);
                true
            }
            News::Closed => false,
        }
    }

    /// The last frame, if one has arrived.
    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }

    /// What the window is saying about the tab, if anything.
    pub fn sentence(&self) -> Option<&str> {
        self.sentence.as_deref()
    }

    /// What to compose a window of `window` device pixels from.
    pub fn scene(&self, window: (u32, u32), replication: u32) -> Scene<'_> {
        Scene {
            window,
            replication,
            frame: self.frame(),
            sentence: self.sentence(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_frame(byte: u8) -> Frame {
        Frame {
            width: 1,
            height: 1,
            pixels: vec![byte, byte, byte, 255],
        }
    }

    #[test]
    fn a_frame_is_kept_until_another_replaces_it() {
        let mut showing = Showing::default();
        assert!(showing.hear(News::Painted(a_frame(1))));
        assert_eq!(showing.frame(), Some(&a_frame(1)));
        assert!(showing.hear(News::Painted(a_frame(2))));
        assert_eq!(showing.frame(), Some(&a_frame(2)));
    }

    #[test]
    fn a_sentence_leaves_the_frame_where_it_is() {
        let mut showing = Showing::default();
        showing.hear(News::Painted(a_frame(1)));
        assert!(showing.hear(News::Said("gone".to_owned())));
        assert_eq!(showing.frame(), Some(&a_frame(1)));
        assert_eq!(showing.sentence(), Some("gone"));
    }

    #[test]
    fn a_new_frame_means_the_renderer_answered_and_clears_the_sentence() {
        let mut showing = Showing::default();
        showing.hear(News::Said("gone".to_owned()));
        showing.hear(News::Painted(a_frame(1)));
        assert_eq!(showing.sentence(), None);
    }

    #[test]
    fn closing_changes_nothing_on_the_screen() {
        let mut showing = Showing::default();
        showing.hear(News::Painted(a_frame(1)));
        assert!(!showing.hear(News::Closed));
        assert_eq!(showing.frame(), Some(&a_frame(1)));
    }

    #[test]
    fn the_scene_is_what_was_sent_at_the_size_the_window_is() {
        let mut showing = Showing::default();
        showing.hear(News::Painted(a_frame(1)));
        let scene = showing.scene((30, 20), 2);
        assert_eq!(scene.window, (30, 20));
        assert_eq!(scene.replication, 2);
        assert_eq!(scene.frame, Some(&a_frame(1)));
        assert_eq!(scene.sentence, None);
    }
}
