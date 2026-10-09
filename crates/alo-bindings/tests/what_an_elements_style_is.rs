/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 342: `el.style`, a `CSSStyleDeclaration` over the `style`
//! attribute (ADR 0033 §§ 3–6) — what `alo-downloads`' script greys its
//! buttons with.
//!
//! Each row is a script and what it answers, and most answer the attribute
//! they left too, since the attribute is the only copy of the block and is
//! what the cascade reads. Every script runs twice, the second time with the
//! collector running at every allocation, and the two must agree — in what
//! they answer and in how many changes they made to the document.

use alo_bindings::{adopt, document, install};
use alo_dom::parse_document;
use alo_js::heap::{Ref, Root};
use alo_js::interpret::{Engine, Trouble};
use alo_js::numeric;
use alo_js::object::Value;
use alo_js::script;

const PAGE: &str = "<!DOCTYPE html><html><head></head><body>\
                    <p id=plain>no style</p>\
                    <p id=styled style='color:red;margin: 0 4px'>styled</p>\
                    <svg><rect id=rect style='fill: blue'></rect></svg>\
                    <math><mi id=mi>x</mi></math>\
                    </body></html>";

/// An engine with [`PAGE`] installed.
struct Page {
    engine: Engine,
    _root: Root,
    cell: Ref,
}

impl Page {
    fn new(stress: bool) -> Result<Self, String> {
        let mut engine = alo_bindings::engine(None).map_err(|why| why.to_string())?;
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

/// The script `body` with `p` the element with no `style`, `q` the one with
/// one, and `s` and `t` their declarations.
fn on(body: &str) -> String {
    answer(&format!(
        "var p = document.getElementById('plain'); var s = p.style; \
         var q = document.getElementById('styled'); var t = q.style; {body}"
    ))
}

#[test]
fn style_is_one_object_on_html_and_svg_elements_and_on_no_other() {
    for (source, expected) in [
        (
            "var p = document.getElementById('plain'); p.style === p.style",
            "true",
        ),
        (
            "var r = document.getElementById('rect'); r.style === r.style",
            "true",
        ),
        (
            "document.getElementById('plain').style === document.getElementById('styled').style",
            "false",
        ),
        // A MathML element has no `style`: this engine builds no MathML.
        ("document.getElementById('mi').style", "undefined"),
        (
            "document.getElementById('plain').firstChild.style",
            "undefined",
        ),
        // On the prototype, not the element.
        (
            "document.getElementById('plain').hasOwnProperty('style')",
            "false",
        ),
        // An SVG element's chain is `SVGElement`'s, not `HTMLElement`'s:
        // it has no `click`, and its `style` is its own interface's.
        (
            "typeof document.getElementById('rect').click + ' ' + \
             typeof document.getElementById('plain').click",
            "undefined function",
        ),
        (
            "var r = document.getElementById('rect'); r.style.fill + ' ' + r.style.length",
            "blue 1",
        ),
        ("document.body.style.parentRule", "null"),
    ] {
        assert_eq!(answer(source), expected, "{source}");
    }
}

#[test]
fn a_named_attribute_reads_and_writes_the_attribute() {
    for (body, expected) in [
        // What `alo-downloads`' `mark(a)` writes, read back as written.
        (
            "s.background = '#c7bfb2'; s.background + ' | ' + p.getAttribute('style')",
            "#c7bfb2 | background: #c7bfb2;",
        ),
        // Camel-cased and dashed are the same property.
        (
            "s.backgroundColor = 'red'; s['background-color'] + ' ' + \
             s.getPropertyValue('background-color')",
            "red red",
        ),
        (
            "s['font-size'] = '12px'; s.fontSize + ' | ' + p.getAttribute('style')",
            "12px | font-size: 12px;",
        ),
        // A counted shorthand's longhands read through it.
        (
            "t.marginLeft + ' ' + t.marginTop + ' ' + t.color",
            "4px 0 red",
        ),
        // A shorthand read by kind stays whole, and does not answer its
        // longhands (ADR 0033 § 5).
        ("s.background = 'blue'; '[' + s.backgroundColor + ']'", "[]"),
        // `[LegacyNullToEmptyString]`: `null` is the empty string, a removal.
        (
            "t.color = null; t.color + '|' + q.getAttribute('style')",
            "|margin: 0 4px;",
        ),
        // A value is a `DOMString`, and an object's `toString` runs.
        (
            "s.color = { toString: function () { return 'green'; } }; s.color",
            "green",
        ),
        ("s.width = 5; s.width", "5"),
        // Setting a named attribute makes it normal, as `setProperty` with
        // no priority does.
        (
            "s.setProperty('color', 'red', 'important'); s.color = 'blue'; \
             s.getPropertyPriority('color') + '|' + s.color",
            "|blue",
        ),
    ] {
        assert_eq!(on(body), expected, "{body}");
    }
}

#[test]
fn a_property_this_engine_does_not_act_on_is_an_ordinary_property() {
    for (body, expected) in [
        ("'cursor' in s", "false"),
        ("'pointerEvents' in s", "false"),
        ("'backgroundColor' in s", "true"),
        ("'background-color' in s", "true"),
        // `alo-downloads`' last two writes land on the object.
        (
            "s.cursor = 'default'; s.pointerEvents = 'none'; \
             s.hasOwnProperty('cursor') + ' ' + s.cursor + ' ' + s.pointerEvents + ' ' + \
             p.getAttribute('style')",
            "true default none null",
        ),
        (
            "s.setProperty('cursor', 'default'); p.getAttribute('style')",
            "null",
        ),
        // A declaration the attribute holds is read whatever its name.
        (
            "p.setAttribute('style', 'cursor: default'); s.getPropertyValue('cursor') + ' ' + s.length",
            "default 1",
        ),
    ] {
        assert_eq!(on(body), expected, "{body}");
    }
}

#[test]
fn the_methods_are_cssoms_over_the_attribute() {
    for (body, expected) in [
        ("s.length + ' ' + t.length", "0 2"),
        (
            "t.item(0) + ' ' + t.item(1) + ' [' + t.item(2) + ']'",
            "color margin []",
        ),
        // `unsigned long`: modulo 2³², and an object's `valueOf` runs.
        ("t.item(4294967297)", "margin"),
        ("t.item({ valueOf: function () { return 1; } })", "margin"),
        ("t.item(-1) === ''", "true"),
        ("t.cssText", "color: red; margin: 0 4px;"),
        (
            "s.setProperty('COLOR', 'red', 'IMPORTANT'); \
             s.getPropertyValue('color') + ' ' + s.getPropertyPriority('Color') + ' | ' + s.cssText",
            "red important | color: red !important;",
        ),
        // A priority that is neither empty nor `important` is ignored.
        ("s.setProperty('color', 'red', 'high'); s.length", "0"),
        // An empty value removes.
        ("t.setProperty('color', ''); t.cssText", "margin: 0 4px;"),
        // Custom properties keep their case, are trimmed as a sheet's value
        // is, and have no named attribute.
        (
            "s.setProperty('--Brand', ' #fff'); s.getPropertyValue('--Brand') + '|' + \
             s.getPropertyValue('--brand') + '|' + s['--Brand']",
            "#fff||undefined",
        ),
        // `removeProperty` answers what it removed, and a shorthand takes its
        // longhands with it.
        (
            "t.marginLeft = '9px'; var was = t.removeProperty('margin'); \
             was + ' | ' + t.cssText",
            "0 4px | color: red;",
        ),
        (
            "t.removeProperty('width') + '|' + t.cssText",
            "|color: red; margin: 0 4px;",
        ),
        // Setting a shorthand removes its longhands first.
        (
            "s.backgroundColor = 'red'; s.background = 'blue'; s.cssText",
            "background: blue;",
        ),
        // The attribute is read every time.
        (
            "q.setAttribute('style', 'width: 3px'); t.width + ' ' + t.length",
            "3px 1",
        ),
        (
            "q.removeAttribute('style'); t.length + ' ' + t.cssText",
            "0 ",
        ),
    ] {
        assert_eq!(on(body), expected, "{body}");
    }
}

#[test]
fn css_text_and_assigning_style_replace_the_whole_attribute() {
    for (body, expected) in [
        (
            "t.cssText = 'width:1px;height : 2px'; t.width + ' ' + q.getAttribute('style')",
            "1px width: 1px; height: 2px;",
        ),
        // `[PutForwards=cssText]`.
        (
            "p.style = 'color: red'; (p.style === s) + ' ' + p.getAttribute('style')",
            "true color: red;",
        ),
        // Nothing left is an empty attribute, not none, as CSSOM's update
        // steps set it.
        ("t.cssText = ''; q.getAttribute('style') === ''", "true"),
        ("p.style = ''; p.getAttribute('style') === ''", "true"),
        // `cssText` is not `[LegacyNullToEmptyString]`.
        ("t.cssText = null; q.getAttribute('style')", ""),
        // What a sheet would drop is dropped.
        (
            "t.cssText = 'color: red; ; : 1; width: 2px'; t.cssText",
            "color: red; width: 2px;",
        ),
    ] {
        assert_eq!(on(body), expected, "{body}");
    }
}

#[test]
fn a_write_is_a_change_only_when_it_changes_the_block() {
    let prelude = "var p = document.getElementById('plain'); \
                   var q = document.getElementById('styled'); ";
    for (body, changes) in [
        ("p.style.color = 'red'", 1),
        ("q.style.color = 'red'", 0),
        ("q.style.setProperty('color', 'red', '')", 0),
        ("q.style.removeProperty('width')", 0),
        ("q.style.cursor = 'pointer'", 0),
        ("q.style.setProperty('color', 'red', 'nope')", 0),
        ("q.style.color = 'blue'", 1),
        ("q.style.removeProperty('color')", 1),
        // `cssText` always runs the update steps.
        ("q.style.cssText = q.style.cssText", 1),
        (
            "p.style.color = 'red'; p.style.color = 'red'; p.style.color = ''",
            2,
        ),
    ] {
        let source = format!("{prelude}{body}");
        let (answered, made) = both_ways(&source);
        assert!(!answered.starts_with('!'), "{body}: {answered}");
        assert_eq!(made, changes, "{body}");
    }
}

#[test]
fn every_member_checks_its_brand() {
    for body in [
        "var s = document.body.style; var o = { f: s.getPropertyValue }; o.f('color')",
        "var s = document.body.style; s.setProperty.call({}, 'color', 'red')",
        "var s = document.body.style; s.removeProperty.call(document.body, 'color')",
        "var s = document.body.style; s.item.call(document.body.classList, 0)",
        "var s = document.body.style; s.getPropertyPriority.call(1, 'color')",
        // A named accessor, reached through an object that inherits from a
        // declaration without being one.
        "function F() {} F.prototype = document.body.style; new F().color",
        "function F() {} F.prototype = document.body.style; new F().color = 'red'",
        "function F() {} F.prototype = document.body.style; new F().cssText",
        // `style` itself, on something that inherits from an element.
        "function F() {} F.prototype = document.body; new F().style",
        "function F() {} F.prototype = document.getElementById('rect'); new F().style",
    ] {
        assert_eq!(thrown(body), "TypeError", "{body}");
    }
    assert_eq!(
        thrown("document.body.style.setProperty('color')"),
        "TypeError"
    );
    assert_eq!(
        thrown("document.body.style.getPropertyValue()"),
        "TypeError"
    );
}

#[test]
fn a_second_object_among_set_propertys_arguments_is_refused_by_name() {
    let answered = answer(
        "var o = { toString: function () { return 'color'; } }; \
         document.body.style.setProperty(o, o)",
    );
    assert!(
        answered.starts_with("! ") && answered.contains("queue item 221"),
        "{answered}"
    );
    // One object is converted in full, wherever it is.
    assert_eq!(
        on("s.setProperty('color', { toString: function () { return 'red'; } }); s.color"),
        "red"
    );
    assert_eq!(
        on("s.setProperty({ toString: function () { return 'color'; } }, 'red'); s.color"),
        "red"
    );
}

#[test]
fn the_declaration_keeps_its_element_through_a_collection() {
    assert_eq!(
        answer(
            "var s = (function () { var d = document.createElement('div'); \
             d.style.color = 'red'; return d.style; })(); \
             for (var i = 0; i < 200; i++) { var junk = { i: i }; } s.cssText"
        ),
        "color: red;"
    );
}

#[test]
fn a_hostile_value_is_refused_or_kept_never_crashed_on() {
    for (body, expected) in [
        // A value that would be a second declaration, or carry its own
        // priority, or swallow what follows it, is refused (ADR 0033 § 5).
        ("s.color = 'red; width: 1px'; s.length", "0"),
        ("s.color = 'red !important'; s.length", "0"),
        ("s.color = 'red ! IMPORTANT'; s.length", "0"),
        ("s.width = 'calc(1px'; s.length", "0"),
        ("s.content = '\"open'; s.length", "0"),
        ("s.color = 'red /* open'; s.length", "0"),
        ("s.color = '   '; s.length", "0"),
        // A NUL is kept as the value it is part of.
        (
            "s.color = 'r\\u0000d'; s.length + ' ' + (s.color === 'r\\u0000d')",
            "1 true",
        ),
        // A hostile attribute is read, never panicked on.
        (
            "var o = ''; for (var i = 0; i < 2000; i++) o += '(';  \
             p.setAttribute('style', o); s.length + ' ' + (s.cssText === '')",
            "0 true",
        ),
        // Stray closing braces are what the style sheet's own declaration
        // parser recovers from, so the declaration reads as the cascade
        // reads it (item 341).
        (
            "p.setAttribute('style', '}}}} color: red'); s.cssText",
            "color: red;",
        ),
        // A megabyte value is kept, read back and removed.
        (
            "var v = 'a'; for (var i = 0; i < 20; i++) v += v; \
             s.setProperty('--big', v); var kept = s.getPropertyValue('--big') === v; \
             var gone = s.removeProperty('--big') === v; kept + ' ' + gone + ' ' + s.length",
            "true true 0",
        ),
    ] {
        assert_eq!(on(body), expected, "{body}");
    }
}

/// The page's own `mark(a)`, run over its own markup: the button greyed by
/// its `style` attribute, and its last two writes on the object.
#[test]
fn alo_downloads_mark_runs_to_its_end() {
    assert_eq!(
        on("function mark(a) { \
              a.textContent = 'Building — available shortly'; \
              a.removeAttribute('href'); \
              a.style.background = '#c7bfb2'; \
              a.style.cursor = 'default'; \
              a.style.pointerEvents = 'none'; \
            } \
            var a = document.createElement('a'); a.setAttribute('href', '/x'); \
            mark(a); a.getAttribute('style') + ' | ' + a.style.cursor + ' ' + \
            a.style.pointerEvents + ' ' + a.getAttribute('href')"),
        "background: #c7bfb2; | default none null"
    );
}
