/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 343 (ADR 0034): a page's `style-src`, applied to its inline
//! style, over a real load.
//!
//! *Under `style-src 'self'`, a `<style>` and a `style` attribute are both
//! refused and reported. The same attribute is applied under a digest with
//! `'unsafe-hashes'`. A value written through `element.style` is applied
//! under the refusing policy.* And from the ADR: the record compared by
//! value, a `setAttribute` replacing what the declaration wrote refused, and
//! a refused attribute read as `""` through `element.style`.
//!
//! Each test loads a page into a renderer and reads what was drawn — the
//! computed colour of an element, in its own words — and what the load
//! answered: its issues and the objections the browser process would post.

use alo_js::interpret::Trouble;
use alo_js::{Value, script};
use alo_layout::Size;
use alo_net::csp::{Inline, Placement};
use alo_renderer::violations::Objection;
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;

/// A small window; nothing here is about where anything is.
const WINDOW: Size = Size {
    width: 200.0,
    height: 100.0,
};

/// The colour of a `<p>` nothing styled: the user-agent sheet's.
const UNSTYLED: &str = "black";

/// SHA-256 of `color: red`, in base64, as a policy names it.
const COLOR_RED: &str = "'sha256-NerDAUWfwD31YdZHveMrq0GLjsNFMwxLpZl0dPUeCcw='";

/// What a load answered, taken apart.
struct Answer {
    issues: Vec<String>,
    objections: Vec<Objection>,
}

/// Load a page into a renderer in this process.
fn load(page: Page) -> (Renderer, Answer) {
    let mut renderer = Renderer::new(FontDatabase::new());
    let answer = match renderer.handle(ToRenderer::Load(Box::new(page))) {
        FromRenderer::Loaded {
            issues, objections, ..
        } => Answer { issues, objections },
        other => Answer {
            issues: vec![format!("not loaded: {other:?}")],
            objections: Vec::new(),
        },
    };
    (renderer, answer)
}

/// The colour drawn for the element whose `id` is `wanted`, as computed —
/// or what stood in the way of reading it.
fn colour(renderer: &mut Renderer, wanted: &str) -> String {
    // Read the rendering, which draws it again if a script changed it.
    renderer.handle(ToRenderer::Paint);
    let Some(document) = renderer.document() else {
        return "no document".to_owned();
    };
    let Some(id) = document.descendants(document.root()).find(|id| {
        document
            .element(*id)
            .is_some_and(|element| element.attr("id") == Some(wanted))
    }) else {
        return format!("no #{wanted}");
    };
    let Some(drawing) = renderer.rendered() else {
        return "not drawn".to_owned();
    };
    drawing
        .styles
        .get(id)
        .and_then(|style| style.get("color"))
        .unwrap_or("none")
        .to_owned()
}

/// What the global `out` holds, or why not.
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

/// Run `source` on the page as a script of the page's own would be, after
/// its load: a task on its loop, then its jobs.
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

fn style(placement: Placement) -> Objection {
    Objection {
        policy: 0,
        kind: Inline::Style,
        placement,
    }
}

#[test]
fn under_style_src_self_a_style_and_a_style_attribute_are_refused_and_reported() {
    let page = Page::new(
        r#"<style>#a { color: red }</style><p id=a>x</p><p id=b style="color: blue">y</p>"#,
        WINDOW,
    )
    .with_policy("style-src 'self'; report-uri /csp");
    let (mut renderer, answer) = load(page);
    assert_eq!(colour(&mut renderer, "a"), UNSTYLED, "the <style> applied");
    assert_eq!(
        colour(&mut renderer, "b"),
        UNSTYLED,
        "the attribute applied"
    );
    assert_eq!(
        answer.objections,
        vec![style(Placement::Element), style(Placement::Attribute)],
    );
    let refusals: Vec<_> = answer
        .issues
        .iter()
        .filter(|issue| issue.contains("is not applied"))
        .collect();
    assert_eq!(refusals.len(), 2, "{:?}", answer.issues);
    assert!(refusals[0].contains("does not allow inline style"));
    assert!(refusals[1].contains("does not allow a style attribute"));
    assert!(refusals[1].contains("style-src"));
}

#[test]
fn the_same_page_under_no_policy_is_drawn_as_it_always_was() {
    let page = Page::new(
        r#"<style>#a { color: red }</style><p id=a>x</p><p id=b style="color: blue">y</p>"#,
        WINDOW,
    );
    let (mut renderer, answer) = load(page);
    assert_eq!(colour(&mut renderer, "a"), "red");
    assert_eq!(colour(&mut renderer, "b"), "blue");
    assert!(answer.objections.is_empty());
}

#[test]
fn a_style_attribute_is_applied_under_its_digest_only_with_unsafe_hashes() {
    let markup = r#"<p id=a style="color: red">x</p>"#;
    let (mut renderer, answer) = load(
        Page::new(markup, WINDOW).with_policy(format!("style-src 'unsafe-hashes' {COLOR_RED}")),
    );
    assert_eq!(colour(&mut renderer, "a"), "red");
    assert!(answer.objections.is_empty());
    let (mut renderer, _) =
        load(Page::new(markup, WINDOW).with_policy(format!("style-src {COLOR_RED}")));
    assert_eq!(colour(&mut renderer, "a"), UNSTYLED);
}

#[test]
fn a_style_presenting_the_policys_nonce_is_applied() {
    let page = Page::new(
        "<style nonce=abc>#a { color: red }</style><style>#b { color: red }</style>\
         <p id=a>x</p><p id=b>y</p>",
        WINDOW,
    )
    .with_policy("style-src 'nonce-abc'");
    let (mut renderer, _) = load(page);
    assert_eq!(colour(&mut renderer, "a"), "red");
    assert_eq!(colour(&mut renderer, "b"), UNSTYLED);
}

#[test]
fn a_value_written_through_element_style_is_applied_under_the_refusing_policy() {
    let page = Page::new(
        "<p id=a>x</p><script>\
         document.getElementById('a').style.color = 'blue';\
         out = document.getElementById('a').style.color</script>",
        WINDOW,
    )
    .with_policy("style-src 'none'");
    let (mut renderer, answer) = load(page);
    assert_eq!(out(&mut renderer), "blue");
    assert_eq!(colour(&mut renderer, "a"), "blue");
    assert!(
        answer.objections.is_empty(),
        "what the declaration wrote is not reported: {:?}",
        answer.objections
    );
}

#[test]
fn a_refused_attribute_reads_as_empty_and_a_write_starts_from_nothing() {
    let page = Page::new(
        r#"<p id=a style="color: red; margin: 0">x</p><script>
         const p = document.getElementById('a');
         out = p.style.cssText + '|' + p.style.color + '|' + p.style.length + '|' +
           p.getAttribute('style');
         </script>"#,
        WINDOW,
    )
    .with_policy("style-src 'self'");
    let (mut renderer, _) = load(page);
    assert_eq!(out(&mut renderer), "||0|color: red; margin: 0");
    assert_eq!(
        run(
            &mut renderer,
            "const q = document.getElementById('a'); q.style.width = '10px'; \
             out = q.getAttribute('style')"
        ),
        ""
    );
    assert_eq!(
        out(&mut renderer),
        "width: 10px;",
        "the injected colour was not adopted"
    );
    assert_eq!(colour(&mut renderer, "a"), UNSTYLED);
}

#[test]
fn a_set_attribute_replacing_what_the_declaration_wrote_is_refused() {
    let page = Page::new(
        "<p id=a>x</p><script>document.getElementById('a').style.color = 'blue'</script>",
        WINDOW,
    )
    .with_policy("style-src 'none'");
    let (mut renderer, _) = load(page);
    assert_eq!(colour(&mut renderer, "a"), "blue");
    run(
        &mut renderer,
        "document.getElementById('a').setAttribute('style', 'color: green')",
    );
    assert_eq!(
        colour(&mut renderer, "a"),
        UNSTYLED,
        "a value the declaration did not write is judged by the policy, and the old style \
         is gone with it (ADR 0034, what this costs)",
    );
    run(
        &mut renderer,
        "document.getElementById('a').setAttribute('style', 'color: blue;')",
    );
    assert_eq!(
        colour(&mut renderer, "a"),
        "blue",
        "the exact text the declaration wrote is still its own",
    );
}

#[test]
fn a_meta_policy_reaches_back_over_style_before_it_and_the_page_is_told() {
    let page = Page::new(
        r#"<head><style>#a { color: red }</style>
           <meta http-equiv="Content-Security-Policy" content="style-src 'none'"></head>
           <body><p id=a>x</p><p id=b style="color: blue">y</p>
           <script>out = document.getElementById('b').style.color</script></body>"#,
        WINDOW,
    );
    let (mut renderer, answer) = load(page);
    assert_eq!(colour(&mut renderer, "a"), UNSTYLED);
    assert_eq!(colour(&mut renderer, "b"), UNSTYLED);
    assert_eq!(out(&mut renderer), "", "the heap holds the <meta> policy");
    assert!(
        answer.objections.is_empty(),
        "a <meta> policy's objections do not cross: {:?}",
        answer.objections
    );
}

#[test]
fn a_meta_policy_after_the_last_script_is_still_held() {
    let page = Page::new(
        r#"<head><script>out = 'ran'</script>
           <meta http-equiv="Content-Security-Policy" content="style-src 'none'"></head>
           <body><p id=a style="color: blue">y</p></body>"#,
        WINDOW,
    );
    let (mut renderer, _) = load(page);
    assert_eq!(out(&mut renderer), "ran");
    assert_eq!(colour(&mut renderer, "a"), UNSTYLED);
    assert_eq!(
        run(
            &mut renderer,
            "out = document.getElementById('a').style.color"
        ),
        ""
    );
    assert_eq!(
        out(&mut renderer),
        "",
        "element.style asks the same policies"
    );
}

#[test]
fn a_watched_policy_refuses_nothing_and_is_still_told() {
    let page = Page::new(r#"<p id=a style="color: blue">y</p>"#, WINDOW)
        .watched_by("style-src 'none'; report-uri /watched");
    let (mut renderer, answer) = load(page);
    assert_eq!(colour(&mut renderer, "a"), "blue");
    assert_eq!(answer.objections, vec![style(Placement::Attribute)]);
    assert!(
        answer
            .issues
            .iter()
            .any(|issue| issue.contains("is applied, but a policy being watched")),
        "{:?}",
        answer.issues
    );
}

#[test]
fn a_page_with_more_refused_style_than_a_load_carries_says_how_many() {
    let markup = r#"<p style="color: red">x</p>"#.repeat(100);
    let (_, answer) = load(Page::new(markup, WINDOW).with_policy("style-src 'none'"));
    assert_eq!(answer.objections.len(), 64);
    assert!(
        answer
            .issues
            .iter()
            .any(|issue| issue.starts_with("36 more policy objections to this page's inline style")),
        "{:?}",
        answer.issues
    );
}
