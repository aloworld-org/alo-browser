/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 364 (ADR 0039 § 1): a page is told whether it can be seen.
//!
//! - A page reads its visibility state while it loads: `"visible"` and
//!   `false` for a page loaded visible, `"hidden"` and `true` for one
//!   loaded hidden.
//! - `ToRenderer::Visibility` is a task that sets the state and, only if it
//!   changed, fires one `visibilitychange` — trusted, bubbling, not
//!   cancellable — at the document and then, bubbling, at the window.
//! - It is answered as a delivery is, carrying what the listeners asked
//!   for; across the boundary the browser process decides a fetch they ask
//!   for as the document's.
//! - A listener that throws is said, and the next still hears it; a page
//!   asked to stop is stopped and says so; a page that never ran script is
//!   told and answers nothing; a renderer holding nothing fails.
//!
//! Every task after the load runs ordinarily and with the collector at
//! every allocation.

use alo_layout::Size;
use alo_net::cause::Cause;
use alo_renderer::fetch_decide::Decided;
use alo_renderer::host::Renderers;
use alo_renderer::message::Failure;
use alo_renderer::tab::{Tab, Tabs};
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer, Visibility};
use alo_text::FontDatabase;
use alo_url::Url;

/// Where every page here is.
const HERE: &str = "https://shop.example/a/";

/// What a page reads of its visibility as it loads, into the root's
/// `data-read`, and every `visibilitychange` its document and its window
/// hear, into the root's `data-heard`: where, the event's type, its phase,
/// whether it is trusted, whether it bubbles, whether it can be cancelled,
/// and the state the listener reads. `extra` is more script, after that.
fn page(extra: &str) -> String {
    format!(
        "<!DOCTYPE html><html><body><script>\
         var root = document.documentElement; var heard = ''; \
         root.setAttribute('data-read', document.visibilityState + ' ' + document.hidden); \
         function note(where) {{ return function (e) {{ \
           heard = (heard ? heard + ' ' : '') + where + ':' + e.type + ':' + e.eventPhase + \
             ':' + e.isTrusted + ':' + e.bubbles + ':' + e.cancelable + ':' + \
             document.visibilityState + ':' + document.hidden; \
           root.setAttribute('data-heard', heard); }}; }} \
         document.addEventListener('visibilitychange', note('document')); \
         window.addEventListener('visibilitychange', note('window')); \
         {extra}</script></body></html>"
    )
}

fn url(text: &str) -> Url {
    alo_url::parse(text).unwrap_or_else(|_| Url::about_blank())
}

/// A renderer with `page` loaded, and what its load said.
fn loaded(page: Page) -> (Renderer, Vec<String>) {
    let mut renderer = Renderer::new(FontDatabase::new());
    let issues = match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded { issues, .. } => issues,
        other => vec![format!("not loaded: {other:?}")],
    };
    (renderer, issues)
}

/// [`page`] with `extra`, loaded at [`HERE`] as `visibility` says, its heap
/// collecting at every allocation from then on if `stress`.
fn shown(extra: &str, visibility: Visibility, stress: bool) -> Result<Renderer, String> {
    let (mut renderer, issues) = loaded(
        Page::new(page(extra), Size::new(400.0, 200.0))
            .at(url(HERE))
            .shown_as(visibility),
    );
    if !issues.is_empty() {
        return Err(format!("the load said {issues:?}"));
    }
    let looping = renderer
        .event_loop()
        .ok_or("none of the page's script ran")?;
    looping.engine().objects().heap_mut().stress(stress);
    Ok(renderer)
}

/// The root element's attribute `name`, if the page's script wrote one.
fn root(renderer: &Renderer, name: &str) -> Option<String> {
    let document = renderer.document()?;
    let html = document
        .descendants(document.root())
        .find(|id| document.element(*id).is_some())?;
    document.element(html)?.attr(name).map(ToOwned::to_owned)
}

/// Tell the page `to`, and answer what the task said and asked to fetch.
fn tell(renderer: &mut Renderer, to: Visibility) -> (Vec<String>, Vec<String>) {
    match renderer.handle(ToRenderer::Visibility(to)) {
        FromRenderer::Delivered {
            issues,
            fetches,
            navigation,
            sheets,
            ..
        } => {
            assert_eq!(navigation, None);
            assert!(sheets.is_empty(), "{sheets:?}");
            (issues, fetches.into_iter().map(|ask| ask.url).collect())
        }
        other => (vec![format!("not delivered: {other:?}")], Vec::new()),
    }
}

/// What the document and the window heard of one change to `state`, in the
/// order they heard it: at the document, at its target, then at the window
/// as it bubbled.
fn one_change(state: &str, hidden: bool) -> String {
    format!(
        "document:visibilitychange:2:true:true:false:{state}:{hidden} \
         window:visibilitychange:3:true:true:false:{state}:{hidden}"
    )
}

#[test]
fn a_page_reads_whether_it_is_seen_as_it_loads() {
    let (visible, issues) = loaded(Page::new(page(""), Size::new(400.0, 200.0)));
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(
        root(&visible, "data-read").as_deref(),
        Some("visible false")
    );
    let (hidden, issues) =
        loaded(Page::new(page(""), Size::new(400.0, 200.0)).shown_as(Visibility::Hidden));
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(root(&hidden, "data-read").as_deref(), Some("hidden true"));
    // Nothing was fired at either: a page starts in its state, it does not
    // change into it.
    assert_eq!(root(&visible, "data-heard"), None);
    assert_eq!(root(&hidden, "data-heard"), None);
}

#[test]
fn hiding_a_page_fires_one_visibilitychange_at_the_document_then_the_window() {
    for stress in [false, true] {
        let mut renderer =
            shown("", Visibility::Visible, stress).unwrap_or_else(|why| panic!("{why}"));

        let (issues, fetches) = tell(&mut renderer, Visibility::Hidden);
        assert!(issues.is_empty(), "{issues:?}");
        assert!(fetches.is_empty(), "{fetches:?}");
        let hidden = one_change("hidden", true);
        assert_eq!(root(&renderer, "data-heard"), Some(hidden.clone()));

        // Hidden again is no change, and nothing is fired.
        let (issues, _) = tell(&mut renderer, Visibility::Hidden);
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(
            root(&renderer, "data-heard"),
            Some(hidden.clone()),
            "stress: {stress}"
        );

        let (issues, _) = tell(&mut renderer, Visibility::Visible);
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(
            root(&renderer, "data-heard"),
            Some(format!("{hidden} {}", one_change("visible", false))),
            "stress: {stress}"
        );
    }
}

#[test]
fn a_page_loaded_hidden_hears_nothing_until_it_is_shown() {
    for stress in [false, true] {
        let mut renderer =
            shown("", Visibility::Hidden, stress).unwrap_or_else(|why| panic!("{why}"));
        let (issues, _) = tell(&mut renderer, Visibility::Hidden);
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(root(&renderer, "data-heard"), None);
        let (issues, _) = tell(&mut renderer, Visibility::Visible);
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(
            root(&renderer, "data-heard"),
            Some(one_change("visible", false)),
            "stress: {stress}"
        );
    }
}

#[test]
fn what_a_listener_asks_to_fetch_is_in_the_answer() {
    for stress in [false, true] {
        let mut renderer = shown(
            "document.addEventListener('visibilitychange', function () { \
               fetch('beacon?' + document.visibilityState); });",
            Visibility::Visible,
            stress,
        )
        .unwrap_or_else(|why| panic!("{why}"));
        let (issues, fetches) = tell(&mut renderer, Visibility::Hidden);
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(fetches, [format!("{HERE}beacon?hidden")]);
        let (_, fetches) = tell(&mut renderer, Visibility::Hidden);
        assert!(fetches.is_empty(), "no change, no listener, no fetch");
    }
}

#[test]
fn a_listener_that_throws_is_said_and_the_window_still_hears() {
    for stress in [false, true] {
        let mut renderer = shown(
            "document.addEventListener('visibilitychange', function () { \
               throw new TypeError('not now'); });",
            Visibility::Visible,
            stress,
        )
        .unwrap_or_else(|why| panic!("{why}"));
        let (issues, _) = tell(&mut renderer, Visibility::Hidden);
        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(
            issues[0].starts_with("the page's visibility: ") && issues[0].contains("not now"),
            "{issues:?}"
        );
        assert_eq!(
            root(&renderer, "data-heard"),
            Some(one_change("hidden", true))
        );
    }
}

#[test]
fn a_page_asked_to_stop_is_stopped_and_says_so() {
    let mut renderer = shown("", Visibility::Visible, false).unwrap_or_else(|why| panic!("{why}"));
    let Some(looping) = renderer.event_loop() else {
        panic!("the page's script ran");
    };
    looping.stop_switch().ask();
    let (issues, _) = tell(&mut renderer, Visibility::Hidden);
    assert!(
        issues.iter().any(|line| line.contains("the page stopped")),
        "{issues:?}"
    );
    assert_eq!(root(&renderer, "data-heard"), None, "nobody heard it");
    // A stopped page queues nothing again, and says why.
    let (issues, _) = tell(&mut renderer, Visibility::Visible);
    assert!(
        issues.iter().any(|line| line.contains("the page stopped")),
        "{issues:?}"
    );
    assert_eq!(root(&renderer, "data-heard"), None);
}

#[test]
fn a_page_that_never_ran_script_is_told_and_answers_nothing() {
    let (mut renderer, _) = loaded(Page::new("<p>hi</p>", Size::new(100.0, 100.0)));
    for to in [Visibility::Hidden, Visibility::Visible] {
        assert_eq!(
            renderer.handle(ToRenderer::Visibility(to)),
            FromRenderer::Delivered {
                issues: Vec::new(),
                objections: Vec::new(),
                navigation: None,
                fetches: Vec::new(),
                sheets: Vec::new(),
            }
        );
    }
    assert!(renderer.event_loop().is_none(), "it was given no heap");

    let mut nothing = Renderer::new(FontDatabase::new());
    assert_eq!(
        nothing.handle(ToRenderer::Visibility(Visibility::Hidden)),
        FromRenderer::Failed(Failure::NothingLoaded)
    );
}

// --- What the browser process decides -----------------------------------------

#[test]
fn across_the_boundary_a_listeners_fetch_is_the_documents() {
    let mut tabs = Tabs::over(Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[]));
    let tab = tabs.open(url(HERE));
    let loaded = tabs.load(
        tab,
        Page::new(
            page(
                "document.addEventListener('visibilitychange', function () { \
                   fetch('beacon?' + document.visibilityState); });",
            ),
            Size::new(400.0, 200.0),
        )
        .at(url(HERE)),
        Cause::Person { tab },
    );
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    let document = tabs.tab(tab).and_then(Tab::document).expect("a document");
    assert!(tabs.fetches(tab).is_empty(), "nothing asked at load");

    let told = tabs.visibility(tab, Visibility::Hidden);
    assert!(
        matches!(told, Ok(FromRenderer::Delivered { .. })),
        "{told:?}"
    );
    let decided = tabs.fetches(tab);
    let [Decided::Make(fetch)] = decided.as_slice() else {
        panic!("one fetch to make: {decided:?}");
    };
    assert_eq!(fetch.request.url, url(&format!("{HERE}beacon?hidden")));
    assert_eq!(fetch.request.cause, Cause::Document { document });

    // Hidden again is no change across the boundary either.
    let told = tabs.visibility(tab, Visibility::Hidden);
    assert!(
        matches!(told, Ok(FromRenderer::Delivered { .. })),
        "{told:?}"
    );
    assert!(tabs.fetches(tab).is_empty(), "nothing was fired");
}
