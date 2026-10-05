/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 236, cut from 233: the renderer holds a page's event loop, and
//! a page's own classic scripts run as it loads — in document order, each a
//! task followed by its checkpoint, and only where the page's
//! `Content-Security-Policy` allows them.
//!
//! *A page's script runs, oldest first, with each script's microtasks before
//! the next script; a throw is said and the next script runs; a script that
//! is fetched, a module and an import map are said not to have run; a script
//! the page's policy forbids — by header or by a `<meta>` written before it,
//! with nonces only from markup the page could have written — does not run
//! and says why; a resize runs nothing again; a new page starts from nothing;
//! and a script that never ends costs its renderer, not the browser.*
//!
//! # How a test sees what a script did
//!
//! No script can reach the document yet (queue item 80), so what a script
//! leaves behind is read from the page's own engine through
//! [`Renderer::event_loop`], which is not part of the boundary — the reason
//! [`Renderer::rendered`] is not.

use std::time::{Duration, Instant};

use alo_js::interpret::Trouble;
use alo_js::{Value, script};
use alo_layout::Size;
use alo_renderer::host::Renderers;
use alo_renderer::site::Site;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;

/// A small window; nothing here is about where anything is.
const WINDOW: Size = Size {
    width: 200.0,
    height: 100.0,
};

fn page(html: &str) -> Page {
    Page::new(html, WINDOW)
}

/// Load a page into a fresh renderer, and answer the renderer and the issues
/// its load came back with.
fn load(page: Page) -> (Renderer, Vec<String>) {
    let mut renderer = Renderer::new(FontDatabase::new());
    let issues = match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded { issues, .. } => issues,
        other => vec![format!("not loaded: {other:?}")],
    };
    (renderer, issues)
}

/// The issues that are about scripts, which every one of starts `script `.
fn about_scripts(issues: &[String]) -> Vec<&str> {
    issues
        .iter()
        .map(String::as_str)
        .filter(|issue| issue.starts_with("script "))
        .collect()
}

/// What the global `out` holds, as text, or why it could not be read — "no
/// script ran" when the page's loop was never made.
fn out(renderer: &mut Renderer) -> String {
    let Some(looping) = renderer.event_loop() else {
        return "no script ran".to_owned();
    };
    let program = match script("this.out") {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    match looping.engine().evaluate(&program) {
        Ok(Value::Text(held)) => match looping.engine().objects().units(held) {
            Some(units) => String::from_utf16_lossy(units),
            None => "a string that has gone".to_owned(),
        },
        Ok(other) => format!("{other:?}"),
        Err(Trouble::Escaped(escape)) => format!("! {escape}"),
        Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
    }
}

// --- Order -------------------------------------------------------------------

#[test]
fn scripts_run_in_document_order_with_each_ones_jobs_before_the_next() {
    let (mut renderer, issues) = load(page(
        "<head><script>out = 'a'; queueMicrotask(() => { out += 'b'; \
         queueMicrotask(() => { out += 'c' }) })</script></head>\
         <body><p>text</p><script>out += 'd'; queueMicrotask(() => { out += 'f' }); \
         out += 'e'</script><script>out += 'g'</script></body>",
    ));
    assert_eq!(about_scripts(&issues), Vec::<&str>::new());
    assert_eq!(out(&mut renderer), "abcdefg");
}

#[test]
fn a_throw_is_said_and_the_next_script_runs() {
    let (mut renderer, issues) = load(page(
        "<script>out = 'a'; queueMicrotask(() => { throw 4 }); \
         throw new TypeError('no such thing')</script>\
         <script>out += 'b'; queueMicrotask(() => { out += 'c' })</script>\
         <script>out += </script>\
         <script>out += 'd'</script>",
    ));
    let said = about_scripts(&issues);
    assert_eq!(said.len(), 3, "{said:?}");
    // A `TypeError` a script made is reported as the object it is: naming it
    // by its `name` and `message` without running a getter is item 239.
    assert_eq!(said.first(), Some(&"script 1: uncaught: an object"));
    assert_eq!(said.get(1), Some(&"script 1: uncaught: 4"));
    assert!(
        said.get(2)
            .is_some_and(|it| it.starts_with("script 3: not a script:")),
        "{said:?}"
    );
    assert_eq!(out(&mut renderer), "abcd");
}

#[test]
fn a_script_that_stops_the_page_stops_every_script_after_it() {
    // A builtin this engine refuses by name is not a throw a page can catch:
    // it ends the page's script (ADR 0016 § 7).
    let (mut renderer, issues) = load(page(
        "<script>out = 'a'; (function () {}).toString(); out += 'x'</script>\
         <script>out += 'b'</script>\
         <script>out += 'c'</script>",
    ));
    let said = about_scripts(&issues);
    assert!(
        said.first()
            .is_some_and(|it| it.starts_with("script 1: the page stopped")),
        "{said:?}"
    );
    assert_eq!(
        said.get(1..),
        Some(
            &[
                "script 2: not run, because the page's script has stopped",
                "script 3: not run, because the page's script has stopped",
            ][..]
        ),
    );
    assert_eq!(out(&mut renderer), "a");
}

// --- What does not run -------------------------------------------------------

#[test]
fn a_fetched_script_a_module_and_an_import_map_are_said_not_to_have_run() {
    let (mut renderer, issues) = load(page(
        "<script src=app.js></script>\
         <script type=module>out = 'module'</script>\
         <script type=importmap>{}</script>\
         <script>out = 'classic'</script>",
    ));
    let said = about_scripts(&issues);
    assert_eq!(said.len(), 3, "{said:?}");
    assert!(
        said.first()
            .is_some_and(|it| it.starts_with("script 1: not run")
                && it.contains("\"app.js\"")
                && it.contains("238")),
        "{said:?}"
    );
    assert!(
        said.get(1)
            .is_some_and(|it| it.starts_with("script 2: not run") && it.contains("77")),
        "{said:?}"
    );
    assert!(
        said.get(2)
            .is_some_and(|it| it.starts_with("script 3: not run") && it.contains("77")),
        "{said:?}"
    );
    assert_eq!(out(&mut renderer), "classic");
}

#[test]
fn a_data_block_a_nomodule_fallback_and_a_templates_script_are_not_scripts() {
    let (mut renderer, issues) = load(page(
        "<script type=text/plain>out = 'data'</script>\
         <script nomodule>out = 'fallback'</script>\
         <template><script>out = 'inert'</script></template>\
         <script>out = this.out === undefined ? 'only me' : 'not only me'</script>",
    ));
    assert_eq!(about_scripts(&issues), Vec::<&str>::new());
    assert_eq!(out(&mut renderer), "only me");
}

#[test]
fn a_page_with_no_script_has_no_loop() {
    let (mut renderer, issues) = load(page("<p>nothing to run</p>"));
    assert_eq!(about_scripts(&issues), Vec::<&str>::new());
    assert!(renderer.event_loop().is_none());
}

// --- The page's policy -------------------------------------------------------

/// What `out` is after loading `html` under these policies, and what was said.
fn under(policies: &[&str], html: &str) -> (String, Vec<String>) {
    let mut page = page(html);
    for policy in policies {
        page = page.with_policy(*policy);
    }
    let (mut renderer, issues) = load(page);
    let said = about_scripts(&issues)
        .into_iter()
        .map(ToOwned::to_owned)
        .collect();
    (out(&mut renderer), said)
}

#[test]
fn a_policy_that_forbids_inline_script_is_obeyed_and_says_so() {
    for policy in [
        "script-src 'self'",
        "default-src 'self'",
        "script-src 'none'",
        "script-src 'unsafe-inline' 'nonce-abc'",
    ] {
        let (ran, said) = under(&[policy], "<script>out = 'ran'</script>");
        assert_eq!(ran, "no script ran", "{policy}");
        assert!(
            said.first()
                .is_some_and(|it| it.starts_with("script 1: refused:")),
            "{policy}: {said:?}"
        );
    }
}

#[test]
fn a_policy_that_allows_inline_script_lets_it_run() {
    for policy in [
        "script-src 'unsafe-inline'",
        "default-src 'self' 'unsafe-inline'",
        "img-src 'none'",
        "script-src 'sha256-fgFJA3JuizDxeTMjfPzgq5uzcLfnYe5mo6sJdUo89zc='",
    ] {
        let (ran, said) = under(&[policy], "<script>out='hashed'</script>");
        assert_eq!(ran, "hashed", "{policy}: {said:?}");
    }
}

#[test]
fn a_hash_names_exactly_the_text_it_was_taken_of() {
    let (ran, said) = under(
        &["script-src 'sha256-fgFJA3JuizDxeTMjfPzgq5uzcLfnYe5mo6sJdUo89zc='"],
        "<script> out='hashed'</script>",
    );
    assert_eq!(ran, "no script ran");
    assert_eq!(said.len(), 1, "{said:?}");
}

#[test]
fn two_policies_are_both_obeyed() {
    let (ran, _) = under(
        &["script-src 'unsafe-inline'", "script-src 'none'"],
        "<script>out = 'ran'</script>",
    );
    assert_eq!(ran, "no script ran");
}

#[test]
fn a_nonce_lets_in_the_script_that_carries_it_and_no_other() {
    let (ran, said) = under(
        &["script-src 'nonce-abc'"],
        "<script nonce=abc>out = 'a'</script>\
         <script nonce=abd>out += 'x'</script>\
         <script>out += 'y'</script>\
         <script nonce=abc>out += 'b'</script>",
    );
    assert_eq!(ran, "ab");
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(
        said.first()
            .is_some_and(|it| it.starts_with("script 2: refused"))
    );
    assert!(
        said.get(1)
            .is_some_and(|it| it.starts_with("script 3: refused"))
    );
}

#[test]
fn a_nonce_in_markup_an_injection_left_lets_nothing_in() {
    for html in [
        // A dangling `<script src=… x="` swallowing the page's own tag.
        "<script x=\"<script \" nonce=abc>out = 'injected'</script>",
        "<script <style nonce=abc>out = 'injected'</script>",
        // A tag that named an attribute twice.
        "<script nonce=abc nonce=abc>out = 'injected'</script>",
        "<script id=a nonce=abc id=b>out = 'injected'</script>",
    ] {
        let (ran, said) = under(&["script-src 'nonce-abc'"], html);
        assert_eq!(ran, "no script ran", "{html}");
        assert_eq!(said.len(), 1, "{html}: {said:?}");
    }
}

#[test]
fn a_meta_policy_governs_the_scripts_after_it_and_not_before() {
    let (ran, said) = under(
        &[],
        "<head><script>out = 'a'</script>\
         <meta http-equiv=Content-Security-Policy content=\"script-src 'nonce-n'\">\
         <script>out += 'x'</script><script nonce=n>out += 'b'</script></head>\
         <body><script>out += 'y'</script></body>",
    );
    assert_eq!(ran, "ab");
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(
        said.first()
            .is_some_and(|it| it.starts_with("script 2: refused"))
    );
    assert!(
        said.get(1)
            .is_some_and(|it| it.starts_with("script 4: refused"))
    );
}

#[test]
fn a_meta_policy_narrows_the_headers_and_never_widens_them() {
    let (ran, _) = under(
        &["script-src 'none'"],
        "<head><meta http-equiv=Content-Security-Policy \
         content=\"script-src 'unsafe-inline'\"></head>\
         <script>out = 'ran'</script>",
    );
    assert_eq!(ran, "no script ran");
}

#[test]
fn a_meta_policy_outside_head_is_not_a_policy() {
    let (ran, said) = under(
        &[],
        "<body><meta http-equiv=Content-Security-Policy content=\"script-src 'none'\">\
         <script>out = 'ran'</script></body>",
    );
    assert_eq!(ran, "ran", "{said:?}");
}

#[test]
fn a_policy_this_engine_cannot_read_refuses_rather_than_allows() {
    // A misspelt keyword is a source that matches nothing, not a reason to
    // fall back to no policy at all.
    for policy in ["script-src 'unsafe-inlined'", "script-src \u{1}"] {
        let (ran, said) = under(&[policy], "<script>out = 'ran'</script>");
        assert_eq!(ran, "no script ran", "{policy}");
        assert_eq!(said.len(), 1, "{policy}: {said:?}");
    }
}

// --- A page's life -----------------------------------------------------------

#[test]
fn a_resize_lays_the_page_out_again_and_runs_none_of_its_script() {
    let (mut renderer, issues) = load(page(
        "<script>out = (this.out === undefined ? '' : this.out) + 'x'; throw 'once'</script>",
    ));
    assert_eq!(about_scripts(&issues), vec!["script 1: uncaught: \"once\""]);
    // Mark the page's own realm, so that a resize which made a new one —
    // and ran the script again in it — is told apart from one that kept it.
    let marked = renderer
        .event_loop()
        .and_then(|looping| {
            let program = script("this.out += '|'").ok()?;
            looping.engine().evaluate(&program).ok()
        })
        .is_some();
    assert!(marked, "the page's realm could not be marked");

    let resized = renderer.handle(ToRenderer::Resize(Size {
        width: 300.0,
        height: 150.0,
    }));
    match resized {
        FromRenderer::Loaded { issues, .. } => {
            assert_eq!(about_scripts(&issues), Vec::<&str>::new());
        }
        other => panic!("a resize did not lay out: {other:?}"),
    }
    assert_eq!(out(&mut renderer), "x|");
}

#[test]
fn a_new_page_starts_with_nothing_the_last_one_left() {
    let mut renderer = Renderer::new(FontDatabase::new());
    let first = renderer.handle(ToRenderer::Load(Box::new(page(
        "<script>out = 'first'</script>",
    ))));
    assert!(matches!(first, FromRenderer::Loaded { .. }));
    assert_eq!(out(&mut renderer), "first");

    let second = renderer.handle(ToRenderer::Load(Box::new(page(
        "<script>out = this.out === undefined ? 'fresh' : 'leaked: ' + this.out</script>",
    ))));
    assert!(matches!(second, FromRenderer::Loaded { .. }));
    assert_eq!(out(&mut renderer), "fresh");

    let third = renderer.handle(ToRenderer::Load(Box::new(page("<p>no script</p>"))));
    assert!(matches!(third, FromRenderer::Loaded { .. }));
    assert!(
        renderer.event_loop().is_none(),
        "the last page's loop outlived it"
    );
}

#[test]
fn a_page_still_renders_when_its_script_fails() {
    let (renderer, issues) = load(
        page("<p>still here</p><script>throw new Error('broken')</script>")
            .with_sheet("p { height: 20px; margin: 0 }"),
    );
    assert_eq!(
        about_scripts(&issues),
        vec!["script 1: uncaught: an object"]
    );
    let Some(drawn) = renderer.rendered() else {
        panic!("nothing was rendered");
    };
    let paragraph = drawn.boxes.ids().find(|id| {
        drawn
            .boxes
            .get(*id)
            .and_then(|boxed| boxed.kind.node())
            .and_then(|node| drawn.document.element(node))
            .is_some_and(|element| element.name.is_html("p"))
    });
    let Some(laid) = paragraph.and_then(|id| drawn.layout.border_box(id)) else {
        panic!("the paragraph is not laid out");
    };
    assert_eq!((laid.size.width, laid.size.height), (184.0, 20.0));
    assert_eq!((laid.origin.x, laid.origin.y), (8.0, 8.0));
}

// --- Hostile pages -----------------------------------------------------------

#[test]
fn every_prefix_of_a_page_with_scripts_loads_or_says_why() {
    let whole = "<head><meta http-equiv=Content-Security-Policy \
                 content=\"script-src 'nonce-q' 'sha256-abc='\">\
                 <script nonce=q>out = 'a'; queueMicrotask(() => { out += `${1 + 1}`; \
                 throw new RangeError('r') })</script></head>\
                 <body><script nonce=q type=text/javascript>for (const x of [1, 2]) \
                 { try { out += x } catch (e) {} finally { out += ';' } }</script>\
                 <script src=x.js></script><script type=module>import 'y'</script></body>";
    let mut cuts = 0_usize;
    for (at, _) in whole.char_indices() {
        let Some(prefix) = whole.get(..at) else {
            continue;
        };
        let (_renderer, issues) = load(page(prefix));
        assert!(
            !issues.iter().any(|issue| issue.starts_with("not loaded")),
            "cut at {at}: {issues:?}"
        );
        cuts += 1;
    }
    assert!(cuts > 300);
    let (mut renderer, issues) = load(page(whole));
    assert_eq!(out(&mut renderer), "a21;2;", "{issues:?}");
}

/// The renderer binary, waiting a short time for an answer.
fn renderers(patience: Duration) -> Renderers {
    Renderers::running(env!("CARGO_BIN_EXE_alo-render"), &[]).waiting_at_most(patience)
}

fn site(text: &str) -> Option<Site> {
    alo_url::parse(text).ok().map(|url| Site::of(&url))
}

#[test]
fn a_script_that_never_ends_costs_its_renderer_and_not_the_browser() {
    let patience = Duration::from_millis(400);
    let mut renderers = renderers(patience);
    let (Some(endless), Some(elsewhere)) = (
        site("https://endless.example/"),
        site("https://elsewhere.example/"),
    ) else {
        panic!("the two sites are not URLs");
    };

    let began = Instant::now();
    let asked = renderers.ask(
        &endless,
        &ToRenderer::Load(Box::new(page(
            "<p>hello</p><script>while (true) {}</script>",
        ))),
    );
    let waited = began.elapsed();
    let Err(gone) = asked else {
        panic!("a page whose script never ends finished loading: {asked:?}");
    };
    assert!(gone.why.contains("said nothing"), "{gone:?}");
    assert!(waited < patience * 8, "it waited {waited:?}");

    // Another site's renderer is untouched, and the same site can be loaded
    // again with a page that ends.
    let other = renderers.ask(
        &elsewhere,
        &ToRenderer::Load(Box::new(page("<script>let a = 1</script>"))),
    );
    assert!(
        matches!(other, Ok(FromRenderer::Loaded { .. })),
        "{other:?}"
    );
    let again = renderers.ask(
        &endless,
        &ToRenderer::Load(Box::new(page("<p>calmer now</p>"))),
    );
    assert!(
        matches!(again, Ok(FromRenderer::Loaded { .. })),
        "{again:?}"
    );
}
