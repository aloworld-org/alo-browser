/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 369: `navigator.sendBeacon` in a page (ADR 0040 §§ 4 and 5).
//!
//! A beacon is an ask, recorded in the document cell as a fetch is, and
//! claiming to outlive its page. So each test runs a script, takes the asks
//! as the renderer would, and reads what was asked; the keep-alive count is
//! freed by delivering an answer as the renderer does.
//!
//! Every script runs twice, the second time with the collector running at
//! every allocation, and the two must agree.

use alo_bindings::fetching::{self, Asked, Keepalive, MOST_KEPT_ALIVE};
use alo_bindings::{Identity, adopt, delivering, install, introduce, navigating, offer};
use alo_dom::parse_document;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::script;
use alo_net::cors::{Credentials, Mode};
use alo_net::redirect;

/// An engine with a page's document at `https://example.com/start`, whose
/// base is `/app/`, given `fetch` and a `navigator`.
struct Page {
    engine: Engine,
    _root: Root,
    cell: Ref,
}

impl Page {
    fn new(stress: bool) -> Result<Self, String> {
        let mut engine = alo_bindings::engine(None).map_err(|why| why.to_string())?;
        let cell = adopt(
            engine.objects(),
            parse_document("<!DOCTYPE html><base href=/app/><p>x</p>"),
        )
        .map_err(|why| why.to_string())?;
        let root = engine.objects().heap_mut().root(cell);
        let url = alo_url::parse("https://example.com/start").map_err(|why| why.to_string())?;
        navigating::locate(engine.objects(), cell, url);
        install(&mut engine, cell).map_err(|why| why.to_string())?;
        offer(&mut engine, cell).map_err(|why| why.to_string())?;
        introduce(
            &mut engine,
            cell,
            Identity {
                user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) alo/0.0",
                platform: "MacIntel",
            },
        )
        .map_err(|why| why.to_string())?;
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
            Ok(other) => format!("{other:?}"),
            Err(Trouble::Escaped(escape)) => format!("! {escape}"),
            Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
        }
    }

    /// Every ask the page has made since the last take.
    fn asks(&mut self) -> Vec<Asked> {
        fetching::take(self.engine.objects(), self.cell).unwrap_or_default()
    }

    /// A network error for ask `number` arrives, as the renderer takes one:
    /// whether it freed a keep-alive count, and whether anything waited for
    /// it.
    fn answer(&mut self, number: u64) -> Result<(bool, bool), String> {
        let freed = fetching::answered(self.engine.objects(), self.cell, number)
            .ok_or("not a document cell")?;
        let waited = delivering::answer(self.engine.objects(), self.cell, number, None)
            .map(|delivery| delivery.is_some())
            .map_err(|why| why.to_string())?;
        Ok((freed, waited))
    }
}

/// Run `test` on a page as it is, and again with the collector at every
/// allocation — or say why a page could not be made.
fn both(test: impl Fn(&mut Page)) -> Result<(), String> {
    for stress in [false, true] {
        let mut page = Page::new(stress)?;
        test(&mut page);
    }
    Ok(())
}

/// A script that leaves `big` holding 65 536 `x`s: `alo-js` has no
/// `String.prototype.repeat` to ask for it with.
const BIG: &str = "var big = 'x'; for (var i = 0; i < 16; i++) { big = big + big; } ";

#[test]
fn a_string_is_sent_as_text_by_post_and_outlives_its_page() {
    both(|page| {
        assert_eq!(
            page.run("'' + navigator.sendBeacon('/_alo/collect', 'd=1000&p=%2F&w=800')"),
            "true"
        );
        let asks = page.asks();
        let [ask] = asks.as_slice() else {
            panic!("one ask: {asks:?}");
        };
        assert_eq!(ask.url.serialised, "https://example.com/_alo/collect");
        assert_eq!(ask.method, "POST");
        assert_eq!(
            ask.headers,
            [(
                "Content-Type".to_owned(),
                "text/plain;charset=UTF-8".to_owned()
            )]
        );
        assert_eq!(ask.body, b"d=1000&p=%2F&w=800");
        assert_eq!(
            (ask.mode, ask.credentials, ask.redirect, ask.referrer),
            (
                Mode::NoCors,
                Credentials::Include,
                redirect::Mode::Follow,
                None
            )
        );
        assert_eq!(ask.keepalive, Keepalive::Beacon);
        // Nothing waits for its answer — `sendBeacon` never says what became
        // of it — and its arrival frees its bytes.
        assert_eq!(Ok((true, false)), page.answer(ask.number));
    })
    .expect("the pages are made");
}

#[test]
fn no_data_is_no_body_and_any_other_value_is_its_string() {
    both(|page| {
        assert_eq!(
            page.run(
                "'' + navigator.sendBeacon('one') + navigator.sendBeacon('two', null) + \
                 navigator.sendBeacon('three', undefined) + navigator.sendBeacon('four', 42) + \
                 navigator.sendBeacon('five', { toString: function () { return 'made'; } })"
            ),
            "truetruetruetruetrue"
        );
        let asks = page.asks();
        let read: Vec<(String, String, usize)> = asks
            .iter()
            .map(|ask| {
                (
                    ask.url.serialised.clone(),
                    String::from_utf8_lossy(&ask.body).into_owned(),
                    ask.headers.len(),
                )
            })
            .collect();
        // Relative to the document's base, `/app/`.
        assert_eq!(
            read,
            [
                ("https://example.com/app/one".to_owned(), String::new(), 0),
                ("https://example.com/app/two".to_owned(), String::new(), 0),
                ("https://example.com/app/three".to_owned(), String::new(), 0),
                (
                    "https://example.com/app/four".to_owned(),
                    "42".to_owned(),
                    1
                ),
                (
                    "https://example.com/app/five".to_owned(),
                    "made".to_owned(),
                    1
                ),
            ]
        );
        assert!(asks.iter().all(|ask| ask.keepalive == Keepalive::Beacon));
    })
    .expect("the pages are made");
}

#[test]
fn sixty_four_kibibytes_may_be_in_flight_and_an_answer_frees_them() {
    both(|page| {
        assert_eq!(
            page.run(&format!(
                "{BIG} '' + navigator.sendBeacon('/c', big) + ' ' + \
                 navigator.sendBeacon('/c', 'x') + ' ' + navigator.sendBeacon('/c')"
            )),
            "true false true",
            "the whole bound, then one byte past it; an empty body fits"
        );
        let asks = page.asks();
        assert_eq!(asks.len(), 2, "a beacon refused asks nothing");
        assert_eq!(asks[0].body.len(), MOST_KEPT_ALIVE);
        // The answer to the first arrives, a network error as like as a
        // response, and its bytes come off the count.
        assert_eq!(Ok((true, false)), page.answer(asks[0].number));
        assert_eq!(page.run("'' + navigator.sendBeacon('/c', 'x')"), "true");
        assert_eq!(page.asks().len(), 1);
        // Answering again, or a number never asked, frees nothing more.
        assert_eq!(Ok((false, false)), page.answer(asks[0].number));
        assert_eq!(Ok((false, false)), page.answer(9999));
    })
    .expect("the pages are made");
}

#[test]
fn a_body_past_the_bound_alone_is_refused_and_asks_nothing() {
    both(|page| {
        assert_eq!(
            page.run(&format!("{BIG} '' + navigator.sendBeacon('/c', big + 'x')")),
            "false"
        );
        assert!(page.asks().is_empty());
        // Nothing was counted for it.
        assert_eq!(
            page.run(&format!("{BIG} '' + navigator.sendBeacon('/c', big)")),
            "true"
        );
    })
    .expect("the pages are made");
}

#[test]
fn a_url_that_is_not_one_or_not_the_webs_throws_a_type_error() {
    both(|page| {
        for (url, said) in [
            ("'http://['", "is not a URL a beacon can be sent to"),
            ("'data:text/plain,hi'", "is a data: URL"),
            ("'javascript:void 0'", "is a javascript: URL"),
            ("'ftp://example.com/x'", "is a ftp: URL"),
        ] {
            assert_eq!(
                page.run(&format!(
                    "var e; try {{ navigator.sendBeacon({url}, 'x'); }} catch (x) {{ e = x; }} \
                     e.name + ': ' + e.message"
                ))
                .split_once(": ")
                .map(|(name, message)| (name.to_owned(), message.contains(said))),
                Some(("TypeError".to_owned(), true)),
                "{url}"
            );
        }
        assert!(page.asks().is_empty());
    })
    .expect("the pages are made");
}

#[test]
fn web_idl_checks_this_and_the_arguments_first() {
    both(|page| {
        assert_eq!(
            page.run(
                "var named = ''; \
                 try { navigator.sendBeacon(); } catch (x) { named += x.name; } \
                 try { navigator.sendBeacon.call({}, '/c'); } catch (x) { named += x.name; } \
                 named"
            ),
            "TypeErrorTypeError"
        );
        assert!(page.asks().is_empty());
        // `url` behind a `toString` is converted, and so is `data` after it.
        assert_eq!(
            page.run("'' + navigator.sendBeacon({ toString: function () { return '/o'; } }, 'b')"),
            "true"
        );
        let asks = page.asks();
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0].url.serialised, "https://example.com/o");
        assert_eq!(asks[0].body, b"b");
        // Both behind one is refused by name, after the first has run.
        let both = page.run(
            "var ran = 0; navigator.sendBeacon( \
               { toString: function () { ran++; return '/o'; } }, \
               { toString: function () { ran++; return 'b'; } })",
        );
        assert!(both.starts_with("! "), "{both}");
        assert_eq!(page.run("'' + ran"), "1");
        assert!(page.asks().is_empty());
    })
    .expect("the pages are made");
}

#[test]
fn a_fetch_does_not_outlive_its_page_and_is_not_counted() {
    both(|page| {
        assert_eq!(
            page.run(&format!(
                "{BIG} fetch('/f', {{ method: 'POST', body: big }}); \
                 '' + navigator.sendBeacon('/c', big)"
            )),
            "true"
        );
        let asks = page.asks();
        assert_eq!(
            asks.iter().map(|ask| ask.keepalive).collect::<Vec<_>>(),
            [Keepalive::Not, Keepalive::Beacon]
        );
    })
    .expect("the pages are made");
}
