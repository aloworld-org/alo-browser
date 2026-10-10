/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 364 (ADR 0039 § 1): `document.visibilityState` and
//! `document.hidden`.
//!
//! - A document is `"hidden"`, and `hidden` is `true`, until the embedder
//!   states otherwise into its cell — HTML's initial state.
//! - Each read reads the cell now: a state stated between two reads is the
//!   second read's answer.
//! - Both are getters on `Document.prototype` with no setter, so assigning
//!   changes nothing — and throws in strict code — and nothing but a
//!   document answers them.
//! - A second document, which no window was associated with, is `"hidden"`
//!   whatever the page's is.
//!
//! Every script runs twice, the second time with the collector running at
//! every allocation, and the two must agree.

use alo_bindings::{
    DocumentCell, Interface, Visibility, adopt, document, furnish, install, prototype_of,
    visibility, wrap,
};
use alo_dom::parse_document;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::{Property, Value};
use alo_js::script;

/// An engine with a page's document installed, and a second document in
/// the same realm as `other`.
struct Page {
    engine: Engine,
    _roots: [Root; 2],
    cell: Ref,
}

impl Page {
    fn new(stress: bool) -> Result<Self, String> {
        let mut engine = alo_bindings::engine(None).map_err(|why| why.to_string())?;
        let cell = adopt(engine.objects(), parse_document("<!DOCTYPE html><p>x</p>"))
            .map_err(|why| why.to_string())?;
        let root = engine.objects().heap_mut().root(cell);
        install(&mut engine, cell).map_err(|why| why.to_string())?;
        let other =
            adopt(engine.objects(), parse_document("<p>y</p>")).map_err(|why| why.to_string())?;
        let other_root = engine.objects().heap_mut().root(other);
        furnish(&mut engine, other).map_err(|why| why.to_string())?;
        let objects = engine.objects();
        let node = document(objects, other).ok_or("a document")?.root();
        let prototype = prototype_of(objects, other, node);
        let wrapper = wrap(objects, other, node, prototype).map_err(|why| format!("{why:?}"))?;
        let global = engine.global().map_err(|why| why.to_string())?;
        let name: Vec<u16> = "other".encode_utf16().collect();
        engine
            .objects()
            .define_named(
                global,
                &name,
                Property::data(Value::Object(wrapper), true, false, true),
            )
            .map_err(|why| why.to_string())?;
        engine.objects().heap_mut().stress(stress);
        Ok(Self {
            engine,
            _roots: [root, other_root],
            cell,
        })
    }

    /// State `to` into the page's document cell, as the renderer does.
    fn state(&mut self, to: Visibility) -> Option<bool> {
        visibility::update(self.engine.objects(), self.cell, to)
    }

    /// What `source` evaluates to, as text.
    fn run(&mut self, source: &str) -> String {
        let program = match script(source) {
            Ok(program) => program,
            Err(why) => return format!("? did not parse: {why}"),
        };
        match self.engine.evaluate(&program) {
            Ok(Value::Text(held)) => self
                .engine
                .objects()
                .units(held)
                .map_or_else(|| "?".to_owned(), String::from_utf16_lossy),
            Ok(Value::Bool(answer)) => answer.to_string(),
            Ok(other) => format!("{other:?}"),
            Err(Trouble::Escaped(escape)) => format!("! {escape}"),
            Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
        }
    }

    /// The getter and the setter of `Document.prototype`'s own `member`.
    fn member(&mut self, member: &str) -> Option<(Option<Value>, Option<Value>)> {
        let objects = self.engine.objects();
        let prototype = objects
            .embedded::<DocumentCell>(self.cell)?
            .interfaces()
            .prototype(Interface::Document)?;
        let units: Vec<u16> = member.encode_utf16().collect();
        let key = objects.existing_key(&units)?;
        objects
            .own_property(prototype, key)
            .ok()?
            .map(|property| (property.getter(), property.setter()))
    }
}

/// What each step answers, both ways: a step is a state to state first, if
/// any, and a script.
fn both_ways(steps: &[(Option<Visibility>, &str)]) -> Vec<String> {
    let answers = [false, true].map(|stress| match Page::new(stress) {
        Ok(mut page) => steps
            .iter()
            .map(|(state, source)| {
                if let Some(to) = state
                    && page.state(*to).is_none()
                {
                    return "not a document cell".to_owned();
                }
                page.run(source)
            })
            .collect(),
        Err(why) => vec![why],
    });
    let [ordinary, stressed] = answers;
    assert_eq!(
        ordinary, stressed,
        "{steps:?} answered differently when the collector ran at every allocation"
    );
    ordinary
}

const READ: &str = "document.visibilityState + ' ' + document.hidden";

#[test]
fn a_document_is_hidden_until_it_is_told_otherwise() {
    assert_eq!(both_ways(&[(None, READ)]), ["hidden true"]);
}

#[test]
fn each_read_reads_the_state_now() {
    assert_eq!(
        both_ways(&[
            (Some(Visibility::Visible), READ),
            (Some(Visibility::Hidden), READ),
            (Some(Visibility::Visible), READ),
        ]),
        ["visible false", "hidden true", "visible false"]
    );
}

#[test]
fn stating_the_state_says_whether_it_changed() {
    let Ok(mut page) = Page::new(false) else {
        panic!("an empty heap holds a page");
    };
    assert_eq!(
        page.state(Visibility::Hidden),
        Some(false),
        "it starts hidden"
    );
    assert_eq!(page.state(Visibility::Visible), Some(true));
    assert_eq!(page.state(Visibility::Visible), Some(false));
    assert_eq!(page.state(Visibility::Hidden), Some(true));
}

#[test]
fn both_are_read_only_getters_on_the_prototype() {
    let Ok(mut page) = Page::new(false) else {
        panic!("an empty heap holds a page");
    };
    for member in ["visibilityState", "hidden"] {
        let Some((getter, setter)) = page.member(member) else {
            panic!("Document.prototype has no {member}");
        };
        assert!(getter.is_some(), "{member} has a getter");
        assert!(
            matches!(setter, None | Some(Value::Undefined)),
            "{member} has no setter"
        );
    }
    assert_eq!(
        both_ways(&[(
            Some(Visibility::Visible),
            "document.hasOwnProperty('visibilityState') + ' ' + \
             document.hasOwnProperty('hidden') + ' ' + ('hidden' in document) + ' ' + \
             typeof document.visibilityState + ' ' + typeof document.hidden"
        )]),
        ["false false true string boolean"]
    );
    assert_eq!(
        both_ways(&[(
            Some(Visibility::Visible),
            "document.visibilityState = 'hidden'; document.hidden = true; \
             var strict = (function () { 'use strict'; \
               try { document.hidden = true; return 'nothing'; } \
               catch (e) { return e.name; } })(); \
             strict + ' ' + document.visibilityState + ' ' + document.hidden"
        )]),
        ["TypeError visible false"]
    );
}

#[test]
fn nothing_but_a_document_answers_them() {
    assert_eq!(
        both_ways(&[(
            Some(Visibility::Visible),
            "var fake = {}; fake.__proto__ = document.__proto__; \
             function caught(f) { try { f(); return 'nothing'; } catch (e) { return e.name; } } \
             caught(function () { return fake.visibilityState; }) + ' ' + \
             caught(function () { return fake.hidden; })"
        )]),
        ["TypeError TypeError"]
    );
}

#[test]
fn a_document_no_window_shows_is_hidden_whatever_the_page_is() {
    assert_eq!(
        both_ways(&[
            (
                Some(Visibility::Visible),
                "other.visibilityState + ' ' + other.hidden"
            ),
            (None, READ),
        ]),
        ["hidden true", "visible false"]
    );
}
