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
/// browser sends today nests two deep; an accessibility tree's nodes (item
/// 620) are an array of objects holding objects, under six.
pub const MOST_NESTING: usize = 16;

/// The longest text read out of a message — a method's name, a version, a
/// reason Chrome gave — in characters.
pub const MOST_TEXT: usize = 4096;

/// A command this browser sends Chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// `Browser.getVersion`: which Chrome this is.
    GetVersion,
    /// `Browser.close`: close every window and end.
    Close,
}

impl Command {
    /// The method Chrome knows it by.
    pub fn method(self) -> &'static str {
        match self {
            Self::GetVersion => "Browser.getVersion",
            Self::Close => "Browser.close",
        }
    }
}

/// `command`, written as the message that asks for it under `id`.
pub fn written(id: u64, command: Command) -> Vec<u8> {
    let mut fields = Map::new();
    fields.insert("id".to_owned(), Value::from(id));
    fields.insert("method".to_owned(), Value::from(command.method()));
    fields.insert("params".to_owned(), Value::Object(Map::new()));
    Value::Object(fields).to_string().into_bytes()
}

/// A message from Chrome, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// The answer to the command sent under `id`.
    Reply {
        /// The command's own `id`.
        id: u64,
        /// Its `result`, written again as JSON for whoever asked to read
        /// ([`version`]), or the reason Chrome gave for not carrying it out.
        result: Result<Vec<u8>, Declined>,
    },
    /// Something Chrome tells of without being asked.
    Event {
        /// What it is, by the protocol's name for it.
        method: String,
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
    match (fields.get("id"), fields.get("method")) {
        (Some(id), _) => {
            let id = id.as_u64().ok_or(Malformed::BadField { field: "id" })?;
            let result = match (fields.get("result"), fields.get("error")) {
                (_, Some(error)) => Err(declined(error)?),
                (Some(result), None) => Ok(result.to_string().into_bytes()),
                (None, None) => return Err(Malformed::BadField { field: "result" }),
            };
            Ok(Message::Reply { id, result })
        }
        (None, Some(method)) => Ok(Message::Event {
            method: text(Some(method), "method")?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_written_as_chrome_reads_it() {
        assert_eq!(
            written(1, Command::GetVersion),
            br#"{"id":1,"method":"Browser.getVersion","params":{}}"#
        );
        assert_eq!(
            written(u64::MAX, Command::Close),
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
                method: "Target.targetCreated".to_owned()
            })
        );
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
