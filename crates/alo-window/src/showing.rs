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
//! So this holds a frame and a sentence for the selected tab, and the same
//! for the tab strip (queue item 528), and changes only when [`News`]
//! arrives. A resize changes nothing here: the window's size is the window's,
//! and each frame stays the size it was painted until a new one replaces it.

use crate::compose::Scene;
use crate::message::News;
use alo_renderer::Frame;

/// The selected tab and the strip, as far as the window has been told.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Showing {
    frame: Option<Frame>,
    sentence: Option<String>,
    strip: Option<Frame>,
    strip_sentence: Option<String>,
}

impl Showing {
    /// Take in what the conductor said. Whether the window has to be drawn
    /// again because of it.
    ///
    /// A new frame clears the sentence, because a renderer that painted has
    /// answered; a sentence leaves the frame, because a tab keeps the last
    /// frame it painted (ADR 0005). The strip's are kept apart from the
    /// tab's, by the same rules.
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
            News::Strip(frame) => {
                self.strip = Some(frame);
                self.strip_sentence = None;
                true
            }
            News::StripSaid(sentence) => {
                self.strip_sentence = Some(sentence);
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

    /// The strip's last frame, if one has arrived.
    pub fn strip(&self) -> Option<&Frame> {
        self.strip.as_ref()
    }

    /// What the window is saying about the strip, if anything.
    pub fn strip_sentence(&self) -> Option<&str> {
        self.strip_sentence.as_deref()
    }

    /// What to compose a window of `window` device pixels from.
    pub fn scene(&self, window: (u32, u32), replication: u32) -> Scene<'_> {
        Scene {
            window,
            replication,
            strip: self.strip(),
            strip_sentence: self.strip_sentence(),
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
    fn the_strip_is_kept_apart_from_the_tab() {
        let mut showing = Showing::default();
        showing.hear(News::Painted(a_frame(1)));
        assert!(showing.hear(News::Strip(a_frame(2))));
        assert_eq!(showing.frame(), Some(&a_frame(1)));
        assert_eq!(showing.strip(), Some(&a_frame(2)));
        // The strip's sentence leaves its frame, and the tab's alone.
        assert!(showing.hear(News::StripSaid("gone".to_owned())));
        assert_eq!(showing.strip(), Some(&a_frame(2)));
        assert_eq!(showing.strip_sentence(), Some("gone"));
        assert_eq!(showing.sentence(), None);
        // A new strip frame clears its sentence and nothing of the tab's.
        showing.hear(News::Said("tab".to_owned()));
        showing.hear(News::Strip(a_frame(3)));
        assert_eq!(showing.strip_sentence(), None);
        assert_eq!(showing.sentence(), Some("tab"));
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
        assert_eq!((scene.strip, scene.strip_sentence), (None, None));
    }
}
