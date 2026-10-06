/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An event, as a script holds it (ADR 0018 §§ 2 and 8, queue item 254).
//!
//! An embedder cell, like a [`Wrapper`](crate::Wrapper): the event's own
//! state — its type, its three init flags, its phase, its target and current
//! target, the flags its listeners set — and an ordinary object's part for
//! whatever a page hangs off it. A `CustomEvent` is the same cell with its
//! `detail`, and a `PointerEvent` the same cell with its [`Shape`] saying so.
//!
//! # A dispatch's state lives here
//!
//! ADR 0018 § 2: the standard keeps a dispatch's state in the event —
//! `eventPhase`, `currentTarget` and `composedPath()` are read from it by
//! the listeners — and so does this. While an event is being dispatched it
//! holds the dispatch's [`Progress`]: the path, where in it the dispatch
//! is, and the copy of the current target's listeners. That is what lets the
//! native that drives a dispatch keep nothing across a suspension but a step
//! number, as `alo-js`'s natives must ([`crate::dispatch`]).
//!
//! # An event the browser fires
//!
//! A page makes its events with `new Event(…)`; the browser makes its own
//! with [`create`], the standard's *create an event*: an `Event` — or a
//! `PointerEvent`, for a click (queue item 256) — inheriting from the
//! prototype the page's document cell holds, its type and init flags as the
//! browser gives them ([`Firing`]), and nothing else. Its dispatch is
//! the event loop's (queue item 255), and is trusted (ADR 0018 § 4).
//!
//! It is given `isTrusted` there as the constructors give it, from the cell's
//! unforgeables ([`crate::unforgeable`], ADR 0019 § 3): an own accessor that
//! reads the trusted flag kept here, which the dispatch sets as ADR 0018 § 4
//! says.
//!
//! # What is not here
//!
//! `timeStamp` is a clock and item 92's; `returnValue`, `cancelBubble`,
//! `srcElement` and `initEvent` are law 1's.

use alo_js::heap::{Barrier, Field, Ref, Trace, Tracer};
use alo_js::object::{Exotic, Internal, Key, Objects, Ordinary, Property, Stored, Value};
use alo_js::{Escape, Fault};

use crate::dispatch::Progress;
use crate::document_cell::DocumentCell;
use crate::interface::Interface;
use crate::unforgeable;

/// Which phase a dispatch is in, as `eventPhase` answers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Not being dispatched.
    None,
    /// On the way down, at an ancestor of the target.
    Capturing,
    /// At the target.
    AtTarget,
    /// On the way up, at an ancestor of the target.
    Bubbling,
}

impl Phase {
    /// The number `eventPhase` answers, which is the constant of the same
    /// name.
    pub const fn number(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Capturing => 1,
            Self::AtTarget => 2,
            Self::Bubbling => 3,
        }
    }
}

/// One of the init dictionary's three flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Init {
    /// `bubbles`.
    Bubbles,
    /// `cancelable`.
    Cancelable,
    /// `composed`.
    Composed,
}

impl Init {
    /// Where it is in an event's flags.
    const fn index(self) -> usize {
        match self {
            Self::Bubbles => 0,
            Self::Cancelable => 1,
            Self::Composed => 2,
        }
    }
}

/// Which interface an event is an instance of, beyond `Event`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// An `Event`, and nothing more.
    Event,
    /// A `CustomEvent`, which has a `detail`.
    Custom,
    /// A `PointerEvent`, and so a `MouseEvent` and a `UIEvent` (ADR 0018
    /// § 5). The only one this engine makes is a click no pointing device
    /// caused — the browser's for an agent's `Activate` — so its members
    /// are that click's and are not held: every coordinate `0`, no button,
    /// no modifier, `pointerId` `-1`, `pointerType` `""`. A pointer that
    /// really points brings fields with it, with the window that has one.
    Pointer,
}

/// How far a listener has stopped a dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// It has not.
    No,
    /// `stopPropagation`: no target after this one.
    Propagation,
    /// `stopImmediatePropagation`: no listener after this one.
    Immediately,
}

/// An `Event` or a `CustomEvent`.
///
/// The dispatch flag is having a [`Progress`]: one is kept from the moment a
/// dispatch begins until it ends.
#[derive(Debug)]
pub struct Event {
    own: Ordinary,
    shape: Shape,
    kind: Vec<u16>,
    /// `bubbles`, `cancelable` and `composed`, in [`Init`]'s order.
    init: [bool; 3],
    trusted: bool,
    phase: Phase,
    stop: Stop,
    canceled: bool,
    in_passive_listener: bool,
    target: Field,
    current_target: Field,
    detail: Stored,
    progress: Option<Progress>,
}

impl Event {
    /// A new event of `shape` inheriting from `prototype`, with the empty
    /// type and every flag unset: what the constructor then initializes.
    pub fn new(prototype: Option<Ref>, shape: Shape) -> Self {
        Self {
            own: Ordinary::with_prototype(prototype),
            shape,
            kind: Vec::new(),
            init: [false; 3],
            trusted: false,
            phase: Phase::None,
            stop: Stop::No,
            canceled: false,
            in_passive_listener: false,
            target: Field::empty(),
            current_target: Field::empty(),
            // A `CustomEvent`'s `detail` defaults to `null`.
            detail: Stored::holding(Value::Null),
            progress: None,
        }
    }

    /// Whether it is a `CustomEvent`.
    pub fn is_custom(&self) -> bool {
        self.shape == Shape::Custom
    }

    /// Whether it is a `PointerEvent` — and so a `MouseEvent` and a
    /// `UIEvent`, since no other instance of either is ever made.
    pub fn is_pointer(&self) -> bool {
        self.shape == Shape::Pointer
    }

    /// Its type, as the code units it was made with.
    pub fn kind(&self) -> &[u16] {
        &self.kind
    }

    /// One of its init flags.
    pub fn init(&self, which: Init) -> bool {
        self.init.get(which.index()).copied().unwrap_or(false)
    }

    /// Whether the browser dispatched it rather than a script (ADR 0018 § 4).
    pub const fn trusted(&self) -> bool {
        self.trusted
    }

    /// The phase it is in.
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// Whether a listener cancelled it: `defaultPrevented`.
    pub const fn canceled(&self) -> bool {
        self.canceled
    }

    /// Whether it is being dispatched now.
    pub const fn dispatching(&self) -> bool {
        self.progress.is_some()
    }

    /// Whether `stopPropagation` was called during this dispatch.
    pub fn propagation_stopped(&self) -> bool {
        self.stop != Stop::No
    }

    /// Whether `stopImmediatePropagation` was.
    pub fn stopped_immediately(&self) -> bool {
        self.stop == Stop::Immediately
    }

    /// The target it was last dispatched to, as its wrapper.
    pub const fn target(&self) -> Option<Ref> {
        self.target.get()
    }

    /// The target whose listeners are being called, as its wrapper.
    pub const fn current_target(&self) -> Option<Ref> {
        self.current_target.get()
    }

    /// A `CustomEvent`'s `detail`.
    pub const fn detail(&self) -> Value {
        self.detail.get()
    }

    /// The dispatch in progress, if there is one.
    pub const fn progress(&self) -> Option<&Progress> {
        self.progress.as_ref()
    }

    /// Set its type, as the constructor does.
    pub fn set_kind(&mut self, kind: Vec<u16>) {
        self.kind = kind;
    }

    /// Set an init flag, as the constructor does.
    pub fn set_init(&mut self, which: Init, to: bool) {
        if let Some(flag) = self.init.get_mut(which.index()) {
            *flag = to;
        }
    }

    /// Set a `CustomEvent`'s `detail`, as its constructor does.
    pub fn set_detail(&mut self, barrier: &mut Barrier, detail: Value) {
        self.detail.set(barrier, detail);
    }

    /// `stopPropagation()`.
    pub fn stop_propagation(&mut self) {
        if self.stop == Stop::No {
            self.stop = Stop::Propagation;
        }
    }

    /// `stopImmediatePropagation()`.
    pub const fn stop_immediate_propagation(&mut self) {
        self.stop = Stop::Immediately;
    }

    /// `preventDefault()`: cancels it if it is cancelable and no passive
    /// listener is running.
    pub fn prevent_default(&mut self) {
        if self.init(Init::Cancelable) && !self.in_passive_listener {
            self.canceled = true;
        }
    }

    /// Begin a dispatch to `target`: the dispatch flag set, `isTrusted` as
    /// the dispatcher says, and the dispatch's progress kept here.
    pub(crate) fn start(
        &mut self,
        barrier: &mut Barrier,
        target: Ref,
        trusted: bool,
        progress: Progress,
    ) {
        self.trusted = trusted;
        self.target.set(barrier, Some(target));
        self.progress = Some(progress);
    }

    /// The dispatch in progress, to move.
    pub(crate) const fn progress_mut(&mut self) -> Option<&mut Progress> {
        self.progress.as_mut()
    }

    /// Reach a new target on the path: its phase and its wrapper, if it has
    /// one.
    pub(crate) fn reach(&mut self, barrier: &mut Barrier, phase: Phase, current: Option<Ref>) {
        self.phase = phase;
        self.current_target.set(barrier, current);
    }

    /// A listener is about to be called: passive or not.
    pub(crate) const fn calling(&mut self, passive: bool) {
        self.in_passive_listener = passive;
    }

    /// End the dispatch: phase none, no current target, the dispatch and
    /// stop flags unset — `defaultPrevented` stays — and the progress handed
    /// back so its path can be let go of.
    pub(crate) fn finish(&mut self, barrier: &mut Barrier) -> Option<Progress> {
        self.phase = Phase::None;
        self.current_target.set(barrier, None);
        self.stop = Stop::No;
        self.in_passive_listener = false;
        let mut progress = self.progress.take();
        if let Some(progress) = progress.as_mut() {
            progress.let_go(barrier);
        }
        progress
    }
}

impl Internal for Event {
    fn own_property(&self, key: Key) -> Option<&Property> {
        self.own.own_property(key)
    }

    fn define_own(&mut self, barrier: &mut Barrier, key: Key, property: Property) -> bool {
        self.own.define_own(barrier, key, property)
    }

    fn delete_own(&mut self, key: Key) -> bool {
        self.own.delete_own(key)
    }

    fn own_keys(&self) -> Vec<Key> {
        self.own.own_keys()
    }

    fn prototype(&self) -> Option<Ref> {
        self.own.prototype()
    }

    fn set_prototype(&mut self, barrier: &mut Barrier, to: Option<Ref>) -> bool {
        self.own.set_prototype(barrier, to)
    }

    fn is_extensible(&self) -> bool {
        self.own.is_extensible()
    }

    fn prevent_extensions(&mut self) -> bool {
        self.own.prevent_extensions()
    }
}

impl Trace for Event {
    fn trace(&self, tracer: &mut Tracer) {
        self.own.trace(tracer);
        self.target.trace(tracer);
        self.current_target.trace(tracer);
        self.detail.trace(tracer);
        if let Some(progress) = &self.progress {
            progress.trace(tracer);
        }
    }

    fn footprint(&self) -> usize {
        self.own
            .footprint()
            .saturating_add(self.kind.capacity().saturating_mul(2))
            .saturating_add(self.progress.as_ref().map_or(0, Progress::footprint))
    }
}

impl Exotic for Event {
    fn describe(&self) -> &'static str {
        match self.shape {
            Shape::Event => "an Event",
            Shape::Custom => "a CustomEvent",
            Shape::Pointer => "a PointerEvent",
        }
    }
}

/// How the `Event` constructor makes its instance.
pub fn make_event(prototype: Option<Ref>) -> Box<dyn Exotic> {
    Box::new(Event::new(prototype, Shape::Event))
}

/// How the `CustomEvent` constructor makes its instance.
pub fn make_custom_event(prototype: Option<Ref>) -> Box<dyn Exotic> {
    Box::new(Event::new(prototype, Shape::Custom))
}

/// Which interface an event the browser fires is an instance of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fired {
    /// `Event`: what HTML's *fire an event* makes — an `input` or a
    /// `change`, say.
    Event,
    /// `PointerEvent`: a `click` (ADR 0018 § 5).
    PointerEvent,
}

impl Fired {
    /// The interface, and the event's shape.
    const fn interface(self) -> (Interface, Shape) {
        match self {
            Self::Event => (Interface::Event, Shape::Event),
            Self::PointerEvent => (Interface::PointerEvent, Shape::Pointer),
        }
    }
}

/// An event the browser fires: its interface, its type and its three init
/// flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Firing<'a> {
    /// Its interface.
    pub interface: Fired,
    /// Its type — `"click"`, say.
    pub kind: &'a str,
    /// `bubbles`.
    pub bubbles: bool,
    /// `cancelable`.
    pub cancelable: bool,
    /// `composed`.
    pub composed: bool,
}

impl Firing<'static> {
    /// The click keyboard activation fires, and so an agent's `Activate`
    /// (ADR 0018 § 5): a `PointerEvent` `click`, bubbling, cancelable and
    /// composed.
    pub const CLICK: Self = Self {
        interface: Fired::PointerEvent,
        kind: "click",
        bubbles: true,
        cancelable: true,
        composed: true,
    };

    /// The `input` a toggled checkbox or radio fires: an `Event`, bubbling
    /// and composed, not cancelable (HTML's activation behaviour for both).
    pub const INPUT: Self = Self {
        interface: Fired::Event,
        kind: "input",
        bubbles: true,
        cancelable: false,
        composed: true,
    };

    /// The `change` that follows it: an `Event`, bubbling and nothing more.
    pub const CHANGE: Self = Self {
        interface: Fired::Event,
        kind: "change",
        bubbles: true,
        cancelable: false,
        composed: false,
    };
}

/// Make the event `firing` describes, inheriting from its interface's
/// prototype as the document `cell` holds it and with the unforgeable
/// members of that interface and those it inherits — the standard's *create
/// an event*, for the browser's own dispatch.
///
/// **A safepoint.** `cell` must be rooted by the caller, and the event
/// answered is held by nothing: the caller holds it before anything else
/// allocates.
///
/// # Errors
///
/// [`Escape::Full`] when the heap cannot hold it; a fault when `cell` is not
/// a document cell whose interfaces have been made.
pub fn create(objects: &mut Objects, cell: Ref, firing: &Firing<'_>) -> Result<Ref, Escape> {
    let (interface, shape) = firing.interface.interface();
    let prototype = objects
        .embedded::<DocumentCell>(cell)
        .ok_or(Escape::fault(Fault::NotAnObject))?
        .interfaces()
        .prototype(interface)
        .ok_or(Escape::fault(Fault::Gone))?;
    let mut event = Event::new(Some(prototype), shape);
    event.set_kind(firing.kind.encode_utf16().collect());
    event.set_init(Init::Bubbles, firing.bubbles);
    event.set_init(Init::Cancelable, firing.cancelable);
    event.set_init(Init::Composed, firing.composed);
    let made = objects
        .foreign(Box::new(event))
        .map_err(|why| Escape::refused(why, 0))?;
    // Copying allocates nothing, so the event nothing holds yet survives it.
    unforgeable::copy(objects, cell, made, interface)?;
    Ok(made)
}
