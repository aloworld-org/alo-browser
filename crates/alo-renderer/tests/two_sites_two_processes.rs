//! ADR 0005's central claim, as processes that actually exist.
//!
//! These tests spawn the real renderer binary and talk to it over real pipes.
//! One of them kills a process while another is running, which is the only way
//! to check the thing the whole design is for: **a renderer dying takes its tab
//! and nothing else.**

use alo_css::media::ColorScheme;
use alo_layout::geometry::Size;
use alo_renderer::host::{MOST_RENDERERS, Renderers};
use alo_renderer::message::{FromRenderer, ToRenderer};
use alo_renderer::page::Page;
use alo_renderer::site::Site;

/// The renderer binary, as cargo built it for this test.
fn renderers() -> Renderers {
    Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[])
}

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

fn site(text: &str) -> Site {
    Site::of(&url(text))
}

fn a_page(text: &str) -> ToRenderer {
    ToRenderer::Load(Box::new(Page {
        html: format!("<p>{text}</p>"),
        sheets: vec!["p { margin: 8px; font-size: 16px }".to_owned()],
        viewport: Size {
            width: 200.0,
            height: 100.0,
        },
        scheme: ColorScheme::Light,
    }))
}

// --- The claim ---------------------------------------------------------------

/// The sentence ADR 0005 is written to make true.
#[test]
fn two_sites_are_two_processes_and_two_tabs_on_one_site_are_one() {
    let mut renderers = renderers();
    let bank = site("https://bank.example/statement");
    let news = site("https://news.example/today");

    assert!(matches!(
        renderers.ask(&bank, &a_page("a statement")),
        Ok(FromRenderer::Loaded { .. })
    ));
    assert!(matches!(
        renderers.ask(&news, &a_page("the news")),
        Ok(FromRenderer::Loaded { .. })
    ));

    assert_eq!(renderers.len(), 2, "two sites shared a process");
    assert_ne!(
        renderers.process_of(&bank),
        renderers.process_of(&news),
        "two sites are in the same process"
    );

    // A second tab on a site already open is the same process, not a third.
    let same_site_again = site("https://bank.example/settings");
    assert!(renderers.ask(&same_site_again, &a_page("settings")).is_ok());
    assert_eq!(
        renderers.len(),
        2,
        "a second tab on one site started a process"
    );
    assert_eq!(renderers.started(), 2);
}

/// The reason for all of it: a renderer dying takes its tab and nothing else.
#[test]
fn killing_one_renderer_leaves_the_other_running() {
    let mut renderers = renderers();
    let bank = site("https://bank.example/");
    let news = site("https://news.example/");
    assert!(renderers.ask(&bank, &a_page("a statement")).is_ok());
    assert!(renderers.ask(&news, &a_page("the news")).is_ok());

    // Kill the news renderer the way the operating system would.
    let victim = renderers
        .process_of(&news)
        .unwrap_or_else(|| panic!("the news renderer has no process"));
    let killed = std::process::Command::new("kill")
        .arg("-9")
        .arg(victim.to_string())
        .status();
    assert!(
        killed.is_ok_and(|status| status.success()),
        "could not kill it"
    );
    // Give the operating system a moment to actually reap it.
    std::thread::sleep(std::time::Duration::from_millis(200));

    let gone = renderers.ask(&news, &a_page("the news again"));
    assert!(gone.is_err(), "a dead renderer answered");
    let why = gone.err().map(|why| why.to_string()).unwrap_or_default();
    assert!(
        why.contains("news.example"),
        "the failure should say whose renderer it was: {why:?}"
    );

    // And the other site is untouched — which is the entire point.
    assert!(
        matches!(
            renderers.ask(&bank, &a_page("still here")),
            Ok(FromRenderer::Loaded { .. })
        ),
        "one renderer dying took another site's with it"
    );
}

/// A browser that silently starts another one hides a bug somebody needs to
/// see, and turns a page that crashes its renderer every time into an
/// invisible loop.
#[test]
fn a_dead_renderer_is_not_quietly_restarted_to_answer_the_same_question() {
    let mut renderers = renderers();
    let news = site("https://news.example/");
    assert!(renderers.ask(&news, &a_page("the news")).is_ok());
    assert_eq!(renderers.started(), 1);

    let victim = renderers.process_of(&news).unwrap_or(0);
    let _ = std::process::Command::new("kill")
        .arg("-9")
        .arg(victim.to_string())
        .status();
    std::thread::sleep(std::time::Duration::from_millis(200));

    assert!(renderers.ask(&news, &a_page("again")).is_err());
    assert_eq!(
        renderers.started(),
        1,
        "the failing request silently started another renderer"
    );
    assert!(!renderers.holds(&news), "a dead renderer was kept");

    // A *deliberate* load afterwards does get a fresh process. Asking again is
    // the caller's decision, which is exactly the distinction.
    assert!(renderers.ask(&news, &a_page("reloaded")).is_ok());
    assert_eq!(renderers.started(), 2);
}

// --- What crosses ------------------------------------------------------------

/// A whole exchange through two processes: load, paint, read the tree. If the
/// wire format were wrong anywhere, this is where it would show.
#[test]
fn a_page_loads_paints_and_reads_back_across_the_boundary() {
    let mut renderers = renderers();
    let here = site("https://example.com/");

    let loaded = renderers.ask(&here, &a_page("hello from another process"));
    assert!(
        matches!(loaded, Ok(FromRenderer::Loaded { .. })),
        "{loaded:?}"
    );

    let painted = renderers.ask(&here, &ToRenderer::Paint);
    let Ok(FromRenderer::Painted(frame)) = painted else {
        panic!("nothing was painted: {painted:?}");
    };
    assert_eq!(frame.width, 200);
    assert_eq!(frame.height, 100);
    assert_eq!(
        frame.pixels.len(),
        200 * 100 * 4,
        "the frame's size and its pixels disagree"
    );

    let tree = renderers.ask(&here, &ToRenderer::ReadTree);
    let Ok(FromRenderer::Tree(snapshot)) = tree else {
        panic!("no tree came back: {tree:?}");
    };
    let root = snapshot
        .root
        .unwrap_or_else(|| panic!("the tree that came back has no root"));
    assert!(
        format!("{root:?}").contains("hello from another process"),
        "the page's own text did not survive two processes"
    );
}

/// A renderer with nothing loaded says so rather than dying, and stays usable.
#[test]
fn a_renderer_asked_for_something_it_does_not_have_answers_and_carries_on() {
    let mut renderers = renderers();
    let here = site("https://example.com/");

    let nothing = renderers.ask(&here, &ToRenderer::Paint);
    assert!(
        matches!(nothing, Ok(FromRenderer::Failed(_))),
        "expected a failure message, got {nothing:?}"
    );
    assert!(renderers.holds(&here), "it exited rather than answering");

    // And it still works.
    assert!(renderers.ask(&here, &a_page("now there is a page")).is_ok());
    assert!(matches!(
        renderers.ask(&here, &ToRenderer::Paint),
        Ok(FromRenderer::Painted(_))
    ));
    assert_eq!(renderers.started(), 1, "it was restarted somewhere");
}

// --- Bounds ------------------------------------------------------------------

/// N processes cost N processes, so the price has to have a ceiling. A browser
/// with three hundred tabs on three hundred sites cannot be three hundred
/// processes.
#[test]
fn there_is_a_ceiling_on_how_many_renderers_exist() {
    let mut renderers = renderers();
    for n in 0..(MOST_RENDERERS + 4) {
        let site = site(&format!("https://site{n}.example/"));
        assert!(
            renderers.ask(&site, &a_page("a page")).is_ok(),
            "site {n} could not be opened"
        );
        assert!(
            renderers.len() <= MOST_RENDERERS,
            "there are {} renderers, past the ceiling of {MOST_RENDERERS}",
            renderers.len()
        );
    }
    assert_eq!(renderers.len(), MOST_RENDERERS);
    // The oldest went; the newest is still there.
    assert!(!renderers.holds(&site("https://site0.example/")));
    assert!(renderers.holds(&site(&format!(
        "https://site{}.example/",
        MOST_RENDERERS + 3
    ))));
}

/// Sites are told apart by scheme as well as host, because `http://` and
/// `https://` are not one another (ADR 0005, and the origin rules in
/// `alo-url`).
#[test]
fn a_scheme_is_enough_to_make_it_a_different_site() {
    assert_ne!(site("http://example.com/"), site("https://example.com/"));
    assert_eq!(
        site("https://example.com/one"),
        site("https://example.com/two")
    );
}

/// The host is not the site, and this test asserted that it was until queue
/// item 156 rented the public suffix list. ADR 0005 says a site is the scheme
/// and the **registrable domain**: two subdomains of one organisation share a
/// process, and two organisations under one public suffix never do — which no
/// comparison of host strings could have decided.
#[test]
fn two_subdomains_are_one_site_and_two_organisations_are_not() {
    assert_eq!(
        site("https://a.example.com/"),
        site("https://b.example.com/")
    );
    assert_eq!(
        site("https://www.example.com/"),
        site("https://example.com/")
    );
    assert_ne!(
        site("https://www.bbc.co.uk/"),
        site("https://www.gov.co.uk/")
    );
}
