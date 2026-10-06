/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 263, cut from 261: a script's `click()` follows a link
//! (ADR 0020).
//!
//! *A script at load calling `a.click()` reaches `Tabs::load`'s caller as a
//! navigation with `Cause::Document` and the claim* a script's click*;
//! inside an agent's `Activate`, the same click carries `Cause::Agent` and
//! the same claim; a refused scheme is said and navigates nowhere.*
//!
//! Two halves. The renderer's: what it **asks**, in the answer to the
//! message whose work made the ask — resolved against the document's base,
//! one per answer and the last, the agent's own link included, and the
//! links it did not follow said by name. And the browser process's, driven
//! through real [`Tabs`] over the real confined `alo-render` binary: what it
//! **decides**, with the cause it assigns from which message was answered,
//! whatever the renderer claimed.

use alo_agent::{Outcome, Target, Verb};
use alo_dom::{Document, NodeId};
use alo_layout::Size;
use alo_net::activity::{Activity, Happened};
use alo_net::cause::Cause;
use alo_renderer::ask::{Asked, By};
use alo_renderer::host::Renderers;
use alo_renderer::navigate::{Decided, Rule};
use alo_renderer::tab::{Tab, Tabs};
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::{Font, FontDatabase, Slant, Weight};
use alo_url::Url;
use std::time::SystemTime;

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

/// A URL, or `about:blank` — against which no relative link resolves — if
/// the text was wrong, so a mistake shows up as the assertion it broke.
fn url(text: &str) -> Url {
    alo_url::parse(text).unwrap_or_else(|_| Url::about_blank())
}

/// Where every page here is, unless it says otherwise.
const HERE: &str = "https://shop.example/a/things";

/// A function writing into the page's first `<p>`, and the elements after it
/// as `first`, `second` and `third`.
const PRELUDE: &str = "const out = document.body.firstChild; \
                       function say(what) { out.textContent = out.textContent + ' ' + what; } \
                       const first = out.nextSibling; const second = first.nextSibling; \
                       const third = second.nextSibling; ";

/// A page at `at` of a `<p>`, `markup`, and a script running `script` after
/// [`PRELUDE`].
fn page(at: &str, markup: &str, script: &str) -> Page {
    Page::new(
        format!("<!doctype html><body><p>-</p>{markup}<script>{PRELUDE}{script}</script></body>"),
        Size::new(400.0, 300.0),
    )
    .at(url(at))
}

/// Load `page`, collecting at every allocation from then on if `stress`, and
/// answer the renderer and what the load said.
fn load(page: Page, stress: bool) -> (Renderer, FromRenderer) {
    let mut renderer = Renderer::new(fonts());
    let answer = renderer.handle(ToRenderer::Load(Box::new(page)));
    if let Some(page_loop) = renderer.event_loop() {
        page_loop.engine().objects().heap_mut().stress(stress);
    }
    (renderer, answer)
}

/// What a load or an act asked for, and what it said — or, for any other
/// answer, nothing asked and a line saying what it was.
fn asked(answer: &FromRenderer) -> (Option<Asked>, Vec<String>) {
    match answer {
        FromRenderer::Loaded {
            navigation, issues, ..
        }
        | FromRenderer::Acted {
            navigation, issues, ..
        } => (navigation.clone(), issues.clone()),
        other => (None, vec![format!("not an answer that can ask: {other:?}")]),
    }
}

/// Activate whatever is called `name`.
fn activate(renderer: &mut Renderer, name: &str) -> FromRenderer {
    renderer.handle(ToRenderer::Act {
        target: Target::Named(name.to_owned()),
        verb: Verb::Activate,
    })
}

/// The outcome an act answered, if it was an act's answer.
fn outcome(answer: &FromRenderer) -> Option<Outcome> {
    match answer {
        FromRenderer::Acted { outcome, .. } => Some(outcome.clone()),
        _ => None,
    }
}

/// The `which`th element named `local`, in tree order.
fn nth(document: &Document, local: &str, which: usize) -> Option<NodeId> {
    document
        .descendants(document.root())
        .filter(|node| {
            document
                .element(*node)
                .is_some_and(|element| element.name.is_html(local))
        })
        .nth(which)
}

/// The text of the page's first `<p>`.
fn heard(renderer: &Renderer) -> String {
    renderer
        .document()
        .and_then(|document| Some(document.text_content(nth(document, "p", 0)?)))
        .unwrap_or_default()
}

/// The ask for `to`, with no referrer policy, as a renderer answers it.
fn going(to: &str, by: By, replaced: u32) -> Asked {
    Asked {
        url: to.to_owned(),
        by,
        referrer: None,
        replaced,
    }
}

// --- What the renderer asks ---------------------------------------------------

#[test]
fn a_scripts_click_at_load_is_asked_for_and_the_script_carries_on() {
    for stress in [false, true] {
        let (renderer, answer) = load(
            page(
                HERE,
                "<a href='../b/next?x#y'>Next</a>",
                "first.addEventListener('click', () => say('click')); first.click(); say('after');",
            ),
            stress,
        );
        let (navigation, issues) = asked(&answer);
        assert_eq!(
            navigation,
            Some(going("https://shop.example/b/next?x#y", By::Script, 0)),
            "resolved against the document's URL, as a script's click"
        );
        assert!(issues.is_empty(), "{issues:?}");
        assert_eq!(heard(&renderer), "- click after", "stress {stress}");
    }
}

#[test]
fn the_documents_first_base_href_is_what_a_link_resolves_against() {
    let (_, answer) = load(
        page(
            HERE,
            "<base href=/docs/><a href=page>Page</a><base href=/other/>",
            "second.click();",
        ),
        false,
    );
    assert_eq!(
        asked(&answer).0,
        Some(going("https://shop.example/docs/page", By::Script, 0))
    );
}

#[test]
fn one_task_asks_once_for_the_last_and_says_how_many_it_replaced() {
    let (_, answer) = load(
        page(
            HERE,
            "<a href=/one>1</a><a href=/two>2</a><a href=/three>3</a>",
            "first.click(); third.click(); second.click();",
        ),
        false,
    );
    let (navigation, issues) = asked(&answer);
    assert_eq!(
        navigation,
        Some(going("https://shop.example/two", By::Script, 2))
    );
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert!(
        issues
            .first()
            .is_some_and(|issue| issue.contains("2 times")),
        "{issues:?}"
    );
}

#[test]
fn a_link_nobody_could_follow_is_said_and_asks_for_nothing() {
    let (renderer, answer) = load(
        page(
            HERE,
            "<a href=/x target=_blank>New</a><a href='http://[::'>Broken</a>",
            "first.click(); second.click(); say('after');",
        ),
        false,
    );
    let (navigation, issues) = asked(&answer);
    assert_eq!(navigation, None);
    assert_eq!(heard(&renderer), "- after", "not followed is not thrown");
    assert_eq!(issues.len(), 2, "{issues:?}");
    assert!(issues.iter().any(|issue| issue.contains("queue item 118")));
    assert!(issues.iter().any(|issue| issue.contains("goes nowhere")));
}

#[test]
fn a_scripts_click_on_a_download_is_refused_by_name_and_asks_for_nothing() {
    let (renderer, answer) = load(
        page(
            HERE,
            "<a href=/report.csv download>Save</a>",
            "first.addEventListener('click', () => say('click')); first.click(); \
             say('not reached');",
        ),
        false,
    );
    let (navigation, issues) = asked(&answer);
    assert_eq!(navigation, None);
    assert_eq!(heard(&renderer), "- click", "its listeners ran first");
    assert!(
        issues.iter().any(|issue| issue.contains("queue item 264")),
        "{issues:?}"
    );
}

#[test]
fn a_listeners_click_inside_an_agents_activate_is_in_the_acts_answer() {
    for stress in [false, true] {
        let (mut renderer, loaded) = load(
            page(
                HERE,
                "<button>Go</button><a href=/next>Next</a>",
                "first.addEventListener('click', () => second.click());",
            ),
            stress,
        );
        assert_eq!(asked(&loaded), (None, Vec::new()));
        let answer = activate(&mut renderer, "Go");
        assert!(
            matches!(outcome(&answer), Some(Outcome::Activated { .. })),
            "{answer:?}"
        );
        assert_eq!(
            asked(&answer),
            (
                Some(going("https://shop.example/next", By::Script, 0)),
                Vec::new()
            )
        );
        // Taken with the first answer: the second press asks afresh, and
        // replaces nothing the first asked for.
        let again = activate(&mut renderer, "Go");
        assert_eq!(
            asked(&again).0,
            Some(going("https://shop.example/next", By::Script, 0)),
        );
    }
}

#[test]
fn the_agents_own_link_is_the_browsers_ask_unless_a_listener_cancels_it() {
    let (mut renderer, _) = load(page(HERE, "<a href=/next>Next</a>", ""), false);
    let answer = activate(&mut renderer, "Next");
    assert!(
        matches!(outcome(&answer), Some(Outcome::Followed { ref to, .. }) if to == "/next"),
        "{answer:?}"
    );
    assert_eq!(
        asked(&answer).0,
        Some(going("https://shop.example/next", By::Browser, 0))
    );

    let (mut renderer, _) = load(
        page(
            HERE,
            "<a href=/next>Next</a>",
            "first.addEventListener('click', e => e.preventDefault());",
        ),
        false,
    );
    let answer = activate(&mut renderer, "Next");
    assert!(
        matches!(outcome(&answer), Some(Outcome::Activated { .. })),
        "{answer:?}"
    );
    assert_eq!(asked(&answer), (None, Vec::new()));
}

#[test]
fn the_agents_link_replaces_what_its_listener_clicked_and_a_cancelled_one_does_not() {
    for stress in [false, true] {
        let (mut renderer, _) = load(
            page(
                HERE,
                "<a href=/mine>Mine</a><a href=/theirs>Theirs</a>",
                "first.addEventListener('click', () => second.click());",
            ),
            stress,
        );
        let answer = activate(&mut renderer, "Mine");
        assert_eq!(
            asked(&answer).0,
            Some(going("https://shop.example/mine", By::Browser, 1)),
            "the link's own activation runs after its listeners, and is last"
        );

        let (mut renderer, _) = load(
            page(
                HERE,
                "<a href=/mine>Mine</a><a href=/theirs>Theirs</a>",
                "first.addEventListener('click', e => { second.click(); e.preventDefault(); });",
            ),
            stress,
        );
        let answer = activate(&mut renderer, "Mine");
        assert!(
            matches!(outcome(&answer), Some(Outcome::Activated { .. })),
            "{answer:?}"
        );
        assert_eq!(
            asked(&answer).0,
            Some(going("https://shop.example/theirs", By::Script, 0)),
            "cancelling the agent's click does not undo the script's"
        );
    }
}

#[test]
fn a_page_that_never_ran_script_asks_for_the_agents_link_itself() {
    let mut renderer = Renderer::new(fonts());
    renderer.handle(ToRenderer::Load(Box::new(
        Page::new(
            "<a href=/next referrerpolicy=origin>Next</a><a href=/new target=_blank>New</a>\
             <a href=/f download>Save</a>",
            Size::new(400.0, 300.0),
        )
        .at(url(HERE)),
    )));
    assert!(renderer.event_loop().is_none(), "no heap was built");
    let answer = activate(&mut renderer, "Next");
    assert!(
        matches!(outcome(&answer), Some(Outcome::Followed { .. })),
        "{answer:?}"
    );
    assert_eq!(
        asked(&answer).0,
        Some(Asked {
            url: "https://shop.example/next".to_owned(),
            by: By::Browser,
            referrer: Some(alo_net::referrer::Policy::Origin),
            replaced: 0,
        })
    );
    for (name, said) in [("New", "queue item 118"), ("Save", "queue item 264")] {
        let answer = activate(&mut renderer, name);
        assert!(
            matches!(outcome(&answer), Some(Outcome::Activated { .. })),
            "{name}: {answer:?}"
        );
        let (navigation, issues) = asked(&answer);
        assert_eq!(navigation, None);
        assert!(
            issues.iter().any(|issue| issue.contains(said)),
            "{issues:?}"
        );
    }
}

#[test]
fn a_link_on_a_page_nobody_fetched_goes_nowhere_unless_it_is_absolute() {
    let mut renderer = Renderer::new(fonts());
    renderer.handle(ToRenderer::Load(Box::new(Page::new(
        "<a href=/next>Next</a><a href=https://elsewhere.example/>Away</a>",
        Size::new(400.0, 300.0),
    ))));
    let answer = activate(&mut renderer, "Next");
    assert!(
        matches!(outcome(&answer), Some(Outcome::Activated { .. })),
        "{answer:?}"
    );
    let (navigation, issues) = asked(&answer);
    assert_eq!(navigation, None);
    assert!(
        issues.iter().any(|issue| issue.contains("goes nowhere")),
        "{issues:?}"
    );
    let answer = activate(&mut renderer, "Away");
    assert_eq!(
        asked(&answer).0,
        Some(going("https://elsewhere.example/", By::Browser, 0))
    );
}

#[test]
fn a_page_whose_script_stopped_still_asks_for_the_agents_link() {
    let (mut renderer, _) = load(
        page(
            HERE,
            "<a href=/next>Next</a>",
            "first.addEventListener('click', () => say('heard'));",
        ),
        false,
    );
    renderer
        .event_loop()
        .expect("a scripted page")
        .stop_switch()
        .ask();
    // The first click's task is where the page stops; the second is not
    // queued at all, and the renderer follows the link itself.
    let _ = activate(&mut renderer, "Next");
    let answer = activate(&mut renderer, "Next");
    assert_eq!(heard(&renderer), "-", "no listener ran");
    assert!(
        matches!(outcome(&answer), Some(Outcome::Followed { .. })),
        "{answer:?}"
    );
    assert_eq!(
        asked(&answer).0,
        Some(going("https://shop.example/next", By::Browser, 0))
    );
}

// --- What the browser process decides -----------------------------------------

/// The renderer binary, as cargo built it for this test.
fn tabs() -> Tabs {
    Tabs::over(Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[]))
}

#[test]
fn a_scripts_click_at_load_reaches_the_tabs_caller_as_the_documents() {
    let mut tabs = tabs();
    let tab = tabs.open(url(HERE));
    let loaded = tabs.load(
        tab,
        page(HERE, "<a href=/basket>Basket</a>", "first.click();"),
        Cause::Person { tab },
    );
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    let document = tabs.tab(tab).and_then(Tab::document).expect("a document");
    assert_eq!(
        tabs.tab(tab).and_then(Tab::address),
        Some(&url(HERE)),
        "the browser process keeps its own copy of where the page is"
    );
    let Some(Decided::Go(navigation)) = tabs.navigation(tab) else {
        panic!("the page's click did not reach the caller");
    };
    assert_eq!(navigation.url, url("https://shop.example/basket"));
    assert_eq!(navigation.cause, Cause::Document { document });
    assert_eq!(navigation.by, By::Script);
    assert_eq!(
        navigation.referrer.as_deref(),
        Some(HERE),
        "the same origin gets the whole URL by default"
    );
    assert_eq!(tabs.navigation(tab), None, "handed over once");
}

#[test]
fn inside_an_agents_activate_the_same_click_is_the_agents() {
    let mut tabs = tabs();
    let tab = tabs.open(url(HERE));
    let loaded = tabs.load(
        tab,
        page(
            HERE,
            "<button>Go</button><a href=/basket>Basket</a>",
            "first.addEventListener('click', () => second.click());",
        ),
        Cause::Person { tab },
    );
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );
    assert_eq!(tabs.navigation(tab), None, "the load asked for nothing");
    let document = tabs.tab(tab).and_then(Tab::document).expect("a document");

    let acted = tabs.act(tab, Target::Named("Go".to_owned()), Verb::Activate);
    let Ok((action, FromRenderer::Acted { .. })) = acted else {
        panic!("Go was not activated: {acted:?}");
    };
    let Some(Decided::Go(navigation)) = tabs.navigation(tab) else {
        panic!("the listener's click did not reach the caller");
    };
    assert_eq!(navigation.url, url("https://shop.example/basket"));
    assert_eq!(navigation.cause, Cause::Agent { action, document });
    assert_eq!(navigation.by, By::Script, "the claim is kept beside it");

    // An act whose answer asks for nothing leaves nothing waiting.
    let acted = tabs.act(tab, Target::Named("Basket".to_owned()), Verb::Activate);
    assert!(acted.is_ok(), "{acted:?}");
    let Some(Decided::Go(own)) = tabs.navigation(tab) else {
        panic!("the agent's own link did not reach the caller");
    };
    assert_eq!(own.by, By::Browser);
    assert!(matches!(own.cause, Cause::Agent { .. }));
    let refused = tabs.act(tab, Target::Named("Nowhere".to_owned()), Verb::Activate);
    assert!(
        matches!(refused, Ok((_, FromRenderer::Refused(_)))),
        "{refused:?}"
    );
    assert_eq!(tabs.navigation(tab), None);
}

#[test]
fn a_refused_scheme_is_said_and_recorded_and_goes_nowhere() {
    for (to, rule) in [
        (
            "data:text/html,<form>",
            Rule::Scheme {
                scheme: "data".to_owned(),
            },
        ),
        (
            "javascript:alert(1)",
            Rule::Scheme {
                scheme: "javascript".to_owned(),
            },
        ),
        ("file:///etc/passwd", Rule::FileFromElsewhere),
    ] {
        let mut tabs = tabs();
        let tab = tabs.open(url(HERE));
        let loaded = tabs.load(
            tab,
            page(HERE, &format!("<a href='{to}'>Away</a>"), "first.click();"),
            Cause::Person { tab },
        );
        assert!(
            matches!(loaded, Ok(FromRenderer::Loaded { .. })),
            "{loaded:?}"
        );
        let document = tabs.tab(tab).and_then(Tab::document).expect("a document");
        let Some(Decided::Refused(refusal)) = tabs.navigation(tab) else {
            panic!("{to} was not refused");
        };
        assert_eq!(refusal.rule, rule, "{to}");
        assert_eq!(refusal.cause, Cause::Document { document });
        let said = refusal.to_string();
        assert!(
            said.contains("refused") && said.contains("a script's click"),
            "{said}"
        );

        let mut activity = Activity::new();
        assert!(refusal.record(&mut activity, SystemTime::UNIX_EPOCH));
        let line = activity.latest().expect("a line");
        assert_eq!(line.cause(), &Cause::Document { document });
        assert!(
            matches!(line.happened(), Happened::Refused { .. }),
            "{line:?}"
        );
    }
}
