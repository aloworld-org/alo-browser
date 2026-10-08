/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 327: `getElementById`, `hidden` and `classList` — what
//! `alo-downloads`' script marks its card with.
//!
//! Each row is a script and what the standards say it answers, and most
//! answer the attribute they left too, since a member that answered right
//! and wrote the wrong `class` is the failure the render shows. Every
//! script runs twice, the second time with the collector running at every
//! allocation, and the two must agree.

use alo_bindings::{adopt, document, install};
use alo_dom::parse_document;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::numeric;
use alo_js::object::Value;
use alo_js::script;

const PAGE: &str = "<!DOCTYPE html><html><head></head><body>\
                    <div id=card class='card  card'>\
                    <span id=badge hidden>Your device</span></div>\
                    <p id=dup>first</p><p id=dup>second</p>\
                    <p id=''>empty</p>\
                    <template><i id=inside></i></template>\
                    <svg><g id=drawn class=a></g></svg>\
                    </body></html>";

/// An engine with [`PAGE`] installed.
struct Page {
    engine: Engine,
    _root: Root,
    cell: Ref,
}

impl Page {
    fn new(stress: bool) -> Result<Self, String> {
        let mut engine = Engine::new().map_err(|why| why.to_string())?;
        let cell = adopt(engine.objects(), parse_document(PAGE)).map_err(|why| why.to_string())?;
        let root = engine.objects().heap_mut().root(cell);
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
            Ok(Value::Number(number)) => numeric::text_of(number),
            Ok(Value::Null) => "null".to_owned(),
            Ok(Value::Undefined) => "undefined".to_owned(),
            Ok(other) => format!("{other:?}"),
            Err(Trouble::Escaped(escape)) => format!("! {escape}"),
            Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
        }
    }

    /// The change count of the document.
    fn changes(&mut self) -> u64 {
        document(self.engine.objects(), self.cell).map_or(0, alo_dom::Document::change_count)
    }
}

/// Run `source` on [`PAGE`] both ways, and answer what it answered and how
/// many changes it made to the document.
fn both_ways(source: &str) -> (String, u64) {
    let mut answers = [false, true].map(|stress| match Page::new(stress) {
        Ok(mut page) => {
            let before = page.changes();
            let answer = page.run(source);
            (answer, page.changes().saturating_sub(before))
        }
        Err(why) => (format!("! no page: {why}"), 0),
    });
    let [plain, stressed] = &mut answers;
    assert_eq!(plain, stressed, "{source}");
    core::mem::take(plain)
}

/// What `source` answers on [`PAGE`].
fn answer(source: &str) -> String {
    both_ways(source).0
}

/// A script that returns what `body` threw, by name, or `nothing`.
fn thrown(body: &str) -> String {
    answer(&format!(
        "(function () {{ try {{ {body}; return 'nothing'; }} catch (e) {{ \
         return e.name + (Error.prototype.isPrototypeOf(e) ? '' : ' (not an Error)'); }} }})()"
    ))
}

#[test]
fn get_element_by_id_answers_the_first_in_tree_order_or_null() {
    for (source, expected) in [
        (
            "document.getElementById('card').className === undefined",
            "true",
        ),
        (
            "document.getElementById('card') === document.documentElement.lastChild.firstChild",
            "true",
        ),
        ("document.getElementById('dup').textContent", "first"),
        ("document.getElementById('nothing')", "null"),
        // An element whose `id` is empty has no ID.
        ("document.getElementById('')", "null"),
        // Exactly, not case-insensitively and not trimmed.
        ("document.getElementById('CARD')", "null"),
        ("document.getElementById(' card')", "null"),
        // A template's contents are not the document's descendants.
        ("document.getElementById('inside')", "null"),
        // An element in another namespace has an ID too.
        (
            "document.getElementById('drawn') === \
             document.getElementById('card').parentNode.lastChild.firstChild",
            "true",
        ),
        // `DOMString`: an object's own `toString` runs.
        (
            "document.getElementById({ toString: function () { return 'badge'; } }).textContent",
            "Your device",
        ),
        // One wrapper per node.
        (
            "document.getElementById('card') === document.getElementById('card')",
            "true",
        ),
    ] {
        assert_eq!(answer(source), expected, "{source}");
    }
    assert_eq!(
        thrown("document.getElementById()"),
        "TypeError",
        "too few arguments"
    );
    assert_eq!(
        thrown("document.getElementById.call(document.body, 'card')"),
        "TypeError",
        "the brand check"
    );
}

#[test]
fn hidden_reflects_its_attribute_as_html_says() {
    let badge = "var b = document.getElementById('badge');";
    for (source, expected) in [
        ("b.hidden", "true"),
        ("document.getElementById('card').hidden", "false"),
        ("b.hidden = false; b.getAttribute('hidden')", "null"),
        ("b.hidden = false; b.hidden", "false"),
        ("b.hidden = true; b.getAttribute('hidden')", ""),
        (
            "b.setAttribute('hidden', 'UNTIL-Found'); typeof b.hidden + ' ' + b.hidden",
            "string until-found",
        ),
        ("b.setAttribute('hidden', 'other'); b.hidden", "true"),
        (
            "b.hidden = 'until-FOUND'; b.getAttribute('hidden')",
            "until-found",
        ),
        ("b.hidden = 'yes'; b.getAttribute('hidden')", ""),
        ("b.hidden = ''; b.getAttribute('hidden')", "null"),
        ("b.hidden = null; b.getAttribute('hidden')", "null"),
        ("b.hidden = undefined; b.getAttribute('hidden')", "null"),
        ("b.hidden = 0; b.getAttribute('hidden')", "null"),
        ("b.hidden = -0; b.getAttribute('hidden')", "null"),
        ("b.hidden = NaN; b.getAttribute('hidden')", "null"),
        ("b.hidden = 2; b.getAttribute('hidden')", ""),
        ("b.hidden = 'false'; b.getAttribute('hidden')", ""),
        (
            "b.hidden = { toString: function () { return 'until-found'; } }; \
             b.getAttribute('hidden')",
            "until-found",
        ),
        ("b.hidden = {}; b.getAttribute('hidden')", ""),
    ] {
        assert_eq!(answer(&format!("{badge} {source}")), expected, "{source}");
    }
    // An SVG element is not an HTMLElement, so it has no `hidden`.
    assert_eq!(
        answer("document.getElementById('drawn').hidden"),
        "undefined"
    );
    // Taking away an attribute that was not there changes nothing.
    assert_eq!(
        both_ways("document.getElementById('card').hidden = false"),
        ("false".to_owned(), 0)
    );
}

#[test]
fn class_list_is_one_live_set_over_the_class_attribute() {
    let card = "var c = document.getElementById('card'); var l = c.classList;";
    for (source, expected) in [
        ("l === c.classList", "true"),
        ("l.length", "1"),
        ("l.value", "card  card"),
        ("'' + l", "card  card"),
        ("l.toString()", "card  card"),
        ("l.contains('card')", "true"),
        ("l.contains('Card')", "false"),
        // `contains` validates nothing.
        ("l.contains('')", "false"),
        ("l.contains('a b')", "false"),
        // The page's own `classList.add`, which rewrites the set once.
        ("l.add('rec'); c.getAttribute('class')", "card rec"),
        ("l.add('rec', 'rec', 'b'); l.value", "card rec b"),
        ("l.add(); c.getAttribute('class')", "card"),
        ("l.remove('card'); c.getAttribute('class')", ""),
        ("l.remove('nothing'); c.getAttribute('class')", "card"),
        ("l.toggle('card')", "false"),
        ("l.toggle('card'); c.getAttribute('class')", ""),
        ("l.toggle('rec')", "true"),
        ("l.toggle('rec'); c.getAttribute('class')", "card rec"),
        (
            "l.toggle('card', true) + ' ' + c.getAttribute('class')",
            "true card  card",
        ),
        (
            "l.toggle('rec', false) + ' ' + c.getAttribute('class')",
            "false card  card",
        ),
        (
            "l.toggle('card', false) + ' ' + c.getAttribute('class')",
            "false ",
        ),
        (
            "l.toggle('rec', 1) + ' ' + c.getAttribute('class')",
            "true card rec",
        ),
        (
            "l.toggle('rec', undefined) + ' ' + l.value",
            "true card rec",
        ),
        // Live: `setAttribute` is seen at once, and so is a removal.
        (
            "c.setAttribute('class', 'x y x'); l.length + ' ' + l.contains('y')",
            "2 true",
        ),
        ("c.removeAttribute('class'); l.length + ' ' + l.value", "0 "),
        (
            "l.value = ' a  b '; c.getAttribute('class') + '|' + l.length",
            " a  b |2",
        ),
        // An object token's own `toString` runs, beside primitives.
        (
            "l.add(1, { toString: function () { return 'o'; } }, true); l.value",
            "card 1 o true",
        ),
        // Expandos are kept, and the list is not a node.
        ("l.mine = 4; c.classList.mine", "4"),
        (
            "typeof l.item + typeof l.replace + typeof l.forEach",
            "undefinedundefinedundefined",
        ),
        // `[PutForwards=value]` is queue item 328's: assigning is ignored.
        ("c.classList = 'z'; c.getAttribute('class')", "card  card"),
    ] {
        assert_eq!(answer(&format!("{card} {source}")), expected, "{source}");
    }
}

#[test]
fn class_list_writes_no_attribute_an_element_did_not_have_for_an_empty_set() {
    let badge = "var b = document.getElementById('badge'); var l = b.classList;";
    assert_eq!(
        both_ways(&format!("{badge} l.remove('x'); b.getAttribute('class')")),
        ("null".to_owned(), 0)
    );
    assert_eq!(
        both_ways(&format!("{badge} l.add(); b.getAttribute('class')")),
        ("null".to_owned(), 0)
    );
    assert_eq!(
        both_ways(&format!(
            "{badge} l.toggle('x', false); b.getAttribute('class')"
        )),
        ("null".to_owned(), 0)
    );
    // An element that has one, even empty, has it rewritten.
    assert_eq!(
        both_ways(&format!(
            "{badge} b.setAttribute('class', '  '); l.remove('x'); b.getAttribute('class')"
        )),
        (String::new(), 2)
    );
    // `toggle(token, true)` on a token already there runs no update.
    assert_eq!(
        both_ways("document.getElementById('card').classList.toggle('card', true)"),
        ("true".to_owned(), 0)
    );
}

#[test]
fn each_refusal_is_the_dom_exception_the_standard_names() {
    let list = "var l = document.getElementById('card').classList;";
    for (body, expected) in [
        ("l.add('')", "SyntaxError"),
        ("l.add('a b')", "InvalidCharacterError"),
        ("l.add('a\\tb')", "InvalidCharacterError"),
        ("l.add('a\\fb')", "InvalidCharacterError"),
        ("l.remove('')", "SyntaxError"),
        ("l.remove('x', 'a\\nb')", "InvalidCharacterError"),
        ("l.toggle('')", "SyntaxError"),
        ("l.toggle(' ')", "InvalidCharacterError"),
        // The first refused token decides.
        ("l.add('a b', '')", "InvalidCharacterError"),
        // A no-break space is not ASCII whitespace.
        ("l.add('a\\u00a0b')", "nothing"),
        ("l.toggle()", "TypeError"),
        ("l.contains()", "TypeError"),
    ] {
        assert_eq!(thrown(&format!("{list} {body}")), expected, "{body}");
    }
    // Nothing is written when any token is refused, even a valid one before
    // it.
    assert_eq!(
        both_ways(&format!(
            "{list} try {{ l.add('ok', ''); }} catch (e) {{}} l.value"
        )),
        ("card  card".to_owned(), 0)
    );
}

#[test]
fn every_member_checks_its_brand() {
    for body in [
        "var o = {}; o.add = document.body.classList.add; o.add('x')",
        "var l = document.body.classList; l.contains.call({}, 'x')",
        "var l = document.body.classList; l.toggle.call(document.body, 'x')",
        "var l = document.body.classList; l.remove.call(l.add, 'x')",
        "var l = document.body.classList; l.toString.call(1)",
    ] {
        assert_eq!(thrown(body), "TypeError", "{body}");
    }
    // `classList` itself is an `Element`'s, and a text node is not one.
    assert_eq!(
        answer("document.getElementById('badge').firstChild.classList"),
        "undefined"
    );
}

#[test]
fn a_second_object_among_the_tokens_is_refused_by_name() {
    let answered = answer(
        "var o = { toString: function () { return 'o'; } }; \
         document.body.classList.add(o, o)",
    );
    assert!(
        answered.starts_with("! ") && answered.contains("queue item 221"),
        "{answered}"
    );
}

#[test]
fn the_list_keeps_its_element_through_a_collection() {
    assert_eq!(
        answer(
            "var l = (function () { var d = document.createElement('div'); \
             d.classList.add('kept'); return d.classList; })(); \
             for (var i = 0; i < 200; i++) { var junk = { i: i }; } l.value"
        ),
        "kept"
    );
}

#[test]
fn a_hostile_class_is_answered_not_crashed_on() {
    // Twenty thousand copies of one token, every kind of ASCII whitespace
    // between them, and a token as long as the rest.
    assert_eq!(
        answer(
            "var c = document.getElementById('card'); var s = ''; \
             for (var i = 0; i < 20000; i++) s += 'x\\t\\n\\f\\r '; \
             var long = ''; for (var j = 0; j < 20000; j++) long += 'y'; \
             c.setAttribute('class', s + long); \
             c.classList.add('z'); \
             c.classList.length + ' ' + c.classList.contains('x') + ' ' + \
             c.classList.contains(long) + ' ' + (c.getAttribute('class') === 'x ' + long + ' z')"
        ),
        "3 true true true"
    );
}
