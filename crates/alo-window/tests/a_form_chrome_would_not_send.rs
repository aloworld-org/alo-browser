/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A form Chrome would not send, answered as the engine answers it: queue
//! item 631, ADR 0051 § 6 and its amendment of 25 October 2026.
//!
//! The same form, pressed with an e-mail address that is not one and a
//! required field left empty, is answered `NotSubmitted` by this engine and
//! by the machine's Chrome: the engine with each control and its
//! constraint, Chrome with the first control and the words of the bubble it
//! draws, which are the engine's words for it. A page cannot forge the
//! bubble, a bubble left over is not the next press's, a page's own words
//! are carried as they are, and a press that sends the form still answers
//! `Submitted`.
//!
//! A failing control Chrome cannot give the keyboard — `hidden`, or inside
//! an `inert` part of the page — gets no bubble, and is known by what Chrome
//! logs of it instead (item 646): the press answers `NotSubmitted` with
//! Chrome's words and no box, beside a bubble's control or alone. A press
//! whose failing control's `invalid` the page cancels answers `Activated`.

use alo_agent::Outcome;
use alo_agent::verb::{Invalid, Target, Verb};
use alo_box::BoxId;
use alo_dom::constraint::Constraint;
use alo_layout::Size;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_window::chrome::Chrome;
use alo_window::chrome_tab::Tab;

/// The size the pages are laid out at.
const SIZE: (u32, u32) = (600, 400);

/// Where the pages say they are.
const AT: &str = "https://example.test/form";

/// A form whose e-mail address is not one and whose name is required and
/// empty, with a button that sends it.
const FORM: &str = r#"<!doctype html>
<html lang=en><title>A form</title>
<form action="/sent">
<label>Mail <input name=mail type=email value=nope></label>
<label>Name <input name=who required></label>
<button>Send</button>
</form>
"#;

fn named(name: &str) -> Target {
    Target::Named(name.to_owned())
}

/// `FORM`'s button pressed through this browser's own engine.
fn through_ours() -> Result<Outcome, String> {
    pressed_by_ours(FORM, "Send")
}

/// The button of `page` named `button`, pressed through this browser's own
/// engine.
fn pressed_by_ours(page: &str, button: &str) -> Result<Outcome, String> {
    let address = alo_url::parse(AT).map_err(|why| format!("{why:?}"))?;
    let mut renderer = Renderer::new(alo_corpus::corpus_fonts());
    let page = Page::new(page.to_owned(), Size::new(600.0, 400.0)).at(address);
    match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded { .. } => {}
        other => return Err(format!("the form did not load: {other:?}")),
    }
    match renderer.handle(ToRenderer::Act {
        target: named(button),
        verb: Verb::Activate,
    }) {
        FromRenderer::Acted { outcome, .. } => Ok(outcome),
        other => Err(format!("the press: {other:?}")),
    }
}

/// The node of the page in `tab` that reads as `line`, whatever states
/// follow it.
fn id_of(tab: &mut Tab<'_>, line: &str) -> Result<BoxId, String> {
    let read = tab.read().map_err(|why| why.to_string())?;
    read.nodes()
        .into_iter()
        .find(|node| {
            let read = node.to_string();
            read == line || read.starts_with(&format!("{line} ["))
        })
        .map(|node| node.id)
        .ok_or_else(|| format!("no {line} in {}", read.to_outline()))
}

fn opened<'c>(chrome: &'c mut Chrome, page: &str) -> Result<Tab<'c>, String> {
    Tab::open_frozen(chrome, AT, page.as_bytes().to_vec(), SIZE).map_err(|why| why.to_string())
}

#[test]
fn a_form_chrome_would_not_send_is_not_submitted_in_the_engines_words() {
    let ours = through_ours().unwrap();
    let Outcome::NotSubmitted { invalid, .. } = &ours else {
        panic!("the engine sent it: {ours:?}");
    };
    let said: Vec<_> = invalid
        .iter()
        .map(|one| (one.constraint, one.message.as_str()))
        .collect();
    assert_eq!(
        said,
        [
            (
                Some(Constraint::TypeMismatch),
                "Please include an '@' in the email address. 'nope' is missing an '@'."
            ),
            (
                Some(Constraint::ValueMissing),
                "Please fill out this field."
            ),
        ]
    );

    let mut chrome = Chrome::start().unwrap();
    let mut tab = opened(&mut chrome, FORM).unwrap();
    let button = id_of(&mut tab, r#"button "Send""#).unwrap();
    let mail = id_of(&mut tab, r#"textbox "Mail""#).unwrap();
    let name = id_of(&mut tab, r#"textbox "Name""#).unwrap();
    // Chrome tells of the first control and its words, which are the
    // engine's words for it; and of no constraint.
    let first = invalid.first().unwrap();
    let not_sent = |control: BoxId, message: &str| Outcome::NotSubmitted {
        node: button,
        invalid: vec![Invalid {
            control: Some(control),
            constraint: None,
            message: message.to_owned(),
        }],
    };
    assert_eq!(
        tab.act(&named("Send"), &Verb::Activate),
        Ok(not_sent(mail, &first.message))
    );
    // Pressed again, it is refused again, by a bubble of its own.
    assert_eq!(
        tab.act(&named("Send"), &Verb::Activate),
        Ok(not_sent(mail, &first.message))
    );
    // Put right, the next control is the one it stops at.
    tab.act(
        &named("Mail"),
        &Verb::PutText("ada@example.test".to_owned()),
    )
    .unwrap();
    assert_eq!(
        tab.act(&named("Send"), &Verb::Activate),
        Ok(not_sent(name, "Please fill out this field."))
    );
    // And filled in, it is sent.
    tab.act(&named("Name"), &Verb::PutText("Ada".to_owned()))
        .unwrap();
    let sent = tab.act(&named("Send"), &Verb::Activate).unwrap();
    assert!(
        matches!(&sent, Outcome::Submitted { to, .. }
            if to == "https://example.test/sent?mail=ada%40example.test&who=Ada"),
        "{sent:?}"
    );
}

/// A page with Chrome's words in an alert of its own, a button of no form
/// whose press shows it, a form whose `submit` it cancels, a form whose
/// field it gives words of its own, and a form whose `invalid` it cancels.
const FORGED: &str = r#"<!doctype html>
<html lang=en><title>Forged</title>
<div role=alert aria-live=assertive id=said></div>
<button type=button onclick="said.textContent = 'Please fill out this field. '">Say</button>
<form action="/one">
<label>First <input name=a required></label>
<button>Send first</button>
</form>
<form action="/two" onsubmit="event.preventDefault()"><button>Send second</button></form>
<form action="/three">
<label>Third <input name=c required oninvalid="this.setCustomValidity('Fill it in, please  ')"></label>
<button>Send third</button>
</form>
<form action="/four">
<label>Fourth <input name=d required oninvalid="event.preventDefault()"></label>
<button>Send fourth</button>
</form>
"#;

#[test]
fn only_chromes_own_new_bubble_says_a_form_was_not_sent() {
    let mut chrome = Chrome::start().unwrap();
    let mut tab = opened(&mut chrome, FORGED).unwrap();
    let activated = |tab: &mut Tab<'_>, button: &str| {
        let node = id_of(tab, &format!("button {button:?}")).unwrap();
        assert_eq!(
            tab.act(&named(button), &Verb::Activate),
            Ok(Outcome::Activated {
                node,
                name: Some(button.to_owned()),
            }),
            "{button}"
        );
    };
    // The page's own alert, in Chrome's words, is not Chrome's bubble.
    activated(&mut tab, "Say");
    // A bubble shown for one press is not the next press's: the second
    // form's `submit` is cancelled, and nothing was refused.
    let first = tab.act(&named("Send first"), &Verb::Activate).unwrap();
    assert!(
        matches!(&first, Outcome::NotSubmitted { invalid, .. } if invalid.len() == 1),
        "{first:?}"
    );
    activated(&mut tab, "Send second");
    // A page's own words are carried as they are, its spaces run
    // together as Chrome shows them, and no constraint is guessed at.
    let third = id_of(&mut tab, r#"textbox "Third""#).unwrap();
    let Ok(Outcome::NotSubmitted { invalid, .. }) = tab.act(&named("Send third"), &Verb::Activate)
    else {
        panic!("the third form was not refused");
    };
    assert_eq!(
        invalid,
        [Invalid {
            control: Some(third),
            constraint: None,
            message: "Fill it in, please".to_owned(),
        }]
    );
    // A form whose `invalid` the page cancels is not sent, and Chrome
    // shows nothing: answered as a press the page took (item 646).
    activated(&mut tab, "Send fourth");
}

/// A form whose only failing control is `hidden`; one whose shown control
/// and a `hidden` one after it fail; one whose failing control is inside an
/// `inert` part of the page; and one whose `hidden` failing control's
/// `invalid` the page cancels.
const UNFOCUSABLE: &str = r#"<!doctype html>
<html lang=en><title>Unfocusable</title>
<form action="/one">
<input name=a required hidden>
<label>Shown <input name=b value=x></label>
<button>Send first</button>
</form>
<form action="/two">
<label>Second <input name=c required></label>
<input name=d required hidden>
<button>Send second</button>
</form>
<form action="/three">
<div inert><label>Third <input name=e required></label></div>
<button>Send third</button>
</form>
<form action="/four">
<input name=f required hidden oninvalid="event.preventDefault()">
<button>Send fourth</button>
</form>
"#;

/// What Chrome logs of a failing control named `name` it cannot focus.
fn unfocusable(name: &str) -> Invalid {
    Invalid {
        control: None,
        constraint: None,
        message: format!("An invalid form control with name='{name}' is not focusable."),
    }
}

#[test]
fn a_failing_control_chrome_cannot_focus_is_told_of_by_what_chrome_logs() {
    // The engine: each failing control, the hidden ones with no box.
    let drawn = |page: &str, button: &str| {
        let ours = pressed_by_ours(page, button).unwrap();
        let Outcome::NotSubmitted { invalid, .. } = &ours else {
            panic!("the engine sent {button}: {ours:?}");
        };
        invalid
            .iter()
            .map(|one| (one.control.is_some(), one.constraint))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        drawn(UNFOCUSABLE, "Send first"),
        [(false, Some(Constraint::ValueMissing))]
    );
    assert_eq!(
        drawn(UNFOCUSABLE, "Send second"),
        [
            (true, Some(Constraint::ValueMissing)),
            (false, Some(Constraint::ValueMissing))
        ]
    );

    let mut chrome = Chrome::start().unwrap();
    let mut tab = opened(&mut chrome, UNFOCUSABLE).unwrap();
    let not_sent = |tab: &mut Tab<'_>, button: &str| {
        let node = id_of(tab, &format!("button {button:?}")).unwrap();
        match tab.act(&named(button), &Verb::Activate) {
            Ok(Outcome::NotSubmitted {
                node: pressed,
                invalid,
            }) if pressed == node => invalid,
            other => panic!("{button}: {other:?}"),
        }
    };
    // A hidden control alone: no bubble, and Chrome's words for it.
    assert_eq!(not_sent(&mut tab, "Send first"), [unfocusable("a")]);
    // Pressed again, it is told of once: what was logged before is not
    // this press's.
    assert_eq!(not_sent(&mut tab, "Send first"), [unfocusable("a")]);
    // Beside a bubble, after the control the bubble is about.
    let second = id_of(&mut tab, r#"textbox "Second""#).unwrap();
    assert_eq!(
        not_sent(&mut tab, "Send second"),
        [
            Invalid {
                control: Some(second),
                constraint: None,
                message: "Please fill out this field.".to_owned(),
            },
            unfocusable("d"),
        ]
    );
    // Drawn but inert: nothing an agent can name either.
    assert_eq!(not_sent(&mut tab, "Send third"), [unfocusable("e")]);
    // Its `invalid` cancelled, Chrome logs nothing: the page took the press.
    let fourth = id_of(&mut tab, r#"button "Send fourth""#).unwrap();
    assert_eq!(
        tab.act(&named("Send fourth"), &Verb::Activate),
        Ok(Outcome::Activated {
            node: fourth,
            name: Some("Send fourth".to_owned()),
        })
    );
}
