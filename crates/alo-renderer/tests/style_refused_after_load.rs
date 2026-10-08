/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 346 (ADR 0034 § 4): inline style a policy objects to, found by
//! a draw after the page loaded, reported.
//!
//! *A script that sets a refused `style` attribute after load produces one
//! report from the browser process, and the same attribute set twice
//! produces one. A flood is bounded and said.* And from the item: what a
//! draw found for a `Paint` waits for the next answer that can carry it, and
//! a delivered response's reaction is carried in its `Delivered`.
//!
//! Each test loads a real page into a renderer, changes it the way a page's
//! own script would — a listener the agent's click sets off, a reaction to a
//! fetch — and hands what crosses to [`violations::reports`], which is what
//! the browser process posts from.

use alo_agent::{Target, Verb};
use alo_js::interpret::Trouble;
use alo_js::script;
use alo_layout::Size;
use alo_net::cause::{Cause, Identities};
use alo_net::csp::{Inline, Placement};
use alo_net::csp_report;
use alo_renderer::fetch::Fetched;
use alo_renderer::violations::{self, MOST_OBJECTIONS, Objection};
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;

/// A small window; nothing here is about where anything is.
const WINDOW: Size = Size {
    width: 200.0,
    height: 100.0,
};

/// The policy every page here is served under.
const POLICY: &str = "style-src 'self'; report-uri /csp";

/// A page whose button, when clicked, runs `listener`.
fn page(body: &str, listener: &str) -> Page {
    Page::new(
        format!(
            "{body}<button id=go>Go</button><script>\
             document.getElementById('go').addEventListener('click', function () {{ \
             {listener} }})</script>"
        ),
        WINDOW,
    )
    .with_policy(POLICY)
}

/// What an answer carried: its issues and its objections, or why it was not
/// the answer expected.
#[derive(Debug)]
struct Carried {
    issues: Vec<String>,
    objections: Vec<Objection>,
}

/// Load `page`, answering what the load carried.
fn load(renderer: &mut Renderer, page: &Page) -> Option<Carried> {
    match renderer.handle(ToRenderer::Load(Box::new(page.clone()))) {
        FromRenderer::Loaded {
            issues, objections, ..
        } => Some(Carried { issues, objections }),
        _ => None,
    }
}

/// Click the button named `Go`, answering what the act carried.
fn click(renderer: &mut Renderer) -> Option<Carried> {
    match renderer.handle(ToRenderer::Act {
        target: Target::Named("Go".to_owned()),
        verb: Verb::Activate,
    }) {
        FromRenderer::Acted {
            issues, objections, ..
        } => Some(Carried { issues, objections }),
        _ => None,
    }
}

/// Run `source` on the page as a script of its own would be, after its
/// load. Empty when it ran to its end.
fn run(renderer: &mut Renderer, source: &str) -> String {
    let Some(looping) = renderer.event_loop() else {
        return "no script ran".to_owned();
    };
    let program = match script(source) {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    match looping.engine().evaluate(&program) {
        Ok(_) => String::new(),
        Err(Trouble::Escaped(escape)) => format!("! {escape}"),
        Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
    }
}

/// The reports the browser process would post for `objections`, from its
/// own copy of `page`'s headers.
fn reports(page: &Page, objections: &[Objection]) -> violations::Reports {
    let url = alo_url::parse("https://shop.example/").ok();
    let Some(url) = url else {
        return violations::Reports::default();
    };
    let about = csp_report::Page::at(
        url,
        Cause::Document {
            document: Identities::default().a_document(),
        },
    );
    violations::reports(page, &about, objections)
}

fn attribute() -> Objection {
    Objection {
        policy: 0,
        kind: Inline::Style,
        placement: Placement::Attribute,
    }
}

#[test]
fn a_style_attribute_a_listener_sets_is_reported_once_per_text() {
    // Each click writes the next of red, blue, red: two texts, the first
    // written twice.
    let page = page(
        "<p id=a>x</p><script>this.clicks = 0</script>",
        "clicks = clicks + 1; let text = 'color: red'; \
         if (clicks === 2) { text = 'color: blue' } \
         document.getElementById('a').setAttribute('style', text)",
    );
    let mut renderer = Renderer::new(FontDatabase::new());
    let loaded = load(&mut renderer, &page).expect("loaded");
    assert!(loaded.objections.is_empty(), "{loaded:?}");

    let first = click(&mut renderer).expect("acted");
    assert_eq!(first.objections, vec![attribute()], "{first:?}");
    let posted = reports(&page, &first.objections);
    assert!(posted.disbelieved.is_empty(), "{:?}", posted.disbelieved);
    assert_eq!(posted.posts.len(), 1);
    let body = String::from_utf8_lossy(&posted.posts[0].body);
    assert!(
        body.contains("\"violated-directive\":\"style-src\""),
        "{body}"
    );
    assert_eq!(
        posted.posts[0].url.to_string(),
        "https://shop.example/csp",
        "posted where the headers said"
    );
    assert!(
        first.issues.iter().all(|issue| !issue.contains("uncaught")),
        "{first:?}"
    );

    let second = click(&mut renderer).expect("acted");
    assert_eq!(
        second.objections,
        vec![attribute()],
        "another text: {second:?}"
    );

    let draws = renderer.draws();
    let third = click(&mut renderer).expect("acted");
    assert!(renderer.draws() > draws, "the change was drawn");
    assert!(
        third.objections.is_empty(),
        "the same element and text again: {third:?}"
    );
}

#[test]
fn what_a_paint_found_waits_for_the_next_answer_that_carries_objections() {
    let page = page("<p id=a>x</p>", "");
    let mut renderer = Renderer::new(FontDatabase::new());
    load(&mut renderer, &page).expect("loaded");
    assert_eq!(
        run(
            &mut renderer,
            "document.getElementById('a').setAttribute('style', 'color: red')"
        ),
        ""
    );
    let draws = renderer.draws();
    assert!(matches!(
        renderer.handle(ToRenderer::Paint),
        FromRenderer::Painted(_)
    ));
    assert!(matches!(
        renderer.handle(ToRenderer::ReadTree),
        FromRenderer::Tree(_)
    ));
    assert_eq!(renderer.draws(), draws + 1, "the paint drew it");

    let acted = click(&mut renderer).expect("acted");
    assert_eq!(acted.objections, vec![attribute()], "{acted:?}");
    let again = click(&mut renderer).expect("acted");
    assert!(again.objections.is_empty(), "carried once: {again:?}");
}

#[test]
fn a_reaction_to_a_fetch_is_carried_in_its_delivered() {
    let page = Page::new(
        "<p id=a>x</p><script>\
         const set = function () { \
           document.getElementById('a').setAttribute('style', 'color: red') }; \
         fetch('/x').then(set, set)</script>",
        WINDOW,
    )
    .with_policy(POLICY);
    // A page with an address, so that its fetch is asked for rather than
    // refused as it loads.
    let Some(here) = alo_url::parse("https://shop.example/").ok() else {
        panic!("an address");
    };
    let page = page.at(here);
    let mut renderer = Renderer::new(FontDatabase::new());
    let FromRenderer::Loaded {
        objections,
        fetches,
        ..
    } = renderer.handle(ToRenderer::Load(Box::new(page.clone())))
    else {
        panic!("not loaded");
    };
    assert!(objections.is_empty(), "{objections:?} {fetches:?}");
    let number = fetches.first().map(|ask| ask.number).expect("one fetch");
    let FromRenderer::Delivered {
        issues, objections, ..
    } = renderer.handle(ToRenderer::Fetched(Box::new(Fetched::failed(number))))
    else {
        panic!("not delivered");
    };
    assert_eq!(objections, vec![attribute()], "{issues:?}");
    assert_eq!(reports(&page, &objections).posts.len(), 1);
}

#[test]
fn a_flood_after_load_is_bounded_and_said() {
    let body = "<p>x</p>".repeat(100);
    let page = page(
        &body,
        "const ps = document.querySelectorAll('p'); \
         for (let i = 0; i < ps.length; i = i + 1) { \
           ps.item(i).setAttribute('style', 'color: red') }",
    );
    let mut renderer = Renderer::new(FontDatabase::new());
    load(&mut renderer, &page).expect("loaded");
    let acted = click(&mut renderer).expect("acted");
    assert_eq!(
        acted.objections.len(),
        MOST_OBJECTIONS,
        "{:?}",
        acted.issues
    );
    assert!(
        acted.issues.iter().any(|issue| issue
            .starts_with("36 more policy objections to this page's inline style were not passed")),
        "{:?}",
        acted.issues
    );
    assert_eq!(
        reports(&page, &acted.objections).posts.len(),
        MOST_OBJECTIONS
    );
    // What was counted and not passed on is still remembered, so the next
    // draw does not find the flood again.
    assert_eq!(
        run(
            &mut renderer,
            "document.getElementById('go').setAttribute('data-again', '')"
        ),
        ""
    );
    let again = click(&mut renderer).expect("acted");
    assert!(again.objections.is_empty(), "{again:?}");
    assert!(
        again
            .issues
            .iter()
            .all(|issue| !issue.contains("more policy objections")),
        "{:?}",
        again.issues
    );
}

#[test]
fn a_new_load_forgets_what_the_last_page_objected_to() {
    let page = Page::new(r#"<p style="color: red">x</p>"#, WINDOW).with_policy(POLICY);
    let mut renderer = Renderer::new(FontDatabase::new());
    let first = load(&mut renderer, &page).expect("loaded");
    assert_eq!(first.objections, vec![attribute()]);
    let second = load(&mut renderer, &page).expect("loaded");
    assert_eq!(
        second.objections,
        vec![attribute()],
        "a new document is a new page"
    );
}
