/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 256, cut from 81: an agent's `Activate` is a keyboard's click
//! (ADR 0018 §§ 4–7).
//!
//! On a page that runs script, `Activate` dispatches the one `click` that
//! keyboard activation fires — a trusted `PointerEvent` with no position —
//! as one task run before the agent is answered, with HTML's activation
//! steps around it: a checkbox or radio changed before the listeners and put
//! back if one cancels, `input` and `change` after, a link followed only if
//! nobody cancelled. The agent changes no ARIA state there: the page does.
//!
//! Every page is acted on twice where a collection could lose something —
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
    let answer = renderer.handle(ToRenderer::Load(Box::new(
        Page::new(
            format!("<!doctype html><body>{body}</body>"),
            Size::new(400.0, 300.0),
        )
        // An address, so a link resolves (ADR 0020 § 2); `about:blank`, where
        // none does, only if the text were wrong.
        .at(alo_url::parse("https://example.com/").unwrap_or_else(|_| alo_url::Url::about_blank())),
    )));
    assert!(
        matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()),
        "the page loads and says nothing: {answer:?}"
    );
    if let Some(page_loop) = renderer.event_loop() {
        page_loop.engine().objects().heap_mut().stress(stress);
    }
    renderer
}

/// Activate whatever is called `name`, answering what it did and said, or
/// what the renderer answered instead.
fn activate(renderer: &mut Renderer, name: &str) -> Result<(Outcome, Vec<String>), String> {
    match renderer.handle(ToRenderer::Act {
        target: Target::Named(name.to_owned()),
        verb: Verb::Activate,
    }) {
        FromRenderer::Acted {
            outcome, issues, ..
        } => Ok((outcome, issues)),
        other => Err(format!("{name} should be operable: {other:?}")),
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

/// An attribute of the `which`th element named `local`.
fn attribute(renderer: &Renderer, local: &str, which: usize, name: &str) -> Option<String> {
    let document = renderer.document()?;
    document
        .element(nth(document, local, which)?)
        .and_then(|element| element.attr(name))
        .map(str::to_owned)
}

/// What the agent's tree says now, or nothing.
fn outline(renderer: &mut Renderer) -> String {
    match renderer.handle(ToRenderer::ReadTree) {
        FromRenderer::Tree(snapshot) => snapshot.to_outline(),
        _ => String::new(),
    }
}

/// What a listener writes into the page's `<p>`.
const SAY: &str = "const out = document.body.firstChild; \
                   function say(what) { out.textContent = out.textContent + ' ' + what; } ";

#[test]
fn the_click_is_the_one_keyboard_activation_fires_and_carries_no_position() {
    for stress in [false, true] {
        let mut renderer = loaded(
            &format!(
                "<p>-</p><button>Go</button><script>{SAY}\
                 out.nextSibling.addEventListener('click', e => say(\
                   e.type + ' ' + e.isTrusted + ' ' + e.bubbles + ' ' + e.cancelable + ' ' + \
                   e.composed + ' ' + e.eventPhase + ' ' + e.pointerId + ' [' + e.pointerType + \
                   '] ' + e.clientX + ' ' + e.clientY + ' ' + e.screenX + ' ' + e.screenY + ' ' + \
                   e.button + ' ' + e.buttons + ' ' + e.detail + ' ' + e.ctrlKey + ' ' + \
                   e.shiftKey + ' ' + e.altKey + ' ' + e.metaKey + ' ' + e.relatedTarget));\
                 </script>"
            ),
            stress,
        );
        let (outcome, issues) = activate(&mut renderer, "Go").expect("an operable control");
        assert!(issues.is_empty(), "{issues:?}");
        assert!(
            matches!(&outcome, Outcome::Activated { name, .. } if name.as_deref() == Some("Go")),
            "{outcome:?}"
        );
        assert_eq!(
            heard(&renderer),
            "- click true true true true 2 -1 [] 0 0 0 0 0 0 0 false false false false null",
            "stress {stress}"
        );
    }
}

#[test]
fn its_members_are_its_interfaces_and_refuse_anything_else() {
    // A plain event has none of a click's members, and none of the three
    // interfaces has a constructor on the global yet.
    let mut renderer = loaded(
        &format!(
            "<p>-</p><button>Go</button><script>{SAY}\
             out.nextSibling.addEventListener('click', e => {{\
               const plain = new Event('x');\
               say(plain.clientX === undefined && plain.pointerId === undefined);\
               say(typeof MouseEvent + ' ' + typeof PointerEvent + ' ' + typeof UIEvent);\
             }});\
             </script>"
        ),
        false,
    );
    let (_, issues) = activate(&mut renderer, "Go").expect("an operable control");
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(heard(&renderer), "- true undefined undefined undefined");
}

#[test]
fn a_cancelled_click_on_a_checkbox_leaves_it_unticked_and_fires_nothing_after() {
    for stress in [false, true] {
        let mut renderer = loaded(
            &format!(
                "<p>-</p><input type=checkbox aria-label=Box><script>{SAY}\
                 const box = out.nextSibling;\
                 box.addEventListener('click', e => {{\
                   say('click:' + (box.getAttribute('checked') !== null));\
                   e.preventDefault();\
                 }});\
                 box.addEventListener('input', () => say('input'));\
                 box.addEventListener('change', () => say('change'));\
                 </script>"
            ),
            stress,
        );
        let (outcome, issues) = activate(&mut renderer, "Box").expect("an operable control");
        assert!(issues.is_empty(), "{issues:?}");
        assert!(matches!(outcome, Outcome::Activated { .. }), "{outcome:?}");
        assert_eq!(
            heard(&renderer),
            "- click:true",
            "ticked while the listener ran, nothing after it was cancelled (stress {stress})"
        );
        assert_eq!(attribute(&renderer, "input", 0, "checked"), None);
        let tree = outline(&mut renderer);
        assert!(
            tree.contains("checkbox \"Box\" [checked=false]"),
            "the agent reads it unticked: {tree}"
        );
    }
}

#[test]
fn a_click_nobody_cancels_ticks_the_box_then_fires_input_and_change_in_its_task() {
    for stress in [false, true] {
        let mut renderer = loaded(
            &format!(
                "<p>-</p><input type=checkbox aria-label=Box><script>{SAY}\
                 const box = out.nextSibling;\
                 box.addEventListener('click', e => {{\
                   say('click:' + (box.getAttribute('checked') !== null));\
                   queueMicrotask(() => say('job'));\
                 }});\
                 box.addEventListener('input', e => say('input:' + e.isTrusted + ':' + \
                   e.bubbles + ':' + e.composed + ':' + e.cancelable));\
                 box.addEventListener('change', e => say('change:' + e.isTrusted + ':' + \
                   e.bubbles + ':' + e.composed + ':' + e.cancelable + ':' + \
                   (e.pointerId === undefined)));\
                 </script>"
            ),
            stress,
        );
        let (_, issues) = activate(&mut renderer, "Box").expect("an operable control");
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(
            heard(&renderer),
            "- click:true job input:true:true:true:false change:true:true:false:false:true",
            "stress {stress}"
        );
        assert_eq!(
            attribute(&renderer, "input", 0, "checked").as_deref(),
            Some("")
        );
        assert!(outline(&mut renderer).contains("checkbox \"Box\" [checked=true]"));
    }
}

#[test]
fn a_cancelled_click_on_a_radio_puts_the_old_choice_back() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><input type=radio name=s aria-label=Small checked>\
             <input type=radio name=s aria-label=Large><script>{SAY}\
             const large = out.nextSibling.nextSibling;\
             large.addEventListener('click', e => {{\
               say(out.nextSibling.getAttribute('checked') + '/' + large.getAttribute('checked'));\
               e.preventDefault();\
             }});\
             </script>"
        ),
        false,
    );
    let (_, issues) = activate(&mut renderer, "Large").expect("an operable control");
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(heard(&renderer), "- null/", "Large was chosen while it ran");
    assert_eq!(
        attribute(&renderer, "input", 0, "checked").as_deref(),
        Some("")
    );
    assert_eq!(attribute(&renderer, "input", 1, "checked"), None);
}

#[test]
fn a_link_is_followed_only_when_nobody_cancels_its_click() {
    let link = |cancel: bool| {
        format!(
            "<p>-</p><a href=/next>Next</a><script>{SAY}\
             out.nextSibling.addEventListener('click', e => {{ say('click'); {} }});\
             </script>",
            if cancel { "e.preventDefault();" } else { "" }
        )
    };
    let mut renderer = loaded(&link(false), false);
    let (outcome, _) = activate(&mut renderer, "Next").expect("an operable control");
    assert!(
        matches!(&outcome, Outcome::Followed { to, .. } if to == "/next"),
        "{outcome:?}"
    );
    assert_eq!(heard(&renderer), "- click");

    let mut renderer = loaded(&link(true), false);
    let (outcome, issues) = activate(&mut renderer, "Next").expect("an operable control");
    assert!(issues.is_empty(), "{issues:?}");
    assert!(
        matches!(&outcome, Outcome::Activated { name, .. } if name.as_deref() == Some("Next")),
        "pressed, and not followed: {outcome:?}"
    );
    assert_eq!(heard(&renderer), "- click");
}

#[test]
fn on_a_page_that_runs_script_aria_state_is_the_pages_to_change() {
    // A listener that keeps the promise: flipped once, by the page — an
    // agent that flipped it as well would flip it back.
    let mut renderer = loaded(
        &format!(
            "<p>-</p><div role=switch aria-checked=false>Dark</div><script>{SAY}\
             const sw = out.nextSibling;\
             sw.addEventListener('click', () => sw.setAttribute('aria-checked', \
               sw.getAttribute('aria-checked') === 'true' ? 'false' : 'true'));\
             </script>"
        ),
        false,
    );
    activate(&mut renderer, "Dark").expect("an operable control");
    assert_eq!(
        attribute(&renderer, "div", 0, "aria-checked").as_deref(),
        Some("true")
    );

    // A page that runs script and does not listen: the agent does not keep
    // the page's promise for it.
    let mut renderer = loaded(
        &format!("<p>-</p><div role=switch aria-checked=false>Dark</div><script>{SAY}</script>"),
        false,
    );
    activate(&mut renderer, "Dark").expect("an operable control");
    assert_eq!(
        attribute(&renderer, "div", 0, "aria-checked").as_deref(),
        Some("false")
    );
}

#[test]
fn a_listener_that_throws_is_said_in_the_answer_and_the_click_carries_on() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><button>Go</button><script>{SAY}\
             const b = out.nextSibling;\
             b.addEventListener('click', () => {{ throw new Error('the first listener'); }});\
             b.addEventListener('click', () => say('second'));\
             </script>"
        ),
        false,
    );
    let (_, issues) = activate(&mut renderer, "Go").expect("an operable control");
    assert_eq!(heard(&renderer), "- second");
    assert_eq!(issues.len(), 1, "{issues:?}");
    let said = issues.first().map(String::as_str).unwrap_or_default();
    assert!(
        said.starts_with("the click: ") && said.contains("the first listener"),
        "{said}"
    );
}

#[test]
fn a_page_whose_script_stopped_still_has_its_box_ticked_and_is_said_to_have_heard_nothing() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><input type=checkbox aria-label=Box><script>{SAY}\
             out.nextSibling.addEventListener('click', () => say('heard'));\
             </script>"
        ),
        false,
    );
    renderer
        .event_loop()
        .expect("a scripted page")
        .stop_switch()
        .ask();

    // The click's task is where the page stops: its box was ticked first.
    let (_, issues) = activate(&mut renderer, "Box").expect("an operable control");
    assert_eq!(heard(&renderer), "-", "no listener ran");
    assert!(
        issues
            .iter()
            .any(|issue| issue.contains("the page stopped")),
        "{issues:?}"
    );
    assert_eq!(
        attribute(&renderer, "input", 0, "checked").as_deref(),
        Some("")
    );

    // From then on nothing is queued, the box is still the document's to
    // change, and the answer says nobody heard.
    let (outcome, issues) = activate(&mut renderer, "Box").expect("an operable control");
    assert!(matches!(outcome, Outcome::Activated { .. }), "{outcome:?}");
    assert_eq!(attribute(&renderer, "input", 0, "checked"), None);
    assert!(
        issues
            .iter()
            .any(|issue| issue.starts_with("the click: nobody heard it")),
        "{issues:?}"
    );
}

#[test]
fn a_page_that_never_ran_script_is_acted_on_as_stage_one_was() {
    // No heap is built to find out nobody listens: the box is ticked and an
    // ARIA switch flipped by the agent (ADR 0018 § 7's accommodation).
    let mut renderer = loaded(
        "<p>-</p><input type=checkbox aria-label=Box>\
         <div role=switch aria-checked=false>Dark</div>",
        false,
    );
    let (_, issues) = activate(&mut renderer, "Box").expect("an operable control");
    assert!(issues.is_empty());
    activate(&mut renderer, "Dark").expect("an operable control");
    assert!(renderer.event_loop().is_none(), "no heap was built");
    assert_eq!(
        attribute(&renderer, "input", 0, "checked").as_deref(),
        Some("")
    );
    assert_eq!(
        attribute(&renderer, "div", 0, "aria-checked").as_deref(),
        Some("true")
    );
}
