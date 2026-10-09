/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The DOM standard's *dispatch*, written once (ADR 0018 § 2, queue item
//! 254).
//!
//! It is a **stepper**: [`begin`] a dispatch, then ask [`next`] what to do —
//! *call this listener, with this `this`* or *done* — and say [`returned`]
//! when the call has finished. Its state is the event's ([`Progress`], held
//! by the [`Event`] cell), so whoever drives it keeps nothing but a step
//! number across a call: a script's `dispatchEvent` drives it from a native
//! that suspends once per listener ([`crate::interface::event_target`]), and
//! the renderer's event loop drives the same steps for the browser, with a
//! microtask checkpoint after each (ADR 0018 § 3, queue item 255).
//!
//! How a listener's callback is called — itself if it is a function, its
//! `handleEvent` if it is not — is [`invoke`], which both drivers ask, so
//! the two cannot come to disagree about it either.
//!
//! # The algorithm, as the standard has it
//!
//! - The **path** is the target and its ancestors up to the root of its
//!   tree, computed when the dispatch begins and not changed by a listener
//!   moving anything — then, when that root is the page's own document and
//!   the event is not a `load`, the page's **`Window`** (ADR 0037 § 3):
//!   HTML's *get the parent* of a `Document` is its relevant global object,
//!   unless the event is `load`. A dispatch **at** the window has a path of
//!   the window alone. A document no window was associated with has paths
//!   that stop at its root. There are no shadow trees and so no retargeting
//!   (item 87).
//! - The **capture pass** goes root to target, the **bubble pass** target to
//!   root — the second skipping every ancestor when the event does not
//!   bubble. At the target the phase is `AT_TARGET` in both, its capture
//!   listeners called in the first and the rest in the second.
//! - Each target's listeners are **copied when its turn comes**, as ids
//!   ([`crate::listeners`]): one added to it during its turn does not run,
//!   one removed does not either, and a `once` listener is removed before it
//!   is called.
//! - `stopPropagation` ends the dispatch at the next target;
//!   `stopImmediatePropagation` after the current listener.
//! - At the end the phase is `NONE`, `currentTarget` is `null`, the dispatch
//!   and stop flags are unset, and the answer is whether it was cancelled.
//!
//! A step of the path is an [`Entry`]: a node, or the window. The window is
//! found through the document cell's edge to it, so a target of either kind
//! is one more step of the same stepper, never a second one.
//!
//! # A node on the path is kept for as long as the dispatch
//!
//! The standard's path holds its targets strongly, so a listener that
//! detaches an ancestor and drops every reference to it still has that
//! ancestor's bubble listeners called. A node here is an id, so the document
//! cell is told which nodes are on a dispatch's path and keeps their trees and
//! wrappers through any collection until the dispatch ends
//! ([`DocumentCell`](crate::DocumentCell), [`crate::liveness`]). The window
//! needs no keeping: the realm roots it, and the document cell holds it.

use alo_dom::NodeId;
use alo_js::heap::{Barrier, Field, Ref, Tracer};
use alo_js::object::{Found, Objects, Value};
use alo_js::{Escape, Fault};

use crate::document_cell::DocumentCell;
use crate::embed;
use crate::event::{Event, Phase};
use crate::listeners;
use crate::tree;
use crate::window::Window;

/// Which of the two passes along the path a dispatch is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pass {
    Capturing,
    Bubbling,
}

/// One step of a dispatch's path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// A node of the document cell the path is in.
    Node(NodeId),
    /// That document's `Window`.
    Window,
}

impl Entry {
    /// The node, if it is one.
    pub const fn node(self) -> Option<NodeId> {
        match self {
            Self::Node(node) => Some(node),
            Self::Window => None,
        }
    }
}

/// The object an entry of the path in the document `cell` holds is: the
/// node's wrapper, if it has one, or the window.
pub fn object_of(objects: &Objects, cell: Ref, entry: Entry) -> Option<Ref> {
    let held = objects.embedded::<DocumentCell>(cell)?;
    match entry {
        Entry::Node(node) => held.wrapper(node),
        Entry::Window => held.window(),
    }
}

/// A dispatch in progress: the event holds it.
#[derive(Debug)]
pub struct Progress {
    /// The document cell whose nodes the path is.
    document: Field,
    /// The target, then each ancestor up to its tree's root, then the
    /// window when the path reaches it.
    path: Vec<Entry>,
    pass: Pass,
    /// Where on the path the dispatch is; [`None`] before the first target.
    at: Option<usize>,
    /// The ids of the current target's listeners for this type, copied when
    /// its turn came.
    listeners: Vec<u64>,
    /// The next of them to call.
    next: usize,
    /// The callback being called, held while it runs — a `once` listener is
    /// already out of its list, and its `handleEvent` is read after.
    invoking: Field,
}

impl Progress {
    /// Its references, as edges of the event that holds it.
    pub fn trace(&self, tracer: &mut Tracer) {
        self.document.trace(tracer);
        self.invoking.trace(tracer);
    }

    /// The bytes it owns: the path and the copy of a target's listeners.
    pub fn footprint(&self) -> usize {
        self.path
            .capacity()
            .saturating_mul(size_of::<Entry>())
            .saturating_add(self.listeners.capacity().saturating_mul(size_of::<u64>()))
    }

    /// The path, target first.
    pub fn path(&self) -> &[Entry] {
        &self.path
    }

    /// The document cell the path is in.
    pub const fn document(&self) -> Option<Ref> {
        self.document.get()
    }

    /// The callback being called.
    pub const fn invoking(&self) -> Option<Ref> {
        self.invoking.get()
    }

    /// Drop its references, as the dispatch ends.
    pub(crate) fn let_go(&mut self, barrier: &mut Barrier) {
        self.document.set(barrier, None);
        self.invoking.set(barrier, None);
    }
}

/// Why a dispatch could not begin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The event is being dispatched already: `InvalidStateError`.
    Dispatching,
    /// The object given is not an event.
    NotAnEvent,
    /// The target is not a node's wrapper or a `Window` with a document.
    NotATarget,
}

/// What the driver is to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Call this listener's callback — a function, or an object whose
    /// `handleEvent` is called — with `this` as the current target, then say
    /// [`returned`].
    Call {
        /// The callback.
        callback: Ref,
        /// The current target: a node's wrapper, or the window.
        this: Ref,
    },
    /// The dispatch is over; `canceled` is whether a listener cancelled it.
    Done {
        /// `defaultPrevented`, which `dispatchEvent` answers the opposite of.
        canceled: bool,
    },
}

/// How to call a listener's callback: the standard's *call a user object's
/// operation*, for `EventListener`'s one operation, `handleEvent`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Invoke {
    /// Call `callee` with `this` and the event. A callee that is not a
    /// function — a callback object with no `handleEvent` — is the
    /// `TypeError` calling it throws, which the standard reports.
    Call {
        /// What to call.
        callee: Value,
        /// Its `this`.
        this: Value,
    },
    /// `handleEvent` is a getter on the callback object: call `getter` with
    /// `this` and nothing else, then call what it answered with the same
    /// `this` and the event. A getter that throws is the listener throwing.
    Get {
        /// The getter.
        getter: Value,
        /// The callback object, the `this` of both calls.
        this: Ref,
    },
}

/// How to call `callback`, a listener [`next`] answered, whose current target
/// is `this`: a function is called with the current target as its `this`,
/// and any other object's `handleEvent` — looked up now, as the standard
/// says — with the object as its `this`.
///
/// Allocates nothing in the heap and runs nothing: a getter is answered for
/// the driver to call.
///
/// # Errors
///
/// A fault for a reference this engine has lost.
pub fn invoke(objects: &Objects, callback: Ref, this: Ref) -> Result<Invoke, Escape> {
    if objects.callable(callback).is_some() {
        return Ok(Invoke::Call {
            callee: Value::Object(callback),
            this: Value::Object(this),
        });
    }
    let name: Vec<u16> = "handleEvent".encode_utf16().collect();
    let found = match objects.existing_key(&name) {
        Some(key) => objects.get(callback, key)?,
        None => Found::Missing,
    };
    Ok(match found {
        Found::Value(callee) => Invoke::Call {
            callee,
            this: Value::Object(callback),
        },
        Found::Getter(getter) if getter != Value::Undefined => Invoke::Get {
            getter,
            this: callback,
        },
        // Not there: calling `undefined` is the `TypeError` the standard
        // reports for a `handleEvent` that is not callable.
        Found::Missing | Found::Getter(_) => Invoke::Call {
            callee: Value::Undefined,
            this: Value::Object(callback),
        },
    })
}

/// Begin dispatching `event` to `target`, a node's wrapper or a `Window` —
/// `trusted` when the browser dispatches it (ADR 0018 § 4).
///
/// Allocates nothing in the heap.
///
/// # Errors
///
/// [`Refusal`]: an event already being dispatched, or a value that is not an
/// event or not an event target.
pub fn begin(objects: &mut Objects, event: Ref, target: Ref, trusted: bool) -> Result<(), Refusal> {
    let kind = match objects.embedded::<Event>(event) {
        Some(held) if held.dispatching() => return Err(Refusal::Dispatching),
        Some(held) => held.kind().to_vec(),
        None => return Err(Refusal::NotAnEvent),
    };
    let (cell, path) = if let Some((cell, node)) = embed::node_of(objects, target) {
        (cell, path_from(objects, cell, node, &kind)?)
    } else {
        let cell = objects
            .embedded::<Window>(target)
            .and_then(Window::document)
            .ok_or(Refusal::NotATarget)?;
        (cell, vec![Entry::Window])
    };
    let nodes: Vec<NodeId> = path.iter().filter_map(|entry| entry.node()).collect();
    objects.write_embedded::<DocumentCell, _>(cell, |held, _| held.enter_path(&nodes));
    let started = objects.write_embedded::<Event, _>(event, |held, barrier| {
        let mut document = Field::empty();
        document.set(barrier, Some(cell));
        let progress = Progress {
            document,
            path,
            pass: Pass::Capturing,
            at: None,
            listeners: Vec::new(),
            next: 0,
            invoking: Field::empty(),
        };
        held.start(barrier, target, trusted, progress);
    });
    started.ok_or(Refusal::NotAnEvent)
}

/// The path of an event of type `kind` dispatched at `node`, in the document
/// `cell` holds: the node and its ancestors up to their root, then the
/// window when that root is the document and `kind` is not `load`.
fn path_from(
    objects: &Objects,
    cell: Ref,
    node: NodeId,
    kind: &[u16],
) -> Result<Vec<Entry>, Refusal> {
    let held = objects
        .embedded::<DocumentCell>(cell)
        .ok_or(Refusal::NotATarget)?;
    let document = held.document();
    let mut path = vec![Entry::Node(node)];
    let limit = tree::budget(document);
    let mut at = node;
    while let Some(parent) = document.parent(at) {
        if path.len() > limit {
            // A cycle, which `alo-dom`'s validity rules make impossible: the
            // path stops rather than going round for ever.
            break;
        }
        path.push(Entry::Node(parent));
        at = parent;
    }
    let load = "load".encode_utf16().eq(kind.iter().copied());
    if at == document.root() && held.window().is_some() && !load {
        path.push(Entry::Window);
    }
    Ok(path)
}

/// What to do next in `event`'s dispatch.
///
/// Allocates nothing in the heap, so the driver may build its call from
/// what this answers.
///
/// # Errors
///
/// A fault if `event` is not an event being dispatched, which the driver
/// began.
pub fn next(objects: &mut Objects, event: Ref) -> Result<Next, Escape> {
    loop {
        let (cell, current, pending) = {
            let held = objects
                .embedded::<Event>(event)
                .ok_or(Escape::fault(Fault::NotAnObject))?;
            let progress = held.progress().ok_or(Escape::fault(Fault::Gone))?;
            let cell = progress.document().ok_or(Escape::fault(Fault::Gone))?;
            (
                cell,
                held.current_target(),
                progress.listeners.get(progress.next).copied(),
            )
        };

        if let Some(id) = pending {
            if let Some(called) = take_listener(objects, event, current, id)? {
                return Ok(called);
            }
            continue;
        }

        if !advance(objects, event, cell)? {
            return finish(objects, event, cell);
        }
    }
}

/// A listener the driver was told to call has returned, or thrown and been
/// reported: the passive flag is unset, and `stopImmediatePropagation`
/// ends the current target's turn.
pub fn returned(objects: &mut Objects, event: Ref) {
    objects.write_embedded::<Event, _>(event, |held, barrier| {
        held.calling(false);
        let immediate = held.stopped_immediately();
        if let Some(progress) = held.progress_mut() {
            progress.invoking.set(barrier, None);
            if immediate {
                progress.next = progress.listeners.len();
            }
        }
    });
}

/// The listener `id` of the current target, if it is to be called now: its
/// turn taken, a `once` listener removed, the passive flag set.
fn take_listener(
    objects: &mut Objects,
    event: Ref,
    current: Option<Ref>,
    id: u64,
) -> Result<Option<Next>, Escape> {
    let pass = objects
        .write_embedded::<Event, _>(event, |held, _| {
            held.progress_mut().map(|progress| {
                progress.next = progress.next.saturating_add(1);
                progress.pass
            })
        })
        .flatten()
        .ok_or(Escape::fault(Fault::Gone))?;
    let Some(target) = current else {
        return Ok(None);
    };
    let Some((callback, capture, once, passive)) = listeners::of(objects, target)
        .and_then(|list| list.get(id))
        .and_then(|listener| {
            Some((
                listener.callback()?,
                listener.capture(),
                listener.once(),
                listener.passive(),
            ))
        })
    else {
        // Removed since its target's turn began.
        return Ok(None);
    };
    let wanted = match pass {
        Pass::Capturing => capture,
        Pass::Bubbling => !capture,
    };
    if !wanted {
        return Ok(None);
    }
    if once {
        listeners::change(objects, target, |list, barrier| list.remove_id(barrier, id));
    }
    objects.write_embedded::<Event, _>(event, |held, barrier| {
        held.calling(passive);
        if let Some(progress) = held.progress_mut() {
            progress.invoking.set(barrier, Some(callback));
        }
    });
    Ok(Some(Next::Call {
        callback,
        this: target,
    }))
}

/// Move to the next target on the path, copying its listeners, and say
/// whether there was one — none after `stopPropagation`, or past the end.
fn advance(objects: &mut Objects, event: Ref, cell: Ref) -> Result<bool, Escape> {
    loop {
        let (stopped, bubbles, pass, at, length) = {
            let held = objects
                .embedded::<Event>(event)
                .ok_or(Escape::fault(Fault::NotAnObject))?;
            let progress = held.progress().ok_or(Escape::fault(Fault::Gone))?;
            (
                held.propagation_stopped(),
                held.init(crate::event::Init::Bubbles),
                progress.pass,
                progress.at,
                progress.path.len(),
            )
        };
        if stopped {
            return Ok(false);
        }
        let (pass, at) = match (pass, at) {
            (Pass::Capturing, None) => (Pass::Capturing, length.saturating_sub(1)),
            (Pass::Capturing, Some(0)) => (Pass::Bubbling, 0),
            (Pass::Capturing, Some(at)) => (Pass::Capturing, at.saturating_sub(1)),
            (Pass::Bubbling, Some(at)) => (Pass::Bubbling, at.saturating_add(1)),
            (Pass::Bubbling, None) => return Err(Escape::fault(Fault::Gone)),
        };
        if at >= length {
            return Ok(false);
        }
        let phase = match (pass, at) {
            (_, 0) => Phase::AtTarget,
            (Pass::Capturing, _) => Phase::Capturing,
            (Pass::Bubbling, _) => Phase::Bubbling,
        };

        // Where the dispatch is, recorded first, so that an ancestor skipped
        // because the event does not bubble is passed rather than reached.
        let entry = objects
            .write_embedded::<Event, _>(event, |held, _| {
                held.progress_mut().and_then(|progress| {
                    progress.pass = pass;
                    progress.at = Some(at);
                    progress.listeners.clear();
                    progress.next = 0;
                    progress.path.get(at).copied()
                })
            })
            .flatten()
            .ok_or(Escape::fault(Fault::Gone))?;
        if phase == Phase::Bubbling && !bubbles {
            continue;
        }

        let kind = objects
            .embedded::<Event>(event)
            .map(|held| held.kind().to_vec())
            .unwrap_or_default();
        let current = object_of(objects, cell, entry);
        let listeners = current
            .and_then(|target| listeners::of(objects, target))
            .map(|list| list.matching(&kind))
            .unwrap_or_default();
        objects.write_embedded::<Event, _>(event, |held, barrier| {
            held.reach(barrier, phase, current);
            if let Some(progress) = held.progress_mut() {
                progress.listeners = listeners;
            }
        });
        return Ok(true);
    }
}

/// End the dispatch, let the document cell stop keeping its path, and say
/// whether it was cancelled.
fn finish(objects: &mut Objects, event: Ref, cell: Ref) -> Result<Next, Escape> {
    let (canceled, progress) = objects
        .write_embedded::<Event, _>(event, |held, barrier| {
            let progress = held.finish(barrier);
            (held.canceled(), progress)
        })
        .ok_or(Escape::fault(Fault::NotAnObject))?;
    if let Some(progress) = progress {
        let nodes: Vec<NodeId> = progress
            .path
            .iter()
            .filter_map(|entry| entry.node())
            .collect();
        objects.write_embedded::<DocumentCell, _>(cell, |held, _| held.leave_path(&nodes));
    }
    Ok(Next::Done { canceled })
}
