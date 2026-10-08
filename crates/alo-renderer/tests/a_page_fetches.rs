/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 335: `fetch` in a page, across the boundary (ADR 0032 § 1).
//!
//! A script's `fetch` is an ask in the answer to the message whose work made
//! it — a `Load`'s, an `Act`'s, or a delivery's — and the answer comes back
//! as a message of its own whose handling is a task: the promise settles,
//! its reactions run, the page is drawn again if they changed it, and what
//! they asked for in turn is that message's answer.
//!
//! Two halves, as for navigation: the renderer in this process, and real
//! [`Tabs`] over the real confined `alo-render` binary, where the asks are
//! decided by the browser process with the cause it assigns from which
//! message was answered.

use alo_agent::{Target, Verb};
use alo_layout::Size;
use alo_net::cause::Cause;
use alo_renderer::fetch::{Answer, FetchAsk, Fetched, Kind, Readable};
use alo_renderer::fetch_decide::Decided;
use alo_renderer::host::Renderers;
use alo_renderer::tab::{Tab, Tabs};
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_url::Url;

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
    database.map_generic("sans-serif", "DejaVu Sans");
    database
}

/// Where every page here is.
const HERE: &str = "https://shop.example/a/";

fn url(text: &str) -> Url {
    alo_url::parse(text).unwrap_or_else(|_| Url::about_blank())
}

/// A page at [`HERE`] of `<p id=out>-</p>`, `markup`, and `script`, with
/// `say(what)` appending to the paragraph.
fn page(markup: &str, script: &str) -> Page {
    Page::new(
        format!(
            "<!doctype html><body><p id=out>-</p>{markup}<script>\
             var out = document.getElementById('out'); \
             function say(what) {{ out.textContent = out.textContent + ' ' + what; }} \
             {script}</script></body>"
        ),
        Size::new(400.0, 200.0),
    )
    .at(url(HERE))
}

/// A response from [`HERE`]'s own origin, with `body`.
fn basic(body: &str) -> Answer {
    Answer::Response(Box::new(Readable {
        kind: Kind::Basic,
        status: 200,
        status_text: "OK".to_owned(),
        url: Some(format!("{HERE}data")),
        redirected: false,
        headers: vec![("Content-Type".to_owned(), "text/plain".to_owned())],
        body: body.as_bytes().to_vec(),
    }))
}

/// The text of the page's `<p id=out>`.
fn heard(renderer: &Renderer) -> String {
    let Some(document) = renderer.document() else {
        return "no document".to_owned();
    };
    document
        .descendants(document.root())
        .find(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.attr("id") == Some("out"))
        })
        .map(|node| document.text_content(node))
        .unwrap_or_default()
}

/// The text the page's drawing holds for `<p id=out>` — what was drawn,
/// not what the document says.
fn drawn(renderer: &Renderer) -> Vec<String> {
    let Some(drawing) = renderer.rendered() else {
        return Vec::new();
    };
    drawing
        .boxes
        .ids()
        .filter_map(|id| drawing.boxes.get(id).and_then(alo_box::BoxNode::text))
        .map(|text| text.trim().to_owned())
        .filter(|text| text.starts_with('-'))
        .collect()
}

/// What a delivery said, and what its reactions asked for.
type Said = (Vec<String>, Vec<FetchAsk>);

/// Deliver `answer` to ask `number`: what the delivery said, and what its
/// reactions asked for — or what the renderer answered instead.
fn deliver(renderer: &mut Renderer, number: u64, answer: Answer) -> Result<Said, String> {
    match renderer.handle(ToRenderer::Fetched(Box::new(Fetched { number, answer }))) {
        FromRenderer::Delivered {
            issues, fetches, ..
        } => Ok((issues, fetches)),
        other => Err(format!("a delivery answered {other:?}")),
    }
}

/// Load `page`, collecting at every allocation from then on if `stress`:
/// the renderer, and what the load asked to fetch — or what the renderer
/// answered instead.
fn load(page: Page, stress: bool) -> Result<(Renderer, Vec<FetchAsk>), String> {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(page)));
    if let Some(page_loop) = renderer.event_loop() {
        page_loop.engine().objects().heap_mut().stress(stress);
    }
    match answer {
        FromRenderer::Loaded { fetches, .. } => Ok((renderer, fetches)),
        other => Err(format!("not loaded: {other:?}")),
    }
}

#[test]
fn a_loads_ask_is_answered_by_a_task_and_the_page_is_drawn_again() {
    for stress in [false, true] {
        let (mut renderer, asks) = load(
            page(
                "",
                "fetch('data').then(function (r) { return r.text(); })
                   .then(function (t) { say(t); return fetch('more', { method: 'POST',
                                                                      body: t }); })
                   .then(function (r) { say(r.status); });",
            ),
            stress,
        )
        .expect("loaded");
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0].url, format!("{HERE}data"));
        assert_eq!(heard(&renderer), "-", "nothing is answered at load");
        let (issues, again) =
            deliver(&mut renderer, asks[0].number, basic("thawed")).expect("delivered");
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(heard(&renderer), "- thawed");
        assert_eq!(drawn(&renderer), ["- thawed"], "the page was drawn again");
        // The reaction's own fetch is the delivery's ask.
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].url, format!("{HERE}more"));
        assert_eq!(again[0].method, "POST");
        assert_eq!(again[0].body, b"thawed");
        assert_ne!(again[0].number, asks[0].number);
        let (issues, none) = deliver(&mut renderer, again[0].number, basic("")).expect("delivered");
        assert!(issues.is_empty() && none.is_empty(), "{issues:?} {none:?}");
        assert_eq!(heard(&renderer), "- thawed 200");
    }
}

#[test]
fn an_acts_ask_is_in_the_acts_answer() {
    let (mut renderer, asks) = load(
        page(
            "<button id=go>Go</button>",
            "document.getElementById('go').addEventListener('click', function () {
               fetch('clicked').catch(function (e) { say(e.name); });
             });",
        ),
        false,
    )
    .expect("loaded");
    assert!(asks.is_empty());
    let FromRenderer::Acted { fetches, .. } = renderer.handle(ToRenderer::Act {
        target: Target::Named("Go".to_owned()),
        verb: Verb::Activate,
    }) else {
        panic!("not acted");
    };
    assert_eq!(fetches.len(), 1);
    assert_eq!(fetches[0].url, format!("{HERE}clicked"));
    deliver(&mut renderer, fetches[0].number, Answer::NetworkError).expect("delivered");
    assert_eq!(heard(&renderer), "- TypeError");
}

#[test]
fn a_new_page_lets_go_of_what_the_last_one_waited_for() {
    let (mut renderer, asks) = load(
        page("", "fetch('data').then(function () { say('late'); });"),
        false,
    )
    .expect("loaded");
    assert_eq!(asks.len(), 1);
    let next = renderer.handle(ToRenderer::Load(Box::new(page("", "say('second');"))));
    assert!(matches!(next, FromRenderer::Loaded { .. }), "{next:?}");
    let (issues, _) = deliver(&mut renderer, asks[0].number, basic("x")).expect("delivered");
    assert_eq!(issues.len(), 1);
    assert!(
        issues[0].contains("nothing on this page is waiting for fetch 0"),
        "{issues:?}"
    );
    assert_eq!(heard(&renderer), "- second", "the last page's reaction ran");
}

// --- What the browser process decides -----------------------------------------

/// The renderer binary, as cargo built it for this test.
fn tabs() -> Tabs {
    Tabs::over(Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[]))
}

#[test]
fn across_the_boundary_a_fetch_is_decided_as_the_documents_and_answered() {
    let mut tabs = tabs();
    let tab = tabs.open(url(HERE));
    let loaded = tabs.load(
        tab,
        page(
            "",
            "fetch('data').then(function (r) { return r.text(); })
               .then(function (t) { say(t); return fetch('https://elsewhere.example/x',
                                                         { mode: 'same-origin' }); })
               .catch(function (e) { say(e.name); });",
        ),
        Cause::Person { tab },
    );
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    let document = tabs.tab(tab).and_then(Tab::document).expect("a document");
    let decided = tabs.fetches(tab);
    let [Decided::Make(fetch)] = decided.as_slice() else {
        panic!("one fetch to make: {decided:?}");
    };
    assert_eq!(fetch.request.url, url(&format!("{HERE}data")));
    assert_eq!(fetch.request.cause, Cause::Document { document });
    let delivered = tabs.fetched(
        tab,
        Fetched {
            number: fetch.number,
            answer: basic("from the other side"),
        },
    );
    let Ok(Some(FromRenderer::Delivered { issues, .. })) = delivered else {
        panic!("not delivered: {delivered:?}");
    };
    // The reaction's same-origin ask to another origin is refused here,
    // before anything is sent: said to the person among the delivery's
    // issues, and answered to the page as a network error.
    assert_eq!(
        issues,
        [
            "the page's fetch of \"https://elsewhere.example/x\" was refused: it asked for the \
             same origin only, and https://elsewhere.example is not https://shop.example"
        ]
    );
    let decided = tabs.fetches(tab);
    let [Decided::Refused(refusal)] = decided.as_slice() else {
        panic!("one refusal: {decided:?}");
    };
    let delivered = tabs.fetched(tab, refusal.answer());
    assert!(
        matches!(delivered, Ok(Some(FromRenderer::Delivered { .. }))),
        "{delivered:?}"
    );
    let painted = tabs.ask(tab, &ToRenderer::ReadTree);
    let Ok(FromRenderer::Tree(snapshot)) = painted else {
        panic!("no tree: {painted:?}");
    };
    let said = format!("{snapshot:?}");
    assert!(
        said.contains("- from the other side TypeError"),
        "the page across the boundary did not hear both answers: {said}"
    );
}
