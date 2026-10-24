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

/// A command this browser sends Chrome.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// `Page.setLifecycleEventsEnabled`: be told of each stage of every
    /// load, by the loader it belongs to.
    TellOfStages,
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
            Self::TellOfStages => "Page.setLifecycleEventsEnabled",
            Self::LayOutAt { .. } => "Emulation.setDeviceMetricsOverride",
            Self::Navigate { .. } => "Page.navigate",
            Self::Fulfil { .. } => "Fetch.fulfillRequest",
            Self::Refuse { .. } => "Fetch.failRequest",
            Self::ReadTree => "Accessibility.getFullAXTree",
            Self::WhereDrawn { .. } => "DOM.getContentQuads",
            Self::Window => "Page.getLayoutMetrics",
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
            | Self::ReadTree
            | Self::Window => {}
            Self::CreateTarget { url } | Self::Navigate { url } => {
                put("url", Value::from(url.as_str()));
            }
            Self::AttachToTarget { target } => {
                put("targetId", Value::from(target.as_str()));
                put("flatten", Value::Bool(true));
            }
            Self::CloseTarget { target } => put("targetId", Value::from(target.as_str())),
            Self::TellOfStages => put("enabled", Value::Bool(true)),
            Self::PauseEveryRequest => {
                let mut every = Map::new();
                every.insert("urlPattern".to_owned(), Value::from("*"));
                put("patterns", Value::Array(vec![Value::Object(every)]));
            }
            Self::LayOutAt { width, height } => {
                put("width", Value::from(*width));
                put("height", Value::from(*height));
                // Nought is Chrome's word for "the screen's own".
                put("deviceScaleFactor", Value::from(0));
                put("mobile", Value::Bool(false));
            }
            Self::Fulfil { request, body } => {
                put("requestId", Value::from(request.as_str()));
                put("responseCode", Value::from(200));
                let mut header = Map::new();
                header.insert("name".to_owned(), Value::from("Content-Type"));
                header.insert("value".to_owned(), Value::from("text/html; charset=utf-8"));
                put("responseHeaders", Value::Array(vec![Value::Object(header)]));
                put("body", Value::from(base64(body)));
            }
            Self::Refuse { request } => {
                put("requestId", Value::from(request.as_str()));
                put("errorReason", Value::from("BlockedByClient"));
            }
            Self::WhereDrawn { node } => put("backendNodeId", Value::from(*node)),
        }
        params
    }
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
}

/// A `Fetch.requestPaused` event's `params`, read.
///
/// # Errors
///
/// [`Malformed`], for params with no `requestId` or no `request.url`.
pub fn paused(params: &[u8]) -> Result<Paused, Malformed> {
    let fields = object(params)?;
    let Some(Value::Object(request)) = fields.get("request") else {
        return Err(Malformed::BadField { field: "request" });
    };
    Ok(Paused {
        request: text(fields.get("requestId"), "requestId")?,
        url: words(request.get("url"), "url")?,
    })
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
            written(6, Some("S"), &Command::Refuse { request: "R".to_owned() }),
            br#"{"id":6,"method":"Fetch.failRequest","params":{"errorReason":"BlockedByClient","requestId":"R"},"sessionId":"S"}"#
        );
        assert_eq!(
            written(7, Some("S"), &Command::WhereDrawn { node: 62 }),
            br#"{"id":7,"method":"DOM.getContentQuads","params":{"backendNodeId":62},"sessionId":"S"}"#
        );
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
                url: "https://httpbin.org/favicon.ico".to_owned()
            })
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
}
