/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Queue item 325, ADR 0030 § 4: the browser process composes what this
//! browser says it is, and the renderer is **told**.
//!
//! *A page's `navigator.userAgent` is the string `alo-net` sends as the
//! `User-Agent` header, and its `navigator.platform` the platform beside it;
//! a page told something else answers what it was told, after the crossing
//! between processes as before it.*
//!
//! What a script leaves in its globals is read from the page's own engine
//! through [`Renderer::event_loop`], as `a_page_runs_its_scripts.rs` does.

use alo_js::interpret::Trouble;
use alo_js::{Value, script};
use alo_layout::Size;
use alo_renderer::wire::{read_to_renderer, write_to_renderer};
use alo_renderer::{FromRenderer, Page, Renderer, ToRenderer};
use alo_text::FontDatabase;

/// The script that says what the page was told.
const SAYS: &str = "<script>out = navigator.userAgent + '|' + navigator.platform</script>";

/// What the global `out` holds after `page` loaded, or why it could not be
/// read.
fn told(page: Page) -> String {
    let mut renderer = Renderer::new(FontDatabase::new());
    let FromRenderer::Loaded { issues, .. } = renderer.handle(ToRenderer::Load(Box::new(page)))
    else {
        return "not loaded".to_owned();
    };
    if let Some(issue) = issues.iter().find(|issue| issue.starts_with("script ")) {
        return issue.clone();
    }
    let Some(looping) = renderer.event_loop() else {
        return "no script ran".to_owned();
    };
    let program = match script("this.out") {
        Ok(program) => program,
        Err(why) => return format!("did not parse: {why}"),
    };
    match looping.engine().evaluate(&program) {
        Ok(Value::Text(held)) => looping.engine().objects().units(held).map_or_else(
            || "a string that has gone".to_owned(),
            String::from_utf16_lossy,
        ),
        Ok(other) => format!("{other:?}"),
        Err(Trouble::Escaped(escape)) => format!("! {escape}"),
        Err(Trouble::NotCompiled(refusal)) => format!("? {refusal}"),
    }
}

#[test]
fn a_page_reads_the_header_the_browser_sends() {
    let page = Page::new(SAYS, Size::new(100.0, 50.0));
    assert_eq!(page.user_agent, alo_net::user_agent::user_agent());
    assert_eq!(page.platform, alo_net::user_agent::platform());
    assert_eq!(
        told(page),
        format!(
            "{}|{}",
            alo_net::user_agent::user_agent(),
            alo_net::user_agent::platform()
        ),
    );
}

#[test]
fn a_page_told_something_else_answers_what_it_was_told_across_the_crossing() {
    let mut page = Page::new(SAYS, Size::new(100.0, 50.0));
    page.user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) alo/0.0".to_owned();
    page.platform = "Win32".to_owned();
    let crossed = match read_to_renderer(&write_to_renderer(&ToRenderer::Load(Box::new(page)))) {
        Ok(ToRenderer::Load(page)) => *page,
        other => panic!("the page did not cross: {other:?}"),
    };
    assert_eq!(
        told(crossed),
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) alo/0.0|Win32",
    );
}
