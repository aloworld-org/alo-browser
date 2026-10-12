/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 472: `FormData` and `URLSearchParams`, and either as the body
//! of a fetch or a beacon (ADR 0051 § 7).
//!
//! The contact form's script sends `new URLSearchParams(new FormData(form))`
//! by `fetch`. Each value, body and error name here is what Chrome 154
//! answered for the same script on the same markup, measured over its
//! debugging protocol; the words of an error are Chrome's only for a pair
//! that is not one and a symbol among a record's names, and this engine's
//! own elsewhere. Every script runs twice, the second time with the
//! collector at every allocation: `URLSearchParams`
//! walks a sequence by calling the page's script at every turn, and keeps
//! what it has in hand in its slots and its instance between them.
//!
//! This engine has no `Symbol`, `String` or `querySelector` for a script to
//! name yet (queue item 73 and ADR 0017 § 8), so each page is given
//! `ITERATOR`, the realm's own `ITERATOR`, and `q(selector)`, the
//! first of `querySelectorAll`'s answer.

use alo_bindings::fetching::{self, Asked};
use alo_bindings::{Identity, adopt, install, introduce, navigating, offer};
use alo_dom::parse_document;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::object::symbol::WellKnown;
use alo_js::object::{Property, Value};
use alo_js::script;
use alo_net::cors::Mode;

/// The contact form's markup, as alo Sites publishes it, with a second form
/// beside it and a file field in a third.
const MARKUP: &str = "<!DOCTYPE html><body>\
    <form id=contact action=\"/f/f4K9sL2wN7qR5tYx8vB1cA\" method=post>\
    <p class=hp><input id=website name=website type=text></p>\
    <p><input id=name name=name type=text required></p>\
    <p><input id=email name=email type=email required></p>\
    <p><textarea id=message name=message required></textarea></p>\
    <p><button id=send type=submit name=go value=1>Send</button>\
    <button id=plain type=button>Plain</button><label id=label>L</label></p>\
    </form>\
    <form id=other><button id=elsewhere>Other</button></form>\
    <form id=files><input name=up type=file></form>\
    <div id=out></div>";

/// An engine with a page's document at `https://example.com/start`.
struct Page {
    engine: Engine,
    _root: Root,
    cell: Ref,
}

impl Page {
    fn new(stress: bool) -> Result<Self, String> {
        let mut engine = alo_bindings::engine(None).map_err(|why| why.to_string())?;
        let cell =
            adopt(engine.objects(), parse_document(MARKUP)).map_err(|why| why.to_string())?;
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
        let global = engine.global().map_err(|why| why.to_string())?;
        let (intrinsics, objects) = engine.intrinsics();
        let iterator = intrinsics
            .well_known(objects, WellKnown::Iterator)
            .map_err(|why| why.to_string())?;
        let name: Vec<u16> = "ITERATOR".encode_utf16().collect();
        let property = Property::data(Value::Symbol(iterator), false, false, false);
        engine
            .objects()
            .define_named(global, &name, property)
            .map_err(|why| format!("{why:?}"))?;
        engine.objects().heap_mut().stress(stress);
        let mut page = Self {
            engine,
            _root: root,
            cell,
        };
        page.run("function q(selector) { return document.querySelectorAll(selector)[0]; }");
        Ok(page)
    }

    /// What `source` evaluates to, as text: a string as itself, a throw as
    /// `! ` and what it said.
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
        let _ = self.engine.checkpoint(&mut |_, _, _| {});
        answer
    }

    /// Every ask the page has made since the last take.
    fn asks(&mut self) -> Vec<Asked> {
        fetching::take(self.engine.objects(), self.cell).unwrap_or_default()
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

/// Fill the contact form as the measurement did.
const FILLED: &str = "var form = q('#contact');\
    q('#name').setAttribute('value', 'Ada Lovelace');\
    q('#email').setAttribute('value', 'ada@example.com');\
    q('#message').textContent = 'Two kilos a week, please.\\nThanks & é';";

#[test]
fn the_contact_forms_body_is_the_one_chrome_sent() {
    both(|page| {
        page.run(FILLED);
        // Chrome 154 posted exactly this for the same fields.
        assert_eq!(
            page.run("new URLSearchParams(new FormData(form)).toString()"),
            "website=&name=Ada+Lovelace&email=ada%40example.com\
             &message=Two+kilos+a+week%2C+please.%0AThanks+%26+%C3%A9"
        );
    })
    .expect("the pages are made");
}

#[test]
fn a_form_data_walks_its_entries_in_tree_order_as_pairs() {
    both(|page| {
        page.run(FILLED);
        assert_eq!(
            page.run(
                "var out = ''; for (var pair of new FormData(form)) { \
                 out = out + pair[0] + '=' + pair[1] + '(' + pair.length + ')|'; } out"
            ),
            "website=(2)|name=Ada Lovelace(2)|email=ada@example.com(2)|\
             message=Two kilos a week, please.\nThanks & é(2)|"
        );
        // The submitter is sent only when given, and then in tree order.
        assert_eq!(
            page.run(
                "var out = ''; for (var pair of new FormData(form, q('#send'))) \
                 { out = out + pair[0] + ','; } out"
            ),
            "website,name,email,message,go,"
        );
        assert_eq!(
            page.run(
                "var made = new FormData(); var out = 'none'; \
                 for (var pair of made) { out = 'some'; } out"
            ),
            "none"
        );
    })
    .expect("the pages are made");
}

#[test]
fn its_iterator_is_web_idls_and_ends_for_good() {
    both(|page| {
        page.run(FILLED);
        assert_eq!(
            page.run("'' + (FormData.prototype[ITERATOR] === FormData.prototype.entries)"),
            "true"
        );
        assert_eq!(
            page.run(
                "var it = new FormData(form).entries(); \
                 '' + (it.__proto__.__proto__ === [].values().__proto__.__proto__) + \
                 (it[ITERATOR]() === it)"
            ),
            "truetrue"
        );
        assert_eq!(
            page.run(
                "var it = new FormData(form).entries(); var a = it.next(); var b = it.next(); \
                 it.next(); it.next(); var e = it.next(); var f = it.next(); \
                 '' + a.done + a.value[0] + (a.value !== b.value) + e.done + e.value + f.done"
            ),
            "falsewebsitetruetrueundefinedtrue"
        );
        assert_eq!(
            page.run(
                "try { new FormData(form).entries().next.call({}); 'no' } \
                 catch (e) { e.name + ': ' + e.message }"
            ),
            "TypeError: 'next' was called on something that is not a FormData Iterator"
        );
    })
    .expect("the pages are made");
}

#[test]
fn a_form_data_refuses_what_chrome_refuses() {
    both(|page| {
        for (call, said) in [
            (
                "FormData()",
                "TypeError: 'FormData' is a constructor, and needs 'new'",
            ),
            (
                "new FormData(document.body)",
                "TypeError: argument 1 to 'FormData' is not a <form>",
            ),
            (
                "new FormData(null)",
                "TypeError: argument 1 to 'FormData' is not a <form>",
            ),
            (
                "new FormData({})",
                "TypeError: argument 1 to 'FormData' is not a <form>",
            ),
            (
                "new FormData(q('#contact'), q('#label'))",
                "TypeError: the submitter given to 'FormData' is not a submit button",
            ),
            (
                "new FormData(q('#contact'), q('#plain'))",
                "TypeError: the submitter given to 'FormData' is not a submit button",
            ),
            (
                "new FormData(q('#contact'), {})",
                "TypeError: argument 2 to 'FormData' is not an HTMLElement of the form's document",
            ),
            (
                "new FormData(q('#contact'), \
                 q('#elsewhere'))",
                "NotFoundError: the submitter given to 'FormData' is not owned by its form",
            ),
        ] {
            assert_eq!(
                page.run(&format!(
                    "try {{ {call}; 'made' }} catch (e) {{ e.name + ': ' + e.message }}"
                )),
                said,
                "{call}"
            );
        }
    })
    .expect("the pages are made");
}

#[test]
fn what_it_cannot_hand_over_is_refused_by_name() {
    both(|page| {
        let seen = page.run("new FormData(q('#files'))");
        assert!(
            seen.starts_with("! ") && seen.contains("File API"),
            "{seen}"
        );
        // A `formdata` listener anywhere on the event's path would go
        // unheard, so the list is not made.
        for listener in ["q('#contact')", "document", "window"] {
            let mut page = Page::new(false).expect("a page");
            let seen = page.run(&format!(
                "{listener}.addEventListener('formdata', function () {{}}); \
                 new FormData(q('#contact'))"
            ));
            assert!(
                seen.starts_with("! ") && seen.contains("queue item 491"),
                "{listener}: {seen}"
            );
        }
        // One for another event, or on another form, hears nothing of it.
        assert_eq!(
            page.run(
                "q('#other').addEventListener('formdata', function () {}); \
                 document.addEventListener('submit', function () {}); \
                 typeof new FormData(q('#contact'))"
            ),
            "object"
        );
    })
    .expect("the pages are made");
}

#[test]
fn a_string_is_parsed_after_one_question_mark() {
    both(|page| {
        for (init, written) in [
            (
                "'?a=b+c&d=%zz&&e&=f&g=%C3%A9%2B'",
                "a=b+c&d=%25zz&e=&=f&g=%C3%A9%2B",
            ),
            ("'??a=1'", "%3Fa=1"),
            ("null", "null="),
            ("undefined", ""),
            ("12", "12="),
            ("'a=%F0%9F%98%80&b=%FF'", "a=%F0%9F%98%80&b=%EF%BF%BD"),
        ] {
            assert_eq!(
                page.run(&format!("new URLSearchParams({init}).toString()")),
                written,
                "{init}"
            );
        }
        assert_eq!(page.run("new URLSearchParams().toString()"), "");
        assert_eq!(
            page.run("try { URLSearchParams() } catch (e) { e.name + ': ' + e.message }"),
            "TypeError: 'URLSearchParams' is a constructor, and needs 'new'"
        );
    })
    .expect("the pages are made");
}

#[test]
fn a_record_is_its_own_enumerable_names_in_order() {
    both(|page| {
        assert_eq!(
            page.run("new URLSearchParams({ b: 'x y', a: 1, 2: true, 1: null }).toString()"),
            "1=null&2=true&b=x+y&a=1"
        );
        assert_eq!(
            page.run(
                "var o = { a: 1 }; o.__proto__ = { inherited: 2 }; \
                 new URLSearchParams(o).toString()"
            ),
            "a=1"
        );
        // A function's own `prototype` is not enumerable, so it is not read.
        assert_eq!(
            page.run("function f() {} f.a = 1; new URLSearchParams(f).toString()"),
            "a=1"
        );
        // Two names that are one `USVString` set the first's value, and a
        // sequence keeps both, as Chrome 154 does.
        assert_eq!(
            page.run(
                "var o = {}; o['\\uD800'] = 1; o.b = 2; o['\\uDC00'] = 3; \
                 new URLSearchParams(o).toString() + ' | ' + \
                 new URLSearchParams([['\\uD800', '1'], ['\\uDC00', '2']]).toString()"
            ),
            "%EF%BF%BD=3&b=2 | %EF%BF%BD=1&%EF%BF%BD=2"
        );
        // An `undefined` `[Symbol.iterator]` makes it a record, and a symbol
        // among a record's enumerable names is Chrome 154's `TypeError`.
        assert_eq!(
            page.run(
                "var o = { a: 1 }; o[ITERATOR] = undefined; \
                 try { new URLSearchParams(o) } catch (e) { e.name + ': ' + e.message }"
            ),
            "TypeError: Cannot convert a Symbol value to a string"
        );
        let seen = page.run("new URLSearchParams({ a: {} })");
        assert!(
            seen.starts_with("! ") && seen.contains("is an object"),
            "{seen}"
        );
    })
    .expect("the pages are made");
}

/// A script whose iterable writes down every step of the protocol: each
/// getter and call, each `toString`, in order.
const LOGGED: &str = "var log = '';\
    function pairOf(a, b) { var items = [a, b]; var at = 0; var it = {}; \
      it[ITERATOR] = function () { log = log + 'I'; return { \
        get next() { log = log + 'n'; return function () { \
          var i = at; at = at + 1; \
          return { get done() { log = log + 'd'; return i >= items.length; }, \
                   get value() { log = log + 'v'; return items[i]; } }; }; } }; }; \
      return it; }\
    var outer = {}; outer[ITERATOR] = function () { log = log + 'O'; var k = 0; \
      var all = [pairOf('a', { toString: function () { log = log + 's'; return 'b c'; } }), \
                 pairOf('d', 'e')]; \
      return { next: function () { log = log + 'N'; var i = k; k = k + 1; \
        return i < all.length ? { done: false, value: all[i] } : { done: true }; } }; };";

#[test]
fn a_sequence_is_walked_one_call_at_a_time_in_web_idls_order() {
    both(|page| {
        page.run(LOGGED);
        assert_eq!(
            page.run("new URLSearchParams(outer).toString()"),
            "a=b+c&d=e"
        );
        // For each pair: its iterator, its `next` read once, then for each
        // string `done` and `value` — the object's `toString` after its
        // `value` — and a last `done` that ends it.
        assert_eq!(page.run("log"), "ONIndvdvsdNIndvdvdN");
        assert_eq!(
            page.run("new URLSearchParams([['x', 1], ['y', 'z']]).toString()"),
            "x=1&y=z"
        );
    })
    .expect("the pages are made");
}

#[test]
fn an_item_that_is_not_a_pair_is_chromes_type_error() {
    both(|page| {
        for (init, said) in [
            (
                "[['a']]",
                "Sequence initializer must only contain pair elements",
            ),
            (
                "[['a', 'b', 'c']]",
                "Sequence initializer must only contain pair elements",
            ),
            (
                "[1]",
                "The provided value cannot be converted to a sequence.",
            ),
            (
                "[{}]",
                "The provided value cannot be converted to a sequence.",
            ),
        ] {
            assert_eq!(
                page.run(&format!(
                    "try {{ new URLSearchParams({init}); 'made' }} \
                     catch (e) {{ e.name + ': ' + e.message }}"
                )),
                format!("TypeError: {said}"),
                "{init}"
            );
        }
        // A symbol among a pair's strings is `ToString`'s own `TypeError`.
        assert_eq!(
            page.run("try { new URLSearchParams([[ITERATOR, 'b']]) } catch (e) { e.name }"),
            "TypeError"
        );
        // Every item of a pair too long is converted before it is refused.
        assert_eq!(
            page.run(
                "var made = 0; var counted = { toString: function () { made = made + 1; \
                 return 'x'; } }; try { new URLSearchParams([[counted, counted, counted, \
                 counted]]) } catch (e) { } '' + made"
            ),
            "4"
        );
        // A throw on the way is the page's own, not wrapped.
        assert_eq!(
            page.run(
                "var bad = {}; bad[ITERATOR] = function () { throw new RangeError('mine'); }; \
                 try { new URLSearchParams(bad) } catch (e) { e.name + ': ' + e.message }"
            ),
            "RangeError: mine"
        );
    })
    .expect("the pages are made");
}

#[test]
fn a_fetch_sends_one_as_chrome_does() {
    both(|page| {
        page.run(FILLED);
        page.run(
            "fetch(form.getAttribute('action'), { method: 'POST', \
             body: new URLSearchParams(new FormData(form)) })",
        );
        let asks = page.asks();
        assert_eq!(asks.len(), 1);
        let ask = &asks[0];
        assert_eq!(ask.method, "POST");
        assert_eq!(
            ask.url.serialised,
            "https://example.com/f/f4K9sL2wN7qR5tYx8vB1cA"
        );
        assert_eq!(
            ask.headers,
            [(
                "Content-Type".to_owned(),
                "application/x-www-form-urlencoded;charset=UTF-8".to_owned()
            )]
        );
        assert_eq!(
            String::from_utf8_lossy(&ask.body),
            "website=&name=Ada+Lovelace&email=ada%40example.com\
             &message=Two+kilos+a+week%2C+please.%0AThanks+%26+%C3%A9"
        );
        // A type the page set is kept, as for a string.
        page.run(
            "fetch('/x', { method: 'POST', headers: { 'content-type': 'text/x-mine' }, \
             body: new URLSearchParams('a=1') })",
        );
        let asks = page.asks();
        assert_eq!(
            asks[0].headers,
            [("content-type".to_owned(), "text/x-mine".to_owned())]
        );
        assert_eq!(asks[0].body, b"a=1");
        let seen = page.run("fetch('/x', { method: 'POST', body: new FormData(form) })");
        assert!(
            seen.starts_with("! ") && seen.contains("queue item 492"),
            "{seen}"
        );
        assert!(page.asks().is_empty());
    })
    .expect("the pages are made");
}

#[test]
fn a_beacon_sends_one_without_running_its_to_string() {
    both(|page| {
        page.run("URLSearchParams.prototype.toString = function () { return 'mine'; };");
        assert_eq!(
            page.run("'' + navigator.sendBeacon('/b', new URLSearchParams({ c: 'f 4' }))"),
            "true"
        );
        let asks = page.asks();
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0].body, b"c=f+4");
        assert_eq!(asks[0].mode, Mode::NoCors, "the type is safelisted");
        assert_eq!(
            asks[0].headers,
            [(
                "Content-Type".to_owned(),
                "application/x-www-form-urlencoded;charset=UTF-8".to_owned()
            )]
        );
        let seen = page.run("navigator.sendBeacon('/b', new FormData())");
        assert!(
            seen.starts_with("! ") && seen.contains("queue item 492"),
            "{seen}"
        );
        assert!(page.asks().is_empty());
    })
    .expect("the pages are made");
}

#[test]
fn class_name_and_replace_with_put_a_paragraph_where_the_form_was() {
    both(|page| {
        // The contact form's success path, on the frozen markup.
        assert_eq!(
            page.run(
                "var form = q('#contact'); \
                 var done = document.createElement('p'); \
                 var before = '[' + done.className + ']'; \
                 done.className = 'form-success'; done.textContent = 'Thanks — talk soon.'; \
                 form.replaceWith(done); \
                 before + done.className + '|' + done.getAttribute('class') + '|' + \
                 (q('#contact') === undefined) + '|' + \
                 q('.form-success').textContent"
            ),
            "[]form-success|form-success|true|Thanks — talk soon."
        );
        assert_eq!(
            page.run("var p = document.createElement('p'); p.className = null; p.className"),
            "null"
        );
        assert_eq!(
            page.run(
                "var out = q('#out'); out.textContent = ''; \
                 var i = document.createElement('i'); out.appendChild(i); \
                 var b = document.createElement('b'); b.setAttribute('id', 'made'); \
                 i.replaceWith('t', b, 'u'); \
                 out.textContent + out.firstChild.nextSibling.getAttribute('id') + \
                 (out.lastChild.previousSibling === b) + (i.parentNode === null)"
            ),
            "tumadetruetrue"
        );
        assert_eq!(
            page.run(
                "var i = document.createElement('i'); q('#out').appendChild(i); \
                 try { i.replaceWith(document.body) } catch (e) { e.name }"
            ),
            "HierarchyRequestError"
        );
    })
    .expect("the pages are made");
}
