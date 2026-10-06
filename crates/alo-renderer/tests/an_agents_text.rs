/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 257, cut from 81: an agent's `PutText` fires what replacing a
//! field's text fires (ADR 0018 § 5).
//!
//! On a page that runs script, `PutText` is one task run before the agent is
//! answered: a trusted `beforeinput` — an `InputEvent` whose `inputType` is
//! `"insertReplacementText"` and whose `data` is the text — which the page
//! may cancel; then, if it did not, the text, an `input` like it but not
//! cancelable, and a `change`. A cancelled `beforeinput` changes nothing and
//! answers `TextCanceled`.
//!
//! Every page whose answer a collection could change is acted on twice —
//! once ordinarily and once with the collector at every allocation — and the
//! two must say the same.

use alo_agent::{Outcome, Target, Verb};
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

/// `body` loaded, its scripts run, with the collector at every allocation
/// from then on if `stress`.
fn loaded(body: &str, stress: bool) -> Renderer {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(Page::new(
        format!("<!doctype html><body>{body}</body>"),
        Size::new(400.0, 300.0),
    ))));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "the page loads and says nothing: {answer:?}"
    );
    if let Some(page_loop) = renderer.event_loop() {
        page_loop.engine().objects().heap_mut().stress(stress);
    }
    renderer
}

/// Put `text` into whatever is called `name`, answering what it did and
/// said, or what the renderer answered instead.
fn put(renderer: &mut Renderer, name: &str, text: &str) -> Result<(Outcome, Vec<String>), String> {
    match renderer.handle(ToRenderer::Act {
        target: Target::Named(name.to_owned()),
        verb: Verb::PutText(text.to_owned()),
    }) {
        FromRenderer::Acted {
            outcome, issues, ..
        } => Ok((outcome, issues)),
        other => Err(format!("{name} should take text: {other:?}")),
    }
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

/// The text of the page's first `<p>`, where its script writes what it
/// heard — empty if there is no page or no `<p>`.
fn heard(renderer: &Renderer) -> String {
    renderer
        .document()
        .and_then(|document| Some(document.text_content(nth(document, "p", 0)?)))
        .unwrap_or_default()
}

/// The first `<input>`'s `value`, which is its text until item 82.
fn value(renderer: &Renderer) -> Option<String> {
    let document = renderer.document()?;
    document
        .element(nth(document, "input", 0)?)
        .and_then(|element| element.attr("value"))
        .map(str::to_owned)
}

/// What the agent's tree says now, or nothing.
fn outline(renderer: &mut Renderer) -> String {
    match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => snapshot.to_outline(),
        _ => String::new(),
    }
}

/// What a listener writes into the page's `<p>`, and the field after it.
const SAY: &str = "const out = document.body.firstChild; const field = out.nextSibling; \
                   function say(what) { out.textContent = out.textContent + ' ' + what; } ";

/// The closing page: a field, and an `input` listener echoing its text into
/// the `<output>` after it.
const ECHO: &str = "<p>-</p><input aria-label=Amount value=0><output>nothing yet</output>\
                    <script>\
                    const amount = document.body.firstChild.nextSibling;\
                    amount.addEventListener('input', () => {\
                      amount.nextSibling.textContent = 'You typed ' + amount.getAttribute('value');\
                    });\
                    </script>";

#[test]
fn a_pages_input_listener_echoes_the_text_and_the_agent_reads_the_echo() {
    for stress in [false, true] {
        let mut renderer = loaded(ECHO, stress);
        let before = outline(&mut renderer);
        assert!(before.contains("nothing yet"), "{before}");

        let (outcome, issues) = put(&mut renderer, "Amount", "12.50").expect("a field");
        assert!(issues.is_empty(), "{issues:?}");
        assert!(
            matches!(&outcome, Outcome::TextPut { text, .. } if text == "12.50"),
            "{outcome:?}"
        );
        let after = outline(&mut renderer);
        assert!(
            after.contains("You typed 12.50"),
            "the agent reads the page's echo (stress {stress}):\n{after}"
        );
        assert!(!after.contains("nothing yet"), "{after}");
        assert_eq!(value(&renderer).as_deref(), Some("12.50"));

        // The echo is laid out where its text puts it, in numbers.
        let Some(drawing) = renderer.rendered() else {
            panic!("nothing was rendered");
        };
        let laid = drawing.layout.to_outline(&drawing.boxes);
        let line = laid
            .lines()
            .find(|line| line.contains("You typed 12.50"))
            .unwrap_or_default()
            .trim()
            .to_owned();
        assert_eq!(line, ECHO_LAID, "stress {stress}:\n{laid}");
    }
}

/// Where the echo's text is laid out, as the layout outline says it.
const ECHO_LAID: &str = "text \"You typed 12.50\" → 129.45313×18.625 at (174, 55.2)";

#[test]
fn beforeinput_then_input_then_change_each_trusted_and_each_with_its_members() {
    for stress in [false, true] {
        let mut renderer = loaded(
            &format!(
                "<p>-</p><input aria-label=Field value=old><script>{SAY}\
                 field.addEventListener('beforeinput', e => {{\
                   say(e.type + ':' + e.isTrusted + ':' + e.bubbles + ':' + e.cancelable + ':' + \
                     e.composed + ':' + e.eventPhase + ':' + e.inputType + ':' + e.data + ':' + \
                     e.isComposing + ':' + e.detail + ':' + field.getAttribute('value'));\
                   queueMicrotask(() => say('job:' + field.getAttribute('value')));\
                 }});\
                 field.addEventListener('input', e => say(e.type + ':' + e.isTrusted + ':' + \
                   e.bubbles + ':' + e.cancelable + ':' + e.composed + ':' + e.inputType + ':' + \
                   e.data + ':' + field.getAttribute('value')));\
                 field.addEventListener('change', e => say(e.type + ':' + e.isTrusted + ':' + \
                   e.bubbles + ':' + e.cancelable + ':' + e.composed + ':' + \
                   (e.inputType === undefined) + ':' + field.getAttribute('value')));\
                 document.addEventListener('beforeinput', e => say('up:' + e.type + ':' + \
                   (e.target === field)));\
                 document.addEventListener('input', e => say('up:' + e.type));\
                 document.addEventListener('change', e => say('up:' + e.type));\
                 </script>"
            ),
            stress,
        );
        let (_, issues) = put(&mut renderer, "Field", "new").expect("a field");
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(
            heard(&renderer),
            "- beforeinput:true:true:true:true:2:insertReplacementText:new:false:0:old \
             job:old up:beforeinput:true \
             input:true:true:false:true:insertReplacementText:new:new up:input \
             change:true:true:false:false:true:new up:change",
            "the listeners on beforeinput read the old text and its job runs before the text \
             goes in; each event bubbles to the document (stress {stress})"
        );
    }
}

#[test]
fn a_cancelled_beforeinput_leaves_the_field_as_it_was_and_answers_so() {
    for stress in [false, true] {
        let mut renderer = loaded(
            &format!(
                "<p>-</p><input aria-label=Field value=kept><script>{SAY}\
                 field.addEventListener('beforeinput', e => {{ say('asked:' + e.data); \
                   e.preventDefault(); }});\
                 field.addEventListener('input', () => say('input'));\
                 field.addEventListener('change', () => say('change'));\
                 </script>"
            ),
            stress,
        );
        let (outcome, issues) = put(&mut renderer, "Field", "refused").expect("a field");
        assert!(issues.is_empty(), "{issues:?}");
        assert!(
            matches!(&outcome, Outcome::TextCanceled { text, .. } if text == "refused"),
            "{outcome:?}"
        );
        assert_eq!(heard(&renderer), "- asked:refused", "stress {stress}");
        assert_eq!(value(&renderer).as_deref(), Some("kept"));
        let tree = outline(&mut renderer);
        assert!(
            tree.contains("textbox \"Field\" at (8, 50.625) 166×23.2\n    text \"kept\""),
            "the agent reads the field as it was:\n{tree}"
        );
    }
}

#[test]
fn input_cannot_be_cancelled_and_a_passive_beforeinput_listener_cannot_cancel() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><input aria-label=Field value=old><script>{SAY}\
             field.addEventListener('beforeinput', e => {{ e.preventDefault(); \
               say('passive:' + e.defaultPrevented); }}, {{ passive: true }});\
             field.addEventListener('input', e => {{ e.preventDefault(); \
               say('input:' + e.defaultPrevented); }});\
             </script>"
        ),
        false,
    );
    let (outcome, issues) = put(&mut renderer, "Field", "in").expect("a field");
    assert!(issues.is_empty(), "{issues:?}");
    assert!(matches!(outcome, Outcome::TextPut { .. }), "{outcome:?}");
    assert_eq!(heard(&renderer), "- passive:false input:false");
    assert_eq!(value(&renderer).as_deref(), Some("in"));
}

#[test]
fn an_input_event_is_a_ui_event_with_only_the_members_built() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><input aria-label=Field><script>{SAY}\
             field.addEventListener('beforeinput', e => {{\
               const input = e.__proto__; const ui = input.__proto__;\
               say(typeof InputEvent + ' ' + typeof UIEvent);\
               say(input.hasOwnProperty('data') && input.hasOwnProperty('inputType') && \
                 input.hasOwnProperty('isComposing') && ui.hasOwnProperty('detail'));\
               say(ui.__proto__.hasOwnProperty('preventDefault'));\
               say(('dataTransfer' in e) + ' ' + ('getTargetRanges' in e) + ' ' + \
                 ('pointerId' in e) + ' ' + ('clientX' in e));\
               try {{ ({{ __proto__: input }}).data; }} catch (x) {{ say(x.name); }}\
               try {{ ({{ __proto__: ui }}).detail; }} catch (x) {{ say(x.name); }}\
               const plain = new Event('x');\
               say(plain.data + ' ' + plain.inputType);\
             }});\
             </script>"
        ),
        false,
    );
    let (_, issues) = put(&mut renderer, "Field", "x").expect("a field");
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        heard(&renderer),
        "- undefined undefined true true false false false false TypeError TypeError \
         undefined undefined",
        "InputEvent inherits UIEvent inherits Event, has no constructor on the global, and \
         its members refuse what is not one"
    );
}

#[test]
fn a_listener_that_throws_is_said_in_the_answer_and_the_text_still_goes_in() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><input aria-label=Field value=old><script>{SAY}\
             field.addEventListener('beforeinput', () => {{ throw new Error('the first'); }});\
             field.addEventListener('beforeinput', () => say('second'));\
             field.addEventListener('input', () => say('input'));\
             </script>"
        ),
        false,
    );
    let (outcome, issues) = put(&mut renderer, "Field", "new").expect("a field");
    assert!(matches!(outcome, Outcome::TextPut { .. }), "{outcome:?}");
    assert_eq!(heard(&renderer), "- second input");
    assert_eq!(value(&renderer).as_deref(), Some("new"));
    assert_eq!(issues.len(), 1, "{issues:?}");
    let said = issues.first().map(String::as_str).unwrap_or_default();
    assert!(
        said.starts_with("the text: ") && said.contains("the first"),
        "{said}"
    );
}

#[test]
fn a_field_a_beforeinput_listener_took_away_still_takes_the_text_and_hears_input() {
    for stress in [false, true] {
        let mut renderer = loaded(
            &format!(
                "<p>-</p><input aria-label=Field value=old><script>{SAY}\
                 field.addEventListener('beforeinput', () => field.remove());\
                 field.addEventListener('input', () => say('input:' + \
                   field.getAttribute('value')));\
                 field.addEventListener('change', () => say('change'));\
                 document.addEventListener('input', () => say('the document heard'));\
                 </script>"
            ),
            stress,
        );
        let (outcome, issues) = put(&mut renderer, "Field", "new").expect("a field");
        assert!(issues.is_empty(), "{issues:?}");
        assert!(matches!(outcome, Outcome::TextPut { .. }), "{outcome:?}");
        assert_eq!(heard(&renderer), "- input:new change", "stress {stress}");
        assert_eq!(
            value(&renderer),
            None,
            "the field is no longer in the document"
        );
    }
}

#[test]
fn a_page_whose_script_stopped_still_takes_the_text_and_is_said_to_have_heard_nothing() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><input aria-label=Field value=old><script>{SAY}\
             field.addEventListener('beforeinput', e => {{ say('asked'); e.preventDefault(); }});\
             </script>"
        ),
        false,
    );
    renderer
        .event_loop()
        .expect("a scripted page")
        .stop_switch()
        .ask();

    // The task is where the page stops, before any listener: the listener
    // that would have refused never ran, nobody refused, and the text goes
    // in — as a stopped page's box is still ticked by a click.
    let (outcome, issues) = put(&mut renderer, "Field", "first").expect("a field");
    assert!(matches!(outcome, Outcome::TextPut { .. }), "{outcome:?}");
    assert_eq!(heard(&renderer), "-");
    assert_eq!(value(&renderer).as_deref(), Some("first"));
    assert!(
        issues
            .iter()
            .any(|issue| issue.starts_with("the text: ") && issue.contains("the page stopped")),
        "{issues:?}"
    );

    // From then on nothing is queued: the text is the document's to take,
    // nobody is asked, and the answer says so.
    let (outcome, issues) = put(&mut renderer, "Field", "second").expect("a field");
    assert!(matches!(outcome, Outcome::TextPut { .. }), "{outcome:?}");
    assert_eq!(heard(&renderer), "-");
    assert_eq!(value(&renderer).as_deref(), Some("second"));
    assert!(
        issues
            .iter()
            .any(|issue| issue.starts_with("the text: nobody heard it")),
        "{issues:?}"
    );
}

#[test]
fn a_page_that_never_ran_script_takes_the_text_as_stage_one_did() {
    let mut renderer = loaded("<p>-</p><input aria-label=Field value=old>", false);
    let (outcome, issues) = put(&mut renderer, "Field", "new").expect("a field");
    assert!(issues.is_empty(), "{issues:?}");
    assert!(matches!(outcome, Outcome::TextPut { .. }), "{outcome:?}");
    assert!(renderer.event_loop().is_none(), "no heap was built");
    assert_eq!(value(&renderer).as_deref(), Some("new"));
    assert!(outline(&mut renderer).contains("new"));
}
