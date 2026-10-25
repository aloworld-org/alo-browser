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
//!
//! It holds the ring around the box an agent's verb last acted on (queue
//! item 624) for exactly as long as the frame it was measured against: a
//! new frame of the tab, or another tab chosen, takes it away, because its
//! rectangles are a layout's and the next frame may be another's
//! ([`crate::mark`]). A sentence about the tab leaves it, as it leaves the
//! frame.
//!
//! It holds the browser's own lines about agents: the sentence each verb an
//! agent asked for was answered with, whether it ran (queue item 623) or
//! was refused (625), for as long as [`AgentLines`] keeps them — through new
//! frames, other sentences and other tabs chosen — because they are about
//! what an agent did, not about what a tab shows, and a person who looked
//! away for a second still needs to read them.

use crate::agent_lines::{AgentLine, AgentLines};
use crate::compose::Scene;
use crate::mark::Marked;
use crate::message::News;
use crate::scale::Scale;
use alo_renderer::Frame;

/// The selected tab and the strip, as far as the window has been told.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Showing {
    frame: Option<Frame>,
    sentence: Option<String>,
    strip: Option<Frame>,
    strip_sentence: Option<String>,
    agent_lines: AgentLines,
    marked: Vec<Marked>,
}

impl Showing {
    /// Take in what the conductor said. Whether the window has to be drawn
    /// again because of it.
    ///
    /// A new frame clears the sentence, because a renderer that painted has
    /// answered; a sentence leaves the frame, because a tab keeps the last
    /// frame it painted (ADR 0005). The strip's are kept apart from the
    /// tab's, by the same rules.
    ///
    /// Another tab chosen replaces both the frame and the sentence: they were
    /// the tab left's, and the tab chosen is shown by its own last frame, or
    /// by the window's background when it has none (queue item 529).
    pub fn hear(&mut self, news: News) -> bool {
        match news {
            News::Painted(frame) => {
                self.frame = Some(frame);
                self.sentence = None;
                self.marked.clear();
                true
            }
            News::Said(sentence) => {
                self.sentence = Some(sentence);
                true
            }
            News::Chosen(frame) => {
                self.frame = frame;
                self.sentence = None;
                self.marked.clear();
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
            News::Marked(marked) => {
                self.marked = marked;
                true
            }
            News::AgentSaid(sentence) => {
                self.agent_lines.did(sentence);
                true
            }
            News::AgentRefused(sentence) => {
                self.agent_lines.refused(sentence);
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

    /// The sentences agents' verbs were answered with that the window still
    /// shows, oldest first.
    pub fn agent_lines(&self) -> &[AgentLine] {
        self.agent_lines.lines()
    }

    /// The rectangles of the box an agent's verb last acted on, over the
    /// frame shown, while that frame is shown.
    pub fn marked(&self) -> &[Marked] {
        &self.marked
    }

    /// What to compose a window of `window` device pixels from.
    pub fn scene(&self, window: (u32, u32), scale: Scale) -> Scene<'_> {
        Scene {
            window,
            scale,
            strip: self.strip(),
            strip_sentence: self.strip_sentence(),
            frame: self.frame(),
            sentence: self.sentence(),
            agent_lines: self.agent_lines(),
            marked: self.marked(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the window says of agents, sentence by sentence.
    fn told(showing: &Showing) -> Vec<&str> {
        showing
            .agent_lines()
            .iter()
            .map(|line| line.sentence.as_str())
            .collect()
    }

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
    fn another_tab_chosen_replaces_the_frame_and_the_sentence_and_not_the_strip() {
        let mut showing = Showing::default();
        showing.hear(News::Painted(a_frame(1)));
        showing.hear(News::Said("gone".to_owned()));
        showing.hear(News::Strip(a_frame(2)));
        assert!(showing.hear(News::Chosen(Some(a_frame(3)))));
        assert_eq!(showing.frame(), Some(&a_frame(3)));
        assert_eq!(showing.sentence(), None);
        assert_eq!(showing.strip(), Some(&a_frame(2)));
        // A tab that has painted nothing is the window's background.
        assert!(showing.hear(News::Chosen(None)));
        assert_eq!(showing.frame(), None);
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
        let scene = showing.scene((30, 20), Scale::TWO);
        assert_eq!(scene.window, (30, 20));
        assert_eq!(scene.scale, Scale::TWO);
        assert_eq!(scene.frame, Some(&a_frame(1)));
        assert_eq!(scene.sentence, None);
        assert_eq!((scene.strip, scene.strip_sentence), (None, None));
        assert!(scene.agent_lines.is_empty());
    }

    #[test]
    fn what_an_agent_did_is_kept_until_the_next_verb_whatever_else_arrives() {
        let mut showing = Showing::default();
        assert!(told(&showing).is_empty());
        assert!(showing.hear(News::AgentSaid("activated box#3".to_owned())));
        assert_eq!(told(&showing), ["activated box#3"]);
        // Nothing of the tab's or the strip's touches it, nor another tab.
        for news in [
            News::Painted(a_frame(1)),
            News::Said("gone".to_owned()),
            News::Strip(a_frame(2)),
            News::StripSaid("gone".to_owned()),
            News::Chosen(None),
        ] {
            showing.hear(news);
            assert_eq!(told(&showing), ["activated box#3"]);
        }
        // And it touches nothing of theirs.
        showing.hear(News::Painted(a_frame(4)));
        showing.hear(News::AgentSaid("scrolled box#1 to 0, 40".to_owned()));
        assert_eq!(showing.frame(), Some(&a_frame(4)));
        assert_eq!(showing.sentence(), None);
        assert_eq!(told(&showing), ["scrolled box#1 to 0, 40"]);
        let scene = showing.scene((30, 20), Scale::ONE);
        assert_eq!(scene.agent_lines, showing.agent_lines());
        assert!(!News::AgentSaid(String::new()).is_strip());
    }

    #[test]
    fn a_ring_lasts_as_long_as_the_frame_it_was_measured_against() {
        let ring = vec![Marked {
            x: 4,
            y: 6,
            width: 20,
            height: 10,
        }];
        let mut showing = Showing::default();
        assert!(showing.hear(News::Painted(a_frame(1))));
        assert!(showing.marked().is_empty());
        assert!(showing.hear(News::Marked(ring.clone())));
        assert_eq!(showing.marked(), ring.as_slice());
        assert_eq!(showing.scene((30, 20), Scale::ONE).marked, ring.as_slice());
        // What leaves the frame leaves the ring: a sentence about the tab,
        // the strip's news and the agent's line.
        for news in [
            News::Said("gone".to_owned()),
            News::Strip(a_frame(2)),
            News::StripSaid("gone".to_owned()),
            News::AgentSaid("activated box#3".to_owned()),
            News::AgentRefused("box#4 says it is disabled".to_owned()),
        ] {
            showing.hear(news);
            assert_eq!(showing.marked(), ring.as_slice());
        }
        // A new frame of the tab takes it away, and so does another tab.
        showing.hear(News::Painted(a_frame(3)));
        assert!(showing.marked().is_empty());
        showing.hear(News::Marked(ring.clone()));
        showing.hear(News::Chosen(Some(a_frame(4))));
        assert!(showing.marked().is_empty());
        assert_eq!(
            told(&showing),
            ["activated box#3", "box#4 says it is disabled"]
        );
        assert!(!News::Marked(ring).is_strip());
    }

    #[test]
    fn a_refusal_is_kept_beside_what_an_agent_did_whatever_else_arrives() {
        let mut showing = Showing::default();
        showing.hear(News::Painted(a_frame(1)));
        assert!(showing.hear(News::AgentSaid("activated box#3".to_owned())));
        assert!(showing.hear(News::AgentRefused(
            "nothing on the page is \"A pony\"".to_owned()
        )));
        assert!(showing.hear(News::AgentRefused("box#4 says it is disabled".to_owned())));
        for news in [
            News::Painted(a_frame(2)),
            News::Said("gone".to_owned()),
            News::Chosen(None),
        ] {
            showing.hear(news);
            assert_eq!(
                told(&showing),
                [
                    "activated box#3",
                    "nothing on the page is \"A pony\"",
                    "box#4 says it is disabled",
                ]
            );
        }
        let refused: Vec<bool> = showing.agent_lines().iter().map(|l| l.refused).collect();
        assert_eq!(refused, [false, true, true]);
        // A refusal touches nothing of the tab's.
        assert_eq!(showing.frame(), None);
        assert_eq!(showing.sentence(), None);
        assert!(!News::AgentRefused(String::new()).is_strip());
    }
}
