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
//! # The card's page beneath the first screen
//!
//! Queue item 869, ADR 0100: the live card's page is drawn by a renderer of
//! its own on a thread of its own ([`crate::conversation_picture`]), and
//! the first screen leaves its place clear and says where
//! ([`Drawn::Beneath`]). A frame of the card asks for that picture alone,
//! never for the first screen, so a key's edit in the box is never behind
//! one.
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

use crate::conversation::{ChatId, Pressed, Showing};
use crate::conversation_picture::CardPictures;
use alo_paint::Point;
use alo_renderer::conversation_claim::Claim;
use alo_renderer::conversation_decide::{self, decide};
use alo_renderer::conversation_scroll::{Reach, Span};
use alo_renderer::host::{Gone, Renderers};
use alo_renderer::panel::{self, Panel};
use alo_renderer::site::Site;
use alo_renderer::split_place::Stands;
use alo_renderer::start::{self, SettingsKeyed, Start};
use alo_renderer::strip_decide::unasked;
use alo_renderer::{Frame, FromRenderer, ToRenderer};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

/// Which part of the conversation's window: the chats panel or the first
/// screen, as `alo-renderer` names them.
pub use alo_renderer::conversation_claim::Part;

/// What the window is told of a part.
#[derive(Debug, Clone, PartialEq)]
pub enum Drawn {
    /// Its renderer painted it: in device pixels at the window's scale.
    Painted(Part, Frame),
    /// Why its renderer painted nothing, in this browser's words.
    Said(Part, String),
    /// What a person's press on it was decided to be.
    Pressed(Part, Pressed),
    /// Which chat's row the pointer is over in the panel, as its renderer
    /// found it among the chats it painted, or none (queue item 789).
    Hovered(Option<ChatId>),
    /// How far the first screen's conversation reaches, as its renderer
    /// measured it after painting this many turns (queue item 703): what
    /// the window keeps a person's wheel inside. Told just before the
    /// [`Drawn::Painted`] of the frame it was measured in.
    Reached {
        /// How many turns were painted.
        turns: usize,
        /// How far they were painted scrolled: where the live card's page
        /// box was drawn ([`alo_renderer::card_place::page_box`]).
        down: f32,
        /// How many offers for what was typed were painted over the box,
        /// which the room is shorter by (queue item 759).
        offered: usize,
        /// Where the live card was painted: in the conversation, or in the
        /// split view while alo's agent works on it (queue item 819).
        stands: Stands,
        /// How far they reach.
        reach: Reach,
        /// What was drawn was *Settings*, not the conversation (queue item
        /// 880): what the reach is of, and where the keyboard was.
        settings: Option<SettingsDrawn>,
    },
    /// Where the part's field — the one box, or *Search chats* — was laid
    /// out as it was just drawn, or nothing when it drew none (queue item
    /// 861). Told just before the [`Drawn::Painted`] of the frame it was
    /// laid out in.
    Field(Part, Option<alo_renderer::field::FieldLaidOut>),
    /// Every drawing asked of the thread up to this number
    /// ([`ConversationDrawing::asked_through`]) has been drawn, or replaced
    /// by a later one before it was: told after what came of the newest
    /// of them, so a window that has heard it holds the frames of what it
    /// last asked (queue item 862).
    Through(u64),
    /// Where the first screen left its place clear for the card's page, in
    /// device pixels of the frame it was just drawn in, or nothing when it
    /// left none (queue item 869, ADR 0100). Told just before the
    /// [`Drawn::Painted`] of that frame.
    Beneath(Option<alo_renderer::beneath::Beneath>),
    /// The card's page drawn for that place, or nothing when it could not
    /// be (and was said); `number` says which picture asked
    /// ([`ConversationDrawing::pictured_through`]) it answers, every one
    /// before it replaced (queue item 869).
    Pictured {
        /// The picture, at the place's size.
        frame: Option<Frame>,
        /// The number of the newest picture asked that it answers.
        number: u64,
    },
}

/// A drawing of *Settings*, as its renderer measured it (queue item 880):
/// what the keyboard was on as it was drawn, and where that stood down the
/// page, if anything was.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SettingsDrawn {
    /// What the keyboard was on, and where it stood.
    pub keyed: Option<(SettingsKeyed, Span)>,
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
    /// offered made it the one large kind of work — numbered in the order
    /// asked.
    Draw(Box<Asked>, u64),
    /// A person pressed this part here, in its CSS pixels.
    Press(Part, Point),
    /// The pointer is over the panel here, in its CSS pixels, no button
    /// held (queue item 789).
    Hover(Point),
}

/// A thread holding the panel's renderer and the first screen's.
#[derive(Debug)]
pub struct ConversationDrawing {
    /// Where work is sent; [`None`] once finished.
    asked: Option<Sender<Work>>,
    /// What was last sent, so the same is not sent twice.
    sent: Option<Asked>,
    /// How many drawings have been sent: the number of the newest.
    numbered: u64,
    thread: Option<JoinHandle<()>>,
    /// The thread drawing the card's page beneath the first screen (queue
    /// item 869); [`None`] once finished.
    pictures: Option<CardPictures>,
}

impl ConversationDrawing {
    /// Start a thread that draws what it is asked in renderers of
    /// `renderers`', one for each part at a site of its own, and tells the
    /// window what came of it with `tell`. The renderers are handed the
    /// interface's typefaces as well as `renderers`' fonts
    /// ([`crate::fonts::for_the_interface`]).
    ///
    /// # Errors
    ///
    /// What the system said when no thread could be started.
    pub fn start(renderers: Renderers, tell: TellConversation) -> std::io::Result<Self> {
        // Both parts are the interface, so their renderers are handed the
        // design's typefaces beside whatever fonts they were given (ADR 0084).
        let faces = crate::fonts::for_the_interface(renderers.fonts());
        // The card's page has a renderer of its own, which draws no text.
        let twin = renderers.twin();
        let renderers = renderers.with_fonts(faces);
        // Both threads tell the window, one at a time.
        let tell = std::sync::Arc::new(std::sync::Mutex::new(tell));
        let telling = std::sync::Arc::clone(&tell);
        let pictures = CardPictures::start(
            twin,
            Box::new(move |drawn| telling.lock().is_ok_and(|tell| tell(drawn))),
        )?;
        let tell: TellConversation =
            Box::new(move |drawn| tell.lock().is_ok_and(|tell| tell(drawn)));
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
            numbered: 0,
            thread: Some(thread),
            pictures: Some(pictures),
        })
    }

    /// Have the card's page drawn from `png` for the first screen's clear
    /// `place`, unless that is what was last asked (queue item 869).
    /// Returns at once: the picture is told to the window.
    pub fn picture(&mut self, png: &[u8], place: alo_renderer::beneath::Beneath) {
        if let Some(pictures) = self.pictures.as_mut() {
            pictures.draw(png, place);
        }
    }

    /// Forget the picture last asked, so the next is asked whatever it is:
    /// the card it was of has gone (queue item 869).
    pub fn forget_picture(&mut self) {
        if let Some(pictures) = self.pictures.as_mut() {
            pictures.forget();
        }
    }

    /// The number of the newest picture of the card's page asked, which
    /// [`Drawn::Pictured`] says once it is drawn; 0 before any is.
    pub fn pictured_through(&self) -> u64 {
        self.pictures
            .as_ref()
            .map_or(0, CardPictures::asked_through)
    }

    /// Have `asked` drawn, unless it is what was last sent. Returns at once:
    /// what was painted is told to the window.
    pub fn draw(&mut self, asked: Asked) {
        if self.sent.as_ref() == Some(&asked) {
            return;
        }
        if let Some(sender) = &self.asked {
            self.numbered += 1;
            // A thread that has gone is a window that is closing, which
            // nothing here waits for.
            let _ = sender.send(Work::Draw(Box::new(asked.clone()), self.numbered));
        }
        self.sent = Some(asked);
    }

    /// The number of the newest drawing sent, which [`Drawn::Through`]
    /// says once it is drawn; 0 before any is.
    pub fn asked_through(&self) -> u64 {
        self.numbered
    }

    /// How much of the conversation's bar the first screen was last asked
    /// to show (queue item 793), while it was asked to show one.
    pub fn sent_bar_seen(&self) -> Option<f32> {
        self.sent
            .as_ref()
            .and_then(|asked| asked.start.bar)
            .map(|bar| bar.seen)
    }

    /// A person pressed `part` at `at`, in its CSS pixels: have it claimed
    /// and decided. Returns at once: what was decided is told to the window.
    pub fn press(&mut self, part: Part, at: Point) {
        if let Some(sender) = &self.asked {
            // As for a drawing: a thread that has gone is a window closing.
            let _ = sender.send(Work::Press(part, at));
        }
    }

    /// The pointer moved over the panel to `at`, in its CSS pixels: have
    /// the row under it found. Returns at once: which chat it is over is
    /// told to the window. Of hovers waiting, only the newest is asked.
    pub fn hover(&mut self, at: Point) {
        if let Some(sender) = &self.asked {
            let _ = sender.send(Work::Hover(at));
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
        let pictured = self.pictures.take().is_none_or(CardPictures::finish);
        self.thread
            .take()
            .is_none_or(|thread| thread.join().is_ok())
            && pictured
    }
}

/// One part's renderer, as the thread keeps it.
struct Held {
    /// Its site, which no page can name.
    site: Site,
    /// What happened to it, once it has gone.
    gone: Option<Gone>,
    /// The last frame it sent, as the window was told it: what it is asked
    /// to paint only the changes to (queue item 872, ADR 0102).
    frame: Option<Frame>,
}

impl Held {
    fn at(site: Site) -> Self {
        Self {
            site,
            gone: None,
            frame: None,
        }
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
            .rposition(|work| matches!(work, Work::Draw(..)));
        let last_hover = waiting
            .iter()
            .rposition(|work| matches!(work, Work::Hover(_)));
        for (place, work) in waiting.into_iter().enumerate() {
            let said = match work {
                Work::Draw(asked, number) if Some(place) == last_drawing => {
                    let mut said = holding.draw(*asked);
                    said.push(Drawn::Through(number));
                    said
                }
                Work::Press(part, at) => holding.press(part, at).into_iter().collect(),
                Work::Hover(at) if Some(place) == last_hover => {
                    holding.hover(at).into_iter().collect()
                }
                // A drawing or a hover a later one replaces.
                Work::Draw(..) | Work::Hover(_) => Vec::new(),
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
        let panel_of = |asked: &Asked| {
            (
                asked.panel.clone(),
                asked.showing.chats.clone(),
                asked.showing.hidden,
                asked.showing.hovered,
                asked.showing.closing,
                asked.showing.undo,
            )
        };
        if last.as_ref().map(panel_of) != Some(panel_of(&asked)) {
            let work = ToRenderer::Panel(Box::new(asked.panel.clone()));
            let came = drawn(&mut self.renderers, &mut self.panel, Part::Panel, &work);
            if matches!(came, Some(Drawn::Painted(..))) {
                self.painted.chats.clone_from(&asked.showing.chats);
                // Whether it was the rail, which draws its own two buttons
                // and none of the panel's (queue item 711).
                self.painted.hidden = asked.showing.hidden;
                // Which row offered Delete, which its × (789, 791), and
                // which chat Undo.
                self.painted.hovered = asked.showing.hovered;
                self.painted.closing = asked.showing.closing;
                self.painted.undo = asked.showing.undo;
                // Where *Search chats*' words were laid out, told before
                // the frame (queue item 861).
                said.extend(self.field(Part::Panel));
            }
            said.extend(came);
        }
        let start_of = |asked: &Asked| {
            (
                asked.start.clone(),
                asked.showing.sites.clone(),
                asked.showing.waiting.clone(),
                asked.showing.card,
                asked.showing.found.clone(),
                asked.showing.offers.clone(),
                (
                    asked.showing.tabs.clone(),
                    asked.showing.more.clone(),
                    asked.showing.see_all.clone(),
                    asked.showing.pictured.clone(),
                    asked.showing.show_more.clone(),
                    asked.showing.full_answer.clone(),
                ),
                (
                    asked.showing.working,
                    asked.showing.asking,
                    asked.showing.choices.clone(),
                    asked.showing.shield.clone(),
                    asked.showing.menu.clone(),
                ),
                asked.showing.faces.clone(),
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
                // And the sites a search's results showed, whose titles a
                // press claims by place (queue item 708).
                self.painted.found.clone_from(&asked.showing.found);
                // And what it offered for what was typed, which a press
                // claims by place (queue item 714).
                self.painted.offers.clone_from(&asked.showing.offers);
                // And the search results' open tabs, *More sites ›*, *See
                // all images* and images, each claimed by place (queue
                // items 743, 744).
                self.painted.tabs.clone_from(&asked.showing.tabs);
                self.painted.more.clone_from(&asked.showing.more);
                self.painted.see_all.clone_from(&asked.showing.see_all);
                self.painted.pictured.clone_from(&asked.showing.pictured);
                // And each *Show more sites* that can be pressed (761).
                self.painted.show_more.clone_from(&asked.showing.show_more);
                // And each *Full answer ›* (queue item 707).
                self.painted
                    .full_answer
                    .clone_from(&asked.showing.full_answer);
                // And the agent at work, the question it asks and a stuck
                // agent's choices (queue item 704): *Allow* allows only the
                // question the picture pressed showed.
                self.painted.working = asked.showing.working;
                // And the run held with the page the person's (queue item
                // 825): *Let alo continue* carries on only the run the
                // picture pressed showed held. Left out at first, it was
                // refused through both Chromiums.
                self.painted.paused = asked.showing.paused;
                // And the site the card's shield was drawn for (queue item
                // 807): a press on it switches only that site. Left out at
                // first, every press on the shield was refused by hand.
                self.painted.shield.clone_from(&asked.showing.shield);
                // And the lines of alo's menu over the card's page (queue
                // item 798): a press claims one by its place, decided only
                // against the menu the picture pressed showed.
                self.painted.menu.clone_from(&asked.showing.menu);
                self.painted.asking = asked.showing.asking;
                self.painted.choices.clone_from(&asked.showing.choices);
                // And the faces *Settings* offered, which a press claims by
                // place, decided only while the picture pressed showed them
                // (queue item 854). Left out at first, every face pressed
                // was refused through the window's own road.
                self.painted.faces.clone_from(&asked.showing.faces);
                // How far the turns painted reach, for a person's wheel, and
                // how far they were scrolled (queue item 703) — told before
                // the frame, so the window aims a press on it by the offset
                // it was drawn at from the moment it has it.
                // Where the box's words were laid out, told before the
                // frame (queue item 861).
                said.extend(self.field(Part::Start));
                // Where the card's page goes beneath it (queue item 869).
                said.extend(self.beneath(&asked.start));
                // *Settings* scrolls as the conversation does (queue item
                // 880), so its reach is measured too.
                if asked.start.conversing() || asked.start.settings {
                    said.extend(self.reach(
                        asked.start.turns.len(),
                        (asked.start.down, asked.start.offers.len()),
                        asked.start.stands(),
                        (asked.start.settings, asked.start.settings_keyed),
                    ));
                }
                said.extend(came);
            } else {
                said.extend(came);
            }
        }
        self.drawn = Some(asked);
        said
    }

    /// Ask the first screen's renderer how far the `turns` it just painted,
    /// scrolled `down` with `offered` offers over the box, the live card
    /// standing as `stands` — or *Settings*, the keyboard on what the last
    /// says, when the first is so (queue item 880) — reach: what the
    /// window is told — nothing when it answered anything but a reach,
    /// which keeps the wheel where it was, or said, when it went.
    fn reach(
        &mut self,
        turns: usize,
        (down, offered): (f32, usize),
        stands: Stands,
        (settings, keyed_as): (bool, Option<SettingsKeyed>),
    ) -> Option<Drawn> {
        if self.start.gone.is_some() {
            return None;
        }
        match self
            .renderers
            .ask(&self.start.site, &ToRenderer::ConversationReach)
        {
            Ok(FromRenderer::ConversationReached(reach, keyed)) => Some(Drawn::Reached {
                turns,
                down,
                offered,
                stands,
                reach: reach.bounded(),
                settings: settings.then(|| SettingsDrawn {
                    keyed: keyed_as.zip(keyed.map(Span::bounded)),
                }),
            }),
            Ok(_) => None,
            Err(gone) => {
                let said = gone.to_string();
                self.start.gone = Some(gone);
                Some(Drawn::Said(Part::Start, said))
            }
        }
    }

    /// Ask `part`'s renderer where its field's words were laid out as it
    /// just drew them (queue item 861): what the window places a press's
    /// caret by and keeps the words' scroll in. Nothing when it answered
    /// anything else; said, when it went.
    fn field(&mut self, part: Part) -> Option<Drawn> {
        let held = match part {
            Part::Panel => &mut self.panel,
            Part::Start => &mut self.start,
        };
        if held.gone.is_some() {
            return None;
        }
        match self.renderers.ask(&held.site, &ToRenderer::FieldLaidOut) {
            Ok(FromRenderer::FieldLaidOut(laid)) => Some(Drawn::Field(part, laid)),
            Ok(_) => None,
            Err(gone) => {
                let said = gone.to_string();
                held.gone = Some(gone);
                Some(Drawn::Said(part, said))
            }
        }
    }

    /// Ask the first screen's renderer where it left its place clear for
    /// the card's page, when `start` has a card drawn beneath it (queue
    /// item 869): what the window is told — nothing when it has no such
    /// card, which the window takes as no place, when its renderer has gone
    /// or answered anything else — or said, when it went.
    fn beneath(&mut self, start: &Start) -> Option<Drawn> {
        let beneath = start
            .card()
            .is_some_and(|card| card.picture_at == alo_renderer::card::PictureAt::Beneath);
        if !beneath || self.start.gone.is_some() {
            return None;
        }
        match self
            .renderers
            .ask(&self.start.site, &ToRenderer::PictureBeneath)
        {
            Ok(FromRenderer::PictureBeneath(place)) => Some(Drawn::Beneath(place)),
            Ok(_) => None,
            Err(gone) => {
                let said = gone.to_string();
                self.start.gone = Some(gone);
                Some(Drawn::Said(Part::Start, said))
            }
        }
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

impl Holding {
    /// Ask the panel's renderer what is under the pointer at `at`: which
    /// chat's row, among the chats it painted — its *Delete* or × is its
    /// row's too, and on the rail a chat's button (queue item 760) — or
    /// none. Nothing when the panel's renderer has gone or nothing
    /// was painted; anything else it answers is no row.
    fn hover(&mut self, at: Point) -> Option<Drawn> {
        self.drawn.as_ref()?;
        if self.panel.gone.is_some() {
            return None;
        }
        let answer = self
            .renderers
            .ask(&self.panel.site, &ToRenderer::ConversationHover(at))
            .ok()?;
        let over = match answer {
            FromRenderer::ConversationClaimed(Some(Claim::Chat { at })) => {
                self.painted.chats.get(at).copied()
            }
            FromRenderer::ConversationClaimed(Some(Claim::DeleteChat)) => self.painted.hovered,
            // An open row's × is its row's too (queue item 791).
            FromRenderer::ConversationClaimed(Some(Claim::CloseChat)) => self.painted.closing,
            _ => None,
        };
        Some(Drawn::Hovered(over))
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
    // Only what changed since the frame it holds, once it holds one: the
    // renderer says the rest is as it was (queue item 872).
    let paint = if held.frame.is_some() {
        ToRenderer::PaintOver
    } else {
        ToRenderer::Paint
    };
    let answer = renderers
        .ask(&held.site, work)
        .and_then(|built| match built {
            FromRenderer::Loaded { .. } => renderers.ask(&held.site, &paint),
            other => Ok(other),
        });
    let mut patched = false;
    let answer = match answer {
        Ok(FromRenderer::PaintedOver(patch)) => {
            patched = held.frame.as_mut().is_some_and(|frame| patch.onto(frame));
            match held.frame.as_ref().filter(|_| patched) {
                Some(frame) => Ok(FromRenderer::Painted(frame.clone())),
                // Not of the frame it holds: the whole frame asked for, and
                // kept as it comes.
                None => renderers.ask(&held.site, &ToRenderer::Paint),
            }
        }
        other => other,
    };
    if !patched {
        held.frame = match &answer {
            Ok(FromRenderer::Painted(frame)) => Some(frame.clone()),
            _ => None,
        };
    }
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
                ..Panel::default()
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
