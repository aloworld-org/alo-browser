/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 470: the browser process decides a form's submission (ADR
//! 0051 § 5).
//!
//! *`web-a-form`'s ask from 468 is decided as `Cause::Agent` with Chrome's
//! exact body and `Content-Type: application/x-www-form-urlencoded`;
//! `alo-sites-booking`'s as a `GET` of its action with the date as the
//! query; a query on the action is replaced, as measured; an `http` action
//! from an `https` page is refused and recorded.*
//!
//! Driven through real [`Tabs`] over the real confined `alo-render` binary,
//! as a person's tab would be: the agent fills and presses in the renderer,
//! the renderer asks with names and values, and this process writes the
//! request and decides it, with the cause it assigns from which message was
//! answered. The two frozen pages are read from `alo-corpus`'s cases, where
//! they were frozen; nothing here touches the network.

use alo_agent::{Outcome, Target, Verb};
use alo_dom::submission::Method;
use alo_layout::Size;
use alo_net::activity::{Activity, Happened};
use alo_net::cause::{ActionId, Cause, DocumentId};
use alo_renderer::host::Renderers;
use alo_renderer::navigate::{Decided, Rule};
use alo_renderer::tab::{Tab, TabId, Tabs};
use alo_renderer::{FromRenderer, Page};
use alo_url::Url;
use std::path::PathBuf;
use std::time::SystemTime;

/// The renderer binary, as cargo built it for this test.
fn tabs() -> Tabs {
    Tabs::over(Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[]))
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

/// A tab showing `page`, loaded as a person's, and its id.
fn showing(tabs: &mut Tabs, page: Page) -> TabId {
    let tab = tabs.open(page.url.clone());
    let loaded = tabs.load(tab, page, Cause::Person { tab });
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    assert_eq!(tabs.navigation(tab), None, "the load asked for nothing");
    tab
}

/// Ask for `verb` on whatever is called `name`, and the action it was —
/// [`None`] when the tab could not be asked.
fn act(tabs: &mut Tabs, tab: TabId, name: &str, verb: Verb) -> Option<(ActionId, FromRenderer)> {
    tabs.act(tab, Target::Named(name.to_owned()), verb).ok()
}

/// Press `name`: the action it was, whether the renderer answered it as a
/// submission, and what this process decided.
fn pressed(tabs: &mut Tabs, tab: TabId, name: &str) -> (Option<ActionId>, bool, Option<Decided>) {
    let answered = act(tabs, tab, name, Verb::Activate);
    let submitted = matches!(
        answered,
        Some((
            _,
            FromRenderer::Acted {
                outcome: Outcome::Submitted { .. },
                ..
            }
        ))
    );
    (
        answered.map(|(action, _)| action),
        submitted,
        tabs.navigation(tab),
    )
}

/// The document a tab is showing.
fn document_of(tabs: &Tabs, tab: TabId) -> Option<DocumentId> {
    tabs.tab(tab).and_then(Tab::document)
}

#[test]
fn the_pizza_form_is_sent_by_post_with_chromes_body_as_the_agents() {
    let mut tabs = tabs();
    let origin = frozen("web-a-form", "origin.txt");
    let at = url(origin.lines().next().unwrap_or_default().trim());
    let tab = showing(
        &mut tabs,
        Page::new(frozen("web-a-form", "page.html"), size("web-a-form")).at(at),
    );
    let put = act(
        &mut tabs,
        tab,
        "Customer name:",
        Verb::PutText("Ada Lovelace".to_owned()),
    );
    assert!(
        matches!(put, Some((_, FromRenderer::Acted { .. }))),
        "{put:?}"
    );
    assert_eq!(tabs.navigation(tab), None, "typing asked for nothing");

    let (action, submitted, decided) = pressed(&mut tabs, tab, "Submit order");
    assert!(submitted, "the renderer did not answer a submission");
    let Some(Decided::Go(navigation)) = decided else {
        panic!("the form was not sent: {decided:?}");
    };
    let (Some(action), Some(document)) = (action, document_of(&tabs, tab)) else {
        panic!("no action or no document");
    };
    assert_eq!(navigation.cause, Cause::Agent { action, document });
    assert_eq!(navigation.url, url("https://httpbin.org/post"));
    assert_eq!(navigation.method, Method::Post);
    let Some(body) = navigation.body.as_ref() else {
        panic!("a POST with no body");
    };
    // Chrome 154 (ADR 0051, *Measured*): this body, this type, and no
    // charset parameter.
    assert_eq!(body.content_type, "application/x-www-form-urlencoded");
    assert_eq!(
        String::from_utf8_lossy(&body.bytes),
        "custname=Ada+Lovelace&custtel=&custemail=&delivery=&comments="
    );
    assert_eq!(
        navigation.referrer.as_deref(),
        Some("https://httpbin.org/forms/post")
    );
}

#[test]
fn the_booking_form_is_sent_by_get_with_the_day_as_its_query() {
    let html = frozen("alo-sites-booking", "page.html");
    // The same markup without its analytics script: a page that runs script
    // submits by queue item 485, and this is the form's own submission.
    let Some((before, rest)) = html.split_once("<script>") else {
        panic!("the frozen page has lost its script");
    };
    let Some((_, after)) = rest.split_once("</script>") else {
        panic!("and its end");
    };
    let address = frozen("alo-sites-booking", "address.txt");
    let at = url(address.lines().next().unwrap_or_default().trim());
    let mut tabs = tabs();
    let tab = showing(
        &mut tabs,
        Page::new(format!("{before}{after}"), size("alo-sites-booking")).at(at),
    );
    let put = act(
        &mut tabs,
        tab,
        "Choose a day",
        Verb::PutText("2026-10-20".to_owned()),
    );
    assert!(
        matches!(put, Some((_, FromRenderer::Acted { .. }))),
        "{put:?}"
    );

    let (action, submitted, decided) = pressed(&mut tabs, tab, "See free times");
    assert!(submitted, "the renderer did not answer a submission");
    let Some(Decided::Go(navigation)) = decided else {
        panic!("the form was not sent: {decided:?}");
    };
    // Chrome 154 requested exactly this.
    assert_eq!(
        navigation.url.serialised,
        "https://nordwind.alosites.com/b/studio-consultation?date=2026-10-20"
    );
    assert_eq!(navigation.method, Method::Get);
    assert_eq!(navigation.body, None);
    let (Some(action), Some(document)) = (action, document_of(&tabs, tab)) else {
        panic!("no action or no document");
    };
    assert_eq!(navigation.cause, Cause::Agent { action, document });
}

/// A small page at `at` with one form, sent by `method` to `action`, with
/// one text field called *Search* and a button called *Go*.
fn a_form(at: &str, method: &str, action: &str) -> Page {
    Page::new(
        format!(
            "<!doctype html><form method={method} action='{action}'>\
             <label>Search <input name=a></label><button>Go</button></form>"
        ),
        Size::new(400.0, 300.0),
    )
    .at(url(at))
}

#[test]
fn a_query_on_the_action_is_replaced_as_chrome_replaces_it() {
    let mut tabs = tabs();
    let tab = showing(
        &mut tabs,
        a_form(
            "https://example.test/",
            "get",
            "https://example.test/get?q=9",
        ),
    );
    let put = act(
        &mut tabs,
        tab,
        "Search",
        Verb::PutText("hello world&=é".to_owned()),
    );
    assert!(put.is_some());
    let (_, submitted, decided) = pressed(&mut tabs, tab, "Go");
    assert!(submitted, "the renderer did not answer a submission");
    let Some(Decided::Go(navigation)) = decided else {
        panic!("the form was not sent: {decided:?}");
    };
    // Chrome 154 requested `https://example.test/get?a=…`, the action's own
    // `q=9` gone, and wrote these bytes for that text (ADR 0051).
    assert_eq!(
        navigation.url.serialised,
        "https://example.test/get?a=hello+world%26%3D%C3%A9"
    );
}

#[test]
fn an_http_action_from_an_https_page_is_refused_said_and_recorded() {
    let mut tabs = tabs();
    let tab = showing(
        &mut tabs,
        a_form(
            "https://shop.example/account",
            "post",
            "http://shop.example/login",
        ),
    );
    let put = act(
        &mut tabs,
        tab,
        "Search",
        Verb::PutText("hunter2".to_owned()),
    );
    assert!(put.is_some());
    let (action, submitted, decided) = pressed(&mut tabs, tab, "Go");
    assert!(submitted, "the renderer did not answer a submission");
    let Some(Decided::Refused(refusal)) = decided else {
        panic!("a secure page's form was sent in the clear: {decided:?}");
    };
    let (Some(action), Some(document)) = (action, document_of(&tabs, tab)) else {
        panic!("no action or no document");
    };
    assert_eq!(refusal.rule, Rule::Insecure);
    assert_eq!(refusal.cause, Cause::Agent { action, document });
    let said = refusal.to_string();
    assert!(
        said.contains("refused") && said.contains("anybody on the network"),
        "{said}"
    );
    assert!(
        !said.contains("hunter2"),
        "the refusal says what was typed: {said}"
    );

    let mut activity = Activity::new();
    assert!(refusal.record(&mut activity, SystemTime::UNIX_EPOCH));
    let Some(line) = activity.latest() else {
        panic!("nothing was recorded");
    };
    assert_eq!(line.method(), "POST");
    assert_eq!(line.url().serialised, "http://shop.example/login");
    assert_eq!(line.cause(), &Cause::Agent { action, document });
    assert!(
        matches!(line.happened(), Happened::Refused { rule } if rule.contains("network")),
        "{line:?}"
    );
}

#[test]
fn the_documents_header_policy_decides_where_its_forms_may_go() {
    let mut tabs = tabs();
    let tab = showing(
        &mut tabs,
        a_form(
            "https://shop.example/",
            "get",
            "https://elsewhere.example/search",
        )
        .with_policy("form-action 'self'"),
    );
    let (_, submitted, decided) = pressed(&mut tabs, tab, "Go");
    assert!(submitted, "the renderer did not answer a submission");
    let Some(Decided::Refused(refusal)) = decided else {
        panic!("the form went where its policy forbids: {decided:?}");
    };
    assert!(
        matches!(&refusal.rule, Rule::FormAction { said } if said.contains("form-action allows 'self'")),
        "{refusal}"
    );
    assert_eq!(refusal.method, Method::Get);

    // The same form under a policy that names where it goes is sent.
    let tab = showing(
        &mut tabs,
        a_form(
            "https://shop.example/",
            "get",
            "https://elsewhere.example/search",
        )
        .with_policy("default-src 'none'; form-action https://elsewhere.example"),
    );
    let (_, _, decided) = pressed(&mut tabs, tab, "Go");
    let Some(Decided::Go(navigation)) = decided else {
        panic!("the form was not sent: {decided:?}");
    };
    assert_eq!(
        navigation.url.serialised,
        "https://elsewhere.example/search?a="
    );
}
