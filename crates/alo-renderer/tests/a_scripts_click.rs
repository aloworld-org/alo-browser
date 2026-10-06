/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 261, cut from 256: `HTMLElement` and a script's `el.click()`
//! (ADR 0018 § 6).
//!
//! *A page's script calls `box.click()` and its listeners read the box
//! ticked, `isTrusted` `false`, and no microtask between them; a listener's
//! `preventDefault` leaves the box as it was; and a second `click()` from
//! inside the first's listener does nothing.*
//!
//! Each page's script sets its listeners up at load, and puts what it is
//! testing in a listener on a button called *Go*, which an agent then
//! presses: so the script calling `click()` runs inside the browser's own
//! dispatch, after the heap has been told to collect at every allocation.
//! What its listeners heard is written into the page's first `<p>`. A job a
//! listener queues runs at the checkpoint after *Go*'s listener, so it is
//! heard after everything `click()` did. Every page is pressed twice — once
//! ordinarily and once collecting at every allocation — and the two must
//! agree: `click()` keeps its click, its `input` and `change` and what the
//! click's pre-activation changed across everything its listeners allocate.

use alo_agent::{Target, Verb};
use alo_dom::{Document, NodeId};
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};

fn fonts() -> FontDatabase {
    let mut database = FontDatabase::new();
    if let Some(font) = Font::load(
        "DejaVu Sans",
        Weight::NORMAL,
        Slant::Normal,
        dejavu::sans::regular().to_vec(),
    ) {
        database.add(font);
    }
    database.map_generic("system-ui", "DejaVu Sans");
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

/// What a listener writes into the page's `<p>`, whether a box is ticked as
/// a listener reads it, and *Go* and the first element after it.
const PRELUDE: &str = "const out = document.body.firstChild; \
                       function say(what) { out.textContent = out.textContent + ' ' + what; } \
                       function ticked(b) { return b.getAttribute('checked') !== null; } \
                       const go = out.nextSibling; const first = go.nextSibling; ";

/// A page of `markup` after *Go*, whose script runs `setup` at load and
/// `pressed` when *Go* is pressed — collecting at every allocation from the
/// press on if `stress` — and what the press said.
fn press(markup: &str, setup: &str, pressed: &str, stress: bool) -> (Renderer, Vec<String>) {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(Page::new(
        format!(
            "<!doctype html><body><p>-</p><button>Go</button>{markup}<script>{PRELUDE}{setup}\
             go.addEventListener('click', () => {{ {pressed} }});</script></body>"
        ),
        Size::new(400.0, 300.0),
    ))));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "the page loads and says nothing: {answer:?}"
    );
    if let Some(page_loop) = renderer.event_loop() {
        page_loop.engine().objects().heap_mut().stress(stress);
    }
    let answer = renderer.handle(ToRenderer::Act {
        target: Target::Named("Go".to_owned()),
        verb: Verb::Activate,
    });
    assert!(
        matches!(answer, FromRenderer::Acted { .. }),
        "Go should be operable: {answer:?}"
    );
    let issues = match answer {
        FromRenderer::Acted { issues, .. } => issues,
        _ => Vec::new(),
    };
    (renderer, issues)
}

/// The same, both ways, each saying nothing; answers what was heard, which
/// must agree.
fn heard_both_ways(markup: &str, setup: &str, pressed: &str) -> (Renderer, String) {
    let (_, issues) = press(markup, setup, pressed, false);
    assert!(issues.is_empty(), "{issues:?}");
    let (stressed, issues) = press(markup, setup, pressed, true);
    assert!(issues.is_empty(), "{issues:?}");
    let (renderer, _) = press(markup, setup, pressed, false);
    assert_eq!(
        heard(&renderer),
        heard(&stressed),
        "the collector at every allocation changed what was heard"
    );
    let said = heard(&renderer);
    (renderer, said)
}

/// The `which`th element named `local`, in tree order.
fn nth(document: &Document, local: &str, which: usize) -> Option<NodeId> {
    document
        .descendants(document.root())
        .filter(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.name.is_html(local))
        })
        .nth(which)
}

/// The text of the page's first `<p>`.
fn heard(renderer: &Renderer) -> String {
    renderer
        .document()
        .and_then(|document| Some(document.text_content(nth(document, "p", 0)?)))
        .unwrap_or_default()
}

/// An attribute of the `which`th element named `local`.
fn attribute(renderer: &Renderer, local: &str, which: usize, name: &str) -> Option<String> {
    let document = renderer.document()?;
    document
        .element(nth(document, local, which)?)
        .and_then(|element| element.attr(name))
        .map(str::to_owned)
}

#[test]
fn a_scripts_click_ticks_the_box_untrusted_with_no_microtask_between_its_listeners() {
    let (renderer, said) = heard_both_ways(
        "<input type=checkbox>",
        "first.addEventListener('click', e => {\
           say('click:' + ticked(first) + ':' + e.isTrusted + ':' + e.pointerId + ':' + \
             e.bubbles + ':' + e.cancelable + ':' + e.composed + ':' + e.clientX);\
           queueMicrotask(() => say('job'));\
         });\
         first.addEventListener('click', () => say('second'));\
         first.addEventListener('input', e => say('input:' + e.isTrusted));\
         first.addEventListener('change', e => say('change:' + e.isTrusted));",
        "say('answered:' + first.click());",
    );
    assert_eq!(
        said,
        "- click:true:false:-1:true:true:true:0 second input:false change:false \
         answered:undefined job",
        "every listener inside the call, the job after the script that called it"
    );
    assert_eq!(
        attribute(&renderer, "input", 0, "checked").as_deref(),
        Some("")
    );
}

#[test]
fn a_listener_that_cancels_leaves_the_box_as_it_was_and_nothing_follows() {
    let (renderer, said) = heard_both_ways(
        "<input type=checkbox checked>",
        "first.addEventListener('click', e => { say('click:' + ticked(first)); \
           e.preventDefault(); });\
         first.addEventListener('input', () => say('input'));\
         first.addEventListener('change', () => say('change'));",
        "first.click(); say('after:' + ticked(first));",
    );
    assert_eq!(
        said, "- click:false after:true",
        "unticked while it ran, put back after"
    );
    assert_eq!(
        attribute(&renderer, "input", 0, "checked").as_deref(),
        Some("")
    );
}

#[test]
fn a_second_click_from_inside_the_first_does_nothing() {
    let (_, said) = heard_both_ways(
        "<input type=checkbox>",
        "let clicks = 0;\
         first.addEventListener('click', () => {\
           clicks = clicks + 1; say('click' + clicks + ':' + first.click());\
         });",
        "first.click(); say('ticked:' + ticked(first));\
         first.click(); say('ticked:' + ticked(first));",
    );
    assert_eq!(
        said, "- click1:undefined ticked:true click2:undefined ticked:false",
        "one click each time, and the flag down between them"
    );
}

#[test]
fn a_click_on_another_element_from_inside_a_listener_runs_whole() {
    let (renderer, said) = heard_both_ways(
        "<input type=checkbox><input type=checkbox>",
        "const two = first.nextSibling;\
         first.addEventListener('click', () => { say('one'); two.click(); say('back'); });\
         two.addEventListener('click', () => say('two:' + ticked(two)));\
         two.addEventListener('change', () => say('two changed'));\
         first.addEventListener('change', () => say('one changed'));",
        "first.click();",
    );
    assert_eq!(said, "- one two:true two changed back one changed");
    assert_eq!(
        attribute(&renderer, "input", 1, "checked").as_deref(),
        Some("")
    );
}

#[test]
fn a_disabled_control_is_not_clicked_and_a_disabled_fieldsets_legend_is() {
    let (renderer, said) = heard_both_ways(
        "<input type=checkbox disabled>\
         <fieldset disabled><legend><input type=checkbox></legend>\
         <input type=checkbox></fieldset><div>d</div>",
        "const fieldset = first.nextSibling;\
         const boxes = [first, fieldset.firstChild.firstChild, fieldset.lastChild];\
         const div = fieldset.nextSibling;\
         div.addEventListener('click', e => say('div:' + e.isTrusted));",
        "for (const b of boxes) { b.addEventListener('click', () => say('heard')); b.click(); }\
         div.click();",
    );
    assert_eq!(
        said, "- heard div:false",
        "only the legend's box, and any element at all"
    );
    assert_eq!(attribute(&renderer, "input", 0, "checked"), None);
    assert_eq!(
        attribute(&renderer, "input", 1, "checked").as_deref(),
        Some("")
    );
    assert_eq!(attribute(&renderer, "input", 2, "checked"), None);
}

#[test]
fn a_cancelled_click_on_a_radio_puts_the_old_choice_back() {
    let (_, said) = heard_both_ways(
        "<input type=radio name=s checked><input type=radio name=s>",
        "const large = first.nextSibling;\
         large.addEventListener('click', e => {\
           say(first.getAttribute('checked') + '/' + large.getAttribute('checked'));\
           e.preventDefault();\
         });",
        "large.click(); say(first.getAttribute('checked') + '/' + large.getAttribute('checked'));",
    );
    assert_eq!(said, "- null/ /null");
}

#[test]
fn a_box_a_listener_takes_out_of_the_page_changes_and_nobody_is_told() {
    let (_, said) = heard_both_ways(
        "<input type=checkbox>",
        "first.addEventListener('click', () => first.remove());\
         first.addEventListener('input', () => say('input'));\
         first.addEventListener('change', () => say('change'));",
        "first.click(); say('ticked:' + ticked(first));",
    );
    assert_eq!(said, "- ticked:true");
}

#[test]
fn a_listener_that_throws_is_reported_and_the_click_carries_on() {
    for stress in [false, true] {
        let (renderer, issues) = press(
            "<input type=checkbox>",
            "first.addEventListener('click', () => { throw new Error('the first listener'); });\
             first.addEventListener('click', () => say('second'));\
             first.addEventListener('input', { handleEvent() { say('input object'); } });\
             first.addEventListener('change', { get handleEvent() { \
               say('getter'); return () => say('change object'); } });",
            "first.click(); say('after');",
            stress,
        );
        assert_eq!(
            heard(&renderer),
            "- second input object getter change object after",
            "stress {stress}"
        );
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(
            issues
                .first()
                .is_some_and(|issue| issue.contains("the first listener")),
            "{issues:?}"
        );
    }
}

#[test]
fn a_link_cancelled_needs_nothing_and_a_link_followed_from_script_is_refused_by_name() {
    let (_, said) = heard_both_ways(
        "<a href=/next><span>Next</span></a>",
        "first.addEventListener('click', e => { say('click'); e.preventDefault(); });",
        "first.firstChild.click(); say('after');",
    );
    assert_eq!(said, "- click after", "the span's click is the link's");

    let (renderer, issues) = press(
        "<a href=/next>Next</a>",
        "first.addEventListener('click', () => say('click'));",
        "first.click(); say('not reached');",
        false,
    );
    assert_eq!(heard(&renderer), "- click", "its listeners ran first");
    assert!(
        issues.iter().any(|issue| issue.contains("queue item 263")),
        "{issues:?}"
    );
}

#[test]
fn click_is_an_html_elements_and_no_other_nodes() {
    let (_, said) = heard_both_ways(
        "<div>d</div><svg><g></g></svg>",
        "const g = first.nextSibling.firstChild; const html = first.__proto__;",
        "say(typeof first.click + ' ' + typeof g.click + ' ' + typeof HTMLElement);\
         say(html.__proto__ === g.__proto__ && html !== g.__proto__);\
         say(out.__proto__ === html && html.hasOwnProperty('click'));\
         try { first.click.call(g); } catch (e) { say(e.name); }\
         try { first.click.call(out.firstChild); } catch (e) { say(e.name); }\
         try { first.click.call({}); } catch (e) { say(e.name); }",
    );
    assert_eq!(
        said, "- function undefined undefined true true TypeError TypeError TypeError",
        "an HTML element's chain has HTMLElement between it and Element; an SVG one's does not"
    );
}
