/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 485, cut from 468: an agent's press on a submit button, on a
//! page that runs script (ADR 0051 §§ 4 and 6).
//!
//! The press is a click the page hears, and the click's task submits the
//! button's form: a trusted `invalid` at each control that fails, in tree
//! order, and nothing more; or a trusted `SubmitEvent` at the form, which a
//! listener may cancel; and then the ask, built from what the listeners
//! left. HTML's guard keeps a listener that presses the button again from
//! submitting twice. A form this engine cannot check is refused before the
//! page hears anything, and a page whose script has stopped is pressed as a
//! page that never ran script.
//!
//! The pages whose events carry an object a collection could lose — the
//! `SubmitEvent` holding its submitter — are acted on twice, once with the
//! collector at every allocation, and the two must say the same.

use alo_agent::{Outcome, Refusal, Target, Verb};
use alo_dom::constraint::Constraint;
use alo_dom::submission::{Entry, Method, Value};
use alo_dom::{Document, NodeId};
use alo_layout::Size;
use alo_renderer::ask::Asked;
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

/// `body` loaded at `https://example.com/page`, its scripts run, with the
/// collector at every allocation from then on if `stress` — or what the
/// renderer answered instead.
fn loaded(body: &str, stress: bool) -> Result<Renderer, String> {
    let at = alo_url::parse("https://example.com/page").map_err(|why| why.why.clone())?;
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(
        Page::new(
            format!("<!doctype html><body>{body}</body>"),
            Size::new(400.0, 300.0),
        )
        .at(at),
    )));
    if !matches!(&answer, FromRenderer::Loaded { issues, .. } if issues.is_empty()) {
        return Err(format!("the page loads and says nothing: {answer:?}"));
    }
    if let Some(page_loop) = renderer.event_loop() {
        page_loop.engine().objects().heap_mut().stress(stress);
    }
    Ok(renderer)
}

/// What an `Activate` on whatever is called `name` came to.
#[derive(Debug)]
struct Pressed {
    outcome: Outcome,
    asked: Option<Asked>,
    issues: Vec<String>,
}

/// Activate whatever is called `name`: what it did, or what the renderer
/// answered instead.
fn press(renderer: &mut Renderer, name: &str) -> Result<Pressed, Box<FromRenderer>> {
    match renderer.handle(ToRenderer::Act {
        target: Target::Named(name.to_owned()),
        verb: Verb::Activate,
    }) {
        FromRenderer::Acted {
            outcome,
            navigation,
            issues,
            ..
        } => Ok(Pressed {
            outcome,
            asked: navigation,
            issues,
        }),
        other => Err(Box::new(other)),
    }
}

/// The first element named `local`, in tree order.
fn first(document: &Document, local: &str) -> Option<NodeId> {
    document.descendants(document.root()).find(|node| {
        document
            .element(*node)
            .is_some_and(|element| element.name.is_html(local))
    })
}

/// The text of the page's first `<p>`, where its script writes what it
/// heard — empty if there is no page or no `<p>`.
fn heard(renderer: &Renderer) -> String {
    renderer
        .document()
        .and_then(|document| Some(document.text_content(first(document, "p")?)))
        .unwrap_or_default()
}

/// An entry list as `name=value` lines.
fn written(asked: Option<&Asked>) -> Vec<String> {
    asked
        .and_then(|asked| asked.submission.as_ref())
        .map(|submission| {
            submission
                .entries
                .iter()
                .map(|Entry { name, value }| match value {
                    Value::Text(text) => format!("{name}={text}"),
                    Value::NoFile => format!("{name}=<no file>"),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// What a listener writes into the page's `<p>`, which is the body's first
/// child.
const SAY: &str = "const out = document.body.firstChild; \
                   function say(what) { out.textContent = out.textContent + ' ' + what; } ";

#[test]
fn submit_is_a_trusted_submit_event_that_names_its_button() {
    for stress in [false, true] {
        let mut renderer = loaded(
            &format!(
                "<p>-</p><form action=/sent method=post>\
                 <label>Name <input name=n value=Ada></label><button name=go value=1>Send</button>\
                 </form><script>{SAY}\
                 const form = document.querySelectorAll('form')[0];\
                 const button = document.querySelectorAll('button')[0];\
                 form.addEventListener('submit', e => say(\
                   e.type + ' ' + e.isTrusted + ' ' + e.bubbles + ' ' + e.cancelable + ' ' + \
                   e.composed + ' ' + e.eventPhase + ' ' + (e.target === form) + ' ' + \
                   (e.submitter === button) + ' ' + ('submitter' in e) + ' ' + \
                   e.hasOwnProperty('submitter')));\
                 document.addEventListener('submit', e => {{\
                   try {{ say(e.__proto__.submitter); }} catch (err) {{ say(err.name); }}\
                   say(typeof SubmitEvent); }});\
                 </script>"
            ),
            stress,
        )
        .expect("the page loads");
        let pressed = press(&mut renderer, "Send").expect("an operable control");
        // As Chrome 154 fires it (ADR 0051, measured): trusted, bubbling,
        // cancelable, at the form, the pressed button its submitter — read
        // from the prototype, which refuses what is not a `SubmitEvent`.
        // There is no constructor on the global (ADR 0013 § 3).
        assert_eq!(
            heard(&renderer),
            "- submit true true true false 2 true true true false TypeError undefined",
            "stress {stress}"
        );
        assert!(
            matches!(
                &pressed.outcome,
                Outcome::Submitted { method: Method::Post, to, .. } if to == "https://example.com/sent"
            ),
            "{pressed:?}"
        );
        assert!(pressed.issues.is_empty(), "{pressed:?}");
        assert_eq!(written(pressed.asked.as_ref()), ["n=Ada", "go=1"]);
    }
}

#[test]
fn a_submit_listener_that_cancels_keeps_the_form_and_the_press_is_the_pages() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><form action=/sent><button>Send</button></form><script>{SAY}\
             document.querySelectorAll('form')[0].addEventListener('submit', e => {{\
               e.preventDefault(); say('kept ' + e.defaultPrevented); }});\
             </script>"
        ),
        false,
    )
    .expect("the page loads");
    let pressed = press(&mut renderer, "Send").expect("an operable control");
    assert_eq!(heard(&renderer), "- kept true");
    assert!(
        matches!(&pressed.outcome, Outcome::Activated { name, .. } if name.as_deref() == Some("Send")),
        "{pressed:?}"
    );
    assert_eq!(pressed.asked, None, "nothing was asked for");
    assert!(
        pressed
            .issues
            .iter()
            .any(|line| line.contains("cancelled its submit event")),
        "{pressed:?}"
    );
}

#[test]
fn invalid_is_fired_at_each_failing_control_in_tree_order_and_submit_is_not() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><form action=/sent>\
             <label>A <input name=a required></label>\
             <label>B <input name=b value=fine required></label>\
             <label>C <input name=c type=email value=nope></label>\
             <button>Send</button></form><script>{SAY}\
             const form = document.querySelectorAll('form')[0];\
             form.addEventListener('invalid', e => say(\
               e.target.getAttribute('name') + ' ' + e.isTrusted + ' ' + e.bubbles + ' ' + \
               e.cancelable + ' ' + e.eventPhase), true);\
             form.addEventListener('invalid', e => say('bubbled'));\
             form.addEventListener('submit', e => say('submit'));\
             </script>"
        ),
        false,
    )
    .expect("the page loads");
    let pressed = press(&mut renderer, "Send").expect("an operable control");
    // Captured at the form on the way down to each, never bubbling back up,
    // and no `submit`.
    assert_eq!(
        heard(&renderer),
        "- a true false true 1 c true false true 1"
    );
    let Outcome::NotSubmitted { invalid, .. } = &pressed.outcome else {
        panic!("the form was sent: {pressed:?}");
    };
    let said: Vec<_> = invalid
        .iter()
        .map(|each| (each.constraint, each.message.as_str()))
        .collect();
    assert_eq!(
        said,
        [
            (
                Some(Constraint::ValueMissing),
                "Please fill out this field."
            ),
            (
                Some(Constraint::TypeMismatch),
                "Please include an '@' in the email address. 'nope' is missing an '@'."
            ),
        ]
    );
    assert_eq!(pressed.asked, None);
}

#[test]
fn a_listener_that_cancels_invalid_still_does_not_send_the_form() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><form action=/sent><label>A <input name=a required></label>\
             <button>Send</button></form><script>{SAY}\
             document.querySelectorAll('input')[0].addEventListener('invalid', e => {{\
               e.preventDefault(); say('quiet'); }});\
             </script>"
        ),
        false,
    )
    .expect("the page loads");
    let pressed = press(&mut renderer, "Send").expect("an operable control");
    assert_eq!(heard(&renderer), "- quiet");
    assert!(
        matches!(pressed.outcome, Outcome::NotSubmitted { .. }),
        "{pressed:?}"
    );
    assert_eq!(pressed.asked, None);
}

#[test]
fn novalidate_fires_no_invalid_and_sends_the_form() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><form action=/sent novalidate><label>A <input name=a required></label>\
             <button>Send</button></form><script>{SAY}\
             document.addEventListener('invalid', e => say('invalid'), true);\
             document.addEventListener('submit', e => say('submit'));\
             </script>"
        ),
        false,
    )
    .expect("the page loads");
    let pressed = press(&mut renderer, "Send").expect("an operable control");
    assert_eq!(heard(&renderer), "- submit");
    assert!(
        matches!(
            pressed.outcome,
            Outcome::Submitted {
                method: Method::Get,
                ..
            }
        ),
        "{pressed:?}"
    );
    assert_eq!(written(pressed.asked.as_ref()), ["a="]);
}

#[test]
fn a_submit_listener_that_presses_the_button_again_does_not_send_twice() {
    for stress in [false, true] {
        let mut renderer = loaded(
            &format!(
                "<p>-</p><form action=/sent><button>Send</button></form><script>{SAY}\
                 const button = document.querySelectorAll('button')[0];\
                 let submits = 0;\
                 button.addEventListener('click', e => say('click'));\
                 document.querySelectorAll('form')[0].addEventListener('submit', e => {{\
                   submits = submits + 1; say('submit ' + submits); button.click(); }});\
                 </script>"
            ),
            stress,
        )
        .expect("the page loads");
        let pressed = press(&mut renderer, "Send").expect("an operable control");
        // HTML's guard: the second click is heard, and its submission does
        // nothing at all — no second `submit`, nothing thrown.
        assert_eq!(
            heard(&renderer),
            "- click submit 1 click",
            "stress {stress}"
        );
        assert!(pressed.issues.is_empty(), "{pressed:?}");
        assert!(
            matches!(pressed.outcome, Outcome::Submitted { .. }),
            "{pressed:?}"
        );
        assert_eq!(pressed.asked.map(|asked| asked.replaced), Some(0));
    }
}

#[test]
fn what_a_submit_listener_changes_is_what_is_sent() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><form action=/sent><label>A <input name=a value=before></label>\
             <button>Send</button></form><script>{SAY}\
             document.querySelectorAll('form')[0].addEventListener('submit', e => \
               document.querySelectorAll('input')[0].setAttribute('value', 'after'));\
             </script>"
        ),
        false,
    )
    .expect("the page loads");
    let pressed = press(&mut renderer, "Send").expect("an operable control");
    assert_eq!(written(pressed.asked.as_ref()), ["a=after"]);
    // The entries, not a query: the browser process writes them into the
    // URL (ADR 0051 § 5).
    assert_eq!(
        pressed.asked.map(|asked| asked.url).as_deref(),
        Some("https://example.com/sent")
    );
}

#[test]
fn a_form_a_submit_listener_takes_away_is_not_sent() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><form action=/sent><button>Send</button></form><script>{SAY}\
             const form = document.querySelectorAll('form')[0];\
             form.addEventListener('submit', e => form.parentNode.removeChild(form));\
             </script>"
        ),
        false,
    )
    .expect("the page loads");
    let pressed = press(&mut renderer, "Send").expect("an operable control");
    assert!(
        matches!(pressed.outcome, Outcome::Activated { .. }),
        "{pressed:?}"
    );
    assert_eq!(pressed.asked, None);
    assert!(
        pressed
            .issues
            .iter()
            .any(|line| line.contains("took the form out of its document")),
        "{pressed:?}"
    );
}

#[test]
fn a_constraint_this_engine_cannot_check_refuses_the_press_before_the_page_hears_it() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><form action=/sent><label>A <input name=a pattern=x value=y></label>\
             <button>Send</button></form><script>{SAY}\
             document.addEventListener('click', e => say('click'));\
             </script>"
        ),
        false,
    )
    .expect("the page loads");
    let refused = press(&mut renderer, "Send").err();
    assert!(
        matches!(
            refused.as_deref(),
            Some(FromRenderer::Refused(Refusal::Unchecked {
                constraint: Constraint::PatternMismatch,
                control: Some(_),
                ..
            }))
        ),
        "{refused:?}"
    );
    assert_eq!(
        heard(&renderer),
        "-",
        "nobody heard a press that was refused"
    );
}

#[test]
fn a_form_this_engine_does_not_send_refuses_the_press_before_the_page_hears_it() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><form action=/sent method=dialog><button>Send</button></form>\
             <script>{SAY}document.addEventListener('click', e => say('click'));</script>"
        ),
        false,
    )
    .expect("the page loads");
    let refused = press(&mut renderer, "Send").err();
    assert!(
        matches!(
            refused.as_deref(),
            Some(FromRenderer::Refused(Refusal::NotSubmittable { why, .. })) if why.contains("dialog")
        ),
        "{refused:?}"
    );
    assert_eq!(heard(&renderer), "-");
}

#[test]
fn a_page_whose_script_stopped_sends_its_form_as_a_page_that_never_ran_script() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><form action=/sent><label>A <input name=a value=1></label>\
             <button>Send</button></form><script>{SAY}\
             document.addEventListener('submit', e => {{ e.preventDefault(); say('submit'); }});\
             </script>"
        ),
        false,
    )
    .expect("the page loads");
    renderer
        .event_loop()
        .expect("a scripted page")
        .stop_switch()
        .ask();
    // The click's task is where the page stops, before anybody heard it.
    let stopped = press(&mut renderer, "Send").expect("an operable control");
    assert!(
        stopped
            .issues
            .iter()
            .any(|line| line.contains("the page stopped")),
        "{stopped:?}"
    );
    assert_eq!(stopped.asked, None);

    // From then on nobody can cancel it, so the form's own rules decide.
    let pressed = press(&mut renderer, "Send").expect("an operable control");
    assert_eq!(heard(&renderer), "-", "no listener ran");
    assert!(
        matches!(
            pressed.outcome,
            Outcome::Submitted {
                method: Method::Get,
                ..
            }
        ),
        "{pressed:?}"
    );
    assert_eq!(written(pressed.asked.as_ref()), ["a=1"]);
}

#[test]
fn a_scripts_click_on_a_submit_button_is_refused_by_name_until_it_is_built() {
    let mut renderer = loaded(
        &format!(
            "<p>-</p><form action=/sent><button>Send</button></form>\
             <button id=other>Other</button><script>{SAY}\
             const send = document.querySelectorAll('button')[0];\
             document.getElementById('other').addEventListener('click', e => send.click());\
             document.addEventListener('submit', e => say('submit'));\
             </script>"
        ),
        false,
    )
    .expect("the page loads");
    let pressed = press(&mut renderer, "Other").expect("an operable control");
    assert_eq!(heard(&renderer), "-", "no submit was fired");
    assert_eq!(pressed.asked, None);
    assert!(
        pressed
            .issues
            .iter()
            .any(|line| line.contains("queue item 487")),
        "{pressed:?}"
    );
}
