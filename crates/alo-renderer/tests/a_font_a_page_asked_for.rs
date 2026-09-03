/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! A page asks for a font by name, and somebody answers.
//!
//! Queue item 170, end to end and over the real boundary. Before it, a renderer
//! held whatever short list the browser process had found at startup, a page
//! asking for anything else was drawn in whatever was to hand, and **nothing
//! anywhere said so** — a render that was stable, diffable, and not what the
//! page looks like in any other browser.
//!
//! Three things close the item and there is a test for each:
//!
//! 1. a renderer says which family it wanted and did not have,
//! 2. the browser process — the only side that may open a file — answers with
//!    it, and
//! 3. a family that genuinely is not on this machine comes back as a **named**
//!    substitution rather than a silence.
//!
//! Sibling to `a_font_a_renderer_was_handed.rs`, which is about the fonts a
//! renderer is *given*. This one is about the ones it has to ask for.

use alo_css::media::ColorScheme;
use alo_layout::geometry::Size;
use alo_renderer::fonts;
use alo_renderer::host::Renderers;
use alo_renderer::message::{FromRenderer, ToRenderer};
use alo_renderer::page::Page;
use alo_renderer::site::Site;

const RENDERER: &str = env!("CARGO_BIN_EXE_alo-render");

/// A family no machine has, spelt so that it could not be a real one.
const NOWHERE: &str = "A Family Nobody Ships 8f2c";

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

/// A page whose text asks for one family and names no fallback.
fn asking_for(family: &str) -> ToRenderer {
    ToRenderer::Load(Box::new(Page {
        html: "<p>text needs a font</p>".to_owned(),
        sheets: vec![format!("p {{ font-family: '{family}'; font-size: 16px }}")],
        viewport: Size {
            width: 200.0,
            height: 60.0,
        },
        scheme: ColorScheme::Light,
    }))
}

/// A family this machine really has, by the name the font gives itself.
///
/// [`None`] on a machine with no font files at all, which is a real state — a
/// stripped container is one — and the tests that need one say so rather than
/// failing.
fn a_family_this_machine_has() -> Option<String> {
    fonts::from_this_machine()
        .iter()
        .find_map(|face| alo_text::family_in(&face.bytes))
}

// --- 1. The renderer says what it wanted -------------------------------------

#[test]
fn a_renderer_says_which_family_it_wanted_and_did_not_have() {
    let mut renderers = Renderers::running(RENDERER, &[]);
    let site = Site::of(&url("https://example.com/"));

    let answer = renderers.ask(&site, &asking_for(NOWHERE));
    let Ok(FromRenderer::Loaded { issues, wanted }) = answer else {
        panic!("the page did not load: {answer:?}");
    };
    assert!(
        wanted.iter().any(|family| family == NOWHERE),
        "the family the page asked for is not in what it wanted: {wanted:?}",
    );
    assert!(
        issues.iter().any(|issue| issue.contains(NOWHERE)),
        "and a person is told as well as the browser process: {issues:?}",
    );
}

#[test]
fn a_family_a_renderer_already_holds_is_wanted_by_nobody() {
    let Some(family) = a_family_this_machine_has() else {
        return;
    };
    let faces = fonts::named(&family);
    assert!(!faces.is_empty(), "{family:?} was found and then was not");

    let mut renderers = Renderers::running(RENDERER, &[]).with_fonts(faces);
    let site = Site::of(&url("https://example.com/"));

    let answer = renderers.ask(&site, &asking_for(&family));
    let Ok(FromRenderer::Loaded { issues, wanted }) = answer else {
        panic!("the page did not load: {answer:?}");
    };
    assert!(
        !wanted.iter().any(|asked| asked == &family),
        "a renderer asked for a family it was already holding: {wanted:?}",
    );
    assert!(
        !issues.iter().any(|issue| issue.contains(&family)),
        "and nothing was substituted, so nothing should say it was: {issues:?}",
    );
}

// --- 2. The browser process answers ------------------------------------------

#[test]
fn the_browser_process_answers_with_a_family_the_machine_has() {
    let Some(family) = a_family_this_machine_has() else {
        return;
    };
    // Deliberately started with **no** fonts, so the only way the renderer can
    // end up with this family is the exchange this test is about.
    let mut renderers = Renderers::running(RENDERER, &[]);
    let site = Site::of(&url("https://example.com/"));

    let answer = renderers.ask(&site, &asking_for(&family));
    let Ok(FromRenderer::Loaded { wanted, .. }) = answer else {
        panic!("the page did not load: {answer:?}");
    };
    assert!(
        wanted.iter().any(|asked| asked == &family),
        "a renderer with no fonts should have wanted {family:?}: {wanted:?}",
    );

    let absent = renderers
        .supply(&site, &wanted)
        .expect("the renderer stayed alive while being sent fonts");
    assert!(
        !absent.iter().any(|missing| missing == &family),
        "{family:?} is on this machine and was reported as not: {absent:?}",
    );

    // The page again, and this time it gets what it asked for. Loading it a
    // second time is the *caller's* decision, which is why `supply` does not
    // do it — see its own documentation.
    let answer = renderers.ask(&site, &asking_for(&family));
    let Ok(FromRenderer::Loaded { issues, wanted }) = answer else {
        panic!("the page did not load a second time: {answer:?}");
    };
    assert!(
        !wanted.iter().any(|asked| asked == &family),
        "the family was sent over and is still being asked for: {wanted:?}",
    );
    assert!(
        !issues.iter().any(|issue| issue.contains(&family)),
        "and nothing was substituted for it any more: {issues:?}",
    );
}

// --- 3. What is not on the machine is named ----------------------------------

#[test]
fn a_family_this_machine_does_not_have_comes_back_named() {
    let mut renderers = Renderers::running(RENDERER, &[]);
    let site = Site::of(&url("https://example.com/"));

    let answer = renderers.ask(&site, &asking_for(NOWHERE));
    let Ok(FromRenderer::Loaded { wanted, .. }) = answer else {
        panic!("the page did not load: {answer:?}");
    };

    let absent = renderers
        .supply(&site, &wanted)
        .expect("the renderer stayed alive");
    assert!(
        absent.iter().any(|missing| missing == NOWHERE),
        "a family nobody has must be reported by name, not by silence: {absent:?}",
    );
}

#[test]
fn a_family_nobody_ships_is_found_nowhere_on_this_machine() {
    assert!(
        fonts::named(NOWHERE).is_empty(),
        "this machine claims to have a font invented for a test",
    );
}

// --- The name came off a page, so it is a stranger's -------------------------

#[test]
fn a_family_name_is_never_read_as_a_path() {
    // Each of these is a family name as far as this engine is concerned: it is
    // compared against the family a font states about *itself*, and no font
    // states any of them. Finding a file would mean the name had been joined to
    // a directory somewhere, which is the bug this asserts the absence of.
    for hostile in [
        "../../../../etc/passwd",
        "/System/Library/Fonts/Helvetica.ttc",
        "..",
        ".",
        "/",
        "DejaVuSans.ttf",
        "\0",
        "font\u{0}name",
    ] {
        assert!(
            fonts::named(hostile).is_empty(),
            "{hostile:?} was resolved to a font",
        );
    }
}

#[test]
fn a_family_of_nothing_is_asked_after_nowhere() {
    assert!(fonts::named("").is_empty());
    assert!(fonts::named("   ").is_empty());
    assert!(fonts::named("\n\t").is_empty());
}
