/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! The agent's protocol on the wire: one JSON object per line (ADR 0055 § 3).
//!
//! One of the two files that name `serde_json`, which is rented — the other
//! is [`crate::chrome_protocol`], Chrome's. What a line means
//! is [`crate::asking`]'s; this reads a line into a [`Request`] and writes an
//! [`Answer`] into a line, and the contract a caller builds against is
//! `docs/contracts/agent-socket.md`.
//!
//! # Bounded in front of the rented reader
//!
//! A line's length is the socket's to refuse ([`crate::agents`]). Its nesting
//! is refused here, by a scan of the bytes **before** they are parsed, so the
//! reader is never handed a line deeper than [`MOST_NESTING`] — its own limit
//! is somebody else's choice. Every field's length is checked after.
//!
//! # The same answer is the same bytes
//!
//! An object's keys are written in order, so a caller — or a test — can
//! compare an answer to the one it expected without parsing it.

use crate::asking::{
    Answer, Ask, Echo, MOST_ADDRESS, MOST_NAME, MOST_NESTING, Refused, Request, VERSION,
    outcome_kind,
};
use crate::json_depth;
use crate::pages::RenderedBy;
use alo_agent::verb::Invalid as Unmet;
use alo_agent::{Outcome, Refusal, ScrollBy, Target, Verb};
use alo_box::{BoxId, KnownRole, Role, States};
use alo_layout::Rect;
use alo_renderer::SnapshotNode;
use serde_json::{Map, Value};

/// Every field a request may carry, in any version this browser speaks.
const FIELDS: [&str; 8] = ["v", "id", "ask", "tab", "target", "verb", "text", "scroll"];

/// Read one line — without its newline — into a request.
pub fn read(line: &[u8]) -> Request {
    let refused = |refused: Refused| Request {
        echo: None,
        ask: Err(refused),
    };
    if json_depth::of(line) > MOST_NESTING {
        return refused(Refused::TooDeep { most: MOST_NESTING });
    }
    let Ok(value) = serde_json::from_slice::<Value>(line) else {
        return refused(Refused::NotJson);
    };
    let Value::Object(fields) = value else {
        return refused(Refused::NotAnObject);
    };
    let echo = match fields.get("id").map(echo) {
        None => None,
        Some(Ok(echo)) => Some(echo),
        Some(Err(why)) => return refused(why),
    };
    Request {
        echo,
        ask: ask(&fields),
    }
}

/// What a caller asked to have handed back.
fn echo(value: &Value) -> Result<Echo, Refused> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .map(Echo::Number)
            .ok_or(Refused::BadField { field: "id" }),
        Value::String(text) => Ok(Echo::Text(bounded(text, "id", MOST_NAME)?.to_owned())),
        _ => Err(Refused::BadField { field: "id" }),
    }
}

/// `text`, if it is at most `most` characters.
fn bounded<'a>(text: &'a str, field: &'static str, most: usize) -> Result<&'a str, Refused> {
    if text.chars().count() > most {
        return Err(Refused::FieldTooLong { field, most });
    }
    Ok(text)
}

/// The text in `field`, which must be there.
fn text<'a>(
    fields: &'a Map<String, Value>,
    field: &'static str,
    most: usize,
) -> Result<&'a str, Refused> {
    match fields.get(field) {
        None => Err(Refused::MissingField { field }),
        Some(Value::String(text)) => bounded(text, field, most),
        Some(_) => Err(Refused::BadField { field }),
    }
}

/// What the request asks, from its fields.
fn ask(fields: &Map<String, Value>) -> Result<Ask, Refused> {
    if let Some(unknown) = fields.keys().find(|key| !FIELDS.contains(&key.as_str())) {
        return Err(Refused::UnknownField {
            field: unknown.clone(),
        });
    }
    match fields.get("v") {
        Some(Value::Number(number)) if number.as_u64() == Some(VERSION) => {}
        _ => return Err(Refused::UnknownVersion { speaks: VERSION }),
    }
    let asked = match text(fields, "ask", MOST_NAME)? {
        "tabs" => Ask::Tabs,
        "read" => Ask::Read {
            tab: text(fields, "tab", MOST_ADDRESS)?.to_owned(),
        },
        "renderer" => Ask::Renderer {
            tab: text(fields, "tab", MOST_ADDRESS)?.to_owned(),
        },
        "act" => {
            let tab = text(fields, "tab", MOST_ADDRESS)?.to_owned();
            let target = match fields.get("target") {
                None => return Err(Refused::MissingField { field: "target" }),
                Some(Value::Object(target)) => self::target(target)?,
                Some(_) => return Err(Refused::BadField { field: "target" }),
            };
            Ask::Act {
                tab,
                target,
                verb: verb(fields)?,
            }
        }
        _ => return Err(Refused::BadField { field: "ask" }),
    };
    // A field that belongs to another ask is refused as not this one's, so
    // that a verb sent with a `read` is said rather than ignored.
    let takes: &[&str] = match &asked {
        Ask::Tabs => &["v", "id", "ask"],
        Ask::Read { .. } | Ask::Renderer { .. } => &["v", "id", "ask", "tab"],
        Ask::Act { verb, .. } => match verb {
            Verb::Activate => &["v", "id", "ask", "tab", "target", "verb"],
            Verb::PutText(_) => &["v", "id", "ask", "tab", "target", "verb", "text"],
            Verb::Scroll(_) => &["v", "id", "ask", "tab", "target", "verb", "scroll"],
        },
    };
    if let Some(stray) = fields.keys().find(|key| !takes.contains(&key.as_str())) {
        return Err(Refused::UnknownField {
            field: stray.clone(),
        });
    }
    Ok(asked)
}

/// A target: a name, a role, both, or a node an earlier read named.
fn target(fields: &Map<String, Value>) -> Result<Target, Refused> {
    let bad = Refused::BadField { field: "target" };
    if fields
        .keys()
        .any(|key| !matches!(key.as_str(), "name" | "role" | "node"))
    {
        return Err(bad);
    }
    let name = match fields.get("name") {
        None => None,
        Some(Value::String(name)) => Some(bounded(name, "name", MOST_NAME)?.to_owned()),
        Some(_) => return Err(bad),
    };
    let role = match fields.get("role") {
        None => None,
        Some(Value::String(role)) => Some(role_named(bounded(role, "role", MOST_NAME)?)),
        Some(_) => return Err(bad),
    };
    match (fields.get("node"), name, role) {
        (Some(node), None, None) => node
            .as_u64()
            .and_then(|node| usize::try_from(node).ok())
            // A claim, as an id off any wire is: it means something only
            // against the tree the caller read it from, and a verb aimed at
            // one that is not there is refused as not found.
            .map(|node| Target::Node(BoxId::from_wire(node)))
            .ok_or(bad),
        (None, Some(name), None) => Ok(Target::Named(name)),
        (None, None, Some(role)) => Ok(Target::OfRole(role)),
        (None, Some(name), Some(role)) => Ok(Target::NamedOfRole { role, name }),
        _ => Err(bad),
    }
}

/// The role with ARIA's name `name`, as a tree reads it back.
fn role_named(name: &str) -> Role {
    match name {
        "generic" => Role::Generic,
        "presentation" => Role::Presentational,
        _ => KnownRole::named(name).map_or_else(|| Role::Declared(name.into()), Role::Known),
    }
}

/// The verb, and what it carries.
fn verb(fields: &Map<String, Value>) -> Result<Verb, Refused> {
    match text(fields, "verb", MOST_NAME)? {
        "activate" => Ok(Verb::Activate),
        "put_text" => match fields.get("text") {
            None => Err(Refused::MissingField { field: "text" }),
            // Bounded by the line, which is the most text a request can carry.
            Some(Value::String(text)) => Ok(Verb::PutText(text.clone())),
            Some(_) => Err(Refused::BadField { field: "text" }),
        },
        "scroll" => match fields.get("scroll") {
            None => Err(Refused::MissingField { field: "scroll" }),
            Some(Value::String(to)) if to == "start" => Ok(Verb::Scroll(ScrollBy::ToStart)),
            Some(Value::String(to)) if to == "end" => Ok(Verb::Scroll(ScrollBy::ToEnd)),
            Some(Value::Number(pixels)) => pixels
                .as_f64()
                .map(pixels_of)
                .filter(|pixels| pixels.is_finite())
                .map(|pixels| Verb::Scroll(ScrollBy::Pixels(pixels)))
                .ok_or(Refused::BadField { field: "scroll" }),
            Some(_) => Err(Refused::BadField { field: "scroll" }),
        },
        _ => Err(Refused::BadField { field: "verb" }),
    }
}

/// A distance in pixels, as layout holds one. A number too large for that is
/// infinite, and refused by the caller.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a pixel is an f32 in layout, and one out of its range becomes infinite and is refused"
)]
fn pixels_of(number: f64) -> f32 {
    number as f32
}

/// Write `answer` as one line, its newline included, handing back `echo`.
pub fn write(echo: Option<&Echo>, answer: &Answer) -> String {
    let mut line = Map::new();
    line.insert("v".to_owned(), Value::from(VERSION));
    if let Some(echo) = echo {
        let echoed = match echo {
            Echo::Number(number) => Value::from(*number),
            Echo::Text(text) => Value::from(text.as_str()),
        };
        line.insert("id".to_owned(), echoed);
    }
    match answer {
        Answer::Tabs(tabs) => {
            line.insert("answer".to_owned(), Value::from("tabs"));
            line.insert("tabs".to_owned(), Value::from(tabs.clone()));
        }
        Answer::Tree { tab, snapshot } => {
            line.insert("answer".to_owned(), Value::from("tree"));
            line.insert("tab".to_owned(), Value::from(tab.as_str()));
            line.insert(
                "tree".to_owned(),
                snapshot.root.as_ref().map_or(Value::Null, node),
            );
        }
        Answer::Renderer { tab, rendered_by } => {
            line.insert("answer".to_owned(), Value::from("renderer"));
            line.insert("tab".to_owned(), Value::from(tab.as_str()));
            line.insert("renderer".to_owned(), renderer(rendered_by));
        }
        Answer::Done {
            tab,
            outcome,
            issues,
        } => {
            line.insert("answer".to_owned(), Value::from("done"));
            line.insert("tab".to_owned(), Value::from(tab.as_str()));
            line.insert("outcome".to_owned(), self::outcome(outcome));
            line.insert("issues".to_owned(), Value::from(issues.clone()));
        }
        Answer::Refused(refused) => {
            line.insert("answer".to_owned(), Value::from("refused"));
            line.insert("refusal".to_owned(), self::refused(refused));
        }
    }
    if let Some(said) = answer.said() {
        line.insert("said".to_owned(), Value::from(said));
    }
    let mut written = Value::Object(line).to_string();
    written.push('\n');
    written
}

/// A renderer, as a kind and, for Chrome, the product and version it gives
/// itself.
fn renderer(rendered_by: &RenderedBy) -> Value {
    let mut fields = Map::new();
    match rendered_by {
        RenderedBy::ThisEngine => {
            fields.insert("kind".to_owned(), Value::from("ThisEngine"));
        }
        RenderedBy::Chrome { product } => {
            fields.insert("kind".to_owned(), Value::from("Chrome"));
            fields.insert("product".to_owned(), Value::from(product.as_str()));
        }
    }
    Value::Object(fields)
}

/// A node of the tree, and everything under it.
fn node(node: &SnapshotNode) -> Value {
    let mut fields = Map::new();
    fields.insert("node".to_owned(), id(node.id));
    fields.insert("role".to_owned(), Value::from(node.role.as_str()));
    if let Some(name) = &node.name {
        fields.insert("name".to_owned(), Value::from(name.as_str()));
    }
    let states = states(&node.states);
    if !states.is_empty() {
        fields.insert("states".to_owned(), Value::Object(states));
    }
    if let Some(placeholder) = &node.placeholder {
        fields.insert("placeholder".to_owned(), Value::from(placeholder.as_str()));
    }
    if let Some(marker) = &node.marker {
        fields.insert("marker".to_owned(), Value::from(marker.as_str()));
    }
    fields.insert("rect".to_owned(), rect(node.rect));
    if node.rects.len() > 1 {
        let pieces = node.rects.iter().copied().map(rect).collect();
        fields.insert("pieces".to_owned(), Value::Array(pieces));
    }
    // A list's option is a node and never a place on the screen (ADR 0064
    // § 5): its `rect` is there, as version 1 promises, and means nothing.
    if node.rects.is_empty() {
        fields.insert("undrawn".to_owned(), Value::Bool(true));
    }
    if node.offscreen {
        fields.insert("offscreen".to_owned(), Value::Bool(true));
    }
    if node.scrolls {
        fields.insert("scrolls".to_owned(), Value::Bool(true));
    }
    let children = node.children.iter().map(self::node).collect();
    fields.insert("children".to_owned(), Value::Array(children));
    Value::Object(fields)
}

/// A box's id, as a number.
fn id(id: BoxId) -> Value {
    Value::from(id.as_usize())
}

/// Where a box was drawn, in CSS pixels. A rectangle is read, never acted on
/// (ADR 0002): no verb takes one back.
fn rect(rect: Rect) -> Value {
    let mut fields = Map::new();
    for (name, length) in [
        ("x", rect.left()),
        ("y", rect.top()),
        ("width", rect.size.width),
        ("height", rect.size.height),
    ] {
        fields.insert(name.to_owned(), self::length(length));
    }
    Value::Object(fields)
}

/// A length layout holds, written as the shortest number that reads back as
/// the same one: `23.2` rather than the `23.200000762939453` its widening
/// would write. Infinite or not a number is `null`, which JSON has for it.
fn length(length: f32) -> Value {
    length
        .to_string()
        .parse::<f64>()
        .map_or(Value::Null, Value::from)
}

/// The states a box is in that are not its defaults, by ARIA's names where
/// it has them.
fn states(states: &States) -> Map<String, Value> {
    let mut fields = Map::new();
    for (name, on) in [
        ("disabled", states.disabled),
        ("required", states.required),
        ("readonly", states.read_only),
        ("busy", states.busy),
        ("hidden", states.hidden),
        ("takes_text", states.takes_text),
        ("broken", states.broken),
    ] {
        if on {
            fields.insert(name.to_owned(), Value::Bool(true));
        }
    }
    for (name, flag) in [
        ("selected", states.selected),
        ("expanded", states.expanded),
        ("pressed", states.pressed),
    ] {
        if let Some(flag) = flag {
            fields.insert(name.to_owned(), Value::Bool(flag));
        }
    }
    if let Some(checked) = states.checked {
        fields.insert("checked".to_owned(), Value::from(checked.to_string()));
    }
    if let Some(invalid) = states.invalid {
        let said = match invalid {
            alo_box::Invalid::Yes => "true",
            alo_box::Invalid::Unknown => "unknown",
        };
        fields.insert("invalid".to_owned(), Value::from(said));
    }
    if let Some(level) = states.level {
        fields.insert("level".to_owned(), Value::from(level));
    }
    if let Some(current) = states.current {
        fields.insert("current".to_owned(), Value::from(current.to_string()));
    }
    fields
}

/// A target, as a request names one.
fn target_value(target: &Target) -> Value {
    let mut fields = Map::new();
    match target {
        Target::Named(name) => {
            fields.insert("name".to_owned(), Value::from(name.as_str()));
        }
        Target::OfRole(role) => {
            fields.insert("role".to_owned(), Value::from(role.as_str()));
        }
        Target::NamedOfRole { role, name } => {
            fields.insert("role".to_owned(), Value::from(role.as_str()));
            fields.insert("name".to_owned(), Value::from(name.as_str()));
        }
        Target::Node(node) => {
            fields.insert("node".to_owned(), id(*node));
        }
    }
    Value::Object(fields)
}

/// How far something scrolled, as a request says it.
fn scrolled(by: ScrollBy) -> Value {
    match by {
        ScrollBy::Pixels(pixels) => length(pixels),
        ScrollBy::ToStart => Value::from("start"),
        ScrollBy::ToEnd => Value::from("end"),
    }
}

/// What a verb did, as a kind and its fields.
fn outcome(outcome: &Outcome) -> Value {
    let mut fields = Map::new();
    fields.insert("kind".to_owned(), Value::from(outcome_kind(outcome)));
    fields.insert("node".to_owned(), id(outcome.node()));
    match outcome {
        Outcome::Activated { name, .. } => {
            if let Some(name) = name {
                fields.insert("name".to_owned(), Value::from(name.as_str()));
            }
        }
        Outcome::Followed { to, .. } => {
            fields.insert("to".to_owned(), Value::from(to.as_str()));
        }
        Outcome::TextPut { text, .. } | Outcome::TextCanceled { text, .. } => {
            fields.insert("text".to_owned(), Value::from(text.as_str()));
        }
        Outcome::Scrolled { by, .. } => {
            fields.insert("by".to_owned(), scrolled(*by));
        }
        Outcome::Chosen { list, chosen, .. } => {
            fields.insert("list".to_owned(), id(*list));
            fields.insert("chosen".to_owned(), Value::Bool(*chosen));
        }
        Outcome::Submitted { method, to, .. } => {
            fields.insert("method".to_owned(), Value::from(method.name()));
            fields.insert("to".to_owned(), Value::from(to.as_str()));
        }
        Outcome::NotSubmitted { invalid, .. } => {
            let unmet = invalid.iter().map(self::unmet).collect();
            fields.insert("invalid".to_owned(), Value::Array(unmet));
        }
    }
    Value::Object(fields)
}

/// A control that failed its constraint, as a form not sent says it.
fn unmet(unmet: &Unmet) -> Value {
    let mut fields = Map::new();
    fields.insert("control".to_owned(), unmet.control.map_or(Value::Null, id));
    fields.insert(
        "constraint".to_owned(),
        unmet
            .constraint
            .map_or(Value::Null, |constraint| Value::from(constraint.name())),
    );
    fields.insert("message".to_owned(), Value::from(unmet.message.as_str()));
    Value::Object(fields)
}

/// A refusal, as a kind and its fields.
fn refused(refused: &Refused) -> Value {
    let mut fields = Map::new();
    fields.insert("kind".to_owned(), Value::from(refused.kind()));
    let mut put = |name: &str, value: Value| {
        fields.insert(name.to_owned(), value);
    };
    match refused {
        Refused::NotJson | Refused::NotAnObject => {}
        Refused::LineTooLong { most }
        | Refused::TooDeep { most }
        | Refused::TooManyCallers { most } => put("most", Value::from(*most)),
        Refused::UnknownVersion { speaks } => put("speaks", Value::from(*speaks)),
        Refused::MissingField { field } | Refused::BadField { field } => {
            put("field", Value::from(*field));
        }
        Refused::UnknownField { field } => put("field", Value::from(field.as_str())),
        Refused::FieldTooLong { field, most } => {
            put("field", Value::from(*field));
            put("most", Value::from(*most));
        }
        Refused::NotAnAddress { tab }
        | Refused::NoSuchTab { tab }
        | Refused::NothingLoaded { tab }
        | Refused::TabGone { tab, .. }
        | Refused::NotCarriedOut { tab, .. } => put("tab", Value::from(tab.as_str())),
        Refused::Ambiguous { tab, count } => {
            put("tab", Value::from(tab.as_str()));
            put("count", Value::from(*count));
        }
        Refused::NotGranted { tab, site } => {
            put("tab", Value::from(tab.as_str()));
            // A site with no name is an identity of ours, and crosses as
            // nothing rather than as a number nobody outside could use.
            let named = site.host().map(|_| Value::from(site.to_string()));
            put("site", named.unwrap_or(Value::Null));
        }
        Refused::TreeTooDeep { tab, most } => {
            put("tab", Value::from(tab.as_str()));
            put("most", Value::from(*most));
        }
        Refused::Verb { tab, refusal } => {
            put("tab", Value::from(tab.as_str()));
            verb_refused(refusal, &mut put);
        }
    }
    Value::Object(fields)
}

/// The fields of one of `verb.rs`'s refusals, as it holds them.
fn verb_refused(refusal: &Refusal, put: &mut impl FnMut(&str, Value)) {
    match refusal {
        Refusal::NotFound { target } => put("target", target_value(target)),
        Refusal::Ambiguous { target, candidates } => {
            put("target", target_value(target));
            put(
                "candidates",
                Value::Array(candidates.iter().copied().map(id).collect()),
            );
        }
        Refusal::NotOperable { node, role } | Refusal::NotAField { node, role } => {
            put("node", id(*node));
            put("role", Value::from(role.as_str()));
        }
        Refusal::Disabled { node }
        | Refusal::ReadOnly { node }
        | Refusal::Chooses { node }
        | Refusal::DoesNotScroll { node } => {
            put("node", id(*node));
        }
        Refusal::NotADate { node, text } => {
            put("node", id(*node));
            put("text", Value::from(text.as_str()));
        }
        Refusal::Unbuilt { node, kind } => {
            put("node", id(*node));
            put("field_kind", Value::from(kind.as_str()));
        }
        Refusal::TooLong { node, most, length } => {
            put("node", id(*node));
            put("most", Value::from(*most));
            put("length", Value::from(*length));
        }
        Refusal::Unchecked {
            node,
            control,
            constraint,
        } => {
            put("node", id(*node));
            put("control", control.map_or(Value::Null, id));
            put("constraint", Value::from(constraint.name()));
        }
        Refusal::NotSubmittable { node, why } => {
            put("node", id(*node));
            put("why", Value::from(why.as_str()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asked(line: &str) -> Result<Ask, Refused> {
        read(line.as_bytes()).ask
    }

    #[test]
    fn each_ask_is_read() {
        assert_eq!(asked(r#"{"v":1,"ask":"tabs"}"#), Ok(Ask::Tabs));
        assert_eq!(
            asked(r#"{"v":1,"ask":"read","tab":"https://example.com/"}"#),
            Ok(Ask::Read {
                tab: "https://example.com/".to_owned()
            })
        );
        assert_eq!(
            asked(
                r#"{"v":1,"ask":"act","tab":"t","target":{"role":"textbox","name":"Name:"},"verb":"put_text","text":"Ada"}"#
            ),
            Ok(Ask::Act {
                tab: "t".to_owned(),
                target: Target::NamedOfRole {
                    role: Role::Known(KnownRole::TextBox),
                    name: "Name:".to_owned()
                },
                verb: Verb::PutText("Ada".to_owned()),
            })
        );
        assert_eq!(
            asked(
                r#"{"v":1,"ask":"act","tab":"t","target":{"node":7},"verb":"scroll","scroll":"end"}"#
            ),
            Ok(Ask::Act {
                tab: "t".to_owned(),
                target: Target::Node(BoxId::from_wire(7)),
                verb: Verb::Scroll(ScrollBy::ToEnd),
            })
        );
    }

    #[test]
    fn a_form_not_sent_names_its_constraint_only_where_the_renderer_said_it() {
        let not_sent = Outcome::NotSubmitted {
            node: BoxId::from_wire(9),
            invalid: vec![
                alo_agent::verb::Invalid {
                    control: Some(BoxId::from_wire(4)),
                    constraint: Some(alo_dom::constraint::Constraint::ValueMissing),
                    message: "Please fill out this field.".to_owned(),
                },
                // Chrome's bubble: words, and no constraint.
                alo_agent::verb::Invalid {
                    control: Some(BoxId::from_wire(5)),
                    constraint: None,
                    message: "Fill it in, please".to_owned(),
                },
            ],
        };
        assert_eq!(
            outcome(&not_sent).get("invalid").map(ToString::to_string),
            Some(
                r#"[{"constraint":"valueMissing","control":4,"message":"Please fill out this field."},{"constraint":null,"control":5,"message":"Fill it in, please"}]"#
                    .to_owned()
            )
        );
    }

    #[test]
    fn an_option_chosen_says_its_list_and_whether_it_is_chosen() {
        let chosen = |chosen: bool| {
            outcome(&Outcome::Chosen {
                node: BoxId::from_wire(43),
                list: BoxId::from_wire(29),
                chosen,
            })
            .to_string()
        };
        assert_eq!(
            chosen(true),
            r#"{"chosen":true,"kind":"Chosen","list":29,"node":43}"#
        );
        assert_eq!(
            chosen(false),
            r#"{"chosen":false,"kind":"Chosen","list":29,"node":43}"#
        );
    }

    #[test]
    fn a_node_that_is_not_drawn_says_so_beside_its_rect() {
        let option = alo_renderer::SnapshotNode {
            id: BoxId::from_wire(43),
            role: Role::Known(KnownRole::Option),
            name: Some("Huge".to_owned()),
            states: alo_box::States::default(),
            placeholder: None,
            marker: None,
            rect: alo_layout::Rect::ZERO,
            rects: Vec::new(),
            offscreen: true,
            scrolls: false,
            children: Vec::new(),
        };
        assert_eq!(
            node(&option).to_string(),
            r#"{"children":[],"name":"Huge","node":43,"offscreen":true,"rect":{"height":0.0,"width":0.0,"x":0.0,"y":0.0},"role":"option","undrawn":true}"#
        );
        let drawn = alo_renderer::SnapshotNode {
            rects: vec![alo_layout::Rect::new(1.0, 2.0, 3.0, 4.0)],
            rect: alo_layout::Rect::new(1.0, 2.0, 3.0, 4.0),
            offscreen: false,
            ..option
        };
        assert!(node(&drawn).get("undrawn").is_none());
    }

    #[test]
    fn what_is_not_a_request_is_refused_by_kind() {
        assert_eq!(asked("{"), Err(Refused::NotJson));
        assert_eq!(asked("[1]"), Err(Refused::NotAnObject));
        assert_eq!(
            asked(r#"{"ask":"tabs"}"#),
            Err(Refused::UnknownVersion { speaks: 1 })
        );
        assert_eq!(
            asked(r#"{"v":2,"ask":"tabs"}"#),
            Err(Refused::UnknownVersion { speaks: 1 })
        );
        assert_eq!(
            asked(r#"{"v":1}"#),
            Err(Refused::MissingField { field: "ask" })
        );
        assert_eq!(
            asked(r#"{"v":1,"ask":"click"}"#),
            Err(Refused::BadField { field: "ask" })
        );
        assert_eq!(
            asked(r#"{"v":1,"ask":"tabs","verbose":true}"#),
            Err(Refused::UnknownField {
                field: "verbose".to_owned()
            })
        );
        assert_eq!(
            asked(r#"{"v":1,"ask":"read"}"#),
            Err(Refused::MissingField { field: "tab" })
        );
        assert_eq!(
            asked(r#"{"v":1,"ask":"read","tab":"t","verb":"activate"}"#),
            Err(Refused::UnknownField {
                field: "verb".to_owned()
            }),
            "a field another ask takes is not this one's"
        );
    }

    #[test]
    fn a_verb_is_refused_without_what_it_carries() {
        let act = |rest: &str| {
            asked(&format!(
                r#"{{"v":1,"ask":"act","tab":"t","target":{{"name":"n"}},{rest}}}"#
            ))
        };
        assert_eq!(
            act(r#""verb":"put_text""#),
            Err(Refused::MissingField { field: "text" })
        );
        assert_eq!(
            act(r#""verb":"scroll","scroll":"sideways""#),
            Err(Refused::BadField { field: "scroll" })
        );
        assert_eq!(
            act(r#""verb":"scroll","scroll":1e300"#),
            Err(Refused::BadField { field: "scroll" }),
            "a distance layout cannot hold"
        );
        assert_eq!(
            act(r#""verb":"activate","text":"x""#),
            Err(Refused::UnknownField {
                field: "text".to_owned()
            })
        );
        assert_eq!(
            act(r#""verb":"hover""#),
            Err(Refused::BadField { field: "verb" })
        );
    }

    #[test]
    fn a_target_names_one_thing_one_way() {
        let target = |target: &str| {
            asked(&format!(
                r#"{{"v":1,"ask":"act","tab":"t","target":{target},"verb":"activate"}}"#
            ))
        };
        let bad = Err(Refused::BadField { field: "target" });
        assert_eq!(target(r#"{"node":3,"name":"n"}"#), bad);
        assert_eq!(target("{}"), bad);
        assert_eq!(target(r#"{"x":10,"y":20}"#), bad, "never a coordinate");
        assert_eq!(target(r#"{"node":-1}"#), bad);
        assert_eq!(target(r#""Save""#), bad);
    }

    #[test]
    fn every_bound_is_refused_and_nothing_is_cut() {
        let long = "n".repeat(MOST_NAME + 1);
        assert_eq!(
            asked(&format!(
                r#"{{"v":1,"ask":"act","tab":"t","target":{{"name":"{long}"}},"verb":"activate"}}"#
            )),
            Err(Refused::FieldTooLong {
                field: "name",
                most: MOST_NAME
            })
        );
        let deep = format!("{}{}", "[".repeat(5), "]".repeat(5));
        assert_eq!(asked(&deep), Err(Refused::TooDeep { most: MOST_NESTING }));
        assert_eq!(
            asked(r#"{"v":1,"ask":"read","tab":"[[[[[[[[[["}"#),
            Ok(Ask::Read {
                tab: "[[[[[[[[[[".to_owned()
            }),
            "a bracket in a string is text, not nesting"
        );
    }

    #[test]
    fn what_was_echoed_comes_back_and_is_bounded() {
        assert_eq!(
            read(br#"{"v":1,"id":"a","ask":"tabs"}"#).echo,
            Some(Echo::Text("a".to_owned()))
        );
        assert_eq!(
            read(br#"{"v":1,"id":42,"ask":"tabs"}"#).echo,
            Some(Echo::Number(42))
        );
        assert_eq!(
            read(br#"{"v":1,"id":-1,"ask":"tabs"}"#).ask,
            Err(Refused::BadField { field: "id" })
        );
    }

    #[test]
    fn an_answer_is_one_line_with_its_keys_in_order() {
        assert_eq!(
            write(
                Some(&Echo::Number(3)),
                &Answer::Tabs(vec!["https://example.com/".to_owned()])
            ),
            "{\"answer\":\"tabs\",\"id\":3,\"tabs\":[\"https://example.com/\"],\"v\":1}\n"
        );
        assert_eq!(
            write(
                None,
                &Answer::Refused(Refused::Verb {
                    tab: "t".to_owned(),
                    refusal: Refusal::TooLong {
                        node: BoxId::from_wire(4),
                        most: 3,
                        length: 5
                    }
                })
            ),
            "{\"answer\":\"refused\",\"refusal\":{\"kind\":\"TooLong\",\"length\":5,\"most\":3,\
             \"node\":4,\"tab\":\"t\"},\"said\":\"box#4 takes at most 3 characters, and the \
             text is 5 long\",\"v\":1}\n"
        );
        // A list takes no text, and says which list (ADR 0064 § 4).
        assert_eq!(
            write(
                None,
                &Answer::Refused(Refused::Verb {
                    tab: "t".to_owned(),
                    refusal: Refusal::Chooses {
                        node: BoxId::from_wire(6),
                    }
                })
            ),
            "{\"answer\":\"refused\",\"refusal\":{\"kind\":\"Chooses\",\"node\":6,\"tab\":\"t\"},\
             \"said\":\"box#6 is a list to choose from, which takes no text; activate an option \
             of it instead\",\"v\":1}\n"
        );
    }

    #[test]
    fn a_site_refused_crosses_as_its_name_and_a_site_with_none_as_null() {
        let site = |text: &str| {
            alo_renderer::site::Site::of(
                &alo_url::parse(text).unwrap_or_else(|_| alo_url::Url::about_blank()),
            )
        };
        let refused = |tab: &str| {
            write(
                None,
                &Answer::Refused(Refused::NotGranted {
                    tab: tab.to_owned(),
                    site: site(tab),
                }),
            )
        };
        let named = refused("https://www.example.org/a");
        assert!(
            named.starts_with(
                "{\"answer\":\"refused\",\"refusal\":{\"kind\":\"NotGranted\",\
                 \"site\":\"https://example.org\",\"tab\":\"https://www.example.org/a\"},"
            ),
            "{named}"
        );
        let none = refused("data:text/html,hi");
        assert!(none.contains("\"site\":null,"), "{none}");
    }

    #[test]
    fn a_verb_not_carried_out_crosses_as_its_kind_with_the_tab() {
        let refused = Answer::Refused(Refused::NotCarriedOut {
            tab: "https://example.com/".to_owned(),
            said: "Chrome drew box#4 nowhere".to_owned(),
        });
        assert_eq!(
            write(None, &refused),
            "{\"answer\":\"refused\",\"refusal\":{\"kind\":\"NotCarriedOut\",\
             \"tab\":\"https://example.com/\"},\"said\":\"nothing was done in the tab at \
             https://example.com/: Chrome drew box#4 nowhere\",\"v\":1}\n"
        );
    }

    #[test]
    fn which_renderer_draws_a_tab_is_asked_by_its_address_alone() {
        assert_eq!(
            asked(r#"{"v":1,"ask":"renderer","tab":"https://example.com/"}"#),
            Ok(Ask::Renderer {
                tab: "https://example.com/".to_owned()
            })
        );
        assert_eq!(
            asked(r#"{"v":1,"ask":"renderer"}"#),
            Err(Refused::MissingField { field: "tab" })
        );
        assert_eq!(
            asked(r#"{"v":1,"ask":"renderer","tab":"t","target":{"name":"n"}}"#),
            Err(Refused::UnknownField {
                field: "target".to_owned()
            }),
            "a field another ask takes is not this one's"
        );
        assert_eq!(
            asked(&format!(
                r#"{{"v":1,"ask":"renderer","tab":"{}"}}"#,
                "a".repeat(MOST_ADDRESS + 1)
            )),
            Err(Refused::FieldTooLong {
                field: "tab",
                most: MOST_ADDRESS
            })
        );
    }

    #[test]
    fn a_renderer_crosses_as_its_kind_with_chromes_own_name_for_itself() {
        let answered = |rendered_by| {
            write(
                Some(&Echo::Number(9)),
                &Answer::Renderer {
                    tab: "https://example.com/".to_owned(),
                    rendered_by,
                },
            )
        };
        assert_eq!(
            answered(RenderedBy::ThisEngine),
            "{\"answer\":\"renderer\",\"id\":9,\"renderer\":{\"kind\":\"ThisEngine\"},\
             \"said\":\"this browser's engine\",\"tab\":\"https://example.com/\",\"v\":1}\n"
        );
        assert_eq!(
            answered(RenderedBy::Chrome {
                product: "Chrome/155.0.8059.86".to_owned()
            }),
            "{\"answer\":\"renderer\",\"id\":9,\"renderer\":{\"kind\":\"Chrome\",\
             \"product\":\"Chrome/155.0.8059.86\"},\"said\":\"Chrome/155.0.8059.86, driven by \
             this browser\",\"tab\":\"https://example.com/\",\"v\":1}\n"
        );
    }

    #[test]
    fn a_length_is_written_as_layout_holds_it() {
        assert_eq!(length(23.2).to_string(), "23.2");
        assert_eq!(length(146.656_25).to_string(), "146.65625");
        assert_eq!(length(f32::NAN), Value::Null);
        assert_eq!(length(f32::INFINITY), Value::Null);
    }

    #[test]
    fn hostile_lines_are_refused_and_never_panic() {
        for line in [
            &b""[..],
            b"\xff\xfe",
            b"\"",
            b"{\"v\":1,\"ask\":\"act\",\"tab\":\"t\",\"target\":{\"node\":18446744073709551615},\"verb\":\"activate\"}",
            b"{\"v\":1e999}",
            b"{\"v\":1,\"ask\":\"tabs\",\"ask\":\"read\"}",
            b"]]]]]]]]]]",
            b"\"\\\"[[[[[[[[",
        ] {
            let _ = read(line);
        }
    }
}
