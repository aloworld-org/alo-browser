/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 335: `fetch` in a page (ADR 0032 §§ 1, 2 and 4).
//!
//! A page's `fetch` is an ask recorded in its document cell, and the answer
//! is delivered later, as the browser process sent it. So each test here
//! runs a script, takes the asks as the renderer would, delivers an answer
//! as the renderer would — one call of what `alo-bindings` names, then a
//! checkpoint — and reads what the page's reactions did.
//!
//! Every script runs twice, the second time with the collector running at
//! every allocation, and the two must agree: a promise waits in the
//! document cell between the script and the answer, held by nothing else.

use alo_bindings::fetching::{self, Asked};
use alo_bindings::response::Kind;
use alo_bindings::{Responded, adopt, delivering, install, navigating, offer};
use alo_dom::parse_document;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::Value;
use alo_js::script;
use alo_net::cors::{Credentials, Mode};
use alo_net::redirect;

/// An engine with a page's document at `https://example.com/app/`, given
/// `fetch`.
struct Page {
    engine: Engine,
    _root: Root,
    cell: Ref,
    /// What the checkpoints reported, as text.
    reported: Vec<String>,
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
        engine.objects().heap_mut().stress(stress);
        Ok(Self {
            engine,
            _root: root,
            cell,
            reported: Vec::new(),
        })
    }

    /// What `source` evaluates to, as text, after a checkpoint.
    fn run(&mut self, source: &str) -> String {
        let program = match script(source) {
            Ok(program) => program,
            Err(why) => return format!("? did not parse: {why}"),
        };
        let answer = match self.engine.evaluate(&program) {
            Ok(Value::Text(held)) => self
                .engine
                .objects()
                .units(held)
                .map_or_else(|| "?".to_owned(), String::from_utf16_lossy),
            Ok(other) => format!("{other:?}"),
            Err(Trouble::Escaped(escape)) => format!("! {escape}"),
            Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
        };
        self.checkpoint();
        answer
    }

    fn checkpoint(&mut self) {
        let reported = &mut self.reported;
        let drained = self.engine.checkpoint(&mut |_, thrown, _| {
            reported.push(format!("{thrown:?}"));
        });
        if let Err(escape) = drained {
            self.reported.push(format!("stopped: {escape}"));
        }
    }

    /// Every ask the page has made since the last take.
    fn asks(&mut self) -> Vec<Asked> {
        fetching::take(self.engine.objects(), self.cell).unwrap_or_default()
    }

    /// Deliver the answer to ask `number`, as the renderer does: whether
    /// anything waited for it — or, for an answer that could not be made or
    /// a call that did not finish, what went wrong.
    fn deliver(&mut self, number: u64, responded: Option<Responded>) -> Result<bool, String> {
        let delivery = delivering::answer(self.engine.objects(), self.cell, number, responded)
            .map_err(|why| why.to_string())?;
        let Some(delivery) = delivery else {
            return Ok(false);
        };
        let root = match delivery.arguments[1] {
            Value::Object(response) => Some(self.engine.objects().heap_mut().root(response)),
            _ => None,
        };
        let called = self
            .engine
            .call(delivery.callee, Value::Undefined, &delivery.arguments);
        if let Some(root) = root {
            self.engine.objects().heap_mut().release(root);
        }
        called.map_err(|why| why.to_string())?;
        self.checkpoint();
        Ok(true)
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

fn basic(body: &str) -> Responded {
    Responded {
        kind: Kind::Basic,
        status: 200,
        status_text: "OK".to_owned(),
        url: Some("https://example.com/app/hello.txt".to_owned()),
        redirected: false,
        headers: vec![
            ("Content-Type".to_owned(), "text/plain".to_owned()),
            ("X-Two".to_owned(), "a".to_owned()),
            ("x-two".to_owned(), "b".to_owned()),
        ],
        body: body.as_bytes().to_vec(),
    }
}

#[test]
fn a_same_origin_text_body_is_read_after_its_answer_arrives() {
    both(|page| {
        let made = page.run(
            "var seen = 'waiting';
             var p = fetch('hello.txt');
             p.then(function (r) {
               seen = r.type + ' ' + r.status + ' ' + r.ok + ' ' + r.statusText + ' ' + r.url
                 + ' ' + r.redirected + ' ' + r.headers.get('content-type')
                 + ' ' + r.headers.get('X-TWO') + ' ' + r.headers.has('x-none')
                 + ' ' + (r.headers === r.headers) + ' ' + r.bodyUsed;
               return r.text();
             }).then(function (t) { seen = seen + ' | ' + t; });
             typeof p.then",
        );
        assert_eq!(made, "function");
        assert_eq!(page.run("seen"), "waiting", "nothing is answered yet");
        let asks = page.asks();
        assert_eq!(asks.len(), 1);
        let ask = &asks[0];
        assert_eq!(ask.url.serialised, "https://example.com/app/hello.txt");
        assert_eq!(ask.method, "GET");
        assert!(ask.headers.is_empty() && ask.body.is_empty());
        assert_eq!(
            (ask.mode, ask.credentials, ask.redirect, ask.referrer),
            (
                Mode::Cors,
                Credentials::SameOrigin,
                redirect::Mode::Follow,
                None
            )
        );
        assert!(page.asks().is_empty(), "the asks were taken");
        assert_eq!(Ok(true), page.deliver(ask.number, Some(basic("hé there"))));
        assert_eq!(
            page.run("seen"),
            "basic 200 true OK https://example.com/app/hello.txt false text/plain a, b false \
             true false | hé there"
        );
        assert!(page.reported.is_empty(), "{:?}", page.reported);
        assert_eq!(
            Ok(false),
            page.deliver(ask.number, Some(basic("again"))),
            "answered twice"
        );
    })
    .expect("the pages are made");
}

#[test]
fn an_opaque_answer_shows_the_page_nothing() {
    both(|page| {
        page.run(
            "var seen = 'waiting';
             fetch('https://other.example/x', { mode: 'no-cors' }).then(function (r) {
               seen = r.type + ' ' + r.status + ' ' + r.ok + ' [' + r.url + '] [' + r.statusText
                 + '] ' + r.headers.has('content-type');
               return r.text();
             }).then(function (t) { seen = seen + ' [' + t + ']'; });",
        );
        let asks = page.asks();
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0].mode, Mode::NoCors);
        let opaque = Responded {
            kind: Kind::Opaque,
            status: 0,
            status_text: String::new(),
            url: None,
            redirected: false,
            headers: Vec::new(),
            body: Vec::new(),
        };
        assert_eq!(Ok(true), page.deliver(asks[0].number, Some(opaque)));
        assert_eq!(page.run("seen"), "opaque 0 false [] [] false []");
    })
    .expect("the pages are made");
}

#[test]
fn a_fetch_that_failed_rejects_with_one_type_error_whatever_the_reason() {
    both(|page| {
        page.run(
            "var seen = 'waiting';
             fetch('/gone').then(function () { seen = 'fulfilled'; },
                                 function (e) { seen = e.name + ': ' + e.message; });",
        );
        let asks = page.asks();
        assert_eq!(asks[0].url.serialised, "https://example.com/gone");
        assert_eq!(Ok(true), page.deliver(asks[0].number, None));
        assert_eq!(
            page.run("seen"),
            "TypeError: the fetch failed, and a page is not told why"
        );
    })
    .expect("the pages are made");
}

#[test]
fn what_the_request_steps_refuse_rejects_and_asks_for_nothing() {
    both(|page| {
        for (init, said) in [
            (
                "{ method: 'CONNECT' }",
                "CONNECT is a method no page may use",
            ),
            ("{ method: 'no way' }", "\"no way\" is not a method"),
            ("{ mode: 'navigate' }", "navigate mode"),
            ("{ mode: 'sideways' }", "\"sideways\" is not a RequestMode"),
            (
                "{ method: 'PUT', mode: 'no-cors' }",
                "can only GET, HEAD or POST",
            ),
            ("{ body: 'x' }", "a GET cannot have a body"),
            ("{ headers: { 'a b': 'c' } }", "is not a header"),
            ("{ headers: { a: 'Ā' } }", "is not a ByteString"),
            ("{ window: {} }", "'window' can only be null"),
            ("7", "is not a dictionary"),
        ] {
            let seen = page.run(&format!(
                "var seen = 'waiting';
                 fetch('/x', {init}).catch(function (e) {{ seen = e.name + ': ' + e.message; }});
                 seen"
            ));
            assert_eq!(seen, "waiting", "{init}: the rejection is a job");
            let seen = page.run("seen");
            assert!(
                seen.starts_with("TypeError: ") && seen.contains(said),
                "{init}: {seen}"
            );
        }
        let seen = page.run(
            "var seen = 'waiting';
             fetch('https://user:pw@example.com/').catch(function (e) { seen = e.message; });
             fetch('http://[::').catch(function (e) { seen = seen + ' / ' + e.message; });
             'ran'",
        );
        assert_eq!(seen, "ran", "a refusal rejects, it does not escape");
        let seen = page.run("seen");
        assert!(
            seen.contains("names a user or a password") && seen.contains("is not a URL"),
            "{seen}"
        );
        assert!(
            page.asks().is_empty(),
            "a refused fetch asked for something"
        );
    })
    .expect("the pages are made");
}

#[test]
fn headers_are_checked_and_what_the_guard_drops_is_dropped() {
    both(|page| {
        page.run(
            "fetch('/p', { method: 'post', body: 'a=1',
                           headers: { 'X-A': ' 1 ', Cookie: 'no', 'Sec-Fetch-X': 'no',
                                      Accept: 'x' } });
             fetch('/q', { method: 'POST', mode: 'no-cors', body: 'b',
                           headers: { 'X-A': '1', Accept: 'x',
                                      'Content-Type': 'application/json' } });
             fetch('/r', { credentials: 'include', redirect: 'manual',
                           referrerPolicy: 'no-referrer', headers: {} });",
        );
        let asks = page.asks();
        assert_eq!(asks.len(), 3);
        let pairs = |ask: &Asked| -> Vec<(String, String)> { ask.headers.clone() };
        let owned = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect()
        };
        assert_eq!(asks[0].method, "POST");
        assert_eq!(asks[0].body, b"a=1");
        assert_eq!(
            pairs(&asks[0]),
            owned(&[
                ("X-A", "1"),
                ("Accept", "x"),
                ("Content-Type", "text/plain;charset=UTF-8"),
            ])
        );
        // Under no-cors only what a form could have sent survives, and a
        // JSON content type is not that.
        assert_eq!(
            pairs(&asks[1]),
            owned(&[
                ("Accept", "x"),
                ("Content-Type", "text/plain;charset=UTF-8")
            ])
        );
        assert_eq!(
            (asks[2].credentials, asks[2].redirect, asks[2].referrer),
            (
                Credentials::Include,
                redirect::Mode::Manual,
                Some(alo_net::referrer::Policy::NoReferrer)
            )
        );
        let numbers: Vec<u64> = asks.iter().map(|ask| ask.number).collect();
        assert_eq!(numbers, [0, 1, 2], "each ask its own number, in order");
    })
    .expect("the pages are made");
}

#[test]
fn the_init_is_read_member_by_member_in_order() {
    both(|page| {
        let order = page.run(
            "var order = '';
             var init = {
               get window() { order += 'w'; return null; },
               get method() { order += 'm'; return { toString: function () { order += 't';
                                                                             return 'HEAD'; } }; },
               get body() { order += 'b'; return undefined; },
               get credentials() { order += 'c'; return 'omit'; },
               get mode() { order += 'o'; return undefined; }
             };
             fetch('/o', init);
             order",
        );
        assert_eq!(order, "bcmtow");
        let asks = page.asks();
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0].method, "HEAD");
        assert_eq!(asks[0].credentials, Credentials::Omit);
        let thrown = page.run(
            "var seen = 'waiting';
             fetch('/o', { get method() { throw new RangeError('mine'); } })
               .catch(function (e) { seen = e.name + ': ' + e.message; });
             'ran'",
        );
        assert_eq!(
            thrown, "ran",
            "a getter's throw rejects, it does not escape"
        );
        assert_eq!(page.run("seen"), "RangeError: mine");
        assert!(page.asks().is_empty());
    })
    .expect("the pages are made");
}

#[test]
fn what_is_not_built_is_refused_by_name() {
    both(|page| {
        for (call, said) in [
            ("fetch('data:,hi')", "a data: URL"),
            ("fetch('blob:https://example.com/x')", "a blob: URL"),
            ("fetch('/x', { body: 7, method: 'POST' })", "not a string"),
            ("fetch('/x', { signal: {} })", "'signal'"),
            ("fetch('/x', { cache: 'no-store' })", "'cache'"),
            ("fetch('/x', { keepalive: true })", "outlives its document"),
            ("fetch('/x', { headers: [['a', 'b']] })", "list of pairs"),
            (
                "fetch('/x', { headers: { a: {} } })",
                "whose value is an object",
            ),
        ] {
            let seen = page.run(call);
            assert!(
                seen.starts_with("! ") && seen.contains(said),
                "{call}: {seen}"
            );
        }
        assert!(page.asks().is_empty());
    })
    .expect("the pages are made");
}

#[test]
fn a_body_is_read_once_and_a_second_read_rejects() {
    both(|page| {
        page.run(
            "var seen = '';
             fetch('/t').then(function (r) {
               return r.text().then(function (t) {
                 seen = t + ' ' + r.bodyUsed;
                 return r.text();
               });
             }).catch(function (e) { seen = seen + ' / ' + e.name + ': ' + e.message; });",
        );
        let asks = page.asks();
        assert_eq!(Ok(true), page.deliver(asks[0].number, Some(basic("once"))));
        assert_eq!(
            page.run("seen"),
            "once true / TypeError: this response's body has already been read"
        );
    })
    .expect("the pages are made");
}

#[test]
fn an_answer_nothing_waits_for_is_none_and_touches_nothing() {
    both(|page| {
        assert_eq!(Ok(false), page.deliver(0, None));
        assert_eq!(Ok(false), page.deliver(41, Some(basic("x"))));
        assert!(page.reported.is_empty());
    })
    .expect("the pages are made");
}
