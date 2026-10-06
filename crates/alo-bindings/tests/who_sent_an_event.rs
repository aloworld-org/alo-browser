/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 260: `isTrusted`, as Web IDL's `[LegacyUnforgeable]`
//! attribute (ADR 0019).
//!
//! *`e.isTrusted` is `false` after a script's `dispatchEvent`, its property
//! is the instance's own and the same getter on two events, and a page
//! cannot replace it.* The browser's half — `true` in a listener for the
//! browser's dispatch — is `alo-renderer`'s, which drives that dispatch.
//!
//! Each script runs against an adopted, installed document, both ordinarily
//! and with the collector running at every allocation, and the two must
//! agree: the getter is held by the document cell's unforgeables object and
//! by every event it was copied to, and nothing else.

use alo_bindings::event::{self, Firing};
use alo_bindings::{Interface, adopt, install};
use alo_dom::parse_document;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::{Found, Value};
use alo_js::{numeric, script};

/// An engine with a page's document installed, and the root on its cell.
struct Page {
    engine: Engine,
    _root: Root,
    cell: Ref,
}

impl Page {
    fn new(stress: bool) -> Result<Self, String> {
        let mut engine = Engine::new().map_err(|why| why.to_string())?;
        let cell = adopt(
            engine.objects(),
            parse_document("<!DOCTYPE html><html><body><p id=p>x</p></body></html>"),
        )
        .map_err(|why| why.to_string())?;
        let root = engine.objects().heap_mut().root(cell);
        install(&mut engine, cell).map_err(|why| why.to_string())?;
        engine.objects().heap_mut().stress(stress);
        Ok(Self {
            engine,
            _root: root,
            cell,
        })
    }

    /// What `source` evaluates to, as a string.
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
            Ok(Value::Number(number)) => numeric::text_of(number),
            Ok(other) => format!("{other:?}"),
            Err(Trouble::Escaped(escape)) => format!("! {escape}"),
            Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
        }
    }

    /// The global `name`, which a script left an object in.
    fn global(&mut self, name: &str) -> Result<Ref, String> {
        let global = self.engine.global().map_err(|why| why.to_string())?;
        let units: Vec<u16> = name.encode_utf16().collect();
        match self.engine.objects().get_named(global, &units) {
            Ok(Found::Value(Value::Object(held))) => Ok(held),
            other => Err(format!("the global '{name}' is not an object: {other:?}")),
        }
    }

    /// `object`'s own `isTrusted`, as its getter, its setter, whether it is
    /// enumerable and whether it is configurable.
    fn own_is_trusted(&mut self, object: Ref) -> Result<(Value, Value, bool, bool), String> {
        let objects = self.engine.objects();
        let units: Vec<u16> = "isTrusted".encode_utf16().collect();
        let key = objects
            .existing_key(&units)
            .ok_or("'isTrusted' is not interned")?;
        let property = objects
            .own_property(object, key)
            .map_err(|why| why.to_string())?
            .ok_or("'isTrusted' is not its own property")?;
        match (property.getter(), property.setter()) {
            (Some(getter), Some(setter)) => Ok((
                getter,
                setter,
                property.is_enumerable(),
                property.is_configurable(),
            )),
            _ => Err("'isTrusted' is not an accessor".to_owned()),
        }
    }

    /// The document cell's unforgeables object for `interface`.
    fn unforgeables(&mut self, interface: Interface) -> Option<Ref> {
        self.engine
            .objects()
            .embedded::<alo_bindings::DocumentCell>(self.cell)
            .and_then(|held| held.interfaces().unforgeables(interface))
    }
}

/// Run `source` both ways and answer what it answered.
fn run(source: &str) -> String {
    let answer = |stress| Page::new(stress).map_or_else(|why| why, |mut page| page.run(source));
    let ordinary = answer(false);
    let stressed = answer(true);
    assert_eq!(
        ordinary, stressed,
        "{source} answered differently when the collector ran at every allocation"
    );
    ordinary
}

/// The walk every script starts from.
const NAMES: &str = "var p = document.documentElement.lastChild.firstChild; var out = ''; \
                     function say(what) { out += (out === '' ? '' : ',') + what; }";

#[test]
fn a_script_s_own_event_is_not_trusted() {
    assert_eq!(
        run(&format!(
            "{NAMES} var e = new Event('x'); say(e.isTrusted); \
             p.addEventListener('x', ev => say(ev.isTrusted)); \
             say(p.dispatchEvent(e)); say(e.isTrusted); \
             say(new CustomEvent('y', {{ detail: 1 }}).isTrusted); \
             say(typeof e.isTrusted); out;"
        )),
        "false,false,true,false,false,boolean"
    );
}

#[test]
fn it_is_the_instance_s_own_and_not_the_prototype_s() {
    assert_eq!(
        run(&format!(
            "{NAMES} say(typeof Event.prototype.isTrusted); \
             say(typeof CustomEvent.prototype.isTrusted); \
             say(new Event('x').isTrusted); out;"
        )),
        "undefined,undefined,false"
    );
}

#[test]
fn a_page_cannot_replace_it() {
    assert_eq!(
        run(&format!(
            "{NAMES} var e = new Event('x'); \
             say(delete e.isTrusted); say(e.isTrusted); \
             e.isTrusted = true; say(e.isTrusted); \
             say((function () {{ 'use strict'; \
                  try {{ e.isTrusted = true; return 'assigned'; }} \
                  catch (thrown) {{ return thrown.name; }} }})()); \
             say((function () {{ 'use strict'; \
                  try {{ delete e.isTrusted; return 'deleted'; }} \
                  catch (thrown) {{ return thrown.name; }} }})()); \
             Event.prototype.isTrusted = true; say(new Event('y').isTrusted); \
             var c = new CustomEvent('z'); c.__proto__ = null; say(c.isTrusted); out;"
        )),
        "false,false,false,TypeError,TypeError,false,false"
    );
}

#[test]
fn every_event_shares_one_getter_per_realm() {
    for stress in [false, true] {
        let mut page = Page::new(stress).unwrap();
        assert_eq!(
            page.run(
                "var a = new Event('a'); var b = new CustomEvent('b'); \
                 var junk = null; for (var i = 0; i < 100; i++) { junk = { i: i, next: junk }; } \
                 a.isTrusted === b.isTrusted"
            ),
            "true"
        );
        let a = page.global("a").unwrap();
        let b = page.global("b").unwrap();
        let (getter, setter, enumerable, configurable) = page.own_is_trusted(a).unwrap();
        assert!(
            matches!(getter, Value::Object(_)),
            "the getter is a function"
        );
        assert_eq!(setter, Value::Undefined, "'isTrusted' has no setter");
        assert!(enumerable, "'isTrusted' is enumerable");
        assert!(!configurable, "'isTrusted' is not configurable");
        assert_eq!(
            page.own_is_trusted(b).unwrap().0,
            getter,
            "two events' getters are one function"
        );

        // The browser's event gets the same property, from the same object.
        let cell = page.cell;
        let made = event::create(
            page.engine.objects(),
            cell,
            &Firing {
                kind: "click",
                bubbles: true,
                cancelable: true,
                composed: true,
            },
        )
        .unwrap();
        let held = page.engine.objects().heap_mut().root(made);
        assert_eq!(page.own_is_trusted(made).unwrap().0, getter);
        page.engine.objects().heap_mut().release(held);

        // And it is the getter the document cell's unforgeables hold; an
        // interface with no unforgeable member has no such object.
        let unforgeables = page.unforgeables(Interface::Event).unwrap();
        assert_eq!(page.own_is_trusted(unforgeables).unwrap().0, getter);
        assert_eq!(page.unforgeables(Interface::Node), None);
        assert_eq!(page.unforgeables(Interface::CustomEvent), None);
    }
}

#[test]
fn a_second_install_is_refused_and_the_first_host_stands() {
    let mut page = Page::new(false).unwrap();
    let other = adopt(
        page.engine.objects(),
        parse_document("<!DOCTYPE html><html><body></body></html>"),
    )
    .unwrap();
    let held = page.engine.objects().heap_mut().root(other);
    let refused = install(&mut page.engine, other);
    assert!(
        matches!(&refused, Err(escape) if escape.to_string().contains("already defined")),
        "a second install was not refused: {refused:?}"
    );
    page.engine.objects().heap_mut().release(held);
    assert_eq!(page.run("new Event('x').isTrusted"), "false");
}
