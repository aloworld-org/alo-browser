/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 325: `navigator` (ADR 0030 §§ 4–5).
//!
//! *Every member in ADR 0030 § 5's table answers what it says*, for each of
//! § 2's three kinds of system, from what the browser process told the
//! renderer and nothing this crate composed — so each row here is handed
//! over as an [`Identity`], and a made-up one is answered as told.
//!
//! Every script runs twice, the second time with the collector running at
//! every allocation, and the two must agree.

use alo_bindings::{DocumentCell, Identity, Interface, adopt, install, introduce};
use alo_dom::parse_document;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::{Property, Value};
use alo_js::script;

/// ADR 0030 § 2's rows: the string, the platform, and what `appVersion`
/// answers.
const ROWS: [(&str, &str, &str); 3] = [
    (
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) alo/0.0",
        "MacIntel",
        "5.0 (Macintosh)",
    ),
    (
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) alo/0.0",
        "Win32",
        "5.0 (Windows)",
    ),
    (
        "Mozilla/5.0 (X11; Linux x86_64) alo/0.0",
        "Linux x86_64",
        "5.0 (X11)",
    ),
];

/// An engine with a page's document installed and `navigator` introduced.
struct Page {
    engine: Engine,
    _root: Root,
    cell: Ref,
}

impl Page {
    fn new(identity: Identity<'_>, stress: bool) -> Result<Self, String> {
        let mut engine = Engine::new().map_err(|why| why.to_string())?;
        let cell = adopt(engine.objects(), parse_document("<!DOCTYPE html><p>x</p>"))
            .map_err(|why| why.to_string())?;
        let root = engine.objects().heap_mut().root(cell);
        install(&mut engine, cell).map_err(|why| why.to_string())?;
        introduce(&mut engine, cell, identity).map_err(|why| why.to_string())?;
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

    /// Put the getter of `Navigator.prototype`'s `member` on the global
    /// object as `getter`, so a script can call it on something else — the
    /// one way to reach it until `Object.getOwnPropertyDescriptor` (item 73).
    fn expose_getter(&mut self, member: &str) -> Option<()> {
        let global = self.engine.global().ok()?;
        let objects = self.engine.objects();
        let prototype = objects
            .embedded::<DocumentCell>(self.cell)?
            .interfaces()
            .prototype(Interface::Navigator)?;
        let units: Vec<u16> = member.encode_utf16().collect();
        let key = objects.existing_key(&units)?;
        let getter = objects.own_property(prototype, key).ok()??.getter()?;
        let name: Vec<u16> = "getter".encode_utf16().collect();
        objects
            .define_named(global, &name, Property::data(getter, true, true, true))
            .ok()
            .filter(|defined| *defined)
            .map(drop)
    }
}

/// Run `source` on a page told `identity`, both ways, and answer what it
/// answered.
fn both_ways(identity: Identity<'_>, source: &str) -> String {
    let mut answers = [false, true].map(|stress| match Page::new(identity, stress) {
        Ok(mut page) => page.run(source),
        Err(why) => format!("! no page: {why}"),
    });
    let [plain, stressed] = &mut answers;
    assert_eq!(plain, stressed, "{source}");
    core::mem::take(plain)
}

/// Every member of § 5's table, joined with `|`.
const EVERY_MEMBER: &str = "var n = navigator; n.appCodeName + '|' + n.appName + '|' + \
                            n.appVersion + '|' + n.platform + '|' + n.product + '|' + \
                            n.productSub + '|' + n.userAgent + '|' + n.vendor + '|' + \
                            n.vendorSub + '|' + n.oscpu + '|' + n.taintEnabled()";

#[test]
fn every_member_answers_what_adr_0030_says_for_each_kind_of_system() {
    for (user_agent, platform, app_version) in ROWS {
        let identity = Identity {
            user_agent,
            platform,
        };
        assert_eq!(
            both_ways(identity, EVERY_MEMBER),
            format!(
                "Mozilla|Netscape|{app_version}|{platform}|Gecko|20100101|{user_agent}|||\
                 |false"
            ),
        );
    }
}

#[test]
fn what_the_page_reads_is_what_it_was_told_and_nothing_composed() {
    // Not a row of the table: a renderer that composed its own answer, or
    // read this machine's, would show here.
    let told = Identity {
        user_agent: "Mozilla/5.0 (Plan 9; told) alo/7.1",
        platform: "told",
    };
    assert_eq!(
        both_ways(
            told,
            "navigator.userAgent + '|' + navigator.platform + '|' + navigator.appVersion"
        ),
        "Mozilla/5.0 (Plan 9; told) alo/7.1|told|5.0 (Plan 9)",
    );
    // A string HTML does not read makes `appVersion` empty, as HTML says.
    let unread = Identity {
        user_agent: "alo/7.1",
        platform: "",
    };
    assert_eq!(
        both_ways(
            unread,
            "'[' + navigator.appVersion + '][' + navigator.platform + ']'"
        ),
        "[][]",
    );
}

#[test]
fn the_downloads_page_lines_find_the_system_it_was_told() {
    // `alo-downloads`' first five lines, which item 325 was opened by.
    let lines = "var p = navigator.platform || ''; var ua = navigator.userAgent || ''; \
                 var isMac = /Mac/.test(p) || /Mac OS X/.test(ua); \
                 var isWin = /Win/.test(p) || /Windows/.test(ua); \
                 (isMac ? 'mac' : '') + (isWin ? 'win' : '')";
    let found: Vec<String> = ROWS
        .iter()
        .map(|(user_agent, platform, _)| {
            both_ways(
                Identity {
                    user_agent,
                    platform,
                },
                lines,
            )
        })
        .collect();
    assert_eq!(found, ["mac", "win", ""]);
}

#[test]
fn navigator_is_one_object_a_page_may_replace() {
    let [(user_agent, platform, _), ..] = ROWS;
    let identity = Identity {
        user_agent,
        platform,
    };
    assert_eq!(
        both_ways(identity, "(navigator === navigator) + typeof navigator"),
        "trueobject",
    );
    // `[Replaceable]`: assigning replaces it, and deleting takes it away.
    assert_eq!(both_ways(identity, "navigator = 'mine'; navigator"), "mine");
    assert_eq!(
        both_ways(identity, "delete navigator; typeof navigator"),
        "undefined"
    );
    // No interface object, as for every interface but the two events.
    assert_eq!(both_ways(identity, "typeof Navigator"), "undefined");
}

#[test]
fn what_is_not_decided_is_absent() {
    let [(user_agent, platform, _), ..] = ROWS;
    let identity = Identity {
        user_agent,
        platform,
    };
    // ADR 0030, *What this does not decide*: absent rather than approximate.
    assert_eq!(
        both_ways(
            identity,
            "var n = navigator; typeof n.language + ',' + typeof n.languages + ',' + \
             typeof n.webdriver + ',' + typeof n.onLine + ',' + typeof n.cookieEnabled + ',' + \
             typeof n.hardwareConcurrency + ',' + typeof n.userAgentData + ',' + typeof n.plugins"
        ),
        "undefined,undefined,undefined,undefined,undefined,undefined,undefined,undefined",
    );
}

#[test]
fn a_member_used_on_something_else_is_a_type_error() {
    let [(user_agent, platform, _), ..] = ROWS;
    let identity = Identity {
        user_agent,
        platform,
    };
    assert_eq!(
        both_ways(
            identity,
            "try { navigator.taintEnabled.call({}); 'no' } catch (e) { e.name }"
        ),
        "TypeError",
    );
    for member in ["userAgent", "platform", "appVersion", "vendor", "oscpu"] {
        let mut page = match Page::new(identity, false) {
            Ok(page) => page,
            Err(why) => panic!("no page: {why}"),
        };
        assert_eq!(page.expose_getter(member), Some(()), "{member}");
        assert_eq!(
            page.run("try { getter.call({}); 'no' } catch (e) { e.name + ': ' + e.message }"),
            format!("TypeError: '{member}' was used on something that is not a Navigator"),
        );
        // And on the real one, answers.
        assert_eq!(
            page.run(&format!("getter.call(navigator) === navigator.{member}")),
            "true",
        );
    }
}

#[test]
fn a_second_navigator_is_refused_and_the_first_stands() {
    let [(user_agent, platform, _), ..] = ROWS;
    let identity = Identity {
        user_agent,
        platform,
    };
    let mut page = match Page::new(identity, false) {
        Ok(page) => page,
        Err(why) => panic!("no page: {why}"),
    };
    let other = Identity {
        user_agent: "other",
        platform: "other",
    };
    let refused = introduce(&mut page.engine, page.cell, other);
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(page.run("navigator.platform"), platform);
}
