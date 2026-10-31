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
use crate::card_input::{CardInput, Went as Button};
use crate::card_keys;
use crate::conversation::{Conversation, Pressed as Decided};
use crate::conversation_box::{self, Typed};
use crate::conversation_chrome::{ChromeOpener, Placing, Streamed, Went};
use crate::conversation_compose::ConversationScene;
use crate::conversation_drawing::{Asked, ConversationDrawing, Drawn, Part};
use crate::conversation_place::{ConversationPlacement, panel_viewport, start_viewport};
use crate::conversation_search::{Searched, Searcher};
use crate::keys::{Held, Press};
use crate::scale::Scale;
use crate::typing;
use alo_layout::Size;
use alo_renderer::Frame;
use alo_renderer::results::{PROVIDER, Showing};

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
    /// Whether the main button went down over the card's page and is still
    /// held: its moves and its release are the page's (queue item 718).
    on_the_page: bool,
    /// The modifier keys held.
    held: Held,
    /// Whether the window has the keyboard, and the front is to be taken
    /// back.
    front: Front,
    /// Whether the window is to be drawn again.
    redraw: bool,
    /// Lines to say on the terminal.
    said: Vec<String>,
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
            on_the_page: false,
            held: Held::NOTHING,
            front: Front::default(),
            redraw: false,
            said: Vec::new(),
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
    /// time it is resized, moved or given another scale.
    pub fn measured_as(&mut self, measured: Measured) {
        self.window = Some(measured);
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
        let Some(drawing) = self.drawing.as_mut() else {
            return;
        };
        drawing.draw(Asked {
            panel: self
                .conversation
                .panel(panel_viewport(window, scale), scale),
            start: self
                .conversation
                .start(start_viewport(window, scale), scale),
            showing: self.conversation.showing(),
        });
    }

    /// What the window is drawn from now, at its size and scale: the first
    /// screen's line says why its renderer painted nothing, or what became
    /// of an address, and otherwise what the last press came to when it was
    /// refused.
    pub fn scene(&self) -> Option<ConversationScene<'_>> {
        let (window, scale) = self.measured()?;
        Some(ConversationScene {
            window,
            scale,
            panel: self.panel.as_ref(),
            panel_sentence: self.panel_said.as_deref(),
            start: self.start.as_ref(),
            start_sentence: self.start_said.as_deref().or(self.told.as_deref()),
        })
    }

    /// The first screen's viewport while it shows the card's picture of its
    /// page, which is when the page can be pressed (queue item 718).
    fn card_on_screen(&self) -> Option<Size> {
        let (window, scale) = self.measured()?;
        self.conversation
            .card()
            .and_then(|card| card.frame.as_ref())
            .map(|_| start_viewport(window, scale))
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
        Some(card_aim::on_the_page(
            &placed,
            scene.scale,
            self.card_on_screen()?,
            at,
        ))
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

    /// The cursor left the window.
    pub fn cursor_left(&mut self) {
        self.cursor = None;
    }

    /// The main button went down with nothing held: a press over the card's
    /// page is the page's, sent to it, and gives it the keyboard (queue
    /// item 718); a press over a part is handed to the thread that holds
    /// its renderer, to be claimed and decided.
    pub fn main_button_pressed(&mut self) {
        if self.held != Held::NOTHING {
            return;
        }
        let Some(at) = self.cursor else {
            return;
        };
        match self.aimed(at) {
            Some(card_aim::Aimed::Page { x, y }) => {
                self.on_the_page = true;
                if self.conversation.press_the_page() {
                    self.ask_drawn();
                }
                self.to_the_page(CardInput::Pointer {
                    went: Button::Pressed,
                    x,
                    y,
                });
            }
            Some(card_aim::Aimed::Part(part, on)) => {
                if let Some(drawing) = self.drawing.as_mut() {
                    drawing.press(part, on);
                }
            }
            None => {}
        }
    }

    /// The cursor moved to `at`: the page's, as a drag, while a press on
    /// it is held.
    pub fn moved(&mut self, at: (f64, f64)) {
        self.cursor = Some(at);
        if !self.on_the_page {
            return;
        }
        match self.on_the_card(at) {
            Some((x, y)) => self.to_the_page(CardInput::Pointer {
                went: Button::Dragged,
                x,
                y,
            }),
            // The card went while the button was held.
            None => self.on_the_page = false,
        }
    }

    /// The main button came up: the page's, where the cursor is, after a
    /// press on it.
    pub fn main_button_released(&mut self) {
        if !std::mem::replace(&mut self.on_the_page, false) {
            return;
        }
        if let Some((x, y)) = self.cursor.and_then(|at| self.on_the_card(at)) {
            self.to_the_page(CardInput::Pointer {
                went: Button::Released,
                x,
                y,
            });
        }
    }

    /// A wheel or a trackpad turned: the page's, scrolled, when the cursor
    /// is over the card's page; nobody's anywhere else, since neither part
    /// scrolls.
    pub fn wheeled(&self, turned: Turned) {
        let Some(at) = self.cursor else {
            return;
        };
        let (Some(card_aim::Aimed::Page { x, y }), Some((_, scale))) =
            (self.aimed(at), self.measured())
        else {
            return;
        };
        let (right, down) = card_aim::scrolled(turned, scale);
        self.to_the_page(CardInput::Wheel { x, y, right, down });
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
        let go = matches!(pressed, Decided::Go | Decided::Found(_));
        let had_a_card = self.conversation.card().is_some();
        self.card_pressed(&pressed);
        if self.conversation.act(pressed) {
            self.ask_drawn();
        }
        if go && let Some(chosen) = self.conversation.chosen() {
            self.start_said = Some(conversation_box::said(chosen, self.searching()));
            self.go();
            self.search(had_a_card);
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
    /// provider (queue item 708) — the card it put away closed, its page
    /// with it, when one was shown. Returns at once.
    fn search(&mut self, had_a_card: bool) {
        let Some((number, words)) = self.conversation.searching() else {
            return;
        };
        if let Some(searcher) = &self.searcher {
            searcher.search(number, words);
        }
        if had_a_card && let Some(chrome) = &self.chrome {
            chrome.shut();
        }
    }

    /// What came of a search: shown when it is the newest, and the first
    /// screen's line saying so.
    fn searched(&mut self, searched: Searched) {
        let line = match &searched.showing {
            Showing::Found(found) if found.is_empty() => format!(
                "{PROVIDER} found no sites for \u{201c}{}\u{201d}",
                searched.words
            ),
            Showing::Found(found) => format!(
                "{PROVIDER} found {} sites for \u{201c}{}\u{201d}",
                found.len(),
                searched.words
            ),
            Showing::Unanswered(why) => why.clone(),
            Showing::Searching => return,
        };
        if self.conversation.searched(searched) {
            self.start_said = Some(line);
            self.ask_drawn();
        }
    }

    /// Open where it was decided to go, if anywhere, in Chrome, as a card in
    /// the conversation streamed at the window's scale (ADR 0076 B, queue
    /// item 717). Returns at once.
    fn go(&mut self) {
        let Some(url) = self.conversation.going().cloned() else {
            return;
        };
        let Some((_, scale)) = self.measured() else {
            return;
        };
        self.front.owed = self.front.focused;
        if let Some(chrome) = &self.chrome {
            chrome.open(url, Streamed::at(scale));
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
                ))
            })
            .unwrap_or(Placing::APART)
    }

    /// What the card's buttons were decided to be, carried out by the
    /// thread holding its Chrome. *Back* and *Forward* are claimed only when
    /// there is somewhere to go, which nothing yet reads (queue item 737).
    fn card_pressed(&self, pressed: &Decided) {
        let Some(chrome) = &self.chrome else {
            return;
        };
        match pressed {
            Decided::Expand => chrome.expand(self.over_the_first_screen()),
            Decided::CloseTheCard => chrome.shut(),
            _ => {}
        }
    }

    /// The window can be seen, or cannot: the card's page is streamed at
    /// the window's scale while it can, and not while it cannot (ADR 0076:
    /// only while the card is on the screen). A new scale is frames of
    /// another size, asked for the same way.
    pub fn seen(&self, seen: bool) {
        let (Some(chrome), Some((_, scale))) = (&self.chrome, self.measured()) else {
            return;
        };
        if self.conversation.card().is_some() {
            chrome.stream(seen.then(|| Streamed::at(scale)));
        }
    }

    /// What became of an address chosen, from the thread holding Chrome:
    /// the card shown, given its newest frame or where its page is now, or
    /// taken away, and the first screen's line saying so.
    fn went(&mut self, went: Went) {
        match &went {
            Went::Framed(_) | Went::Unstreamed(_) | Went::Moved(_) => {}
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
                self.conversation.open_card(url);
                true
            }
            Went::Framed(png) => self.conversation.framed(png),
            Went::Moved(url) => self.conversation.moved_card(url),
            Went::Unopened { .. } | Went::Closed(_) => {
                self.front.owed = false;
                self.conversation.card_failed()
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
            Went::Unstreamed(_) => false,
        };
        if changed {
            self.ask_drawn();
        }
    }

    /// What a thread told the window: acted on, and the window drawn again.
    pub fn heard(&mut self, heard: Heard) {
        self.redraw = true;
        match heard {
            Heard::Chrome(went) => self.went(went),
            Heard::Searched(searched) => self.searched(searched),
            Heard::Drawn(Drawn::Painted(Part::Panel, frame)) => {
                self.panel = Some(frame);
                self.panel_said = None;
            }
            Heard::Drawn(Drawn::Painted(Part::Start, frame)) => {
                self.start = Some(frame);
                self.start_said = None;
            }
            Heard::Drawn(Drawn::Said(Part::Panel, said)) => self.panel_said = Some(said),
            Heard::Drawn(Drawn::Said(Part::Start, said)) => self.start_said = Some(said),
            Heard::Drawn(Drawn::Pressed(part, pressed)) => self.pressed(part, pressed),
        }
    }

    /// A key went down, read as `press`, writing `text` if it writes any:
    /// the card's page's while it has the keyboard; otherwise an edit of the
    /// one box, if it makes one, drawn again when it changed what the box
    /// holds, and what it chose said.
    pub fn key(&mut self, press: &Press, text: Option<&str>) {
        if self.conversation.keys_to_the_card() {
            // The card's page has the keyboard (queue item 718).
            if let Some(key) = card_keys::key(press, text) {
                self.to_the_page(CardInput::Key(key));
            }
            return;
        }
        let Some(edit) = typing::edit(press, text) else {
            return;
        };
        match self.conversation.edit(edit) {
            Typed::Unchanged => {}
            Typed::Changed => self.ask_drawn(),
            Typed::Chose(chosen) => {
                let had_a_card = self.conversation.card().is_some();
                // A choice is a decision as a press is: what the last one
                // came to is no longer said.
                self.told = None;
                self.start_said = Some(conversation_box::said(&chosen, self.searching()));
                // The box, emptied when a site or a search was chosen,
                // drawn so.
                self.ask_drawn();
                self.go();
                self.search(had_a_card);
                self.redraw = true;
            }
        }
    }

    /// Stop both threads and wait for them: the renderers stopped, and the
    /// Chrome showing a page closed and its profile removed.
    pub fn finish(&mut self) {
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
    use crate::conversation_chrome::Shown;
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
        window.heard(Heard::Searched(Searched {
            number: 1,
            words: "what is x".to_owned(),
            showing: Showing::Unanswered(crate::brave_search::unanswered(429)),
        }));
        assert_eq!(
            window.scene().and_then(|scene| scene.start_sentence),
            Some(crate::brave_search::unanswered(429).as_str())
        );
        assert_eq!(
            window
                .conversation()
                .results()
                .map(|results| results.showing.clone()),
            Some(Showing::Unanswered(crate::brave_search::unanswered(429)))
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
}
