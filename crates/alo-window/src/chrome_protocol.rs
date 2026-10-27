/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What a message to and from Chrome says (ADR 0058 § 3).
//!
//! Chrome's DevTools protocol: a command is `{"id", "method", "params"}`, and
//! Chrome answers with the same `id` and a `result` or an `error`, or tells of
//! something on its own with a `method` and no `id`. [`crate::chrome_wire`]
//! finds where a message ends; this reads one into a [`Message`] and writes a
//! [`Command`].
//!
//! It names `serde_json`, which is rented, as [`crate::protocol`] does for the
//! agent's socket, and nothing it hands out is a type of that crate's.
//!
//! # The far side is a stranger
//!
//! Chrome is a process holding pages a stranger wrote, so a message from it is
//! read as bytes a stranger chose. Every message's nesting is refused before
//! it is parsed ([`MOST_NESTING`], scanned by [`crate::json_depth`]), every
//! text read from it is bounded ([`MOST_TEXT`]), and anything that is not what
//! the protocol says is refused as a [`Malformed`] kind, never a panic.

use crate::json_depth;
use core::fmt;
use serde_json::{Map, Value};

/// The deepest a message from Chrome may nest. A reply to every command this
/// browser sends nests at most ten deep: Chrome 155's accessibility tree for
/// `web-a-form` (item 620) is the deepest, its nodes' names holding the
/// sources they were computed from.
pub const MOST_NESTING: usize = 16;

/// The longest text read out of a message — a method's name, a version, a
/// reason Chrome gave, an identifier — in characters.
///
/// Not the words of a page: a name in an accessibility tree is a paragraph's
/// text, as long as its author wrote it, and is bounded by the message it
/// arrived in ([`crate::chrome_wire::MOST_MESSAGE`]) rather than by this.
pub const MOST_TEXT: usize = 4096;

/// The most nodes an accessibility tree is read with. Chrome 155 describes
/// `web-a-form` in 85; a page of a hundred thousand is refused rather than
/// read.
pub const MOST_NODES: usize = 100_000;

/// The most rectangles one node is read as occupying: a link wrapped across
/// this many lines is far past any page a person reads.
pub const MOST_QUADS: usize = 4096;

/// The most attributes one element is read with, from `DOM.describeNode`.
/// An element with more is refused rather than read.
pub const MOST_ATTRIBUTES: usize = 4096;

/// A mouse event, as `Input.dispatchMouseEvent` names its kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mouse {
    /// The pointer arrives over the point, no button held.
    Moved,
    /// The left button goes down.
    Pressed,
    /// The left button comes up: with the press before it at the same
    /// point, a click.
    Released,
}

/// A key a person presses, as `Input.dispatchKeyEvent` sends one: one with
/// a command Chrome's editing carries out, or a digit typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chord {
    /// ⌘A on macOS, Ctrl+A elsewhere: select everything in the field.
    SelectAll,
    /// Backspace: take away what is selected.
    DeleteBackward,
    /// A digit, typed as a person types it: the key going down makes the
    /// character, which is how a segment of Chrome's date field is filled
    /// (queue item 632).
    Digit(Digit),
}

/// One of the ten digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Digit(u8);

impl Digit {
    /// `digit`, if it is an ASCII digit.
    pub fn of(digit: char) -> Option<Self> {
        u8::try_from(digit)
            .ok()
            .filter(u8::is_ascii_digit)
            .map(Self)
    }
}

/// A command this browser sends Chrome.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// `Browser.getVersion`: which Chrome this is.
    GetVersion,
    /// `Browser.close`: close every window and end.
    Close,
    /// `Target.createTarget`: a new tab, showing `url`.
    CreateTarget {
        /// What it shows first.
        url: String,
    },
    /// `Target.attachToTarget`, flattened: a session over the pipe, through
    /// which the tab `target` is spoken to.
    AttachToTarget {
        /// The tab, by the id [`target`] read.
        target: String,
    },
    /// `Target.closeTarget`: close the tab `target`.
    CloseTarget {
        /// The tab, by the id [`target`] read.
        target: String,
    },
    /// `Fetch.enable` for every request: each one is paused and told of, and
    /// goes nowhere until it is answered.
    PauseEveryRequest,
    /// `Page.enable`: be told of a page's loading.
    TellOfLoading,
    /// `Network.enable`: be told of a page's network, for the one thing
    /// nothing else tells of — a WebSocket, before its handshake
    /// (`Network.webSocketCreated`, ADR 0060 § 3).
    TellOfSockets,
    /// `Network.enable` again, to a target already telling of its network:
    /// changes nothing, and is answered by its renderer after every request
    /// it told of before (measured against Chrome 155, queue item 647).
    HearTheNetworkOut,
    /// `Page.setLifecycleEventsEnabled`: be told of each stage of every
    /// load, by the loader it belongs to.
    TellOfStages,
    /// `Runtime.discardConsoleEntries`: forget what a page's script has
    /// said so far, so that [`Command::TellOfScript`] tells of none of it
    /// again (measured against Chrome 155: taken while the domain is off).
    ForgetWhatWasSaid,
    /// `Runtime.enable`: be told of what a page's script says to its
    /// console and of what it throws and nobody catches, from now — and,
    /// first, of everything it said before that was not forgotten.
    TellOfScript,
    /// `Runtime.disable`: no longer. Answered by the page's own renderer,
    /// after everything it told of before (queue item 639).
    StopTellingOfScript,
    /// `Log.clear`: forget what Chrome itself has logged of the page, so
    /// that [`Command::TellOfTheLog`] does not tell of it again. Measured
    /// against Chrome 155: `Runtime.discardConsoleEntries` does not.
    ForgetTheLog,
    /// `Log.enable`: be told of what Chrome itself logs of the page — not
    /// its script's words, Chrome's — from now and, first, of everything it
    /// logged before that was not forgotten (queue item 646).
    TellOfTheLog,
    /// `Log.disable`: no longer.
    StopTellingOfTheLog,
    /// `Emulation.setDeviceMetricsOverride`: lay pages out at this size, at
    /// the screen's own scale.
    LayOutAt {
        /// In CSS pixels.
        width: u32,
        /// In CSS pixels.
        height: u32,
    },
    /// `Page.navigate`: go to `url`.
    Navigate {
        /// Where.
        url: String,
    },
    /// `Fetch.fulfillRequest`: answer the paused request `request` with
    /// `body`, as HTML.
    Fulfil {
        /// The request, by the id Chrome paused it under.
        request: String,
        /// What it is answered with.
        body: Vec<u8>,
    },
    /// `Fetch.continueRequest`: let the paused request `request` go on, as
    /// Chrome made it, to Chrome's own network (ADR 0060 § 1).
    Continue {
        /// The request, by the id Chrome paused it under.
        request: String,
    },
    /// `Fetch.failRequest`: refuse the paused request `request`, as blocked
    /// by the browser rather than lost by the network.
    Refuse {
        /// The request, by the id Chrome paused it under.
        request: String,
    },
    /// `Accessibility.getFullAXTree`: the whole tree of the page.
    ReadTree,
    /// `DOM.getContentQuads`: where a node was drawn.
    WhereDrawn {
        /// The node, by Chrome's `backendNodeId`.
        node: u64,
    },
    /// `Page.getLayoutMetrics`: the window the page sits in.
    Window,
    /// `DOM.describeNode`: what a node is — its name and attributes.
    Describe {
        /// The node, by Chrome's `backendNodeId`.
        node: u64,
    },
    /// `DOM.describeNode`, for a node Chrome handed over as a value of the
    /// page's — an argument of what it logged — rather than by its id: read
    /// with [`backend_node`] for which node it is.
    DescribeValue {
        /// The value, by the `objectId` Chrome gave it.
        object: String,
    },
    /// `Accessibility.getPartialAXTree`, for one node and nothing around
    /// it: what it holds now.
    ReadNode {
        /// The node, by Chrome's `backendNodeId`.
        node: u64,
    },
    /// `DOM.scrollIntoViewIfNeeded`: bring a node into the window, as a
    /// person scrolls to what they are about to press.
    ScrollIntoView {
        /// The node, by Chrome's `backendNodeId`.
        node: u64,
    },
    /// `DOM.getNodeForLocation`: what a person pressing this point would
    /// press — never a way of choosing what to act on (ADR 0002), only of
    /// checking that a click lands on the thing an agent named.
    NodeAt {
        /// In CSS pixels of the window, as Chrome takes them: whole.
        x: i64,
        /// In CSS pixels of the window, as Chrome takes them: whole.
        y: i64,
    },
    /// `Input.dispatchMouseEvent`, with the left button.
    Mouse {
        /// Which.
        kind: Mouse,
        /// In CSS pixels of the window.
        x: f64,
        /// In CSS pixels of the window.
        y: f64,
        /// Whether the platform's toggle is held as it happens — ⌘ on
        /// macOS, Ctrl elsewhere — which in a list of several choices
        /// toggles the option clicked rather than choosing it alone (ADR
        /// 0064 § 6). Measured on macOS against Chrome 155; elsewhere it is
        /// Chrome's rule there, measured where it is first run.
        toggles: bool,
    },
    /// `DOM.focus`: give a node the keyboard, as a person clicking into a
    /// field does.
    Focus {
        /// The node, by Chrome's `backendNodeId`.
        node: u64,
    },
    /// `Input.dispatchKeyEvent`: a key going down, or coming up.
    Key {
        /// Which.
        chord: Chord,
        /// Down, or up.
        down: bool,
    },
    /// `Input.insertText`: text arriving in the focused field as typing's
    /// does, with `beforeinput` and `input` the page hears.
    InsertText {
        /// What arrives.
        text: String,
    },
    /// `Target.setAutoAttach`: be attached to every target that starts from
    /// now on, each held before it loads or runs anything until it is let go
    /// or closed — or, with `hold` false, no longer. Sent to Chrome itself,
    /// every tab that opens; sent through a target's session, every frame and
    /// worker it starts.
    HoldNewTabs {
        /// Whether to.
        hold: bool,
    },
    /// `Runtime.runIfWaitingForDebugger`: let a tab held by
    /// [`Command::HoldNewTabs`] go on.
    LetGo,
    /// `Target.getTargets`: every target Chrome holds — among them the page
    /// of a shown Chrome's own window ([`pages`]).
    Targets,
    /// `Browser.getWindowForTarget`: the window a tab is drawn in, and where
    /// it is on the screen ([`window_of`]).
    WindowOf {
        /// The tab, by the id [`target`] or [`pages`] read.
        target: String,
    },
    /// `Target.activateTarget`: bring the tab `target`, and its window, in
    /// front of every other window — out of Stage Manager's strip, where
    /// macOS keeps all but the window in front as a thumbnail.
    Activate {
        /// The tab, by the id [`target`] or [`pages`] read.
        target: String,
    },
    /// `Browser.setWindowBounds`: put a window of Chrome's at `bounds` on
    /// the screen — which the system may overrule, so where it went is read
    /// again with [`Command::WindowOf`].
    PlaceWindow {
        /// The window, by the id [`window_of`] read.
        window: i64,
        /// Where.
        bounds: Bounds,
    },
    /// `DOM.getDocument`: the page's document, read so that Chrome's DOM
    /// domain is on, which [`Command::ShowMarks`] needs first. Its answer
    /// is not read.
    ReadDocument,
    /// `Overlay.enable`: let Chrome draw a mark over its page
    /// ([`Command::Mark`], ADR 0066).
    ShowMarks,
    /// `Overlay.highlightNode`: mark a node over the page, as ADR 0066 § 3
    /// says — its border in `border`, its other regions clear, and Chrome's
    /// label naming it, with the accessibility part. Drawn by Chrome in its
    /// own compositor, against the node, and never told to the page.
    Mark {
        /// The node, by Chrome's `backendNodeId`.
        node: u64,
        /// The border's colour: red, green and blue, opaque.
        border: (u8, u8, u8),
    },
    /// `Overlay.hideHighlight`: take any mark away.
    Unmark,
}

impl Command {
    /// The method Chrome knows it by.
    pub fn method(&self) -> &'static str {
        match self {
            Self::GetVersion => "Browser.getVersion",
            Self::Close => "Browser.close",
            Self::CreateTarget { .. } => "Target.createTarget",
            Self::AttachToTarget { .. } => "Target.attachToTarget",
            Self::CloseTarget { .. } => "Target.closeTarget",
            Self::PauseEveryRequest => "Fetch.enable",
            Self::TellOfLoading => "Page.enable",
            Self::TellOfSockets | Self::HearTheNetworkOut => "Network.enable",
            Self::TellOfStages => "Page.setLifecycleEventsEnabled",
            Self::ForgetWhatWasSaid => "Runtime.discardConsoleEntries",
            Self::TellOfScript => "Runtime.enable",
            Self::StopTellingOfScript => "Runtime.disable",
            Self::ForgetTheLog => "Log.clear",
            Self::TellOfTheLog => "Log.enable",
            Self::StopTellingOfTheLog => "Log.disable",
            Self::LayOutAt { .. } => "Emulation.setDeviceMetricsOverride",
            Self::Navigate { .. } => "Page.navigate",
            Self::Fulfil { .. } => "Fetch.fulfillRequest",
            Self::Continue { .. } => "Fetch.continueRequest",
            Self::Refuse { .. } => "Fetch.failRequest",
            Self::ReadTree => "Accessibility.getFullAXTree",
            Self::WhereDrawn { .. } => "DOM.getContentQuads",
            Self::Window => "Page.getLayoutMetrics",
            Self::Describe { .. } | Self::DescribeValue { .. } => "DOM.describeNode",
            Self::ReadNode { .. } => "Accessibility.getPartialAXTree",
            Self::ScrollIntoView { .. } => "DOM.scrollIntoViewIfNeeded",
            Self::NodeAt { .. } => "DOM.getNodeForLocation",
            Self::Mouse { .. } => "Input.dispatchMouseEvent",
            Self::Focus { .. } => "DOM.focus",
            Self::Key { .. } => "Input.dispatchKeyEvent",
            Self::InsertText { .. } => "Input.insertText",
            Self::HoldNewTabs { .. } => "Target.setAutoAttach",
            Self::LetGo => "Runtime.runIfWaitingForDebugger",
            Self::Targets => "Target.getTargets",
            Self::WindowOf { .. } => "Browser.getWindowForTarget",
            Self::Activate { .. } => "Target.activateTarget",
            Self::PlaceWindow { .. } => "Browser.setWindowBounds",
            Self::ReadDocument => "DOM.getDocument",
            Self::ShowMarks => "Overlay.enable",
            Self::Mark { .. } => "Overlay.highlightNode",
            Self::Unmark => "Overlay.hideHighlight",
        }
    }

    /// Its `params`.
    fn params(&self) -> Map<String, Value> {
        let mut params = Map::new();
        let mut put = |name: &str, value: Value| {
            params.insert(name.to_owned(), value);
        };
        match self {
            Self::GetVersion
            | Self::Close
            | Self::TellOfLoading
            | Self::TellOfSockets
            | Self::HearTheNetworkOut
            | Self::ForgetWhatWasSaid
            | Self::TellOfScript
            | Self::StopTellingOfScript
            | Self::ForgetTheLog
            | Self::TellOfTheLog
            | Self::StopTellingOfTheLog
            | Self::ReadTree
            | Self::Window
            | Self::LetGo
            | Self::Targets
            | Self::ReadDocument
            | Self::ShowMarks
            | Self::Unmark => {}
            Self::CreateTarget { url } | Self::Navigate { url } => {
                put("url", Value::from(url.as_str()));
            }
            Self::AttachToTarget { target } => {
                put("targetId", Value::from(target.as_str()));
                put("flatten", Value::Bool(true));
            }
            Self::CloseTarget { target }
            | Self::WindowOf { target }
            | Self::Activate { target } => {
                put("targetId", Value::from(target.as_str()));
            }
            Self::PlaceWindow { window, bounds } => {
                put("windowId", Value::from(*window));
                put("bounds", bounds.written());
            }
            Self::TellOfStages => put("enabled", Value::Bool(true)),
            Self::PauseEveryRequest => put("patterns", every_request()),
            Self::LayOutAt { width, height } => {
                put("width", Value::from(*width));
                put("height", Value::from(*height));
                // Nought is Chrome's word for "the screen's own".
                put("deviceScaleFactor", Value::from(0));
                put("mobile", Value::Bool(false));
            }
            Self::Fulfil { request, body } => {
                for (name, value) in fulfilled(request, body) {
                    put(name, value);
                }
            }
            Self::Continue { request } => put("requestId", Value::from(request.as_str())),
            Self::Refuse { request } => {
                put("requestId", Value::from(request.as_str()));
                put("errorReason", Value::from("BlockedByClient"));
            }
            Self::WhereDrawn { node }
            | Self::Describe { node }
            | Self::ScrollIntoView { node }
            | Self::Focus { node } => put("backendNodeId", Value::from(*node)),
            Self::DescribeValue { object } => put("objectId", Value::from(object.as_str())),
            Self::Mark { node, border } => {
                put("backendNodeId", Value::from(*node));
                put("highlightConfig", marked(*border));
            }
            Self::ReadNode { node } => {
                put("backendNodeId", Value::from(*node));
                put("fetchRelatives", Value::Bool(false));
            }
            Self::NodeAt { x, y } => {
                put("x", Value::from(*x));
                put("y", Value::from(*y));
            }
            Self::Mouse {
                kind,
                x,
                y,
                toggles,
            } => {
                for (name, value) in mouse(*kind, (*x, *y), *toggles) {
                    put(name, value);
                }
            }
            Self::Key { chord, down } => {
                for (name, value) in key(*chord, *down) {
                    put(name, value);
                }
            }
            Self::InsertText { text } => put("text", Value::from(text.as_str())),
            Self::HoldNewTabs { hold } => {
                put("autoAttach", Value::Bool(*hold));
                put("waitForDebuggerOnStart", Value::Bool(*hold));
                put("flatten", Value::Bool(true));
            }
        }
        params
    }
}

/// The `patterns` of `Fetch.enable` that pause every request.
fn every_request() -> Value {
    let mut every = Map::new();
    every.insert("urlPattern".to_owned(), Value::from("*"));
    Value::Array(vec![Value::Object(every)])
}

/// The `params` of `Fetch.fulfillRequest` answering `request` with `body`,
/// as HTML.
fn fulfilled(request: &str, body: &[u8]) -> Vec<(&'static str, Value)> {
    let mut header = Map::new();
    header.insert("name".to_owned(), Value::from("Content-Type"));
    header.insert("value".to_owned(), Value::from("text/html; charset=utf-8"));
    vec![
        ("requestId", Value::from(request)),
        ("responseCode", Value::from(200)),
        ("responseHeaders", Value::Array(vec![Value::Object(header)])),
        ("body", Value::from(base64(body))),
    ]
}

/// How Chrome is asked to mark a node: the border in `border`, every other
/// region left out and so clear, and its label shown with the accessibility
/// part — the node's name and role (ADR 0066 § 1).
fn marked((red, green, blue): (u8, u8, u8)) -> Value {
    let mut colour = Map::new();
    for (name, value) in [("r", red), ("g", green), ("b", blue)] {
        colour.insert(name.to_owned(), Value::from(value));
    }
    colour.insert("a".to_owned(), Value::from(1));
    let mut config = Map::new();
    config.insert("borderColor".to_owned(), Value::Object(colour));
    config.insert("showInfo".to_owned(), Value::Bool(true));
    config.insert("showAccessibilityInfo".to_owned(), Value::Bool(true));
    Value::Object(config)
}

/// The `params` of `Input.dispatchMouseEvent` for `kind` at `x`, `y`, with
/// the platform's toggle held when `toggles`.
fn mouse(kind: Mouse, (x, y): (f64, f64), toggles: bool) -> Vec<(&'static str, Value)> {
    let (kind, button) = match kind {
        Mouse::Moved => ("mouseMoved", "none"),
        Mouse::Pressed => ("mousePressed", "left"),
        Mouse::Released => ("mouseReleased", "left"),
    };
    vec![
        ("type", Value::from(kind)),
        ("x", Value::from(x)),
        ("y", Value::from(y)),
        ("button", Value::from(button)),
        ("clickCount", Value::from(1)),
        ("modifiers", Value::from(if toggles { TOGGLE } else { 0 })),
    ]
}

/// The platform's command modifier, as `Input` takes modifiers: ⌘ on macOS
/// is Chrome's 4, Ctrl elsewhere 2. It is the one a shortcut holds, and the
/// one a click in a list of several holds to toggle an option.
const TOGGLE: i64 = if cfg!(target_os = "macos") { 4 } else { 2 };

/// The `params` of `Input.dispatchKeyEvent` for `chord`, pressed when
/// `down` and let go otherwise.
fn key(chord: Chord, down: bool) -> Vec<(&'static str, Value)> {
    let (key, code, keycode, command, modifiers) = match chord {
        Chord::SelectAll => ("a", "KeyA", 65, "selectAll", TOGGLE),
        Chord::DeleteBackward => ("Backspace", "Backspace", 8, "deleteBackward", 0),
        Chord::Digit(digit) => return typed(digit, down),
    };
    let mut fields = vec![
        (
            "type",
            Value::from(if down { "rawKeyDown" } else { "keyUp" }),
        ),
        ("key", Value::from(key)),
        ("code", Value::from(code)),
        ("windowsVirtualKeyCode", Value::from(keycode)),
        ("modifiers", Value::from(modifiers)),
    ];
    // The command is what Chrome's editing carries out; a key alone does
    // nothing on macOS, where the shortcut is the menu's rather than the
    // page's.
    if down {
        fields.push(("commands", Value::Array(vec![Value::from(command)])));
    }
    fields
}

/// The `params` of `Input.dispatchKeyEvent` for `digit`, typed: going down
/// with its text, which is what makes the character, and coming up without.
fn typed(Digit(digit): Digit, down: bool) -> Vec<(&'static str, Value)> {
    let typed = char::from(digit).to_string();
    let mut fields = vec![
        ("type", Value::from(if down { "keyDown" } else { "keyUp" })),
        ("key", Value::from(typed.as_str())),
        ("code", Value::from(format!("Digit{typed}"))),
        ("windowsVirtualKeyCode", Value::from(digit)),
        ("modifiers", Value::from(0)),
    ];
    if down {
        fields.push(("text", Value::from(typed)));
    }
    fields
}

/// `command`, written as the message that asks for it under `id`, through
/// `session` when it is for a tab rather than for Chrome itself.
pub fn written(id: u64, session: Option<&str>, command: &Command) -> Vec<u8> {
    let mut fields = Map::new();
    fields.insert("id".to_owned(), Value::from(id));
    if let Some(session) = session {
        fields.insert("sessionId".to_owned(), Value::from(session));
    }
    fields.insert("method".to_owned(), Value::from(command.method()));
    fields.insert("params".to_owned(), Value::Object(command.params()));
    Value::Object(fields).to_string().into_bytes()
}

/// `bytes` in base64, as `Fetch.fulfillRequest` takes a body.
fn base64(bytes: &[u8]) -> String {
    const LETTERS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3).saturating_mul(4));
    for three in bytes.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|at| three.get(at).copied().unwrap_or(0));
        let all = u32::from(a) << 16 | u32::from(b) << 8 | u32::from(c);
        for (at, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
            let letter = LETTERS
                .get(usize::try_from(all >> shift & 63).unwrap_or(0))
                .copied()
                .unwrap_or(b'=');
            out.push(if at <= three.len() {
                char::from(letter)
            } else {
                '='
            });
        }
    }
    out
}

/// A message from Chrome, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// The answer to the command sent under `id`.
    Reply {
        /// The command's own `id`.
        id: u64,
        /// The session it was sent through, if it was sent to a tab.
        session: Option<String>,
        /// Its `result`, written again as JSON for whoever asked to read
        /// ([`version`]), or the reason Chrome gave for not carrying it out.
        result: Result<Vec<u8>, Declined>,
    },
    /// Something Chrome tells of without being asked.
    Event {
        /// What it is, by the protocol's name for it.
        method: String,
        /// The session it is told through, if it is a tab's.
        session: Option<String>,
        /// Its `params`, written again as JSON for whoever reads it
        /// ([`paused`]); an empty object when it had none.
        params: Vec<u8>,
    },
    /// A message too long to read, skipped
    /// ([`crate::chrome_wire::Frame::Skipped`]): what its first bytes say
    /// it was, read by [`skipped`]. Its session is not known — Chrome writes
    /// that last.
    Skipped {
        /// The answer's `id`, if it was an answer.
        id: Option<u64>,
        /// The event's `method`, if it was an event.
        method: Option<String>,
    },
}

/// Chrome's reason for not carrying a command out: its `error`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declined {
    /// The protocol's code for it.
    pub code: i64,
    /// Chrome's own words, for a person reading.
    pub message: String,
}

impl fmt::Display for Declined {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Chrome declined it ({}): {}", self.code, self.message)
    }
}

/// Why a message from Chrome was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Malformed {
    /// It nests deeper than `most`.
    TooDeep {
        /// [`MOST_NESTING`].
        most: usize,
    },
    /// It is not JSON.
    NotJson,
    /// It is JSON, but not an object.
    NotAnObject,
    /// It is neither an answer, having no `id`, nor an event, having no
    /// `method`.
    Neither,
    /// A field is missing or not what the protocol says it is.
    BadField {
        /// The field, by its name in the protocol.
        field: &'static str,
    },
    /// A text is longer than `most` characters.
    TooLong {
        /// The field, by its name in the protocol.
        field: &'static str,
        /// [`MOST_TEXT`].
        most: usize,
    },
}

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooDeep { most } => {
                write!(f, "a message from Chrome nests deeper than {most}")
            }
            Self::NotJson => f.write_str("a message from Chrome is not JSON"),
            Self::NotAnObject => f.write_str("a message from Chrome is not an object"),
            Self::Neither => f.write_str("a message from Chrome is neither an answer nor an event"),
            Self::BadField { field } => {
                write!(f, "a message from Chrome has no proper `{field}`")
            }
            Self::TooLong { field, most } => {
                write!(
                    f,
                    "a message from Chrome has a `{field}` over {most} characters"
                )
            }
        }
    }
}

/// Read one message — without its NUL — from Chrome.
///
/// # Errors
///
/// [`Malformed`], for anything that is not a message the protocol describes.
pub fn read(message: &[u8]) -> Result<Message, Malformed> {
    let fields = object(message)?;
    let session = match fields.get("sessionId") {
        None => None,
        session => Some(text(session, "sessionId")?),
    };
    match (fields.get("id"), fields.get("method")) {
        (Some(id), _) => {
            let id = id.as_u64().ok_or(Malformed::BadField { field: "id" })?;
            let result = match (fields.get("result"), fields.get("error")) {
                (_, Some(error)) => Err(declined(error)?),
                (Some(result), None) => Ok(result.to_string().into_bytes()),
                (None, None) => return Err(Malformed::BadField { field: "result" }),
            };
            Ok(Message::Reply {
                id,
                session,
                result,
            })
        }
        (None, Some(method)) => Ok(Message::Event {
            method: text(Some(method), "method")?,
            session,
            params: match fields.get("params") {
                None => b"{}".to_vec(),
                Some(params @ Value::Object(_)) => params.to_string().into_bytes(),
                Some(_) => return Err(Malformed::BadField { field: "params" }),
            },
        }),
        (None, None) => Err(Malformed::Neither),
    }
}

/// Which Chrome answered, from `Browser.getVersion`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    /// Its product and version, as `Chrome/155.0.8059.86`.
    pub product: String,
    /// The version of the protocol it speaks.
    pub protocol: String,
    /// The revision it was built from.
    pub revision: String,
    /// The user agent its pages are sent with.
    pub user_agent: String,
    /// Its JavaScript engine's version.
    pub javascript: String,
}

/// A reply's `result` to `Browser.getVersion`, read.
///
/// # Errors
///
/// [`Malformed`], for a result that is not the one the protocol describes.
pub fn version(result: &[u8]) -> Result<Version, Malformed> {
    let fields = object(result)?;
    Ok(Version {
        product: text(fields.get("product"), "product")?,
        protocol: text(fields.get("protocolVersion"), "protocolVersion")?,
        revision: text(fields.get("revision"), "revision")?,
        user_agent: text(fields.get("userAgent"), "userAgent")?,
        javascript: text(fields.get("jsVersion"), "jsVersion")?,
    })
}

/// The tab a reply's `result` to `Target.createTarget` names.
///
/// # Errors
///
/// [`Malformed`], for a result with no `targetId`.
pub fn target(result: &[u8]) -> Result<String, Malformed> {
    text(object(result)?.get("targetId"), "targetId")
}

/// The session a reply's `result` to `Target.attachToTarget` names.
///
/// # Errors
///
/// [`Malformed`], for a result with no `sessionId`.
pub fn session(result: &[u8]) -> Result<String, Malformed> {
    text(object(result)?.get("sessionId"), "sessionId")
}

/// Whether a `Page.navigate` went: the loader the new page is loaded by
/// when it did, or the reason Chrome gave when it did not.
///
/// # Errors
///
/// [`Malformed`], for a result with neither a `loaderId` nor an
/// `errorText`, or one that is not text.
pub fn navigated(result: &[u8]) -> Result<Result<String, String>, Malformed> {
    let fields = object(result)?;
    match fields.get("errorText") {
        None => text(fields.get("loaderId"), "loaderId").map(Ok),
        reason => text(reason, "errorText").map(Err),
    }
}

/// A `Page.lifecycleEvent`'s `params`: the loader it belongs to, and the
/// stage — `load` among them — it tells of.
///
/// # Errors
///
/// [`Malformed`], for params without a `loaderId` and a `name`.
pub fn stage(params: &[u8]) -> Result<(String, String), Malformed> {
    let fields = object(params)?;
    Ok((
        text(fields.get("loaderId"), "loaderId")?,
        text(fields.get("name"), "name")?,
    ))
}

/// A request Chrome has paused, from a `Fetch.requestPaused` event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paused {
    /// Its id, which answering it names.
    pub request: String,
    /// What it asks for, as Chrome spells the address.
    pub url: String,
    /// Its method, as Chrome spells it.
    pub method: String,
    /// What kind of thing it is, in Chrome's word: `Document`,
    /// `Stylesheet`, `XHR`, `Other`, …
    pub kind: String,
    /// The frame it is for, by Chrome's id: a tab's own frame has the tab's
    /// id.
    pub frame: String,
    /// Whether a redirect led here — a hop after the first of a navigation
    /// or a fetch (`redirectedRequestId`).
    pub redirected: bool,
    /// The id the page's renderer tells of it by (`networkId`), when Chrome
    /// gave one: a navigation's is the loader of the document it commits,
    /// and a beacon has none (measured against Chrome 155, item 647).
    pub network: Option<String>,
}

/// A `Fetch.requestPaused` event's `params`, read.
///
/// # Errors
///
/// [`Malformed`], for params with no `requestId`, no `request.url`, no
/// `request.method`, no `resourceType` or no `frameId`, each text.
pub fn paused(params: &[u8]) -> Result<Paused, Malformed> {
    let fields = object(params)?;
    let Some(Value::Object(request)) = fields.get("request") else {
        return Err(Malformed::BadField { field: "request" });
    };
    Ok(Paused {
        request: text(fields.get("requestId"), "requestId")?,
        url: words(request.get("url"), "url")?,
        method: text(request.get("method"), "method")?,
        kind: text(fields.get("resourceType"), "resourceType")?,
        frame: text(fields.get("frameId"), "frameId")?,
        redirected: match fields.get("redirectedRequestId") {
            None => false,
            redirected => {
                text(redirected, "redirectedRequestId")?;
                true
            }
        },
        network: match fields.get("networkId") {
            None => None,
            network => Some(text(network, "networkId")?),
        },
    })
}

/// A request a page's renderer told of, from a `Network.requestWillBeSent`
/// event's `params`: its id — a paused request's `networkId` — and the
/// loader of the document that made it, which a dedicated worker's request
/// has as nothing (`""`, measured against Chrome 155).
///
/// # Errors
///
/// [`Malformed`], for params with no `requestId` or no `loaderId`, each
/// text.
pub fn told(params: &[u8]) -> Result<(String, String), Malformed> {
    let fields = object(params)?;
    Ok((
        text(fields.get("requestId"), "requestId")?,
        text(fields.get("loaderId"), "loaderId")?,
    ))
}

/// A frame that has gone to a new document, from a `Page.frameNavigated`
/// event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed {
    /// The frame's id.
    pub frame: String,
    /// Whether it is inside another frame.
    pub inside: bool,
    /// The document's address, as Chrome spells it, with its fragment.
    pub url: String,
}

/// A frame that has gone to a new document, from a `Page.frameNavigated`
/// event: its id, whether it is inside another frame, and its address.
///
/// # Errors
///
/// [`Malformed`], for params with no `frame` holding an `id` and a `url`, or
/// a `parentId` or `urlFragment` that is not text.
pub fn committed(params: &[u8]) -> Result<Committed, Malformed> {
    let Some(Value::Object(frame)) = object(params)?.get("frame").cloned() else {
        return Err(Malformed::BadField { field: "frame" });
    };
    let inside = match frame.get("parentId") {
        None => false,
        parent => {
            text(parent, "parentId")?;
            true
        }
    };
    let id = text(frame.get("id"), "id")?;
    let mut url = words(frame.get("url"), "url")?;
    // Chrome keeps a fragment apart from the address it belongs to.
    if let Some(fragment) = frame.get("urlFragment") {
        url.push_str(&words(Some(fragment), "urlFragment")?);
    }
    Ok(Committed {
        frame: id,
        inside,
        url,
    })
}

/// A frame that moved to another address of the document it holds — a link
/// to a part of it, `history.pushState` — from a
/// `Page.navigatedWithinDocument` event. No document was committed and none
/// was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Within {
    /// The frame's id.
    pub frame: String,
    /// The address it moved to, as Chrome spells it, with its fragment.
    pub url: String,
    /// How, in Chrome's words: `fragment`, `historyApi` or `other`.
    pub how: String,
}

/// A `Page.navigatedWithinDocument` event's `params`, read. Measured against
/// Chrome 155 on 2026-10-24: a link to `#part` is told of as `fragment` with
/// the whole address, an empty fragment's `#` kept, and `pushState` as
/// `historyApi`.
///
/// # Errors
///
/// [`Malformed`], for params without a `frameId`, a `url` and a
/// `navigationType`, each text.
pub fn within(params: &[u8]) -> Result<Within, Malformed> {
    let fields = object(params)?;
    Ok(Within {
        frame: text(fields.get("frameId"), "frameId")?,
        url: words(fields.get("url"), "url")?,
        how: text(fields.get("navigationType"), "navigationType")?,
    })
}

/// A frame made inside another, from a `Page.frameAttached` event: the
/// frame's id, and the id of the frame holding it. Measured against Chrome
/// 155 on 2026-10-25: told of to the session of the frame holding it, before
/// the frame asks for its document.
///
/// # Errors
///
/// [`Malformed`], for params without a `frameId` and a `parentFrameId`,
/// each text.
pub fn frame_made(params: &[u8]) -> Result<(String, String), Malformed> {
    let fields = object(params)?;
    Ok((
        text(fields.get("frameId"), "frameId")?,
        text(fields.get("parentFrameId"), "parentFrameId")?,
    ))
}

/// A frame gone from where it was, from a `Page.frameDetached` event: the
/// frame's id, and why in Chrome's word — `remove` for a frame taken out of
/// its page, `swap` for one that went on in a process of its own (measured
/// against Chrome 155 on 2026-10-25: an other-site frame is told of as
/// `swap` after its document committed in its own target).
///
/// # Errors
///
/// [`Malformed`], for params without a `frameId` and a `reason`, each text.
pub fn frame_gone(params: &[u8]) -> Result<(String, String), Malformed> {
    let fields = object(params)?;
    Ok((
        text(fields.get("frameId"), "frameId")?,
        text(fields.get("reason"), "reason")?,
    ))
}

/// The address of a WebSocket a page opened, from a
/// `Network.webSocketCreated` event, as Chrome spells it.
///
/// # Errors
///
/// [`Malformed`], for params with no `url` that is text.
pub fn socket(params: &[u8]) -> Result<String, Malformed> {
    words(object(params)?.get("url"), "url")
}

/// Where a node was drawn, from `DOM.getContentQuads`: each quad's four
/// corners as `x, y` pairs, in CSS pixels of the window as the page sits.
///
/// # Errors
///
/// [`Malformed`], for a result whose `quads` are not lists of eight finite
/// numbers, or are more than [`MOST_QUADS`].
pub fn quads(result: &[u8]) -> Result<Vec<[f64; 8]>, Malformed> {
    let bad = Malformed::BadField { field: "quads" };
    let Some(Value::Array(quads)) = object(result)?.get("quads").cloned() else {
        return Err(bad);
    };
    if quads.len() > MOST_QUADS {
        return Err(bad);
    }
    quads
        .iter()
        .map(|quad| {
            let Value::Array(numbers) = quad else {
                return Err(bad);
            };
            let mut corners = [0.0; 8];
            if numbers.len() != corners.len() {
                return Err(bad);
            }
            for (corner, number) in corners.iter_mut().zip(numbers) {
                *corner = number.as_f64().filter(|n| n.is_finite()).ok_or(bad)?;
            }
            Ok(corners)
        })
        .collect()
}

/// The window a page sits in, from `Page.getLayoutMetrics`'s
/// `cssLayoutViewport`: its width and height in CSS pixels.
///
/// # Errors
///
/// [`Malformed`], for a result without a `cssLayoutViewport` holding a
/// finite, non-negative `clientWidth` and `clientHeight`.
pub fn window(result: &[u8]) -> Result<(f64, f64), Malformed> {
    let bad = Malformed::BadField {
        field: "cssLayoutViewport",
    };
    let Some(Value::Object(viewport)) = object(result)?.get("cssLayoutViewport").cloned() else {
        return Err(bad);
    };
    let length = |name: &str| {
        viewport
            .get(name)
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite() && *n >= 0.0)
            .ok_or(bad)
    };
    Ok((length("clientWidth")?, length("clientHeight")?))
}

/// An element, as `DOM.describeNode` describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Described {
    /// Its name as the DOM gives it: upper case for an HTML element in an
    /// HTML document, as written for any other.
    pub node_name: String,
    /// Its local name.
    pub local_name: String,
    /// Its attributes, name and value, in the order Chrome lists them.
    pub attributes: Vec<(String, String)>,
}

/// A reply's `result` to `DOM.describeNode`, read.
///
/// # Errors
///
/// [`Malformed`], for a result with no `node`, a name that is not text, or
/// `attributes` that are not pairs of texts, or more than
/// [`MOST_ATTRIBUTES`] of them.
pub fn described(result: &[u8]) -> Result<Described, Malformed> {
    let Some(Value::Object(node)) = object(result)?.get("node").cloned() else {
        return Err(Malformed::BadField { field: "node" });
    };
    let bad = Malformed::BadField {
        field: "attributes",
    };
    let listed = match node.get("attributes") {
        None => Vec::new(),
        Some(Value::Array(listed)) if listed.len() <= MOST_ATTRIBUTES * 2 => listed.clone(),
        Some(_) => return Err(bad),
    };
    if listed.len() % 2 != 0 {
        return Err(bad);
    }
    let attributes = listed
        .chunks_exact(2)
        .map(|pair| {
            Ok((
                text(pair.first(), "attributes")?,
                words(pair.get(1), "attributes")?,
            ))
        })
        .collect::<Result<_, Malformed>>()?;
    Ok(Described {
        node_name: text(node.get("nodeName"), "nodeName")?,
        local_name: text(node.get("localName"), "localName")?,
        attributes,
    })
}

/// The node a reply's `result` to `DOM.describeNode` describes, by its
/// `backendNodeId`.
///
/// # Errors
///
/// [`Malformed`], for a result with no `node` holding a `backendNodeId`
/// that is a whole number.
pub fn backend_node(result: &[u8]) -> Result<u64, Malformed> {
    let Some(Value::Object(node)) = object(result)?.get("node").cloned() else {
        return Err(Malformed::BadField { field: "node" });
    };
    node.get("backendNodeId")
        .and_then(Value::as_u64)
        .ok_or(Malformed::BadField {
            field: "backendNodeId",
        })
}

/// The node a reply's `result` to `DOM.getNodeForLocation` names, by its
/// `backendNodeId`.
///
/// # Errors
///
/// [`Malformed`], for a result with no `backendNodeId` that is a whole
/// number.
pub fn node_at(result: &[u8]) -> Result<u64, Malformed> {
    object(result)?
        .get("backendNodeId")
        .and_then(Value::as_u64)
        .ok_or(Malformed::BadField {
            field: "backendNodeId",
        })
}

/// A navigation a page asked for, from `Page.frameRequestedNavigation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requested {
    /// The frame that is to navigate.
    pub frame: String,
    /// Why, in Chrome's words: `anchorClick`, `formSubmissionPost`, …
    pub reason: String,
    /// Where to, resolved, as Chrome spells it.
    pub url: String,
    /// Where it is to be shown: `currentTab`, `newTab`, `newWindow`, or
    /// `download`.
    pub disposition: String,
}

/// A `Page.frameRequestedNavigation` event's `params`, read.
///
/// # Errors
///
/// [`Malformed`], for params without a `frameId`, a `reason`, a `url` and a
/// `disposition`, each text.
pub fn requested(params: &[u8]) -> Result<Requested, Malformed> {
    let fields = object(params)?;
    Ok(Requested {
        frame: text(fields.get("frameId"), "frameId")?,
        reason: text(fields.get("reason"), "reason")?,
        url: words(fields.get("url"), "url")?,
        disposition: text(fields.get("disposition"), "disposition")?,
    })
}

/// Something Chrome itself logged of a page, from `Log.entryAdded`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logged {
    /// What part of Chrome logged it: `rendering`, `network`,
    /// `violation`, …
    pub source: String,
    /// How bad: `verbose`, `info`, `warning` or `error`.
    pub level: String,
    /// Its words, which may hold the page's own — a control's name — and so
    /// are as long as the page made them.
    pub text: String,
    /// Each argument that is a node, by the `objectId` Chrome gave it, in
    /// order, at most [`MOST_ARGS`]; any other argument is passed over.
    pub nodes: Vec<String>,
}

/// A `Log.entryAdded` event's `params`, read. Measured against Chrome 155
/// on 2026-10-25: an `entry` with a `source`, a `level` and a `text`, and —
/// for a form control it could not give the keyboard — `args` holding the
/// control as a value of `subtype` `node` with an `objectId`.
///
/// # Errors
///
/// [`Malformed`], for params without an `entry` holding a `source`, a
/// `level` and a `text`, each text, or with `args` that are not a list.
pub fn logged(params: &[u8]) -> Result<Logged, Malformed> {
    let Some(Value::Object(entry)) = object(params)?.get("entry").cloned() else {
        return Err(Malformed::BadField { field: "entry" });
    };
    let given = match entry.get("args") {
        None => Vec::new(),
        Some(Value::Array(given)) => given.clone(),
        Some(_) => return Err(Malformed::BadField { field: "args" }),
    };
    let nodes = given
        .iter()
        .take(MOST_ARGS)
        .filter(|arg| arg.get("subtype").and_then(Value::as_str) == Some("node"))
        .map(|arg| text(arg.get("objectId"), "objectId"))
        .collect::<Result<_, _>>()?;
    Ok(Logged {
        source: text(entry.get("source"), "source")?,
        level: text(entry.get("level"), "level")?,
        text: words(entry.get("text"), "text")?,
        nodes,
    })
}

/// A target Chrome attached this browser to, from `Target.attachedToTarget`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attached {
    /// The session it is spoken to through.
    pub session: String,
    /// The target, by its id.
    pub target: String,
    /// What it is, in Chrome's word: `page`, `iframe`, `worker`,
    /// `service_worker`, …
    pub kind: String,
    /// The tab that opened it, if one did.
    pub opener: Option<String>,
}

/// A `Target.attachedToTarget` event's `params`, read.
///
/// # Errors
///
/// [`Malformed`], for params with no `sessionId`, no `targetInfo` holding a
/// `targetId` and a `type`, or an `openerId` that is not text.
pub fn attached(params: &[u8]) -> Result<Attached, Malformed> {
    let fields = object(params)?;
    let Some(Value::Object(info)) = fields.get("targetInfo") else {
        return Err(Malformed::BadField {
            field: "targetInfo",
        });
    };
    Ok(Attached {
        session: text(fields.get("sessionId"), "sessionId")?,
        target: text(info.get("targetId"), "targetId")?,
        kind: text(info.get("type"), "type")?,
        opener: match info.get("openerId") {
            None => None,
            opener => Some(text(opener, "openerId")?),
        },
    })
}

/// The session a `Target.detachedFromTarget` event's `params` say has
/// ended.
///
/// # Errors
///
/// [`Malformed`], for params with no `sessionId` that is text.
pub fn detached(params: &[u8]) -> Result<String, Malformed> {
    text(object(params)?.get("sessionId"), "sessionId")
}

/// What the first bytes of a message too long to read say it was. Chrome
/// 155 writes an answer as `{"id":…,` and an event as `{"method":"…",`,
/// first (measured, queue item 639); anything else says nothing, and is
/// neither.
pub fn skipped(head: &[u8]) -> Message {
    let id = head.strip_prefix(br#"{"id":"#).and_then(|rest| {
        let digits = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
        let ended = matches!(rest.get(digits), Some(b',' | b'}'));
        let digits = rest
            .get(..digits)
            .filter(|digits| ended && !digits.is_empty())?;
        core::str::from_utf8(digits).ok()?.parse().ok()
    });
    let method = head.strip_prefix(br#"{"method":""#).and_then(|rest| {
        let name = rest
            .iter()
            .take_while(|byte| byte.is_ascii_alphanumeric() || **byte == b'.')
            .count();
        let ended = rest.get(name) == Some(&b'"');
        let name = rest.get(..name).filter(|name| ended && !name.is_empty())?;
        core::str::from_utf8(name).ok().map(str::to_owned)
    });
    Message::Skipped { id, method }
}

/// The most of a console call's values read; the rest are counted.
pub const MOST_ARGS: usize = 64;

/// The most places of a call stack read; the rest are counted, as the
/// engine's own trace is bounded.
pub const MOST_PLACES: usize = 32;

/// The most characters of any one text a page chose — a string it wrote or
/// threw, an error's words, a script's address — that are read; the rest
/// are counted. The engine repeats as many of a string's code units
/// (`alo-renderer`'s `LONGEST_SAID`); this counts characters.
pub const LONGEST_SHOWN: usize = 1024;

/// A page's text, cut at [`LONGEST_SHOWN`] characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Words {
    /// What was kept.
    pub kept: String,
    /// How many characters more there were.
    pub more: usize,
}

impl Words {
    /// `text`, cut.
    fn of(text: &str) -> Self {
        let kept: String = text.chars().take(LONGEST_SHOWN).collect();
        let more = text.chars().count().saturating_sub(LONGEST_SHOWN);
        Self { kept, more }
    }
}

/// A value a page's script said or threw, as Chrome describes it — a
/// `RemoteObject` — read without asking Chrome anything more about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shown {
    /// `undefined`.
    Undefined,
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number, as Chrome writes it: `4`, `NaN`, `-0`.
    Number(Words),
    /// A `BigInt`, as Chrome writes it: `5n`.
    BigInt(Words),
    /// A string.
    Text(Words),
    /// A symbol, as Chrome describes it: `Symbol(x)`.
    Symbol(Words),
    /// An error object — a `DOMException` among them — by the first line of
    /// Chrome's description, its name and message.
    Error(Words),
    /// Any other object, a function among them, by Chrome's description of
    /// it: `Object`, `Array(2)`, `body`, a function's source.
    Object(Words),
}

/// Where a script was when it said something: its address, and a line and a
/// column counted from one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// The script's address, as Chrome names it: the page's own for a
    /// script in its markup; empty for one with none.
    pub url: Words,
    /// From one.
    pub line: u64,
    /// From one.
    pub column: u64,
}

/// Where: innermost first, at most [`MOST_PLACES`], and how many more.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Places {
    /// The places kept.
    pub places: Vec<Place>,
    /// How many more there were.
    pub left_out: usize,
}

/// What a page's script said, as Chrome told of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// A call to its console.
    Console {
        /// Which, in Chrome's word: `log`, `error`, `warning`, …
        kind: String,
        /// What it was given, at most [`MOST_ARGS`].
        args: Vec<Shown>,
        /// How many more it was given.
        more_args: usize,
        /// Where it was called.
        at: Places,
    },
    /// A throw nothing caught.
    Thrown {
        /// Chrome's words for it: `Uncaught`, `Uncaught (in promise)`.
        text: String,
        /// What was thrown, when Chrome says.
        what: Option<Shown>,
        /// Where it was thrown.
        at: Places,
    },
}

/// A `Runtime.consoleAPICalled` event's `params`, read. Measured against
/// Chrome 155 on 2026-10-25: a `type`, every value given as an `args`
/// entry, and the call's `stackTrace`.
///
/// # Errors
///
/// [`Malformed`], for params without a `type` and an array of `args`, a
/// value that is not one the protocol describes, or a `stackTrace` that is
/// not one.
pub fn console_called(params: &[u8]) -> Result<Said, Malformed> {
    let fields = object(params)?;
    let Some(Value::Array(given)) = fields.get("args") else {
        return Err(Malformed::BadField { field: "args" });
    };
    Ok(Said::Console {
        kind: text(fields.get("type"), "type")?,
        args: given
            .iter()
            .take(MOST_ARGS)
            .map(shown)
            .collect::<Result<_, _>>()?,
        more_args: given.len().saturating_sub(MOST_ARGS),
        at: match fields.get("stackTrace") {
            None => Places::default(),
            Some(trace) => places(trace)?,
        },
    })
}

/// A `Runtime.exceptionThrown` event's `params`, read. Measured against
/// Chrome 155 on 2026-10-25: `exceptionDetails` with Chrome's `text`, the
/// thrown value as `exception`, and where — a `stackTrace`, or for a
/// script that did not parse, only a `url`, a `lineNumber` and a
/// `columnNumber`, each counted from nought.
///
/// # Errors
///
/// [`Malformed`], for params without `exceptionDetails` holding a `text`,
/// or with an `exception`, a `stackTrace` or a place that is not one.
pub fn thrown(params: &[u8]) -> Result<Said, Malformed> {
    let fields = object(params)?;
    let Some(Value::Object(details)) = fields.get("exceptionDetails") else {
        return Err(Malformed::BadField {
            field: "exceptionDetails",
        });
    };
    let at = match (details.get("stackTrace"), details.get("url")) {
        (Some(trace), _) => places(trace)?,
        (None, Some(url)) => Places {
            places: vec![place(details, url)?],
            left_out: 0,
        },
        (None, None) => Places::default(),
    };
    Ok(Said::Thrown {
        text: text(details.get("text"), "text")?,
        what: details.get("exception").map(shown).transpose()?,
        at,
    })
}

/// A `StackTrace`'s `callFrames`, innermost first.
fn places(trace: &Value) -> Result<Places, Malformed> {
    let Some(Value::Array(frames)) = trace.get("callFrames") else {
        return Err(Malformed::BadField {
            field: "callFrames",
        });
    };
    let places = frames
        .iter()
        .take(MOST_PLACES)
        .map(|frame| match (frame, frame.get("url")) {
            (Value::Object(frame), Some(url)) => place(frame, url),
            _ => Err(Malformed::BadField { field: "url" }),
        })
        .collect::<Result<_, _>>()?;
    Ok(Places {
        places,
        left_out: frames.len().saturating_sub(MOST_PLACES),
    })
}

/// A place: `url`, and the `lineNumber` and `columnNumber` beside it in
/// `fields`, counted from nought.
fn place(fields: &Map<String, Value>, url: &Value) -> Result<Place, Malformed> {
    let from_one = |field| {
        fields
            .get(field)
            .and_then(Value::as_u64)
            .map(|at| at.saturating_add(1))
            .ok_or(Malformed::BadField { field })
    };
    let Value::String(url) = url else {
        return Err(Malformed::BadField { field: "url" });
    };
    Ok(Place {
        url: Words::of(url),
        line: from_one("lineNumber")?,
        column: from_one("columnNumber")?,
    })
}

/// A `RemoteObject`, by its `type`, its `subtype` and what it carries.
fn shown(value: &Value) -> Result<Shown, Malformed> {
    let Value::Object(fields) = value else {
        return Err(Malformed::BadField { field: "args" });
    };
    let said = |field: &'static str| match fields.get(field) {
        Some(Value::String(said)) => Some(Words::of(said)),
        _ => None,
    };
    let subtype = fields.get("subtype").and_then(Value::as_str);
    Ok(
        match (text(fields.get("type"), "type")?.as_str(), subtype) {
            ("undefined", _) => Shown::Undefined,
            ("object", Some("null")) => Shown::Null,
            ("boolean", _) => Shown::Bool(
                fields
                    .get("value")
                    .and_then(Value::as_bool)
                    .ok_or(Malformed::BadField { field: "value" })?,
            ),
            ("number", _) => Shown::Number(
                said("description")
                    .or_else(|| said("unserializableValue"))
                    .ok_or(Malformed::BadField {
                        field: "description",
                    })?,
            ),
            ("bigint", _) => Shown::BigInt(
                said("unserializableValue")
                    .or_else(|| said("description"))
                    .ok_or(Malformed::BadField {
                        field: "unserializableValue",
                    })?,
            ),
            ("string", _) => {
                Shown::Text(said("value").ok_or(Malformed::BadField { field: "value" })?)
            }
            ("symbol", _) => Shown::Symbol(said("description").ok_or(Malformed::BadField {
                field: "description",
            })?),
            ("object", Some("error")) => {
                let described = match fields.get("description") {
                    Some(Value::String(described)) => described.lines().next().unwrap_or_default(),
                    _ => {
                        return Err(Malformed::BadField {
                            field: "description",
                        });
                    }
                };
                Shown::Error(Words::of(described))
            }
            ("object" | "function", _) => {
                Shown::Object(said("description").or_else(|| said("className")).ok_or(
                    Malformed::BadField {
                        field: "description",
                    },
                )?)
            }
            _ => return Err(Malformed::BadField { field: "type" }),
        },
    )
}

/// The most targets [`pages`] reads: a Chrome holding more is refused
/// rather than read. A shown Chrome 155 holds six as it starts — its window's
/// page, two of its own interface and three of its extensions'.
pub const MOST_TARGETS: usize = 4096;

/// The pages Chrome holds that nothing is attached to yet, by their
/// `targetId`, from a reply's `result` to `Target.getTargets`. A shown
/// Chrome's own window is one of these as it starts; its interface
/// (`browser_ui`), its extensions and their workers are not pages.
///
/// # Errors
///
/// [`Malformed`], for a result without `targetInfos`, more than
/// [`MOST_TARGETS`] of them, or one without a `targetId`, a `type` and an
/// `attached`.
pub fn pages(result: &[u8]) -> Result<Vec<String>, Malformed> {
    Ok(targets(result)?
        .into_iter()
        .filter(|held| held.kind == "page" && !held.attached)
        .map(|held| held.id)
        .collect())
}

/// Whether Chrome still holds the target `target`, of whatever kind, from a
/// reply's `result` to `Target.getTargets`: a tab whose page a person has
/// closed is not held (item 672).
///
/// # Errors
///
/// [`Malformed`], as [`pages`].
pub fn holds(result: &[u8], target: &str) -> Result<bool, Malformed> {
    Ok(targets(result)?.iter().any(|held| held.id == target))
}

/// What a page is called and where it is, by Chrome's word: what the tab
/// in the frame around Chrome's window shows (queue item 681).
///
/// Both are a stranger's strings — a page chose its title, and its
/// address is wherever it went — so they are kept as text, cut to
/// `alo-renderer`'s [`LONGEST_TITLE`](alo_renderer::strip::LONGEST_TITLE)
/// as every title this browser keeps is, and only ever drawn by the tab
/// strip's own renderer (ADR 0024 § 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    /// Its title, as Chrome gives it: the document's, or its address when
    /// it has none.
    pub title: String,
    /// Its address.
    pub address: String,
}

/// What Chrome calls the target `target` and where it is, from a reply's
/// `result` to `Target.getTargets` — or [`None`] when it holds no such
/// target.
///
/// # Errors
///
/// [`Malformed`], as [`pages`], and for the target asked about without a
/// `title` or a `url`.
pub fn named(result: &[u8], target: &str) -> Result<Option<Named>, Malformed> {
    let Some(held) = targets(result)?.into_iter().find(|held| held.id == target) else {
        return Ok(None);
    };
    let title = held.title.ok_or(Malformed::BadField { field: "title" })?;
    let address = held.url.ok_or(Malformed::BadField { field: "url" })?;
    Ok(Some(Named {
        title: alo_renderer::strip::longest(&title).to_owned(),
        address: alo_renderer::strip::longest(&address).to_owned(),
    }))
}

/// One target in a reply to `Target.getTargets`.
struct Held {
    id: String,
    kind: String,
    attached: bool,
    /// Its `title`, if it was given as text.
    title: Option<String>,
    /// Its `url`, if it was given as text.
    url: Option<String>,
}

/// Every target in a reply's `result` to `Target.getTargets`: its id, its
/// type, whether something is attached to it, and its title and address
/// where they were given.
fn targets(result: &[u8]) -> Result<Vec<Held>, Malformed> {
    let bad = Malformed::BadField {
        field: "targetInfos",
    };
    let Some(Value::Array(targets)) = object(result)?.get("targetInfos").cloned() else {
        return Err(bad);
    };
    if targets.len() > MOST_TARGETS {
        return Err(bad);
    }
    let mut read = Vec::new();
    for target in &targets {
        let Value::Object(info) = target else {
            return Err(bad);
        };
        let id = text(info.get("targetId"), "targetId")?;
        let kind = text(info.get("type"), "type")?;
        let attached = info
            .get("attached")
            .and_then(Value::as_bool)
            .ok_or(Malformed::BadField { field: "attached" })?;
        read.push(Held {
            id,
            kind,
            attached,
            title: words(info.get("title"), "title").ok(),
            url: words(info.get("url"), "url").ok(),
        });
    }
    Ok(read)
}

/// Where a window is on the screen, in the screen's own points — what
/// macOS calls a point and Chrome a pixel of the screen, half a device
/// pixel on a screen of scale two — as `Browser.setWindowBounds` takes it
/// and `Browser.getWindowForTarget` gives it: the whole window, its title
/// bar included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// Its left edge, from the left of the main screen.
    pub left: i64,
    /// Its top edge, from the top of the main screen.
    pub top: i64,
    /// Its width.
    pub width: i64,
    /// Its height, its title bar included.
    pub height: i64,
}

impl Bounds {
    /// As `Browser.setWindowBounds` takes it.
    fn written(&self) -> Value {
        let mut at = Map::new();
        for (name, value) in [
            ("left", self.left),
            ("top", self.top),
            ("width", self.width),
            ("height", self.height),
        ] {
            at.insert(name.to_owned(), Value::from(value));
        }
        Value::Object(at)
    }
}

/// Whether a window is on the screen as a window, in Chrome's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    /// `normal`: on the screen, at its bounds.
    Normal,
    /// `minimized`: in the Dock, where nobody sees the page.
    Minimized,
    /// `maximized`: filling the screen but for the menu bar and the Dock.
    Maximized,
    /// `fullscreen`: the whole screen, a space of its own.
    Fullscreen,
}

/// A window of Chrome's, from `Browser.getWindowForTarget`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChromeWindow {
    /// Its `windowId`, which [`Command::PlaceWindow`] names it by.
    pub id: i64,
    /// Where it is.
    pub bounds: Bounds,
    /// Whether it is shown at those bounds.
    pub state: WindowState,
}

/// A reply's `result` to `Browser.getWindowForTarget`, read.
///
/// # Errors
///
/// [`Malformed`], for a result without a whole `windowId`, or `bounds`
/// without a whole `left`, `top`, `width` and `height` — the last two not
/// negative — and a `windowState` that is one of Chrome's four.
pub fn window_of(result: &[u8]) -> Result<ChromeWindow, Malformed> {
    let fields = object(result)?;
    let id = fields
        .get("windowId")
        .and_then(Value::as_i64)
        .ok_or(Malformed::BadField { field: "windowId" })?;
    let Some(Value::Object(bounds)) = fields.get("bounds") else {
        return Err(Malformed::BadField { field: "bounds" });
    };
    let whole = |field: &'static str| {
        bounds
            .get(field)
            .and_then(Value::as_i64)
            .ok_or(Malformed::BadField { field })
    };
    let (width, height) = (whole("width")?, whole("height")?);
    if width < 0 || height < 0 {
        return Err(Malformed::BadField { field: "bounds" });
    }
    let state = match text(bounds.get("windowState"), "windowState")?.as_str() {
        "normal" => WindowState::Normal,
        "minimized" => WindowState::Minimized,
        "maximized" => WindowState::Maximized,
        "fullscreen" => WindowState::Fullscreen,
        _ => {
            return Err(Malformed::BadField {
                field: "windowState",
            });
        }
    };
    Ok(ChromeWindow {
        id,
        bounds: Bounds {
            left: whole("left")?,
            top: whole("top")?,
            width,
            height,
        },
        state,
    })
}

/// One node of Chrome's accessibility tree, as `Accessibility.getFullAXTree`
/// describes it — read, and not yet anything of this browser's.
/// [`crate::chrome_tree`] is what makes it one.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AxNode {
    /// Its id in this tree, which its parent's `children` name it by.
    pub id: String,
    /// Whether Chrome would have a screen reader pass over it.
    pub ignored: bool,
    /// Its role, as Chrome names it: ARIA's word, or one of Chrome's own
    /// such as `StaticText`.
    pub role: String,
    /// What it is called, if anything — possibly empty.
    pub name: Option<String>,
    /// What it holds, as text, if Chrome says.
    pub value: Option<String>,
    /// What else is true of it, by Chrome's name, as text: `true` and
    /// `false`, a token such as `mixed`, or a number. A property whose value
    /// is a list of nodes is not read.
    pub properties: Vec<(String, String)>,
    /// Its children, by [`AxNode::id`], in order.
    pub children: Vec<String>,
    /// The DOM node it describes, by Chrome's `backendDOMNodeId`, if any.
    pub node: Option<u64>,
}

/// A reply's `result` to `Accessibility.getFullAXTree`, read.
///
/// # Errors
///
/// [`Malformed`], for a node that is not the shape the protocol says, or
/// more than [`MOST_NODES`] of them.
pub fn ax_nodes(result: &[u8]) -> Result<Vec<AxNode>, Malformed> {
    let Some(Value::Array(nodes)) = object(result)?.get("nodes").cloned() else {
        return Err(Malformed::BadField { field: "nodes" });
    };
    if nodes.len() > MOST_NODES {
        return Err(Malformed::BadField { field: "nodes" });
    }
    nodes.iter().map(ax_node).collect()
}

/// One node of [`ax_nodes`].
fn ax_node(node: &Value) -> Result<AxNode, Malformed> {
    let Value::Object(fields) = node else {
        return Err(Malformed::BadField { field: "nodes" });
    };
    let ignored = match fields.get("ignored") {
        Some(Value::Bool(ignored)) => *ignored,
        _ => return Err(Malformed::BadField { field: "ignored" }),
    };
    let role = match fields.get("role") {
        Some(Value::Object(role)) => Some(text(role.get("value"), "role")?),
        // An ignored node may have no role at all.
        None if ignored => None,
        _ => return Err(Malformed::BadField { field: "role" }),
    };
    let mut properties = Vec::new();
    if let Some(listed) = fields.get("properties") {
        let Value::Array(listed) = listed else {
            return Err(Malformed::BadField {
                field: "properties",
            });
        };
        for property in listed {
            let Value::Object(property) = property else {
                return Err(Malformed::BadField {
                    field: "properties",
                });
            };
            let name = text(property.get("name"), "name")?;
            if let Some(value) = property.get("value").and_then(ax_value).transpose()? {
                properties.push((name, value));
            }
        }
    }
    let children = match fields.get("childIds") {
        None => Vec::new(),
        Some(Value::Array(children)) => children
            .iter()
            .map(|child| text(Some(child), "childIds"))
            .collect::<Result<_, _>>()?,
        Some(_) => return Err(Malformed::BadField { field: "childIds" }),
    };
    Ok(AxNode {
        id: text(fields.get("nodeId"), "nodeId")?,
        ignored,
        role: role.unwrap_or_default(),
        name: fields.get("name").and_then(ax_value).transpose()?,
        value: fields.get("value").and_then(ax_value).transpose()?,
        properties,
        children,
        node: match fields.get("backendDOMNodeId") {
            None => None,
            Some(node) => Some(node.as_u64().ok_or(Malformed::BadField {
                field: "backendDOMNodeId",
            })?),
        },
    })
}

/// An `AXValue`'s `value`, as text: [`None`] when it has none, or holds a
/// list of nodes, which nothing here reads.
fn ax_value(value: &Value) -> Option<Result<String, Malformed>> {
    let Value::Object(fields) = value else {
        return Some(Err(Malformed::BadField { field: "value" }));
    };
    match fields.get("value")? {
        Value::String(_) => Some(words(fields.get("value"), "value")),
        Value::Bool(flag) => Some(Ok(flag.to_string())),
        Value::Number(number) => Some(Ok(number.to_string())),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

/// `json` as an object, its depth refused before it is parsed.
fn object(json: &[u8]) -> Result<Map<String, Value>, Malformed> {
    if json_depth::of(json) > MOST_NESTING {
        return Err(Malformed::TooDeep { most: MOST_NESTING });
    }
    match serde_json::from_slice::<Value>(json) {
        Ok(Value::Object(fields)) => Ok(fields),
        Ok(_) => Err(Malformed::NotAnObject),
        Err(_) => Err(Malformed::NotJson),
    }
}

/// An `error`'s code and words.
fn declined(error: &Value) -> Result<Declined, Malformed> {
    let Value::Object(fields) = error else {
        return Err(Malformed::BadField { field: "error" });
    };
    Ok(Declined {
        code: fields
            .get("code")
            .and_then(Value::as_i64)
            .ok_or(Malformed::BadField { field: "code" })?,
        message: text(fields.get("message"), "message")?,
    })
}

/// A text field, at most [`MOST_TEXT`] characters.
fn text(value: Option<&Value>, field: &'static str) -> Result<String, Malformed> {
    let Some(Value::String(text)) = value else {
        return Err(Malformed::BadField { field });
    };
    if text.chars().count() > MOST_TEXT {
        return Err(Malformed::TooLong {
            field,
            most: MOST_TEXT,
        });
    }
    Ok(text.clone())
}

/// A text that is a page's words — a name, a value, an address — and so as
/// long as the page made it, bounded by the message it came in rather than
/// by [`MOST_TEXT`].
fn words(value: Option<&Value>, field: &'static str) -> Result<String, Malformed> {
    let Some(Value::String(words)) = value else {
        return Err(Malformed::BadField { field });
    };
    Ok(words.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bad<T>(field: &'static str) -> Result<T, Malformed> {
        Err(Malformed::BadField { field })
    }

    #[test]
    fn a_command_is_written_as_chrome_reads_it() {
        assert_eq!(
            written(1, None, &Command::GetVersion),
            br#"{"id":1,"method":"Browser.getVersion","params":{}}"#
        );
        assert_eq!(
            written(u64::MAX, None, &Command::Close),
            br#"{"id":18446744073709551615,"method":"Browser.close","params":{}}"#
        );
    }

    #[test]
    fn a_reply_carries_its_id_and_its_result() {
        let reply = read(br#"{"id":7,"result":{"a":[1,2]}}"#).unwrap();
        assert_eq!(
            reply,
            Message::Reply {
                id: 7,
                session: None,
                result: Ok(br#"{"a":[1,2]}"#.to_vec())
            }
        );
    }

    #[test]
    fn a_reply_declined_carries_chromes_reason() {
        assert_eq!(
            read(br#"{"id":2,"error":{"code":-32601,"message":"'Nope.x' wasn't found"}}"#),
            Ok(Message::Reply {
                id: 2,
                session: None,
                result: Err(Declined {
                    code: -32601,
                    message: "'Nope.x' wasn't found".to_owned()
                })
            })
        );
    }

    #[test]
    fn an_event_is_named_by_its_method() {
        assert_eq!(
            read(br#"{"method":"Target.targetCreated","params":{"targetInfo":{}}}"#),
            Ok(Message::Event {
                method: "Target.targetCreated".to_owned(),
                session: None,
                params: br#"{"targetInfo":{}}"#.to_vec(),
            })
        );
    }

    #[test]
    fn a_command_to_a_tab_names_its_session_and_its_params() {
        assert_eq!(
            written(
                3,
                Some("S1"),
                &Command::Navigate {
                    url: "https://httpbin.org/forms/post".to_owned()
                }
            ),
            br#"{"id":3,"method":"Page.navigate","params":{"url":"https://httpbin.org/forms/post"},"sessionId":"S1"}"#
        );
        assert_eq!(
            written(4, None, &Command::AttachToTarget { target: "T".to_owned() }),
            br#"{"id":4,"method":"Target.attachToTarget","params":{"flatten":true,"targetId":"T"}}"#
        );
        assert_eq!(
            written(5, Some("S"), &Command::PauseEveryRequest),
            br#"{"id":5,"method":"Fetch.enable","params":{"patterns":[{"urlPattern":"*"}]},"sessionId":"S"}"#
        );
        assert_eq!(
            written(5, Some("S"), &Command::HearTheNetworkOut),
            br#"{"id":5,"method":"Network.enable","params":{},"sessionId":"S"}"#
        );
        assert_eq!(
            written(6, Some("S"), &Command::Refuse { request: "R".to_owned() }),
            br#"{"id":6,"method":"Fetch.failRequest","params":{"errorReason":"BlockedByClient","requestId":"R"},"sessionId":"S"}"#
        );
        assert_eq!(
            written(7, Some("S"), &Command::WhereDrawn { node: 62 }),
            br#"{"id":7,"method":"DOM.getContentQuads","params":{"backendNodeId":62},"sessionId":"S"}"#
        );
    }

    #[test]
    fn a_mark_is_written_as_chromes_overlay_reads_it() {
        assert_eq!(
            written(
                8,
                Some("S"),
                &Command::Mark {
                    node: 62,
                    border: (0xe7, 0x6f, 0x51)
                }
            ),
            br#"{"id":8,"method":"Overlay.highlightNode","params":{"backendNodeId":62,"highlightConfig":{"borderColor":{"a":1,"b":81,"g":111,"r":231},"showAccessibilityInfo":true,"showInfo":true}},"sessionId":"S"}"#
        );
        for (command, method) in [
            (Command::ReadDocument, "DOM.getDocument"),
            (Command::ShowMarks, "Overlay.enable"),
            (Command::Unmark, "Overlay.hideHighlight"),
        ] {
            assert_eq!(
                String::from_utf8_lossy(&written(9, Some("S"), &command)),
                format!(r#"{{"id":9,"method":"{method}","params":{{}},"sessionId":"S"}}"#)
            );
        }
    }

    #[test]
    fn a_click_is_written_as_the_input_chrome_reads() {
        let at = |command: &Command| {
            String::from_utf8_lossy(&written(9, Some("S"), command)).into_owned()
        };
        assert_eq!(
            at(&Command::Mouse {
                kind: Mouse::Pressed,
                x: 35.0,
                y: 249.0,
                toggles: false,
            }),
            r#"{"id":9,"method":"Input.dispatchMouseEvent","params":{"button":"left","clickCount":1,"modifiers":0,"type":"mousePressed","x":35.0,"y":249.0},"sessionId":"S"}"#
        );
        let modifier = if cfg!(target_os = "macos") { 4 } else { 2 };
        assert!(
            at(&Command::Mouse {
                kind: Mouse::Pressed,
                x: 35.0,
                y: 249.0,
                toggles: true,
            })
            .contains(&format!(
                r#""clickCount":1,"modifiers":{modifier},"type":"mousePressed""#
            )),
            "a click that toggles holds the platform's modifier"
        );
        assert!(
            at(&Command::Mouse {
                kind: Mouse::Moved,
                x: 0.0,
                y: 0.0,
                toggles: false,
            })
            .contains(r#""button":"none","clickCount":1,"modifiers":0,"type":"mouseMoved""#)
        );
        assert!(
            at(&Command::Mouse {
                kind: Mouse::Released,
                x: 0.0,
                y: 0.0,
                toggles: false,
            })
            .contains(r#""type":"mouseReleased""#)
        );
    }

    #[test]
    fn a_verb_is_written_as_the_input_chrome_reads() {
        let at = |command: &Command| {
            String::from_utf8_lossy(&written(9, Some("S"), command)).into_owned()
        };
        assert_eq!(
            at(&Command::NodeAt { x: 35, y: -2 }),
            r#"{"id":9,"method":"DOM.getNodeForLocation","params":{"x":35,"y":-2},"sessionId":"S"}"#
        );
        let modifier = if cfg!(target_os = "macos") { 4 } else { 2 };
        assert_eq!(
            at(&Command::Key {
                chord: Chord::SelectAll,
                down: true
            }),
            format!(
                r#"{{"id":9,"method":"Input.dispatchKeyEvent","params":{{"code":"KeyA","commands":["selectAll"],"key":"a","modifiers":{modifier},"type":"rawKeyDown","windowsVirtualKeyCode":65}},"sessionId":"S"}}"#
            )
        );
        // A key coming up carries out nothing.
        assert_eq!(
            at(&Command::Key {
                chord: Chord::DeleteBackward,
                down: false
            }),
            r#"{"id":9,"method":"Input.dispatchKeyEvent","params":{"code":"Backspace","key":"Backspace","modifiers":0,"type":"keyUp","windowsVirtualKeyCode":8},"sessionId":"S"}"#
        );
        // A digit going down makes its character; its key code is the
        // digit's own, which is its ASCII code.
        let five = Chord::Digit(Digit::of('5').unwrap());
        assert_eq!(
            at(&Command::Key {
                chord: five,
                down: true
            }),
            r#"{"id":9,"method":"Input.dispatchKeyEvent","params":{"code":"Digit5","key":"5","modifiers":0,"text":"5","type":"keyDown","windowsVirtualKeyCode":53},"sessionId":"S"}"#
        );
        assert_eq!(
            at(&Command::Key {
                chord: five,
                down: false
            }),
            r#"{"id":9,"method":"Input.dispatchKeyEvent","params":{"code":"Digit5","key":"5","modifiers":0,"type":"keyUp","windowsVirtualKeyCode":53},"sessionId":"S"}"#
        );
        assert_eq!(Digit::of('0'), Some(Digit(b'0')));
        assert_eq!(Digit::of('a'), None);
        assert_eq!(Digit::of('٣'), None, "a digit of another script");
        assert_eq!(
            at(&Command::InsertText {
                text: "Ada \"L\"".to_owned()
            }),
            r#"{"id":9,"method":"Input.insertText","params":{"text":"Ada \"L\""},"sessionId":"S"}"#
        );
        for (command, method) in [
            (Command::Describe { node: 4 }, "DOM.describeNode"),
            (Command::Focus { node: 4 }, "DOM.focus"),
            (
                Command::ScrollIntoView { node: 4 },
                "DOM.scrollIntoViewIfNeeded",
            ),
        ] {
            assert_eq!(
                at(&command),
                format!(
                    r#"{{"id":9,"method":"{method}","params":{{"backendNodeId":4}},"sessionId":"S"}}"#
                )
            );
        }
        assert_eq!(
            at(&Command::ReadNode { node: 4 }),
            r#"{"id":9,"method":"Accessibility.getPartialAXTree","params":{"backendNodeId":4,"fetchRelatives":false},"sessionId":"S"}"#
        );
    }

    #[test]
    fn what_a_verb_is_answered_is_read_as_chrome_155_wrote_it() {
        // Cut from Chrome 155's replies on 2026-10-24.
        assert_eq!(
            described(
                br##"{"node":{"nodeId":0,"backendNodeId":4,"nodeType":1,"nodeName":"INPUT","localName":"input","nodeValue":"","childNodeCount":0,"attributes":["name","m","type","email"],"shadowRoots":[{"nodeId":0,"backendNodeId":42,"nodeType":11,"nodeName":"#document-fragment","localName":"","nodeValue":"","childNodeCount":1,"shadowRootType":"user-agent"}]}}"##
            ),
            Ok(Described {
                node_name: "INPUT".to_owned(),
                local_name: "input".to_owned(),
                attributes: vec![
                    ("name".to_owned(), "m".to_owned()),
                    ("type".to_owned(), "email".to_owned()),
                ],
            })
        );
        assert_eq!(
            described(br#"{"node":{"nodeName":"BUTTON","localName":"button","attributes":[]}}"#)
                .map(|button| button.attributes.len()),
            Ok(0)
        );
        assert_eq!(
            node_at(br#"{"backendNodeId":76,"frameId":"C8737101013A69B023A314D542538DF6"}"#),
            Ok(76)
        );
        assert_eq!(
            requested(
                br#"{"frameId":"9539EA3915583FF1207EA009417EC2B9","reason":"formSubmissionPost","url":"https://httpbin.org/post","disposition":"currentTab"}"#
            ),
            Ok(Requested {
                frame: "9539EA3915583FF1207EA009417EC2B9".to_owned(),
                reason: "formSubmissionPost".to_owned(),
                url: "https://httpbin.org/post".to_owned(),
                disposition: "currentTab".to_owned(),
            })
        );
    }

    #[test]
    fn a_tab_opened_by_another_is_read_as_chrome_155_tells_of_it() {
        assert_eq!(
            String::from_utf8_lossy(&written(2, None, &Command::HoldNewTabs { hold: true })),
            r#"{"id":2,"method":"Target.setAutoAttach","params":{"autoAttach":true,"flatten":true,"waitForDebuggerOnStart":true}}"#
        );
        assert_eq!(
            String::from_utf8_lossy(&written(3, None, &Command::HoldNewTabs { hold: false })),
            r#"{"id":3,"method":"Target.setAutoAttach","params":{"autoAttach":false,"flatten":true,"waitForDebuggerOnStart":false}}"#
        );
        // Cut from Chrome 155's event on 2026-10-24, a link with
        // `target=_blank` pressed.
        assert_eq!(
            attached(
                br#"{"sessionId":"A1","targetInfo":{"targetId":"B2","type":"page","title":"","url":"","attached":true,"openerId":"20360EF611555F6E57382076C619E00F","canAccessOpener":false,"browserContextId":"C3"},"waitingForDebugger":true}"#
            ),
            Ok(Attached {
                session: "A1".to_owned(),
                target: "B2".to_owned(),
                kind: "page".to_owned(),
                opener: Some("20360EF611555F6E57382076C619E00F".to_owned()),
            })
        );
        assert_eq!(
            attached(br#"{"sessionId":"A","targetInfo":{"targetId":"B2","type":"page"}}"#)
                .map(|tab| tab.opener),
            Ok(None)
        );
        assert_eq!(
            attached(br#"{"sessionId":"A","targetInfo":{"targetId":"B2"}}"#),
            bad("type")
        );
        assert_eq!(
            attached(br#"{"sessionId":"A","targetInfo":{"targetId":"B2","type":["iframe"]}}"#),
            bad("type")
        );
        assert_eq!(attached(br#"{"sessionId":"A"}"#), bad("targetInfo"));
        assert_eq!(
            attached(br#"{"sessionId":"A","targetInfo":[]}"#),
            bad("targetInfo")
        );
        assert_eq!(
            attached(br#"{"targetInfo":{"targetId":"B"}}"#),
            bad("sessionId")
        );
        assert_eq!(
            attached(br#"{"sessionId":"A","targetInfo":{}}"#),
            bad("targetId")
        );
        assert_eq!(
            attached(
                br#"{"sessionId":"A","targetInfo":{"targetId":"B","type":"page","openerId":7}}"#
            ),
            bad("openerId")
        );
        assert_eq!(
            String::from_utf8_lossy(&written(4, Some("A"), &Command::LetGo)),
            r#"{"id":4,"method":"Runtime.runIfWaitingForDebugger","params":{},"sessionId":"A"}"#
        );
    }

    #[test]
    fn a_frame_and_a_worker_of_a_tab_are_read_as_chrome_155_tells_of_them() {
        // Cut from Chrome 155's events on 2026-10-24, through a tab's own
        // session: a frame from another site, then a service worker.
        assert_eq!(
            attached(
                br#"{"sessionId":"F1","targetInfo":{"targetId":"F2","type":"iframe","title":"","url":"","attached":true,"canAccessOpener":false,"browserContextId":"C3"},"waitingForDebugger":true}"#
            ),
            Ok(Attached {
                session: "F1".to_owned(),
                target: "F2".to_owned(),
                kind: "iframe".to_owned(),
                opener: None,
            })
        );
        assert_eq!(
            attached(
                br#"{"sessionId":"W1","targetInfo":{"targetId":"W2","type":"service_worker","title":"","url":"http://127.0.0.1:53040/sw.js","attached":true,"canAccessOpener":false},"waitingForDebugger":true}"#
            )
            .map(|worker| worker.kind),
            Ok("service_worker".to_owned())
        );
        assert_eq!(
            detached(br#"{"sessionId":"F1","targetId":"F2"}"#),
            Ok("F1".to_owned())
        );
        assert_eq!(detached(br#"{"targetId":"F2"}"#), bad("sessionId"));
        assert_eq!(detached(br#"{"sessionId":7}"#), bad("sessionId"));
        assert_eq!(detached(b"[]"), Err(Malformed::NotAnObject));
        assert_eq!(
            String::from_utf8_lossy(&written(
                5,
                Some("F1"),
                &Command::Continue {
                    request: "R".to_owned()
                }
            )),
            r#"{"id":5,"method":"Fetch.continueRequest","params":{"requestId":"R"},"sessionId":"F1"}"#
        );
    }

    #[test]
    fn what_a_verb_is_answered_that_is_not_one_is_refused_as_a_kind() {
        assert_eq!(described(b"{}"), bad("node"));
        assert_eq!(described(br#"{"node":[]}"#), bad("node"));
        assert_eq!(
            described(br#"{"node":{"localName":"a","attributes":[]}}"#),
            bad("nodeName")
        );
        assert_eq!(
            described(br#"{"node":{"nodeName":"A","attributes":[]}}"#),
            bad("localName")
        );
        for not_pairs in [
            &br#"{"node":{"nodeName":"A","localName":"a","attributes":["href"]}}"#[..],
            br#"{"node":{"nodeName":"A","localName":"a","attributes":["href",1]}}"#,
            br#"{"node":{"nodeName":"A","localName":"a","attributes":[1,"x"]}}"#,
            br#"{"node":{"nodeName":"A","localName":"a","attributes":{}}}"#,
        ] {
            assert_eq!(described(not_pairs), bad("attributes"));
        }
        let too_many = format!(
            r#"{{"node":{{"nodeName":"A","localName":"a","attributes":[{}]}}}}"#,
            vec![r#""x","y""#; MOST_ATTRIBUTES + 1].join(",")
        );
        assert_eq!(described(too_many.as_bytes()), bad("attributes"));
        let most = format!(
            r#"{{"node":{{"nodeName":"A","localName":"a","attributes":[{}]}}}}"#,
            vec![r#""x","y""#; MOST_ATTRIBUTES].join(",")
        );
        assert_eq!(
            described(most.as_bytes()).map(|node| node.attributes.len()),
            Ok(MOST_ATTRIBUTES)
        );
        let long_name = format!(
            r#"{{"node":{{"nodeName":"{0}","localName":"a"}}}}"#,
            "A".repeat(MOST_TEXT + 1)
        );
        assert_eq!(
            described(long_name.as_bytes()),
            Err(Malformed::TooLong {
                field: "nodeName",
                most: MOST_TEXT
            })
        );
        for not_a_node in [
            &b"{}"[..],
            br#"{"backendNodeId":-1}"#,
            br#"{"backendNodeId":1.5}"#,
            br#"{"backendNodeId":"76"}"#,
        ] {
            assert_eq!(node_at(not_a_node), bad("backendNodeId"));
        }
        let all = r#""frameId":"F","reason":"anchorClick","url":"u","disposition":"currentTab""#;
        for (field, gone) in [
            ("frameId", r#""frameId":"F","#),
            ("reason", r#""reason":"anchorClick","#),
            ("url", r#""url":"u","#),
        ] {
            let params = format!("{{{}}}", all.replace(gone, ""));
            assert_eq!(requested(params.as_bytes()), bad(field), "{params}");
        }
        assert_eq!(
            requested(
                format!("{{{}}}", all.replace(r#","disposition":"currentTab""#, "")).as_bytes()
            ),
            bad("disposition")
        );
        assert_eq!(requested(b"[]"), Err(Malformed::NotAnObject));
    }

    #[test]
    fn a_body_is_sent_in_base64() {
        // RFC 4648 § 10's own vectors.
        for (bytes, written) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(bytes.as_bytes()), written);
        }
        assert_eq!(base64(&[0xff, 0xfe, 0x00]), "//4A");
        let fulfil = written(
            8,
            Some("S"),
            &Command::Fulfil {
                request: "R".to_owned(),
                body: b"<p>hi".to_vec(),
            },
        );
        let fulfil = String::from_utf8(fulfil).unwrap();
        assert!(fulfil.contains(r#""body":"PHA+aGk=""#), "{fulfil}");
        assert!(fulfil.contains(r#""responseCode":200"#), "{fulfil}");
    }

    #[test]
    fn a_tabs_messages_carry_its_session() {
        assert_eq!(
            read(br#"{"id":9,"result":{},"sessionId":"S1"}"#),
            Ok(Message::Reply {
                id: 9,
                session: Some("S1".to_owned()),
                result: Ok(b"{}".to_vec())
            })
        );
        assert_eq!(
            read(br#"{"method":"Page.loadEventFired","sessionId":"S1"}"#),
            Ok(Message::Event {
                method: "Page.loadEventFired".to_owned(),
                session: Some("S1".to_owned()),
                params: b"{}".to_vec(),
            })
        );
        assert_eq!(
            read(br#"{"id":1,"result":{},"sessionId":5}"#),
            bad("sessionId")
        );
        assert_eq!(read(br#"{"method":"M","params":[1]}"#), bad("params"));
    }

    /// Item 647: what a renderer tells of a request, and a paused
    /// request's id for it.
    #[test]
    fn a_request_a_renderer_told_of_is_read_as_chrome_155_wrote_it() {
        // A sub-resource's, as Chrome 155 told of it on 2026-10-25, cut.
        assert_eq!(
            told(
                br#"{"requestId":"35276.7","loaderId":"E72554449136EB07321E2ECE550E5495","documentURL":"http://127.0.0.1:50301/next.html","request":{"url":"http://127.0.0.1:50301/next.css","method":"GET","headers":{}},"type":"Stylesheet","frameId":"F"}"#
            ),
            Ok((
                "35276.7".to_owned(),
                "E72554449136EB07321E2ECE550E5495".to_owned()
            ))
        );
        assert_eq!(
            told(br#"{"requestId":"1.1","loaderId":""}"#),
            Ok(("1.1".to_owned(), String::new()))
        );
        assert_eq!(told(br#"{"requestId":"1.1"}"#), bad("loaderId"));
        assert_eq!(told(br#"{"loaderId":"L"}"#), bad("requestId"));
        assert_eq!(told(br#"{"requestId":7,"loaderId":"L"}"#), bad("requestId"));
        assert_eq!(told(b"[]"), Err(Malformed::NotAnObject));
        assert_eq!(told(br#"{"requestId":"1""#), Err(Malformed::NotJson));
        assert_eq!(
            paused(br#"{"requestId":"i","request":{"url":"u","method":"GET"},"frameId":"F","resourceType":"Other","networkId":5}"#),
            bad("networkId")
        );
    }

    #[test]
    fn a_tab_and_its_loading_are_read_as_chrome_155_wrote_them() {
        assert_eq!(
            target(br#"{"targetId":"6D9C1C29E4A1B1AF4B9A2E0F52E7A9A1"}"#),
            Ok("6D9C1C29E4A1B1AF4B9A2E0F52E7A9A1".to_owned())
        );
        assert_eq!(
            session(br#"{"sessionId":"25057CA2B162E02BE7DC288E2859484B"}"#),
            Ok("25057CA2B162E02BE7DC288E2859484B".to_owned())
        );
        assert_eq!(
            navigated(
                br#"{"frameId":"356802D683199705C350AFB81778A114","loaderId":"4CA991F60657F9312AD4905BBF81C1E0","isDownload":false}"#
            ),
            Ok(Ok("4CA991F60657F9312AD4905BBF81C1E0".to_owned()))
        );
        assert_eq!(
            navigated(
                br#"{"frameId":"F","loaderId":"L","errorText":"net::ERR_BLOCKED_BY_CLIENT"}"#
            ),
            Ok(Err("net::ERR_BLOCKED_BY_CLIENT".to_owned()))
        );
        assert_eq!(
            stage(br#"{"frameId":"F","loaderId":"L","name":"load","timestamp":1.5}"#),
            Ok(("L".to_owned(), "load".to_owned()))
        );
        assert_eq!(
            paused(
                br#"{"requestId":"interception-job-1.0","request":{"url":"https://httpbin.org/favicon.ico","method":"GET"},"frameId":"F","resourceType":"Other"}"#
            ),
            Ok(Paused {
                request: "interception-job-1.0".to_owned(),
                url: "https://httpbin.org/favicon.ico".to_owned(),
                method: "GET".to_owned(),
                kind: "Other".to_owned(),
                frame: "F".to_owned(),
                redirected: false,
                network: None,
            })
        );
        // A redirect's second hop, cut from Chrome 155's on 2026-10-24.
        assert_eq!(
            paused(
                br#"{"requestId":"interception-job-2.0","request":{"url":"http://localhost:56168/page.html","method":"GET","headers":{}},"frameId":"6E8136AF8E77D33D416721A3E4AE05AF","resourceType":"Document","networkId":"0C6B5699858B4B5F4400C77CBFB45BA3","redirectedRequestId":"interception-job-1.0"}"#
            )
            .map(|paused| (paused.kind, paused.redirected, paused.network)),
            Ok((
                "Document".to_owned(),
                true,
                Some("0C6B5699858B4B5F4400C77CBFB45BA3".to_owned())
            ))
        );
        assert_eq!(
            committed(
                br#"{"frame":{"id":"6E8136AF8E77D33D416721A3E4AE05AF","loaderId":"0C6B","url":"http://localhost:56168/page.html"},"type":"Navigation"}"#
            ),
            Ok(Committed {
                frame: "6E8136AF8E77D33D416721A3E4AE05AF".to_owned(),
                inside: false,
                url: "http://localhost:56168/page.html".to_owned()
            })
        );
        assert_eq!(
            committed(br#"{"frame":{"id":"C","parentId":"P","url":"u"}}"#),
            Ok(Committed {
                frame: "C".to_owned(),
                inside: true,
                url: "u".to_owned()
            })
        );
        assert_eq!(
            committed(br##"{"frame":{"id":"F","url":"https://a.test/p","urlFragment":"#x"}}"##)
                .map(|committed| committed.url),
            Ok("https://a.test/p#x".to_owned())
        );
        assert_eq!(
            socket(
                br#"{"requestId":"29040.6","url":"ws://127.0.0.1:56168/ws","initiator":{"type":"script"}}"#
            ),
            Ok("ws://127.0.0.1:56168/ws".to_owned())
        );
        assert_eq!(
            quads(br#"{"quads":[[117.3125,16,270.3125,16,270.3125,37,117.3125,37]]}"#),
            Ok(vec![[
                117.3125, 16.0, 270.3125, 16.0, 270.3125, 37.0, 117.3125, 37.0
            ]])
        );
        assert_eq!(
            window(
                br#"{"cssLayoutViewport":{"pageX":0,"pageY":0,"clientWidth":600,"clientHeight":700},"cssContentSize":{"x":0,"y":0,"width":600,"height":700}}"#
            ),
            Ok((600.0, 700.0))
        );
    }

    #[test]
    fn a_move_within_a_document_is_read_as_chrome_155_wrote_it() {
        // Both cut from Chrome 155's on 2026-10-24.
        assert_eq!(
            within(
                br#"{"frameId":"ECD71688D9DB2E3433469C906B3BA487","url":"http://127.0.0.1:60476/a.html#part","navigationType":"fragment"}"#
            ),
            Ok(Within {
                frame: "ECD71688D9DB2E3433469C906B3BA487".to_owned(),
                url: "http://127.0.0.1:60476/a.html#part".to_owned(),
                how: "fragment".to_owned(),
            })
        );
        assert_eq!(
            within(
                br#"{"frameId":"ECD7","url":"http://127.0.0.1:60476/pushed.html?x","navigationType":"historyApi"}"#
            )
            .map(|within| (within.url, within.how)),
            Ok((
                "http://127.0.0.1:60476/pushed.html?x".to_owned(),
                "historyApi".to_owned()
            ))
        );
    }

    #[test]
    fn a_frame_made_and_gone_is_read_as_chrome_155_wrote_it() {
        // Cut from Chrome 155's on 2026-10-25.
        assert_eq!(
            frame_made(
                br#"{"frameId":"87E6FC1E571944E4C68A13873420196A","parentFrameId":"FD9DE64FB81D3A86EA5C2AD40CAEA1ED"}"#
            ),
            Ok((
                "87E6FC1E571944E4C68A13873420196A".to_owned(),
                "FD9DE64FB81D3A86EA5C2AD40CAEA1ED".to_owned()
            ))
        );
        assert_eq!(
            frame_gone(br#"{"frameId":"D70531C2DC63B5CB52FA7E9B16F73A92","reason":"swap"}"#),
            Ok((
                "D70531C2DC63B5CB52FA7E9B16F73A92".to_owned(),
                "swap".to_owned()
            ))
        );
        assert_eq!(
            frame_gone(br#"{"frameId":"F","reason":"remove"}"#).map(|(_, why)| why),
            Ok("remove".to_owned())
        );
    }

    #[test]
    fn a_frame_made_or_gone_that_is_not_one_is_refused_as_a_kind() {
        assert_eq!(frame_made(b"{}"), bad("frameId"));
        assert_eq!(frame_made(br#"{"frameId":"F"}"#), bad("parentFrameId"));
        assert_eq!(
            frame_made(br#"{"frameId":"F","parentFrameId":null}"#),
            bad("parentFrameId")
        );
        assert_eq!(
            frame_made(br#"{"frameId":7,"parentFrameId":"P"}"#),
            bad("frameId")
        );
        assert_eq!(frame_gone(br#"{"reason":"remove"}"#), bad("frameId"));
        assert_eq!(frame_gone(br#"{"frameId":"F"}"#), bad("reason"));
        assert_eq!(
            frame_gone(br#"{"frameId":"F","reason":["remove"]}"#),
            bad("reason")
        );
        assert_eq!(frame_made(b"[]"), Err(Malformed::NotAnObject));
        assert_eq!(frame_gone(b"\"F\""), Err(Malformed::NotAnObject));
        assert_eq!(
            frame_made(br#"{"frameId":"F","parentFrameId":"P"#),
            Err(Malformed::NotJson)
        );
        let long = format!(
            r#"{{"frameId":"{}","parentFrameId":"P"}}"#,
            "x".repeat(MOST_TEXT + 1)
        );
        assert_eq!(
            frame_made(long.as_bytes()),
            Err(Malformed::TooLong {
                field: "frameId",
                most: MOST_TEXT
            })
        );
    }

    #[test]
    fn a_move_within_a_document_that_is_not_one_is_refused_as_a_kind() {
        assert_eq!(within(b"{}"), bad("frameId"));
        assert_eq!(
            within(br#"{"frameId":"F","navigationType":"fragment"}"#),
            bad("url")
        );
        assert_eq!(
            within(br#"{"frameId":"F","url":7,"navigationType":"fragment"}"#),
            bad("url")
        );
        assert_eq!(
            within(br#"{"frameId":"F","url":"u"}"#),
            bad("navigationType")
        );
        assert_eq!(
            within(br#"{"frameId":["F"],"url":"u","navigationType":"fragment"}"#),
            bad("frameId")
        );
        assert_eq!(within(b"[]"), Err(Malformed::NotAnObject));
        assert_eq!(
            within(br#"{"frameId":"F","url":"u","navigationType":"frag"#),
            Err(Malformed::NotJson)
        );
        let long = format!(
            r#"{{"frameId":"F","url":"u","navigationType":"{}"}}"#,
            "x".repeat(MOST_TEXT + 1)
        );
        assert_eq!(
            within(long.as_bytes()),
            Err(Malformed::TooLong {
                field: "navigationType",
                most: MOST_TEXT
            })
        );
    }

    #[test]
    fn a_tab_reply_that_is_not_one_is_refused_as_a_kind() {
        assert_eq!(target(b"{}"), bad("targetId"));
        assert_eq!(session(br#"{"sessionId":null}"#), bad("sessionId"));
        assert_eq!(navigated(br#"{"errorText":3}"#), bad("errorText"));
        assert_eq!(navigated(br#"{"frameId":"F"}"#), bad("loaderId"));
        assert_eq!(stage(br#"{"loaderId":"L"}"#), bad("name"));
        assert_eq!(stage(br#"{"name":"load"}"#), bad("loaderId"));
        assert_eq!(paused(br#"{"requestId":"R"}"#), bad("request"));
        assert_eq!(paused(br#"{"requestId":"R","request":{}}"#), bad("url"));
        assert_eq!(paused(br#"{"request":{"url":"u"}}"#), bad("requestId"));
        let all = r#""requestId":"R","request":{"url":"u","method":"GET"},"resourceType":"Fetch","frameId":"F""#;
        for (field, gone) in [
            ("method", r#","method":"GET""#),
            ("resourceType", r#","resourceType":"Fetch""#),
            ("frameId", r#","frameId":"F""#),
        ] {
            let params = format!("{{{}}}", all.replace(gone, ""));
            assert_eq!(paused(params.as_bytes()), bad(field), "{params}");
        }
        let redirected = format!(r#"{{{all},"redirectedRequestId":7}}"#);
        assert_eq!(paused(redirected.as_bytes()), bad("redirectedRequestId"));
        assert_eq!(committed(b"{}"), bad("frame"));
        assert_eq!(committed(br#"{"frame":"F"}"#), bad("frame"));
        assert_eq!(committed(br#"{"frame":{}}"#), bad("id"));
        assert_eq!(
            committed(br#"{"frame":{"id":"C","parentId":1}}"#),
            bad("parentId")
        );
        assert_eq!(committed(br#"{"frame":{"id":"C"}}"#), bad("url"));
        assert_eq!(
            committed(br#"{"frame":{"id":"C","url":"u","urlFragment":5}}"#),
            bad("urlFragment")
        );
        assert_eq!(socket(b"{}"), bad("url"));
        assert_eq!(socket(br#"{"url":null}"#), bad("url"));
        assert_eq!(socket(b"[]"), Err(Malformed::NotAnObject));
        for not_quads in [
            &br"{}"[..],
            br#"{"quads":{}}"#,
            br#"{"quads":[[1,2,3]]}"#,
            br#"{"quads":[[1,2,3,4,5,6,7,"8"]]}"#,
            br#"{"quads":[1]}"#,
        ] {
            assert_eq!(quads(not_quads), bad("quads"));
        }
        let too_many = format!(
            r#"{{"quads":[{}]}}"#,
            vec!["[0,0,0,0,0,0,0,0]"; MOST_QUADS + 1].join(",")
        );
        assert_eq!(quads(too_many.as_bytes()), bad("quads"));
        assert_eq!(
            quads(br#"{"quads":[[1e999,0,0,0,0,0,0,0]]}"#),
            Err(Malformed::NotJson)
        );
        let field = "cssLayoutViewport";
        assert_eq!(window(b"{}"), bad(field));
        assert_eq!(
            window(br#"{"cssLayoutViewport":{"clientWidth":-1,"clientHeight":1}}"#),
            bad(field)
        );
        assert_eq!(
            window(br#"{"cssLayoutViewport":{"clientWidth":1}}"#),
            bad(field)
        );
    }

    /// Chrome 155's own description of `web-a-form`'s first field and the
    /// words beside it, cut from its reply on 2026-10-24.
    const A_FIELD: &[u8] = br#"{"nodes":[{"nodeId":"21","ignored":false,"role":{"type":"internalRole","value":"LabelText"},"chromeRole":{"type":"internalRole","value":103},"name":{"type":"computedString","value":"","sources":[{"type":"relatedElement","attribute":"aria-labelledby"},{"type":"attribute","attribute":"aria-label"}]},"properties":[],"parentId":"20","childIds":["62","3"],"backendDOMNodeId":21},{"nodeId":"3","ignored":false,"role":{"type":"role","value":"textbox"},"name":{"type":"computedString","value":"Customer name:","sources":[{"type":"relatedElement","value":{"type":"computedString","value":"Customer name:"},"nativeSource":"label"}]},"properties":[{"name":"invalid","value":{"type":"token","value":"false"}},{"name":"focusable","value":{"type":"booleanOrUndefined","value":true}},{"name":"editable","value":{"type":"token","value":"plaintext"}},{"name":"labelledby","value":{"type":"nodeList","relatedNodes":[{"backendDOMNodeId":21}]}}],"value":{"type":"string","value":""},"childIds":["22"],"backendDOMNodeId":3},{"nodeId":"17","ignored":true,"ignoredReasons":[{"name":"uninteresting","value":{"type":"boolean","value":true}}],"role":{"type":"role","value":"none"},"childIds":["19"],"backendDOMNodeId":17},{"nodeId":"-1000","ignored":false,"role":{"type":"internalRole","value":"InlineTextBox"},"name":{"type":"computedString","value":"Customer name: "},"childIds":[]}]}"#;

    #[test]
    fn an_accessibility_tree_is_read_as_chrome_155_wrote_it() {
        let nodes = ax_nodes(A_FIELD).unwrap();
        assert_eq!(nodes.len(), 4);
        let [label, field, ignored, inline] = &nodes[..] else {
            panic!("four nodes");
        };
        assert_eq!(label.role, "LabelText");
        assert_eq!(label.name.as_deref(), Some(""));
        assert_eq!(label.children, ["62", "3"]);
        assert_eq!(label.node, Some(21));
        assert_eq!(field.role, "textbox");
        assert_eq!(field.name.as_deref(), Some("Customer name:"));
        assert_eq!(field.value.as_deref(), Some(""));
        // The list of nodes that label it is not read; everything else is.
        assert_eq!(
            field.properties,
            [
                ("invalid".to_owned(), "false".to_owned()),
                ("focusable".to_owned(), "true".to_owned()),
                ("editable".to_owned(), "plaintext".to_owned()),
            ]
        );
        assert!(ignored.ignored);
        assert_eq!(ignored.children, ["19"]);
        assert_eq!(inline.node, None);
        assert_eq!(inline.id, "-1000");
    }

    #[test]
    fn an_accessibility_tree_that_is_not_one_is_refused_as_a_kind() {
        assert_eq!(ax_nodes(b"{}"), bad("nodes"));
        assert_eq!(ax_nodes(br#"{"nodes":[1]}"#), bad("nodes"));
        assert_eq!(ax_nodes(br#"{"nodes":[{"nodeId":"1"}]}"#), bad("ignored"));
        assert_eq!(
            ax_nodes(br#"{"nodes":[{"nodeId":"1","ignored":false}]}"#),
            bad("role")
        );
        assert_eq!(
            ax_nodes(br#"{"nodes":[{"ignored":false,"role":{"value":"x"}}]}"#),
            bad("nodeId")
        );
        assert_eq!(
            ax_nodes(br#"{"nodes":[{"nodeId":"1","ignored":false,"role":{"value":"x"},"childIds":[2]}]}"#),
            bad("childIds")
        );
        assert_eq!(
            ax_nodes(br#"{"nodes":[{"nodeId":"1","ignored":false,"role":{"value":"x"},"backendDOMNodeId":-3}]}"#),
            bad("backendDOMNodeId")
        );
        assert_eq!(
            ax_nodes(
                br#"{"nodes":[{"nodeId":"1","ignored":false,"role":{"value":"x"},"name":"n"}]}"#
            ),
            bad("value")
        );
        assert_eq!(
            ax_nodes(br#"{"nodes":[{"nodeId":"1","ignored":false,"role":{"value":"x"},"properties":[{"value":{"value":1}}]}]}"#),
            bad("name")
        );
        // An ignored node needs no role, and a node no name.
        assert!(ax_nodes(br#"{"nodes":[{"nodeId":"1","ignored":true}]}"#).is_ok());
        let one = r#"{"nodeId":"1","ignored":true}"#;
        let too_many = format!(r#"{{"nodes":[{}]}}"#, vec![one; MOST_NODES + 1].join(","));
        assert_eq!(ax_nodes(too_many.as_bytes()), bad("nodes"));
    }

    #[test]
    fn chromes_version_is_read_as_chrome_155_wrote_it() {
        // Chrome 155.0.8059.86's own reply, over the pipe, on 2026-10-24.
        let result = br#"{"protocolVersion":"1.3","product":"Chrome/155.0.8059.86","revision":"@804d6b05f089b27af8342622a36558c49401a1d4","userAgent":"Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/155.0.0.0 Safari/537.36","jsVersion":"15.5.35.25"}"#;
        let version = version(result).unwrap();
        assert_eq!(version.product, "Chrome/155.0.8059.86");
        assert_eq!(version.protocol, "1.3");
        assert_eq!(
            version.revision,
            "@804d6b05f089b27af8342622a36558c49401a1d4"
        );
        assert!(version.user_agent.contains("HeadlessChrome/155"));
        assert_eq!(version.javascript, "15.5.35.25");
    }

    #[test]
    fn every_malformed_message_is_refused_as_a_kind() {
        assert_eq!(read(b"not json"), Err(Malformed::NotJson));
        assert_eq!(read(b""), Err(Malformed::NotJson));
        assert_eq!(read(b"\xff\xfe"), Err(Malformed::NotJson));
        assert_eq!(read(br#"{"id":1,"result":{}"#), Err(Malformed::NotJson));
        assert_eq!(read(b"[1]"), Err(Malformed::NotAnObject));
        assert_eq!(read(b"\"id\""), Err(Malformed::NotAnObject));
        assert_eq!(read(b"{}"), Err(Malformed::Neither));
        assert_eq!(read(br#"{"params":{}}"#), Err(Malformed::Neither));
        let bad = |field| Err(Malformed::BadField { field });
        assert_eq!(read(br#"{"id":-1,"result":{}}"#), bad("id"));
        assert_eq!(read(br#"{"id":1.5,"result":{}}"#), bad("id"));
        assert_eq!(read(br#"{"id":"1","result":{}}"#), bad("id"));
        assert_eq!(
            read(br#"{"id":1e999,"result":{}}"#),
            Err(Malformed::NotJson)
        );
        assert_eq!(read(br#"{"id":1}"#), bad("result"));
        assert_eq!(read(br#"{"id":1,"error":"no"}"#), bad("error"));
        assert_eq!(read(br#"{"id":1,"error":{"message":"no"}}"#), bad("code"));
        assert_eq!(read(br#"{"id":1,"error":{"code":1}}"#), bad("message"));
        assert_eq!(read(br#"{"method":5}"#), bad("method"));
        let long = "m".repeat(MOST_TEXT + 1);
        assert_eq!(
            read(format!(r#"{{"method":"{long}"}}"#).as_bytes()),
            Err(Malformed::TooLong {
                field: "method",
                most: MOST_TEXT
            })
        );
    }

    #[test]
    fn a_message_too_deep_is_refused_before_it_is_parsed() {
        let deep = format!(
            r#"{{"id":1,"result":{}{}}}"#,
            "[".repeat(MOST_NESTING),
            "]".repeat(MOST_NESTING)
        );
        assert_eq!(
            read(deep.as_bytes()),
            Err(Malformed::TooDeep { most: MOST_NESTING })
        );
        // Far deeper than the rented reader's own limit: refused here first.
        let deeper = "[".repeat(100_000);
        assert_eq!(
            read(deeper.as_bytes()),
            Err(Malformed::TooDeep { most: MOST_NESTING })
        );
    }

    #[test]
    fn a_version_that_is_not_one_is_refused_as_a_kind() {
        assert_eq!(version(b"[]"), Err(Malformed::NotAnObject));
        assert_eq!(
            version(br#"{"product":5}"#),
            Err(Malformed::BadField { field: "product" })
        );
        assert_eq!(
            version(
                br#"{"product":"Chrome/1","protocolVersion":"1.3","revision":"r","userAgent":"u"}"#
            ),
            Err(Malformed::BadField { field: "jsVersion" })
        );
    }

    #[test]
    fn hostile_messages_are_read_without_a_panic() {
        for message in [
            &b"{\"id\":18446744073709551616,\"result\":{}}"[..],
            b"{\"id\":1,\"result\":null}",
            b"{\"id\":1,\"error\":{\"code\":9223372036854775808,\"message\":\"\"}}",
            b"{\"id\":1,\"id\":2,\"result\":{}}",
            b"{\"method\":\"\\ud800\"}",
            b"\"\\\"[[[[[[[[[[[[[[[[[[[[[[",
            b"]]]]]]]]",
            b"{\"id\":1,\"result\":{\"x\":\"\\u0000\"}}",
        ] {
            if let Ok(Message::Reply {
                result: Ok(result), ..
            }) = read(message)
            {
                let _ = version(&result);
            }
        }
    }

    #[test]
    fn a_window_is_asked_for_and_placed_by_its_ids() {
        assert_eq!(
            written(1, None, &Command::Targets),
            br#"{"id":1,"method":"Target.getTargets","params":{}}"#
        );
        assert_eq!(
            written(
                2,
                None,
                &Command::WindowOf {
                    target: "T".to_owned()
                }
            ),
            br#"{"id":2,"method":"Browser.getWindowForTarget","params":{"targetId":"T"}}"#
        );
        assert_eq!(
            written(
                4,
                None,
                &Command::Activate {
                    target: "T".to_owned()
                }
            ),
            br#"{"id":4,"method":"Target.activateTarget","params":{"targetId":"T"}}"#
        );
        let bounds = Bounds {
            left: -45,
            top: 34,
            width: 1200,
            height: 832,
        };
        assert_eq!(
            written(3, None, &Command::PlaceWindow { window: 1_852_001_420, bounds }),
            br#"{"id":3,"method":"Browser.setWindowBounds","params":{"bounds":{"height":832,"left":-45,"top":34,"width":1200},"windowId":1852001420}}"#
        );
    }

    #[test]
    fn only_a_page_nothing_is_attached_to_is_a_shown_chromes_window() {
        // As a shown Chrome 155 lists its targets as it starts, cut short.
        let listed = br#"{"targetInfos":[
            {"targetId":"W","type":"service_worker","attached":false},
            {"targetId":"U","type":"browser_ui","attached":false},
            {"targetId":"P","type":"page","attached":false},
            {"targetId":"A","type":"page","attached":true}]}"#;
        assert_eq!(pages(listed), Ok(vec!["P".to_owned()]));
        assert_eq!(pages(br#"{"targetInfos":[]}"#), Ok(Vec::new()));
        assert_eq!(pages(br"{}"), bad("targetInfos"));
        assert_eq!(pages(br#"{"targetInfos":[1]}"#), bad("targetInfos"));
        assert_eq!(
            pages(br#"{"targetInfos":[{"targetId":"P","type":"page"}]}"#),
            bad("attached")
        );
        assert_eq!(
            pages(br#"{"targetInfos":[{"type":"page","attached":false}]}"#),
            bad("targetId")
        );
        let many = format!(
            r#"{{"targetInfos":[{}]}}"#,
            vec![r#"{"targetId":"P","type":"page","attached":true}"#; MOST_TARGETS + 1].join(",")
        );
        assert_eq!(pages(many.as_bytes()), bad("targetInfos"));
    }

    #[test]
    fn a_target_is_held_while_it_is_listed_whatever_its_kind() {
        let listed = br#"{"targetInfos":[
            {"targetId":"U","type":"browser_ui","attached":false},
            {"targetId":"A","type":"page","attached":true}]}"#;
        assert_eq!(holds(listed, "A"), Ok(true), "a tab attached to");
        assert_eq!(holds(listed, "U"), Ok(true));
        assert_eq!(holds(listed, "P"), Ok(false), "a tab closed");
        assert_eq!(holds(br#"{"targetInfos":[]}"#, "A"), Ok(false));
        assert_eq!(holds(br"{}", "A"), bad("targetInfos"));
        assert_eq!(
            holds(br#"{"targetInfos":[{"targetId":"A","type":"page"}]}"#, "A"),
            bad("attached"),
            "refused whole, not believed for the one asked about"
        );
    }

    #[test]
    fn a_target_is_named_by_its_title_and_address_as_chrome_gives_them() {
        let listed = br#"{"targetInfos":[
            {"targetId":"U","type":"browser_ui","attached":false},
            {"targetId":"A","type":"page","attached":true,
             "title":"Invoices </title><b>","url":"https://example.com/a"}]}"#;
        assert_eq!(
            named(listed, "A"),
            Ok(Some(Named {
                title: "Invoices </title><b>".to_owned(),
                address: "https://example.com/a".to_owned(),
            })),
            "a title is text, whatever it holds"
        );
        assert_eq!(named(listed, "P"), Ok(None), "a tab closed");
        assert_eq!(
            named(listed, "U"),
            bad("title"),
            "the one asked about has no title"
        );
        let no_url =
            br#"{"targetInfos":[{"targetId":"A","type":"page","attached":true,"title":"T"}]}"#;
        assert_eq!(named(no_url, "A"), bad("url"));
        let a_number = br#"{"targetInfos":[{"targetId":"A","type":"page","attached":true,"title":7,"url":""}]}"#;
        assert_eq!(named(a_number, "A"), bad("title"));
        assert_eq!(
            named(
                br#"{"targetInfos":[{"targetId":"A","type":"page","attached":true,"title":"T","url":""},{"targetId":"B"}]}"#,
                "A"
            ),
            bad("type"),
            "refused whole, not believed for the one asked about"
        );
    }

    #[test]
    fn a_title_longer_than_a_strip_draws_is_kept_to_it_at_a_character() {
        use alo_renderer::strip::LONGEST_TITLE;
        let long = "é".repeat(LONGEST_TITLE + 5);
        let listed = format!(
            r#"{{"targetInfos":[{{"targetId":"A","type":"page","attached":true,"title":"{long}","url":"{long}"}}]}}"#
        );
        let Ok(Some(read)) = named(listed.as_bytes(), "A") else {
            panic!("not named");
        };
        assert_eq!(read.title, "é".repeat(LONGEST_TITLE));
        assert_eq!(read.address, "é".repeat(LONGEST_TITLE));
    }

    #[test]
    fn a_window_is_read_with_where_it_is_and_whether_it_is_shown() {
        assert_eq!(
            window_of(
                br#"{"windowId":1852001420,"bounds":{"left":0,"top":34,"width":1200,"height":800,"windowState":"normal"}}"#
            ),
            Ok(ChromeWindow {
                id: 1_852_001_420,
                bounds: Bounds {
                    left: 0,
                    top: 34,
                    width: 1200,
                    height: 800
                },
                state: WindowState::Normal,
            })
        );
        let state = |state: &str| {
            window_of(
                format!(
                    r#"{{"windowId":1,"bounds":{{"left":-45,"top":0,"width":0,"height":0,"windowState":"{state}"}}}}"#
                )
                .as_bytes(),
            )
            .map(|window| window.state)
        };
        assert_eq!(state("minimized"), Ok(WindowState::Minimized));
        assert_eq!(state("maximized"), Ok(WindowState::Maximized));
        assert_eq!(state("fullscreen"), Ok(WindowState::Fullscreen));
        assert_eq!(state("docked"), bad("windowState"));
        assert_eq!(window_of(br#"{"bounds":{}}"#), bad("windowId"));
        assert_eq!(
            window_of(br#"{"windowId":1.5,"bounds":{}}"#),
            bad("windowId")
        );
        assert_eq!(window_of(br#"{"windowId":1}"#), bad("bounds"));
        assert_eq!(
            window_of(
                br#"{"windowId":1,"bounds":{"left":0,"top":0,"width":-1,"height":0,"windowState":"normal"}}"#
            ),
            bad("bounds")
        );
        assert_eq!(
            window_of(
                br#"{"windowId":1,"bounds":{"left":0.5,"top":0,"width":1,"height":1,"windowState":"normal"}}"#
            ),
            bad("left")
        );
        assert_eq!(
            window_of(
                br#"{"windowId":1,"bounds":{"left":0,"top":0,"width":1,"height":1e400,"windowState":"normal"}}"#
            ),
            Err(Malformed::NotJson)
        );
    }

    #[test]
    fn a_skipped_message_is_named_by_what_it_began_with() {
        assert_eq!(
            skipped(br#"{"id":42,"result":{"nodes":[{"#),
            Message::Skipped {
                id: Some(42),
                method: None
            }
        );
        assert_eq!(
            skipped(br#"{"method":"Runtime.consoleAPICalled","params":{"type""#),
            Message::Skipped {
                id: None,
                method: Some("Runtime.consoleAPICalled".to_owned())
            }
        );
        // Cut short, in another order, or not a number or a name: nothing.
        for head in [
            &br#"{"id":42"#[..],
            br#"{"id":,"#,
            br#"{"id":99999999999999999999999,"#,
            br#"{"method":"Runtime.consoleAPI"#,
            br#"{"method":"a\"b","#,
            br#"{"params":{},"method":"x","#,
            b"",
            b"\xff\xfe",
        ] {
            assert_eq!(
                skipped(head),
                Message::Skipped {
                    id: None,
                    method: None
                },
                "{head:?}"
            );
        }
    }

    #[test]
    fn the_runtime_and_the_log_are_asked_for_by_their_methods() {
        for (command, method) in [
            (Command::ForgetWhatWasSaid, "Runtime.discardConsoleEntries"),
            (Command::TellOfScript, "Runtime.enable"),
            (Command::StopTellingOfScript, "Runtime.disable"),
            (Command::ForgetTheLog, "Log.clear"),
            (Command::TellOfTheLog, "Log.enable"),
            (Command::StopTellingOfTheLog, "Log.disable"),
        ] {
            assert_eq!(
                written(3, Some("S"), &command),
                format!(r#"{{"id":3,"method":"{method}","params":{{}},"sessionId":"S"}}"#)
                    .into_bytes()
            );
        }
    }

    #[test]
    fn what_chrome_logged_is_read_with_the_nodes_it_names() {
        // As Chrome 155 logged a hidden `required` field on 2026-10-25.
        let entry = br#"{"entry":{"source":"rendering","level":"error","text":"An invalid form control with name='a' is not focusable.","timestamp":1.5,"url":"data:,","args":[{"type":"object","subtype":"node","className":"HTMLInputElement","description":"input","objectId":"-79.2.1"},{"type":"string","value":"x"}]}}"#;
        assert_eq!(
            logged(entry),
            Ok(Logged {
                source: "rendering".to_owned(),
                level: "error".to_owned(),
                text: "An invalid form control with name='a' is not focusable.".to_owned(),
                nodes: vec!["-79.2.1".to_owned()],
            })
        );
        // Most of what Chrome logs names no node.
        let plain = logged(br#"{"entry":{"source":"network","level":"error","text":"Failed"}}"#);
        assert_eq!(plain.map(|one| one.nodes), Ok(Vec::new()));
        for (params, field) in [
            (&b"{}"[..], "entry"),
            (br#"{"entry":[]}"#, "entry"),
            (br#"{"entry":{"level":"error","text":"t"}}"#, "source"),
            (br#"{"entry":{"source":"s","text":"t"}}"#, "level"),
            (br#"{"entry":{"source":"s","level":"l","text":7}}"#, "text"),
            (
                br#"{"entry":{"source":"s","level":"l","text":"t","args":{}}}"#,
                "args",
            ),
            (
                br#"{"entry":{"source":"s","level":"l","text":"t","args":[{"subtype":"node"}]}}"#,
                "objectId",
            ),
        ] {
            assert_eq!(logged(params), bad(field), "{params:?}");
        }
        assert_eq!(logged(b"{"), Err(Malformed::NotJson));
    }

    #[test]
    fn a_value_is_described_by_its_object_and_read_as_a_node() {
        assert_eq!(
            written(
                5,
                Some("S"),
                &Command::DescribeValue {
                    object: "-79.2.1".to_owned()
                }
            ),
            br#"{"id":5,"method":"DOM.describeNode","params":{"objectId":"-79.2.1"},"sessionId":"S"}"#
        );
        assert_eq!(
            backend_node(br#"{"node":{"nodeId":0,"backendNodeId":6,"nodeName":"INPUT"}}"#),
            Ok(6)
        );
        assert_eq!(
            backend_node(br#"{"node":{"nodeId":0}}"#),
            bad("backendNodeId")
        );
        assert_eq!(
            backend_node(br#"{"node":{"backendNodeId":-1}}"#),
            bad("backendNodeId")
        );
        assert_eq!(backend_node(b"{}"), bad("node"));
    }

    #[test]
    fn a_value_said_is_read_by_its_type() {
        let one = |value: &str| {
            console_called(format!(r#"{{"type":"log","args":[{value}]}}"#).as_bytes()).map(|said| {
                match said {
                    Said::Console { args, .. } => args,
                    Said::Thrown { .. } => Vec::new(),
                }
            })
        };
        let words = |kept: &str| Words {
            kept: kept.to_owned(),
            more: 0,
        };
        for (value, shown) in [
            (r#"{"type":"undefined"}"#, Shown::Undefined),
            (
                r#"{"type":"object","subtype":"null","value":null}"#,
                Shown::Null,
            ),
            (r#"{"type":"boolean","value":true}"#, Shown::Bool(true)),
            (
                r#"{"type":"number","unserializableValue":"-0","description":"-0"}"#,
                Shown::Number(words("-0")),
            ),
            (
                r#"{"type":"bigint","unserializableValue":"5n","description":"5n"}"#,
                Shown::BigInt(words("5n")),
            ),
            (
                r#"{"type":"string","value":"hi"}"#,
                Shown::Text(words("hi")),
            ),
            (
                r#"{"type":"symbol","description":"Symbol(x)","objectId":"1"}"#,
                Shown::Symbol(words("Symbol(x)")),
            ),
            (
                r#"{"type":"object","subtype":"error","className":"DOMException","description":"NotFoundError: gone\n    at x"}"#,
                Shown::Error(words("NotFoundError: gone")),
            ),
            (
                r#"{"type":"function","className":"Function","description":"() => 1"}"#,
                Shown::Object(words("() => 1")),
            ),
            (
                r#"{"type":"object","subtype":"node","className":"HTMLBodyElement","description":"body"}"#,
                Shown::Object(words("body")),
            ),
            (
                r#"{"type":"object","className":"Object"}"#,
                Shown::Object(words("Object")),
            ),
        ] {
            assert_eq!(one(value), Ok(vec![shown]), "{value}");
        }
    }

    #[test]
    fn what_a_page_said_is_cut_and_counted_never_held_whole() {
        let long = "é".repeat(LONGEST_SHOWN + 5);
        let many = vec![r#"{"type":"undefined"}"#; MOST_ARGS + 3].join(",");
        let frames =
            vec![r#"{"url":"u","lineNumber":0,"columnNumber":0}"#; MOST_PLACES + 2].join(",");
        let said = console_called(
            format!(
                r#"{{"type":"log","args":[{{"type":"string","value":"{long}"}},{many}],
                    "stackTrace":{{"callFrames":[{frames}]}}}}"#
            )
            .as_bytes(),
        )
        .unwrap();
        let Said::Console {
            args,
            more_args,
            at,
            ..
        } = said
        else {
            panic!("not a console call");
        };
        assert_eq!(args.len(), MOST_ARGS);
        assert_eq!(more_args, 4);
        assert_eq!(
            args.first(),
            Some(&Shown::Text(Words {
                kept: "é".repeat(LONGEST_SHOWN),
                more: 5
            }))
        );
        assert_eq!(at.places.len(), MOST_PLACES);
        assert_eq!(at.left_out, 2);
        assert_eq!(
            at.places.first(),
            Some(&Place {
                url: Words {
                    kept: "u".to_owned(),
                    more: 0
                },
                line: 1,
                column: 1
            }),
            "counted from one"
        );
    }

    #[test]
    fn what_chrome_says_of_script_is_refused_as_a_kind_when_it_is_not_the_protocol() {
        for params in [
            &b""[..],
            b"[]",
            b"{",
            br#"{"type":"log"}"#,
            br#"{"type":"log","args":{}}"#,
            br#"{"type":4,"args":[]}"#,
            br#"{"type":"log","args":[4]}"#,
            br#"{"type":"log","args":[{"type":"alien"}]}"#,
            br#"{"type":"log","args":[{"type":"boolean","value":"yes"}]}"#,
            br#"{"type":"log","args":[{"type":"number"}]}"#,
            br#"{"type":"log","args":[{"type":"string"}]}"#,
            br#"{"type":"log","args":[{"type":"object","subtype":"error"}]}"#,
            br#"{"type":"log","args":[{"type":"object"}]}"#,
            br#"{"type":"log","args":[],"stackTrace":{}}"#,
            br#"{"type":"log","args":[],"stackTrace":{"callFrames":[1]}}"#,
            br#"{"type":"log","args":[],"stackTrace":{"callFrames":[{"url":"u"}]}}"#,
            br#"{"type":"log","args":[],"stackTrace":{"callFrames":[{"url":"u","lineNumber":-1,"columnNumber":0}]}}"#,
            br#"{"type":"log","args":[[[[[[[[[[[[[[[[[[[[1]]]]]]]]]]]]]]]]]]]]}"#,
        ] {
            assert!(console_called(params).is_err(), "{params:?}");
        }
        for params in [
            &b"{}"[..],
            br#"{"exceptionDetails":[]}"#,
            br#"{"exceptionDetails":{}}"#,
            br#"{"exceptionDetails":{"text":"Uncaught","url":4,"lineNumber":0,"columnNumber":0}}"#,
            br#"{"exceptionDetails":{"text":"Uncaught","url":"u"}}"#,
            br#"{"exceptionDetails":{"text":"Uncaught","exception":"x"}}"#,
            br#"{"exceptionDetails":{"text":"Uncaught","stackTrace":[]}}"#,
        ] {
            assert!(thrown(params).is_err(), "{params:?}");
        }
        // A throw with no place and no value is still a throw.
        assert_eq!(
            thrown(br#"{"exceptionDetails":{"text":"Uncaught"}}"#),
            Ok(Said::Thrown {
                text: "Uncaught".to_owned(),
                what: None,
                at: Places::default()
            })
        );
    }
}
