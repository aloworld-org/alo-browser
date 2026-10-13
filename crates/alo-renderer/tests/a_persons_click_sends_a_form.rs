/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 543: a person's click on a frozen form's submit button sends
//! it as an agent's `Activate` does, with what this engine cannot send
//! said, and the submission decided as `Cause::Person` (ADR 0051 §§ 4–6,
//! ADR 0012 § 2).
//!
//! Driven through real [`Tabs`] over the real confined `alo-render` binary.
//! The fields are filled by an agent's `PutText`, since a person's keys do
//! not reach a page yet; the press and release are a person's, at a point
//! inside the button's box, which is asserted in numbers first — the box
//! each frozen case's `agent.txt` holds.
//!
//! Chrome 154, clicked with `Input.dispatchMouseEvent` on the same two
//! frozen pages and its requests intercepted, sent what ADR 0051 measured
//! for a press: the booking form with no day fired `invalid` at the day
//! field and requested nothing; with a day it requested the `GET` below;
//! the pizza form posted the body below; and a cancelled `submit` sent
//! nothing.

use alo_agent::{Outcome, Target, Verb};
use alo_dom::submission::Method;
use alo_layout::{Rect, Size};
use alo_net::cause::Cause;
use alo_renderer::face::Face;
use alo_renderer::fonts::Machine;
use alo_renderer::generic::Generics;
use alo_renderer::host::Renderers;
use alo_renderer::navigate::{Decided, Navigation};
use alo_renderer::pointer::Pointer;
use alo_renderer::tab::{TabId, Tabs};
use alo_renderer::{FromRenderer, Page};
use alo_text::{Slant, Weight};
use alo_url::Url;
use std::path::PathBuf;

/// The renderer binary, as cargo built it for this test, handed the fonts
/// the corpus is laid out with, so that a button is where its case has it.
fn tabs() -> Tabs {
    let faces = [
        ("DejaVu Sans", Weight::NORMAL, dejavu::sans::regular()),
        ("DejaVu Sans", Weight::BOLD, dejavu::sans::bold()),
        (
            "DejaVu Sans Mono",
            Weight::NORMAL,
            dejavu::sans_mono::regular(),
        ),
    ]
    .into_iter()
    .filter_map(|(family, weight, bytes)| Face::new(family, weight, Slant::Normal, bytes.to_vec()))
    .collect();
    let generics = Generics::stating(
        [
            ("sans-serif", "DejaVu Sans"),
            ("system-ui", "DejaVu Sans"),
            ("monospace", "DejaVu Sans Mono"),
        ]
        .into_iter()
        .map(|(generic, family)| (generic.to_owned(), family.to_owned()))
        .collect(),
    );
    Tabs::over(
        Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[])
            .with_machine(Machine { faces, generics }),
    )
}

/// A URL, or `about:blank` if the text was wrong, so a mistake shows up as
/// the assertion it broke.
fn url(text: &str) -> Url {
    alo_url::parse(text).unwrap_or_else(|_| Url::about_blank())
}

/// A frozen case's file, as it was frozen.
fn frozen(case: &str, file: &str) -> String {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "alo-corpus",
        "cases",
        case,
        file,
    ]
    .iter()
    .collect();
    std::fs::read_to_string(&path).unwrap_or_default()
}

/// A frozen case's viewport.
fn size(case: &str) -> Size {
    let written = frozen(case, "size.txt");
    let (width, height) = written.trim().split_once('x').unwrap_or(("800", "600"));
    Size::new(
        width.parse().unwrap_or(800.0),
        height.parse().unwrap_or(600.0),
    )
}

/// A frozen case's page at the address its first line of `file` gives,
/// with the sheet the case's `site.css` holds, if it has one, as the
/// corpus lays it out.
fn case(name: &str, file: &str) -> Page {
    let address = frozen(name, file);
    let page = Page::new(frozen(name, "page.html"), size(name)).at(url(address
        .lines()
        .next()
        .unwrap_or_default()
        .trim()));
    match frozen(name, "site.css") {
        sheet if sheet.is_empty() => page,
        sheet => page.with_sheet(sheet),
    }
}

/// A tab showing `page`, loaded as a person's — [`None`] if it did not load.
fn showing(tabs: &mut Tabs, page: Page) -> Option<TabId> {
    let tab = tabs.open(page.url.clone());
    let loaded = tabs.load(tab, page, Cause::Person { tab });
    matches!(loaded, Ok(FromRenderer::Loaded { .. })).then_some(tab)
}

/// Put `text` into whatever is called `name`, as an agent: whether it went.
fn put(tabs: &mut Tabs, tab: TabId, name: &str, text: &str) -> bool {
    matches!(
        tabs.act(
            tab,
            Target::Named(name.to_owned()),
            Verb::PutText(text.to_owned())
        ),
        Ok((_, FromRenderer::Acted { .. }))
    )
}

/// Where whatever is called `name` is on the page, as the agent reads it.
fn where_is(tabs: &mut Tabs, tab: TabId, name: &str) -> Option<Rect> {
    let Ok(FromRenderer::Tree(snapshot)) = tabs.read(tab) else {
        return None;
    };
    snapshot
        .nodes()
        .into_iter()
        .find(|node| node.name.as_deref() == Some(name))
        .map(|node| node.rect)
}

/// Whether `rect` is at (`x`, `y`), `width` × `height`, to Chrome's 1/64 px.
fn at(rect: Option<Rect>, x: f32, y: f32, width: f32, height: f32) -> bool {
    let near = |a: f32, b: f32| (a - b).abs() < 1.0 / 64.0;
    rect.is_some_and(|rect| {
        near(rect.left(), x)
            && near(rect.top(), y)
            && near(rect.size.width, width)
            && near(rect.size.height, height)
    })
}

/// A person's press and release at the middle of `rect`: what the release
/// said — [`None`] if it was not answered as a delivery — and what this
/// process decided of where the page asked to go.
fn clicked(tabs: &mut Tabs, tab: TabId, rect: Rect) -> (Option<Vec<String>>, Option<Decided>) {
    let x = rect.left() + rect.size.width / 2.0;
    let y = rect.top() + rect.size.height / 2.0;
    let down = tabs.point(tab, Pointer::down(x, y));
    if !matches!(down, Ok(FromRenderer::Delivered { .. })) {
        return (None, None);
    }
    let said = match tabs.point(tab, Pointer::up(x, y)) {
        Ok(FromRenderer::Delivered { issues, .. }) => Some(issues),
        _ => None,
    };
    (said, tabs.navigation(tab))
}

/// An agent's press on whatever is called `name`, and what this process
/// decided — [`None`] unless it was answered as a submission.
fn pressed(tabs: &mut Tabs, tab: TabId, name: &str) -> Option<Decided> {
    let answered = tabs.act(tab, Target::Named(name.to_owned()), Verb::Activate);
    matches!(
        answered,
        Ok((
            _,
            FromRenderer::Acted {
                outcome: Outcome::Submitted { .. },
                ..
            }
        ))
    )
    .then(|| tabs.navigation(tab))
    .flatten()
}

/// What two decided navigations send, which must be the same whoever
/// pressed: where, how, what, and the `Referer`.
fn sends(navigation: &Navigation) -> (String, Method, Option<Vec<u8>>, Option<String>) {
    (
        navigation.url.serialised.clone(),
        navigation.method,
        navigation.body.as_ref().map(|body| body.bytes.clone()),
        navigation.referrer.clone(),
    )
}

#[test]
fn the_pizza_form_a_person_sends_is_posted_as_the_agents_and_is_the_persons() {
    let mut tabs = tabs();
    let tab = showing(&mut tabs, case("web-a-form", "origin.txt")).expect("the page loads");
    assert!(put(&mut tabs, tab, "Customer name:", "Ada Lovelace"));
    let button = where_is(&mut tabs, tab, "Submit order");
    // `web-a-form`'s `agent.txt`.
    assert!(
        at(button, 8.0, 603.1875, 103.078_125, 21.0),
        "the button is where the case has it: {button:?}"
    );
    let (said, decided) = clicked(&mut tabs, tab, button.expect("a button"));
    assert_eq!(said, Some(Vec::new()), "a form sent says nothing");
    let Some(Decided::Go(person)) = decided else {
        panic!("the form was not sent: {decided:?}");
    };
    assert_eq!(person.cause, Cause::Person { tab });
    assert!(person.form);
    // Chrome 154, clicked: this address, this body, this type.
    assert_eq!(person.url, url("https://httpbin.org/post"));
    assert_eq!(person.method, Method::Post);
    let body = person.body.as_ref().expect("a POST with a body");
    assert_eq!(body.content_type, "application/x-www-form-urlencoded");
    assert_eq!(
        String::from_utf8_lossy(&body.bytes),
        "custname=Ada+Lovelace&custtel=&custemail=&delivery=&comments="
    );

    let other = showing(&mut tabs, case("web-a-form", "origin.txt")).expect("the page loads");
    assert!(put(&mut tabs, other, "Customer name:", "Ada Lovelace"));
    let Some(Decided::Go(agent)) = pressed(&mut tabs, other, "Submit order") else {
        panic!("the agent's press sent nothing");
    };
    assert_eq!(
        sends(&person),
        sends(&agent),
        "a person sends what an agent does"
    );
    assert!(matches!(agent.cause, Cause::Agent { .. }));
}

#[test]
fn the_booking_form_a_person_sends_goes_by_get_with_the_day_as_its_query() {
    let page = case("alo-sites-booking", "address.txt");
    assert!(
        page.html.contains("<script>"),
        "the frozen page has lost its script"
    );
    let mut tabs = tabs();
    let tab = showing(&mut tabs, page).expect("the page loads");
    assert!(put(&mut tabs, tab, "Choose a day", "2026-10-20"));
    let button = where_is(&mut tabs, tab, "See free times");
    // `alo-sites-booking`'s `agent.txt`.
    assert!(
        at(button, 432.0, 186.968_75, 179.671_88, 48.375),
        "the button is where the case has it: {button:?}"
    );
    let (said, decided) = clicked(&mut tabs, tab, button.expect("a button"));
    assert!(said.is_some(), "the release was not answered as a delivery");
    let Some(Decided::Go(navigation)) = decided else {
        panic!("the form was not sent: {decided:?}");
    };
    // Chrome 154, clicked, requested exactly this.
    assert_eq!(
        navigation.url.serialised,
        "https://nordwind.alosites.com/b/studio-consultation?date=2026-10-20"
    );
    assert_eq!(navigation.method, Method::Get);
    assert_eq!(navigation.body, None);
    assert!(navigation.form, "a form sent by GET is still a form");
    assert_eq!(navigation.cause, Cause::Person { tab });
}

#[test]
fn the_booking_form_with_no_day_is_not_sent_and_says_which_field() {
    let mut tabs = tabs();
    let tab = showing(&mut tabs, case("alo-sites-booking", "address.txt")).expect("the page loads");
    let button = where_is(&mut tabs, tab, "See free times").expect("a button");
    let (said, decided) = clicked(&mut tabs, tab, button);
    assert_eq!(decided, None, "Chrome 154 requested nothing");
    let said = said.expect("the release was answered as a delivery");
    assert!(
        said.iter().any(|line| line
            == "the person's click: its form was not sent: input name=\"date\" valueMissing: \
                \"Please fill out this field.\""),
        "{said:?}"
    );
}
