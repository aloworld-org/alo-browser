/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A line of the record that waits for Chrome to say which document asked
//! (queue items 647 and 656).
//!
//! # Chrome tells of a page's requests before it tells of the page
//!
//! A frame going to a new document holds two: the one it holds, and the one
//! arriving, minted at the navigation's first hop
//! ([`crate::chrome_record::Causes`]). Until the frame's commit is read, a
//! request paused for the frame was taken as the document it holds — and
//! that was wrong. **Chrome pauses a request in the browser process and
//! tells of a commit from the renderer**, through two channels, so the new
//! document's stylesheet can be paused before its commit is read. Measured
//! against Chrome 155 on 25 October 2026, a page following a link to a page
//! with two sheets, a script, a picture and a `fetch`: never on a quiet
//! machine in 50 runs, and 10 such requests in 100 runs under 96 busy
//! processes. Each was pinned on the old page, and on the agent's action in
//! it, which is how item 647 stopped the loop three times.
//!
//! So the cause cannot be read from which document is the frame's now. It
//! is read from **the request's own loader**, which the renderer that made
//! it names in `Network.requestWillBeSent` — the same event's `requestId` is
//! a paused request's `networkId`, and a navigation's `networkId` is the
//! loader of the document it commits, through every redirect (measured, 126
//! of 126 redirects). A request paused in that window is let go at once —
//! its rule does not depend on its cause — and its line is **owed** here,
//! with the cause it would have had, until either:
//!
//! - **its loader is told of**: the arriving document's, and the line is that
//!   document's; any other, and it is the cause it was kept with; or
//! - **the frame's renderer has answered after everything it told of**:
//!   once the commit is read, `Network.enable` is sent again through the
//!   session it came by, and its reply comes after the `requestWillBeSent`
//!   of every request that renderer made before it (10 of 10 under load).
//!   A line still owed then was never told of — an old page's request as it
//!   was left, which Chrome 155 often never tells of — and is the cause it
//!   was kept with.
//!
//! # A loader is a document Chrome names, never a cause
//!
//! ADR 0060 § 2: nothing Chrome or a page says assigns a cause. A loader is
//! Chrome naming which document asked, as `frameId` names which frame does;
//! it chooses between the two documents this process minted for the frame,
//! and a loader that is neither's is the one the line was kept with. A
//! renderer that lied could move its request from the old page to the
//! arriving one — whose cause, through the chain, is the same navigation —
//! and never to a person or an action of its choosing.
//!
//! # A request Chrome names no id for is the page being left
//!
//! None of this is owed for a request paused with no `networkId`: Chrome
//! names none only for a page it no longer tells of, one being left, and
//! never tells of a loader for it. Before the frame's commit is read that
//! is the document the frame holds; after, it is the one the frame left —
//! the same race the other way round, which a page from another site
//! loses every time ([`crate::chrome_record::Causes::of_unnamed`], queue
//! item 657).

use alo_net::activity::Happened;
use alo_net::cause::{Cause, DocumentId};
use alo_net::request::Purpose;
use alo_url::Url;
use std::collections::HashMap;

/// The most lines owed at once. A line is owed only while its frame goes to
/// a new document, and a page that asks for more in that window — a timer
/// firing every millisecond while the next page is slow to arrive — has
/// the rest written at once with the cause each was kept with, which is
/// that page's.
pub const MOST_OWED: usize = 256;

/// A line of the record whose cause waits on its loader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// What was asked for.
    pub url: Url,
    /// Its method, as Chrome spells it.
    pub method: String,
    /// What kind of thing it is.
    pub purpose: Purpose,
    /// What became of it.
    pub happened: Happened,
    /// The cause it had when it was paused: the document the frame held.
    pub kept: Cause,
    /// The frame it was paused for, by Chrome's id.
    pub frame: String,
    /// The document arriving in that frame as it was paused.
    pub arriving: DocumentId,
    /// That document's loader, by Chrome's id.
    pub loader: String,
}

/// Every line owed, and the frames whose renderers were asked to answer
/// after everything they told of.
#[derive(Debug, Default)]
pub struct Owed {
    /// By the paused request's `networkId`.
    lines: HashMap<String, Line>,
    /// The command asking each frame's renderer, by its id: the frame, and
    /// the requests owed for it when it was sent.
    asked: HashMap<u64, (String, Vec<String>)>,
}

impl Owed {
    /// Whether any line is owed.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Owe `line`, for the request Chrome calls `network` — or hand it
    /// back, to be written now with the cause it was kept with, when
    /// [`MOST_OWED`] are owed already or `network` is.
    pub fn owe(&mut self, network: &str, line: Line) -> Option<Line> {
        if self.lines.len() >= MOST_OWED || self.lines.contains_key(network) {
            return Some(line);
        }
        self.lines.insert(network.to_owned(), line);
        None
    }

    /// Chrome told of the request `network` as made by the document loaded
    /// by `loader`: its line, with its cause, when one is owed.
    pub fn told(&mut self, network: &str, loader: &str) -> Option<(Line, Cause)> {
        let line = self.lines.remove(network)?;
        let cause = if line.loader == loader {
            Cause::Document {
                document: line.arriving,
            }
        } else {
            line.kept.clone()
        };
        Some((line, cause))
    }

    /// Whether a line is owed for the frame `frame`.
    pub fn waits_on(&self, frame: &str) -> bool {
        self.lines.values().any(|line| line.frame == frame)
    }

    /// The command `asked` was sent to the renderer of the frame `frame`,
    /// once its commit was read: the lines owed for the frame now are
    /// written when it answers.
    pub fn asked(&mut self, asked: u64, frame: &str) {
        let owed = self
            .lines
            .iter()
            .filter(|(_, line)| line.frame == frame)
            .map(|(network, _)| network.clone())
            .collect();
        self.asked.insert(asked, (frame.to_owned(), owed));
    }

    /// Whether `id` is a command [`Owed::asked`] was told of.
    pub fn awaits(&self, id: u64) -> bool {
        self.asked.contains_key(&id)
    }

    /// The renderer answered `id`, refusing or not: every line owed when it
    /// was asked that is owed still was never told of, and is handed back
    /// with the cause it was kept with.
    pub fn answered(&mut self, id: u64) -> Vec<Line> {
        let Some((_, owed)) = self.asked.remove(&id) else {
            return Vec::new();
        };
        owed.iter()
            .filter_map(|network| self.lines.remove(network))
            .collect()
    }

    /// The navigation of `frame` that would have committed `arriving` went
    /// no further: no document of it asked for anything, so every line owed
    /// on it is handed back with the cause it was kept with.
    pub fn ended(&mut self, frame: &str, arriving: DocumentId) -> Vec<Line> {
        self.take(|line| line.frame == frame && line.arriving == arriving)
    }

    /// The lines owed for every frame `gone` says is gone, handed back with
    /// the cause each was kept with: no renderer will be asked about it.
    pub fn forgotten(&mut self, gone: impl Fn(&str) -> bool) -> Vec<Line> {
        self.take(|line| gone(&line.frame))
    }

    /// Every line owed, handed back with the cause each was kept with: the
    /// tab is closing.
    pub fn all(&mut self) -> Vec<Line> {
        self.asked.clear();
        self.lines.drain().map(|(_, line)| line).collect()
    }

    /// Every line owed for which `which` holds, taken.
    fn take(&mut self, which: impl Fn(&Line) -> bool) -> Vec<Line> {
        let networks: Vec<String> = self
            .lines
            .iter()
            .filter(|(_, line)| which(line))
            .map(|(network, _)| network.clone())
            .collect();
        networks
            .iter()
            .filter_map(|network| self.lines.remove(network))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_net::cause::{ActionId, Identities, Place};

    struct Two {
        old: DocumentId,
        new: DocumentId,
        action: ActionId,
    }

    fn two() -> Two {
        let mut identities = Identities::default();
        Two {
            old: identities.a_document(),
            new: identities.a_document(),
            action: identities.an_action(),
        }
    }

    fn line(two: &Two, frame: &str, url: &str) -> Line {
        Line {
            url: alo_url::parse(url).unwrap(),
            method: "GET".to_owned(),
            purpose: Purpose::Style,
            happened: Happened::LetGo,
            kept: Cause::Agent {
                action: two.action,
                place: Place::Document(two.old),
            },
            frame: frame.to_owned(),
            arriving: two.new,
            loader: "NEW".to_owned(),
        }
    }

    /// Item 647's order, read from the gate's failure and reproduced under
    /// load: the new page's sheet paused, then the commit, then the sheet
    /// told of by the new page's loader.
    #[test]
    fn a_request_told_of_by_the_arriving_loader_is_the_arriving_documents() {
        let two = two();
        let mut owed = Owed::default();
        assert_eq!(owed.owe("1.7", line(&two, "T", "http://a/next.css")), None);
        assert!(owed.waits_on("T") && !owed.waits_on("F"));
        let (told, cause) = owed.told("1.7", "NEW").unwrap();
        assert_eq!(told.url.to_string(), "http://a/next.css");
        assert_eq!(cause, Cause::Document { document: two.new });
        assert!(owed.is_empty());
        assert_eq!(owed.told("1.7", "NEW"), None, "told of once");
    }

    #[test]
    fn a_request_told_of_by_another_loader_keeps_its_cause() {
        let two = two();
        let mut owed = Owed::default();
        let kept = line(&two, "T", "http://a/tick");
        assert_eq!(owed.owe("1.8", kept.clone()), None);
        // A dedicated worker's request is told of with no loader at all.
        assert_eq!(
            owed.told("1.8", ""),
            Some((kept.clone(), kept.kept.clone()))
        );
        assert_eq!(owed.owe("1.9", kept.clone()), None);
        assert_eq!(
            owed.told("1.9", "OLD").map(|(_, cause)| cause),
            Some(kept.kept)
        );
    }

    #[test]
    fn what_was_never_told_of_keeps_its_cause_once_the_renderer_answers() {
        let two = two();
        let mut owed = Owed::default();
        assert_eq!(owed.owe("1.1", line(&two, "T", "http://a/tick")), None);
        assert_eq!(owed.owe("1.2", line(&two, "T", "http://a/next.css")), None);
        assert_eq!(owed.owe("2.1", line(&two, "F", "http://b/f")), None);
        owed.asked(40, "T");
        assert!(owed.awaits(40) && !owed.awaits(41));
        // Told of before the answer: decided by its loader.
        assert!(owed.told("1.2", "NEW").is_some());
        // Owed after the command was sent: not the command's to settle.
        assert_eq!(owed.owe("1.3", line(&two, "T", "http://a/later")), None);
        let settled = owed.answered(40);
        assert_eq!(settled.len(), 1);
        assert_eq!(settled[0].url.to_string(), "http://a/tick");
        assert!(owed.waits_on("F") && owed.waits_on("T"));
        assert_eq!(owed.answered(40), Vec::new(), "answered once");
    }

    #[test]
    fn no_more_are_owed_than_the_bound_and_none_twice() {
        let two = two();
        let mut owed = Owed::default();
        for request in 0..MOST_OWED {
            assert_eq!(
                owed.owe(&request.to_string(), line(&two, "T", "http://a/t")),
                None
            );
        }
        let over = line(&two, "T", "http://a/over");
        assert_eq!(owed.owe("over", over.clone()), Some(over.clone()));
        let mut once = Owed::default();
        assert_eq!(once.owe("x", over.clone()), None);
        assert_eq!(once.owe("x", over.clone()), Some(over));
    }

    #[test]
    fn an_ended_navigation_a_forgotten_frame_and_a_closing_tab_settle_what_is_owed() {
        let two = two();
        let mut owed = Owed::default();
        assert_eq!(owed.owe("1", line(&two, "T", "http://a/1")), None);
        assert_eq!(owed.owe("2", line(&two, "F", "http://b/2")), None);
        assert_eq!(owed.owe("3", line(&two, "G", "http://c/3")), None);
        assert_eq!(owed.ended("T", two.old), Vec::new(), "not that navigation");
        assert_eq!(owed.ended("T", two.new).len(), 1);
        assert_eq!(owed.forgotten(|frame| frame == "F").len(), 1);
        owed.asked(9, "G");
        assert_eq!(owed.all().len(), 1);
        assert!(owed.is_empty() && !owed.awaits(9));
    }
}
