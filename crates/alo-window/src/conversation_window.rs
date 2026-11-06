/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The conversation's window short of the window server (ADR 0074, queue
//! items 709 to 718): everything `alo --conversation`'s event loop does
//! with what a person did and what its threads told it, in functions that
//! name no `winit` type, so a test drives **the same functions the event
//! loop calls** (queue item 742).
//!
//! `crate::window` reads each event — where the window is and at what
//! scale ([`Measured`]), the cursor, the main button, a wheel, a key, the
//! modifiers held, the window covered or given the keyboard — and hands it
//! here as this browser's own value; the threads' news arrives as
//! [`Heard`]. What the event loop must then do with the window is asked of
//! it here: draw again ([`ConversationWindow::take_redraw`]), take the
//! front back ([`ConversationWindow::take_front`]) and say a line on the
//! terminal ([`ConversationWindow::take_said`]).
//!
//! # Why this is its own file
//!
//! Queue items 717 and 718 were tested by calling the opener and the
//! renderer's claims directly, and every test passed, while in the real
//! window *Expand* and *Close* did nothing: the window's own road — the
//! aim, the drawing thread's claim, its decision against what was painted
//! — refused both, because the first screen's picture was never kept as
//! holding a card, and the refusal was cleared by the next frame of the
//! card sixteen milliseconds later. Run by hand, nothing said why. So the
//! road is here, where a test runs it whole.
//!
//! # A search
//!
//! Queue item 708: words chosen from the box are handed to a thread that
//! asks the search provider ([`crate::conversation_search`]), given with
//! [`ConversationWindow::with_search`]; what comes back is
//! [`Heard::Searched`], drawn as the All tab and said in the first screen's
//! line. A press on a site's title opens it as a card, as an address does.
//!
//! # A refusal is never silent
//!
//! What a press came to when it was refused, or its part's renderer could
//! not answer, is kept as [`ConversationWindow::told`] until the next press
//! is decided — drawn in the first screen's line when no renderer has
//! anything to say there — and said on the terminal. A frame of the card
//! drawn sixty times a second does not clear it, as it clears why a part
//! painted nothing. So is why a card could not be opened, or why its page
//! went on its own (queue item 749): closing the card paints the first
//! screen again, which wiped the reason before anybody read it.
//!
//! # The front, taken back
//!
//! A card's Chromium is started with a window of its own, parked off every
//! display (ADR 0076 B4), and macOS makes a program that shows a window
//! the one in front: measured by hand on 31 October 2026, Return made
//! *alo Chromium* frontmost within 0.6 s and the conversation's window lost
//! the keyboard (through alo's CEF host; CEF's views have no way to show
//! a window without activating its program, and the machine's Chrome was
//! not measured). So a window that had the front as the address
//! was chosen takes it back: as soon as it loses it while the card's
//! Chromium starts, and again once the card is open if it has not got it
//! back ([`ConversationWindow::take_front`]). A person who brings another
//! program in front in those seconds has it taken back too; nothing tells
//! the two apart.

use crate::card_aim::{self, Turned};
use crate::card_input::CardInput;
use crate::card_keys;
use crate::card_pointer::{CardPointer, OnThePage};
use crate::chat_record;
use crate::chat_store::{ChatStore, Opened};
use crate::chrome_history::Way;
use crate::conversation::{Conversation, Pressed as Decided};
use crate::conversation_agent::{Agent, AgentSaid, Run, Said, TellAgent};
use crate::conversation_answer;
use crate::conversation_bar::{self, BarFade};
use crate::conversation_box::{self, Typed};
use crate::conversation_chrome::{ChromeOpener, Placing, Streamed, Went};
use crate::conversation_compose::{ConversationScene, SheetScene};
use crate::conversation_drawing::{Asked, ConversationDrawing, Drawn, Part};
use crate::conversation_fold;
use crate::conversation_keys;
use crate::conversation_place::{
    ConversationPlacement, PANEL_WIDTH, panel_viewport_of, sheet_left, start_viewport_as,
};
use crate::conversation_search::{Came, Searched, Searcher};
use crate::conversation_sheet::Slide;
use crate::keys::{Held, Press};
use crate::scale::Scale;
use crate::typing;
use alo_layout::Size;
use alo_renderer::Frame;
use alo_renderer::conversation_scroll::{self, Reach};
use alo_renderer::results::{PROVIDER, SetUp, Showing, Tab};
use alo_renderer::split_place::Stands;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// What the conversation's window hears from the threads it hands work to.
#[derive(Debug)]
pub enum Heard {
    /// What the drawing thread painted, or why not, or what a press was.
    Drawn(Drawn),
    /// What became of an address chosen, from the thread holding Chrome.
    Chrome(Went),
    /// What came of a search, from the thread asking the search provider
    /// (queue item 708).
    Searched(Searched),
    /// What a run of the agent at work on the card's page said (queue item
    /// 704).
    Agent(AgentSaid),
}

/// Where the window is, as the window server gives it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measured {
    /// Its inside, in device pixels.
    pub size: (u32, u32),
    /// The screen's scale factor.
    pub scale_factor: f64,
    /// Its outer top left corner, in device pixels, if that can be read.
    pub outer: Option<(i32, i32)>,
}

/// The conversation's window's state, short of the window server.
pub struct ConversationWindow {
    /// The scale a person named, if they named one.
    forced: Option<Scale>,
    /// What the browser process holds of the interface.
    conversation: Conversation,
    /// The thread drawing the two parts; [`None`] once finished.
    drawing: Option<ConversationDrawing>,
    /// The thread opening an address chosen in Chrome; [`None`] once
    /// finished.
    chrome: Option<ChromeOpener>,
    /// The thread asking the search provider (queue item 708); [`None`]
    /// when the window was given none, or once finished.
    searcher: Option<Searcher>,
    /// How the two threads ended, once waited for.
    ended: Ended,
    /// The panel, as its renderer last painted it.
    panel: Option<Frame>,
    /// Why the panel's renderer painted nothing, since it last painted.
    panel_said: Option<String>,
    /// The first screen, as its renderer last painted it.
    start: Option<Frame>,
    /// What the first screen's line says until the part paints again: why
    /// its renderer painted nothing, or what became of an address.
    start_said: Option<String>,
    /// What the last press came to when it was refused or unanswered, kept
    /// until the next press is decided (queue item 742).
    told: Option<String>,
    /// Where the window is, once there is one.
    window: Option<Measured>,
    /// Where the cursor last was, in device pixels, while it is over the
    /// window.
    cursor: Option<(f64, f64)>,
    /// The pointer over the card's page, as far as the page has been told:
    /// a press held on it, whose moves and release are the page's (queue
    /// item 718), whether it is hovered, and the presses counted (740).
    pointer: CardPointer,
    /// How far the conversation was scrolled when the first screen was
    /// last painted (queue item 703): where the live card's page box was
    /// drawn, which a press on it is aimed by.
    drawn_down: f32,
    /// How many offers for what was typed stood over the box when the first
    /// screen was last painted (queue item 759): the room the live card's
    /// page is seen in was shorter by them.
    drawn_offered: usize,
    /// Where the live card stood when the first screen was last painted:
    /// in the conversation, or in the split view while alo's agent works on
    /// it (queue item 819) — where a press on its page is aimed.
    drawn_stands: Stands,
    /// How far the conversation was scrolled, how many offers stood over
    /// the box, and where the live card stood, in the frame of the first
    /// screen about to be told: its drawing thread says so just before it.
    painting_down: Option<(f32, usize, Stands)>,
    /// The modifier keys held.
    held: Held,
    /// Whether the window has the keyboard, and the front is to be taken
    /// back.
    front: Front,
    /// Whether the window is to be drawn again.
    redraw: bool,
    /// Lines to say on the terminal.
    said: Vec<String>,
    /// What the card's page was last asked to be streamed at, while it is
    /// streamed: a window resized, or its panel hidden or shown, asks again
    /// when its page box is another size (queue item 774).
    streaming: Option<Streamed>,
    /// Where the chats are kept across launches (queue item 757, ADR
    /// 0092), when the window was given somewhere.
    store: Option<ChatStore>,
    /// What was last written, or read, of each chat kept, by its number:
    /// a chat is written again only when this differs.
    written: HashMap<u64, String>,
    /// Why a chat was last not written, said once until it changes.
    unkept: Option<String>,
    /// The agent that works on the card's page, and how its runs tell the
    /// window (queue item 704); [`None`] when the window was given none.
    agent: Option<(Agent, TellAgent)>,
    /// The run of the agent at work, while one is.
    run: Option<Run>,
    /// Whether the conversation is kept scrolled to its newest end, so the
    /// agent's newest line and its question are seen — from what a run
    /// says until a person scrolls.
    following: bool,
    /// Whether the cursor is over the panel, where its row offers *Delete*
    /// (queue item 789): a hover answered after it left is no row.
    over_the_panel: bool,
    /// Where the sites blocking is switched off for are kept, when the
    /// window was given somewhere (queue item 807).
    switches: Option<std::path::PathBuf>,
    /// The sheet sliding over the conversation in a narrow window (queue
    /// item 776).
    slide: Slide,
    /// The panel's last frame painted as the sheet: what slides back in
    /// once it is closed, while the rail is drawn under it.
    sheet_frame: Option<Frame>,
    /// How much of the conversation's bar is seen (queue item 793).
    bar: BarFade,
    /// The chat, the number of turns and how far down they were when the
    /// first screen was last asked to be drawn: a change of the last two is
    /// the conversation moving, which brings its bar back.
    moved_from: Option<(Option<crate::conversation::ChatId>, usize, u32)>,
    /// What the window reads the time from: the machine's clock, or a
    /// test's own ([`ConversationWindow::with_clock`]).
    clock: Box<dyn Fn() -> Instant>,
}

/// How the window's two threads ended.
#[derive(Debug, Clone, Copy)]
struct Ended {
    /// Whether the drawing thread finished rather than panicked.
    drawing: bool,
    /// Whether the Chrome thread finished, and every Chrome it held with
    /// it.
    chrome: bool,
    /// Whether the search thread finished.
    search: bool,
}

/// The window and the front of the screen.
#[derive(Debug, Clone, Copy, Default)]
struct Front {
    /// Whether the window has the keyboard.
    focused: bool,
    /// Whether the window had the keyboard when an address was last
    /// chosen, and so takes the front back once its card is open.
    owed: bool,
    /// Whether the window is to be brought in front.
    asked: bool,
}

/// What a press with `held` is said to have come to (queue item 805).
fn held_press(held: Held) -> String {
    format!(
        "a press with {} held was not acted on: a press with a modifier is nobody's yet \
         (queue item 798)",
        held.said()
    )
}

impl ConversationWindow {
    /// The window's state with nothing drawn yet: drawn by `drawing`, an
    /// address chosen opened by `chrome`, laid out at `forced` when a
    /// person named a scale.
    pub fn new(forced: Option<Scale>, drawing: ConversationDrawing, chrome: ChromeOpener) -> Self {
        Self {
            forced,
            conversation: Conversation::default(),
            drawing: Some(drawing),
            chrome: Some(chrome),
            searcher: None,
            ended: Ended {
                drawing: true,
                chrome: true,
                search: true,
            },
            panel: None,
            panel_said: None,
            start: None,
            start_said: None,
            told: None,
            window: None,
            cursor: None,
            pointer: CardPointer::default(),
            drawn_down: 0.0,
            drawn_offered: 0,
            drawn_stands: Stands::InTheConversation,
            painting_down: None,
            held: Held::NOTHING,
            front: Front::default(),
            redraw: false,
            said: Vec::new(),
            streaming: None,
            store: None,
            written: HashMap::new(),
            unkept: None,
            agent: None,
            run: None,
            following: false,
            over_the_panel: false,
            switches: None,
            slide: Slide::default(),
            sheet_frame: None,
            bar: BarFade::default(),
            moved_from: None,
            clock: Box::new(Instant::now),
        }
    }

    /// The same window, reading the time from `clock` rather than the
    /// machine's: what a test of a slide or a fade sets, so its state is
    /// asserted at given instants rather than waited for (queue items 776,
    /// 793).
    #[must_use]
    pub fn with_clock(mut self, clock: impl Fn() -> Instant + 'static) -> Self {
        self.clock = Box::new(clock);
        self
    }

    /// The time, as the window reads it.
    fn now(&self) -> Instant {
        (self.clock)()
    }

    /// The same window, its card's page worked on by `agent` (queue item
    /// 704, ADR 0096), each run telling the window with `tell`; or, with
    /// none and why, asking alo said not set up, and why.
    #[must_use]
    pub fn with_agent(mut self, agent: Result<Agent, String>, tell: TellAgent) -> Self {
        match agent {
            Ok(agent) => {
                self.conversation.set_agent(SetUp::Ready);
                self.agent = Some((agent, tell));
            }
            Err(why) => self.conversation.set_agent(SetUp::Not(why)),
        }
        self
    }

    /// Start the run the conversation just started, if it did, stopping the
    /// one at work before it; and stop a run the conversation no longer has
    /// at work — stopped, its card closed or put away, another page opened.
    fn follow_runs(&mut self) {
        if let Some((number, asked)) = self.conversation.take_run() {
            if let Some(run) = self.run.take() {
                run.stop();
                self.unmark();
            }
            if let Some((agent, tell)) = &self.agent {
                match agent.start(number, asked, Arc::clone(tell)) {
                    Ok(run) => {
                        self.run = Some(run);
                        self.following = true;
                    }
                    Err(why) => {
                        let _ = self.conversation.stop_agent(&format!(
                            "alo could not start: no thread to work in ({why})."
                        ));
                    }
                }
            }
        }
        let working = self.conversation.working();
        if self
            .run
            .as_ref()
            .is_some_and(|run| Some(run.number()) != working)
            && let Some(run) = self.run.take()
        {
            run.stop();
            self.unmark();
        }
        // The run over, the panel is what the person chose again, and what
        // was said of it coming back is no longer so (queue item 819).
        if !self.conversation.at_work()
            && self.told.as_deref() == Some(conversation_fold::at_work().as_str())
        {
            self.told = None;
        }
    }

    /// Take away anything Chrome draws over the card's page (queue item
    /// 822): a run has ended or been stopped, and nothing of Chrome's is
    /// left over a page nobody is acting on.
    fn unmark(&self) {
        if let Some(chrome) = &self.chrome {
            chrome.unmark();
        }
    }

    /// What a run of the agent said: shown on its card, said on the
    /// terminal, and the conversation kept at its newest end so it is seen.
    fn agent_said(&mut self, said: AgentSaid) {
        if self.run.as_ref().map(Run::number) != Some(said.run) {
            return;
        }
        match &said.said {
            Said::Step { line, why, .. } if why.is_empty() => {
                self.said.push(format!("agent: {line}"));
            }
            Said::Step { line, why, .. } => self.said.push(format!("agent: {line} ({why})")),
            Said::Asking { what, .. } => self.said.push(format!("agent asks: {what}")),
            Said::Allowed => self.said.push("agent: allowed by the person".to_owned()),
            Said::Stuck { why, .. } => self.said.push(format!("agent is stuck: {why}")),
            Said::Done { result } => self.said.push(format!("agent is done: {result}")),
            Said::Stopped { why } => self.said.push(format!("agent stopped: {why}")),
            Said::Spent(spent) => self.said.push(format!(
                "agent: {}'s {} chose a step ({} tokens in, {} out)",
                crate::openai_answer::ANSWERER,
                crate::openai_answer::MODEL,
                spent.input,
                spent.output
            )),
        }
        if self.conversation.agent_said(said) {
            self.following = true;
            self.ask_drawn();
        }
        self.follow_runs();
    }

    /// What a press on the agent's *Allow* or *Not now* was decided to be,
    /// handed to the run at work (queue item 704). Only a person's press
    /// reaches here: no agent's request is one.
    fn agent_pressed(&self, pressed: &Decided) {
        let Some(run) = &self.run else {
            return;
        };
        match pressed {
            Decided::Allow(question) => run.allow(*question),
            Decided::NotNow(question) => run.not_now(*question),
            _ => {}
        }
    }

    /// The same window, keeping its chats in `store` (queue item 757, ADR
    /// 0092) and holding those `opened` there from before, under
    /// *Earlier*: each file that could not be read said, with why, on the
    /// terminal and in the first screen's line, and left as it is.
    #[must_use]
    pub fn with_chats(mut self, store: ChatStore, opened: Opened) -> Self {
        for kept in &opened.chats {
            self.written.insert(
                kept.number,
                chat_record::write(&kept.record.name, &kept.record.turns),
            );
        }
        for damaged in &opened.unread {
            self.tell(damaged.said());
        }
        for said in &opened.said {
            self.tell(said.clone());
        }
        if opened.sealed > 0 {
            self.said.push(format!(
                "{} {} kept in words before {} sealed under the key in this Mac's keychain \
                 (ADR 0098)",
                opened.sealed,
                if opened.sealed == 1 { "chat" } else { "chats" },
                if opened.sealed == 1 { "was" } else { "were" },
            ));
        }
        self.conversation =
            std::mem::take(&mut self.conversation).keeping(opened.chats, opened.highest);
        self.store = Some(store);
        self
    }

    /// The same window, saying on the terminal that the key its chats are
    /// sealed under was `made` just now, in this person's keychain, when it
    /// was (queue item 790): a new keychain item is never made unsaid.
    #[must_use]
    pub fn key_made(mut self, made: bool) -> Self {
        if made {
            self.said.push(format!(
                "a key to seal the chats was made in this Mac's keychain, as the item {} \
                 (ADR 0098); lost, the chats sealed under it cannot be read",
                crate::chat_key::SERVICE
            ));
        }
        self
    }

    /// The same window, its cards' shields showing and switching `off` —
    /// the sites blocking is off for, shared with every card's Chromium —
    /// and each switch kept in `file` when there is one (queue item 807).
    #[must_use]
    pub fn with_blocking_off(
        mut self,
        off: crate::blocking_off::Off,
        file: Option<std::path::PathBuf>,
    ) -> Self {
        self.conversation.set_blocking_off(off);
        self.switches = file;
        self
    }

    /// A person switched blocking for `site` with a card's shield (queue
    /// item 807): the switch kept, said, and the card's page loaded again,
    /// so what it shows is what the switch now asks.
    fn switched(&mut self, site: &str) {
        let off = self.conversation.shield_off(site);
        self.said.push(format!(
            "blocking trackers and adverts is {} for {site} (ADR 0094)",
            if off { "off" } else { "on" }
        ));
        if let Some(file) = &self.switches
            && let Err(why) = crate::blocking_off::write(file, self.conversation.blocking_off())
        {
            self.tell(format!("a site's switch is not kept: {why}"));
        }
        if let Some(chrome) = &self.chrome {
            chrome.travel(crate::chrome_history::Way::Again);
        }
    }

    /// The same window, saying `line` on the terminal and in the first
    /// screen's line as it opens.
    #[must_use]
    pub fn saying(mut self, line: String) -> Self {
        self.tell(line);
        self
    }

    /// The same window, keeping no chats, because of `why`: said on the
    /// terminal and in the first screen's line, so that nobody believes a
    /// chat kept that is not.
    #[must_use]
    pub fn keeping_nothing(mut self, why: &str) -> Self {
        self.tell(format!("chats are not kept: {why}"));
        self
    }

    /// Write every chat whose kept form changed since it was last written
    /// (queue item 757): a chat that cannot be written is said once, until
    /// why changes, and tried again at its next change.
    fn keep(&mut self) {
        let Some(store) = &self.store else {
            return;
        };
        let mut failed = None;
        for (id, name, thread) in self.conversation.to_keep() {
            let written = chat_record::write(name, thread.turns());
            if self.written.get(&id.number()) == Some(&written) {
                continue;
            }
            match store.write(id.number(), &written) {
                Ok(()) => {
                    self.written.insert(id.number(), written);
                }
                Err(why) => failed = Some(why),
            }
        }
        if let Some(why) = failed
            && self.unkept.as_ref() != Some(&why)
        {
            self.tell(format!("a chat is not kept: {why}"));
            self.unkept = Some(why);
        }
    }

    /// The same window, searching what is chosen from the box through
    /// `searcher` (queue item 708), whose being set up or not the box's
    /// offer to search says. A window given none says searching is not set
    /// up.
    #[must_use]
    pub fn with_search(mut self, searcher: Searcher) -> Self {
        self.conversation.set_search(searcher.set_up().clone());
        self.searcher = Some(searcher);
        self
    }

    /// What the browser process holds of the interface.
    pub fn conversation(&self) -> &Conversation {
        &self.conversation
    }

    /// What the last press came to when it was refused or unanswered, until
    /// the next press is decided.
    pub fn told(&self) -> Option<&str> {
        self.told.as_deref()
    }

    /// The window is where `measured` says — given as it opens, and each
    /// time it is resized, moved or given another scale. Narrowed past
    /// [`conversation_fold::FOLD_WIDTH`] the panel folds to its rail, and
    /// widened past it again it unfolds unless a person hid it (queue item
    /// 775): both parts are then laid out again beside it, and the card's
    /// page streamed at its new page box, as when a person hides it.
    /// Unfolded, a sheet that stood over the conversation is gone at once
    /// (queue item 776): the panel stands whole beside it.
    pub fn measured_as(&mut self, measured: Measured) {
        self.window = Some(measured);
        let sheet = self.conversation.sheet();
        if let Some(((width, _), scale)) = self.measured()
            && self.conversation.fit(scale.fitting(width))
        {
            if sheet && !self.conversation.sheet() {
                self.slide.shut();
                self.sheet_frame = None;
            }
            self.redraw = true;
            self.ask_drawn();
        }
    }

    /// The window's size in device pixels and its scale, if there is a
    /// window.
    pub fn measured(&self) -> Option<((u32, u32), Scale)> {
        self.window.map(|window| {
            let scale = self
                .forced
                .unwrap_or_else(|| Scale::of_screen(window.scale_factor));
            (window.size, scale)
        })
    }

    /// Whether the window is to be drawn again since this was last asked.
    pub fn take_redraw(&mut self) -> bool {
        std::mem::take(&mut self.redraw)
    }

    /// Whether the window is to be brought in front since this was last
    /// asked: a card's Chromium took the front as it showed its parked
    /// window, from a window that had it.
    pub fn take_front(&mut self) -> bool {
        std::mem::take(&mut self.front.asked)
    }

    /// The lines to say on the terminal since this was last asked.
    pub fn take_said(&mut self) -> Vec<String> {
        std::mem::take(&mut self.said)
    }

    /// Have both parts drawn at the window's size and scale — which the
    /// drawing thread does only for a part that differs from the last.
    pub fn ask_drawn(&mut self) {
        let Some((window, scale)) = self.measured() else {
            return;
        };
        if self.drawing.is_none() {
            return;
        }
        let side = self.conversation.side();
        let panel = self.conversation.panel(
            panel_viewport_of(window, scale, self.conversation.panel_width()),
            scale,
        );
        let mut start = self
            .conversation
            .start(start_viewport_as(window, scale, side), scale);
        // The conversation moved — scrolled, or a turn added — since it was
        // last drawn: its bar is whole again, and faded as it says (793).
        let now = self.now();
        let here = (
            self.conversation.on_screen(),
            start.turns.len(),
            start.down.to_bits(),
        );
        if let Some((chat, turns, down)) = self.moved_from
            && chat == here.0
            && (turns, down) != (here.1, here.2)
        {
            self.bar.moved(now);
        }
        self.moved_from = Some(here);
        if let Some(bar) = start.bar.as_mut() {
            bar.seen = self.bar.seen(now);
        }
        let showing = self.conversation.showing();
        if let Some(drawing) = self.drawing.as_mut() {
            drawing.draw(Asked {
                panel,
                start,
                showing,
            });
        }
        self.restream();
    }

    /// The card's page streamed at its page box as the first screen is laid
    /// out now, if it is streamed at another (queue item 774): the window
    /// resized, or the panel hidden or shown, makes the column narrower or
    /// wider, and the card with it.
    fn restream(&mut self) {
        let now = self.card_streamed();
        if self.conversation.card().is_some()
            && self.streaming.is_some()
            && now.is_some()
            && self.streaming != now
        {
            self.streaming = now;
            if let Some(chrome) = &self.chrome {
                chrome.stream(now);
            }
        }
    }

    /// What the card's page is streamed at as the first screen is laid out
    /// now: its page box at the window's scale — in the conversation
    /// ([`alo_renderer::card_place::page_size`]), or in the split view while
    /// alo's agent works on it ([`alo_renderer::split_place::page_size`],
    /// queue item 819), so Chrome lays the page out at the split's size for
    /// the run and at the conversation's once it ends.
    fn card_streamed(&self) -> Option<Streamed> {
        let (_, scale) = self.measured()?;
        let room = self.start_room()?;
        let stands = Stands::of(room, self.conversation.card());
        Some(Streamed::at(stands.page_size(room), scale))
    }

    /// What the window is drawn from now, at its size and scale: the first
    /// screen's line says why its renderer painted nothing, or what became
    /// of an address, and otherwise what the last press came to when it was
    /// refused.
    pub fn scene(&self) -> Option<ConversationScene<'_>> {
        let (window, scale) = self.measured()?;
        let now = self.now();
        Some(ConversationScene {
            window,
            scale,
            panel: self.panel.as_ref(),
            panel_sentence: self.panel_said.as_deref(),
            start: self.start.as_ref(),
            start_sentence: self.start_said.as_deref().or(self.told.as_deref()),
            side: self.conversation.side(),
            named: self.conversation.rail_named(),
            sheet: self.sheet_scene(now),
        })
    }

    /// The sheet as it is composed at `now`, while any of it is seen and
    /// its renderer has painted the whole panel for it (queue item 776):
    /// until then the rail alone is drawn.
    fn sheet_scene(&self, now: Instant) -> Option<SheetScene<'_>> {
        let (window, scale) = self.measured()?;
        let whole = scale.device(PANEL_WIDTH).min(window.0);
        let frame = self
            .sheet_frame
            .as_ref()
            .filter(|frame| self.slide.seen(now) && frame.width == whole)?;
        Some(SheetScene {
            frame,
            left: sheet_left(self.slide.left(now), scale),
            shade: self.slide.shade(now),
        })
    }

    /// Where the sheet stands at `now` across the window, in device
    /// pixels, while it is out: its left edge, and how wide it is.
    fn sheet_at(&self, now: Instant) -> Option<(i32, u32, Scale)> {
        let (window, scale) = self.measured()?;
        self.conversation.sheet().then(|| {
            (
                sheet_left(self.slide.left(now), scale),
                scale.device(PANEL_WIDTH).min(window.0),
                scale,
            )
        })
    }

    /// Where the point `at`, in device pixels, is on the sheet standing at
    /// `now`, in the panel's CSS pixels — or [`None`] when it is outside
    /// the sheet, over the shade.
    fn on_the_sheet(&self, at: (f64, f64), now: Instant) -> Option<alo_paint::Point> {
        let (left, width, scale) = self.sheet_at(now)?;
        let x = at.0 - f64::from(left);
        if !(0.0..f64::from(width)).contains(&x) || at.1 < 0.0 {
            return None;
        }
        let factor = f64::from(scale.factor());
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a point on a window, in CSS pixels, far inside an f32's range"
        )]
        let point = alo_paint::Point::new((x / factor) as f32, (at.1 / factor) as f32);
        Some(point)
    }

    /// The first screen's viewport at the window's size and scale, beside
    /// the panel or its rail.
    fn start_room(&self) -> Option<Size> {
        let (window, scale) = self.measured()?;
        Some(start_viewport_as(window, scale, self.conversation.side()))
    }

    /// The first screen's viewport, how far its conversation was scrolled
    /// and how many offers stood over its box as it was last painted, while
    /// it shows the card's picture of its page, which is when the page can
    /// be pressed (queue items 718, 703, 759).
    fn card_on_screen(&self) -> Option<card_aim::OnScreen> {
        let viewport = self.start_room()?;
        self.conversation
            .card()
            .and_then(|card| card.frame.as_ref())
            .map(|_| card_aim::OnScreen {
                viewport,
                down: self.drawn_down,
                offered: self.drawn_offered,
                stands: self.drawn_stands,
            })
    }

    /// Whether the conversation is gliding a wheel's lines (queue item 756):
    /// the event loop is then to call [`Self::glide`] at each of the
    /// display's frames until it is not.
    pub fn gliding(&self) -> bool {
        self.conversation.gliding()
    }

    /// The glide under way moved on to `now`, and the first screen drawn
    /// again if that scrolled it.
    pub fn glide(&mut self, now: Instant) {
        let (Some((_, scale)), Some(room)) = (self.measured(), self.start_room()) else {
            return;
        };
        if self.conversation.glide(now, room, scale.factor()) {
            self.redraw = true;
            self.ask_drawn();
        }
    }

    /// What the point `at`, in device pixels, is over, from the window's
    /// own composition: the card's page, a part, or nothing.
    fn aimed(&self, at: (f64, f64)) -> Option<card_aim::Aimed> {
        let scene = self.scene()?;
        let placed = ConversationPlacement::of(scene.window, scene.scale, scene.parts());
        card_aim::aim(&placed, scene.scale, self.card_on_screen(), at)
    }

    /// Where the point `at` is on the card's page, wherever it is, while
    /// the card's picture is on the screen.
    fn on_the_card(&self, at: (f64, f64)) -> Option<(f64, f64)> {
        let scene = self.scene()?;
        let placed = ConversationPlacement::of(scene.window, scene.scale, scene.parts());
        let painted = self.card_on_screen()?;
        Some(card_aim::on_the_page(&placed, scene.scale, painted, at))
    }

    /// Send the card's page a person's `input`, through the thread holding
    /// its Chrome.
    fn to_the_page(&self, input: CardInput) {
        if let Some(chrome) = &self.chrome {
            chrome.person(input);
        }
    }

    /// The modifier keys held are now `held`.
    pub fn held(&mut self, held: Held) {
        self.held = held;
    }

    /// The window was given the keyboard, or lost it: lost while a card's
    /// Chromium is starting, which is what took it, it takes the front
    /// back at once.
    pub fn focused(&mut self, focused: bool) {
        self.front.focused = focused;
        if !focused && self.front.owed {
            self.front.asked = true;
        }
    }

    /// The cursor left the window: a hover on the card's page ended (queue
    /// item 740).
    pub fn cursor_left(&mut self) {
        self.cursor = None;
        if let Some(input) = self.pointer.left() {
            self.to_the_page(input);
        }
        self.off_the_panel();
    }

    /// The cursor is no longer over the panel: no row offers *Delete*.
    fn off_the_panel(&mut self) {
        self.over_the_panel = false;
        if self.conversation.hover(None) {
            self.redraw = true;
            self.ask_drawn();
        }
    }

    /// Close the sheet `now` — Escape, or a press outside it (queue item
    /// 776): it slides back in, and the rail is drawn under it.
    fn close_the_sheet(&mut self, now: Instant) {
        if self.conversation.close_sheet() {
            self.slide.close(now);
            self.over_the_panel = false;
            self.redraw = true;
            self.ask_drawn();
        }
    }

    /// The pointer is at `at` `now`: whether it is near the conversation's
    /// bar, which is then whole (queue item 793) — over the first screen,
    /// within the room, beside the bar — and the first screen drawn again
    /// if that changed.
    fn near_the_bar(&mut self, at: (f64, f64), now: Instant) {
        let near = match (self.aimed(at), self.start_room()) {
            (Some(card_aim::Aimed::Part(Part::Start, on)), Some(room))
                if self.seen_drawn().is_some() =>
            {
                let top = alo_renderer::card_place::TOP;
                conversation_bar::near(
                    on.x,
                    on.y,
                    conversation_scroll::bar_left(room),
                    (top, top + conversation_scroll::room(room)),
                )
            }
            _ => false,
        };
        if self.bar.pointer(near, now) {
            self.ask_drawn();
        }
    }

    /// Which chat's row the pointer is over, as the panel's renderer found
    /// it (queue item 789): drawn offering *Delete* while the cursor is
    /// still over the panel.
    fn hovered(&mut self, over: Option<crate::conversation::ChatId>) {
        let over = over.filter(|_| self.over_the_panel);
        if self.conversation.hover(over) {
            self.redraw = true;
            self.ask_drawn();
        }
    }

    /// When the window is next to be woken with [`Self::tick`], if it is:
    /// as a chat deleted can no longer be brought back (queue item 789).
    ///
    /// Also each of the display's frames while the sheet slides (queue item
    /// 776), and while the conversation's bar is to fade, when its second
    /// ends and each frame of the fade (793).
    pub fn wake_at(&self) -> Option<Instant> {
        let now = self.now();
        let slide = self
            .slide
            .moving(now)
            .then_some(now + conversation_bar::FRAME);
        let bar = self.seen_drawn().and_then(|_| self.bar.wake_at(now));
        [self.conversation.undo_until(), slide, bar]
            .into_iter()
            .flatten()
            .min()
    }

    /// The time is `now`: a chat deleted whose *Undo* has passed is gone,
    /// the panel drawn again without it, and its file removed; the sheet
    /// slid on, and once back in let go; the bar drawn as faded as it now
    /// is.
    pub fn tick(&mut self, now: Instant) {
        if self.conversation.expire(now) {
            self.redraw = true;
            self.ask_drawn();
        }
        self.remove_gone();
        let back_in = !self.slide.seen(now)
            && !self.conversation.sheet()
            && self.sheet_frame.take().is_some();
        if self.slide.moving(now) || back_in {
            self.redraw = true;
        }
        if self
            .seen_drawn()
            .is_some_and(|seen| seen.to_bits() != self.bar.seen(now).to_bits())
        {
            self.ask_drawn();
            self.redraw = true;
        }
    }

    /// How much of the conversation's bar the first screen was last asked
    /// to show, while it was asked to show one (queue item 793).
    pub fn seen_drawn(&self) -> Option<f32> {
        self.drawing
            .as_ref()
            .and_then(ConversationDrawing::sent_bar_seen)
    }

    /// Remove the file of each chat deleted and gone, and say so (queue item
    /// 789); one that cannot be removed is said, as a chat not kept is.
    fn remove_gone(&mut self) {
        let gone = self.conversation.take_gone();
        for id in gone {
            self.written.remove(&id.number());
            let Some(store) = &self.store else {
                continue;
            };
            match store.remove(id.number()) {
                Ok(()) => self.said.push(format!(
                    "the chat numbered {} was deleted, and its file removed",
                    id.number()
                )),
                Err(why) => self.tell(format!("a deleted chat's file is not removed: {why}")),
            }
        }
    }

    /// The main button went down: see [`Self::main_button_pressed_at`].
    pub fn main_button_pressed(&mut self) {
        self.main_button_pressed_at(Instant::now());
    }

    /// The main button went down `now` with nothing held: a press over the
    /// card's page is the page's, counted with the presses before it, sent
    /// to it, and gives it the keyboard (queue items 718, 740); a press
    /// over a part is handed to the thread that holds its renderer, to be
    /// claimed and decided. A press with a modifier held is nobody's yet
    /// (queue item 798) — and says so, as a refused press does (queue item
    /// 805): a modifier the window server still counts as held, as a tool
    /// posting a key with ⌘ and never its release leaves it, made a first
    /// press on a card just opened look lost.
    pub fn main_button_pressed_at(&mut self, now: Instant) {
        let Some(at) = self.cursor else {
            return;
        };
        if self.held != Held::NOTHING {
            if self.aimed(at).is_some() {
                self.tell(held_press(self.held));
                self.redraw = true;
            }
            return;
        }
        // The sheet out over the conversation (queue item 776): a press on
        // it is the panel's, where the sheet stands now; anywhere else —
        // the shade, the card's page under it — closes it, and nothing
        // under it is pressed.
        if self.conversation.sheet() {
            match self.on_the_sheet(at, now) {
                Some(on) => {
                    if let Some(drawing) = self.drawing.as_mut() {
                        drawing.press(Part::Panel, on);
                    }
                }
                None => self.close_the_sheet(now),
            }
            return;
        }
        match self.aimed(at) {
            Some(card_aim::Aimed::Page { x, y }) => {
                // A press decided as the page's: what an earlier one came
                // to is said no more.
                if self.told.take().is_some() {
                    self.redraw = true;
                }
                // *Click anywhere to take over* (queue item 704): the agent
                // at work stops, and the page is the person's.
                if self
                    .conversation
                    .stop_agent(crate::conversation::TAKEN_OVER)
                    .is_some()
                {
                    self.follow_runs();
                    self.redraw = true;
                    self.ask_drawn();
                }
                if self.conversation.press_the_page() {
                    self.ask_drawn();
                }
                let pressed = self.pointer.pressed(now, (x, y));
                self.to_the_page(pressed);
            }
            Some(card_aim::Aimed::Part(part, on)) => {
                if let Some(drawing) = self.drawing.as_mut() {
                    drawing.press(part, on);
                }
            }
            None => {}
        }
    }

    /// The cursor moved to `at`: the page's — as a drag while a press on it
    /// is held, as a hover while it is over the page box, and once more as
    /// it leaves it (queue items 718, 740).
    pub fn moved(&mut self, at: (f64, f64)) {
        self.moved_at(at, self.now());
    }

    /// The cursor moved to `at` at `now`, as [`Self::moved`]: over the sheet
    /// while it is out (queue item 776), its row hovered and nothing under
    /// it; near the conversation's bar, the bar whole (queue item 793).
    pub fn moved_at(&mut self, at: (f64, f64), now: Instant) {
        self.cursor = Some(at);
        if self.conversation.sheet() {
            match self.on_the_sheet(at, now) {
                Some(on) => {
                    self.over_the_panel = true;
                    if let Some(drawing) = self.drawing.as_mut() {
                        drawing.hover(on);
                    }
                }
                None if self.over_the_panel => self.off_the_panel(),
                None => {}
            }
            // Nothing under the shade is hovered: the page's hover ends.
            if let Some(input) = self.pointer.moved(None) {
                self.to_the_page(input);
            }
            return;
        }
        // Over the panel, the row under the pointer offers Delete or its ×
        // (queue items 789, 791), and on the rail its name stands beside it
        // (760); off it, none does.
        match self.aimed(at) {
            Some(card_aim::Aimed::Part(Part::Panel, on)) => {
                self.over_the_panel = true;
                if let Some(drawing) = self.drawing.as_mut() {
                    drawing.hover(on);
                }
            }
            _ if self.over_the_panel => self.off_the_panel(),
            _ => {}
        }
        self.near_the_bar(at, now);
        let on = self.on_the_card(at).map(|on| OnThePage {
            at: on,
            over: matches!(self.aimed(at), Some(card_aim::Aimed::Page { .. })),
        });
        if let Some(input) = self.pointer.moved(on) {
            self.to_the_page(input);
        }
    }

    /// The main button came up: the page's, where the cursor is, after a
    /// press on it.
    pub fn main_button_released(&mut self) {
        let at = self.cursor.and_then(|at| self.on_the_card(at));
        if let Some(input) = self.pointer.released(at) {
            self.to_the_page(input);
        }
    }

    /// A wheel or a trackpad turned: the page's, scrolled, when the cursor
    /// is over the card's page (queue item 718); the conversation's,
    /// scrolled down as far as the page would be, anywhere else over the
    /// first screen while it holds a turn (queue item 703), and drawn
    /// again; nobody's over the panel.
    pub fn wheeled(&mut self, turned: Turned) {
        self.wheeled_at(turned, Instant::now());
    }

    /// A wheel or a trackpad turned `now`, as [`Self::wheeled`]: over the
    /// conversation, a wheel's lines glided as Chrome glides them, from
    /// `now` (queue item 756), and a trackpad's pixels followed as they
    /// come.
    pub fn wheeled_at(&mut self, turned: Turned, now: Instant) {
        let Some(at) = self.cursor else {
            return;
        };
        let (Some((_, scale)), Some(room)) = (self.measured(), self.start_room()) else {
            return;
        };
        let (right, down) = card_aim::scrolled(turned, scale);
        // Nothing under the sheet scrolls, and the panel does not (776).
        if self.conversation.sheet() {
            return;
        }
        match self.aimed(at) {
            Some(card_aim::Aimed::Page { x, y }) => {
                self.to_the_page(CardInput::Wheel { x, y, right, down });
            }
            // The split view scrolls nothing of its own (queue item 819).
            Some(card_aim::Aimed::Part(Part::Start, _))
                if self.conversation.conversing() && self.drawn_stands != Stands::Split =>
            {
                // A person scrolling is followed, not the agent.
                self.following = false;
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "a wheel's turn in CSS pixels, far inside an f32's range; what the first screen is laid out in"
                )]
                let by = down as f32;
                let moved = match turned {
                    Turned::Lines { .. } => {
                        self.conversation.glide_lines(by, room, now, scale.factor())
                            && self.conversation.glide(now, room, scale.factor())
                    }
                    Turned::Pixels { .. } => self.conversation.scroll(by, room),
                };
                if moved {
                    self.redraw = true;
                    self.ask_drawn();
                }
            }
            _ => {}
        }
    }

    /// What a press on `part` was decided to be: acted on, and the parts
    /// drawn again if it changed what they show; or, refused or unanswered,
    /// kept to say until the next press is decided, and said on the
    /// terminal.
    fn pressed(&mut self, part: Part, pressed: Decided) {
        self.told = None;
        match &pressed {
            Decided::Refused(refusal) => self.tell(refusal.to_string()),
            Decided::Failed(failure) => {
                self.tell(format!("{part} could not answer a press: {failure}"));
            }
            _ => {}
        }
        if part == Part::Panel
            && let Some(told) = &self.told
        {
            self.panel_said = Some(told.clone());
        }
        self.redraw = true;
        let go = matches!(
            pressed,
            Decided::Go | Decided::Found(_) | Decided::Offer(_) | Decided::Image(_)
        );
        let had_a_card = self.conversation.card().is_some();
        let show = matches!(pressed, Decided::ShowThePanel);
        let sheet = self.conversation.sheet();
        self.card_pressed(&pressed);
        self.agent_pressed(&pressed);
        if self.conversation.act(pressed) {
            self.ask_drawn();
        }
        self.follow_runs();
        // Shown in a window too narrow for it, the panel slides over the
        // conversation as the sheet; a chat chosen in it, New chat or Hide
        // the panel slides it back (queue item 776).
        let now = self.now();
        match (sheet, self.conversation.sheet()) {
            (false, true) => self.slide.open(now),
            (true, false) => {
                self.slide.close(now);
                // The row hovered in the sheet is no row on the rail under
                // it: unhovered, or the rail would name it (found by hand).
                self.over_the_panel = false;
                if self.conversation.hover(None) {
                    self.ask_drawn();
                }
            }
            _ => {}
        }
        // While alo's agent works the panel is the rail until the run ends,
        // and the window says so (queue item 819).
        if show && self.conversation.at_work() {
            self.tell(conversation_fold::at_work());
        }
        self.ask_further();
        if go && let Some(chosen) = self.conversation.chosen() {
            self.start_said = Some(conversation_box::said(
                chosen,
                self.searching(),
                &self.conversation.asking(),
            ));
            self.go();
            self.search();
        }
        self.shut_if_put_away(had_a_card);
        if let Some(site) = self.conversation.take_switched() {
            self.switched(&site);
        }
        self.keep();
        self.remove_gone();
    }

    /// Close the live card's page if what was just done put the card away —
    /// a search after it, *New chat*, another chat (queue items 708, 703) —
    /// when there was one: one page is open at a time, and it is the
    /// newest turn's. Opening another address replaces it in Chrome itself.
    fn shut_if_put_away(&self, had_a_card: bool) {
        if had_a_card
            && self.conversation.card().is_none()
            && let Some(chrome) = &self.chrome
        {
            chrome.shut();
        }
    }

    /// Keep `told` — what a press came to when refused, or why a card
    /// failed — in the first screen's line until the next press is decided
    /// or something else is chosen, and say it on the terminal.
    fn tell(&mut self, told: String) {
        self.said.push(told.clone());
        self.told = Some(told);
    }

    /// Whether searching is set up, as the conversation holds it.
    fn searching(&self) -> &alo_renderer::results::SetUp {
        self.conversation.search_set_up()
    }

    /// Hand the search just chosen, if one was, to the thread asking the
    /// provider (queue item 708). Returns at once.
    fn search(&mut self) {
        let Some((number, words)) = self.conversation.searching() else {
            return;
        };
        if let Some(searcher) = &self.searcher {
            searcher.search(number, words);
        }
    }

    /// Hand each tab just opened whose results are to be asked for — Videos
    /// or News, once for a turn (queue items 745, 746) — and each further
    /// page of sites pressed for (761) to the thread asking the provider.
    /// Returns at once.
    fn ask_further(&mut self) {
        for further in self.conversation.take_asks() {
            if let Some(searcher) = &self.searcher {
                match further.tab {
                    Tab::Videos => searcher.videos(further.number, &further.words),
                    Tab::News => searcher.news(further.number, &further.words),
                    Tab::Sites => searcher.page(further.number, &further.words, further.offset),
                    Tab::All | Tab::Answer | Tab::Images => {}
                }
            }
        }
    }

    /// What came of a search: shown when it is the newest, and the first
    /// screen's line saying so. Its images (queue item 743) are drawn when
    /// they come, the line left saying what the sites came to.
    fn searched(&mut self, searched: Searched) {
        let line = match &searched.came {
            Came::Sites { showing, .. } => match showing {
                Showing::Found(found) if found.is_empty() => Some(format!(
                    "{PROVIDER} found no sites for \u{201c}{}\u{201d}",
                    searched.words
                )),
                Showing::Found(found) => Some(format!(
                    "{PROVIDER} found {} sites for \u{201c}{}\u{201d}",
                    found.len(),
                    searched.words
                )),
                Showing::Unanswered(why) => Some(why.clone()),
                Showing::Searching => return,
            },
            Came::Answer { answer, spent } => {
                // Said on the terminal, with what it cost, never on the
                // first screen's line, which keeps what the sites came to.
                self.said
                    .extend(conversation_answer::said(&searched.words, answer, *spent));
                None
            }
            Came::Images(_) | Came::Marks(_) | Came::Videos(_) | Came::News(_) | Came::Page(_) => {
                None
            }
        };
        if self.conversation.searched(searched) {
            if let Some(line) = line {
                self.start_said = Some(line);
            }
            self.ask_drawn();
        }
        // A turn of a chat not on screen may have changed too.
        self.keep();
    }

    /// Open where it was decided to go, if anywhere, in Chrome, as a card in
    /// the conversation streamed at the window's scale (ADR 0076 B, queue
    /// item 717). Returns at once.
    fn go(&mut self) {
        let Some(url) = self.conversation.going().cloned() else {
            return;
        };
        // Another page replaces the card, and the agent at work on it stops
        // before the new page is open to any agent (queue item 704).
        if self
            .conversation
            .stop_agent("alo stopped: you opened another page.")
            .is_some()
        {
            self.follow_runs();
        }
        let Some(streamed) = self.card_streamed() else {
            return;
        };
        self.front.owed = self.front.focused;
        if let Some(chrome) = &self.chrome {
            chrome.open(url, streamed);
            self.streaming = Some(streamed);
        }
    }

    /// Where Chrome's own window goes when a card is expanded: over the
    /// first screen when where this window is can be read, and where `alo
    /// --chrome` puts it otherwise (ADR 0075 § 3).
    fn over_the_first_screen(&self) -> Placing {
        self.window
            .and_then(|window| {
                Some(Placing::over_the_first_screen(
                    window.outer?,
                    window.size,
                    window.scale_factor,
                    self.conversation.side(),
                ))
            })
            .unwrap_or(Placing::APART)
    }

    /// What the card's buttons were decided to be, carried out by the
    /// thread holding its Chrome. *Back* and *Forward* are claimed only when
    /// there is somewhere to go, as the tab's session history last said
    /// (queue item 737), and are a person's: nothing an agent sends is one.
    fn card_pressed(&self, pressed: &Decided) {
        let Some(chrome) = &self.chrome else {
            return;
        };
        match pressed {
            Decided::Expand => chrome.expand(self.over_the_first_screen()),
            Decided::CloseTheCard => chrome.shut(),
            Decided::Back => chrome.travel(Way::Back),
            Decided::Forward => chrome.travel(Way::Forward),
            _ => {}
        }
    }

    /// The window can be seen, or cannot: the card's page is streamed at
    /// its page box at the window's scale while it can, and not while it
    /// cannot (ADR 0076: only while the card is on the screen). A new scale
    /// is frames of another size, asked for the same way.
    pub fn seen(&mut self, seen: bool) {
        let (Some(chrome), Some(streamed)) = (&self.chrome, self.card_streamed()) else {
            return;
        };
        if self.conversation.card().is_some() {
            let streaming = seen.then_some(streamed);
            chrome.stream(streaming);
            self.streaming = streaming;
        }
    }

    /// What became of an address chosen, from the thread holding Chrome:
    /// the card shown, given its newest frame or where its page is now, or
    /// taken away, and the first screen's line saying so.
    fn went(&mut self, went: Went) {
        let said = went.said();
        let framed = matches!(went, Went::Framed(_));
        match &went {
            // Each is shown by the card itself.
            Went::Framed(_)
            | Went::Unstreamed(_)
            | Went::Moved(_)
            | Went::Loading(_)
            | Went::Travel(_) => {}
            // Said on the terminal only, one line a request: the card shows
            // nothing of it yet (ADR 0094 § 6).
            Went::Blocked { .. } => self.said.push(said.clone()),
            // A card put away for a search's results, its page closed
            // after: the line says the search (queue item 708).
            Went::Shut(_) if self.conversation.card().is_none() => {}
            // A card that could not be opened, or whose page went on its
            // own, is kept as a refused press is (queue item 749): closing
            // the card paints the first screen, which clears its line.
            failed @ (Went::Unopened { .. } | Went::Closed(_)) => {
                self.start_said = None;
                self.tell(failed.said());
            }
            other => self.start_said = Some(other.said()),
        }
        let changed = match went {
            Went::Opening(url) => {
                // Another page: nothing held, hovered or counted on it.
                self.pointer.forget();
                self.conversation.open_card(url);
                true
            }
            Went::Framed(png) => self.conversation.framed(png),
            Went::Moved(url) => self.conversation.moved_card(url),
            Went::Loading(loading) => self.conversation.card_loading(loading),
            Went::Travel(travel) => self.conversation.card_travel(travel),
            // A page not opened leaves why in its turn; one that went on
            // its own leaves its address (queue item 703).
            Went::Unopened { .. } => {
                self.front.owed = false;
                self.conversation.card_failed(Some(said))
            }
            Went::Closed(_) => {
                self.front.owed = false;
                self.conversation.card_failed(None)
            }
            Went::Shut(_) => {
                self.front.owed = false;
                self.conversation.close_card()
            }
            Went::Opened { .. } => {
                // Its Chromium took the front as it showed its parked
                // window; the window had it as the address was chosen, and
                // has not had it back.
                self.front.asked |= std::mem::take(&mut self.front.owed) && !self.front.focused;
                false
            }
            // Counted on the card's shield (queue item 807).
            Went::Blocked { .. } => self.conversation.blocked(),
            Went::Unstreamed(_) => false,
        };
        if changed {
            self.ask_drawn();
            // A frame is a picture, never kept: what else changed may be.
            if !framed {
                self.keep();
                // The card gone or replaced: its agent with it.
                self.follow_runs();
            }
        }
    }

    /// What a thread told the window: acted on, and the window drawn again.
    pub fn heard(&mut self, heard: Heard) {
        self.redraw = true;
        match heard {
            Heard::Chrome(went) => self.went(went),
            Heard::Searched(searched) => self.searched(searched),
            Heard::Agent(said) => self.agent_said(said),
            Heard::Drawn(Drawn::Painted(Part::Panel, frame)) => {
                // Painted whole while the sheet is out (queue item 776): what
                // slides, and what slides back in once it is closed.
                if self.conversation.sheet() {
                    self.sheet_frame = Some(frame.clone());
                }
                self.panel = Some(frame);
                self.panel_said = None;
            }
            Heard::Drawn(Drawn::Painted(Part::Start, frame)) => {
                self.start = Some(frame);
                self.start_said = None;
                // Aimed by the offset this frame was drawn at: what its
                // drawing thread told just before it, or none for a first
                // screen with no turns (queue item 703).
                (self.drawn_down, self.drawn_offered, self.drawn_stands) = self
                    .painting_down
                    .take()
                    .unwrap_or((0.0, 0, Stands::InTheConversation));
            }
            Heard::Drawn(Drawn::Said(Part::Panel, said)) => self.panel_said = Some(said),
            Heard::Drawn(Drawn::Said(Part::Start, said)) => self.start_said = Some(said),
            Heard::Drawn(Drawn::Pressed(part, pressed)) => self.pressed(part, pressed),
            Heard::Drawn(Drawn::Hovered(over)) => self.hovered(over),
            Heard::Drawn(Drawn::Reached {
                turns,
                down,
                offered,
                stands,
                reach,
            }) => self.reached(turns, (down, offered, stands), reach),
        }
    }

    /// The first screen is about to be told painted with `turns` turns
    /// scrolled `down`, with `offered` offers over the box and the live card
    /// standing as the third of `painting` says, which reach as `reach` says
    /// (queue items 703, 759, 819): the live card's page box is
    /// aimed where it was drawn, in the room it was seen in, once that frame
    /// is had, and the conversation is kept inside its reach — drawn again
    /// if that moved it, or placed the sign of more above or below anew.
    fn reached(&mut self, turns: usize, painting: (f32, usize, Stands), reach: Reach) {
        self.painting_down = Some(painting);
        if painting.2 == Stands::Split {
            // The split view draws no conversation (queue item 819): what
            // it reaches, measured on nothing, is not the conversation's,
            // and it is kept where a person left it for when the run ends.
            return;
        }
        let Some(room) = self.start_room() else {
            return;
        };
        let scale = self.measured().map_or(1.0, |(_, scale)| scale.factor());
        let bar = |window: &Self| {
            window
                .conversation
                .thread()
                .and_then(|thread| thread.bar(room, scale))
        };
        let before = bar(self);
        // Kept inside the reach, or the sign of more above or below placed
        // by it for the first time or anew (queue item 755): drawn again.
        let moved = self.conversation.reached(reach, turns, room);
        // The agent's newest line and its question seen (queue item 704).
        let followed = self.following
            && self
                .conversation
                .card()
                .is_some_and(|card| card.agent.is_some())
            && self
                .conversation
                .scroll_by_key(conversation_keys::Scroll::ToNewest, room);
        if moved || followed || bar(self) != before {
            self.ask_drawn();
        }
    }

    /// A key went down, read as `press`, writing `text` if it writes any:
    /// the card's page's while it has the keyboard; otherwise an edit of the
    /// one box, if it makes one, drawn again when it changed what the box
    /// holds, and what it chose said.
    pub fn key(&mut self, press: &Press, text: Option<&str>) {
        // The sheet out (queue item 776): Escape closes it, whatever has
        // the keyboard; *Search chats* in it still takes what is typed, and
        // nothing under it takes a key.
        if self.conversation.sheet() {
            if press.key == crate::keys::Key::Escape && press.held == Held::NOTHING {
                let now = self.now();
                self.close_the_sheet(now);
            } else if self.conversation.find().focused()
                && let Some(edit) = typing::edit(press, text)
                && self.conversation.edit(edit) == Typed::Changed
            {
                self.ask_drawn();
                self.redraw = true;
            }
            return;
        }
        if self.conversation.keys_to_the_card() {
            // The card's page has the keyboard (queue item 718).
            if let Some(key) = card_keys::key(press, text) {
                self.to_the_page(CardInput::Key(key));
            }
            return;
        }
        // The paging keys scroll the conversation while nothing holding the
        // keyboard would take them (queue item 754).
        if self.conversation.keys_scroll()
            && let Some(room) = self.start_room()
            && let Some(scroll) = conversation_keys::scroll(press, conversation_scroll::room(room))
        {
            self.following = false;
            if self.conversation.scroll_by_key(scroll, room) {
                self.redraw = true;
                self.ask_drawn();
            }
            return;
        }
        let Some(edit) = typing::edit(press, text) else {
            return;
        };
        // Whether a card was live before the edit, which a search chosen by
        // Return puts away: asked after it, the card had already gone and
        // its page was never closed (found by hand, queue item 703).
        let had_a_card = self.conversation.card().is_some();
        match self.conversation.edit(edit) {
            Typed::Unchanged => {}
            Typed::Changed => self.ask_drawn(),
            Typed::Chose(chosen) => {
                // A choice is a decision as a press is: what the last one
                // came to is no longer said.
                self.told = None;
                self.start_said = Some(conversation_box::said(
                    &chosen,
                    self.searching(),
                    &self.conversation.asking(),
                ));
                // The box, emptied when a site or a search was chosen,
                // drawn so.
                self.ask_drawn();
                self.go();
                self.search();
                self.shut_if_put_away(had_a_card);
                self.follow_runs();
                self.redraw = true;
                self.keep();
            }
        }
    }

    /// Stop both threads and wait for them: the renderers stopped, and the
    /// Chrome showing a page closed and its profile removed.
    pub fn finish(&mut self) {
        // The last of what is kept written, the live card's address with it.
        self.keep();
        // A chat deleted and still held for its Undo is gone as the window
        // closes, and its file removed (queue item 789).
        self.conversation.let_go();
        self.remove_gone();
        // The agent at work stops before its card's Chrome closes.
        if let Some(run) = self.run.take() {
            let _ = run.finish();
        }
        if let Some(drawing) = self.drawing.take() {
            self.ended.drawing = drawing.finish();
        }
        if let Some(chrome) = self.chrome.take() {
            self.ended.chrome = chrome.finish();
        }
        if let Some(searcher) = self.searcher.take() {
            self.ended.search = searcher.finish();
        }
    }

    /// Why the window's threads did not end well, once [`finish`]ed: the
    /// drawing thread or the one holding Chrome did not finish.
    ///
    /// [`finish`]: ConversationWindow::finish
    pub fn unfinished(&self) -> Option<&'static str> {
        if !self.ended.drawing {
            Some("the thread drawing the chats panel and the first screen did not finish")
        } else if !self.ended.chrome {
            Some("the thread holding Chrome for the conversation did not finish")
        } else if !self.ended.search {
            Some("the thread searching for the conversation did not finish")
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::beside::Chromium;
    use crate::chrome_history::Travel;
    use crate::conversation_chrome::Shown;
    use crate::conversation_place::start_viewport;
    use crate::conversation_thread::Thread;
    use crate::keys::Key;
    use alo_renderer::conversation_claim::Claim;
    use alo_renderer::conversation_decide::Refusal;
    use alo_renderer::host::Renderers;
    use std::path::PathBuf;

    /// A window whose renderers never start and whose Chrome is never
    /// asked for, at the design's size at one.
    fn window() -> ConversationWindow {
        let drawing = ConversationDrawing::start(
            Renderers::running("/nonexistent/alo-render", &[]),
            Box::new(|_| true),
        )
        .unwrap();
        let chrome = ChromeOpener::start(Chromium::Machines, Box::new(|_| true)).unwrap();
        let mut window = ConversationWindow::new(Some(Scale::ONE), drawing, chrome);
        window.measured_as(Measured {
            size: (1280, 820),
            scale_factor: 1.0,
            outer: Some((0, 0)),
        });
        window
    }

    fn painted() -> Frame {
        Frame {
            width: 1,
            height: 1,
            pixels: vec![0; 4],
        }
    }

    #[test]
    fn a_cards_shield_counts_refusals_and_a_press_keeps_its_sites_switch() {
        let directory =
            std::env::temp_dir().join(format!("alo-window-shield-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let file = crate::blocking_off::file_in(&directory);
        let off = crate::blocking_off::Off::default();
        let mut window = window().with_blocking_off(off.clone(), Some(file.clone()));
        let url = alo_url::parse("https://www.theguardian.com/international").unwrap();
        window.heard(Heard::Chrome(Went::Opening(url.clone())));
        let refused = || Went::Blocked {
            url: "https://securepubads.g.doubleclick.net/gpt.js".to_owned(),
            by: crate::blocking::Blocked {
                list: Some("EasyList"),
                line: Some(1),
                rule: Some("||doubleclick.net^".to_owned()),
            },
        };
        window.heard(Heard::Chrome(refused()));
        window.heard(Heard::Chrome(refused()));
        let shield = |window: &ConversationWindow| {
            window
                .conversation()
                .card()
                .and_then(|card| card.shield)
                .map(|shield| (shield.blocked, shield.on))
        };
        assert_eq!(shield(&window), Some((2, true)));
        let _ = window.take_said();
        // Pressed: off for the site, kept in the file, said.
        window.heard(Heard::Drawn(Drawn::Pressed(
            Part::Start,
            Decided::Shield("theguardian.com".to_owned()),
        )));
        assert_eq!(shield(&window), Some((0, false)));
        assert!(off.is_off(&url.serialised), "the Chromiums' switches too");
        assert_eq!(
            crate::blocking_off::read(&file),
            Ok(vec!["theguardian.com".to_owned()])
        );
        assert!(
            window.take_said().iter().any(|line| line
                == "blocking trackers and adverts is off for theguardian.com (ADR 0094)"),
        );
        // Refused while off: nothing is (and nothing is counted).
        window.heard(Heard::Chrome(refused()));
        assert_eq!(shield(&window), Some((0, false)));
        // On again.
        window.heard(Heard::Drawn(Drawn::Pressed(
            Part::Start,
            Decided::Shield("theguardian.com".to_owned()),
        )));
        assert_eq!(crate::blocking_off::read(&file), Ok(Vec::new()));
        assert!(!off.is_off(&url.serialised));
        window.finish();
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_refused_press_is_said_until_the_next_press_and_a_frame_does_not_clear_it() {
        let mut window = window();
        let refusal = Refusal::NoCard {
            claim: Claim::Expand,
        };
        window.heard(Heard::Drawn(Drawn::Pressed(
            Part::Start,
            Decided::Refused(refusal.clone()),
        )));
        assert_eq!(window.told(), Some(refusal.to_string().as_str()));
        assert_eq!(window.take_said(), vec![refusal.to_string()]);
        // The card's next frame paints the first screen again.
        window.heard(Heard::Drawn(Drawn::Painted(Part::Start, painted())));
        assert_eq!(
            window.scene().and_then(|scene| scene.start_sentence),
            Some(refusal.to_string().as_str()),
            "still said in the first screen's line"
        );
        assert!(window.take_redraw());
        // The next press decided clears it.
        window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::Nothing)));
        assert_eq!(window.told(), None);
        assert_eq!(window.scene().and_then(|scene| scene.start_sentence), None);
        assert!(window.take_said().is_empty());
        window.finish();
    }

    #[test]
    fn a_press_with_a_modifier_held_is_said_and_one_with_none_is_not() {
        use crate::keys::Modifier;
        let mut window = window();
        let sized = |width: u32, height: u32| Frame {
            width,
            height,
            pixels: vec![0; usize::try_from(width * height * 4).unwrap()],
        };
        window.heard(Heard::Drawn(Drawn::Painted(Part::Panel, sized(240, 820))));
        window.heard(Heard::Drawn(Drawn::Painted(Part::Start, sized(1040, 820))));
        let _ = window.take_redraw();
        // Over nothing of the window: nobody's, and nothing said.
        window.moved((-10.0, -10.0));
        window.held(Held::NOTHING.with(Modifier::Command));
        window.main_button_pressed();
        assert_eq!(window.told(), None);
        // Over the first screen, with ⌘ — as a tool that posted ⌘ and never
        // its release leaves it held (805).
        window.moved((640.0, 400.0));
        window.main_button_pressed();
        let said = "a press with ⌘ held was not acted on: a press with a modifier is \
                    nobody's yet (queue item 798)";
        assert_eq!(window.told(), Some(said));
        assert_eq!(window.take_said(), vec![said.to_owned()]);
        assert!(window.take_redraw(), "drawn in the first screen's line");
        assert_eq!(
            window.scene().and_then(|scene| scene.start_sentence),
            Some(said)
        );
        window.held(Held::NOTHING.with(Modifier::Shift).with(Modifier::Control));
        window.main_button_pressed();
        assert!(window.take_said()[0].starts_with("a press with ⌃⇧ held"));
        // With nothing held the press is handed on, and says nothing.
        window.held(Held::NOTHING);
        window.main_button_pressed();
        assert!(window.take_said().is_empty());
        window.finish();
    }

    #[test]
    fn a_press_is_aimed_by_the_offset_the_frame_it_lands_on_was_drawn_at() {
        let mut window = window();
        let reach = Reach {
            earlier: 400.0,
            newest: 626.0,
        };
        window.heard(Heard::Drawn(Drawn::Reached {
            turns: 2,
            down: -120.0,
            offered: 2,
            stands: Stands::InTheConversation,
            reach,
        }));
        assert_eq!(
            window.drawn_down.to_bits(),
            0.0_f32.to_bits(),
            "not till its frame"
        );
        assert_eq!(window.drawn_offered, 0, "not till its frame");
        window.heard(Heard::Drawn(Drawn::Painted(Part::Start, painted())));
        assert_eq!(window.drawn_down.to_bits(), (-120.0_f32).to_bits());
        assert_eq!(window.drawn_offered, 2, "the room shorter by two offers");
        // A frame with no turns, told with no reach, is not scrolled and has
        // the whole room.
        window.heard(Heard::Drawn(Drawn::Painted(Part::Start, painted())));
        assert_eq!(window.drawn_down.to_bits(), 0.0_f32.to_bits());
        assert_eq!(window.drawn_offered, 0);
        window.finish();
    }

    #[test]
    fn a_wheels_lines_are_glided_as_chromes_and_a_trackpads_pixels_followed() {
        use std::time::Duration;
        let mut window = window();
        let url = |address: &str| alo_url::parse(address).unwrap();
        window.heard(Heard::Chrome(Went::Opening(url("https://a.example/"))));
        window.heard(Heard::Chrome(Went::Opening(url("https://b.example/"))));
        window.heard(Heard::Drawn(Drawn::Reached {
            turns: 2,
            down: 0.0,
            offered: 0,
            stands: Stands::InTheConversation,
            reach: Reach {
                earlier: 900.0,
                newest: 1000.0,
            },
        }));
        let sized = |width: u32, height: u32| Frame {
            width,
            height,
            pixels: vec![0; usize::try_from(width * height * 4).unwrap()],
        };
        window.heard(Heard::Drawn(Drawn::Painted(Part::Panel, sized(240, 820))));
        window.heard(Heard::Drawn(Drawn::Painted(Part::Start, sized(1040, 820))));
        window.moved((640.0, 400.0));
        let down = |window: &ConversationWindow| {
            window
                .conversation()
                .thread()
                .map(|thread| thread.down().to_bits())
        };
        let ms = Duration::from_millis;
        let start = Instant::now();
        // One line back: glided over 200 ms, not jumped.
        window.wheeled_at(
            Turned::Lines {
                across: 0.0,
                down: 1.0,
            },
            start,
        );
        assert!(window.gliding());
        assert_eq!(down(&window), Some(0.0_f32.to_bits()), "not yet moved");
        window.glide(start + ms(100));
        assert_eq!(down(&window), Some((-20.0_f32).to_bits()), "half way");
        // A second line half way: on to 80 back, from where it stands.
        window.wheeled_at(
            Turned::Lines {
                across: 0.0,
                down: 1.0,
            },
            start + ms(100),
        );
        window.glide(start + ms(150));
        let moving = window.conversation().thread().map_or(0.0, Thread::down);
        assert!(moving < -20.0 && moving > -80.0, "{moving}");
        window.glide(start + ms(301));
        assert_eq!(down(&window), Some((-80.0_f32).to_bits()));
        assert!(!window.gliding(), "ended");
        // A trackpad's pixels mid-glide: followed at once, and the glide
        // ended.
        window.wheeled_at(
            Turned::Lines {
                across: 0.0,
                down: -1.0,
            },
            start + ms(400),
        );
        assert!(window.gliding());
        window.wheeled_at(
            Turned::Pixels {
                across: 0.0,
                down: 10.0,
            },
            start + ms(450),
        );
        assert!(!window.gliding());
        assert_eq!(down(&window), Some((-90.0_f32).to_bits()));
        // Never past the first turn's top.
        window.wheeled_at(
            Turned::Lines {
                across: 0.0,
                down: 100.0,
            },
            start + ms(500),
        );
        window.glide(start + ms(1000));
        assert_eq!(down(&window), Some((-900.0_f32).to_bits()));
        window.finish();
    }

    #[test]
    fn the_paging_keys_scroll_the_conversation_a_room_less_a_line_and_to_its_ends() {
        use crate::keys::{Modifier, Press};
        let mut window = window();
        let url = |address: &str| alo_url::parse(address).unwrap();
        window.heard(Heard::Chrome(Went::Opening(url("https://a.example/"))));
        window.heard(Heard::Chrome(Went::Opening(url("https://b.example/"))));
        window.heard(Heard::Drawn(Drawn::Reached {
            turns: 2,
            down: 0.0,
            offered: 0,
            stands: Stands::InTheConversation,
            reach: Reach {
                earlier: 900.0,
                newest: 1000.0,
            },
        }));
        let down = |window: &ConversationWindow| {
            window
                .conversation()
                .thread()
                .map(|thread| thread.down().to_bits())
        };
        let _ = window.take_redraw();
        // The room at 820 is 676: a page is 636.
        window.key(&Press::of(Key::PageUp), None);
        assert_eq!(down(&window), Some((-636.0_f32).to_bits()));
        assert!(window.take_redraw());
        window.key(&Press::of(Key::PageUp).repeating(), None);
        assert_eq!(down(&window), Some((-900.0_f32).to_bits()));
        window.key(&Press::of(Key::End), None);
        assert_eq!(down(&window), Some(324.0_f32.to_bits()));
        window.key(&Press::of(Key::Home), None);
        assert_eq!(down(&window), Some((-900.0_f32).to_bits()));
        window.key(&Press::of(Key::Space), Some(" "));
        assert_eq!(down(&window), Some((-264.0_f32).to_bits()));
        window.key(&Press::of(Key::Space).with(Modifier::Shift), Some(" "));
        assert_eq!(down(&window), Some((-900.0_f32).to_bits()));
        window.key(&Press::of(Key::PageDown), None);
        assert_eq!(down(&window), Some((-264.0_f32).to_bits()));
        // ⌘ with one is not a scroll.
        window.key(&Press::of(Key::End).with(Modifier::Command), None);
        assert_eq!(down(&window), Some((-264.0_f32).to_bits()));
        // The box given the keyboard, empty: still the conversation's; with
        // a word in it, a space is a space.
        window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::TheBox)));
        window.key(&Press::of(Key::PageDown), None);
        assert_eq!(down(&window), Some(324.0_f32.to_bits()));
        window.key(&Press::character("a"), Some("a"));
        window.key(&Press::of(Key::Space), Some(" "));
        window.key(&Press::of(Key::Home), None);
        assert_eq!(down(&window), Some(324.0_f32.to_bits()), "the box's keys");
        assert_eq!(
            window
                .conversation()
                .start(start_viewport((1280, 820), Scale::ONE), Scale::ONE)
                .typed,
            "a "
        );
        window.finish();
    }

    #[test]
    fn a_card_that_fails_or_closes_on_its_own_says_why_until_the_next_press() {
        let failures: [fn(alo_url::Url) -> Went; 2] = [
            |url| Went::Unopened {
                url,
                why: "Chrome did not answer within 30 s".to_owned(),
            },
            Went::Closed,
        ];
        for failed in failures {
            let mut window = window();
            window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::TheBox)));
            type_and_return(&mut window, "example.com");
            let url = window.conversation().going().cloned().unwrap();
            window.heard(Heard::Chrome(Went::Opening(url.clone())));
            // A press elsewhere, as on a site's title, takes the keyboard
            // from the box.
            window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::Nothing)));
            let focused = |window: &ConversationWindow| {
                let (size, scale) = window.measured().unwrap();
                window
                    .conversation()
                    .start(start_viewport(size, scale), scale)
                    .focused
            };
            assert!(!focused(&window));
            let failed = failed(url);
            let why = failed.said();
            window.heard(Heard::Chrome(failed));
            assert!(window.conversation().card().is_none(), "the card closed");
            assert!(focused(&window), "the box has the keyboard back");
            assert_eq!(window.take_said(), vec![why.clone()], "on the terminal");
            // Closing the card paints the first screen again.
            window.heard(Heard::Drawn(Drawn::Painted(Part::Start, painted())));
            assert_eq!(
                window.scene().and_then(|scene| scene.start_sentence),
                Some(why.as_str()),
                "still said in the first screen's line"
            );
            // The next address typed, with no press first, is gone to; and
            // choosing it clears what was said.
            type_and_return(&mut window, "example.org");
            assert_eq!(
                window.conversation().going().map(ToString::to_string),
                Some("https://example.org/".to_owned()),
                "the next address typed is gone to"
            );
            assert_eq!(window.told(), None);
            window.finish();
        }
        // Closed by the person, it is not a failure, and not kept.
        let mut window = window();
        window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::TheBox)));
        type_and_return(&mut window, "example.com");
        let url = window.conversation().going().cloned().unwrap();
        window.heard(Heard::Chrome(Went::Opening(url.clone())));
        window.heard(Heard::Chrome(Went::Shut(url)));
        assert_eq!(window.told(), None);
        assert!(window.take_said().is_empty());
        window.finish();
    }

    fn shown() -> Shown {
        Shown {
            by: "Chrome/156.0.0.0, driven by this browser".to_owned(),
            process: 7,
            profile: PathBuf::from("/nowhere"),
            window: None,
        }
    }

    /// Type `text` into the box and press Return, as the window reads keys.
    /// Queue item 774: the card's page box follows the column, so a card
    /// streamed is asked again at its new page box — the page laid out at
    /// it — as the window narrows or widens or the panel is hidden, at the
    /// window's scale; never while the window is covered or the card put
    /// away, and not again for a resize that leaves the page box as it was.
    #[test]
    fn a_cards_page_is_streamed_at_its_page_box_as_the_window_narrows_or_the_panel_hides() {
        let mut window = window();
        let resize = |window: &mut ConversationWindow, size: (u32, u32), scale_factor: f64| {
            window.measured_as(Measured {
                size,
                scale_factor,
                outer: Some((0, 0)),
            });
            window.ask_drawn();
        };
        let page = |window: &ConversationWindow| window.streaming.map(|streamed| streamed.page);
        let url = alo_url::parse("https://example.com/").unwrap();
        window.heard(Heard::Chrome(Went::Opening(url)));
        assert_eq!(window.streaming, None, "nothing asked before it is seen");
        window.seen(true);
        // The design's window: the whole page box.
        assert_eq!(window.streaming, Some(Streamed::at((878, 520), Scale::ONE)));
        // Wider: the column stays 880, nothing asked again.
        resize(&mut window, (1400, 820), 1.0);
        assert_eq!(page(&window), Some((878, 520)));
        // 967 wide: a room of 727, the column 663, the page box 661.
        resize(&mut window, (967, 820), 1.0);
        assert_eq!(window.streaming, Some(Streamed::at((661, 520), Scale::ONE)));
        // The panel hidden: the rail's 64 leaves 903, the page box 837.
        window.heard(Heard::Drawn(Drawn::Pressed(
            Part::Panel,
            Decided::HideThePanel,
        )));
        assert_eq!(page(&window), Some((837, 520)));
        // Covered, it is not streamed, nor asked again as it is resized.
        window.seen(false);
        assert_eq!(window.streaming, None);
        resize(&mut window, (720, 820), 1.0);
        assert_eq!(window.streaming, None);
        // Seen again, at 720 beside the rail: 656, the column 592.
        window.seen(true);
        assert_eq!(page(&window), Some((590, 520)));
        // The card put away, a resize asks nothing.
        window.heard(Heard::Chrome(Went::Shut(
            alo_url::parse("https://example.com/").unwrap(),
        )));
        resize(&mut window, (1280, 820), 1.0);
        assert_eq!(page(&window), Some((590, 520)));
        window.finish();
    }

    /// Queue item 772: the window dragged from a display at one to one at
    /// two — measured again at twice the device pixels, the same size in
    /// points — asks for the card's stream at two, the same page box; and
    /// dragged back, at one again. A card's Chromium is parked by a display
    /// at the scale each stream asks (`chrome_card.rs`).
    #[test]
    fn a_card_is_streamed_at_the_scale_of_the_display_the_window_is_moved_to() {
        // No scale named: the window's is its display's.
        let drawing = ConversationDrawing::start(
            Renderers::running("/nonexistent/alo-render", &[]),
            Box::new(|_| true),
        )
        .unwrap();
        let chrome = ChromeOpener::start(Chromium::Machines, Box::new(|_| true)).unwrap();
        let mut window = ConversationWindow::new(None, drawing, chrome);
        let moved = |window: &mut ConversationWindow, scale_factor: f64| {
            let times = if scale_factor > 1.5 { 2 } else { 1 };
            window.measured_as(Measured {
                size: (1280 * times, 820 * times),
                scale_factor,
                outer: Some((0, 0)),
            });
            window.ask_drawn();
        };
        moved(&mut window, 1.0);
        window.heard(Heard::Chrome(Went::Opening(
            alo_url::parse("https://example.com/").unwrap(),
        )));
        window.seen(true);
        assert_eq!(window.streaming, Some(Streamed::at((878, 520), Scale::ONE)));
        moved(&mut window, 2.0);
        assert_eq!(window.streaming, Some(Streamed::at((878, 520), Scale::TWO)));
        assert_eq!(
            window
                .streaming
                .map(|streamed| (streamed.width, streamed.height)),
            Some((1756, 1040))
        );
        moved(&mut window, 1.0);
        assert_eq!(window.streaming, Some(Streamed::at((878, 520), Scale::ONE)));
        window.finish();
    }

    /// Queue item 775: narrowed past 760 the panel folds to its rail on its
    /// own, the card's page streamed beside it as when a person hides it;
    /// widened again it unfolds — unless a person hid it — and *Show the
    /// panel* pressed while too narrow is kept, the panel shown over the
    /// conversation meanwhile (queue item 776).
    #[test]
    fn a_window_narrowed_past_760_folds_the_panel_and_widened_unfolds_it() {
        use crate::conversation_place::Side;
        let mut window = window();
        let measure = |window: &mut ConversationWindow, width: u32| {
            window.measured_as(Measured {
                size: (width, 820),
                scale_factor: 1.0,
                outer: Some((0, 0)),
            });
        };
        // As the event loop does on a resize: measured, then drawn.
        let resize = |window: &mut ConversationWindow, width: u32| {
            measure(window, width);
            window.ask_drawn();
        };
        let page = |window: &ConversationWindow| window.streaming.map(|streamed| streamed.page);
        let side = |window: &ConversationWindow| window.scene().map(|scene| scene.side);
        let url = alo_url::parse("https://example.com/").unwrap();
        window.heard(Heard::Chrome(Went::Opening(url)));
        window.seen(true);
        assert_eq!(page(&window), Some((878, 520)));
        let _ = window.take_redraw();
        // 760 wide the panel stands: a room of 520, the column 456.
        measure(&mut window, 760);
        assert_eq!(side(&window), Some(Side::Panel));
        assert!(!window.take_redraw(), "nothing folded");
        window.ask_drawn();
        assert_eq!(page(&window), Some((454, 520)));
        // 759 folds it: the rail's 64 leaves 695, the column 631, and the
        // page is streamed at 629 by the measure alone.
        measure(&mut window, 759);
        assert_eq!(side(&window), Some(Side::Rail));
        assert!(window.take_redraw(), "folded, drawn again");
        assert_eq!(page(&window), Some((629, 520)));
        assert!(window.conversation().showing().hidden);
        // The least window, 600: 536 beside the rail, the column 472.
        resize(&mut window, 600);
        assert_eq!(page(&window), Some((470, 520)));
        // Widened past the fold it unfolds, nobody having hidden it.
        resize(&mut window, 967);
        assert_eq!(side(&window), Some(Side::Panel));
        assert_eq!(page(&window), Some((661, 520)));
        // Hidden by a person, it stays the rail through a narrow window
        // and back.
        window.heard(Heard::Drawn(Drawn::Pressed(
            Part::Panel,
            Decided::HideThePanel,
        )));
        resize(&mut window, 700);
        resize(&mut window, 1280);
        assert_eq!(side(&window), Some(Side::Rail));
        assert_eq!(page(&window), Some((878, 520)));
        // Shown while too narrow: the conversation still beside the rail,
        // the panel over it as the sheet (queue item 776), nothing said;
        // and the panel back beside it, no sheet, once the window is 760.
        resize(&mut window, 700);
        let _ = window.take_said();
        window.heard(Heard::Drawn(Drawn::Pressed(
            Part::Panel,
            Decided::ShowThePanel,
        )));
        assert_eq!(side(&window), Some(Side::Rail));
        assert!(window.conversation().sheet());
        assert_eq!(window.told(), None);
        assert!(window.take_said().is_empty());
        resize(&mut window, 760);
        assert_eq!(side(&window), Some(Side::Panel));
        assert!(!window.conversation().sheet());
        assert_eq!(page(&window), Some((454, 520)));
        window.finish();
    }

    /// A card open on example.com in `window`, seen, and asking alo set up
    /// with no model behind it: a run started is the conversation's alone,
    /// which is all the window's layout follows.
    fn a_card_alo_can_be_asked_about(window: &mut ConversationWindow) {
        window.conversation.set_agent(SetUp::Ready);
        let url = alo_url::parse("https://example.com/").unwrap();
        window.heard(Heard::Chrome(Went::Opening(url)));
        window.seen(true);
    }

    /// Ask alo `asked` on the card, as Return on words does while it is open.
    fn ask_alo(window: &mut ConversationWindow, asked: &str) {
        window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::TheBox)));
        type_and_return(window, asked);
        assert!(window.conversation().at_work(), "{asked} started a run");
    }

    /// *Stop* pressed on the run at work: the run ends.
    fn stop(window: &mut ConversationWindow) {
        window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::Stop(1))));
        assert!(!window.conversation().at_work(), "stopped");
    }

    /// Queue item 819: while alo's agent works on the card the panel is the
    /// rail and the card's page is streamed at the split view's page box —
    /// 798 × 740 in the design's window, 1198 × 970 in a wide one, 720 ×
    /// 740 at the least it splits in — and back at the conversation's once
    /// the run ends; one pixel narrower than the least, it is the
    /// conversation's page box all along, the panel still the rail.
    #[test]
    fn while_alo_works_the_page_is_streamed_at_the_split_views_page_box_and_after_at_the_cards() {
        use crate::conversation_place::Side;
        let side = |window: &ConversationWindow| window.scene().map(|scene| scene.side);
        let page = |window: &ConversationWindow| window.streaming.map(|streamed| streamed.page);
        for (size, split, after) in [
            ((1280, 820), (798, 740), (878, 520)),
            ((1680, 1050), (1198, 970), (878, 520)),
            ((1202, 820), (720, 740), (878, 520)),
            ((1201, 820), (878, 520), (878, 520)),
        ] {
            let mut window = window();
            window.measured_as(Measured {
                size,
                scale_factor: 1.0,
                outer: Some((0, 0)),
            });
            a_card_alo_can_be_asked_about(&mut window);
            assert_eq!(side(&window), Some(Side::Panel), "{size:?}");
            ask_alo(&mut window, "find the contact page");
            assert_eq!(side(&window), Some(Side::Rail), "{size:?}");
            assert_eq!(
                window.streaming,
                Some(Streamed::at(split, Scale::ONE)),
                "{size:?}"
            );
            stop(&mut window);
            assert_eq!(side(&window), Some(Side::Panel), "{size:?}");
            // Beside the panel again: the column 880 wide from 1040 of room
            // and more, and at 1201 and 1202 too.
            assert_eq!(page(&window), Some(after), "{size:?}");
            window.finish();
        }
    }

    /// At two the same window asks Chrome for twice the device pixels of the
    /// same page box.
    #[test]
    fn at_two_the_split_views_page_box_is_twice_the_device_pixels() {
        let drawing = ConversationDrawing::start(
            Renderers::running("/nonexistent/alo-render", &[]),
            Box::new(|_| true),
        )
        .unwrap();
        let chrome = ChromeOpener::start(Chromium::Machines, Box::new(|_| true)).unwrap();
        let mut window = ConversationWindow::new(Some(Scale::TWO), drawing, chrome);
        window.measured_as(Measured {
            size: (2560, 1640),
            scale_factor: 2.0,
            outer: Some((0, 0)),
        });
        a_card_alo_can_be_asked_about(&mut window);
        ask_alo(&mut window, "find it");
        assert_eq!(window.streaming, Some(Streamed::at((798, 740), Scale::TWO)));
        assert_eq!(
            window
                .streaming
                .map(|streamed| (streamed.width, streamed.height)),
            Some((1596, 1480))
        );
        window.finish();
    }

    /// Queue item 819: the run folds the panel to the rail without writing
    /// over what a person chose. Shown before a run, it is shown after it;
    /// hidden before, hidden after; hidden during it, hidden after; *Show
    /// the panel* pressed during it is kept, said, and the panel is back as
    /// the run ends, the line with it gone.
    #[test]
    fn a_persons_panel_choice_is_kept_across_a_run() {
        use crate::conversation_place::Side;
        let side = |window: &ConversationWindow| window.scene().map(|scene| scene.side);
        let press = |window: &mut ConversationWindow, pressed: Decided| {
            window.heard(Heard::Drawn(Drawn::Pressed(Part::Panel, pressed)));
        };
        let mut window = window();
        a_card_alo_can_be_asked_about(&mut window);
        // Shown before: shown after.
        ask_alo(&mut window, "one");
        stop(&mut window);
        assert_eq!(side(&window), Some(Side::Panel));
        // Hidden during the run: hidden after it.
        ask_alo(&mut window, "two");
        press(&mut window, Decided::HideThePanel);
        assert_eq!(side(&window), Some(Side::Rail));
        stop(&mut window);
        assert_eq!(side(&window), Some(Side::Rail));
        assert!(window.conversation().fold().hidden());
        // Hidden before: the rail through the run and after it.
        ask_alo(&mut window, "three");
        stop(&mut window);
        assert_eq!(side(&window), Some(Side::Rail));
        // Shown during the run: still the rail, said why; the panel back as
        // it ends, and the line gone.
        ask_alo(&mut window, "four");
        let _ = window.take_said();
        press(&mut window, Decided::ShowThePanel);
        assert_eq!(side(&window), Some(Side::Rail));
        let said = conversation_fold::at_work();
        assert_eq!(window.told(), Some(said.as_str()));
        assert_eq!(window.take_said(), vec![said]);
        assert!(!window.conversation().fold().hidden(), "the choice kept");
        stop(&mut window);
        assert_eq!(side(&window), Some(Side::Panel));
        assert_eq!(window.told(), None, "no longer so, no longer said");
        window.finish();
    }

    /// Queue item 819: a press on the split view's page box is aimed at the
    /// page where the split view drew it — 81, 63 of the design's window —
    /// once the first screen painted it so, and taking over ends the run.
    #[test]
    fn a_press_on_the_split_views_page_is_the_pages_and_takes_over() {
        let mut window = window();
        a_card_alo_can_be_asked_about(&mut window);
        window.heard(Heard::Chrome(Went::Framed(vec![1])));
        ask_alo(&mut window, "find it");
        // Painted as the split view: the drawing thread says so before the
        // frame.
        window.heard(Heard::Drawn(Drawn::Reached {
            turns: 1,
            down: 0.0,
            offered: 0,
            stands: Stands::Split,
            reach: Reach {
                earlier: 0.0,
                newest: 0.0,
            },
        }));
        // Beside the rail, the first screen 1216 × 820.
        let whole = Frame {
            width: 1216,
            height: 820,
            pixels: vec![0; 1216 * 820 * 4],
        };
        window.heard(Heard::Drawn(Drawn::Painted(Part::Start, whole)));
        assert_eq!(window.drawn_stands, Stands::Split);
        assert_eq!(
            window.aimed((81.0, 63.0)),
            Some(card_aim::Aimed::Page { x: 0.0, y: 0.0 })
        );
        // Where the conversation's page box was, (321, 145) beside the
        // panel, is the split's page too, at its own place.
        assert_eq!(
            window.aimed((321.0, 145.0)),
            Some(card_aim::Aimed::Page { x: 240.0, y: 82.0 })
        );
        window.moved((400.0, 400.0));
        window.main_button_pressed();
        assert!(!window.conversation().at_work(), "taken over");
        window.finish();
    }

    fn type_and_return(window: &mut ConversationWindow, text: &str) {
        for letter in text.chars() {
            let written = letter.to_string();
            window.key(&Press::character(&written), Some(&written));
        }
        window.key(&Press::of(Key::Return), None);
    }

    #[test]
    fn the_front_is_taken_back_once_a_card_opens_from_a_window_that_had_it() {
        for had_it in [true, false] {
            let mut window = window();
            window.focused(had_it);
            window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::TheBox)));
            type_and_return(&mut window, "example.com");
            let url = window.conversation().going().cloned().unwrap();
            window.heard(Heard::Chrome(Went::Opening(url.clone())));
            assert!(!window.take_front(), "nothing taken yet");
            // Chromium took it as it showed its window: taken back at once.
            window.focused(false);
            assert_eq!(window.take_front(), had_it);
            assert!(!window.take_front(), "once");
            // Not had back by the time the card is open: taken again.
            window.heard(Heard::Chrome(Went::Opened {
                url: url.clone(),
                shown: shown(),
            }));
            assert_eq!(window.take_front(), had_it);
            // Had back, nothing more; and once open, a window losing the
            // front to another program keeps it lost.
            window.focused(false);
            assert!(!window.take_front());
            window.finish();
        }
        // Given back before the card opened, not asked again.
        let mut window = window();
        window.focused(true);
        window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::TheBox)));
        type_and_return(&mut window, "example.com");
        let url = window.conversation().going().cloned().unwrap();
        window.focused(false);
        assert!(window.take_front());
        window.focused(true);
        window.heard(Heard::Chrome(Went::Opened {
            url,
            shown: shown(),
        }));
        assert!(!window.take_front());
        window.finish();
    }

    #[test]
    fn a_search_not_set_up_says_why_and_one_set_up_says_what_came_back() {
        let (tell, told) = std::sync::mpsc::channel();
        let searcher = Searcher::start(
            Err("no Brave Search key in this Mac's keychain (alo-brave-search)".to_owned()),
            Box::new(move |searched| tell.send(searched).is_ok()),
        )
        .unwrap();
        let mut unset = window().with_search(searcher);
        unset.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::TheBox)));
        type_and_return(&mut unset, "what is x");
        assert_eq!(
            unset.scene().and_then(|scene| scene.start_sentence),
            Some(
                "searching is not set up yet: no Brave Search key in this Mac's keychain (alo-brave-search)"
            )
        );
        assert!(told.try_recv().is_err(), "nothing searched");
        unset.finish();

        // Set up, as the conversation holds it, and answered.
        let mut window = window();
        window
            .conversation
            .set_search(alo_renderer::results::SetUp::Ready);
        window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::TheBox)));
        type_and_return(&mut window, "what is x");
        assert_eq!(
            window.scene().and_then(|scene| scene.start_sentence),
            Some("searching Brave Search for \u{201c}what is x\u{201d}")
        );
        window.heard(Heard::Searched(Searched::sites(
            1,
            "what is x",
            Showing::Unanswered(crate::brave_search::unanswered(
                429,
                crate::search_key::Held::Keychain,
            )),
        )));
        assert_eq!(
            window.scene().and_then(|scene| scene.start_sentence),
            Some(crate::brave_search::unanswered(429, crate::search_key::Held::Keychain).as_str())
        );
        assert_eq!(
            window
                .conversation()
                .results()
                .map(|results| results.showing.clone()),
            Some(Showing::Unanswered(crate::brave_search::unanswered(
                429,
                crate::search_key::Held::Keychain
            )))
        );
        window.finish();
        assert_eq!(window.unfinished(), None);
    }

    #[test]
    fn return_on_an_address_empties_the_box_and_the_card_follows_its_page() {
        let mut window = window();
        window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::TheBox)));
        type_and_return(&mut window, "example.com");
        let (size, scale) = window.measured().unwrap();
        let start = window
            .conversation()
            .start(start_viewport(size, scale), scale);
        assert_eq!(start.typed, "");
        assert!(start.offers.is_empty());
        let url = window.conversation().going().cloned().unwrap();
        window.heard(Heard::Chrome(Went::Opening(url)));
        let talk = alo_url::parse("https://example.com/wiki/Talk:Main_Page").unwrap();
        window.heard(Heard::Chrome(Went::Moved(talk.clone())));
        assert_eq!(
            window.conversation().card().map(|card| card.url.clone()),
            Some(talk)
        );
        window.finish();
    }

    #[test]
    fn the_card_is_told_where_its_page_can_go_and_the_line_keeps_what_it_said() {
        let mut window = window();
        window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::TheBox)));
        type_and_return(&mut window, "example.com");
        let url = window.conversation().going().cloned().unwrap();
        window.heard(Heard::Chrome(Went::Opening(url)));
        let opening = window.start_said.clone();
        let _ = window.take_redraw();
        window.heard(Heard::Chrome(Went::Travel(Travel {
            back: true,
            forward: false,
        })));
        assert_eq!(
            window
                .conversation()
                .card()
                .map(|card| (card.back, card.forward)),
            Some((true, false))
        );
        assert!(window.take_redraw(), "the card is drawn again");
        assert_eq!(window.start_said, opening);
        // A press on Back, decided, is carried out and refuses nothing.
        window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::Back)));
        assert_eq!(window.told(), None);
        window.finish();
    }

    #[test]
    fn the_card_is_told_its_page_loaded_and_the_line_keeps_what_it_said() {
        let mut window = window();
        window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::TheBox)));
        type_and_return(&mut window, "example.com");
        let url = window.conversation().going().cloned().unwrap();
        window.heard(Heard::Chrome(Went::Opening(url)));
        let opening = window.start_said.clone();
        assert!(opening.is_some());
        assert_eq!(
            window.conversation().card().map(|card| card.loading),
            Some(true)
        );
        let _ = window.take_redraw();
        window.heard(Heard::Chrome(Went::Loading(false)));
        assert_eq!(
            window.conversation().card().map(|card| card.loading),
            Some(false)
        );
        assert!(window.take_redraw(), "the card is drawn again");
        // The card says it; the first screen's line is left as it was.
        assert_eq!(window.start_said, opening);
        assert_eq!(window.told(), None);
        window.finish();
    }

    #[test]
    fn a_request_the_lists_refused_is_said_on_the_terminal_alone() {
        let mut window = window();
        window.heard(Heard::Drawn(Drawn::Pressed(Part::Start, Decided::TheBox)));
        type_and_return(&mut window, "example.com");
        let url = window.conversation().going().cloned().unwrap();
        window.heard(Heard::Chrome(Went::Opening(url)));
        let opening = window.start_said.clone();
        let _ = window.take_said();
        let blocked = Went::Blocked {
            url: "https://example.com/fbevents.js".to_owned(),
            by: crate::blocking::Blocked {
                list: Some("EasyPrivacy"),
                line: Some(973),
                rule: Some("/fbevents.js".to_owned()),
            },
        };
        let said = blocked.said();
        window.heard(Heard::Chrome(blocked));
        assert_eq!(window.take_said(), vec![said]);
        // Nothing of the window changes: its line, what a press came to.
        assert_eq!(window.start_said, opening);
        assert_eq!(window.told(), None);
        window.finish();
    }

    /// A window on a clock of the test's own, at `at` to begin with: what
    /// the slide and the fade are read from (queue items 776, 793).
    fn on_a_clock(at: Instant) -> (ConversationWindow, std::rc::Rc<std::cell::Cell<Instant>>) {
        let clock = std::rc::Rc::new(std::cell::Cell::new(at));
        let reading = std::rc::Rc::clone(&clock);
        (window().with_clock(move || reading.get()), clock)
    }

    fn frame_of(width: u32, height: u32) -> Frame {
        Frame {
            width,
            height,
            pixels: vec![0; usize::try_from(width * height * 4).unwrap_or(0)],
        }
    }

    /// Queue item 776: below 760, *Show the panel* slides the whole panel
    /// over the conversation, over a shade, in 200 ms; a press on it is the
    /// panel's, a press outside it closes it and is nobody else's, and so
    /// do Escape and a chat chosen in it; nothing under it scrolls.
    #[test]
    fn in_a_narrow_window_the_panel_slides_over_the_conversation_and_closes() {
        use std::time::Duration;
        let ms = Duration::from_millis;
        let t0 = Instant::now();
        let (mut window, clock) = on_a_clock(t0);
        window.measured_as(Measured {
            size: (700, 820),
            scale_factor: 1.0,
            outer: Some((0, 0)),
        });
        let show = |window: &mut ConversationWindow| {
            window.heard(Heard::Drawn(Drawn::Pressed(
                Part::Panel,
                Decided::ShowThePanel,
            )));
        };
        show(&mut window);
        assert!(window.conversation().sheet());
        assert_eq!(window.told(), None, "nothing said: the sheet answers");
        // Laid out whole for the sheet, the first screen still beside the
        // rail.
        assert_eq!(window.conversation().panel_width(), 240);
        assert!(!window.conversation().showing().hidden);
        let sheet = |window: &ConversationWindow| {
            window.scene().and_then(|scene| {
                scene
                    .sheet
                    .map(|sheet| (sheet.left, format!("{:.3}", sheet.shade)))
            })
        };
        assert_eq!(sheet(&window), None, "nothing to slide until it is painted");
        window.heard(Heard::Drawn(Drawn::Painted(
            Part::Panel,
            frame_of(240, 820),
        )));
        window.heard(Heard::Drawn(Drawn::Painted(
            Part::Start,
            frame_of(636, 820),
        )));
        assert_eq!(sheet(&window), None, "at its first instant none is out");
        for (after, left, shade) in [
            (1, -173, "0.005"),
            (50, -74, "0.185"),
            (100, -22, "0.280"),
            (200, 0, "0.320"),
        ] {
            clock.set(t0 + ms(after));
            assert_eq!(sheet(&window), Some((left, shade.to_owned())), "{after} ms");
        }
        assert!(
            window.wake_at().is_none(),
            "out and still: nothing to wake for"
        );
        clock.set(t0 + ms(100));
        assert!(
            window.wake_at().is_some_and(|wake| wake <= t0 + ms(117)),
            "each frame while it slides"
        );
        clock.set(t0 + ms(200));
        // A press on it is the panel's: it stays out. Nothing under it is
        // scrolled by a wheel.
        window.moved_at((120.0, 300.0), t0 + ms(200));
        window.main_button_pressed_at(t0 + ms(200));
        assert!(window.conversation().sheet());
        window.wheeled_at(
            Turned::Lines {
                across: 0.0,
                down: 3.0,
            },
            t0 + ms(200),
        );
        assert!(!window.gliding(), "nothing under the sheet scrolls");
        // A press outside it, over the shade, closes it; it slides back.
        window.moved_at((500.0, 300.0), t0 + ms(300));
        window.main_button_pressed_at(t0 + ms(300));
        assert!(!window.conversation().sheet());
        assert!(window.conversation().showing().hidden, "the rail again");
        clock.set(t0 + ms(350));
        assert_eq!(sheet(&window).map(|(left, _)| left), Some(-3), "going back");
        clock.set(t0 + ms(500));
        assert_eq!(sheet(&window), None, "back in");
        window.tick(t0 + ms(500));
        assert!(window.sheet_frame.is_none(), "let go");
        window.finish();
    }

    /// Queue item 776: Escape closes the sheet, so does a chat chosen in it
    /// — which leaves no row hovered for the rail to name — and widened
    /// past the fold it is gone at once.
    #[test]
    fn the_sheet_closed_by_escape_a_chat_chosen_or_the_window_widened() {
        let (mut window, _clock) = on_a_clock(Instant::now());
        window.measured_as(Measured {
            size: (700, 820),
            scale_factor: 1.0,
            outer: Some((0, 0)),
        });
        let show = |window: &mut ConversationWindow| {
            window.heard(Heard::Drawn(Drawn::Pressed(
                Part::Panel,
                Decided::ShowThePanel,
            )));
        };
        let sheet = |window: &ConversationWindow| {
            window
                .scene()
                .and_then(|scene| scene.sheet.map(|sheet| sheet.left))
        };
        // Escape closes it, whatever has the keyboard.
        show(&mut window);
        assert!(window.conversation().sheet());
        window.key(&Press::of(Key::Escape), Some("\u{1b}"));
        assert!(!window.conversation().sheet());
        // So does a chat chosen in it.
        show(&mut window);
        let chat = window.conversation().showing().chats[0];
        assert!(
            window.conversation.hover(Some(chat)),
            "its row hovered in the sheet"
        );
        window.heard(Heard::Drawn(Drawn::Pressed(
            Part::Panel,
            Decided::Chat(chat),
        )));
        assert!(!window.conversation().sheet());
        // Closed, the row hovered in it is not named on the rail (found by
        // hand: the rail named it, the pointer elsewhere).
        assert!(!window.conversation().rail_named());
        // Widened past the fold, the panel stands beside the conversation
        // and the sheet is gone at once.
        show(&mut window);
        window.measured_as(Measured {
            size: (900, 820),
            scale_factor: 1.0,
            outer: Some((0, 0)),
        });
        assert!(!window.conversation().sheet());
        assert_eq!(sheet(&window), None);
        window.finish();
    }

    /// Queue item 793: the bar is whole while the conversation scrolls and
    /// for a second after, then fades out in 300 ms, drawn at each instant
    /// as faded as it is; the pointer near it brings it back whole.
    #[test]
    fn the_conversations_bar_fades_a_second_after_it_stops_and_comes_back_near() {
        use std::time::Duration;
        let ms = Duration::from_millis;
        let t0 = Instant::now();
        let (mut window, clock) = on_a_clock(t0);
        let url = |address: &str| alo_url::parse(address).unwrap();
        window.heard(Heard::Chrome(Went::Opening(url("https://a.example/"))));
        window.heard(Heard::Chrome(Went::Opening(url("https://b.example/"))));
        window.heard(Heard::Drawn(Drawn::Reached {
            turns: 2,
            down: 0.0,
            offered: 0,
            stands: Stands::InTheConversation,
            reach: Reach {
                earlier: 900.0,
                newest: 1000.0,
            },
        }));
        window.heard(Heard::Drawn(Drawn::Painted(
            Part::Panel,
            frame_of(240, 820),
        )));
        window.heard(Heard::Drawn(Drawn::Painted(
            Part::Start,
            frame_of(1040, 820),
        )));
        let seen =
            |window: &ConversationWindow| window.seen_drawn().map(|seen| format!("{seen:.2}"));
        // Its turns came at t0, which moved it: whole, then faded out.
        assert_eq!(seen(&window), Some("1.00".to_owned()));
        clock.set(t0 + ms(1_300));
        window.tick(t0 + ms(1_300));
        assert_eq!(seen(&window), Some("0.00".to_owned()));
        // Scrolled by a trackpad's pixels at t1: whole again at once.
        let t1 = t0 + ms(2_000);
        clock.set(t1);
        window.moved_at((640.0, 400.0), t1);
        window.wheeled_at(
            Turned::Pixels {
                across: 0.0,
                down: 10.0,
            },
            t1,
        );
        assert_eq!(seen(&window), Some("1.00".to_owned()));
        for (after, expected) in [(999, "1.00"), (1_150, "0.50"), (1_300, "0.00")] {
            clock.set(t1 + ms(after));
            window.tick(t1 + ms(after));
            assert_eq!(seen(&window), Some(expected.to_owned()), "{after} ms");
        }
        // Woken as its second ends, each frame of the fade, and not after.
        clock.set(t1 + ms(500));
        assert_eq!(window.wake_at(), Some(t1 + ms(1_000)));
        clock.set(t1 + ms(1_100));
        assert!(
            window
                .wake_at()
                .is_some_and(|wake| wake > t1 + ms(1_100) && wake <= t1 + ms(1_117))
        );
        clock.set(t1 + ms(1_300));
        assert_eq!(window.wake_at(), None);
        // The pointer near it, 24 either side of 973 across the first
        // screen — 1213 in the window, beside the panel's 240: whole.
        window.moved_at((1_240.0, 400.0), t1 + ms(1_300));
        assert_eq!(seen(&window), Some("1.00".to_owned()));
        assert_eq!(window.wake_at(), None, "nothing to fade while it is near");
        // Not near: 25 past its right edge, or above the room.
        let t2 = t1 + ms(5_000);
        clock.set(t2);
        window.moved_at((1_244.0, 400.0), t2);
        clock.set(t2 + ms(1_300));
        window.tick(t2 + ms(1_300));
        assert_eq!(seen(&window), Some("0.00".to_owned()), "left at t2, faded");
        window.moved_at((1_216.0, 20.0), t2 + ms(1_300));
        assert_eq!(seen(&window), Some("0.00".to_owned()), "above the room");
        window.moved_at((1_216.0, 400.0), t2 + ms(1_300));
        assert_eq!(seen(&window), Some("1.00".to_owned()), "beside it");
        window.finish();
    }
}
