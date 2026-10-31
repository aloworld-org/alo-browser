/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The chats panel and the first screen, each drawn by a confined renderer
//! of its own on a thread of the browser process (ADR 0074, queue item 709).
//!
//! Both are documents of ours built from state, as the tab strip is (ADR
//! 0024 § 4): a chat's name and a site's are strangers' strings, and the
//! browser process never shapes one. So the window hands the state it wants
//! drawn ([`Asked`]) to a thread here, which sends each part to its own
//! renderer — the panel at a site of its own, the first screen at another,
//! each minted as the strip's is — asks it to paint, and tells the window
//! the frame it painted, or why it painted none ([`Drawn`]).
//!
//! # Never waited on
//!
//! ADR 0024 § 2: the event loop never calls a renderer. The window only
//! sends here, and hears what was painted later, as an event like any
//! other. What is asked faster than it is painted is not queued: the thread
//! draws the newest it was sent, and a part that is the same as the last it
//! drew is not drawn again.
//!
//! # A renderer that went keeps its last frame
//!
//! As the strip's does: a part whose renderer died, or said nothing for as
//! long as any renderer is given, is not asked again. The window keeps the
//! last frame it painted and is told what happened, once, in the words
//! `alo-renderer` says it in; no fresh process draws it behind the person's
//! back (ADR 0005).
//!
//! # A person's press
//!
//! Queue item 710, ADR 0024 § 5. The window decides from its own
//! composition which part a press is over ([`crate::conversation_aim`]) and
//! hands it here, in that part's CSS pixels ([`ConversationDrawing::press`]).
//! The thread sends it to the part's renderer, which claims what it was on
//! by its place or its kind, and decides the claim (`alo-renderer`'s
//! `conversation_decide`) against **what that part showed when it last
//! painted** — the picture the person pressed — and what the browser
//! process holds now, the last it asked to be drawn. The window is told
//! what was decided ([`Drawn::Pressed`]) and acts on it
//! ([`crate::conversation::Conversation::act`]).
//!
//! Presses are never dropped or merged, and each is decided after every
//! drawing asked before it. A press on a part whose renderer had already
//! gone is not sent, and its line is already said; one whose renderer goes
//! as it is asked is said as a drawing's would be.

use crate::conversation::{Pressed, Showing};
use alo_paint::Point;
use alo_renderer::conversation_decide::{self, decide};
use alo_renderer::host::{Gone, Renderers};
use alo_renderer::panel::{self, Panel};
use alo_renderer::site::Site;
use alo_renderer::start::{self, Start};
use alo_renderer::strip_decide::unasked;
use alo_renderer::{Frame, FromRenderer, ToRenderer};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

/// Which part of the conversation's window: the chats panel or the first
/// screen, as `alo-renderer` names them.
pub use alo_renderer::conversation_claim::Part;

/// What the window is told of a part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Drawn {
    /// Its renderer painted it: in device pixels at the window's scale.
    Painted(Part, Frame),
    /// Why its renderer painted nothing, in this browser's words.
    Said(Part, String),
    /// What a person's press on it was decided to be.
    Pressed(Part, Pressed),
}

/// How the thread tells the window what was drawn: whether anybody heard.
pub type TellConversation = Box<dyn Fn(Drawn) -> bool + Send>;

/// What the window asks to be drawn: both parts, as they are now.
#[derive(Debug, Clone, PartialEq)]
pub struct Asked {
    /// The chats panel.
    pub panel: Panel,
    /// The first screen.
    pub start: Start,
    /// What the two show, as the browser process names each thing in them:
    /// what a press is decided against once they are painted. Never sent
    /// to a renderer.
    pub showing: Showing,
}

/// What the window hands the thread.
#[derive(Debug, Clone, PartialEq)]
enum Work {
    /// Draw both parts as they are now — boxed, since what is typed and
    /// offered made it the one large kind of work.
    Draw(Box<Asked>),
    /// A person pressed this part here, in its CSS pixels.
    Press(Part, Point),
}

/// A thread holding the panel's renderer and the first screen's.
#[derive(Debug)]
pub struct ConversationDrawing {
    /// Where work is sent; [`None`] once finished.
    asked: Option<Sender<Work>>,
    /// What was last sent, so the same is not sent twice.
    sent: Option<Asked>,
    thread: Option<JoinHandle<()>>,
}

impl ConversationDrawing {
    /// Start a thread that draws what it is asked in renderers of
    /// `renderers`', one for each part at a site of its own, and tells the
    /// window what came of it with `tell`.
    ///
    /// # Errors
    ///
    /// What the system said when no thread could be started.
    pub fn start(renderers: Renderers, tell: TellConversation) -> std::io::Result<Self> {
        let (asked, asking) = channel();
        let holding = Holding {
            renderers,
            panel: Held::at(panel::site()),
            start: Held::at(start::site()),
            drawn: None,
            painted: Showing::default(),
        };
        let thread = std::thread::Builder::new()
            .name("alo-conversation".to_owned())
            .spawn(move || hold(holding, &asking, &tell))?;
        Ok(Self {
            asked: Some(asked),
            sent: None,
            thread: Some(thread),
        })
    }

    /// Have `asked` drawn, unless it is what was last sent. Returns at once:
    /// what was painted is told to the window.
    pub fn draw(&mut self, asked: Asked) {
        if self.sent.as_ref() == Some(&asked) {
            return;
        }
        if let Some(sender) = &self.asked {
            // A thread that has gone is a window that is closing, which
            // nothing here waits for.
            let _ = sender.send(Work::Draw(Box::new(asked.clone())));
        }
        self.sent = Some(asked);
    }

    /// A person pressed `part` at `at`, in its CSS pixels: have it claimed
    /// and decided. Returns at once: what was decided is told to the window.
    pub fn press(&mut self, part: Part, at: Point) {
        if let Some(sender) = &self.asked {
            // As for a drawing: a thread that has gone is a window closing.
            let _ = sender.send(Work::Press(part, at));
        }
    }

    /// Stop the thread and wait for it, its renderers stopped with it:
    /// whether it ended rather than panicked, which the lints make
    /// unreachable and is said rather than assumed.
    ///
    /// Called as the window's event loop ends, never while it runs — a
    /// renderer in the middle of painting is waited for.
    pub fn finish(mut self) -> bool {
        self.asked = None;
        self.thread
            .take()
            .is_none_or(|thread| thread.join().is_ok())
    }
}

/// One part's renderer, as the thread keeps it.
struct Held {
    /// Its site, which no page can name.
    site: Site,
    /// What happened to it, once it has gone.
    gone: Option<Gone>,
}

impl Held {
    fn at(site: Site) -> Self {
        Self { site, gone: None }
    }
}

/// The thread's state: the renderers, each part's, and what it last drew.
struct Holding {
    renderers: Renderers,
    panel: Held,
    start: Held,
    /// What was last drawn, whatever came of it: a part that could not be
    /// drawn is not asked again until it is a different part. What it
    /// showed is what the browser process holds now.
    drawn: Option<Asked>,
    /// What each part showed when it last painted: the panel's chats, the
    /// first screen's sites, waiting chats and whether it showed a card.
    painted: Showing,
}

/// Do the work `asking` sends, in order — of the drawings waiting, only one
/// that no later drawing replaces — telling the window what came of each,
/// until nobody sends any more or the window has gone. The renderers are
/// stopped as `holding` goes.
fn hold(mut holding: Holding, asking: &Receiver<Work>, tell: &TellConversation) {
    while let Ok(first) = asking.recv() {
        let waiting: Vec<Work> = core::iter::once(first).chain(asking.try_iter()).collect();
        let last_drawing = waiting
            .iter()
            .rposition(|work| matches!(work, Work::Draw(_)));
        for (place, work) in waiting.into_iter().enumerate() {
            let said = match work {
                Work::Draw(asked) if Some(place) == last_drawing => holding.draw(*asked),
                Work::Draw(_) => Vec::new(),
                Work::Press(part, at) => holding.press(part, at).into_iter().collect(),
            };
            for drawn in said {
                if !tell(drawn) {
                    return;
                }
            }
        }
    }
}

impl Holding {
    /// Draw each part of `asked` that differs from the last drawn, in its
    /// own renderer, and say what came of each.
    fn draw(&mut self, asked: Asked) -> Vec<Drawn> {
        let last = self.drawn.take();
        let mut said = Vec::new();
        let panel_of = |asked: &Asked| (asked.panel.clone(), asked.showing.chats.clone());
        if last.as_ref().map(panel_of) != Some(panel_of(&asked)) {
            let work = ToRenderer::Panel(Box::new(asked.panel.clone()));
            let came = drawn(&mut self.renderers, &mut self.panel, Part::Panel, &work);
            if matches!(came, Some(Drawn::Painted(..))) {
                self.painted.chats.clone_from(&asked.showing.chats);
            }
            said.extend(came);
        }
        let start_of = |asked: &Asked| {
            (
                asked.start.clone(),
                asked.showing.sites.clone(),
                asked.showing.waiting.clone(),
                asked.showing.card,
            )
        };
        if last.as_ref().map(start_of) != Some(start_of(&asked)) {
            let work = ToRenderer::Start(Box::new(asked.start.clone()));
            let came = drawn(&mut self.renderers, &mut self.start, Part::Start, &work);
            if matches!(came, Some(Drawn::Painted(..))) {
                self.painted.sites.clone_from(&asked.showing.sites);
                self.painted.waiting.clone_from(&asked.showing.waiting);
                // Whether the picture the person pressed held a card: never
                // kept, every press on the card's bar was refused as a press
                // on no card (queue item 742).
                self.painted.card = asked.showing.card;
            }
            said.extend(came);
        }
        self.drawn = Some(asked);
        said
    }

    /// Send a person's press at `at` on `part` to its renderer and decide
    /// its claim: what the window is told, or nothing when the part's
    /// renderer had already gone or nothing was ever asked to be drawn.
    fn press(&mut self, part: Part, at: Point) -> Option<Drawn> {
        let now = self.drawn.as_ref()?.showing.clone();
        let held = match part {
            Part::Panel => &mut self.panel,
            Part::Start => &mut self.start,
        };
        if held.gone.is_some() {
            return None;
        }
        match self
            .renderers
            .ask(&held.site, &ToRenderer::ConversationPress(at))
        {
            Ok(answer) => Some(Drawn::Pressed(
                part,
                decide(answer, part, &self.painted, &now),
            )),
            Err(gone) => {
                let said = gone.to_string();
                held.gone = Some(gone);
                Some(Drawn::Said(part, said))
            }
        }
    }
}

/// What came of sending `work` to `held`'s renderer and asking it to paint,
/// as the window is told it — nothing for a renderer that had already gone,
/// whose going was said when it went.
fn drawn(
    renderers: &mut Renderers,
    held: &mut Held,
    part: Part,
    work: &ToRenderer,
) -> Option<Drawn> {
    if held.gone.is_some() {
        return None;
    }
    let answer = renderers
        .ask(&held.site, work)
        .and_then(|built| match built {
            FromRenderer::Loaded { .. } => renderers.ask(&held.site, &ToRenderer::Paint),
            other => Ok(other),
        });
    Some(match answer {
        Ok(FromRenderer::Painted(frame)) => Drawn::Painted(part, frame),
        Ok(FromRenderer::Failed(failure)) => {
            Drawn::Said(part, format!("{part} could not be drawn: {failure}"))
        }
        Ok(other) => Drawn::Said(
            part,
            conversation_decide::unasked(&other)
                .map(|refused| refused.to_string())
                .or_else(|| unasked(&other).map(|refused| refused.to_string()))
                .unwrap_or_else(|| {
                    format!("{part}'s renderer answered something it was not asked")
                }),
        ),
        Err(gone) => {
            let said = gone.to_string();
            held.gone = Some(gone);
            Drawn::Said(part, said)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alo_layout::Size;
    use alo_renderer::panel::Chat;

    /// Renderers that can never start, each of which goes as it is asked.
    fn nowhere() -> Renderers {
        Renderers::running("/nonexistent/alo-render", &[])
    }

    fn asked(name: &str) -> Asked {
        Asked {
            panel: Panel {
                chats: vec![Chat::new(name, "").current()],
                viewport: Size::new(240.0, 100.0),
                scale: 1.0,
            },
            start: Start {
                viewport: Size::new(300.0, 100.0),
                ..Start::default()
            },
            showing: Showing::default(),
        }
    }

    fn holding() -> Holding {
        Holding {
            renderers: nowhere(),
            panel: Held::at(panel::site()),
            start: Held::at(start::site()),
            drawn: None,
            painted: Showing::default(),
        }
    }

    #[test]
    fn a_press_before_anything_was_drawn_or_on_a_part_gone_is_not_sent() {
        let mut holding = holding();
        assert_eq!(holding.press(Part::Panel, Point::new(1.0, 1.0)), None);
        let _ = holding.draw(asked("a"));
        let started = holding.renderers.started();
        assert_eq!(holding.press(Part::Panel, Point::new(1.0, 1.0)), None);
        assert_eq!(holding.press(Part::Start, Point::new(1.0, 1.0)), None);
        assert_eq!(holding.renderers.started(), started, "nothing was started");
    }

    #[test]
    fn a_press_on_a_renderer_that_goes_as_it_is_asked_is_said_by_its_part() {
        let mut holding = holding();
        let _ = holding.draw(asked("a"));
        holding.start.gone = None;
        let said = holding.press(Part::Start, Point::new(1.0, 1.0));
        assert!(
            matches!(&said, Some(Drawn::Said(Part::Start, words)) if words.starts_with("the renderer for alo:")),
            "{said:?}"
        );
        assert!(holding.start.gone.is_some());
    }

    #[test]
    fn a_renderer_that_cannot_start_is_said_once_by_its_part_and_not_asked_again() {
        let mut holding = holding();
        let said = holding.draw(asked("a"));
        let [
            Drawn::Said(Part::Panel, panel),
            Drawn::Said(Part::Start, start),
        ] = said.as_slice()
        else {
            panic!("both parts said why: {said:?}");
        };
        assert!(panel.starts_with("the renderer for alo:"), "{panel}");
        assert!(start.starts_with("the renderer for alo:"), "{start}");
        assert!(holding.panel.gone.is_some() && holding.start.gone.is_some());
        // A different panel is not drawn by a fresh process, and nothing
        // more is said.
        let started = holding.renderers.started();
        assert_eq!(holding.draw(asked("b")), Vec::new());
        assert_eq!(holding.renderers.started(), started, "nothing was started");
    }

    #[test]
    fn the_same_parts_are_not_drawn_twice() {
        let mut holding = holding();
        let _ = holding.draw(asked("a"));
        // Forget the going, so a part that was drawn again would say so.
        holding.panel.gone = None;
        holding.start.gone = None;
        assert_eq!(holding.draw(asked("a")), Vec::new());
        let only_the_panel = holding.draw(asked("b"));
        assert!(
            matches!(only_the_panel.as_slice(), [Drawn::Said(Part::Panel, _)]),
            "{only_the_panel:?}"
        );
    }

    #[test]
    fn each_part_is_named_in_words() {
        assert_eq!(Part::Panel.to_string(), "the chats panel");
        assert_eq!(Part::Start.to_string(), "the first screen");
    }
}
