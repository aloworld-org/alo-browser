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

use super::ConversationWindow;
use crate::assisted::{self, Asked, Assist, FromTheBus, Published, PublishedPart};
use crate::assisted_record::AssistedRecord;
use crate::conversation::{Field, Pressed as Decided};
use crate::conversation_drawing::Part;
use crate::conversation_place::ConversationPlacement;
use crate::typing::Edit;
use alo_renderer::Snapshot;

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
}

impl ConversationWindow {
    /// What the bus said.
    pub(super) fn bus_said(&mut self, from: FromTheBus) {
        match from {
            FromTheBus::Active => {
                self.assisting.active = true;
                self.assisting.fresh = true;
                self.assisting.changed = true;
                if let Some(drawing) = self.drawing.as_mut() {
                    drawing.read_trees();
                }
            }
            FromTheBus::Inactive => self.assisting.active = false,
            FromTheBus::Asked(asked) => self.assist_asked(asked),
            FromTheBus::Refused(refused) => {
                self.assisted_did(&format!("{} was refused: {}", refused.action, refused.why));
            }
        }
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
        let keyboard = if self.conversation.settings().is_open() {
            None
        } else {
            self.conversation.field_focused().map(|field| match field {
                Field::TheBox => Part::Start,
                Field::SearchChats => Part::Panel,
            })
        };
        self.assisting.changed = false;
        let fresh = core::mem::take(&mut self.assisting.fresh);
        Some(Published {
            scale,
            panel: self.assisting.panel.clone().map(|tree| PublishedPart {
                at: placed.panel,
                tree,
            }),
            start: self.assisting.start.clone().map(|tree| PublishedPart {
                at: placed.start,
                tree,
            }),
            keyboard,
            fresh,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let written: Vec<&str> = window.assisted_record().entries().collect();
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
}
