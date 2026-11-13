/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The conversation's window and assistive technology (ADR 0115, queue
//! item 987): the parts' trees kept as they were last read, published while
//! the bus listens, and what the bus asks checked against them, sent to be
//! decided as a press, acted on, and written in the record.
//!
//! # What is asked is checked against what was published
//!
//! A node is acted on only if the tree its part last answered holds it and
//! offers what was asked ([`crate::assisted::offers`], the same rule the
//! tree was published by): an action the bus forwards without checking what
//! a node offers — AT-SPI's adapter does, ADR 0115 § 2 — is refused here,
//! and so is `SetValue` on a password or a card's details (§ 4).
//!
//! # As a person's press is
//!
//! What it is decided to be is acted on as a person's press decided the
//! same is, by the same function: a press on *New chat* is a new chat. A
//! press of the person's own, made and not yet decided, keeps its place:
//! assistive technology's is never taken for it. The keyboard and text put
//! in are taken only where the press was decided as a field — the one box
//! or *Search chats* — and text replaces everything the field held.
//!
//! # A card's page (ADR 0115 § 3, queue item 988)
//!
//! While the bus listens, the live card's page is read through the agent
//! socket's own orders ([`crate::assisted_page`]) as the bus asks and again
//! after each frame its Chromium streams — one read at a time, the next
//! asked only once it is answered — and published under the card's
//! picture of it, at the page box a person's press on it is aimed by. An
//! action on one of its nodes is checked against the tree it was last read
//! as, by the same offers, and carried out as the socket's `act`; a press
//! the person's gate says pays is held, asked on the card as alo's agent's
//! question is, and done only after *Pay* — *Not now* or *Stop* leaves it
//! undone. A press taken gives the page the keyboard, as a person's press
//! on it does. A card closed or replaced forgets its page and any press
//! held on it.
//!
//! # As *Settings* says (ADR 0115, *Phase (c)'s guard*; queue item 989)
//!
//! *Screen readers*: under *Read only* every action the bus asks is
//! refused and written; under *Off* the window is published alone — no
//! part, no page — and the card's page is not read. Whichever it is, a
//! press from the bus that comes to the switch itself is refused: only a
//! person's own press in *Settings* changes it. A card on a site kept from
//! agents has its page published as one node, [`assisted::KEPT`], is never
//! read, refuses every action on its nodes, and its address is said as
//! the same words wherever alo's own tree would say it
//! ([`assisted::hide_kept`]). When the bus begins and stops listening is a
//! line each in the record, with the time.

use super::ConversationWindow;
use crate::agent_gate::Question;
use crate::agent_sites::AgentSites;
use crate::assisted::{
    self, Asked, Assist, FromTheBus, KeptAddresses, PageRead, Published, PublishedPage,
    PublishedPart,
};
use crate::assisted_page::{PageHeard, PageReader};
use crate::assisted_record::AssistedRecord;
use crate::conversation::{Field, Pressed as Decided};
use crate::conversation_chrome::Went;
use crate::conversation_drawing::Part;
use crate::conversation_place::ConversationPlacement;
use crate::typing::Edit;
use alo_box::BoxId;
use alo_renderer::Snapshot;
use alo_renderer::settings_bus::Bus;
use alo_renderer::turn::Turn;
use alo_url::Url;

/// What the card says a press from assistive technology that pays is: the
/// line above its question, where alo's agent's says what was asked.
pub const PRESSED_BY_THE_BUS: &str = "Assistive technology pressed this on the page";

/// The parts' trees as the bus is to be told them, and what it did.
#[derive(Debug, Default)]
pub(super) struct Assisting {
    /// Whether something on the bus asked for the tree.
    active: bool,
    /// Whether nothing has been published since it asked.
    fresh: bool,
    /// Whether a tree was read since the window last published.
    changed: bool,
    /// The panel's tree, as its renderer last read it.
    panel: Option<Snapshot>,
    /// The first screen's.
    start: Option<Snapshot>,
    /// What assistive technology asked and what came of it.
    record: AssistedRecord,
    /// The card's page, as it was last read for the bus.
    page: Option<PageRead>,
    /// What reads it and acts on it; [`None`] when the window was given no
    /// way to reach a card.
    reader: Option<PageReader>,
    /// Whether a read is asked and not yet answered, and whether another
    /// was wanted meanwhile.
    reading: (bool, bool),
    /// A press that pays, held for the person's yes.
    held: Option<HeldPress>,
    /// The sites agents may reach, as the person named them: a card on
    /// any other is kept from the bus too (queue item 989).
    sites: AgentSites,
}

/// A press from assistive technology on the card's page, held for the
/// person's yes (ADR 0115, *Answered* 1).
#[derive(Debug)]
struct HeldPress {
    /// The question the card asks, by its number.
    number: u64,
    /// The node.
    node: BoxId,
    /// What was asked.
    assist: Assist,
    /// What the person is asked, as the page was judged.
    question: Question,
}

impl ConversationWindow {
    /// What the bus said.
    pub(super) fn bus_said(&mut self, from: FromTheBus) {
        match from {
            FromTheBus::Active => {
                if !self.assisting.active {
                    self.listening_said(true);
                }
                self.assisting.active = true;
                self.assisting.fresh = true;
                self.assisting.changed = true;
                if let Some(drawing) = self.drawing.as_mut() {
                    drawing.read_trees();
                }
                self.read_page();
            }
            FromTheBus::Inactive => {
                if self.assisting.active {
                    self.listening_said(false);
                }
                self.assisting.active = false;
            }
            FromTheBus::Asked(asked) => match self.untaken() {
                Some(why) => self.assisted_did(&format!(
                    "{} on node {} of {} was refused: {why}",
                    asked.assist,
                    asked.node.as_usize(),
                    asked.part
                )),
                None => self.assist_asked(asked),
            },
            FromTheBus::Page { node, assist } => match self.untaken() {
                Some(why) => self.assisted_did(&format!(
                    "{assist} on node {} of the card's page was refused: {why}",
                    node.as_usize()
                )),
                None => self.assist_the_page(node, assist),
            },
            FromTheBus::Refused(refused) => {
                self.assisted_did(&format!("{} was refused: {}", refused.action, refused.why));
            }
        }
    }

    /// Why no action from the bus is taken now, if none is: *Settings ›
    /// Screen readers* is not *Read and act* (queue item 989).
    fn untaken(&self) -> Option<&'static str> {
        match self.conversation.settings().bus() {
            Bus::ReadAndAct => None,
            Bus::ReadOnly => Some("Settings › Screen readers is set to Read only"),
            Bus::Off => Some("Settings › Screen readers is set to Off"),
        }
    }

    /// Write that the bus began listening, when `began`, or stopped — with
    /// the time, never which program — and say it.
    fn listening_said(&mut self, began: bool) {
        let line = self
            .assisting
            .record
            .listening(began, std::time::SystemTime::now());
        self.said.push(line);
    }

    /// *Settings › Screen readers* changed by a person's press: used at
    /// once — published again as it now says, and the card's page read
    /// again if it is published once more.
    pub(super) fn bus_chosen(&mut self) {
        self.assisting.changed = true;
        self.read_page();
    }

    /// Whether the card shown is on a site kept from agents.
    fn card_kept(&self) -> bool {
        self.conversation
            .card()
            .is_some_and(|card| !self.assisting.sites.allows(&card.url))
    }

    /// The address of every page the chat on screen shows or showed: the
    /// card's, and each earlier page's.
    fn addresses_on_screen(&self) -> Vec<&Url> {
        self.conversation
            .thread()
            .map(|thread| {
                thread
                    .turns()
                    .iter()
                    .filter_map(|turn| match turn {
                        Turn::Page(card) => Some(&card.url),
                        Turn::Visited { url, .. } => Some(url),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `asked`, checked against the tree its part last answered, and sent to
    /// be decided — or refused, and written.
    fn assist_asked(&mut self, asked: Asked) {
        let Asked { part, node, assist } = asked;
        let tree = match part {
            Part::Panel => self.assisting.panel.as_ref(),
            Part::Start => self.assisting.start.as_ref(),
        };
        let found = tree.and_then(|tree| assisted::find(tree, node));
        let refused = match found {
            None => Some("no node of that number is published"),
            Some(found) if !assisted::offers(found).holds(&assist) => {
                Some("that node is not offered it")
            }
            Some(_) => None,
        };
        if let Some(why) = refused {
            self.assisted_did(&format!(
                "{assist} on node {} of {part} was refused: {why}",
                node.as_usize()
            ));
            return;
        }
        if let Some(drawing) = self.drawing.as_mut() {
            drawing.assist(part, node, assist);
        }
    }

    /// `part`'s tree, read from the drawing it just painted.
    pub(super) fn tree_read(&mut self, part: Part, tree: Snapshot) {
        match part {
            Part::Panel => self.assisting.panel = Some(tree),
            Part::Start => self.assisting.start = Some(tree),
        }
        self.assisting.changed = true;
    }

    /// What `assist` on a node of `part` was decided to be: acted on as a
    /// person's press decided the same is, and written.
    pub(super) fn assisted(&mut self, part: Part, assist: Assist, decided: Decided) {
        // *Settings* may have changed while it was decided.
        if let Some(why) = self.untaken() {
            self.assisted_did(&format!("{assist} on {part} was refused: {why}"));
            return;
        }
        // The switch itself is a person's alone to press (ADR 0115, *Phase
        // (c)'s guard*, (a); ADR 0026's rule for grants).
        if let Decided::Bus(bus) = decided {
            self.assisted_did(&format!(
                "{assist} on {part} was refused: it came to {}'s choice of Settings › \
                 Screen readers, which only a person's own press there changes",
                bus.said()
            ));
            return;
        }
        let on_a_field = matches!(decided, Decided::TheBox | Decided::SearchChats);
        if !matches!(assist, Assist::Click) && !on_a_field {
            self.assisted_did(&format!(
                "{assist} on {part} was refused: it was decided as {decided:?}, not a field"
            ));
            return;
        }
        self.assisted_did(&format!("{assist} on {part} came to {decided:?}"));
        let persons = self.field_press_aside();
        self.act_on(part, decided);
        self.field_press_put_back(persons);
        if let Assist::SetValue(text) = assist {
            let _ = self.conversation.edit(Edit::SelectAll);
            if matches!(
                self.conversation.edit(Edit::Put(text)),
                crate::conversation_box::Typed::Changed
            ) {
                self.ask_drawn();
                self.redraw = true;
            }
        }
    }

    /// Write `what` in the record, and say it on the terminal.
    fn assisted_did(&mut self, what: &str) {
        let line = self.assisting.record.write(what);
        self.said.push(line);
    }

    /// What assistive technology asked and what came of it.
    pub fn assisted_record(&self) -> &AssistedRecord {
        &self.assisting.record
    }

    /// The same window, a card on any site but `sites` kept from the bus as
    /// it is from agents (queue item 989): those the person named with
    /// `--agent-site`.
    #[must_use]
    pub fn with_agent_sites(mut self, sites: AgentSites) -> Self {
        self.assisting.sites = sites;
        self
    }

    /// The same window, its card's page read and acted on for the bus by
    /// `reader` (ADR 0115 § 3, queue item 988).
    #[must_use]
    pub fn with_assisted_page(mut self, reader: PageReader) -> Self {
        self.assisting.reader = Some(reader);
        self
    }

    /// Read the card's page for the bus, if it listens and a card shows a
    /// page: now, or once the read under way is answered.
    pub(super) fn read_page(&mut self) {
        // Nothing read that is not published (queue item 989).
        if !self.assisting.active
            || self.conversation.card().is_none()
            || !self.conversation.settings().bus().publishes()
            || self.card_kept()
        {
            return;
        }
        let Some(reader) = &self.assisting.reader else {
            return;
        };
        if self.assisting.reading.0 {
            self.assisting.reading.1 = true;
        } else {
            reader.read();
            self.assisting.reading.0 = true;
        }
    }

    /// What became of the card, as the card's page for the bus follows it:
    /// read again after each frame its Chromium streams, at most one read
    /// at a time (ADR 0090's pace) — and forgotten as another card opens,
    /// or none is left.
    pub(super) fn page_follows(&mut self, went: &Went) {
        match went {
            Went::Framed(_) => self.read_page(),
            Went::Opening(_) | Went::Unopened { .. } | Went::Closed(_) | Went::Shut(_) => {
                self.forget_page();
            }
            _ => {}
        }
    }

    /// The card shown changed — another opened, or none: its page and any
    /// press held on it are forgotten.
    fn forget_page(&mut self) {
        self.assisting.page = None;
        self.assisting.changed = true;
        if let Some(held) = self.assisting.held.take() {
            let _ = self.conversation.answer_held(held.number);
            self.assisted_did(&format!(
                "{} on node {} of the card's page was not done: its card went",
                held.assist,
                held.node.as_usize()
            ));
        }
    }

    /// What the thread reading the card's page told.
    pub(super) fn page_heard(&mut self, heard: PageHeard) {
        match heard {
            PageHeard::Read(read) => {
                self.assisting.page = self.conversation.card().map(|_| read);
                self.page_read();
            }
            PageHeard::Unread(_) => {
                self.assisting.page = None;
                self.page_read();
            }
            PageHeard::Held {
                node,
                assist,
                question,
            } => self.hold_for_the_person(node, assist, question),
            PageHeard::Acted {
                node,
                assist,
                done,
                came_to,
            } => {
                self.assisted_did(&format!(
                    "{assist} on node {} of the card's page came to {came_to}",
                    node.as_usize()
                ));
                // Taken, the page has the keyboard, as after a person's
                // press on it.
                if done
                    && matches!(assist, Assist::Click | Assist::Focus)
                    && self.conversation.press_the_page()
                {
                    self.ask_drawn();
                }
                self.read_page();
            }
        }
    }

    /// A read answered: published, and the next asked if one was wanted.
    fn page_read(&mut self) {
        self.assisting.changed = true;
        self.assisting.reading.0 = false;
        if std::mem::take(&mut self.assisting.reading.1) {
            self.read_page();
        }
    }

    /// An action on the card's page's node `node`, checked against the tree
    /// it was last read as, and handed to be carried out — or refused, and
    /// written.
    fn assist_the_page(&mut self, node: BoxId, assist: Assist) {
        // A press held that a run of alo's agent replaced on the card waits
        // no more.
        if self
            .assisting
            .held
            .as_ref()
            .is_some_and(|held| self.conversation.held_question() != Some(held.number))
        {
            self.assisting.held = None;
        }
        let found = match &self.assisting.page {
            Some(PageRead::Tree(tree)) => assisted::find(tree, node),
            Some(PageRead::TooLarge | PageRead::Kept) | None => None,
        };
        let refused = match found {
            _ if self.conversation.card().is_none() => Some("no card's page is shown"),
            // A node read before the card went to a kept site is no node of
            // its page now.
            _ if self.card_kept() => Some("the card's page is kept from agents"),
            None => Some("no node of that number is published"),
            Some(found) if !assisted::offers(found).holds(&assist) => {
                Some("that node is not offered it")
            }
            Some(_) if self.assisting.held.is_some() => {
                Some("a press on the page waits on the person's answer")
            }
            Some(_) => None,
        };
        if let Some(why) = refused {
            self.assisted_did(&format!(
                "{assist} on node {} of the card's page was refused: {why}",
                node.as_usize()
            ));
            return;
        }
        if let Some(reader) = &self.assisting.reader {
            reader.act(node, assist, None);
        }
    }

    /// A press that pays, held: asked on the card as alo's agent's question
    /// is, or refused while alo's agent is at work there.
    fn hold_for_the_person(&mut self, node: BoxId, assist: Assist, question: Question) {
        let held = self.conversation.hold_for_the_person(
            PRESSED_BY_THE_BUS,
            question.what.clone(),
            question.paying.clone(),
        );
        let Some(number) = held else {
            self.assisted_did(&format!(
                "{assist} on node {} of the card's page was refused: it pays, and the person \
                 cannot be asked while alo's agent works on the page or another press waits",
                node.as_usize()
            ));
            return;
        };
        self.assisted_did(&format!(
            "{assist} on node {} of the card's page pays, and waits on the person's answer: {}",
            node.as_usize(),
            question.what
        ));
        self.assisting.held = Some(HeldPress {
            number,
            node,
            assist,
            question,
        });
        self.following = true;
        self.ask_drawn();
        self.redraw = true;
    }

    /// A press on *Allow* (or *Pay*), *Not now* or *Stop*, decided — the
    /// person's own, or the bus's, which is taken as any press is (ADR 0115,
    /// *Answered* 4): the press held done, or left undone.
    pub(super) fn held_pressed(&mut self, pressed: &Decided) {
        let Some(held) = &self.assisting.held else {
            return;
        };
        let (number, allowed) = match pressed {
            Decided::Allow(number) => (*number, true),
            Decided::NotNow(number) => (*number, false),
            Decided::Stop(_) => (held.number, false),
            _ => return,
        };
        if number != held.number {
            return;
        }
        let _ = self.conversation.answer_held(number);
        let Some(held) = self.assisting.held.take() else {
            return;
        };
        self.ask_drawn();
        if !allowed {
            self.assisted_did(&format!(
                "{} on node {} of the card's page was not done: the person said not now",
                held.assist,
                held.node.as_usize()
            ));
            return;
        }
        self.assisted_did(&format!(
            "{} on node {} of the card's page was allowed by the person",
            held.assist,
            held.node.as_usize()
        ));
        if let Some(reader) = &self.assisting.reader {
            reader.act(held.node, held.assist, Some(held.question));
        }
    }

    /// The card's page box in the window, in device pixels, as it was last
    /// painted: where a person's press on its page is aimed.
    fn page_box_in_the_window(&self) -> Option<(f64, f64, f64, f64)> {
        let scene = self.scene()?;
        let placed = ConversationPlacement::of(scene.window, scene.scale, scene.parts());
        let painted = self.card_on_screen()?;
        let page = painted.stands.page_box(painted.viewport, painted.down);
        let device = |css: f32| f64::from(css) * f64::from(scene.scale.factor());
        Some((
            f64::from(placed.start.x) + device(page.left()),
            f64::from(placed.start.y) + device(page.top()),
            device(page.size.width),
            device(page.size.height),
        ))
    }

    /// The window's tree as it is to be published now, if the bus listens
    /// and a part's tree was read since it was last taken: each part where
    /// the window composes it, and which part's field has the keyboard.
    pub fn take_published(&mut self) -> Option<Published> {
        if !self.assisting.active || !self.assisting.changed {
            return None;
        }
        let (placed, scale) = {
            let scene = self.scene()?;
            (
                ConversationPlacement::of(scene.window, scene.scale, scene.parts()),
                scene.scale.factor(),
            )
        };
        // *Off* (queue item 989): the window alone.
        if !self.conversation.settings().bus().publishes() {
            self.assisting.changed = false;
            return Some(Published {
                scale,
                panel: None,
                start: None,
                page: None,
                keyboard: None,
                fresh: core::mem::take(&mut self.assisting.fresh),
            });
        }
        let kept = KeptAddresses::of(&self.assisting.sites, self.addresses_on_screen());
        let hidden = |mut tree: Snapshot| {
            assisted::hide_kept(&mut tree, &kept);
            tree
        };
        let keyboard = if self.conversation.settings().is_open() {
            None
        } else {
            self.conversation.field_focused().map(|field| match field {
                Field::TheBox => Part::Start,
                Field::SearchChats => Part::Panel,
            })
        };
        let read = if self.card_kept() {
            Some(PageRead::Kept)
        } else {
            self.assisting.page.clone()
        };
        let page = read.and_then(|read| {
            self.page_box_in_the_window()
                .map(|at| PublishedPage { at, read })
        });
        self.assisting.changed = false;
        let fresh = core::mem::take(&mut self.assisting.fresh);
        Some(Published {
            scale,
            panel: self.assisting.panel.clone().map(|tree| PublishedPart {
                at: placed.panel,
                tree: hidden(tree),
            }),
            start: self.assisting.start.clone().map(|tree| PublishedPart {
                at: placed.start,
                tree: hidden(tree),
            }),
            page,
            keyboard,
            fresh,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_sites::AgentSites;
    use crate::beside::Chromium;
    use crate::conversation_chrome::ChromeOpener;
    use crate::conversation_drawing::{ConversationDrawing, Drawn};
    use crate::conversation_window::{Heard, Measured};
    use crate::place::PixelRect;
    use crate::scale::Scale;
    use alo_box::{BoxId, KnownRole, Role, States};
    use alo_layout::Rect;
    use alo_renderer::SnapshotNode;
    use alo_renderer::host::Renderers;
    use std::sync::{Arc, Mutex};

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

    fn node(id: usize, role: KnownRole, name: &str, takes_text: bool) -> SnapshotNode {
        SnapshotNode {
            id: BoxId::from_wire(id),
            role: Role::Known(role),
            name: Some(name.to_owned()),
            states: States {
                takes_text,
                ..States::default()
            },
            placeholder: None,
            marker: None,
            rect: Rect::new(10.0, 56.0, 220.0, 40.0),
            rects: vec![Rect::new(10.0, 56.0, 220.0, 40.0)],
            offscreen: false,
            scrolls: false,
            children: Vec::new(),
        }
    }

    /// A frame of `width` by `height` device pixels, painted.
    fn sized(width: u32, height: u32) -> alo_renderer::Frame {
        alo_renderer::Frame {
            width,
            height,
            pixels: vec![0; 4 * width as usize * height as usize],
        }
    }

    /// The panel's *New chat*, node 12, and the first screen's box, 40,
    /// each read after its part painted the design's window at one.
    fn read(window: &mut ConversationWindow) {
        window.heard(Heard::Drawn(Drawn::Painted(Part::Panel, sized(240, 820))));
        window.heard(Heard::Drawn(Drawn::Painted(Part::Start, sized(1040, 820))));
        window.heard(Heard::Drawn(Drawn::Tree(
            Part::Panel,
            Box::new(Snapshot {
                root: Some(node(12, KnownRole::Link, "New chat", false)),
            }),
        )));
        window.heard(Heard::Drawn(Drawn::Tree(
            Part::Start,
            Box::new(Snapshot {
                root: Some(node(40, KnownRole::TextBox, "Ask anything", true)),
            }),
        )));
    }

    fn asked(part: Part, node: usize, assist: Assist) -> Heard {
        Heard::Bus(FromTheBus::Asked(Asked {
            part,
            node: BoxId::from_wire(node),
            assist,
        }))
    }

    #[test]
    fn nothing_is_published_until_the_bus_asks_and_then_each_part_where_it_stands() {
        let mut window = window();
        read(&mut window);
        assert_eq!(window.take_published(), None, "nothing listens yet");
        window.heard(Heard::Bus(FromTheBus::Active));
        read(&mut window);
        let published = window.take_published().expect("published");
        assert!(published.fresh);
        assert_eq!(published.keyboard, Some(Part::Start), "the box has it");
        assert_eq!(
            published.panel.map(|part| part.at),
            Some(PixelRect {
                x: 0,
                y: 0,
                width: 240,
                height: 820
            })
        );
        assert_eq!(
            published.start.map(|part| part.at),
            Some(PixelRect {
                x: 240,
                y: 0,
                width: 1040,
                height: 820
            })
        );
        // Taken, it is not published again until a part is read again.
        assert_eq!(window.take_published(), None);
        read(&mut window);
        assert!(window.take_published().is_some_and(|again| !again.fresh));
    }

    #[test]
    fn what_no_published_node_offers_is_refused_and_written() {
        let mut window = window();
        window.heard(Heard::Bus(FromTheBus::Active));
        read(&mut window);
        let _ = window.take_said();
        window.heard(asked(Part::Panel, 99, Assist::Click));
        window.heard(asked(Part::Panel, 12, Assist::SetValue("x".to_owned())));
        window.heard(asked(Part::Panel, 12, Assist::Focus));
        window.heard(Heard::Bus(FromTheBus::Refused(crate::assisted::Unasked {
            action: "ScrollToPoint".to_owned(),
            why: "it carries a point",
        })));
        // After the line saying the bus began listening (queue item 989).
        let written: Vec<&str> = window.assisted_record().entries().skip(1).collect();
        assert_eq!(
            written,
            [
                "assistive technology: a press on node 99 of the chats panel was refused: no \
                 node of that number is published",
                "assistive technology: text put in, 1 characters on node 12 of the chats panel \
                 was refused: that node is not offered it",
                "assistive technology: the keyboard moved on node 12 of the chats panel was \
                 refused: that node is not offered it",
                "assistive technology: ScrollToPoint was refused: it carries a point",
            ]
        );
        // And said on the terminal, every one.
        assert_eq!(window.take_said().len(), 4);
    }

    #[test]
    fn a_press_decided_is_acted_on_as_a_persons_is() {
        let mut window = window();
        let chats = window.conversation().showing().chats.len();
        window.heard(Heard::Drawn(Drawn::Assisted(
            Part::Panel,
            Assist::Click,
            Box::new(Decided::NewChat),
        )));
        assert_eq!(window.conversation().showing().chats.len(), chats + 1);
        assert_eq!(
            window.assisted_record().entries().last(),
            Some("assistive technology: a press on the chats panel came to NewChat")
        );
    }

    #[test]
    fn text_put_replaces_what_the_box_held_and_the_keyboard_goes_only_to_a_field() {
        let mut window = window();
        let put = |window: &mut ConversationWindow, text: &str| {
            window.heard(Heard::Drawn(Drawn::Assisted(
                Part::Start,
                Assist::SetValue(text.to_owned()),
                Box::new(Decided::TheBox),
            )));
        };
        put(&mut window, "lisbon hotels");
        put(&mut window, "porto");
        assert_eq!(
            window.conversation().field(Field::TheBox).text(),
            "porto",
            "replaced, not added to"
        );
        // The keyboard asked for on what was decided as *New chat*: refused,
        // and no chat made.
        let chats = window.conversation().showing().chats.len();
        window.heard(Heard::Drawn(Drawn::Assisted(
            Part::Panel,
            Assist::Focus,
            Box::new(Decided::NewChat),
        )));
        assert_eq!(window.conversation().showing().chats.len(), chats);
        assert!(
            window
                .assisted_record()
                .entries()
                .last()
                .is_some_and(|line| line.ends_with("not a field")),
            "{:?}",
            window.assisted_record()
        );
        // On *Search chats*, the keyboard goes there.
        window.heard(Heard::Drawn(Drawn::Assisted(
            Part::Panel,
            Assist::Focus,
            Box::new(Decided::SearchChats),
        )));
        assert_eq!(
            window.conversation().field_focused(),
            Some(Field::SearchChats)
        );
    }
    /// A card's thread answering with a checkout whose *Pay UGX 45,000* is
    /// node 7 and *Say hello* node 8, keeping each verb it carries out.
    fn checkout_card() -> (crate::inbox::Orders, Arc<Mutex<Vec<alo_agent::Verb>>>) {
        use crate::asking::{Answer, Ask};
        use crate::inbox::Arrival;
        use crate::message::Order;
        let (orders, _, inbox) = crate::inbox::inbox();
        let acted = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&acted);
        let mut page = node(1, KnownRole::Document, "Kampala Coffee", false);
        page.children = vec![
            node(7, KnownRole::Button, "Pay UGX 45,000", false),
            node(8, KnownRole::Button, "Say hello", false),
        ];
        let page = Snapshot { root: Some(page) };
        std::thread::spawn(move || {
            while let Ok(Arrival::Order(Order::Agent { ask, reply, .. })) = inbox.recv() {
                let answer = match ask {
                    Ok(Ask::Tabs) => Answer::Tabs(vec!["https://shop.example/".to_owned()]),
                    Ok(Ask::Read { tab }) => Answer::Tree {
                        tab,
                        snapshot: Box::new(page.clone()),
                    },
                    Ok(Ask::Act {
                        tab,
                        target: alo_agent::Target::Node(node),
                        verb,
                    }) => {
                        if let Ok(mut acted) = kept.lock() {
                            acted.push(verb);
                        }
                        Answer::Done {
                            tab,
                            outcome: alo_agent::Outcome::Activated { node, name: None },
                            issues: Vec::new(),
                        }
                    }
                    _ => return,
                };
                let _ = reply.send(answer);
            }
        });
        (orders, acted)
    }

    /// The window with a card open on the checkout, painted, the bus
    /// listening, and what its page's reader tells it.
    fn with_the_checkout() -> (
        ConversationWindow,
        std::sync::mpsc::Receiver<PageHeard>,
        Arc<Mutex<Vec<alo_agent::Verb>>>,
    ) {
        let (orders, acted) = checkout_card();
        let (tell, told) = std::sync::mpsc::channel();
        let reader =
            PageReader::start(orders, Box::new(move |heard| tell.send(heard).is_ok())).unwrap();
        let mut window = window().with_assisted_page(reader);
        window.heard(Heard::Chrome(crate::conversation_chrome::Went::Opening(
            alo_url::parse("https://shop.example/").unwrap(),
        )));
        window.heard(Heard::Chrome(crate::conversation_chrome::Went::Framed(
            vec![1],
        )));
        window.heard(Heard::Bus(FromTheBus::Active));
        read(&mut window);
        (window, told, acted)
    }

    /// What the reader tells, handed to the window, to the first that is
    /// not a read — or, before anything was acted on, the first read.
    fn next(window: &mut ConversationWindow, told: &std::sync::mpsc::Receiver<PageHeard>) {
        // The line saying the bus began listening is no action (queue item
        // 989).
        let first = window
            .assisted_record()
            .entries()
            .all(|line| line.contains("the accessibility bus began listening"))
            && window.assisting.page.is_none();
        loop {
            let heard = told
                .recv_timeout(std::time::Duration::from_secs(10))
                .expect("the reader tells");
            let read = matches!(heard, PageHeard::Read(_) | PageHeard::Unread(_));
            window.heard(Heard::Page(heard));
            if !read || first {
                return;
            }
        }
    }

    #[test]
    fn a_cards_page_read_for_the_bus_is_published_at_its_page_box() {
        let (mut window, told, _) = with_the_checkout();
        next(&mut window, &told);
        let published = window.take_published().expect("published");
        let page = published.page.expect("the page");
        assert!(matches!(&page.read, PageRead::Tree(tree) if tree.root.is_some()));
        // The design's window at one: the first screen 240 in, the page box
        // 81 in and 145 down in it, 878 × 520 — where a press on it aims.
        assert_eq!(
            page.at.0.to_bits(),
            (240.0_f64 + 81.0).to_bits(),
            "{:?}",
            page.at
        );
        assert_eq!(page.at.1.to_bits(), 145.0_f64.to_bits(), "{:?}", page.at);
        assert_eq!(
            (page.at.2.to_bits(), page.at.3.to_bits()),
            (878.0_f64.to_bits(), 520.0_f64.to_bits())
        );
        // Its card closed: the page is published no more.
        window.heard(Heard::Chrome(crate::conversation_chrome::Went::Shut(
            alo_url::parse("https://shop.example/").unwrap(),
        )));
        read(&mut window);
        assert!(
            window
                .take_published()
                .is_some_and(|now| now.page.is_none())
        );
    }

    #[test]
    fn a_press_on_the_page_is_carried_out_and_one_that_pays_waits_on_the_persons_pay() {
        let (mut window, told, acted) = with_the_checkout();
        next(&mut window, &told);
        let _ = window.take_said();
        let page = |node, assist| {
            Heard::Bus(FromTheBus::Page {
                node: BoxId::from_wire(node),
                assist,
            })
        };
        // A node not read, and text on a button: refused here.
        window.heard(page(99, Assist::Click));
        window.heard(page(8, Assist::SetValue("x".to_owned())));
        // *Say hello*: pressed, as the socket's activate.
        window.heard(page(8, Assist::Click));
        next(&mut window, &told);
        // *Pay*: held, asked on the card, nothing pressed.
        window.heard(page(7, Assist::Click));
        next(&mut window, &told);
        let asking = window.conversation().held_question().expect("asked");
        assert!(matches!(
            window
                .conversation()
                .card()
                .and_then(|card| card.agent.as_ref())
                .map(|work| &work.now),
            Some(alo_renderer::agent_work::Now::Asking {
                paying: Some(_),
                ..
            })
        ));
        assert_eq!(
            acted.lock().map(|acted| acted.clone()).unwrap_or_default(),
            [alo_agent::Verb::Activate]
        );
        // Another press waits for the person's answer.
        window.heard(page(8, Assist::Click));
        // *Not now* to another question answers nothing; *Pay* presses it.
        window.held_pressed(&Decided::NotNow(asking - 1));
        window.held_pressed(&Decided::Allow(asking));
        assert_eq!(window.conversation().held_question(), None);
        assert_eq!(
            window
                .conversation()
                .card()
                .and_then(|card| card.agent.as_ref()),
            None,
            "the card as it was"
        );
        next(&mut window, &told);
        assert_eq!(
            acted.lock().map(|acted| acted.clone()).unwrap_or_default(),
            [alo_agent::Verb::Activate, alo_agent::Verb::Activate]
        );
        // After the line saying the bus began listening (queue item 989).
        let written: Vec<&str> = window.assisted_record().entries().skip(1).collect();
        assert_eq!(
            written,
            [
                "assistive technology: a press on node 99 of the card's page was refused: no \
                 node of that number is published",
                "assistive technology: text put in, 1 characters on node 8 of the card's page \
                 was refused: that node is not offered it",
                "assistive technology: a press on node 8 of the card's page came to activated \
                 box#8",
                "assistive technology: a press on node 7 of the card's page pays, and waits on \
                 the person's answer: Press “Pay UGX 45,000” on shop.example? It may pay for \
                 something.",
                "assistive technology: a press on node 8 of the card's page was refused: a \
                 press on the page waits on the person's answer",
                "assistive technology: a press on node 7 of the card's page was allowed by the \
                 person",
                "assistive technology: a press on node 7 of the card's page came to activated \
                 box#7",
            ]
        );
        assert!(
            window.conversation().keys_to_the_card(),
            "the page's, as a press gives it"
        );
    }

    #[test]
    fn a_press_held_is_not_done_after_not_now_or_once_its_card_goes() {
        let (mut window, told, acted) = with_the_checkout();
        next(&mut window, &told);
        let pay = Heard::Bus(FromTheBus::Page {
            node: BoxId::from_wire(7),
            assist: Assist::Click,
        });
        window.heard(pay);
        next(&mut window, &told);
        let asking = window.conversation().held_question().expect("asked");
        window.held_pressed(&Decided::NotNow(asking));
        assert_eq!(window.conversation().held_question(), None);
        assert!(
            window
                .assisted_record()
                .entries()
                .last()
                .is_some_and(|line| line.ends_with("the person said not now"))
        );
        window.heard(Heard::Bus(FromTheBus::Page {
            node: BoxId::from_wire(7),
            assist: Assist::Click,
        }));
        next(&mut window, &told);
        assert!(window.conversation().held_question().is_some());
        window.heard(Heard::Chrome(crate::conversation_chrome::Went::Shut(
            alo_url::parse("https://shop.example/").unwrap(),
        )));
        assert_eq!(window.conversation().held_question(), None);
        assert!(
            window
                .assisted_record()
                .entries()
                .last()
                .is_some_and(|line| line.ends_with("its card went"))
        );
        assert!(
            acted.lock().is_ok_and(|acted| acted.is_empty()),
            "nothing pressed"
        );
    }

    /// The window giving the bus `bus`, as *Settings* would.
    fn giving(mut window: ConversationWindow, bus: Bus) -> ConversationWindow {
        window.conversation = std::mem::take(&mut window.conversation).giving_the_bus(bus);
        window
    }

    /// Queue item 989: under *Read only* every action the bus asks is
    /// refused and written, alo's own and the card's page's alike, and
    /// nothing is done; the tree is still published.
    #[test]
    fn read_only_refuses_every_action_and_writes_each() {
        let mut window = giving(window(), Bus::ReadOnly);
        window.heard(Heard::Bus(FromTheBus::Active));
        read(&mut window);
        let chats = window.conversation().showing().chats.len();
        window.heard(asked(Part::Panel, 12, Assist::Click));
        window.heard(asked(Part::Start, 40, Assist::SetValue("porto".to_owned())));
        window.heard(Heard::Bus(FromTheBus::Page {
            node: BoxId::from_wire(7),
            assist: Assist::Click,
        }));
        // Decided after *Settings* changed: refused too.
        window.heard(Heard::Drawn(Drawn::Assisted(
            Part::Panel,
            Assist::Click,
            Box::new(Decided::NewChat),
        )));
        assert_eq!(window.conversation().showing().chats.len(), chats);
        assert_eq!(window.conversation().field(Field::TheBox).text(), "");
        let written: Vec<&str> = window.assisted_record().entries().skip(1).collect();
        assert_eq!(
            written,
            [
                "assistive technology: a press on node 12 of the chats panel was refused: \
                 Settings › Screen readers is set to Read only",
                "assistive technology: text put in, 5 characters on node 40 of the first screen \
                 was refused: Settings › Screen readers is set to Read only",
                "assistive technology: a press on node 7 of the card's page was refused: \
                 Settings › Screen readers is set to Read only",
                "assistive technology: a press on the chats panel was refused: Settings › \
                 Screen readers is set to Read only",
            ]
        );
        let published = window.take_published().expect("still published");
        assert!(published.panel.is_some() && published.start.is_some());
    }

    /// Queue item 989: *Off* publishes the window alone — no part, no page
    /// — refuses every action, and a person's press on *Read and act*
    /// publishes everything again at once.
    #[test]
    fn off_publishes_the_window_alone_until_a_person_chooses_otherwise() {
        let mut window = giving(window(), Bus::Off);
        window.heard(Heard::Bus(FromTheBus::Active));
        read(&mut window);
        let published = window.take_published().expect("the window");
        assert_eq!(
            (
                published.panel.is_none(),
                published.start.is_none(),
                published.page.is_none(),
                published.keyboard
            ),
            (true, true, true, None)
        );
        window.heard(asked(Part::Panel, 12, Assist::Click));
        assert!(
            window
                .assisted_record()
                .entries()
                .last()
                .is_some_and(|line| line.ends_with("is set to Off")),
            "{:?}",
            window.assisted_record()
        );
        // A person's own press on *Read and act*.
        window.heard(Heard::Drawn(Drawn::Pressed(
            Part::Start,
            Decided::Bus(Bus::ReadAndAct),
        )));
        assert_eq!(window.conversation().settings().bus(), Bus::ReadAndAct);
        let published = window.take_published().expect("everything again");
        assert!(published.panel.is_some() && published.start.is_some());
    }

    /// Queue item 989: a press from the bus that comes to the switch is
    /// refused, whichever choice it is on — only a person's press changes
    /// it.
    #[test]
    fn a_press_from_the_bus_on_the_switch_itself_is_refused() {
        let mut window = window();
        for bus in Bus::ALL {
            window.heard(Heard::Drawn(Drawn::Assisted(
                Part::Start,
                Assist::Click,
                Box::new(Decided::Bus(bus)),
            )));
            assert_eq!(window.conversation().settings().bus(), Bus::ReadAndAct);
        }
        assert_eq!(
            window.assisted_record().entries().last(),
            Some(
                "assistive technology: a press on the first screen was refused: it came to \
                 Off's choice of Settings › Screen readers, which only a person's own press \
                 there changes"
            )
        );
        assert_eq!(window.assisted_record().entries().count(), 3);
    }

    /// Queue item 989: one line as the bus begins listening and one as it
    /// stops, with the time; asked again while it listens, and a hundred
    /// frames read meanwhile, no more.
    #[test]
    fn the_bus_beginning_and_stopping_listening_are_written_once_each() {
        let mut window = window();
        window.heard(Heard::Bus(FromTheBus::Active));
        window.heard(Heard::Bus(FromTheBus::Active));
        for _ in 0..100 {
            read(&mut window);
            let _ = window.take_published();
        }
        window.heard(Heard::Bus(FromTheBus::Inactive));
        window.heard(Heard::Bus(FromTheBus::Inactive));
        let written: Vec<&str> = window.assisted_record().entries().collect();
        assert_eq!(written.len(), 2, "{written:?}");
        assert!(
            written[0].starts_with("assistive technology: the accessibility bus began listening, ")
                && written[0].ends_with(" GMT"),
            "{written:?}"
        );
        assert!(
            written[1]
                .starts_with("assistive technology: the accessibility bus stopped listening, "),
            "{written:?}"
        );
    }

    /// Queue item 989 (ADR 0115, *Phase (c)'s guard*, (b)): agents kept to
    /// `example.com`, a card on `shop.example` has its page published as
    /// one node, *this page is kept from agents*, is never read for the
    /// bus, refuses a node of the page read before, and its address is
    /// said nowhere; the card on the named site is published whole again.
    #[test]
    fn a_card_on_a_kept_site_is_one_node_and_its_address_is_said_nowhere() {
        let (window, told, acted) = with_the_checkout();
        let mut window =
            window.with_agent_sites(AgentSites::named(["https://example.com"]).unwrap());
        // The page as it was read before the card was judged kept: never
        // published while it is.
        next(&mut window, &told);
        let start = Snapshot {
            root: Some({
                let mut region = node(41, KnownRole::Region, "shop.example/", false);
                region.children = vec![
                    node(42, KnownRole::Text, "shop.example", false),
                    node(
                        43,
                        KnownRole::Image,
                        alo_renderer::card_document::PICTURED,
                        false,
                    ),
                ];
                region
            }),
        };
        window.heard(Heard::Drawn(Drawn::Tree(Part::Start, Box::new(start))));
        let published = window.take_published().expect("published");
        let page = published.page.expect("the page's graft");
        assert_eq!(page.read, PageRead::Kept);
        let names: Vec<Option<String>> = published
            .start
            .expect("the first screen")
            .tree
            .nodes()
            .into_iter()
            .map(|node| node.name.clone())
            .collect();
        assert_eq!(
            names,
            [
                Some(assisted::KEPT.to_owned()),
                Some(assisted::KEPT.to_owned()),
                Some(alo_renderer::card_document::PICTURED.to_owned()),
            ]
        );
        // Nothing asked of the page for the bus while it is kept.
        window.read_page();
        assert!(
            told.recv_timeout(std::time::Duration::from_millis(500))
                .is_err()
        );
        window.heard(Heard::Bus(FromTheBus::Page {
            node: BoxId::from_wire(8),
            assist: Assist::Click,
        }));
        assert_eq!(
            window.assisted_record().entries().last(),
            Some(
                "assistive technology: a press on node 8 of the card's page was refused: the \
                 card's page is kept from agents"
            )
        );
        assert!(acted.lock().is_ok_and(|acted| acted.is_empty()));
        // On the site named: read, and published whole.
        window.heard(Heard::Chrome(crate::conversation_chrome::Went::Opening(
            alo_url::parse("https://example.com/").unwrap(),
        )));
        window.heard(Heard::Chrome(crate::conversation_chrome::Went::Framed(
            vec![1],
        )));
        let heard = told
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the reader tells");
        window.heard(Heard::Page(heard));
        read(&mut window);
        let published = window.take_published().expect("published");
        assert!(
            published
                .page
                .is_some_and(|page| matches!(page.read, PageRead::Tree(_)))
        );
    }
}
