/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The conversation's window and alo's two fields — the one box and
//! *Search chats* — as text fields a person types into (queue items 861
//! and 848, `docs/design/shell.md`, *The one box as a text field*).
//!
//! The field's words, caret and selection are the conversation's
//! ([`crate::text_field`]); this is what the window adds to them:
//!
//! - **The clipboard**: ⌘C and ⌘X write it as *Copy link* does, and ⌘V
//!   reads it with `pbpaste` ([`crate::clipboard::paste`]) — or a test's
//!   own reader ([`ConversationWindow::paste_with`]), so no test touches a
//!   person's clipboard.
//! - **The kill buffer** (queue item 865): what ⌃K last killed in either
//!   field, put back by ⌃Y; the window's own, never the clipboard.
//! - **A press and a drag**: a press decided as the field puts the caret
//!   where it was, by where its renderer said each place the caret may
//!   stand is ([`alo_renderer::field::FieldLaidOut`]); a second quick press
//!   at one place selects the word, a third everything; the pointer moved
//!   with the button still down after one selects from it. A press with ⇧
//!   alone grows the field's selection to it (queue item 863) — claimed as
//!   the field's only when its renderer decides it landed on one, and
//!   refused and said, as every other press with a modifier is, when not.
//! - **The caret's blink** ([`crate::field_blink`]): shown while the window
//!   and the field have the keyboard and nothing is selected, in its shown
//!   half — and the words' scroll and room, as the renderer last laid them,
//!   sent back with the next drawing so the caret stays in view.
//! - **Input methods**: allowed while a field has the keyboard, as they are
//!   while a card's page has it (queue item 802), the composition the
//!   field's and the candidates put under its caret.

use super::ConversationWindow;
use crate::card_clicks::Clicks;
use crate::conversation::{Field, Pressed as Decided};
use crate::conversation_box::Typed;
use crate::conversation_drawing::Part;
use crate::field_blink::Blink;
use crate::keys::Press;
use crate::typing::{self, Edit};
use alo_renderer::field::FieldLaidOut;
use alo_renderer::panel::Panel;
use alo_renderer::start::Start;
use std::time::Instant;

/// What reads the system's clipboard for a paste: `pbpaste`, or a test's
/// own ([`ConversationWindow::paste_with`]).
type Paster = Box<dyn Fn() -> Result<String, String>>;

/// What the window holds of its fields.
pub(super) struct Fields {
    /// What reads the clipboard.
    paste: Paster,
    /// When the caret last moved.
    blink: Blink,
    /// Where each part's field was last laid out: the panel's, then the
    /// first screen's.
    laid: (Option<FieldLaidOut>, Option<FieldLaidOut>),
    /// Whether the caret was drawn in the drawing last asked.
    shown: bool,
    /// The presses counted on the parts, in their CSS pixels.
    clicks: Clicks,
    /// The last press on a part, where on it and its count, until it is
    /// decided.
    press: Option<(Part, alo_paint::Point, u32)>,
    /// Whether the main button is down.
    down: bool,
    /// The field a press is selecting in while the button is down.
    dragging: Option<Field>,
    /// Whether the press kept until it is decided was made with ⇧ alone
    /// (queue item 863).
    shifted: bool,
    /// What ⌃K last killed, in either field, for ⌃Y (queue item 865): the
    /// window's own, as a Mac's kill buffer is its application's, and never
    /// the clipboard.
    killed: String,
}

impl Default for Fields {
    fn default() -> Self {
        Self {
            paste: Box::new(crate::clipboard::paste),
            blink: Blink::default(),
            laid: (None, None),
            shown: false,
            clicks: Clicks::default(),
            press: None,
            down: false,
            dragging: None,
            shifted: false,
            killed: String::new(),
        }
    }
}

/// The part a field is drawn in.
fn part_of(field: Field) -> Part {
    match field {
        Field::TheBox => Part::Start,
        Field::SearchChats => Part::Panel,
    }
}

impl Fields {
    /// Where `part`'s field was last laid out.
    fn laid(&self, part: Part) -> Option<&FieldLaidOut> {
        match part {
            Part::Panel => self.laid.0.as_ref(),
            Part::Start => self.laid.1.as_ref(),
        }
    }
}

impl ConversationWindow {
    /// Read the clipboard with `paste` rather than the system's: what a
    /// test of ⌘V sets, so no test touches a person's clipboard (queue item
    /// 861).
    pub fn paste_with(&mut self, paste: impl Fn() -> Result<String, String> + 'static) {
        self.fields.paste = Box::new(paste);
    }

    /// The field that has the keyboard, if one does and the card's page
    /// does not.
    fn field_with_keys(&self) -> Option<Field> {
        if self.conversation.keys_to_the_card() {
            return None;
        }
        self.conversation.field_focused()
    }

    /// The caret moved, or the field took the keyboard, `now`: shown at
    /// once.
    fn caret_moved(&mut self, now: Instant) {
        self.fields.blink.restart(now);
    }

    /// Whether `field`'s caret is drawn at `now`: it and the window have
    /// the keyboard, nothing is selected and the blink is in its shown half.
    fn caret_shown(&self, field: Field, now: Instant) -> bool {
        self.front.focused
            && self.field_with_keys() == Some(field)
            && self.conversation.field(field).selection().is_none()
            && self.fields.blink.shown(now)
    }

    /// Put into `panel` and `start` how their fields' carets stand at `now`,
    /// and how far their words were last scrolled in what room, before they
    /// are asked drawn.
    pub(super) fn dress_fields(&mut self, panel: &mut Panel, start: &mut Start, now: Instant) {
        let box_shown = self.caret_shown(Field::TheBox, now);
        let find_shown = self.caret_shown(Field::SearchChats, now);
        start.editing.caret_shown = box_shown;
        panel.editing.caret_shown = find_shown;
        self.fields.shown = box_shown || find_shown;
        if let Some(laid) = self.fields.laid(Part::Start) {
            start.editing.scrolled = laid.scrolled;
            start.editing.room = Some(laid.room);
        }
        if let Some(laid) = self.fields.laid(Part::Panel) {
            panel.editing.scrolled = laid.scrolled;
            panel.editing.room = Some(laid.room);
        }
    }

    /// Where `part`'s field was laid out as it was just drawn (queue item
    /// 861): kept — and the part drawn again when its words' room changed
    /// from the one it was drawn for, or was not known, so the caret is
    /// kept in the room it really has.
    pub(super) fn field_laid(&mut self, part: Part, laid: Option<FieldLaidOut>) {
        let room = |laid: Option<&FieldLaidOut>| laid.map(|laid| laid.room.to_bits());
        let before = room(self.fields.laid(part));
        let now = room(laid.as_ref());
        match part {
            Part::Panel => self.fields.laid.0 = laid,
            Part::Start => self.fields.laid.1 = laid,
        }
        let focused = self
            .field_with_keys()
            .is_some_and(|field| part_of(field) == part);
        if focused && now.is_some() && now != before {
            self.ask_drawn();
        }
    }

    /// A key went down, read as `press`, writing `text`, while a field has
    /// the keyboard: what it came to — the clipboard read for ⌘V and
    /// written for ⌘C and ⌘X, the caret shown at once after an edit — or
    /// nothing, when it is no edit.
    pub(super) fn field_key(&mut self, press: &Press, text: Option<&str>) -> Option<Typed> {
        let edit = typing::edit(press, text)?;
        let edit = match edit {
            Edit::Paste => match (self.fields.paste)() {
                Ok(pasted) => Edit::Pasted(pasted),
                Err(why) => {
                    self.tell(format!("nothing was pasted: {why}"));
                    self.redraw = true;
                    return Some(Typed::Unchanged);
                }
            },
            // ⌃Y puts in what the window last killed (queue item 865).
            Edit::Yank => Edit::Yanked(self.fields.killed.clone()),
            other => other,
        };
        // ⌃K: what it takes, read before it goes, kept if it went.
        let killing = (edit == Edit::Kill)
            .then(|| self.field_with_keys())
            .flatten()
            .map(|field| self.conversation.field(field).to_be_killed());
        let typed = self.conversation.edit(edit);
        if let Some(killed) = killing
            && typed == Typed::Changed
        {
            self.fields.killed = killed;
        }
        match &typed {
            Typed::Copy(text) | Typed::Cut(text) => {
                if let Err(why) = (self.clipboard)(text) {
                    self.tell(format!("the clipboard was not written: {why}"));
                    self.redraw = true;
                }
            }
            Typed::Unchanged | Typed::Changed | Typed::Chose(_) => {}
        }
        if matches!(typed, Typed::Changed | Typed::Cut(_)) {
            let now = self.now();
            self.caret_moved(now);
        }
        Some(typed)
    }

    /// The main button went down `now` on `part` at `on`, in its CSS
    /// pixels: counted, and kept until its renderer's claim is decided.
    pub(super) fn field_pressed_at(&mut self, part: Part, on: alo_paint::Point, now: Instant) {
        let count = self
            .fields
            .clicks
            .pressed(now, (f64::from(on.x), f64::from(on.y)));
        self.fields.press = Some((part, on, count));
        self.fields.down = true;
        self.fields.dragging = None;
        self.fields.shifted = false;
    }

    /// The main button went down `now` on `part` at `on` with ⇧ alone held
    /// (queue item 863): kept as [`Self::field_pressed_at`] keeps a press,
    /// to grow a field's selection if it is decided as one.
    pub(super) fn field_shift_pressed_at(
        &mut self,
        part: Part,
        on: alo_paint::Point,
        now: Instant,
    ) {
        self.field_pressed_at(part, on, now);
        self.fields.shifted = true;
    }

    /// Whether a press decided as `pressed` was made with ⇧ and is to be
    /// refused: decided as anything but a field — or refused or unanswered,
    /// which says so itself — it is nobody's, as every press with a
    /// modifier is (queue items 798, 805, 863), and said.
    pub(super) fn shift_press_refused(&mut self, pressed: &Decided) -> bool {
        if !self.fields.shifted {
            return false;
        }
        if matches!(
            pressed,
            Decided::TheBox | Decided::SearchChats | Decided::Refused(_) | Decided::Failed(_)
        ) {
            return false;
        }
        self.fields.shifted = false;
        self.fields.press = None;
        self.tell(super::held_press(
            crate::keys::Held::NOTHING.with(crate::keys::Modifier::Shift),
        ));
        self.redraw = true;
        true
    }

    /// A press on `part` was decided as `pressed`, and acted on: decided as
    /// a field, its caret placed where it was — a word selected by a second
    /// quick press, everything by a third — and, the button still down, a
    /// drag selects from there. Whether the field changed.
    pub(super) fn field_press_decided(&mut self, part: Part, pressed: &Decided) -> bool {
        let field = match pressed {
            Decided::TheBox => Field::TheBox,
            Decided::SearchChats => Field::SearchChats,
            _ => return false,
        };
        let Some((pressed_on, on, count)) = self.fields.press.take() else {
            return false;
        };
        if pressed_on != part || part_of(field) != part {
            return false;
        }
        let Some(at) = self.fields.laid(part).map(|laid| laid.index_at(on.x)) else {
            return false;
        };
        let Some(text) = self.conversation.field_mut() else {
            return false;
        };
        let shifted = std::mem::replace(&mut self.fields.shifted, false);
        let changed = match count {
            // ⇧ grows the selection to it, however quick (queue item 863).
            _ if shifted => text.grow_to(at),
            1 => text.place(at),
            2 => text.select_word(at),
            _ => text.select_all(),
        };
        if (count == 1 || shifted) && self.fields.down {
            self.fields.dragging = Some(field);
        }
        let now = self.now();
        self.caret_moved(now);
        changed
    }

    /// The pointer moved to `on` over `part`, in its CSS pixels: while a
    /// press is selecting in that part's field, the selection runs to the
    /// place nearest it. Whether the field changed.
    pub(super) fn field_dragged(&mut self, part: Part, on: alo_paint::Point) -> bool {
        let Some(field) = self.fields.dragging else {
            return false;
        };
        if part_of(field) != part || self.field_with_keys() != Some(field) {
            return false;
        }
        let Some(at) = self.fields.laid(part).map(|laid| laid.index_at(on.x)) else {
            return false;
        };
        self.conversation
            .field_mut()
            .is_some_and(|text| text.extend_to(at))
    }

    /// The main button came up: no drag selects any more.
    pub(super) fn field_released(&mut self) {
        self.fields.down = false;
        self.fields.dragging = None;
    }

    /// An input method's composition, `text` with its caret at the byte
    /// `caret` of it, in the field with the keyboard (queue item 848):
    /// whether it was the field's.
    pub(super) fn field_composing(&mut self, text: &str, caret: Option<usize>) -> bool {
        if self.field_with_keys().is_none() {
            return false;
        }
        let caret = caret.map_or(text.chars().count(), |byte| {
            text.get(..byte)
                .map_or(text.chars().count(), |before| before.chars().count())
        });
        let changed = self
            .conversation
            .field_mut()
            .is_some_and(|field| field.compose(text, caret));
        if changed {
            self.ask_drawn();
            self.redraw = true;
        }
        true
    }

    /// What an input method's composition came to, `text`, put into the
    /// field with the keyboard (queue item 848): whether it was the field's.
    pub(super) fn field_committed(&mut self, text: &str) -> bool {
        if self.field_with_keys().is_none() {
            return false;
        }
        if self
            .conversation
            .field_mut()
            .is_some_and(|field| field.commit(text))
        {
            let now = self.now();
            self.caret_moved(now);
            self.ask_drawn();
            self.redraw = true;
        }
        true
    }

    /// Whether input methods are to be allowed for a field: one has the
    /// keyboard.
    pub(super) fn field_takes_input_methods(&self) -> bool {
        self.field_with_keys().is_some()
    }

    /// The rectangle of the caret of the field with the keyboard, in the
    /// window's device pixels — left, top, width and height — as its part
    /// was last laid out, for an input method's candidates (queue item
    /// 848).
    pub(super) fn field_caret_in_the_window(&self) -> Option<(f64, f64, f64, f64)> {
        let field = self.field_with_keys()?;
        let part = part_of(field);
        let laid = self.fields.laid(part)?;
        let held = self.conversation.field(field);
        // A composition stands in place of the selection, its own caret
        // inside it.
        let caret = match held.composing() {
            Some(composing) => {
                held.selection().map_or(held.caret(), |(start, _)| start) + composing.caret
            }
            None => held.caret(),
        };
        let scene = self.scene()?;
        let factor = f64::from(scene.scale.factor());
        let placed = crate::conversation_place::ConversationPlacement::of(
            scene.window,
            scene.scale,
            scene.parts(),
        );
        let (x, y) = match part {
            Part::Start => (f64::from(placed.start.x), f64::from(placed.start.y)),
            Part::Panel => match self.sheet_at(self.now()) {
                Some((left, _, _)) => (f64::from(left), 0.0),
                None => (f64::from(placed.panel.x), f64::from(placed.panel.y)),
            },
        };
        let left = f64::from(laid.left) + f64::from(laid.x_of(caret));
        Some((
            x + left * factor,
            y + f64::from(laid.top) * factor,
            factor,
            f64::from(laid.height) * factor,
        ))
    }

    /// When the caret is next to blink, after `now`, while it blinks.
    pub(super) fn field_wakes_at(&self, now: Instant) -> Option<Instant> {
        let field = self.field_with_keys()?;
        (self.front.focused && self.conversation.field(field).selection().is_none())
            .then(|| self.fields.blink.next_change(now))
    }

    /// The time is `now`: the caret drawn again if it blinked since it was
    /// last asked drawn.
    pub(super) fn field_tick(&mut self, now: Instant) {
        let shown = self
            .field_with_keys()
            .is_some_and(|field| self.caret_shown(field, now));
        if shown != self.fields.shown {
            self.ask_drawn();
            self.redraw = true;
        }
    }
}
