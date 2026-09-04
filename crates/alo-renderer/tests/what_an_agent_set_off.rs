/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! What followed from something an agent did, walked back to the agent.
//!
//! Queue item 199 and ADR 0012 § 3. Item 67 put a [`Cause`] on every request
//! and there it stopped: a request said *this document wanted it* and nothing
//! anywhere said what had caused **that document**. So the second of the two
//! questions `ROADMAP.md` asks — *which agent action* — was unanswerable for
//! every request except the handful an agent made directly, which is the
//! narrowest possible reading of the promise.
//!
//! # Why this file drives real tabs and a real renderer
//!
//! `chain.rs`'s own tests assert the walk over a record built by hand, which is
//! the right place for a cycle and for the bound. What they cannot assert is
//! that the browser process **records anything in the first place** — that
//! loading a page makes a document, that acting on a page mints an action, and
//! that the two are joined by the load the verb led to. A test that recorded
//! its own documents would be asserting the walk against a record it had
//! written, and the failure worth catching is a real load that records nothing.
//!
//! So this spawns the real `alo-render` binary, confined, and asks it to
//! activate a real link on a real page.
//!
//! # The attack this is written against
//!
//! Attribution is a claim, so the question is who can make it. A page cannot:
//! the second load here happens because the *renderer said a link was
//! followed*, and what goes into the record is the cause the **browser
//! process** composed for the tab it was acting in. A renderer that answered
//! with somebody else's document, or with a cause of its own, has nowhere to
//! put it — [`ToRenderer`] and [`FromRenderer`] carry no cause in either
//! direction, and [`Tabs`] takes the document from the tab rather than from the
//! answer.

use alo_agent::{Outcome, Target, Verb};
use alo_layout::geometry::Size;
use alo_net::cause::Cause;
use alo_net::chain::End;
use alo_net::{Purpose, Request};
use alo_renderer::host::Renderers;
use alo_renderer::message::FromRenderer;
use alo_renderer::page::Page;
use alo_renderer::tab::{Lost, Tab, TabId, Tabs};

/// The renderer binary, as cargo built it for this test.
fn tabs() -> Tabs {
    Tabs::over(Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[]))
}

/// A URL, or something plainly not one of these pages if the text was wrong —
/// so a mistake shows up as the assertion it broke rather than as a panic in a
/// helper.
fn url(text: &str) -> alo_url::Url {
    alo_url::parse(text).unwrap_or_else(|_| alo_url::Url {
        scheme: "about".to_owned(),
        host: None,
        port: None,
        path: "not-a-url".to_owned(),
        query: None,
        fragment: None,
        serialised: "about:not-a-url".to_owned(),
    })
}

/// A page with one link on it, which is all this needs to be a page an agent
/// can act on.
fn a_page_with_a_link(text: &str, to: &str, name: &str) -> Page {
    Page::new(
        format!(r#"<p>{text}</p><a href="{to}">{name}</a>"#),
        Size {
            width: 400.0,
            height: 200.0,
        },
    )
    .with_sheet("p, a { display: block; margin: 8px; height: 20px }")
}

/// Load a page into a tab, and say which assertion it was that failed if the
/// renderer did not load it.
fn load(tabs: &mut Tabs, id: TabId, page: Page, cause: Cause) {
    let loaded = tabs.load(id, page, cause);
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{id} did not load: {loaded:?}",
    );
}

// --- The closing condition ---------------------------------------------------

/// An agent activates a link, the page that link goes to loads, and that page
/// fetches something of its own. The fetch is the agent's doing, and only the
/// walk says so — which is the whole of ADR 0012 § 3.
#[test]
fn a_fetch_by_a_page_an_agent_opened_leads_back_to_the_action() {
    let mut tabs = tabs();
    let shopping = tabs.open(url("https://shop.example/things"));
    load(
        &mut tabs,
        shopping,
        a_page_with_a_link("two hats and a lamp", "/basket", "Basket"),
        Cause::Person { tab: shopping },
    );
    let first = tabs.tab(shopping).and_then(Tab::document);
    assert!(first.is_some(), "a page that loaded is not a document");

    // The agent acts. The action is minted here, by the browser process, and
    // the renderer is not asked what caused anything.
    let acted = tabs.act(shopping, Target::Named("Basket".to_owned()), Verb::Activate);
    let Ok((action, FromRenderer::Acted(Outcome::Followed { to, .. }))) = acted else {
        panic!("the agent did not follow the link: {acted:?}");
    };
    assert_eq!(to, "/basket");

    // What the verb led to: the browser process fetches the page the link named
    // and loads it, attributed to the action rather than to the page.
    let Some(opening) = tabs.an_agent_acting(shopping, action) else {
        panic!("the tab is showing no page for the agent to have acted in");
    };
    let fetching_the_basket = Request::get(url("https://shop.example/basket"), opening.clone());
    load(
        &mut tabs,
        shopping,
        a_page_with_a_link("one hat", "/pay", "Pay"),
        opening,
    );
    let second = tabs.tab(shopping).and_then(Tab::document);
    assert_ne!(second, first, "the same address twice is two documents");

    // And what that page then asks for itself.
    let Some(by_the_page) = tabs.a_page_fetching(shopping) else {
        panic!("the tab is showing no page");
    };
    let fetching_a_script = Request::get(url("https://shop.example/basket.js"), by_the_page)
        .for_purpose(Purpose::Script);

    // Both lead back to the action, and through it to the person.
    for request in [&fetching_the_basket, &fetching_a_script] {
        let chain = tabs.chain(&request.cause);
        assert_eq!(
            chain.action(),
            Some(action),
            "{} was not the agent's doing: {chain}",
            request.url.serialised,
        );
        assert!(chain.followed_from(action));
        assert_eq!(chain.person(), Some(shopping), "{chain}");
        assert!(chain.is_whole(), "{chain}");
    }

    // The request the verb made is the action itself and then the person; the
    // script is one link further out, through the document the action opened.
    // That distance is the difference between a chain and a label.
    assert_eq!(tabs.chain(&fetching_the_basket.cause).links().len(), 2);
    assert_eq!(tabs.chain(&fetching_a_script.cause).links().len(), 3);
}

/// The other direction, and the one that makes the first worth anything: a page
/// the person opened themselves, fetching what it needs, is nobody's action.
///
/// An engine whose chains reached an action from everything would be an engine
/// whose record could not answer the question it exists for.
#[test]
fn a_fetch_by_a_page_the_person_opened_is_nobodys_action() {
    let mut tabs = tabs();
    let news = tabs.open(url("https://news.example/today"));
    load(
        &mut tabs,
        news,
        a_page_with_a_link("nothing happened", "/tomorrow", "Tomorrow"),
        Cause::Person { tab: news },
    );

    let Some(by_the_page) = tabs.a_page_fetching(news) else {
        panic!("the tab is showing no page");
    };
    let chain = tabs.chain(&by_the_page);

    assert_eq!(chain.action(), None, "{chain}");
    assert_eq!(chain.person(), Some(news));
    assert_eq!(chain.end(), End::Person(news));
    assert_eq!(chain.links().len(), 2, "the fetch and the page: {chain}");
}

/// An agent acting in one tab does not attribute another tab's browsing to
/// itself. The document a cause names is the one **that tab** is showing, taken
/// from the tab rather than from anything a caller or a renderer said.
#[test]
fn an_action_in_one_tab_does_not_reach_into_another() {
    let mut tabs = tabs();
    let shopping = tabs.open(url("https://shop.example/things"));
    let news = tabs.open(url("https://news.example/today"));
    load(
        &mut tabs,
        shopping,
        a_page_with_a_link("two hats", "/basket", "Basket"),
        Cause::Person { tab: shopping },
    );
    load(
        &mut tabs,
        news,
        a_page_with_a_link("nothing happened", "/tomorrow", "Tomorrow"),
        Cause::Person { tab: news },
    );

    let acted = tabs.act(shopping, Target::Named("Basket".to_owned()), Verb::Activate);
    let Ok((action, _)) = acted else {
        panic!("the agent did not act: {acted:?}");
    };

    let Some(elsewhere) = tabs.a_page_fetching(news) else {
        panic!("the other tab is showing no page");
    };
    let chain = tabs.chain(&elsewhere);
    assert_eq!(chain.action(), None, "an action reached another tab");
    assert!(!chain.followed_from(action));
    assert_eq!(chain.person(), Some(news));

    // And the agent's own tab is unaffected by the other one being open.
    let Some(here) = tabs.an_agent_acting(shopping, action) else {
        panic!("the agent's tab is showing no page");
    };
    assert_eq!(tabs.chain(&here).action(), Some(action));
}

/// A renderer that has gone is not a hole in the record: what it loaded is
/// still what the requests it made were caused by, and the walk still works
/// afterwards. The tab keeps its document for the same reason it keeps its last
/// frame — that is what it is showing.
#[test]
fn a_dead_renderer_does_not_take_what_caused_its_page_with_it() {
    let mut tabs = tabs();
    let shopping = tabs.open(url("https://shop.example/things"));
    load(
        &mut tabs,
        shopping,
        a_page_with_a_link("two hats", "/basket", "Basket"),
        Cause::Person { tab: shopping },
    );
    let Some(by_the_page) = tabs.a_page_fetching(shopping) else {
        panic!("the tab is showing no page");
    };

    let Some(process) = tabs
        .tab(shopping)
        .map(|tab| tab.site().clone())
        .and_then(|site| tabs.renderers().process_of(&site))
    else {
        panic!("a loaded tab has no renderer process");
    };
    assert!(
        std::process::Command::new("kill")
            .arg("-KILL")
            .arg(process.to_string())
            .status()
            .is_ok_and(|status| status.success()),
        "the renderer could not be killed",
    );
    assert!(matches!(tabs.paint(shopping), Err(Lost::Gone(_))));

    let chain = tabs.chain(&by_the_page);
    assert_eq!(chain.person(), Some(shopping), "{chain}");
    assert!(chain.is_whole());
    assert_eq!(
        tabs.a_page_fetching(shopping),
        Some(by_the_page),
        "a tab lost the page it is still showing",
    );
}
