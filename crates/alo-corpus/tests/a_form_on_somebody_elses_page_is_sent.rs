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
//!
//! `alo-sites-contact-form` cancels the press and sends itself by `fetch`
//! with `new URLSearchParams(new FormData(form))` (queue item 472): its
//! request is asserted as Chrome 154 made it, and the case's frozen answer
//! puts its thanks where the form was, laid out where Chrome lays it out and
//! drawn as `sent.png` says.

use alo_agent::{Outcome, Refusal, Target, Verb};
use alo_corpus::{Case, answering, cases_directory, corpus_fonts};
use alo_dom::constraint::Constraint;
use alo_dom::submission::{Entry, Method, Value};
use alo_layout::Size;
use alo_paint::{Canvas, from_png, to_png};
use alo_renderer::ask::Asked;
use alo_renderer::fetch::FetchAsk;
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

/// Where alo Sites' analytics script reports to: its own server's
/// `/_alo/collect`.
const COLLECT: &str = "https://nordwind.alosites.com/_alo/collect";

/// The bodies of those of `asks` that are the analytics script's beacons.
fn reports(asks: &[FetchAsk]) -> Vec<String> {
    asks.iter()
        .filter(|ask| ask.url == COLLECT && ask.method == "POST")
        .map(|ask| String::from_utf8_lossy(&ask.body).into_owned())
        .collect()
}

/// The contact form loaded at its address with its sheet answered, as the
/// corpus loads it, filled as Chrome 154's was, and the analytics beacons
/// its load and its first edit asked for.
fn contact_form_filled() -> Option<(Case, alo_url::Url, Renderer, Vec<String>)> {
    let directory = cases_directory().join("alo-sites-contact-form");
    let case = Case::read(&directory)?;
    let address = alo_url::parse(case.address.as_deref()?).ok()?;
    let mut page = Renderer::new(corpus_fonts());
    let FromRenderer::Loaded {
        fetches, sheets, ..
    } = page.handle(ToRenderer::Load(Box::new(
        Page::new(case.html.clone(), Size::new(case.size.0, case.size.1)).at(address.clone()),
    )))
    else {
        return None;
    };
    let mut beacons = reports(&fetches);
    let beside = answering::Beside {
        responses: &case.responses,
        files: &case.frozen,
    };
    let asks = answering::Asks { fetches, sheets };
    let loaded = answering::answer(&mut page, &address, beside, asks).ok()?;
    (loaded.sheets == 1).then_some(())?;
    for (field, text) in [
        ("Name", "Ada Lovelace"),
        ("Email", "ada@example.com"),
        ("Message", "Two kilos a week, please."),
    ] {
        let FromRenderer::Acted { fetches, .. } =
            act(&mut page, field, Verb::PutText(text.to_owned()))
        else {
            return None;
        };
        beacons.extend(reports(&fetches));
    }
    Some((case, address, page, beacons))
}

/// The text of every element of `page`'s document whose `class` is
/// `class`, in document order.
fn with_class(page: &Renderer, class: &str) -> Vec<String> {
    let Some(document) = page.document() else {
        return Vec::new();
    };
    document
        .descendants(document.root())
        .filter(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.attr("class") == Some(class))
        })
        .map(|node| document.text_content(node))
        .collect()
}

/// How many `<form>` elements `page`'s document holds.
fn forms(page: &Renderer) -> usize {
    page.document().map_or(0, |document| {
        document
            .descendants(document.root())
            .filter(|node| {
                document
                    .element(*node)
                    .is_some_and(|element| element.name.is_html("form"))
            })
            .count()
    })
}

#[test]
fn the_contact_forms_own_listener_sends_it_by_fetch_as_chrome_does() {
    let (case, address, mut page, beacons) =
        contact_form_filled().expect("alo-sites-contact-form loads and is filled");
    // The analytics script runs to its end: its last loop cuts `/f/` off the
    // form's action with `slice(3)`, a property of a string (queue item
    // 488), reports that the form was seen, and its `input` listener, added
    // `once`, reports the first edit and no other.
    assert_eq!(
        beacons,
        [
            "c=f4K9sL2wN7qR5tYx8vB1cA&s=view",
            "c=f4K9sL2wN7qR5tYx8vB1cA&s=start"
        ]
    );

    // The page's first script hears the press, cancels the submission and
    // sends the form itself: `fetch(action, { method: "POST", body: new
    // URLSearchParams(new FormData(form)) })` (queue item 472). Chrome 154
    // asked for exactly this — the honeypot's empty `website` first, as the
    // form holds it.
    let FromRenderer::Acted {
        outcome,
        navigation,
        issues,
        fetches,
        ..
    } = act(&mut page, "Send", Verb::Activate)
    else {
        panic!("the press is answered");
    };
    assert!(
        matches!(&outcome, Outcome::Activated { name: Some(name), .. } if name == "Send"),
        "{outcome:?} {issues:?}"
    );
    assert!(navigation.is_none(), "{navigation:?}");
    assert!(
        issues
            .iter()
            .any(|line| line.contains("form was not sent") && line.contains("cancelled")),
        "the answer says the page took the press: {issues:?}"
    );
    assert!(
        !issues.iter().any(|line| line.contains("Error")),
        "the listener runs to its end: {issues:?}"
    );
    let sent: Vec<&FetchAsk> = fetches.iter().filter(|ask| ask.url != COLLECT).collect();
    assert_eq!(sent.len(), 1, "{fetches:?}");
    let ask = sent[0];
    assert_eq!(ask.method, "POST");
    assert_eq!(
        ask.url,
        "https://nordwind.alosites.com/f/f4K9sL2wN7qR5tYx8vB1cA"
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
         &message=Two+kilos+a+week%2C+please."
    );
    assert!(
        reports(&fetches).is_empty(),
        "a keyboard's click reports nothing"
    );

    // Answered with what alo Sites' server sends for a submission it kept,
    // the listener puts its thanks where the form was, as Chrome 154's did:
    // `<p class="form-success">Thanks — talk soon.</p>`, the form gone.
    let answered = answering::answer(
        &mut page,
        &address,
        answering::Beside {
            responses: &case.responses,
            files: &[],
        },
        answering::Asks {
            fetches,
            sheets: Vec::new(),
        },
    )
    .expect("the case answers the listener's fetch");
    assert_eq!(answered.delivered, 1);
    assert!(answered.unfrozen.is_empty(), "{:?}", answered.unfrozen);
    assert!(answered.issues.is_empty(), "{:?}", answered.issues);
    assert_eq!(forms(&page), 0, "the form is gone");
    assert_eq!(with_class(&page, "form-success"), ["Thanks — talk soon."]);
}

#[test]
fn the_contact_form_offline_is_left_as_it_was() {
    let (_, address, mut page, _) =
        contact_form_filled().expect("alo-sites-contact-form loads and is filled");
    let FromRenderer::Acted { fetches, .. } = act(&mut page, "Send", Verb::Activate) else {
        panic!("the press is answered");
    };
    // With nothing frozen the fetch is a network error, and the listener's
    // `.catch` calls `form.submit()`. Chrome would then post the form as a
    // navigation; here `submit` is absent until queue item 471, so the
    // call is a `TypeError` the delivery reports, the form stays, and
    // nothing more is asked for.
    let answered = answering::answer(
        &mut page,
        &address,
        answering::Beside {
            responses: &[],
            files: &[],
        },
        answering::Asks {
            fetches,
            sheets: Vec::new(),
        },
    )
    .expect("the case answers the listener's fetch");
    assert_eq!(
        answered.unfrozen,
        ["https://nordwind.alosites.com/f/f4K9sL2wN7qR5tYx8vB1cA"]
    );
    assert_eq!(
        answered.issues,
        ["the answer to a fetch: uncaught: TypeError: undefined is not a function"]
    );
    assert_eq!(answered.delivered, 1, "and nothing more was asked for");
    assert_eq!(forms(&page), 1, "the form stays");
    assert!(with_class(&page, "form-success").is_empty());
}

/// The contact form after its press, its fetch answered with what was
/// frozen: the page as a person sees it once their message is sent.
fn sent() -> Option<Renderer> {
    let (case, address, mut page, _) = contact_form_filled()?;
    let FromRenderer::Acted { fetches, .. } = act(&mut page, "Send", Verb::Activate) else {
        return None;
    };
    let beside = answering::Beside {
        responses: &case.responses,
        files: &[],
    };
    let asks = answering::Asks {
        fetches,
        sheets: Vec::new(),
    };
    answering::answer(&mut page, &address, beside, asks).ok()?;
    Some(page)
}

/// The border box of the first element of `page` whose name is `tag` and
/// whose `class` is `class`, as `(x, y, width, height)`.
fn box_of(page: &Renderer, tag: &str, class: Option<&str>) -> Option<(f32, f32, f32, f32)> {
    let (document, drawing) = (page.document()?, page.rendered()?);
    let node = document.descendants(document.root()).find(|node| {
        document
            .element(*node)
            .is_some_and(|element| element.name.is_html(tag) && element.attr("class") == class)
    })?;
    let boxes = &drawing.boxes;
    let id = boxes.ids().find(|id| {
        matches!(
            boxes.get(*id).map(|box_node| &box_node.kind),
            Some(alo_box::BoxKind::Element { node: of, .. }) if *of == node
        )
    })?;
    let rect = drawing.layout.get(id)?.border_box;
    Some((
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    ))
}

#[test]
fn the_thanks_takes_the_forms_place_where_chrome_puts_it() {
    let mut page = sent().expect("the contact form is sent and answered");
    // Chrome 154, at 800 × 600 with the corpus's own `DejaVu Sans`, in its
    // 1/64 px units: with the form gone from column two, the introduction
    // is placed beside the heading and the thanks under it, in column one —
    // the sheet's `.form-success` gives it a padded, bordered box but no
    // place, so it is placed as any grid item is — and the section is
    // 303.375 tall. Chrome's picture of the same moment differs from
    // `sent.png` only inside the glyphs (queue item 462).
    for (tag, class, chrome) in [
        (
            "section",
            Some("s-contact-form contact-split"),
            [0, 0, 51_200, 19_416],
        ),
        ("h2", None, [1_280, 3_072, 18_227, 4_300]),
        ("p", None, [22_579, 3_072, 27_341, 1_740]),
        ("p", Some("form-success"), [1_280, 11_340, 18_227, 3_916]),
    ] {
        let found = box_of(&page, tag, class).expect("the element has a box");
        let found = [found.0, found.1, found.2, found.3];
        for (ours, theirs) in found.iter().zip(chrome) {
            let theirs = f32::from(u16::try_from(theirs).expect("a small number")) / 64.0;
            assert!(
                (ours - theirs).abs() <= 1.0 / 64.0,
                "{tag} {class:?}: {found:?} against Chrome's {chrome:?}"
            );
        }
    }

    // What an agent reads now: the thanks, and no form left to press.
    let FromRenderer::Tree(tree) = page.handle(ToRenderer::ReadTree) else {
        panic!("the tree is read");
    };
    let outline = tree.to_outline();
    assert!(outline.contains("Thanks — talk soon."), "{outline}");
    assert!(!outline.contains("\"Send\""), "{outline}");
}

#[test]
fn the_sent_page_is_drawn_as_its_reference_says() {
    // A reference render of the page once sent, beside the case's own
    // `render.png` of it as loaded: a change that moves a pixel of the
    // thanks says so. Rewritten, like every reference, with
    // `ALO_UPDATE_REFERENCES` set.
    let page = sent().expect("the contact form is sent and answered");
    let Some(drawing) = page.rendered() else {
        panic!("the page is drawn");
    };
    let path = cases_directory()
        .join("alo-sites-contact-form")
        .join("sent.png");
    if std::env::var_os("ALO_UPDATE_REFERENCES").is_some() {
        let written = to_png(&drawing.canvas)
            .ok()
            .and_then(|bytes| std::fs::write(&path, bytes).ok());
        assert!(written.is_some(), "{} is written", path.display());
        return;
    }
    let reference: Option<Canvas> = std::fs::read(&path)
        .ok()
        .and_then(|bytes| from_png(&bytes).ok());
    let Some(reference) = reference else {
        panic!("{} is the reference", path.display());
    };
    assert_eq!(
        (reference.width(), reference.height()),
        (drawing.canvas.width(), drawing.canvas.height())
    );
    let differing = reference
        .pixels()
        .iter()
        .zip(drawing.canvas.pixels())
        .filter(|(want, got)| want.to_rgba8() != got.to_rgba8())
        .count();
    assert_eq!(differing, 0, "pixels differ from {}", path.display());
}
