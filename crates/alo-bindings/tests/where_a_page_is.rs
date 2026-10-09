/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 360: `location`, read.
//!
//! *Each reading member answers the specification's value for an address
//! with and without a port, query and fragment; a document with no address
//! says so rather than inventing one; and a navigating member is refused by
//! name.* And the two places a page reaches it — the global object and the
//! document — answer one object, whose members are its own, unforgeable,
//! and read the address the browser process stated last.
//!
//! Every script runs twice, the second time with the collector running at
//! every allocation, and the two must agree.

use alo_bindings::{DocumentCell, Interface, Location, adopt, install, navigating};
use alo_dom::parse_document;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::symbol::WellKnown;
use alo_js::object::{Property, Value};
use alo_js::script;

/// What every navigating member is refused with.
const REFUSED: &str =
    "! a script navigating by 'location' is queue item 85, through the ask of ADR 0020";

/// Every reading member, joined with `|`.
const EVERY_MEMBER: &str = "var l = location; l.href + '|' + l.origin + '|' + l.protocol + '|' + \
                            l.host + '|' + l.hostname + '|' + l.port + '|' + l.pathname + '|' + \
                            l.search + '|' + l.hash + '|' + l.toString()";

/// An engine with a page's document installed, at `address` if there is
/// one.
struct Page {
    engine: Engine,
    _root: Root,
    cell: Ref,
}

impl Page {
    fn new(address: Option<&str>, stress: bool) -> Result<Self, String> {
        let mut engine = alo_bindings::engine(None).map_err(|why| why.to_string())?;
        let cell = adopt(engine.objects(), parse_document("<!DOCTYPE html><p>x</p>"))
            .map_err(|why| why.to_string())?;
        let root = engine.objects().heap_mut().root(cell);
        if let Some(address) = address {
            let url = alo_url::parse(address).map_err(|why| why.to_string())?;
            navigating::locate(engine.objects(), cell, url);
        }
        install(&mut engine, cell).map_err(|why| why.to_string())?;
        engine.objects().heap_mut().stress(stress);
        Ok(Self {
            engine,
            _root: root,
            cell,
        })
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

    /// The page's `Location`, as the document cell holds it.
    fn location(&mut self) -> Option<Ref> {
        let objects = self.engine.objects();
        objects.embedded::<DocumentCell>(self.cell)?.location()
    }

    /// The own property `name` of `object`, as what a descriptor says.
    fn own(&mut self, object: Ref, name: &str) -> Option<Described> {
        let units: Vec<u16> = name.encode_utf16().collect();
        let objects = self.engine.objects();
        let key = objects.existing_key(&units)?;
        let property = objects.own_property(object, key).ok().flatten()?;
        Some(Described::of(property))
    }
}

/// What a property descriptor says of a property.
#[derive(Debug)]
struct Described {
    value: Option<Value>,
    getter: Option<Value>,
    setter: Option<Value>,
    writable: bool,
    enumerable: bool,
    configurable: bool,
}

impl Described {
    fn of(property: &Property) -> Self {
        Self {
            value: property.value(),
            getter: property.getter(),
            setter: property.setter(),
            writable: property.is_writable(),
            enumerable: property.is_enumerable(),
            configurable: property.is_configurable(),
        }
    }

    /// Whether it has a setter that is a function.
    fn sets(&self) -> bool {
        matches!(self.setter, Some(Value::Object(_)))
    }

    /// Whether it has a getter that is a function.
    fn gets(&self) -> bool {
        matches!(self.getter, Some(Value::Object(_)))
    }
}

/// Run `source` on a page at `address`, both ways, and answer what it
/// answered.
fn both_ways(address: Option<&str>, source: &str) -> String {
    let mut answers = [false, true].map(|stress| match Page::new(address, stress) {
        Ok(mut page) => page.run(source),
        Err(why) => format!("! no page: {why}"),
    });
    let [plain, stressed] = &mut answers;
    assert_eq!(plain, stressed, "{source}");
    core::mem::take(plain)
}

#[test]
fn every_member_reads_an_address_with_every_part() {
    assert_eq!(
        both_ways(
            Some("https://shop.example.com:8443/a/b?x=1#top"),
            EVERY_MEMBER
        ),
        "https://shop.example.com:8443/a/b?x=1#top|https://shop.example.com:8443|https:|\
         shop.example.com:8443|shop.example.com|8443|/a/b|?x=1|#top|\
         https://shop.example.com:8443/a/b?x=1#top"
    );
}

#[test]
fn every_member_reads_an_address_with_no_port_query_or_fragment() {
    assert_eq!(
        both_ways(Some("https://nordwind.alosites.com/"), EVERY_MEMBER),
        "https://nordwind.alosites.com/|https://nordwind.alosites.com|https:|\
         nordwind.alosites.com|nordwind.alosites.com||/|||https://nordwind.alosites.com/"
    );
    // The scheme's own port, written, is no port at all.
    assert_eq!(
        both_ways(
            Some("http://example.com:80/p"),
            "location.host + '|' + location.port"
        ),
        "example.com|"
    );
}

#[test]
fn a_document_with_no_address_is_at_about_blank_and_says_so() {
    assert_eq!(
        both_ways(None, EVERY_MEMBER),
        "about:blank|null|about:||||blank|||about:blank"
    );
}

#[test]
fn the_global_object_and_the_document_answer_one_location() {
    assert_eq!(
        both_ways(
            None,
            "location === document.location && location === location && \
             document.location === document.location"
        ),
        "true"
    );
    assert_eq!(
        both_ways(None, "typeof location + '|' + typeof document.location"),
        "object|object"
    );
}

#[test]
fn it_reads_the_address_stated_last_not_one_copied_when_it_was_made() {
    for stress in [false, true] {
        let Ok(mut page) = Page::new(Some("https://example.com/first"), stress) else {
            panic!("a page");
        };
        assert_eq!(page.run("var held = location; held.pathname"), "/first");
        let Ok(url) = alo_url::parse("https://example.com/second?q") else {
            panic!("a URL");
        };
        navigating::locate(page.engine.objects(), page.cell, url);
        assert_eq!(page.run("held.pathname + held.search"), "/second?q");
    }
}

#[test]
fn a_location_turns_into_its_href() {
    assert_eq!(
        both_ways(
            Some("https://example.com/a?b#c"),
            "'' + location + '|' + `${location}` + '|' + (location + '')"
        ),
        "https://example.com/a?b#c|https://example.com/a?b#c|https://example.com/a?b#c"
    );
    // By `toString`, since `valueOf` answers the object itself.
    assert_eq!(both_ways(None, "location.valueOf() === location"), "true");
    assert_eq!(both_ways(None, "location.valueOf === ({}).valueOf"), "true");
}

#[test]
fn every_navigating_member_is_refused_by_name() {
    for source in [
        "location.href = 'https://example.com/elsewhere'",
        "location.hash = 'top'",
        "location.search = '?q'",
        "location.pathname = '/p'",
        "location.host = 'example.org'",
        "location.hostname = 'example.org'",
        "location.port = '8080'",
        "location.protocol = 'http'",
        "location.assign('/p')",
        "location.replace('/p')",
        "location.reload()",
        "location = '/p'",
        "document.location = '/p'",
        // Strict code as well: the refusal is not a silent nothing in either.
        "'use strict'; location = '/p'",
        "'use strict'; location.hash = 'top'",
    ] {
        assert_eq!(
            both_ways(Some("https://example.com/"), source),
            REFUSED,
            "{source}"
        );
    }
    // A refusal is not a navigation that happened: nothing moved.
    assert_eq!(
        both_ways(
            Some("https://example.com/"),
            "try { location.hash = 'x'; } catch (e) {} location.href"
        ),
        REFUSED
    );
}

#[test]
fn origin_has_no_setter_so_assigning_it_is_the_languages_answer() {
    assert_eq!(
        both_ways(
            Some("https://example.com/"),
            "location.origin = 'x'; location.origin"
        ),
        "https://example.com"
    );
    assert!(
        both_ways(
            Some("https://example.com/"),
            "'use strict'; location.origin = 'x'"
        )
        .starts_with("! TypeError"),
    );
}

#[test]
fn a_member_used_on_something_else_is_a_type_error() {
    for source in [
        "location.toString.call({})",
        "location.assign.call(document, '/p')",
        "location.reload.call(undefined)",
    ] {
        let answer = both_ways(None, source);
        assert!(
            answer.starts_with("! TypeError") && answer.contains("not a Location"),
            "{source}: {answer}"
        );
    }
}

#[test]
fn every_member_is_its_own_unforgeable_property() {
    let Ok(mut page) = Page::new(None, false) else {
        panic!("a page");
    };
    let Some(location) = page.location() else {
        panic!("install made a Location");
    };
    assert!(
        page.engine
            .objects()
            .embedded::<Location>(location)
            .is_some()
    );
    for name in [
        "href", "origin", "protocol", "host", "hostname", "port", "pathname", "search", "hash",
    ] {
        let Some(property) = page.own(location, name) else {
            panic!("{name} is an own property");
        };
        assert!(property.gets(), "{name} has a getter");
        assert_eq!(property.sets(), name != "origin", "{name}'s setter");
        assert!(property.enumerable && !property.configurable, "{name}");
    }
    for name in ["assign", "replace", "reload", "toString"] {
        let Some(property) = page.own(location, name) else {
            panic!("{name} is an own property");
        };
        assert!(
            matches!(property.value, Some(Value::Object(_))),
            "{name} is an operation"
        );
        assert!(
            property.enumerable && !property.writable && !property.configurable,
            "{name}"
        );
    }
    // Neither can be taken away or replaced.
    assert_eq!(
        page.run("delete location.href; delete location.toString; typeof location.href"),
        "string"
    );
    assert!(
        page.run("'use strict'; delete location.href")
            .starts_with("! TypeError")
    );
    assert!(
        page.run("'use strict'; location.toString = function () { return 'x'; }")
            .starts_with("! TypeError")
    );
    assert_eq!(page.run("typeof location.ancestorOrigins"), "undefined");
}

#[test]
fn value_of_and_to_primitive_are_its_own_and_fixed() {
    let Ok(mut page) = Page::new(None, false) else {
        panic!("a page");
    };
    let Some(location) = page.location() else {
        panic!("install made a Location");
    };
    let Some(value_of) = page.own(location, "valueOf") else {
        panic!("valueOf is an own property");
    };
    assert!(matches!(value_of.value, Some(Value::Object(_))));
    assert!(!value_of.writable && !value_of.enumerable && !value_of.configurable);
    let (intrinsics, objects) = page.engine.intrinsics();
    let Ok(key) = intrinsics.well_known_key(objects, WellKnown::ToPrimitive) else {
        panic!("the realm has Symbol.toPrimitive");
    };
    let Ok(Some(to_primitive)) = objects.own_property(location, key) else {
        panic!("Symbol.toPrimitive is an own property");
    };
    assert_eq!(to_primitive.value(), Some(Value::Undefined));
    assert!(
        !to_primitive.is_writable()
            && !to_primitive.is_enumerable()
            && !to_primitive.is_configurable()
    );
}

#[test]
fn location_is_unforgeable_on_the_document_and_the_global_object() {
    let Ok(mut page) = Page::new(None, false) else {
        panic!("a page");
    };
    let Ok(global) = page.engine.global() else {
        panic!("a global object");
    };
    let Some(on_global) = page.own(global, "location") else {
        panic!("location is the global object's own property");
    };
    assert!(on_global.gets() && on_global.sets());
    assert!(on_global.enumerable && !on_global.configurable);

    let Some(Value::Object(document)) = page.own(global, "document").and_then(|seen| seen.value)
    else {
        panic!("document is on the global object");
    };
    let Some(on_document) = page.own(document, "location") else {
        panic!("location is the document's own property");
    };
    assert!(on_document.gets() && on_document.sets());
    assert!(on_document.enumerable && !on_document.configurable);
    // And not `Document.prototype`'s, nor anything on `Location.prototype`.
    let Some(prototype) = page
        .engine
        .objects()
        .embedded::<DocumentCell>(page.cell)
        .and_then(|held| held.interfaces().prototype(Interface::Document))
    else {
        panic!("Document.prototype");
    };
    assert!(page.own(prototype, "location").is_none());
    let Some(prototype) = page
        .engine
        .objects()
        .embedded::<DocumentCell>(page.cell)
        .and_then(|held| held.interfaces().prototype(Interface::Location))
    else {
        panic!("Location.prototype");
    };
    assert_eq!(
        page.engine
            .objects()
            .own_keys(prototype)
            .map(|keys| keys.len()),
        Ok(0)
    );
    assert_eq!(
        page.run("delete location; delete document.location; location === document.location"),
        "true"
    );
}

#[test]
fn the_location_lives_as_long_as_the_page_through_any_number_of_collections() {
    let Ok(mut page) = Page::new(Some("https://example.com/kept"), true) else {
        panic!("a page");
    };
    assert_eq!(
        page.run(
            "var seen = 0, last = ''; for (var i = 0; i < 64; i++) { \
             last = location.pathname; if (location === document.location) { seen++; } } \
             seen + last"
        ),
        "64/kept"
    );
    let found = page.location().map(|location| {
        page.engine
            .objects()
            .embedded::<Location>(location)
            .is_some()
    });
    assert_eq!(found, Some(true));
}
