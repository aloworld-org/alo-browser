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

use alo_renderer::host::{Gone, Renderers};
use alo_renderer::panel::{self, Panel};
use alo_renderer::site::Site;
use alo_renderer::start::{self, Start};
use alo_renderer::strip_decide::unasked;
use alo_renderer::{Frame, FromRenderer, ToRenderer};
use core::fmt;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

/// Which part of the conversation's window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// The chats panel, down the left.
    Panel,
    /// The first screen, beside it.
    Start,
}

impl fmt::Display for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Part::Panel => "the chats panel",
            Part::Start => "the first screen",
        })
    }
}

/// What the window is told of a part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Drawn {
    /// Its renderer painted it: in device pixels at the window's scale.
    Painted(Part, Frame),
    /// Why its renderer painted nothing, in this browser's words.
    Said(Part, String),
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
}

/// A thread holding the panel's renderer and the first screen's.
#[derive(Debug)]
pub struct ConversationDrawing {
    /// Where what is asked is sent; [`None`] once finished.
    asked: Option<Sender<Asked>>,
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
            let _ = sender.send(asked.clone());
        }
        self.sent = Some(asked);
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
    /// drawn is not asked again until it is a different part.
    drawn: Option<Asked>,
}

/// Draw each [`Asked`] `asking` sends, the newest of those waiting, telling
/// the window what came of each part drawn, until nobody sends any more or
/// the window has gone. The renderers are stopped as `holding` goes.
fn hold(mut holding: Holding, asking: &Receiver<Asked>, tell: &TellConversation) {
    while let Ok(first) = asking.recv() {
        let newest = asking.try_iter().last().unwrap_or(first);
        for drawn in holding.draw(newest) {
            if !tell(drawn) {
                return;
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
        if last.as_ref().map(|last| &last.panel) != Some(&asked.panel) {
            let work = ToRenderer::Panel(Box::new(asked.panel.clone()));
            said.extend(drawn(
                &mut self.renderers,
                &mut self.panel,
                Part::Panel,
                &work,
            ));
        }
        if last.as_ref().map(|last| &last.start) != Some(&asked.start) {
            let work = ToRenderer::Start(Box::new(asked.start.clone()));
            said.extend(drawn(
                &mut self.renderers,
                &mut self.start,
                Part::Start,
                &work,
            ));
        }
        self.drawn = Some(asked);
        said
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
            unasked(&other).map_or_else(
                || format!("{part}'s renderer answered something it was not asked"),
                |refused| refused.to_string(),
            ),
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
        }
    }

    fn holding() -> Holding {
        Holding {
            renderers: nowhere(),
            panel: Held::at(panel::site()),
            start: Held::at(start::site()),
            drawn: None,
        }
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
