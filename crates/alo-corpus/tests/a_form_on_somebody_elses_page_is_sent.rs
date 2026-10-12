/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! An agent sending a form on a page nobody wrote for us: queue item 468,
//! ADR 0051 §§ 2–4 and 6.
//!
//! An agent submits as a keyboard user does, by activating the form's
//! submit button, and is told what came of it: the form was checked and
//! asked to go somewhere, or it was checked and said no, in Chrome's own
//! words. Nothing here encodes a body — that is the browser process's
//! (queue item 470, asserted on these same frozen pages in `alo-renderer`'s
//! `a_form_is_decided.rs`) — so what is asserted is the ask a renderer
//! makes: a method, a URL resolved against where the page was served, and
//! the entry list as names and values in Chrome's order.
//!
//! `web-a-form` runs no script, so its press is the whole algorithm.
//! `alo-sites-booking` runs an analytics script, so its press is a click its
//! script hears, and the submission runs inside the click's task with
//! `invalid` and `submit` fired between its steps (queue item 485). It is
//! pressed twice: on the same markup with that script taken out, which is
//! the scriptless half, and on the frozen bytes whole, with one observing
//! script after them that only listens and writes down what it heard.

use alo_agent::{Outcome, Refusal, Target, Verb};
use alo_corpus::{Case, cases_directory, corpus_fonts};
use alo_dom::constraint::Constraint;
use alo_dom::submission::{Entry, Method, Value};
use alo_layout::Size;
use alo_renderer::ask::Asked;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};

/// `html` loaded into a renderer of its own, at `address`.
fn at(html: String, size: (f32, f32), address: &str) -> Option<Renderer> {
    let address = alo_url::parse(address).ok()?;
    let mut renderer = Renderer::new(corpus_fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(
        Page::new(html, Size::new(size.0, size.1)).at(address),
    )));
    matches!(answer, FromRenderer::Loaded { .. }).then_some(renderer)
}

/// The frozen case `name`, loaded at the first line of its `origin.txt` or
/// at its `address.txt`.
fn loaded(name: &str) -> Option<Renderer> {
    let directory = cases_directory().join(name);
    let case = Case::read(&directory)?;
    let address = std::fs::read_to_string(directory.join("address.txt"))
        .or_else(|_| std::fs::read_to_string(directory.join("origin.txt")))
        .ok()?;
    at(case.html, case.size, address.lines().next()?.trim())
}

/// Ask for `verb` on whatever is called `name`.
fn act(renderer: &mut Renderer, name: &str, verb: Verb) -> FromRenderer {
    renderer.handle(ToRenderer::Act {
        target: Target::Named(name.to_owned()),
        verb,
    })
}

/// What a press came to, and the navigation its answer asked for.
fn pressed(renderer: &mut Renderer, name: &str) -> (Option<Outcome>, Option<Asked>, Vec<String>) {
    match act(renderer, name, Verb::Activate) {
        FromRenderer::Acted {
            outcome,
            navigation,
            issues,
            ..
        } => (Some(outcome), navigation, issues),
        _ => (None, None, Vec::new()),
    }
}

/// An entry list as `name=value` lines, a file entry naming no file as
/// `name=<no file>`.
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

#[test]
fn the_pizza_form_asks_to_post_what_chrome_posts() {
    let mut form = loaded("web-a-form").expect("web-a-form loads at its origin");
    let put = act(
        &mut form,
        "Customer name:",
        Verb::PutText("Ada Lovelace".to_owned()),
    );
    assert!(matches!(put, FromRenderer::Acted { .. }), "{put:?}");

    let (outcome, asked, issues) = pressed(&mut form, "Submit order");
    assert!(
        matches!(
            &outcome,
            Some(Outcome::Submitted { method: Method::Post, to, .. })
                if to == "https://httpbin.org/post"
        ),
        "{outcome:?} {issues:?}"
    );
    let asked = asked.expect("the press asked to go somewhere");
    assert_eq!(asked.url, "https://httpbin.org/post");
    assert_eq!(asked.replaced, 0);
    let submission = asked.submission.as_ref().expect("as a form's submission");
    assert_eq!(submission.method, Method::Post);
    assert_eq!(
        submission.enctype.media_type(),
        "application/x-www-form-urlencoded"
    );
    // Chrome 154 posted `custname=Ada+Lovelace&custtel=&custemail=&delivery=&comments=`
    // (ADR 0051, measured): the unchecked radios and boxes and the button
    // with no name send nothing, and the empty time field sends its name.
    assert_eq!(
        written(Some(&asked)),
        [
            "custname=Ada Lovelace",
            "custtel=",
            "custemail=",
            "delivery=",
            "comments="
        ]
    );
}

#[test]
fn the_pizza_form_sends_what_an_agent_chose_in_tree_order() {
    let mut form = loaded("web-a-form").expect("web-a-form loads at its origin");
    for (field, text) in [
        ("Customer name:", "Ada Lovelace"),
        ("Telephone:", "+44 20 7946 0000"),
        ("E-mail address:", "ada@example.com"),
        ("Delivery instructions:", "Ring twice"),
    ] {
        let put = act(&mut form, field, Verb::PutText(text.to_owned()));
        assert!(
            matches!(put, FromRenderer::Acted { .. }),
            "{field}: {put:?}"
        );
    }
    for choice in ["Medium", "Large", "Bacon", "Mushroom"] {
        let done = act(&mut form, choice, Verb::Activate);
        assert!(
            matches!(done, FromRenderer::Acted { .. }),
            "{choice}: {done:?}"
        );
    }
    let (outcome, asked, _) = pressed(&mut form, "Submit order");
    assert!(
        matches!(outcome, Some(Outcome::Submitted { .. })),
        "{outcome:?}"
    );
    // Chrome 154's `FormData` for the same choices on the frozen page.
    assert_eq!(
        written(asked.as_ref()),
        [
            "custname=Ada Lovelace",
            "custtel=+44 20 7946 0000",
            "custemail=ada@example.com",
            "size=large",
            "topping=bacon",
            "topping=mushroom",
            "delivery=",
            "comments=Ring twice"
        ]
    );
}

#[test]
fn an_address_that_is_not_one_stops_the_pizza_form_in_chromes_words() {
    let mut form = loaded("web-a-form").expect("web-a-form loads at its origin");
    let put = act(
        &mut form,
        "E-mail address:",
        Verb::PutText("nope".to_owned()),
    );
    assert!(matches!(put, FromRenderer::Acted { .. }), "{put:?}");
    let (outcome, asked, _) = pressed(&mut form, "Submit order");
    let Some(Outcome::NotSubmitted { invalid, .. }) = outcome else {
        panic!("the form was sent with an address that is not one: {outcome:?}");
    };
    assert_eq!(invalid.len(), 1, "{invalid:?}");
    let field = invalid.first().expect("one control");
    assert!(field.control.is_some(), "the field is named by its box");
    assert_eq!(field.constraint, Constraint::TypeMismatch);
    assert_eq!(
        field.message,
        "Please include an '@' in the email address. 'nope' is missing an '@'."
    );
    assert_eq!(asked, None, "and nothing was asked for");
}

#[test]
fn the_booking_form_is_checked_and_sent_as_chrome_sends_it() {
    let directory = cases_directory().join("alo-sites-booking");
    let case = Case::read(&directory).expect("alo-sites-booking is a case");
    // The same markup without its analytics script: the form's own rules,
    // with nobody listening.
    let (before, rest) = case
        .html
        .split_once("<script>")
        .expect("the page has its script");
    let (_, after) = rest.split_once("</script>").expect("and its end");
    let html = format!("{before}{after}");
    let mut page = at(html, case.size, "https://nordwind.alosites.com/").expect("it loads");

    // Chrome 154, with no day chosen: `invalid` at the date field, whose
    // `validationMessage` is this, and no request.
    let (outcome, asked, _) = pressed(&mut page, "See free times");
    let Some(Outcome::NotSubmitted { invalid, .. }) = outcome else {
        panic!("the form was sent with no day: {outcome:?}");
    };
    let date = invalid.first().expect("the date field");
    assert_eq!(invalid.len(), 1);
    assert_eq!(date.constraint, Constraint::ValueMissing);
    assert_eq!(date.message, "Please fill out this field.");
    assert_eq!(asked, None);

    let put = act(
        &mut page,
        "Choose a day",
        Verb::PutText("2026-10-20".to_owned()),
    );
    assert!(matches!(put, FromRenderer::Acted { .. }), "{put:?}");
    // Chrome 154 requested
    // `GET https://nordwind.alosites.com/b/studio-consultation?date=2026-10-20`.
    let (outcome, asked, _) = pressed(&mut page, "See free times");
    assert!(
        matches!(
            &outcome,
            Some(Outcome::Submitted { method: Method::Get, to, .. })
                if to == "https://nordwind.alosites.com/b/studio-consultation"
        ),
        "{outcome:?}"
    );
    assert_eq!(written(asked.as_ref()), ["date=2026-10-20"]);
}

/// What the observer appended to the frozen booking page writes down: each
/// `invalid` and `submit` it hears at the document, capturing, as the
/// event's type, whether it is trusted, bubbles and is cancelable, and its
/// target's `id` or tag — then, for a `submit`, the `id` of its submitter.
/// It only listens, and writes into an attribute of the document element,
/// which nothing draws.
const OBSERVER: &str = "<script>(function () {\
  var heard = document.documentElement;\
  function note(e) {\
    var said = e.type + ' ' + e.isTrusted + ' ' + e.bubbles + ' ' + e.cancelable + ' ' +\
      (e.target.getAttribute('id') || e.target.getAttribute('class'));\
    if (e.type === 'submit') { said = said + ' ' + e.submitter.textContent; }\
    heard.setAttribute('data-heard', (heard.getAttribute('data-heard') || '') + said + ';');\
  }\
  document.addEventListener('invalid', note, true);\
  document.addEventListener('submit', note, true);\
})();</script>";

/// What the observer has written down so far.
fn heard(renderer: &Renderer) -> String {
    renderer
        .document()
        .and_then(|document| {
            document
                .descendants(document.root())
                .find_map(|node| {
                    document
                        .element(node)
                        .filter(|element| element.name.is_html("html"))
                })
                .and_then(|html| html.attr("data-heard"))
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

#[test]
fn the_frozen_booking_page_runs_script_and_is_checked_and_sent_as_chrome_sends_it() {
    let directory = cases_directory().join("alo-sites-booking");
    let case = Case::read(&directory).expect("alo-sites-booking is a case");
    assert!(case.html.contains("<script>"), "the page has its script");
    let address = std::fs::read_to_string(directory.join("address.txt"))
        .expect("the case says where it was served");
    let address = address.lines().next().unwrap_or_default().trim();
    let mut page = at(format!("{}{OBSERVER}", case.html), case.size, address)
        .expect("the frozen page loads with its observer");

    // Chrome 154, with no day chosen: one trusted `invalid` at the date
    // field, cancelable and not bubbling, no `submit`, and no request; the
    // field's `validationMessage` is this.
    let (outcome, asked, issues) = pressed(&mut page, "See free times");
    let Some(Outcome::NotSubmitted { invalid, .. }) = outcome else {
        panic!("the form was sent with no day: {outcome:?} {issues:?}");
    };
    assert_eq!(invalid.len(), 1, "{invalid:?}");
    let date = invalid.first().expect("the date field");
    assert!(date.control.is_some(), "the field is named by its box");
    assert_eq!(date.constraint, Constraint::ValueMissing);
    assert_eq!(date.message, "Please fill out this field.");
    assert_eq!(asked, None, "and nothing was asked for");
    assert_eq!(heard(&page), "invalid true false true booking-0-date;");

    let put = act(
        &mut page,
        "Choose a day",
        Verb::PutText("2026-10-20".to_owned()),
    );
    assert!(matches!(put, FromRenderer::Acted { .. }), "{put:?}");
    // Chrome 154 requested
    // `GET https://nordwind.alosites.com/b/studio-consultation?date=2026-10-20`,
    // after a trusted `submit` that bubbles, is cancelable and names the
    // button as its submitter.
    let (outcome, asked, issues) = pressed(&mut page, "See free times");
    assert!(
        matches!(
            &outcome,
            Some(Outcome::Submitted { method: Method::Get, to, .. })
                if to == "https://nordwind.alosites.com/b/studio-consultation"
        ),
        "{outcome:?} {issues:?}"
    );
    let asked = asked.expect("the press asked to go somewhere");
    assert_eq!(
        asked.url,
        "https://nordwind.alosites.com/b/studio-consultation"
    );
    assert_eq!(asked.replaced, 0);
    assert_eq!(written(Some(&asked)), ["date=2026-10-20"]);
    assert_eq!(
        heard(&page),
        "invalid true false true booking-0-date;\
         submit true true true booking-day See free times;"
    );
}

#[test]
fn text_past_a_fields_maxlength_is_refused_rather_than_cut() {
    let mut page = loaded("alo-sites-contact-form").expect("alo-sites-contact-form loads");
    let longest = "a".repeat(300);
    let put = act(&mut page, "Name", Verb::PutText(longest.clone()));
    assert!(
        matches!(put, FromRenderer::Acted { .. }),
        "300 fits: {put:?}"
    );
    let refused = act(&mut page, "Name", Verb::PutText(format!("{longest}b")));
    assert!(
        matches!(
            refused,
            FromRenderer::Refused(Refusal::TooLong {
                most: 300,
                length: 301,
                ..
            })
        ),
        "{refused:?}"
    );
}

#[test]
fn the_contact_form_whose_script_stopped_is_sent_as_a_page_without_script_sends_it() {
    let mut page = loaded("alo-sites-contact-form").expect("alo-sites-contact-form loads");
    for (field, text) in [
        ("Name", "Ada Lovelace"),
        ("Email", "ada@example.com"),
        ("Message", "Two kilos a week, please."),
    ] {
        let put = act(&mut page, field, Verb::PutText(text.to_owned()));
        assert!(
            matches!(put, FromRenderer::Acted { .. }),
            "{field}: {put:?}"
        );
    }
    // The page's first script adds the `submit` listener that would send
    // the form by `fetch`; its second, the analytics, stops the page as it
    // loads, on a property of a primitive (queue item 73). Nobody is left to
    // hear the press, so the form's own rules decide, as for a page that
    // never ran script (ADR 0016 § 7): the fields pass, and it asks to `POST`
    // what its markup says, the hidden honeypot field included.
    let (outcome, asked, issues) = pressed(&mut page, "Send");
    assert!(
        matches!(
            &outcome,
            Some(Outcome::Submitted { method: Method::Post, to, .. })
                if to == "https://nordwind.alosites.com/f/f4K9sL2wN7qR5tYx8vB1cA"
        ),
        "{outcome:?} {issues:?}"
    );
    assert!(
        issues
            .iter()
            .any(|line| line.contains("nobody heard it") && line.contains("queue item 73")),
        "and the answer says why nobody heard: {issues:?}"
    );
    assert_eq!(
        written(asked.as_ref()),
        [
            "website=",
            "name=Ada Lovelace",
            "email=ada@example.com",
            "message=Two kilos a week, please."
        ]
    );
}
